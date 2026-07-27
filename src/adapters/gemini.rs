//! Gemini CLI adapter
//!
//! This adapter integrates with Google's Gemini CLI tool.
//! It reads conversation data from the local storage directory at
//! `~/.gemini/`.

use crate::adapters::types::{Adapter, AdapterError, AdapterType, Result};
use crate::core::models::conversation::{Message, Session, TokenUsage};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

/// Gemini CLI adapter implementation
#[derive(Debug)]
pub struct GeminiAdapter {
    /// Base directory for Gemini storage
    data_dir: PathBuf,
}

impl GeminiAdapter {
    /// Create a new Gemini adapter with default paths
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| AdapterError::Other("Could not find home directory".to_string()))?;

        Ok(Self {
            data_dir: home_dir.join(".gemini"),
        })
    }

    /// Create a new Gemini adapter with a custom data directory
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }
}

impl Default for GeminiAdapter {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            data_dir: PathBuf::from(".gemini"),
        })
    }
}

#[async_trait]
impl Adapter for GeminiAdapter {
    fn id(&self) -> &str {
        "gemini"
    }

    fn name(&self) -> &str {
        "Gemini CLI"
    }

    fn icon(&self) -> char {
        '◊'
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::Gemini
    }

    async fn detect(&self, _project_root: &Path) -> Result<bool> {
        Ok(self.data_dir.exists())
    }

    async fn sessions(&self, _project_root: &Path) -> Result<Vec<Session>> {
        if !self.data_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.data_dir).await?;

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

            let mut session = Session::new(&session_id, session_id.clone(), self.id(), created_at);
            session.created_at = created_at;
            session.updated_at = updated_at;

            sessions.push(session);
        }

        // Sort by updated_at, newest first
        sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_at));

        Ok(sessions)
    }

    async fn messages(&self, _session_id: &str) -> Result<Vec<Message>> {
        // Stub: exact format for Gemini CLI conversations is not yet known
        Ok(vec![])
    }

    async fn usage(&self, _session_id: &str) -> Result<Option<TokenUsage>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_adapter() -> (GeminiAdapter, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_data_dir(temp_dir.path().to_path_buf());
        (adapter, temp_dir)
    }

    #[test]
    fn test_new_succeeds() {
        let result = GeminiAdapter::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_adapter_properties() {
        let (adapter, _temp) = create_test_adapter();
        assert_eq!(adapter.id(), "gemini");
        assert_eq!(adapter.name(), "Gemini CLI");
        assert_eq!(adapter.icon(), '◊');
        assert_eq!(adapter.adapter_type(), AdapterType::Gemini);
    }

    #[tokio::test]
    async fn test_detect_no_data() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_data_dir(temp_dir.path().join("nonexistent"));

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
        let adapter = GeminiAdapter::with_data_dir(temp_dir.path().join("nonexistent"));

        let sessions = adapter.sessions(Path::new("/test/project")).await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_sessions_with_dirs() {
        let (adapter, _temp) = create_test_adapter();

        // Create some session directories
        tokio::fs::create_dir_all(adapter.data_dir.join("session-1"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(adapter.data_dir.join("session-2"))
            .await
            .unwrap();

        let sessions = adapter.sessions(Path::new("/test/project")).await.unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[tokio::test]
    async fn test_messages_stub() {
        let (adapter, _temp) = create_test_adapter();

        let messages = adapter.messages("any-session").await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn test_usage_stub() {
        let (adapter, _temp) = create_test_adapter();

        let usage = adapter.usage("any-session").await.unwrap();
        assert!(usage.is_none());
    }
}
