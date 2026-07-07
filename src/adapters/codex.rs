//! Codex CLI adapter
//!
//! Integrates with OpenAI's Codex CLI. Codex keeps a global session index at
//! `~/.codex/session_index.jsonl` (one `{id, thread_name, updated_at}` object
//! per line) and stores each session's rollout payload under
//! `~/.codex/sessions/YYYY/MM/DD/rollout-<ts>-<session-uuid>.jsonl`. Each
//! payload line is `{timestamp, type, payload}`; conversational messages are
//! `response_item` lines whose `payload.type` is `message`.

use crate::adapters::types::{Adapter, AdapterError, AdapterType, Result};
use crate::core::models::conversation::{ContentBlock, Message, Role, Session, TokenUsage};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Codex CLI adapter implementation
#[derive(Debug)]
pub struct CodexAdapter {
    /// Base directory for Codex storage
    base_dir: PathBuf,
}

impl CodexAdapter {
    /// Create a new Codex adapter with default paths
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| AdapterError::Other("Could not find home directory".to_string()))?;

        Ok(Self {
            base_dir: home_dir.join(".codex"),
        })
    }

    /// Create a new Codex adapter with a custom base directory
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Global session index: `~/.codex/session_index.jsonl`
    fn session_index_file(&self) -> PathBuf {
        self.base_dir.join("session_index.jsonl")
    }

    /// Sessions root: `~/.codex/sessions`
    fn sessions_root(&self) -> PathBuf {
        self.base_dir.join("sessions")
    }

    // --- Legacy (`project_mappings.json`) helpers --------------------------

    fn session_dir(&self, session_id: &str) -> PathBuf {
        self.sessions_root().join(session_id)
    }

    fn conversation_file(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("conversation.jsonl")
    }

    fn metadata_file(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("metadata.json")
    }

    fn project_mapping_file(&self) -> PathBuf {
        self.base_dir.join("project_mappings.json")
    }

    /// Read and parse the global session index. Returns an empty list when the
    /// index file is missing.
    async fn read_index(&self) -> Result<Vec<CodexIndexRow>> {
        let path = self.session_index_file();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let mut rows = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<CodexIndexRow>(line) {
                Ok(row) => rows.push(row),
                Err(e) => tracing::warn!("Failed to parse Codex index row: {}", e),
            }
        }
        Ok(rows)
    }

    /// Build a map of `session_id -> payload path` by scanning `sessions/`.
    async fn build_payload_index(&self) -> HashMap<String, PathBuf> {
        let mut map = HashMap::new();
        collect_payload_files(&self.sessions_root(), &mut map).await;
        map
    }

    /// Locate the rollout payload for a single session id (targeted walk).
    async fn find_payload(&self, session_id: &str) -> Option<PathBuf> {
        find_payload_file(&self.sessions_root(), session_id).await
    }

    /// Parse a rollout payload file into renderable messages.
    async fn parse_payload_messages(&self, path: &Path, session_id: &str) -> Result<Vec<Message>> {
        let content = tokio::fs::read_to_string(path).await?;
        Ok(parse_rollout(&content, session_id))
    }

    /// Parse a legacy `conversation.jsonl` (top-level `role`/`content` lines).
    async fn parse_legacy_messages(&self, path: &Path, session_id: &str) -> Result<Vec<Message>> {
        let content = tokio::fs::read_to_string(path).await?;
        let mut messages = Vec::new();
        let mut index = 0usize;

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };

            let role = value.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let role = match role {
                "assistant" => Role::Assistant,
                "system" => Role::System,
                _ => Role::User,
            };

            let text = match value.get("content") {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(serde_json::Value::Array(items)) => items
                    .iter()
                    .filter_map(|item| {
                        item.get("text")
                            .and_then(|t| t.as_str())
                            .or_else(|| item.as_str())
                    })
                    .map(String::from)
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            };

            let id = format!("{}-{}", session_id, index);
            index += 1;
            messages.push(Message {
                id,
                role,
                content: text.clone(),
                timestamp: Utc::now(),
                model: None,
                tool_uses: Vec::new(),
                content_blocks: vec![ContentBlock::Text { content: text }],
                tokens: None,
                is_streaming: false,
                metadata: None,
            });
        }

        Ok(messages)
    }

    /// Enumerate sessions using the legacy `project_mappings.json` layout.
    async fn legacy_sessions(&self, project_root: &Path) -> Result<Vec<Session>> {
        let session_ids = self.find_project_sessions(project_root).await?;
        let mut sessions = Vec::new();

        for session_id in session_ids {
            let session_dir = self.session_dir(&session_id);
            if !session_dir.exists() {
                continue;
            }

            let metadata_path = self.metadata_file(&session_id);
            let (name, created_at, updated_at) =
                match tokio::fs::read_to_string(&metadata_path).await {
                    Ok(content) => match serde_json::from_str::<CodexMetadata>(&content) {
                        Ok(metadata) => (
                            metadata.name.unwrap_or_else(|| session_id.clone()),
                            metadata.created_at,
                            metadata.updated_at,
                        ),
                        Err(_) => (session_id.clone(), Utc::now(), Utc::now()),
                    },
                    Err(_) => {
                        let (created, updated) = file_times(&session_dir).await;
                        (session_id.clone(), created, updated)
                    }
                };

            let conversation_path = self.conversation_file(&session_id);
            let message_count = if conversation_path.exists() {
                count_nonempty_lines(&conversation_path).await
            } else {
                0
            };

            let mut session = Session::new(&session_id, name, self.id());
            session.created_at = created_at;
            session.updated_at = updated_at;
            session.message_count = message_count;
            sessions.push(session);
        }

        sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
        Ok(sessions)
    }

    /// Find sessions associated with a project (legacy mapping file).
    async fn find_project_sessions(&self, project_root: &Path) -> Result<Vec<String>> {
        let mapping_file = self.project_mapping_file();
        if !mapping_file.exists() {
            return Ok(Vec::new());
        }

        let content = tokio::fs::read_to_string(&mapping_file).await?;
        let mappings: ProjectMappings = serde_json::from_str(&content)?;

        let canonical_path =
            std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
        let path_str = canonical_path.to_string_lossy().to_string();

        Ok(mappings
            .projects
            .get(&path_str)
            .cloned()
            .unwrap_or_default())
    }
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            base_dir: PathBuf::from(".codex"),
        })
    }
}

#[async_trait]
impl Adapter for CodexAdapter {
    fn id(&self) -> &str {
        "codex"
    }

    fn name(&self) -> &str {
        "Codex CLI"
    }

    fn icon(&self) -> char {
        '◆'
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::Codex
    }

    async fn detect(&self, project_root: &Path) -> Result<bool> {
        // Backward compat: honor project_mappings.json if present.
        if self.project_mapping_file().exists() {
            let session_ids = self.find_project_sessions(project_root).await?;
            for session_id in session_ids {
                if self.session_dir(&session_id).exists() {
                    return Ok(true);
                }
            }
            return Ok(false);
        }

        // Default: detected when the global session index has any rows.
        let rows = self.read_index().await?;
        Ok(!rows.is_empty())
    }

    async fn sessions(&self, project_root: &Path) -> Result<Vec<Session>> {
        // Backward compat: project_mappings.json layout.
        if self.project_mapping_file().exists() {
            return self.legacy_sessions(project_root).await;
        }

        let rows = self.read_index().await?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let payload_map = self.build_payload_index().await;
        let mut sessions = Vec::with_capacity(rows.len());

        for row in rows {
            let name = row.thread_name.unwrap_or_else(|| row.id.clone());
            let updated_at = row
                .updated_at
                .as_deref()
                .and_then(parse_iso)
                .unwrap_or_else(Utc::now);

            let (created_at, message_count) = match payload_map.get(&row.id) {
                Some(path) => {
                    let (created, _) = file_times(path).await;
                    (created, count_nonempty_lines(path).await)
                }
                None => (updated_at, 0),
            };

            let mut session = Session::new(&row.id, name, self.id());
            session.created_at = created_at;
            session.updated_at = updated_at;
            session.message_count = message_count;
            sessions.push(session);
        }

        sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
        Ok(sessions)
    }

    async fn messages(&self, session_id: &str) -> Result<Vec<Message>> {
        // Backward compat: legacy `sessions/<id>/conversation.jsonl`.
        let legacy = self.conversation_file(session_id);
        if legacy.exists() {
            return self.parse_legacy_messages(&legacy, session_id).await;
        }

        // Default: locate the rollout payload by id.
        if let Some(path) = self.find_payload(session_id).await {
            return self.parse_payload_messages(&path, session_id).await;
        }

        Ok(Vec::new())
    }

    async fn usage(&self, session_id: &str) -> Result<Option<TokenUsage>> {
        // Legacy metadata.json may carry aggregate usage.
        let metadata_path = self.metadata_file(session_id);
        if metadata_path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&metadata_path).await {
                if let Ok(metadata) = serde_json::from_str::<CodexMetadata>(&content) {
                    if let Some(usage) = metadata.total_usage {
                        return Ok(Some(TokenUsage::new(
                            usage.prompt_tokens,
                            usage.completion_tokens,
                        )));
                    }
                }
            }
        }

        // Fall back to summing per-message usage.
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

/// Project mappings structure (legacy)
#[derive(Debug, Deserialize)]
struct ProjectMappings {
    #[serde(default)]
    projects: HashMap<String, Vec<String>>,
}

/// Codex session metadata (legacy layout)
#[derive(Debug, Deserialize)]
struct CodexMetadata {
    name: Option<String>,
    #[serde(with = "chrono::serde::ts_seconds")]
    created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    updated_at: DateTime<Utc>,
    #[serde(default)]
    total_usage: Option<CodexTokenUsage>,
}

/// Codex token usage
#[derive(Debug, Deserialize)]
struct CodexTokenUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
}

/// One row of `~/.codex/session_index.jsonl`.
#[derive(Debug, Deserialize)]
struct CodexIndexRow {
    id: String,
    thread_name: Option<String>,
    updated_at: Option<String>,
}

/// A single JSONL line in a Codex rollout payload.
#[derive(Debug, Deserialize)]
struct CodexLine {
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    payload: Option<CodexPayload>,
}

/// Payload of a `response_item` line carrying a `message`.
#[derive(Debug, Deserialize)]
struct CodexPayload {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default, deserialize_with = "deserialize_text_array")]
    content: Vec<String>,
}

impl CodexPayload {
    fn into_message(self, timestamp: Option<&str>, id: String) -> Message {
        let role = match self.role.as_deref() {
            Some("assistant") => Role::Assistant,
            Some("system") | Some("developer") => Role::System,
            _ => Role::User,
        };

        let content = self.content.join("\n");
        let content_blocks = self
            .content
            .into_iter()
            .map(|text| ContentBlock::Text { content: text })
            .collect();

        Message {
            id,
            role,
            content,
            timestamp: timestamp.and_then(parse_iso).unwrap_or_else(Utc::now),
            model: None,
            tool_uses: Vec::new(),
            content_blocks,
            tokens: None,
            is_streaming: false,
            metadata: None,
        }
    }
}

/// Deserialize a message `content` array into the text it carries. Elements
/// without a `text` field are ignored; a bare string is accepted too.
fn deserialize_text_array<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value =
        Option::<serde_json::Value>::deserialize(deserializer)?.unwrap_or(serde_json::Value::Null);
    let mut out = Vec::new();
    match value {
        serde_json::Value::Null => {}
        serde_json::Value::String(s) => out.push(s),
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    out.push(text.to_string());
                }
            }
        }
        _ => {}
    }
    Ok(out)
}

/// Parse an ISO-8601 / RFC-3339 timestamp into UTC.
fn parse_iso(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Count non-empty lines in a file (best-effort: 0 on read failure).
async fn count_nonempty_lines(path: &Path) -> usize {
    match tokio::fs::read_to_string(path).await {
        Ok(content) => content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
        Err(_) => 0,
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

/// Extract the trailing session uuid from a rollout file stem.
///
/// Rollout files are named `rollout-<ts>-<uuid>.jsonl`, where `<uuid>` is the
/// trailing 36-character `8-4-4-4-12` identifier.
fn session_id_from_stem(stem: &str) -> Option<&str> {
    if stem.len() < 36 {
        return None;
    }
    let candidate = &stem[stem.len() - 36..];
    let bytes = candidate.as_bytes();
    let hyphens_at = [8usize, 13, 18, 23];
    let shaped = hyphens_at.iter().all(|&i| bytes.get(i) == Some(&b'-'));
    shaped.then_some(candidate)
}

/// Recursively collect rollout payload files into `map`, keyed by session id.
async fn collect_payload_files(dir: &Path, map: &mut HashMap<String, PathBuf>) {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return,
    };

    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(e)) => e,
            _ => return,
        };

        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        let path = entry.path();

        if file_type.is_dir() {
            Box::pin(collect_payload_files(&path, map)).await;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }

        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            if let Some(id) = session_id_from_stem(stem) {
                map.entry(id.to_string()).or_insert(path);
            }
        }
    }
}

/// Recursively locate the rollout payload for `session_id`, stopping at the
/// first match.
async fn find_payload_file(dir: &Path, session_id: &str) -> Option<PathBuf> {
    let mut entries = tokio::fs::read_dir(dir).await.ok()?;

    let suffix = format!("-{}.jsonl", session_id);
    while let Some(entry) = entries.next_entry().await.ok()? {
        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        let path = entry.path();

        if file_type.is_dir() {
            if let Some(found) = Box::pin(find_payload_file(&path, session_id)).await {
                return Some(found);
            }
            continue;
        }

        if file_type.is_file()
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| name.ends_with(&suffix))
        {
            return Some(path);
        }
    }

    None
}

/// Parse a rollout payload into messages, keeping only `response_item` lines
/// whose payload is a `message`.
fn parse_rollout(content: &str, session_id: &str) -> Vec<Message> {
    let mut messages = Vec::new();
    let mut index = 0usize;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let parsed: CodexLine = match serde_json::from_str(line) {
            Ok(parsed) => parsed,
            Err(e) => {
                tracing::warn!("Failed to parse Codex rollout line: {}", e);
                continue;
            }
        };

        if parsed.kind.as_deref() != Some("response_item") {
            continue;
        }
        let Some(payload) = parsed.payload else {
            continue;
        };
        if payload.kind.as_deref() != Some("message") {
            continue;
        }

        messages.push(payload.into_message(
            parsed.timestamp.as_deref(),
            format!("{}-{}", session_id, index),
        ));
        index += 1;
    }

    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const SESSION_ID: &str = "11111111-2222-3333-4444-555555555555";

    fn create_test_adapter() -> (CodexAdapter, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_base_dir(temp_dir.path().to_path_buf());
        (adapter, temp_dir)
    }

    fn sample_rollout() -> String {
        format!(
            r#"{{"timestamp":"2026-02-03T09:57:42.068Z","type":"session_meta","payload":{{"id":"{id}","cwd":"/proj"}}}}
{{"timestamp":"2026-02-03T09:57:43.000Z","type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"Build the feature"}}]}}}}
{{"timestamp":"2026-02-03T09:57:44.000Z","type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"On it."}}]}}}}
{{"timestamp":"2026-02-03T09:57:45.000Z","type":"event_msg","payload":{{}}}}"#,
            id = SESSION_ID
        )
    }

    /// Write a rollout payload at the real on-disk path shape.
    async fn write_rollout(adapter: &CodexAdapter, contents: &str) -> PathBuf {
        let path = adapter
            .sessions_root()
            .join("2026")
            .join("02")
            .join("03")
            .join(format!("rollout-2026-02-03T09-57-42-{}.jsonl", SESSION_ID));
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&path, contents).await.unwrap();
        path
    }

    #[tokio::test]
    async fn test_detect_no_data() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let detected = adapter.detect(project).await.unwrap();
        assert!(!detected);
    }

    #[tokio::test]
    async fn test_detect_with_index() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        tokio::fs::write(
            adapter.session_index_file(),
            format!(
                "{{\"id\":\"{id}\",\"thread_name\":\"T\",\"updated_at\":\"2026-02-03T09:57:42Z\"}}\n",
                id = SESSION_ID
            ),
        )
        .await
        .unwrap();

        let detected = adapter.detect(project).await.unwrap();
        assert!(detected);
    }

    #[tokio::test]
    async fn test_sessions_from_index() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        tokio::fs::write(
            adapter.session_index_file(),
            format!(
                "{{\"id\":\"{id}\",\"thread_name\":\"Build the feature\",\"updated_at\":\"2026-02-03T09:57:42Z\"}}\n",
                id = SESSION_ID
            ),
        )
        .await
        .unwrap();
        write_rollout(&adapter, &sample_rollout()).await;

        let sessions = adapter.sessions(project).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, SESSION_ID);
        assert_eq!(sessions[0].name, "Build the feature");
        // Four non-empty lines in the rollout payload.
        assert_eq!(sessions[0].message_count, 4);
    }

    #[tokio::test]
    async fn test_sessions_index_without_payload() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        // Index row present but payload missing -> still listed, count 0.
        tokio::fs::write(
            adapter.session_index_file(),
            format!(
                "{{\"id\":\"{id}\",\"thread_name\":\"Orphan\",\"updated_at\":\"2026-02-03T09:57:42Z\"}}\n",
                id = SESSION_ID
            ),
        )
        .await
        .unwrap();

        let sessions = adapter.sessions(project).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].message_count, 0);
    }

    #[tokio::test]
    async fn test_messages_from_payload() {
        let (adapter, _temp) = create_test_adapter();
        write_rollout(&adapter, &sample_rollout()).await;

        let messages = adapter.messages(SESSION_ID).await.unwrap();
        // Only the two `response_item`/`message` lines become messages.
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[0].content, "Build the feature");
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[1].content, "On it.");
    }

    #[tokio::test]
    async fn test_detect_legacy_project_mappings() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let mapping = serde_json::json!({
            "projects": {
                project.to_string_lossy().to_string(): ["legacy-session"]
            }
        });
        tokio::fs::write(adapter.project_mapping_file(), mapping.to_string())
            .await
            .unwrap();
        tokio::fs::create_dir_all(adapter.session_dir("legacy-session"))
            .await
            .unwrap();

        let detected = adapter.detect(project).await.unwrap();
        assert!(detected);
    }

    #[tokio::test]
    async fn test_messages_legacy_conversation() {
        let (adapter, _temp) = create_test_adapter();

        let session_dir = adapter.session_dir("legacy-session");
        tokio::fs::create_dir_all(&session_dir).await.unwrap();
        let conversation = r#"{"role":"user","content":"Hello","timestamp":1700000000}
{"role":"assistant","content":"Hi!","timestamp":1700000001}"#;
        tokio::fs::write(adapter.conversation_file("legacy-session"), conversation)
            .await
            .unwrap();

        let messages = adapter.messages("legacy-session").await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[1].role, Role::Assistant);
    }
}
