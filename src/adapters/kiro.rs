//! Kiro adapter
//!
//! This adapter integrates with the Kiro AI IDE.
//! It reads configuration and session data from
//! `~/.kiro/`.

use crate::adapters::types::{Adapter, AdapterError, AdapterType, Result};
use crate::core::models::conversation::{Message, Session, TokenUsage};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// Kiro adapter implementation
#[derive(Debug)]
pub struct KiroAdapter {
    /// Base directory for Kiro data
    data_dir: PathBuf,
}

impl KiroAdapter {
    /// Create a new Kiro adapter with default paths
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| AdapterError::Other("Could not find home directory".to_string()))?;

        Ok(Self {
            data_dir: home_dir.join(".kiro"),
        })
    }

    /// Create a new Kiro adapter with a custom data directory
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }
}

impl Default for KiroAdapter {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            data_dir: PathBuf::from(".kiro"),
        })
    }
}

#[async_trait]
impl Adapter for KiroAdapter {
    fn id(&self) -> &str {
        "kiro"
    }

    fn name(&self) -> &str {
        "Kiro"
    }

    fn icon(&self) -> char {
        '○'
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::Kiro
    }

    async fn detect(&self, _project_root: &Path) -> Result<bool> {
        Ok(self.data_dir.exists())
    }

    async fn sessions(&self, _project_root: &Path) -> Result<Vec<Session>> {
        // Stub: Kiro session format is not yet known
        Ok(vec![])
    }

    async fn messages(&self, _session_id: &str) -> Result<Vec<Message>> {
        // Stub: Kiro message format is not yet known
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

    fn create_test_adapter() -> (KiroAdapter, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let adapter = KiroAdapter::with_data_dir(temp_dir.path().to_path_buf());
        (adapter, temp_dir)
    }

    #[test]
    fn test_new_succeeds() {
        let result = KiroAdapter::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_adapter_properties() {
        let (adapter, _temp) = create_test_adapter();
        assert_eq!(adapter.id(), "kiro");
        assert_eq!(adapter.name(), "Kiro");
        assert_eq!(adapter.icon(), '○');
        assert_eq!(adapter.adapter_type(), AdapterType::Kiro);
    }

    #[tokio::test]
    async fn test_detect_no_data() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = KiroAdapter::with_data_dir(temp_dir.path().join("nonexistent"));

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
    async fn test_sessions_stub() {
        let (adapter, _temp) = create_test_adapter();

        let sessions = adapter.sessions(Path::new("/test/project")).await.unwrap();
        assert!(sessions.is_empty());
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
