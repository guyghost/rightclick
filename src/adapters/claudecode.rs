//! Claude Code adapter
//!
//! Integrates with Anthropic's Claude Code CLI tool. Claude Code stores one
//! JSONL transcript per session, flat under
//! `~/.claude/projects/{encode_path(project_root)}/<session-uuid>.jsonl`.
//! Each line is a JSON object whose `type` discriminates the entry kind
//! (`user`, `assistant`, `file-history-snapshot`, `progress`, ...); the actual
//! message payload lives under a nested `message` object.

use crate::adapters::types::{Adapter, AdapterError, AdapterType, Result, encode_path};
use crate::core::models::conversation::{ContentBlock, Message, Role, Session, TokenUsage};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Claude Code adapter implementation
#[derive(Debug)]
pub struct ClaudeCodeAdapter {
    /// Base directory for Claude Code storage
    base_dir: PathBuf,
}

impl ClaudeCodeAdapter {
    /// Create a new Claude Code adapter with default paths
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| AdapterError::Other("Could not find home directory".to_string()))?;

        Ok(Self {
            base_dir: home_dir.join(".claude"),
        })
    }

    /// Create a new Claude Code adapter with a custom base directory
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Get the projects directory path (`~/.claude/projects`)
    fn projects_dir(&self) -> PathBuf {
        self.base_dir.join("projects")
    }

    /// Get the project storage directory for a given project root.
    ///
    /// This is `~/.claude/projects/{encode_path(project_root)}`, the directory
    /// that holds the flat `<session-uuid>.jsonl` transcripts.
    fn project_dir(&self, project_root: &Path) -> PathBuf {
        let encoded = encode_path(project_root);
        self.projects_dir().join(encoded)
    }

    /// Legacy `sessions/` subdir used by older Claude Code layouts.
    fn legacy_sessions_dir(&self, project_root: &Path) -> PathBuf {
        self.project_dir(project_root).join("sessions")
    }

    /// Parse a transcript file into renderable messages.
    async fn parse_messages_file(&self, path: &Path, session_id: &str) -> Result<Vec<Message>> {
        let content = tokio::fs::read_to_string(path).await?;
        Ok(parse_transcript(&content, session_id))
    }

    /// Parse a legacy `conversation.jsonl` (flat top-level `role`/`content`).
    async fn parse_legacy_messages(&self, path: &Path, session_id: &str) -> Result<Vec<Message>> {
        let content = tokio::fs::read_to_string(path).await?;
        Ok(parse_legacy_transcript(&content, session_id))
    }

    /// Enumerate legacy `sessions/<id>/` directories (older layout).
    async fn legacy_sessions(&self, project_root: &Path) -> Result<Vec<Session>> {
        let sessions_dir = self.legacy_sessions_dir(project_root);
        if !sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        let mut entries = tokio::fs::read_dir(&sessions_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_dir() {
                continue;
            }

            let session_id = entry.file_name().to_string_lossy().to_string();
            let session_dir = entry.path();

            let (name, created_at, updated_at) =
                match tokio::fs::read_to_string(session_dir.join("metadata.json")).await {
                    Ok(content) => match serde_json::from_str::<ClaudeSessionMetadata>(&content) {
                        Ok(metadata) => (
                            metadata.name.unwrap_or_else(|| session_id.clone()),
                            metadata.created_at,
                            metadata.updated_at,
                        ),
                        Err(_) => (session_id.clone(), Utc::now(), Utc::now()),
                    },
                    Err(_) => (session_id.clone(), Utc::now(), Utc::now()),
                };

            let conversation_path = session_dir.join("conversation.jsonl");
            let message_count = if conversation_path.exists() {
                tokio::fs::read_to_string(&conversation_path)
                    .await
                    .map(|c| inspect_transcript(&c).0)
                    .unwrap_or(0)
            } else {
                0
            };

            let mut session = Session::new(&session_id, name, self.id(), created_at);
            session.created_at = created_at;
            session.updated_at = updated_at;
            session.message_count = message_count;
            sessions.push(session);
        }

        Ok(sessions)
    }
}

impl Default for ClaudeCodeAdapter {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            base_dir: PathBuf::from(".claude"),
        })
    }
}

#[async_trait]
impl Adapter for ClaudeCodeAdapter {
    fn id(&self) -> &str {
        "claude-code"
    }

    fn name(&self) -> &str {
        "Claude Code"
    }

    fn icon(&self) -> char {
        '◈'
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::ClaudeCode
    }

    async fn detect(&self, project_root: &Path) -> Result<bool> {
        // Primary: flat `<uuid>.jsonl` transcripts in the encoded project dir.
        let project_dir = self.project_dir(project_root);
        if project_dir.exists() && has_flat_transcripts(&project_dir).await {
            return Ok(true);
        }

        // Backward compat: older layouts used a non-empty `sessions/` subdir.
        let sessions_dir = self.legacy_sessions_dir(project_root);
        if sessions_dir.exists() {
            let mut entries = tokio::fs::read_dir(&sessions_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if entry.file_type().await?.is_dir() {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    async fn sessions(&self, project_root: &Path) -> Result<Vec<Session>> {
        let project_dir = self.project_dir(project_root);
        let mut sessions = Vec::new();

        // Primary: enumerate flat `<uuid>.jsonl` transcripts.
        if project_dir.exists() {
            let mut entries = tokio::fs::read_dir(&project_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if !is_jsonl_file(&path).await {
                    continue;
                }

                let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let id = id.to_string();

                let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
                let (message_count, first_user_text) = inspect_transcript(&content);
                let name = first_user_text.unwrap_or_else(|| id.clone());
                let (created_at, updated_at) = file_times(&path).await;

                let mut session = Session::new(&id, name, self.id(), created_at);
                session.created_at = created_at;
                session.updated_at = updated_at;
                session.message_count = message_count;
                sessions.push(session);
            }
        }

        // Backward compat: older `sessions/<id>/conversation.jsonl` layout.
        if sessions.is_empty() {
            sessions = self.legacy_sessions(project_root).await?;
        }

        // Sort by updated_at, newest first
        sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_at));

        Ok(sessions)
    }

    async fn messages(&self, session_id: &str) -> Result<Vec<Message>> {
        // Scan all project directories for the requested session transcript.
        let projects_dir = self.projects_dir();
        if !projects_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = tokio::fs::read_dir(&projects_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_dir() {
                continue;
            }

            let dir = entry.path();

            // Primary: flat `<id>.jsonl`.
            let flat = dir.join(format!("{}.jsonl", session_id));
            if flat.exists() {
                return self.parse_messages_file(&flat, session_id).await;
            }

            // Backward compat: `sessions/<id>/conversation.jsonl`.
            let legacy = dir
                .join("sessions")
                .join(session_id)
                .join("conversation.jsonl");
            if legacy.exists() {
                return self.parse_legacy_messages(&legacy, session_id).await;
            }
        }

        Ok(Vec::new())
    }

    async fn usage(&self, session_id: &str) -> Result<Option<TokenUsage>> {
        let messages = self.messages(session_id).await?;

        let mut total_prompt = 0usize;
        let mut total_completion = 0usize;
        let mut has_usage = false;

        for msg in &messages {
            if let Some(usage) = &msg.tokens {
                total_prompt += usage.prompt_tokens;
                total_completion += usage.completion_tokens;
                has_usage = true;
            }
        }

        if has_usage {
            Ok(Some(TokenUsage::new(total_prompt, total_completion)))
        } else {
            Ok(None)
        }
    }
}

/// Claude session metadata structure (legacy `metadata.json` layout)
#[derive(Debug, Deserialize)]
struct ClaudeSessionMetadata {
    name: Option<String>,
    #[serde(with = "chrono::serde::ts_seconds")]
    created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    updated_at: DateTime<Utc>,
}

/// A single JSONL line in a Claude Code transcript.
#[derive(Debug, Deserialize)]
struct ClaudeLine {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    message: Option<ClaudeMessage>,
}

/// The `message` body shared by `user`/`assistant` transcript lines.
#[derive(Debug, Deserialize)]
struct ClaudeMessage {
    role: Option<String>,
    #[serde(default, deserialize_with = "deserialize_blocks")]
    content: Vec<ClaudeContentBlock>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<ClaudeTokenUsage>,
}

impl ClaudeMessage {
    /// First textual snippet, used as a best-effort session display name.
    fn first_text(&self) -> Option<String> {
        self.content
            .iter()
            .find_map(|block| block.as_text())
            .map(|s| s.to_string())
    }

    fn into_message(self, id: String) -> Message {
        let role = match self.role.as_deref() {
            Some("assistant") => Role::Assistant,
            Some("system") => Role::System,
            _ => Role::User,
        };

        let content = self
            .content
            .iter()
            .filter_map(|block| block.as_text())
            .collect::<Vec<_>>()
            .join("\n");

        let content_blocks = self
            .content
            .into_iter()
            .filter_map(|block| block.into_content_block())
            .collect();

        let tokens = self
            .usage
            .map(|u| TokenUsage::new(u.input_tokens, u.output_tokens));

        Message {
            id,
            role,
            content,
            timestamp: Utc::now(),
            model: self.model,
            tool_uses: Vec::new(),
            content_blocks,
            tokens,
            is_streaming: false,
            metadata: None,
        }
    }
}

/// Claude content block (Anthropic message content shapes).
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClaudeContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
    #[serde(rename = "code")]
    Code {
        #[serde(default)]
        language: Option<String>,
        code: String,
    },
}

impl ClaudeContentBlock {
    fn as_text(&self) -> Option<&str> {
        match self {
            ClaudeContentBlock::Text { text } => Some(text),
            ClaudeContentBlock::Thinking { thinking } => Some(thinking),
            _ => None,
        }
    }

    fn into_content_block(self) -> Option<ContentBlock> {
        match self {
            ClaudeContentBlock::Text { text } => Some(ContentBlock::Text { content: text }),
            ClaudeContentBlock::Thinking { thinking } => Some(ContentBlock::Markdown {
                content: format!("*Thinking:* {}", thinking),
            }),
            ClaudeContentBlock::ToolUse { id, name, input } => Some(ContentBlock::ToolUse {
                id,
                name,
                input: input.to_string(),
            }),
            ClaudeContentBlock::ToolResult {
                tool_use_id,
                content,
            } => Some(ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error: false,
            }),
            ClaudeContentBlock::Code { language, code } => Some(ContentBlock::Code {
                language,
                code,
                file_path: None,
            }),
        }
    }
}

/// Claude token usage
#[derive(Debug, Deserialize)]
struct ClaudeTokenUsage {
    input_tokens: usize,
    output_tokens: usize,
}

/// Deserialize `message.content`, which may be a plain string or an array of
/// content blocks. Unknown block variants are skipped rather than failing to
/// deserialize the entire message.
fn deserialize_blocks<'de, D>(
    deserializer: D,
) -> std::result::Result<Vec<ClaudeContentBlock>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value =
        Option::<serde_json::Value>::deserialize(deserializer)?.unwrap_or(serde_json::Value::Null);
    match value {
        serde_json::Value::Null => Ok(Vec::new()),
        serde_json::Value::String(s) => Ok(vec![ClaudeContentBlock::Text { text: s }]),
        serde_json::Value::Array(arr) => {
            let mut blocks = Vec::with_capacity(arr.len());
            for item in arr {
                if let Ok(block) = serde_json::from_value::<ClaudeContentBlock>(item) {
                    blocks.push(block);
                }
            }
            Ok(blocks)
        }
        _ => Ok(Vec::new()),
    }
}

/// True when `path` is a regular `.jsonl` file.
async fn is_jsonl_file(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()), Some("jsonl"))
        && tokio::fs::metadata(path)
            .await
            .is_ok_and(|metadata| metadata.is_file())
}

/// True when `dir` contains at least one flat `*.jsonl` transcript.
async fn has_flat_transcripts(dir: &Path) -> bool {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return false,
    };
    loop {
        match entries.next_entry().await {
            Ok(Some(entry)) => {
                if is_jsonl_file(&entry.path()).await {
                    return true;
                }
            }
            _ => return false,
        }
    }
}

/// Best-effort `(created, updated)` timestamps from file metadata.
async fn file_times(path: &Path) -> (DateTime<Utc>, DateTime<Utc>) {
    let metadata = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(_) => return (Utc::now(), Utc::now()),
    };

    let to_dt = |time: Option<std::time::SystemTime>| -> DateTime<Utc> {
        time.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| DateTime::from_timestamp(d.as_secs() as i64, d.subsec_nanos()))
            .unwrap_or_else(Utc::now)
    };

    let updated = to_dt(metadata.modified().ok());
    let created = to_dt(metadata.created().ok().or(metadata.modified().ok()));
    (created, updated)
}

/// Returns `(non_empty_line_count, first_user_text)` for a transcript.
fn inspect_transcript(content: &str) -> (usize, Option<String>) {
    let mut count = 0usize;
    let mut first_user_text: Option<String> = None;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        count += 1;

        if first_user_text.is_some() {
            continue;
        }

        if let Ok(line) = serde_json::from_str::<ClaudeLine>(line) {
            if line.kind.as_deref() == Some("user") {
                if let Some(body) = line.message {
                    first_user_text = body.first_text();
                }
            }
        }
    }

    (count, first_user_text)
}

/// Parse a transcript into messages, keeping only `user`/`assistant`/`system`
/// lines that carry a `message` body.
fn parse_transcript(content: &str, session_id: &str) -> Vec<Message> {
    let mut messages = Vec::new();
    let mut index = 0usize;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let parsed: ClaudeLine = match serde_json::from_str(line) {
            Ok(parsed) => parsed,
            Err(e) => {
                tracing::warn!("Failed to parse Claude transcript line: {}", e);
                continue;
            }
        };

        match parsed.kind.as_deref() {
            Some("user") | Some("assistant") | Some("system") => {}
            _ => continue,
        }

        if let Some(body) = parsed.message {
            messages.push(body.into_message(format!("{}-{}", session_id, index)));
            index += 1;
        }
    }

    messages
}

/// Parse a legacy `conversation.jsonl` whose lines carry top-level
/// `role`/`content`/`content_blocks`/`model`/`usage` fields.
fn parse_legacy_transcript(content: &str, session_id: &str) -> Vec<Message> {
    let mut messages = Vec::new();
    let mut index = 0usize;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };

        let role = match value.get("role").and_then(|r| r.as_str()).unwrap_or("user") {
            "assistant" => Role::Assistant,
            "system" => Role::System,
            _ => Role::User,
        };

        let mut blocks = match value.get("content") {
            Some(serde_json::Value::String(s)) => {
                vec![ClaudeContentBlock::Text { text: s.clone() }]
            }
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .filter_map(|item| serde_json::from_value::<ClaudeContentBlock>(item.clone()).ok())
                .collect(),
            _ => Vec::new(),
        };
        if let Some(extra) = value
            .get("content_blocks")
            .and_then(|v| serde_json::from_value::<Vec<ClaudeContentBlock>>(v.clone()).ok())
        {
            blocks.extend(extra);
        }

        let content = blocks
            .iter()
            .filter_map(|block| block.as_text())
            .collect::<Vec<_>>()
            .join("\n");
        let content_blocks = blocks
            .into_iter()
            .filter_map(|block| block.into_content_block())
            .collect();

        let model = value
            .get("model")
            .and_then(|m| m.as_str())
            .map(String::from);
        let tokens = value
            .get("usage")
            .and_then(|u| serde_json::from_value::<ClaudeTokenUsage>(u.clone()).ok())
            .map(|u| TokenUsage::new(u.input_tokens, u.output_tokens));

        messages.push(Message {
            id: format!("{}-{}", session_id, index),
            role,
            content,
            timestamp: Utc::now(),
            model,
            tool_uses: Vec::new(),
            content_blocks,
            tokens,
            is_streaming: false,
            metadata: None,
        });
        index += 1;
    }

    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_adapter() -> (ClaudeCodeAdapter, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let adapter = ClaudeCodeAdapter::with_base_dir(temp_dir.path().to_path_buf());
        (adapter, temp_dir)
    }

    /// Realistic transcript: user + assistant message lines plus a bookkeeping
    /// line that must be ignored when parsing messages but still counted.
    fn sample_transcript() -> &'static str {
        r#"{"type":"file-history-snapshot","snapshot":{}}
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Fix the git diff preview"}]}}
{"type":"assistant","message":{"role":"assistant","model":"claude-opus-4-6","content":[{"type":"thinking","thinking":"Planning the change."},{"type":"text","text":"On it."}],"usage":{"input_tokens":12,"output_tokens":34}}}
{"type":"progress","progress":{}}"#
    }

    #[tokio::test]
    async fn test_detect_no_data() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let detected = adapter.detect(project).await.unwrap();
        assert!(!detected);
    }

    #[tokio::test]
    async fn test_detect_flat_transcript() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let project_dir = adapter.project_dir(project);
        tokio::fs::create_dir_all(&project_dir).await.unwrap();
        tokio::fs::write(project_dir.join("abc.jsonl"), "{}\n")
            .await
            .unwrap();

        let detected = adapter.detect(project).await.unwrap();
        assert!(detected);
    }

    #[tokio::test]
    async fn test_detect_legacy_sessions_dir() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let sessions_dir = adapter.legacy_sessions_dir(project);
        tokio::fs::create_dir_all(sessions_dir.join("legacy-session"))
            .await
            .unwrap();

        let detected = adapter.detect(project).await.unwrap();
        assert!(detected);
    }

    #[tokio::test]
    async fn test_sessions_empty() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let sessions = adapter.sessions(project).await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_sessions_flat_layout() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let project_dir = adapter.project_dir(project);
        tokio::fs::create_dir_all(&project_dir).await.unwrap();
        tokio::fs::write(project_dir.join("sess-1.jsonl"), sample_transcript())
            .await
            .unwrap();

        let sessions = adapter.sessions(project).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "sess-1");
        assert_eq!(sessions[0].name, "Fix the git diff preview");
        // Four non-empty lines in the transcript.
        assert_eq!(sessions[0].message_count, 4);
    }

    #[tokio::test]
    async fn test_sessions_name_falls_back_to_id() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let project_dir = adapter.project_dir(project);
        tokio::fs::create_dir_all(&project_dir).await.unwrap();
        // No user line -> name should fall back to the file stem.
        tokio::fs::write(
            project_dir.join("only-snapshot.jsonl"),
            "{\"type\":\"file-history-snapshot\",\"snapshot\":{}}\n",
        )
        .await
        .unwrap();

        let sessions = adapter.sessions(project).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "only-snapshot");
    }

    #[tokio::test]
    async fn test_messages_flat_layout() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let project_dir = adapter.project_dir(project);
        tokio::fs::create_dir_all(&project_dir).await.unwrap();
        tokio::fs::write(project_dir.join("sess-1.jsonl"), sample_transcript())
            .await
            .unwrap();

        let messages = adapter.messages("sess-1").await.unwrap();
        // Only user + assistant lines become messages.
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[0].content, "Fix the git diff preview");
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[1].model.as_deref(), Some("claude-opus-4-6"));
        let tokens = messages[1].tokens.expect("assistant has usage");
        assert_eq!(tokens.total_tokens, 46);
    }

    #[tokio::test]
    async fn test_messages_legacy_conversation() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let session_dir = adapter.legacy_sessions_dir(project).join("legacy-session");
        tokio::fs::create_dir_all(&session_dir).await.unwrap();
        let conversation = r#"{"type":"message","role":"user","content":"Hello","timestamp":1700000000}
{"type":"message","role":"assistant","content":"Hi there!","timestamp":1700000001}"#;
        tokio::fs::write(session_dir.join("conversation.jsonl"), conversation)
            .await
            .unwrap();

        let messages = adapter.messages("legacy-session").await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[1].role, Role::Assistant);
    }

    #[tokio::test]
    async fn test_messages_corrupt_lines_skipped() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let project_dir = adapter.project_dir(project);
        tokio::fs::create_dir_all(&project_dir).await.unwrap();
        let transcript = r#"this is not json
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Hello"}]}}
{"broken":}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}]}}"#;
        tokio::fs::write(project_dir.join("corrupt.jsonl"), transcript)
            .await
            .unwrap();

        let messages = adapter.messages("corrupt").await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].content, "Hi!");
    }

    #[tokio::test]
    async fn test_messages_tool_use_and_result_blocks() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let project_dir = adapter.project_dir(project);
        tokio::fs::create_dir_all(&project_dir).await.unwrap();
        let transcript = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-1","name":"edit_file","input":{"file":"main.rs"}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool-1","content":"File updated"}]}}"#;
        tokio::fs::write(project_dir.join("tools.jsonl"), transcript)
            .await
            .unwrap();

        let messages = adapter.messages("tools").await.unwrap();
        assert_eq!(messages.len(), 2);
        // tool_use block should map to ContentBlock::ToolUse
        assert_eq!(messages[0].content_blocks.len(), 1);
        assert!(matches!(
            messages[0].content_blocks[0],
            ContentBlock::ToolUse { .. }
        ));
        // tool_result block maps to ContentBlock::ToolResult
        assert!(matches!(
            messages[1].content_blocks[0],
            ContentBlock::ToolResult { .. }
        ));
    }

    #[tokio::test]
    async fn test_messages_thinking_block_becomes_markdown() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let project_dir = adapter.project_dir(project);
        tokio::fs::create_dir_all(&project_dir).await.unwrap();
        let transcript = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Let me plan this."},{"type":"text","text":"Done."}]}}"#;
        tokio::fs::write(project_dir.join("think.jsonl"), transcript)
            .await
            .unwrap();

        let messages = adapter.messages("think").await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content_blocks.len(), 2);
        assert!(matches!(
            messages[0].content_blocks[0],
            ContentBlock::Markdown { .. }
        ));
    }

    #[tokio::test]
    async fn test_sessions_multi_transcript_ordering() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let project_dir = adapter.project_dir(project);
        tokio::fs::create_dir_all(&project_dir).await.unwrap();

        // Write two transcripts; both have identical content for deterministic ordering
        tokio::fs::write(project_dir.join("aaa.jsonl"), sample_transcript())
            .await
            .unwrap();
        tokio::fs::write(project_dir.join("bbb.jsonl"), sample_transcript())
            .await
            .unwrap();

        let sessions = adapter.sessions(project).await.unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[tokio::test]
    async fn test_usage_aggregated_from_messages() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let project_dir = adapter.project_dir(project);
        tokio::fs::create_dir_all(&project_dir).await.unwrap();
        tokio::fs::write(project_dir.join("usage.jsonl"), sample_transcript())
            .await
            .unwrap();

        let usage = adapter.usage("usage").await.unwrap();
        assert!(usage.is_some());
        // sample_transcript assistant has input_tokens:12, output_tokens:34
        assert_eq!(usage.unwrap().total_tokens, 46);
    }
}
