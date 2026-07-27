//! OpenCode adapter
//!
//! This adapter integrates with the OpenCode AI coding tool.
//! It reads conversation data from the local storage directory at
//! `~/.local/share/opencode/storage/`.

use crate::adapters::types::{Adapter, AdapterError, AdapterType, Result};
use crate::core::models::conversation::{Message, Role, Session, TokenUsage};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// OpenCode message metadata JSON structure
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenCodeMessageMetadata {
    id: String,
    #[serde(rename = "sessionID")]
    session_id: String,
    role: String,
    time: OpenCodeTime,
    #[serde(rename = "modelID")]
    model_id: Option<String>,
    tokens: Option<OpenCodeTokens>,
}

/// Timestamps in OpenCode format
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenCodeTime {
    created: i64,
    completed: Option<i64>,
}

/// Token information in OpenCode format
#[derive(Debug, Deserialize)]
struct OpenCodeTokens {
    input: usize,
    output: usize,
}

/// OpenCode part content JSON structure
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenCodePart {
    id: String,
    #[serde(rename = "type")]
    part_type: String,
    text: Option<String>,
    #[serde(rename = "messageID")]
    message_id: String,
    #[serde(rename = "sessionID")]
    session_id: String,
}

/// OpenCode adapter implementation
#[derive(Debug)]
pub struct OpenCodeAdapter {
    /// Base directory for OpenCode storage
    data_dir: PathBuf,
}

impl OpenCodeAdapter {
    /// Create a new OpenCode adapter with default paths
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| AdapterError::Other("Could not find home directory".to_string()))?;

        Ok(Self {
            data_dir: home_dir.join(".local/share/opencode/storage"),
        })
    }

    /// Create a new OpenCode adapter with a custom data directory
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }
}

impl Default for OpenCodeAdapter {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            data_dir: PathBuf::from(".local/share/opencode/storage"),
        })
    }
}

impl OpenCodeAdapter {
    /// Parse role string to Role enum
    fn parse_role(role: &str) -> Role {
        match role.to_lowercase().as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "system" => Role::System,
            "tool" => Role::Tool,
            _ => Role::User,
        }
    }

    /// Load message metadata files for a session
    async fn load_message_metadata(
        &self,
        session_id: &str,
    ) -> Result<Vec<OpenCodeMessageMetadata>> {
        let message_dir = self.data_dir.join("message").join(session_id);

        if !message_dir.exists() {
            return Ok(Vec::new());
        }

        let mut messages = Vec::new();
        let mut entries = tokio::fs::read_dir(&message_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Only process JSON files
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            // Skip part files (we'll handle those separately)
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if filename.starts_with("prt_") {
                continue;
            }

            // Read and parse the message metadata
            let content = tokio::fs::read_to_string(&path).await?;
            let metadata: OpenCodeMessageMetadata =
                serde_json::from_str(&content).map_err(|e| {
                    AdapterError::Other(format!("Failed to parse message metadata: {}", e))
                })?;

            messages.push(metadata);
        }

        // Sort by created timestamp with a stable tie-breaker for filesystems that
        // return equal-timestamp messages in arbitrary directory order.
        messages.sort_by(|a, b| {
            a.time
                .created
                .cmp(&b.time.created)
                .then_with(|| a.id.cmp(&b.id))
        });

        Ok(messages)
    }

    /// Load part files for a session by scanning the part/<message_id>/ directories
    /// Parts are stored at: part/<message_id>/prt_<id>.json
    async fn load_parts(&self, session_id: &str) -> Result<HashMap<String, Vec<OpenCodePart>>> {
        let part_base_dir = self.data_dir.join("part");

        if !part_base_dir.exists() {
            return Ok(HashMap::new());
        }

        let mut parts_by_message: HashMap<String, Vec<OpenCodePart>> = HashMap::new();
        let mut entries = tokio::fs::read_dir(&part_base_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Only process directories (each message has its own part directory)
            if !entry.file_type().await?.is_dir() {
                continue;
            }

            // Read part files in this message's part directory
            let mut part_entries = tokio::fs::read_dir(&path).await?;

            while let Some(part_entry) = part_entries.next_entry().await? {
                let part_path = part_entry.path();

                // Only process JSON files
                if part_path.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }

                // Only process part files (prt_*.json)
                let filename = part_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !filename.starts_with("prt_") {
                    continue;
                }

                // Read and parse the part, skip on error
                if let Ok(content) = tokio::fs::read_to_string(&part_path).await {
                    if let Ok(part) = serde_json::from_str::<OpenCodePart>(&content) {
                        // Only include parts for this session
                        if part.session_id == session_id {
                            parts_by_message
                                .entry(part.message_id.clone())
                                .or_default()
                                .push(part);
                        }
                    }
                }
            }
        }

        Ok(parts_by_message)
    }

    /// Count messages for a session
    async fn count_messages(&self, session_id: &str) -> Result<usize> {
        let message_dir = self.data_dir.join("message").join(session_id);

        if !message_dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        let mut entries = tokio::fs::read_dir(&message_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Only process JSON files
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            // Skip part files
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if filename.starts_with("prt_") {
                continue;
            }

            count += 1;
        }

        Ok(count)
    }
}

#[async_trait]
impl Adapter for OpenCodeAdapter {
    fn id(&self) -> &str {
        "opencode"
    }

    fn name(&self) -> &str {
        "OpenCode"
    }

    fn icon(&self) -> char {
        '◑'
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::OpenCode
    }

    async fn detect(&self, _project_root: &Path) -> Result<bool> {
        Ok(self.data_dir.exists())
    }

    async fn sessions(&self, _project_root: &Path) -> Result<Vec<Session>> {
        let message_dir = self.data_dir.join("message");

        if !message_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        let mut entries = tokio::fs::read_dir(&message_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let file_type = entry.file_type().await?;
            if !file_type.is_dir() {
                continue;
            }

            let session_id = entry.file_name().to_string_lossy().to_string();

            // Get timestamps from directory metadata
            let metadata = tokio::fs::metadata(entry.path()).await.ok();
            let created = metadata.as_ref().and_then(|m| m.created().ok());
            let modified = metadata.as_ref().and_then(|m| m.modified().ok());

            let created_at = created
                .and_then(|t| {
                    DateTime::from_timestamp(
                        t.duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64,
                        0,
                    )
                })
                .unwrap_or_else(Utc::now);
            let updated_at = modified
                .and_then(|t| {
                    DateTime::from_timestamp(
                        t.duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64,
                        0,
                    )
                })
                .unwrap_or_else(Utc::now);

            // Count messages in the session
            let message_count = self.count_messages(&session_id).await.unwrap_or(0);

            let mut session = Session::new(&session_id, session_id.clone(), self.id(), created_at);
            session.created_at = created_at;
            session.updated_at = updated_at;
            session.message_count = message_count;

            sessions.push(session);
        }

        // Sort by updated_at, newest first
        sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_at));

        Ok(sessions)
    }

    async fn messages(&self, session_id: &str) -> Result<Vec<Message>> {
        let metadata_list = self.load_message_metadata(session_id).await?;
        let parts_by_message = self.load_parts(session_id).await?;

        let mut messages = Vec::new();

        for metadata in metadata_list {
            // Build content from parts
            let content = if let Some(parts) = parts_by_message.get(&metadata.id) {
                // Concatenate text from all parts
                parts
                    .iter()
                    .filter_map(|p| p.text.as_ref())
                    .map(|s| s.to_owned())
                    .collect::<Vec<String>>()
                    .join("\n")
            } else {
                // No parts found - empty content
                String::new()
            };

            // Parse timestamp
            let timestamp =
                DateTime::from_timestamp(metadata.time.created / 1000, 0).unwrap_or_else(Utc::now);

            // Parse token usage if available
            let tokens = metadata.tokens.as_ref().map(|t| TokenUsage {
                prompt_tokens: t.input,
                completion_tokens: t.output,
                total_tokens: t.input + t.output,
            });

            // Create message
            let role = Self::parse_role(&metadata.role);
            let message = Message {
                id: metadata.id.clone(),
                role,
                content,
                timestamp,
                model: metadata.model_id.clone(),
                tool_uses: Vec::new(),
                content_blocks: Vec::new(),
                tokens,
                is_streaming: false,
                metadata: None,
            };

            messages.push(message);
        }

        Ok(messages)
    }

    async fn usage(&self, session_id: &str) -> Result<Option<TokenUsage>> {
        let messages = self.messages(session_id).await?;

        let mut total_input = 0;
        let mut total_output = 0;

        for message in &messages {
            if let Some(tokens) = &message.tokens {
                total_input += tokens.prompt_tokens;
                total_output += tokens.completion_tokens;
            }
        }

        if total_input == 0 && total_output == 0 {
            return Ok(None);
        }

        Ok(Some(TokenUsage {
            prompt_tokens: total_input,
            completion_tokens: total_output,
            total_tokens: total_input + total_output,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_adapter() -> (OpenCodeAdapter, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let adapter = OpenCodeAdapter::with_data_dir(temp_dir.path().to_path_buf());
        (adapter, temp_dir)
    }

    #[test]
    fn test_new_succeeds() {
        // new() should succeed even if the directory doesn't exist
        let result = OpenCodeAdapter::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_adapter_properties() {
        let (adapter, _temp) = create_test_adapter();
        assert_eq!(adapter.id(), "opencode");
        assert_eq!(adapter.name(), "OpenCode");
        assert_eq!(adapter.icon(), '◑');
        assert_eq!(adapter.adapter_type(), AdapterType::OpenCode);
    }

    #[tokio::test]
    async fn test_detect_no_data() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = OpenCodeAdapter::with_data_dir(temp_dir.path().join("nonexistent"));

        let detected = adapter.detect(Path::new("/test/project")).await.unwrap();
        assert!(!detected);
    }

    #[tokio::test]
    async fn test_detect_with_dir() {
        let (adapter, _temp) = create_test_adapter();

        let detected = adapter.detect(Path::new("/test/project")).await.unwrap();
        assert!(detected);
    }

    #[tokio::test]
    async fn test_sessions_empty() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = OpenCodeAdapter::with_data_dir(temp_dir.path().join("nonexistent"));

        let sessions = adapter.sessions(Path::new("/test/project")).await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_sessions_with_dirs() {
        let (adapter, _temp) = create_test_adapter();

        // Create some session directories in the message/ subdirectory
        tokio::fs::create_dir_all(adapter.data_dir.join("message/session-1"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(adapter.data_dir.join("message/session-2"))
            .await
            .unwrap();

        let sessions = adapter.sessions(Path::new("/test/project")).await.unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[tokio::test]
    async fn test_messages_empty_session() {
        let (adapter, _temp) = create_test_adapter();

        let messages = adapter.messages("nonexistent-session").await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn test_sessions_with_message_count() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = OpenCodeAdapter::with_data_dir(temp_dir.path().to_path_buf());

        // Create message directory with 3 message files for session-1
        let msg_dir = adapter.data_dir.join("message/session-1");
        tokio::fs::create_dir_all(&msg_dir).await.unwrap();

        // Create message metadata files
        for i in 1..=3 {
            let msg_json = serde_json::json!({
                "id": format!("msg_{}", i),
                "sessionID": "session-1",
                "role": "user",
                "time": {
                    "created": 1700000000000i64 + (i * 1000),
                    "completed": null
                }
            });
            tokio::fs::write(
                msg_dir.join(format!("msg_{}.json", i)),
                msg_json.to_string(),
            )
            .await
            .unwrap();
        }

        // Create session with no messages (empty directory)
        tokio::fs::create_dir_all(adapter.data_dir.join("message/session-2"))
            .await
            .unwrap();

        let sessions = adapter.sessions(Path::new("/test/project")).await.unwrap();
        assert_eq!(sessions.len(), 2);

        // Find session-1 and verify count
        let session1 = sessions.iter().find(|s| s.id == "session-1").unwrap();
        assert_eq!(session1.message_count, 3);

        // Find session-2 and verify count
        let session2 = sessions.iter().find(|s| s.id == "session-2").unwrap();
        assert_eq!(session2.message_count, 0);
    }

    #[tokio::test]
    async fn test_messages_loading() {
        let (adapter, _temp) = create_test_adapter();

        // Create message directory
        let msg_dir = adapter.data_dir.join("message/session-123");
        tokio::fs::create_dir_all(&msg_dir).await.unwrap();

        // Create message metadata
        let msg_metadata = serde_json::json!({
            "id": "msg_test123",
            "sessionID": "session-123",
            "role": "assistant",
            "time": {
                "created": 1700000000000i64,
                "completed": 1700000001000i64
            },
            "modelID": "test-model",
            "tokens": {
                "input": 100,
                "output": 50
            }
        });
        tokio::fs::write(msg_dir.join("msg_test123.json"), msg_metadata.to_string())
            .await
            .unwrap();

        // Create part content in part/<message_id>/ directory
        let part_dir = adapter.data_dir.join("part/msg_test123");
        tokio::fs::create_dir_all(&part_dir).await.unwrap();

        let part_content = serde_json::json!({
            "id": "prt_test789",
            "type": "text",
            "text": "Hello, world!",
            "messageID": "msg_test123",
            "sessionID": "session-123"
        });
        tokio::fs::write(part_dir.join("prt_test789.json"), part_content.to_string())
            .await
            .unwrap();

        // Load messages
        let messages = adapter.messages("session-123").await.unwrap();
        assert_eq!(messages.len(), 1);

        let msg = &messages[0];
        assert_eq!(msg.id, "msg_test123");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content, "Hello, world!");
        assert_eq!(msg.model, Some("test-model".to_string()));

        // Verify timestamp (1700000000000 ms = 2023-11-14 22:13:20 UTC)
        assert_eq!(msg.timestamp.timestamp(), 1700000000);

        // Verify tokens
        assert!(msg.tokens.is_some());
        let tokens = msg.tokens.unwrap();
        assert_eq!(tokens.prompt_tokens, 100);
        assert_eq!(tokens.completion_tokens, 50);
        assert_eq!(tokens.total_tokens, 150);
    }

    #[tokio::test]
    async fn test_messages_with_missing_parts() {
        let (adapter, _temp) = create_test_adapter();

        // Create message directory
        let msg_dir = adapter.data_dir.join("message/session-no-parts");
        tokio::fs::create_dir_all(&msg_dir).await.unwrap();

        // Create message metadata without corresponding part file
        let msg_metadata = serde_json::json!({
            "id": "msg_nopart",
            "sessionID": "session-no-parts",
            "role": "user",
            "time": {
                "created": 1700000000000i64,
                "completed": null
            }
        });
        tokio::fs::write(msg_dir.join("msg_nopart.json"), msg_metadata.to_string())
            .await
            .unwrap();

        // Load messages - should handle gracefully
        let messages = adapter.messages("session-no-parts").await.unwrap();
        assert_eq!(messages.len(), 1);

        let msg = &messages[0];
        assert_eq!(msg.id, "msg_nopart");
        assert_eq!(msg.role, Role::User);
        assert!(msg.content.is_empty()); // Empty content when parts are missing
    }

    #[tokio::test]
    async fn test_usage_calculation() {
        let (adapter, _temp) = create_test_adapter();

        // Create message directory
        let msg_dir = adapter.data_dir.join("message/session-usage");
        tokio::fs::create_dir_all(&msg_dir).await.unwrap();

        // Create multiple messages with tokens
        let messages_data = vec![("msg_1", 100, 50), ("msg_2", 200, 100), ("msg_3", 150, 75)];

        for (id, input, output) in &messages_data {
            let msg_json = serde_json::json!({
                "id": id,
                "sessionID": "session-usage",
                "role": "user",
                "time": {
                    "created": 1700000000000i64,
                    "completed": null
                },
                "tokens": {
                    "input": input,
                    "output": output
                }
            });
            tokio::fs::write(msg_dir.join(format!("{}.json", id)), msg_json.to_string())
                .await
                .unwrap();
        }

        // Calculate usage
        let usage = adapter.usage("session-usage").await.unwrap();
        assert!(usage.is_some());

        let usage = usage.unwrap();
        assert_eq!(usage.prompt_tokens, 450); // 100 + 200 + 150
        assert_eq!(usage.completion_tokens, 225); // 50 + 100 + 75
        assert_eq!(usage.total_tokens, 675); // 450 + 225
    }

    #[tokio::test]
    async fn test_usage_no_tokens() {
        let (adapter, _temp) = create_test_adapter();

        // Create message directory with messages that have no tokens
        let msg_dir = adapter.data_dir.join("message/session-no-tokens");
        tokio::fs::create_dir_all(&msg_dir).await.unwrap();

        let msg_json = serde_json::json!({
            "id": "msg_notokens",
            "sessionID": "session-no-tokens",
            "role": "user",
            "time": {
                "created": 1700000000000i64,
                "completed": null
            }
            // No tokens field
        });
        tokio::fs::write(msg_dir.join("msg_notokens.json"), msg_json.to_string())
            .await
            .unwrap();

        // Usage should be None when no tokens are present
        let usage = adapter.usage("session-no-tokens").await.unwrap();
        assert!(usage.is_none());
    }

    #[tokio::test]
    async fn test_messages_role_mapping() {
        let (adapter, _temp) = create_test_adapter();

        let msg_dir = adapter.data_dir.join("message/session-roles");
        tokio::fs::create_dir_all(&msg_dir).await.unwrap();

        // Test different role mappings
        let roles = vec![
            ("user", Role::User),
            ("assistant", Role::Assistant),
            ("system", Role::System),
            ("tool", Role::Tool),
        ];

        for (role_str, _) in &roles {
            let msg_json = serde_json::json!({
                "id": format!("msg_{}", role_str),
                "sessionID": "session-roles",
                "role": role_str,
                "time": {
                    "created": 1700000000000i64,
                    "completed": null
                }
            });
            tokio::fs::write(
                msg_dir.join(format!("msg_{}.json", role_str)),
                msg_json.to_string(),
            )
            .await
            .unwrap();
        }

        let messages = adapter.messages("session-roles").await.unwrap();
        assert_eq!(messages.len(), 4);

        for (role_str, expected_role) in roles {
            let id = format!("msg_{}", role_str);
            let message = messages
                .iter()
                .find(|message| message.id == id)
                .unwrap_or_else(|| panic!("missing message {id}"));
            assert_eq!(message.role, expected_role);
        }
    }

    #[tokio::test]
    async fn test_messages_multiple_parts() {
        let (adapter, _temp) = create_test_adapter();

        let msg_dir = adapter.data_dir.join("message/session-multi-parts");
        tokio::fs::create_dir_all(&msg_dir).await.unwrap();

        // Create message metadata
        let msg_json = serde_json::json!({
            "id": "msg_multi",
            "sessionID": "session-multi-parts",
            "role": "user",
            "time": {
                "created": 1700000000000i64,
                "completed": null
            }
        });
        tokio::fs::write(msg_dir.join("msg_multi.json"), msg_json.to_string())
            .await
            .unwrap();

        // Create multiple parts in part/<message_id>/ directory
        let part_dir = adapter.data_dir.join("part/msg_multi");
        tokio::fs::create_dir_all(&part_dir).await.unwrap();

        let parts = ["First part", "Second part", "Third part"];
        for (i, text) in parts.iter().enumerate() {
            let part_json = serde_json::json!({
                "id": format!("prt_{:02}", i),  // Use zero-padded numbers for consistent ordering
                "type": "text",
                "text": text,
                "messageID": "msg_multi",
                "sessionID": "session-multi-parts"
            });
            tokio::fs::write(
                part_dir.join(format!("prt_{:02}.json", i)),
                part_json.to_string(),
            )
            .await
            .unwrap();
        }

        let messages = adapter.messages("session-multi-parts").await.unwrap();
        assert_eq!(messages.len(), 1);

        // Parts should be joined with newlines (order depends on file system)
        let content_parts: Vec<&str> = messages[0].content.split('\n').collect();
        assert_eq!(content_parts.len(), 3);
        assert!(content_parts.contains(&"First part"));
        assert!(content_parts.contains(&"Second part"));
        assert!(content_parts.contains(&"Third part"));
    }

    #[tokio::test]
    async fn test_messages_sorted_by_timestamp() {
        let (adapter, _temp) = create_test_adapter();

        let msg_dir = adapter.data_dir.join("message/session-sorted");
        tokio::fs::create_dir_all(&msg_dir).await.unwrap();

        // Create messages with different timestamps in random order
        let messages = vec![
            ("msg_1", 1700000000000i64, "First"),
            ("msg_2", 1700000005000i64, "Second"),
            ("msg_3", 1700000003000i64, "Third"),
        ];

        for (id, created, content) in &messages {
            let msg_json = serde_json::json!({
                "id": id,
                "sessionID": "session-sorted",
                "role": "user",
                "time": {
                    "created": created,
                    "completed": null
                }
            });
            tokio::fs::write(msg_dir.join(format!("{}.json", id)), msg_json.to_string())
                .await
                .unwrap();

            // Create corresponding part in part/<message_id>/ directory
            let part_dir = adapter.data_dir.join("part").join(id);
            tokio::fs::create_dir_all(&part_dir).await.unwrap();

            let part_json = serde_json::json!({
                "id": format!("prt_{}", id),
                "type": "text",
                "text": content,
                "messageID": id,
                "sessionID": "session-sorted"
            });
            tokio::fs::write(
                part_dir.join(format!("prt_{}.json", id)),
                part_json.to_string(),
            )
            .await
            .unwrap();
        }

        let loaded_messages = adapter.messages("session-sorted").await.unwrap();
        assert_eq!(loaded_messages.len(), 3);

        // Verify they are sorted by timestamp
        assert_eq!(loaded_messages[0].id, "msg_1"); // Earliest
        assert_eq!(loaded_messages[1].id, "msg_3"); // Middle
        assert_eq!(loaded_messages[2].id, "msg_2"); // Latest
    }

    #[tokio::test]
    async fn test_messages_sorted_by_id_when_timestamps_match() {
        let (adapter, _temp) = create_test_adapter();

        let msg_dir = adapter.data_dir.join("message/session-same-time");
        tokio::fs::create_dir_all(&msg_dir).await.unwrap();

        for id in ["msg_c", "msg_a", "msg_b"] {
            let msg_json = serde_json::json!({
                "id": id,
                "sessionID": "session-same-time",
                "role": "user",
                "time": {
                    "created": 1700000000000i64,
                    "completed": null
                }
            });
            tokio::fs::write(msg_dir.join(format!("{}.json", id)), msg_json.to_string())
                .await
                .unwrap();
        }

        let loaded_messages = adapter.messages("session-same-time").await.unwrap();
        let loaded_ids: Vec<&str> = loaded_messages
            .iter()
            .map(|message| message.id.as_str())
            .collect();

        assert_eq!(loaded_ids, vec!["msg_a", "msg_b", "msg_c"]);
    }

    #[tokio::test]
    async fn test_sessions_sorted_by_updated_at() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = OpenCodeAdapter::with_data_dir(temp_dir.path().to_path_buf());

        // Create session directories in message/ subdirectory with explicit timestamps by modifying them
        tokio::fs::create_dir_all(adapter.data_dir.join("message/session-a"))
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        tokio::fs::create_dir_all(adapter.data_dir.join("message/session-b"))
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        tokio::fs::create_dir_all(adapter.data_dir.join("message/session-c"))
            .await
            .unwrap();

        // Sleep to ensure all timestamps are different
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Touch session-b to make it the most recently updated
        tokio::fs::write(adapter.data_dir.join("message/session-b/marker"), "test")
            .await
            .unwrap();

        let loaded_sessions = adapter.sessions(Path::new("/test/project")).await.unwrap();
        assert_eq!(loaded_sessions.len(), 3);

        // Verify sessions are sorted by updated_at (newest first)
        // Just verify the order is sorted, don't rely on specific session being first
        let mut prev_updated = None;
        for session in &loaded_sessions {
            if let Some(prev) = prev_updated {
                // Each session should have updated_at <= previous (newest first)
                assert!(
                    session.updated_at <= prev,
                    "Session {} should be after {} in time",
                    session.id,
                    prev
                );
            }
            prev_updated = Some(session.updated_at);
        }
    }

    #[tokio::test]
    async fn test_non_json_files_ignored() {
        let (adapter, _temp) = create_test_adapter();

        let msg_dir = adapter.data_dir.join("message/session-non-json");
        tokio::fs::create_dir_all(&msg_dir).await.unwrap();

        // Create a valid JSON message
        let msg_json = serde_json::json!({
            "id": "msg_valid",
            "sessionID": "session-non-json",
            "role": "user",
            "time": {
                "created": 1700000000000i64,
                "completed": null
            }
        });
        tokio::fs::write(msg_dir.join("msg_valid.json"), msg_json.to_string())
            .await
            .unwrap();

        // Create non-JSON files that should be ignored
        tokio::fs::write(msg_dir.join("readme.txt"), "Not a JSON file")
            .await
            .unwrap();
        tokio::fs::write(msg_dir.join("data.bin"), &[0, 1, 2, 3])
            .await
            .unwrap();
        tokio::fs::write(msg_dir.join(".hidden"), "hidden file")
            .await
            .unwrap();

        let messages = adapter.messages("session-non-json").await.unwrap();
        // Should only load the valid JSON message
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "msg_valid");
    }

    #[tokio::test]
    async fn test_parts_with_missing_text_field() {
        let (adapter, _temp) = create_test_adapter();

        let msg_dir = adapter.data_dir.join("message/session-missing-text");
        tokio::fs::create_dir_all(&msg_dir).await.unwrap();

        // Create message metadata
        let msg_json = serde_json::json!({
            "id": "msg_notext",
            "sessionID": "session-missing-text",
            "role": "user",
            "time": {
                "created": 1700000000000i64,
                "completed": null
            }
        });
        tokio::fs::write(msg_dir.join("msg_notext.json"), msg_json.to_string())
            .await
            .unwrap();

        // Create a part without text field in part/<message_id>/ directory
        let part_dir = adapter.data_dir.join("part/msg_notext");
        tokio::fs::create_dir_all(&part_dir).await.unwrap();

        let part_json = serde_json::json!({
            "id": "prt_notext",
            "type": "image",
            "messageID": "msg_notext",
            "sessionID": "session-missing-text"
        });
        tokio::fs::write(part_dir.join("prt_notext.json"), part_json.to_string())
            .await
            .unwrap();

        let messages = adapter.messages("session-missing-text").await.unwrap();
        assert_eq!(messages.len(), 1);
        // Content should be empty (part with no text is filtered out)
        assert!(messages[0].content.is_empty());
    }

    #[tokio::test]
    async fn test_messages_with_mixed_valid_and_invalid_files() {
        let (adapter, _temp) = create_test_adapter();

        let msg_dir = adapter.data_dir.join("message/session-mixed");
        tokio::fs::create_dir_all(&msg_dir).await.unwrap();

        // Create a valid message
        let valid_msg = serde_json::json!({
            "id": "msg_valid",
            "sessionID": "session-mixed",
            "role": "user",
            "time": {
                "created": 1700000000000i64,
                "completed": null
            }
        });
        tokio::fs::write(msg_dir.join("msg_valid.json"), valid_msg.to_string())
            .await
            .unwrap();

        // Create an invalid JSON message (malformed)
        tokio::fs::write(msg_dir.join("msg_invalid.json"), "{ invalid json }")
            .await
            .unwrap();

        // Create another valid message
        let another_msg = serde_json::json!({
            "id": "msg_another",
            "sessionID": "session-mixed",
            "role": "assistant",
            "time": {
                "created": 1700000001000i64,
                "completed": null
            }
        });
        tokio::fs::write(msg_dir.join("msg_another.json"), another_msg.to_string())
            .await
            .unwrap();

        let messages = adapter.messages("session-mixed").await;
        // Should fail because of the invalid JSON
        assert!(messages.is_err());

        // Verify the error message
        let error = messages.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Failed to parse message metadata")
        );
    }

    #[tokio::test]
    async fn test_messages_empty_text_field() {
        let (adapter, _temp) = create_test_adapter();

        let msg_dir = adapter.data_dir.join("message/session-empty-text");
        tokio::fs::create_dir_all(&msg_dir).await.unwrap();

        // Create message metadata
        let msg_json = serde_json::json!({
            "id": "msg_empty",
            "sessionID": "session-empty-text",
            "role": "user",
            "time": {
                "created": 1700000000000i64,
                "completed": null
            }
        });
        tokio::fs::write(msg_dir.join("msg_empty.json"), msg_json.to_string())
            .await
            .unwrap();

        // Create a part with empty text in part/<message_id>/ directory
        let part_dir = adapter.data_dir.join("part/msg_empty");
        tokio::fs::create_dir_all(&part_dir).await.unwrap();

        let part_json = serde_json::json!({
            "id": "prt_empty",
            "type": "text",
            "text": "",
            "messageID": "msg_empty",
            "sessionID": "session-empty-text"
        });
        tokio::fs::write(part_dir.join("prt_empty.json"), part_json.to_string())
            .await
            .unwrap();

        let messages = adapter.messages("session-empty-text").await.unwrap();
        assert_eq!(messages.len(), 1);
        // Empty text should be included as empty string
        assert_eq!(messages[0].content, "");
    }

    #[tokio::test]
    async fn test_unknown_role_defaults_to_user() {
        let (adapter, _temp) = create_test_adapter();

        let msg_dir = adapter.data_dir.join("message/session-unknown-role");
        tokio::fs::create_dir_all(&msg_dir).await.unwrap();

        // Create message with unknown role
        let msg_json = serde_json::json!({
            "id": "msg_unknown",
            "sessionID": "session-unknown-role",
            "role": "unknown_role",
            "time": {
                "created": 1700000000000i64,
                "completed": null
            }
        });
        tokio::fs::write(msg_dir.join("msg_unknown.json"), msg_json.to_string())
            .await
            .unwrap();

        let messages = adapter.messages("session-unknown-role").await.unwrap();
        assert_eq!(messages.len(), 1);
        // Unknown role should default to User
        assert_eq!(messages[0].role, Role::User);
    }
}
