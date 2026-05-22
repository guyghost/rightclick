//! Claude Code adapter
//!
//! This adapter integrates with Anthropic's Claude Code CLI tool.
//! It reads conversation data from the local storage directory at
//! `~/.claude/projects/{encoded-path}/`.

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

    /// Get the projects directory path
    fn projects_dir(&self) -> PathBuf {
        self.base_dir.join("projects")
    }

    /// Get the project storage directory for a given project root
    fn project_dir(&self, project_root: &Path) -> PathBuf {
        let encoded = encode_path(project_root);
        self.projects_dir().join(encoded)
    }

    /// Get the sessions directory for a project
    fn sessions_dir(&self, project_root: &Path) -> PathBuf {
        self.project_dir(project_root).join("sessions")
    }

    /// Get the conversation file path for a session
    fn conversation_file(&self, project_root: &Path, session_id: &str) -> PathBuf {
        self.sessions_dir(project_root)
            .join(session_id)
            .join("conversation.jsonl")
    }

    /// Read and parse a JSONL file
    async fn read_jsonl<T: for<'de> Deserialize<'de>>(&self, path: &Path) -> Result<Vec<T>> {
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = tokio::fs::read_to_string(path).await?;
        let mut items = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<T>(line) {
                Ok(item) => items.push(item),
                Err(e) => {
                    tracing::warn!("Failed to parse JSONL line in {}: {}", path.display(), e);
                }
            }
        }

        Ok(items)
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
        let sessions_dir = self.sessions_dir(project_root);
        if !sessions_dir.exists() {
            return Ok(false);
        }

        // Check if there are any session directories
        let mut entries = tokio::fs::read_dir(&sessions_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn sessions(&self, project_root: &Path) -> Result<Vec<Session>> {
        let sessions_dir = self.sessions_dir(project_root);
        if !sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        let mut entries = tokio::fs::read_dir(&sessions_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let file_type = entry.file_type().await?;
            if !file_type.is_dir() {
                continue;
            }

            let session_id = entry.file_name().to_string_lossy().to_string();

            // Try to read session metadata
            let metadata_path = entry.path().join("metadata.json");
            let (name, created_at, updated_at) = if metadata_path.exists() {
                match tokio::fs::read_to_string(&metadata_path).await {
                    Ok(content) => match serde_json::from_str::<ClaudeSessionMetadata>(&content) {
                        Ok(metadata) => (
                            metadata.name.unwrap_or_else(|| session_id.clone()),
                            metadata.created_at,
                            metadata.updated_at,
                        ),
                        Err(_) => (session_id.clone(), Utc::now(), Utc::now()),
                    },
                    Err(_) => (session_id.clone(), Utc::now(), Utc::now()),
                }
            } else {
                (session_id.clone(), Utc::now(), Utc::now())
            };

            // Count messages by reading conversation file
            let conversation_path = self.conversation_file(project_root, &session_id);
            let message_count = if conversation_path.exists() {
                let raw_messages: Vec<ClaudeMessage> = self.read_jsonl(&conversation_path).await?;
                raw_messages.len()
            } else {
                0
            };

            let mut session = Session::new(&session_id, name, self.id());
            session.created_at = created_at;
            session.updated_at = updated_at;
            session.message_count = message_count;

            sessions.push(session);
        }

        // Sort by updated_at, newest first
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(sessions)
    }

    async fn messages(&self, session_id: &str) -> Result<Vec<Message>> {
        // We need to find the project for this session
        // This is a bit tricky since we store sessions per project
        // For now, we'll scan all project directories
        let projects_dir = self.projects_dir();
        if !projects_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = tokio::fs::read_dir(&projects_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let file_type = entry.file_type().await?;
            if !file_type.is_dir() {
                continue;
            }

            let conversation_path = entry
                .path()
                .join("sessions")
                .join(session_id)
                .join("conversation.jsonl");
            if conversation_path.exists() {
                let raw_messages: Vec<ClaudeMessage> = self.read_jsonl(&conversation_path).await?;

                return Ok(raw_messages
                    .into_iter()
                    .enumerate()
                    .map(|(idx, msg)| msg.into_message(format!("{}-{}", session_id, idx)))
                    .collect());
            }
        }

        Ok(Vec::new())
    }

    async fn usage(&self, session_id: &str) -> Result<Option<TokenUsage>> {
        // Get messages and calculate usage from them
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

/// Claude session metadata structure
#[derive(Debug, Deserialize)]
struct ClaudeSessionMetadata {
    name: Option<String>,
    #[serde(with = "chrono::serde::ts_seconds")]
    created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    updated_at: DateTime<Utc>,
}

/// Claude message structure (internal format)
#[derive(Debug, Deserialize)]
struct ClaudeMessage {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    msg_type: Option<String>,
    role: Option<String>,
    content: Option<String>,
    #[serde(default)]
    content_blocks: Vec<ClaudeContentBlock>,
    #[serde(with = "chrono::serde::ts_seconds_option", default)]
    timestamp: Option<DateTime<Utc>>,
    #[serde(default)]
    usage: Option<ClaudeTokenUsage>,
    model: Option<String>,
}

impl ClaudeMessage {
    fn into_message(self, id: String) -> Message {
        let role = match self.role.as_deref() {
            Some("user") => Role::User,
            Some("assistant") => Role::Assistant,
            Some("system") => Role::System,
            _ => Role::User,
        };

        let content = if !self.content_blocks.is_empty() {
            // Build content from content blocks
            self.content_blocks
                .iter()
                .filter_map(|block| block.as_text())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            self.content.unwrap_or_default()
        };

        let content_blocks = self
            .content_blocks
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
            timestamp: self.timestamp.unwrap_or_else(Utc::now),
            model: self.model,
            tool_uses: Vec::new(),
            content_blocks,
            tokens,
            is_streaming: false,
            metadata: None,
        }
    }
}

/// Claude content block
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_adapter() -> (ClaudeCodeAdapter, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let adapter = ClaudeCodeAdapter::with_base_dir(temp_dir.path().to_path_buf());
        (adapter, temp_dir)
    }

    #[tokio::test]
    async fn test_detect_no_data() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let detected = adapter.detect(project).await.unwrap();
        assert!(!detected);
    }

    #[tokio::test]
    async fn test_detect_with_session() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        // Create a session directory
        let sessions_dir = adapter.sessions_dir(project);
        tokio::fs::create_dir_all(sessions_dir.join("test-session"))
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
    async fn test_sessions_with_data() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        // Create a session with conversation
        let sessions_dir = adapter.sessions_dir(project);
        let session_dir = sessions_dir.join("test-session");
        tokio::fs::create_dir_all(&session_dir).await.unwrap();

        // Create metadata
        let metadata = serde_json::json!({
            "name": "Test Session",
            "created_at": Utc::now().timestamp(),
            "updated_at": Utc::now().timestamp()
        });
        tokio::fs::write(session_dir.join("metadata.json"), metadata.to_string())
            .await
            .unwrap();

        // Create conversation
        let conversation = r#"{"type": "message", "role": "user", "content": "Hello", "timestamp": 1700000000}
{"type": "message", "role": "assistant", "content": "Hi there!", "timestamp": 1700000001}"#;
        tokio::fs::write(session_dir.join("conversation.jsonl"), conversation)
            .await
            .unwrap();

        let sessions = adapter.sessions(project).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "Test Session");
        assert_eq!(sessions[0].message_count, 2);
    }
}
