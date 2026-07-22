//! Cursor adapter
//!
//! This adapter integrates with the Cursor IDE's AI chat feature.
//! It reads conversation data from the SQLite database at
//! `~/.cursor/chats/{md5-hash}/store.db`.

use crate::adapters::types::{Adapter, AdapterError, AdapterType, Result};
use crate::core::models::conversation::{ContentBlock, Message, Role, Session, TokenUsage};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

/// Cursor adapter implementation
#[derive(Debug)]
pub struct CursorAdapter {
    /// Base directory for Cursor storage
    base_dir: PathBuf,
}

impl CursorAdapter {
    /// Create a new Cursor adapter with default paths
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| AdapterError::Other("Could not find home directory".to_string()))?;

        Ok(Self {
            base_dir: home_dir.join(".cursor"),
        })
    }

    /// Create a new Cursor adapter with a custom base directory
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Get the chats directory path
    fn chats_dir(&self) -> PathBuf {
        self.base_dir.join("chats")
    }

    /// Get the chat directory for a project (using MD5 hash)
    fn project_chat_dir(&self, project_root: &Path) -> PathBuf {
        let hash = md5_hash(&project_root.to_string_lossy());
        self.chats_dir().join(hash)
    }

    /// Get the database path for a project
    fn db_path(&self, project_root: &Path) -> PathBuf {
        self.project_chat_dir(project_root).join("store.db")
    }

    #[allow(dead_code)]
    /// Compute MD5 hash of a string
    fn compute_hash(&self, input: &str) -> String {
        md5_hash(input)
    }

    /// Open a database connection
    fn open_db(&self, path: &Path) -> Result<Connection> {
        Connection::open(path).map_err(AdapterError::from)
    }

    /// Find the database for a session ID by scanning all chat directories
    fn find_db_for_session(&self, session_id: &str) -> Result<Option<PathBuf>> {
        let chats_dir = self.chats_dir();
        if !chats_dir.exists() {
            return Ok(None);
        }

        for entry in std::fs::read_dir(&chats_dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_dir() {
                continue;
            }

            let db_path = entry.path().join("store.db");
            if !db_path.exists() {
                continue;
            }

            // Check if this database contains the session
            let conn = self.open_db(&db_path)?;
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM conversations WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )?;

            if count > 0 {
                return Ok(Some(db_path));
            }
        }

        Ok(None)
    }
}

impl Default for CursorAdapter {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            base_dir: PathBuf::from(".cursor"),
        })
    }
}

#[async_trait]
impl Adapter for CursorAdapter {
    fn id(&self) -> &str {
        "cursor"
    }

    fn name(&self) -> &str {
        "Cursor"
    }

    fn icon(&self) -> char {
        '◉'
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::Cursor
    }

    async fn detect(&self, project_root: &Path) -> Result<bool> {
        let db_path = self.db_path(project_root);
        if !db_path.exists() {
            return Ok(false);
        }

        // Check if the database has any conversations
        let conn = self.open_db(&db_path)?;
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))?;

        Ok(count > 0)
    }

    async fn sessions(&self, project_root: &Path) -> Result<Vec<Session>> {
        let db_path = self.db_path(project_root);
        if !db_path.exists() {
            return Ok(Vec::new());
        }

        let conn = self.open_db(&db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at, 
                    (SELECT COUNT(*) FROM messages WHERE conversation_id = conversations.id) as message_count
             FROM conversations
             ORDER BY updated_at DESC"
        )?;

        let sessions = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let title: Option<String> = row.get(1)?;
            let created_at: i64 = row.get(2)?;
            let updated_at: i64 = row.get(3)?;
            let message_count: i64 = row.get(4)?;

            let created_at = DateTime::from_timestamp(created_at, 0).unwrap_or_else(Utc::now);
            let updated_at = DateTime::from_timestamp(updated_at, 0).unwrap_or_else(Utc::now);

            let mut session = Session::new(
                &id,
                title.unwrap_or_else(|| "Untitled".to_string()),
                "cursor",
            );
            session.created_at = created_at;
            session.updated_at = updated_at;
            session.message_count = message_count as usize;

            Ok(session)
        })?;

        sessions
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(AdapterError::from)
    }

    async fn messages(&self, session_id: &str) -> Result<Vec<Message>> {
        let db_path = self.find_db_for_session(session_id)?;
        let db_path = match db_path {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        let conn = self.open_db(&db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, role, content, created_at, model, metadata
             FROM messages
             WHERE conversation_id = ?1
             ORDER BY created_at ASC",
        )?;

        let messages = stmt.query_map(params![session_id], |row| {
            let id: String = row.get(0)?;
            let role: String = row.get(1)?;
            let content: String = row.get(2)?;
            let created_at: i64 = row.get(3)?;
            let model: Option<String> = row.get(4)?;
            let metadata: Option<String> = row.get(5)?;

            let role = match role.as_str() {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "system" => Role::System,
                _ => Role::User,
            };

            let timestamp = DateTime::from_timestamp(created_at, 0).unwrap_or_else(Utc::now);

            // Parse metadata for token usage and content blocks
            let (tokens, content_blocks) = if let Some(ref meta_str) = metadata {
                parse_cursor_metadata(meta_str)
            } else {
                (None, Vec::new())
            };

            Ok(Message {
                id,
                role,
                content,
                timestamp,
                model,
                tool_uses: Vec::new(),
                content_blocks,
                tokens,
                is_streaming: false,
                metadata: metadata.and_then(|m| serde_json::from_str(&m).ok()),
            })
        })?;

        messages
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(AdapterError::from)
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

/// Parse Cursor message metadata for token usage and content blocks
fn parse_cursor_metadata(metadata: &str) -> (Option<TokenUsage>, Vec<ContentBlock>) {
    #[derive(serde::Deserialize, Default)]
    struct CursorMetadata {
        #[serde(default)]
        usage: Option<CursorUsage>,
        #[serde(default)]
        content_blocks: Vec<serde_json::Value>,
    }

    #[derive(serde::Deserialize)]
    struct CursorUsage {
        prompt_tokens: usize,
        completion_tokens: usize,
    }

    let meta: CursorMetadata = serde_json::from_str(metadata).unwrap_or_default();

    let tokens = meta
        .usage
        .map(|u| TokenUsage::new(u.prompt_tokens, u.completion_tokens));

    let content_blocks = meta
        .content_blocks
        .into_iter()
        .filter_map(parse_content_block)
        .collect();

    (tokens, content_blocks)
}

/// Parse a JSON value into a ContentBlock
fn parse_content_block(value: serde_json::Value) -> Option<ContentBlock> {
    let block_type = value.get("type")?.as_str()?;

    match block_type {
        "text" => {
            let text = value.get("text")?.as_str()?;
            Some(ContentBlock::Text {
                content: text.to_string(),
            })
        }
        "code" => {
            let code = value.get("code")?.as_str()?.to_string();
            let language = value
                .get("language")
                .and_then(|l| l.as_str())
                .map(String::from);
            Some(ContentBlock::Code {
                language,
                code,
                file_path: None,
            })
        }
        "markdown" => {
            let content = value.get("content")?.as_str()?;
            Some(ContentBlock::Markdown {
                content: content.to_string(),
            })
        }
        "image" => {
            let source = value.get("source")?.as_str()?.to_string();
            let alt = value.get("alt").and_then(|a| a.as_str()).map(String::from);
            Some(ContentBlock::Image { source, alt })
        }
        "file" => {
            let path = value.get("path")?.as_str()?.to_string();
            let content = value
                .get("content")
                .and_then(|c| c.as_str())
                .map(String::from);
            let line_count = value
                .get("line_count")
                .and_then(|l| l.as_u64())
                .map(|n| n as usize);
            Some(ContentBlock::File {
                path,
                content,
                line_count,
            })
        }
        _ => None,
    }
}

/// Compute MD5 hash of a string
fn md5_hash(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;

    let mut hasher = DefaultHasher::new();
    hasher.write(input.as_bytes());
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_adapter() -> (CursorAdapter, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let adapter = CursorAdapter::with_base_dir(temp_dir.path().to_path_buf());
        (adapter, temp_dir)
    }

    fn create_test_db(path: &Path) -> Connection {
        let conn = Connection::open(path).unwrap();
        conn.execute(
            "CREATE TABLE conversations (
                id TEXT PRIMARY KEY,
                title TEXT,
                created_at INTEGER,
                updated_at INTEGER
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT,
                role TEXT,
                content TEXT,
                created_at INTEGER,
                model TEXT,
                metadata TEXT
            )",
            [],
        )
        .unwrap();
        conn
    }

    #[tokio::test]
    async fn test_detect_no_data() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let detected = adapter.detect(project).await.unwrap();
        assert!(!detected);
    }

    #[tokio::test]
    async fn test_detect_with_db() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        // Create database
        let db_path = adapter.db_path(project);
        tokio::fs::create_dir_all(db_path.parent().unwrap())
            .await
            .unwrap();

        // Create the database with schema
        let conn = create_test_db(&db_path);
        drop(conn); // Close connection

        let detected = adapter.detect(project).await.unwrap();
        // Should be false because no conversations exist
        assert!(!detected);
    }

    #[tokio::test]
    async fn test_sessions_with_data() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        // Create database with data
        let db_path = adapter.db_path(project);
        tokio::fs::create_dir_all(db_path.parent().unwrap())
            .await
            .unwrap();

        {
            let conn = create_test_db(&db_path);
            conn.execute(
                "INSERT INTO conversations (id, title, created_at, updated_at) 
                 VALUES ('test-conv', 'Test Conversation', 1700000000, 1700000100)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, created_at, model, metadata)
                 VALUES ('msg1', 'test-conv', 'user', 'Hello', 1700000000, NULL, NULL)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, created_at, model, metadata)
                 VALUES ('msg2', 'test-conv', 'assistant', 'Hi!', 1700000100, 'gpt-4', NULL)",
                [],
            ).unwrap();
        }

        let sessions = adapter.sessions(project).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "Test Conversation");
        assert_eq!(sessions[0].message_count, 2);
    }

    #[tokio::test]
    async fn test_messages() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        // Create database with data
        let db_path = adapter.db_path(project);
        tokio::fs::create_dir_all(db_path.parent().unwrap())
            .await
            .unwrap();

        {
            let conn = create_test_db(&db_path);
            conn.execute(
                "INSERT INTO conversations (id, title, created_at, updated_at) 
                 VALUES ('test-conv', 'Test', 1700000000, 1700000100)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, created_at, model, metadata)
                 VALUES ('msg1', 'test-conv', 'user', 'Hello', 1700000000, NULL, NULL)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, created_at, model, metadata)
                 VALUES ('msg2', 'test-conv', 'assistant', 'Hi there!', 1700000100, 'gpt-4', NULL)",
                [],
            ).unwrap();
        }

        let messages = adapter.messages("test-conv").await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[1].content, "Hi there!");
        assert_eq!(messages[1].model, Some("gpt-4".to_string()));
    }

    #[test]
    fn test_parse_cursor_metadata() {
        let metadata = r#"{
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50
            }
        }"#;

        let (tokens, blocks) = parse_cursor_metadata(metadata);
        assert!(tokens.is_some());
        assert_eq!(tokens.unwrap().total_tokens, 150);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_parse_content_blocks() {
        let metadata = r#"{
            "content_blocks": [
                {"type": "text", "text": "Hello"},
                {"type": "code", "code": "fn main() {}", "language": "rust"}
            ]
        }"#;

        let (tokens, blocks) = parse_cursor_metadata(metadata);
        assert!(tokens.is_none());
        assert_eq!(blocks.len(), 2);
        assert!(matches!(blocks[0], ContentBlock::Text { .. }));
        assert!(matches!(blocks[1], ContentBlock::Code { .. }));
    }

    #[test]
    fn test_parse_cursor_metadata_combined_usage_and_blocks() {
        let metadata = r##"{
            "usage": {"prompt_tokens": 50, "completion_tokens": 25},
            "content_blocks": [
                {"type": "markdown", "content": "# Heading"},
                {"type": "image", "source": "/img.png", "alt": "diagram"}
            ]
        }"##;

        let (tokens, blocks) = parse_cursor_metadata(metadata);
        assert_eq!(tokens.unwrap().total_tokens, 75);
        assert_eq!(blocks.len(), 2);
        assert!(matches!(blocks[0], ContentBlock::Markdown { .. }));
        assert!(matches!(blocks[1], ContentBlock::Image { .. }));
    }

    #[test]
    fn test_parse_cursor_metadata_invalid_json_returns_defaults() {
        let (tokens, blocks) = parse_cursor_metadata("not json at all");
        assert!(tokens.is_none());
        assert!(blocks.is_empty());
    }

    #[tokio::test]
    async fn test_messages_with_metadata_and_usage() {
        let (adapter, _temp) = create_test_adapter();
        let project = Path::new("/test/project");

        let db_path = adapter.db_path(project);
        tokio::fs::create_dir_all(db_path.parent().unwrap())
            .await
            .unwrap();

        {
            let conn = create_test_db(&db_path);
            conn.execute(
                "INSERT INTO conversations (id, title, created_at, updated_at) VALUES ('c1','T',1700000000,1700000100)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, created_at, model, metadata) VALUES ('m1','c1','assistant','reply',1700000050,'gpt-4','{\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":20}}')",
                [],
            )
            .unwrap();
        }

        let messages = adapter.messages("c1").await.unwrap();
        assert_eq!(messages.len(), 1);
        let usage = messages[0].tokens.as_ref().expect("should have usage");
        assert_eq!(usage.total_tokens, 30);
    }
}
