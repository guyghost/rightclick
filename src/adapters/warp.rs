//! Warp adapter
//!
//! This adapter integrates with the Warp AI terminal.
//! On macOS, it reads data from `~/Library/Application Support/dev.warp.Warp/`.
//! On Linux, it reads data from `~/.local/share/warp/`.
//! Warp may use SQLite for storage, but the exact schema is not yet known.

use crate::adapters::types::{Adapter, AdapterError, AdapterType, Result};
use crate::core::models::conversation::{Message, Session, TokenUsage};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// Warp adapter implementation
#[derive(Debug)]
pub struct WarpAdapter {
    /// Data directory for Warp storage
    data_dir: PathBuf,
}

impl WarpAdapter {
    /// Create a new Warp adapter with default paths
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| AdapterError::Other("Could not find home directory".to_string()))?;

        // Platform-specific data directory
        let data_dir = if cfg!(target_os = "macos") {
            home_dir.join("Library/Application Support/dev.warp.Warp")
        } else {
            home_dir.join(".local/share/warp")
        };

        Ok(Self { data_dir })
    }

    /// Create a new Warp adapter with a custom data directory
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }
}

impl Default for WarpAdapter {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            data_dir: PathBuf::from("warp"),
        })
    }
}

#[async_trait]
impl Adapter for WarpAdapter {
    fn id(&self) -> &str {
        "warp"
    }

    fn name(&self) -> &str {
        "Warp"
    }

    fn icon(&self) -> char {
        '◐'
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::Warp
    }

    async fn detect(&self, _project_root: &Path) -> Result<bool> {
        Ok(self.data_dir.exists())
    }

    async fn sessions(&self, _project_root: &Path) -> Result<Vec<Session>> {
        // Stub: Warp likely uses SQLite, but the schema is not yet known
        Ok(vec![])
    }

    async fn messages(&self, _session_id: &str) -> Result<Vec<Message>> {
        // Stub: Warp message format is not yet known
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

    fn create_test_adapter() -> (WarpAdapter, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let adapter = WarpAdapter::with_data_dir(temp_dir.path().to_path_buf());
        (adapter, temp_dir)
    }

    #[test]
    fn test_new_succeeds() {
        let result = WarpAdapter::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_adapter_properties() {
        let (adapter, _temp) = create_test_adapter();
        assert_eq!(adapter.id(), "warp");
        assert_eq!(adapter.name(), "Warp");
        assert_eq!(adapter.icon(), '◐');
        assert_eq!(adapter.adapter_type(), AdapterType::Warp);
    }

    #[tokio::test]
    async fn test_detect_no_data() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = WarpAdapter::with_data_dir(temp_dir.path().join("nonexistent"));

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
