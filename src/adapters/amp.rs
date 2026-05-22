//! Amp adapter
//!
//! This adapter integrates with the Amp AI code editor.
//! Amp is primarily a server-side tool with no local data storage,
//! so detection always returns false and sessions/messages are stubbed.

use crate::adapters::types::{Adapter, AdapterType, Result};
use crate::core::models::conversation::{Message, Session, TokenUsage};
use async_trait::async_trait;
use std::path::Path;

/// Amp adapter implementation
#[derive(Debug)]
pub struct AmpAdapter;

impl AmpAdapter {
    /// Create a new Amp adapter
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl Default for AmpAdapter {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl Adapter for AmpAdapter {
    fn id(&self) -> &str {
        "amp"
    }

    fn name(&self) -> &str {
        "Amp"
    }

    fn icon(&self) -> char {
        '◎'
    }

    fn adapter_type(&self) -> AdapterType {
        AdapterType::Amp
    }

    async fn detect(&self, _project_root: &Path) -> Result<bool> {
        // Amp is a server-side tool with no local data
        Ok(false)
    }

    async fn sessions(&self, _project_root: &Path) -> Result<Vec<Session>> {
        // Amp has no local session data
        Ok(vec![])
    }

    async fn messages(&self, _session_id: &str) -> Result<Vec<Message>> {
        // Amp has no local message data
        Ok(vec![])
    }

    async fn usage(&self, _session_id: &str) -> Result<Option<TokenUsage>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_succeeds() {
        let result = AmpAdapter::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_adapter_properties() {
        let adapter = AmpAdapter::new().unwrap();
        assert_eq!(adapter.id(), "amp");
        assert_eq!(adapter.name(), "Amp");
        assert_eq!(adapter.icon(), '◎');
        assert_eq!(adapter.adapter_type(), AdapterType::Amp);
    }

    #[tokio::test]
    async fn test_detect_always_false() {
        let adapter = AmpAdapter::new().unwrap();

        let detected = adapter.detect(Path::new("/test/project")).await.unwrap();
        assert!(!detected);
    }

    #[tokio::test]
    async fn test_sessions_empty() {
        let adapter = AmpAdapter::new().unwrap();

        let sessions = adapter.sessions(Path::new("/test/project")).await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_messages_empty() {
        let adapter = AmpAdapter::new().unwrap();

        let messages = adapter.messages("any-session").await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn test_usage_none() {
        let adapter = AmpAdapter::new().unwrap();

        let usage = adapter.usage("any-session").await.unwrap();
        assert!(usage.is_none());
    }
}
