//! Codex CLI adapter
//!
//! This adapter integrates with OpenAI's Codex CLI tool.
//! It reads conversation data from the local storage directory at
//! `~/.codex/sessions/`.

use crate::core::models::conversation::{ContentBlock, Message, Role, Session, TokenUsage};
use crate::adapters::types::{Adapter, AdapterError, AdapterType, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
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

    /// Get the sessions directory path
    fn sessions_dir(&self) -> PathBuf {
        self.base_dir.join("sessions")
    }

    /// Get the session directory for a specific session
    fn session_dir(&self, session_id: &str) -> PathBuf {
        self.sessions_dir().join(session_id)
    }

    /// Get the conversation file path for a session
    fn conversation_file(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("conversation.jsonl")
    }

    /// Get the metadata file path for a session
    fn metadata_file(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("metadata.json")
    }

    /// Get the project mapping file path
    fn project_mapping_file(&self) -> PathBuf {
        self.base_dir.join("project_mappings.json")
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

    /// Find sessions associated with a project
    async fn find_project_sessions(&self, project_root: &Path) -> Result<Vec<String>> {
        let mapping_file = self.project_mapping_file();
        if !mapping_file.exists() {
            return Ok(Vec::new());
        }

        let content = tokio::fs::read_to_string(&mapping_file).await?;
        let mappings: ProjectMappings = serde_json::from_str(&content)?;

        let canonical_path = std::fs::canonicalize(project_root)
            .unwrap_or_else(|_| project_root.to_path_buf());
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
        // Check if there's a project mapping for this project
        let sessions = self.find_project_sessions(project_root).await?;
        if sessions.is_empty() {
            return Ok(false);
        }

        // Verify at least one session directory exists
        for session_id in sessions {
            let session_dir = self.session_dir(&session_id);
            if session_dir.exists() {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn sessions(&self, project_root: &Path) -> Result<Vec<Session>> {
        let session_ids = self.find_project_sessions(project_root).await?;
        let mut sessions = Vec::new();

        for session_id in session_ids {
            let session_dir = self.session_dir(&session_id);
            if !session_dir.exists() {
                continue;
            }

            // Read metadata
            let metadata_path = self.metadata_file(&session_id);
            let (name, created_at, updated_at) = if metadata_path.exists() {
                match tokio::fs::read_to_string(&metadata_path).await {
                    Ok(content) => {
                        match serde_json::from_str::<CodexMetadata>(&content) {
                            Ok(metadata) => (
                                metadata.name.unwrap_or_else(|| session_id.clone()),
                                metadata.created_at,
                                metadata.updated_at,
                            ),
                            Err(_) => (session_id.clone(), Utc::now(), Utc::now()),
                        }
                    }
                    Err(_) => (session_id.clone(), Utc::now(), Utc::now()),
                }
            } else {
                // Try to get timestamps from directory
                let metadata = tokio::fs::metadata(&session_dir).await.ok();
                let created = metadata.as_ref().and_then(|m| m.created().ok());
                let modified = metadata.as_ref().and_then(|m| m.modified().ok());

                let created_at = created
                    .and_then(|t| DateTime::from_timestamp(
                        t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
                        0
                    ))
                    .unwrap_or_else(Utc::now);
                let updated_at = modified
                    .and_then(|t| DateTime::from_timestamp(
                        t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
                        0
                    ))
                    .unwrap_or_else(Utc::now);

                (session_id.clone(), created_at, updated_at)
            };

            // Count messages
            let conversation_path = self.conversation_file(&session_id);
            let message_count = if conversation_path.exists() {
                let messages: Vec<CodexMessage> = self.read_jsonl(&conversation_path).await?;
                messages.len()
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
        let conversation_path = self.conversation_file(session_id);
        if !conversation_path.exists() {
            return Ok(Vec::new());
        }

        let raw_messages: Vec<CodexMessage> = self.read_jsonl(&conversation_path).await?;

        Ok(raw_messages
            .into_iter()
            .enumerate()
            .map(|(idx, msg)| msg.to_message(format!("{}-{}", session_id, idx)))
            .collect())
    }

    async fn usage(&self, session_id: &str) -> Result<Option<TokenUsage>> {
        // Check metadata for total usage
        let metadata_path = self.metadata_file(session_id);
        if metadata_path.exists() {
            let content = tokio::fs::read_to_string(&metadata_path).await?;
            if let Ok(metadata) = serde_json::from_str::<CodexMetadata>(&content) {
                if let Some(usage) = metadata.total_usage {
                    return Ok(Some(TokenUsage::new(usage.prompt_tokens, usage.completion_tokens)));
                }
            }
        }

        // Fall back to calculating from messages
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

/// Project mappings structure
#[derive(Debug, Deserialize)]
struct ProjectMappings {
    #[serde(default)]
    projects: std::collections::HashMap<String, Vec<String>>,
}

/// Codex session metadata
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

/// Codex message structure (internal format)
#[derive(Debug, Deserialize)]
struct CodexMessage {
    role: String,
    content: Option<String>,
    #[serde(default)]
    content_blocks: Vec<CodexContentBlock>,
    #[serde(with = "chrono::serde::ts_seconds_option", default)]
    timestamp: Option<DateTime<Utc>>,
    #[serde(default)]
    usage: Option<CodexTokenUsage>,
    model: Option<String>,
}

impl CodexMessage {
    fn to_message(self, id: String) -> Message {
        let role = match self.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "system" => Role::System,
            _ => Role::User,
        };

        let content = if !self.content_blocks.is_empty() {
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
            .filter_map(|block| block.to_content_block())
            .collect();

        let tokens = self.usage.map(|u| TokenUsage::new(u.prompt_tokens, u.completion_tokens));

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

/// Codex content block
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum CodexContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "reasoning")]
    Reasoning { content: String },
    #[serde(rename = "tool_call")]
    ToolCall { id: String, function: CodexFunction },
    #[serde(rename = "tool_result")]
    ToolResult { tool_call_id: String, output: String },
    #[serde(rename = "code")]
    Code { language: Option<String>, code: String },
}

impl CodexContentBlock {
    fn as_text(&self) -> Option<&str> {
        match self {
            CodexContentBlock::Text { text } => Some(text),
            CodexContentBlock::Reasoning { content } => Some(content),
            _ => None,
        }
    }

    fn to_content_block(self) -> Option<ContentBlock> {
        match self {
            CodexContentBlock::Text { text } => Some(ContentBlock::Text { content: text }),
            CodexContentBlock::Reasoning { content } => {
                Some(ContentBlock::Markdown { content: format!("*Reasoning:* {}", content) })
            }
            CodexContentBlock::ToolCall { id, function } => Some(ContentBlock::ToolUse {
                id,
                name: function.name,
                input: function.arguments,
            }),
            CodexContentBlock::ToolResult { tool_call_id, output } => Some(ContentBlock::ToolResult {
                tool_use_id: tool_call_id,
                content: output,
                is_error: false,
            }),
            CodexContentBlock::Code { language, code } => Some(ContentBlock::Code {
                language,
                code,
                file_path: None,
            }),
        }
    }
}

/// Codex function call
#[derive(Debug, Deserialize)]
struct CodexFunction {
    name: String,
    arguments: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_adapter() -> (CodexAdapter, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_base_dir(temp_dir.path().to_path_buf());
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
    async fn test_detect_with_mapping() {
        let (adapter, temp) = create_test_adapter();
        let project = Path::new("/test/project");

        // Create project mapping
        let mapping = serde_json::json!({
            "projects": {
                project.to_string_lossy().to_string(): ["test-session"]
            }
        });
        tokio::fs::write(adapter.project_mapping_file(), mapping.to_string())
            .await
            .unwrap();

        // Create session directory
        let session_dir = adapter.session_dir("test-session");
        tokio::fs::create_dir_all(&session_dir).await.unwrap();

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
        let (adapter, temp) = create_test_adapter();
        let project = Path::new("/test/project");

        // Create project mapping
        let mapping = serde_json::json!({
            "projects": {
                project.to_string_lossy().to_string(): ["test-session"]
            }
        });
        tokio::fs::write(adapter.project_mapping_file(), mapping.to_string())
            .await
            .unwrap();

        // Create session with data
        let session_dir = adapter.session_dir("test-session");
        tokio::fs::create_dir_all(&session_dir).await.unwrap();

        // Create metadata
        let metadata = serde_json::json!({
            "name": "Test Session",
            "created_at": Utc::now().timestamp(),
            "updated_at": Utc::now().timestamp(),
            "total_usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50
            }
        });
        tokio::fs::write(
            adapter.metadata_file("test-session"),
            metadata.to_string(),
        )
        .await
        .unwrap();

        // Create conversation
        let conversation = r#"{"role": "user", "content": "Hello", "timestamp": 1700000000}
{"role": "assistant", "content": "Hi!", "timestamp": 1700000001}"#;
        tokio::fs::write(
            adapter.conversation_file("test-session"),
            conversation,
        )
        .await
        .unwrap();

        let sessions = adapter.sessions(project).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "Test Session");
        assert_eq!(sessions[0].message_count, 2);
    }

    #[tokio::test]
    async fn test_messages() {
        let (adapter, temp) = create_test_adapter();

        // Create session with data
        let session_dir = adapter.session_dir("test-session");
        tokio::fs::create_dir_all(&session_dir).await.unwrap();

        // Create conversation with content blocks
        let conversation = r#"{"role": "user", "content": "Hello", "timestamp": 1700000000}
{"role": "assistant", "content": "Hi there!", "timestamp": 1700000001, "model": "gpt-4", "usage": {"prompt_tokens": 10, "completion_tokens": 5}}"#;
        tokio::fs::write(
            adapter.conversation_file("test-session"),
            conversation,
        )
        .await
        .unwrap();

        let messages = adapter.messages("test-session").await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[1].model, Some("gpt-4".to_string()));
        assert!(messages[1].tokens.is_some());
        assert_eq!(messages[1].tokens.unwrap().total_tokens, 15);
    }

    #[tokio::test]
    async fn test_usage_from_metadata() {
        let (adapter, temp) = create_test_adapter();

        // Create session directory
        let session_dir = adapter.session_dir("test-session");
        tokio::fs::create_dir_all(&session_dir).await.unwrap();

        // Create metadata with total usage
        let metadata = serde_json::json!({
            "name": "Test",
            "created_at": Utc::now().timestamp(),
            "updated_at": Utc::now().timestamp(),
            "total_usage": {
                "prompt_tokens": 500,
                "completion_tokens": 200
            }
        });
        tokio::fs::write(
            adapter.metadata_file("test-session"),
            metadata.to_string(),
        )
        .await
        .unwrap();

        let usage = adapter.usage("test-session").await.unwrap();
        assert!(usage.is_some());
        assert_eq!(usage.unwrap().total_tokens, 700);
    }
}
