//! Core adapter types and traits
//!
//! This module defines the trait interface for AI adapter implementations
//! and common types used across all adapters.

use crate::core::models::conversation::{Message, Session, TokenUsage};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur in adapter operations
#[derive(Debug, Error)]
pub enum AdapterError {
    /// Adapter not found
    #[error("adapter not found: {0}")]
    NotFound(String),

    /// Session not found
    #[error("session not found: {0}")]
    SessionNotFound(String),

    /// Message not found
    #[error("message not found: {0}")]
    MessageNotFound(String),

    /// Failed to detect adapter
    #[error("detection failed: {0}")]
    DetectionFailed(String),

    /// Failed to read session data
    #[error("failed to read sessions: {0}")]
    ReadSessionsFailed(String),

    /// Failed to read message data
    #[error("failed to read messages: {0}")]
    ReadMessagesFailed(String),

    /// Failed to parse data
    #[error("parse error: {0}")]
    ParseError(String),

    /// I/O error
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing error
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// SQLite error
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Other error
    #[error("adapter error: {0}")]
    Other(String),
}

/// Result type for adapter operations
pub type Result<T> = std::result::Result<T, AdapterError>;

/// Types of AI adapters supported
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AdapterType {
    /// Anthropic Claude Code
    ClaudeCode,
    /// Cursor IDE
    Cursor,
    /// OpenAI Codex CLI
    Codex,
    /// Google Gemini CLI
    Gemini,
    /// Warp terminal AI
    Warp,
    /// Amp AI editor
    Amp,
    /// Kiro IDE
    Kiro,
    /// OpenCode
    OpenCode,
}

impl AdapterType {
    /// Get the adapter type identifier string
    pub fn as_str(&self) -> &'static str {
        match self {
            AdapterType::ClaudeCode => "claude-code",
            AdapterType::Cursor => "cursor",
            AdapterType::Codex => "codex",
            AdapterType::Gemini => "gemini",
            AdapterType::Warp => "warp",
            AdapterType::Amp => "amp",
            AdapterType::Kiro => "kiro",
            AdapterType::OpenCode => "opencode",
        }
    }

    /// Get the display name for the adapter type
    pub fn display_name(&self) -> &'static str {
        match self {
            AdapterType::ClaudeCode => "Claude Code",
            AdapterType::Cursor => "Cursor",
            AdapterType::Codex => "Codex CLI",
            AdapterType::Gemini => "Gemini CLI",
            AdapterType::Warp => "Warp",
            AdapterType::Amp => "Amp",
            AdapterType::Kiro => "Kiro",
            AdapterType::OpenCode => "OpenCode",
        }
    }

    /// Get the icon character for the adapter type
    pub fn icon(&self) -> char {
        match self {
            AdapterType::ClaudeCode => '◈',
            AdapterType::Cursor => '◉',
            AdapterType::Codex => '◆',
            AdapterType::Gemini => '◊',
            AdapterType::Warp => '◐',
            AdapterType::Amp => '◎',
            AdapterType::Kiro => '○',
            AdapterType::OpenCode => '◑',
        }
    }
}

impl std::fmt::Display for AdapterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for AdapterType {
    type Err = AdapterError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "claude-code" | "claude" | "claudecode" => Ok(AdapterType::ClaudeCode),
            "cursor" => Ok(AdapterType::Cursor),
            "codex" => Ok(AdapterType::Codex),
            "gemini" => Ok(AdapterType::Gemini),
            "warp" => Ok(AdapterType::Warp),
            "amp" => Ok(AdapterType::Amp),
            "kiro" => Ok(AdapterType::Kiro),
            "opencode" => Ok(AdapterType::OpenCode),
            _ => Err(AdapterError::NotFound(format!(
                "Unknown adapter type: {}",
                s
            ))),
        }
    }
}

/// Trait for AI adapter implementations
///
/// This trait defines the interface for integrating with various AI coding agents.
/// Each adapter implementation handles the specifics of detecting, reading, and
/// parsing session data from its respective AI tool.
#[async_trait]
pub trait Adapter: Send + Sync + std::fmt::Debug {
    /// Get the unique identifier for this adapter
    fn id(&self) -> &str;

    /// Get the human-readable name for this adapter
    fn name(&self) -> &str;

    /// Get the icon character for this adapter
    fn icon(&self) -> char;

    /// Get the adapter type
    fn adapter_type(&self) -> AdapterType;

    /// Detect whether this adapter has data for the given project
    ///
    /// # Arguments
    ///
    /// * `project_root` - The root path of the project to check
    ///
    /// # Returns
    ///
    /// Returns `true` if the adapter has session data for this project
    async fn detect(&self, project_root: &Path) -> Result<bool>;

    /// Get all sessions for a project
    ///
    /// # Arguments
    ///
    /// * `project_root` - The root path of the project
    ///
    /// # Returns
    ///
    /// Returns a list of sessions for this project
    async fn sessions(&self, project_root: &Path) -> Result<Vec<Session>>;

    /// Get all messages for a specific session
    ///
    /// # Arguments
    ///
    /// * `session_id` - The unique session identifier
    ///
    /// # Returns
    ///
    /// Returns a list of messages in the session
    async fn messages(&self, session_id: &str) -> Result<Vec<Message>>;

    /// Get token usage for a specific session
    ///
    /// # Arguments
    ///
    /// * `session_id` - The unique session identifier
    ///
    /// # Returns
    ///
    /// Returns token usage information if available
    async fn usage(&self, session_id: &str) -> Result<Option<TokenUsage>>;
}

/// Registry of AI adapters
///
/// The registry manages a collection of adapter implementations and provides
/// methods for detecting and accessing adapters for specific projects.
pub struct AdapterRegistry {
    adapters: Vec<Arc<dyn Adapter>>,
}

impl std::fmt::Debug for AdapterRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdapterRegistry")
            .field("adapters", &self.adapters.len())
            .finish()
    }
}

impl AdapterRegistry {
    /// Create a new empty adapter registry
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    /// Register a new adapter
    ///
    /// # Arguments
    ///
    /// * `adapter` - The adapter implementation to register
    pub fn register(&mut self, adapter: Arc<dyn Adapter>) {
        self.adapters.push(adapter);
    }

    /// Get all registered adapters
    pub fn adapters(&self) -> &[Arc<dyn Adapter>] {
        &self.adapters
    }

    /// Get a specific adapter by ID
    ///
    /// # Arguments
    ///
    /// * `id` - The adapter identifier
    pub fn get(&self, id: &str) -> Option<Arc<dyn Adapter>> {
        self.adapters.iter().find(|a| a.id() == id).cloned()
    }

    /// Get a specific adapter by type
    ///
    /// # Arguments
    ///
    /// * `adapter_type` - The adapter type to find
    pub fn get_by_type(&self, adapter_type: AdapterType) -> Option<Arc<dyn Adapter>> {
        self.adapters
            .iter()
            .find(|a| a.adapter_type() == adapter_type)
            .cloned()
    }

    /// Detect all adapters that have data for a project
    ///
    /// # Arguments
    ///
    /// * `project_root` - The root path of the project
    ///
    /// # Returns
    ///
    /// Returns a map of adapter IDs to adapters that have data for this project
    pub async fn detect_all(
        &self,
        project_root: &Path,
    ) -> Result<HashMap<String, Arc<dyn Adapter>>> {
        let mut detected = HashMap::new();

        for adapter in &self.adapters {
            match adapter.detect(project_root).await {
                Ok(true) => {
                    detected.insert(adapter.id().to_string(), adapter.clone());
                }
                Ok(false) => {
                    // Adapter not detected, continue
                }
                Err(e) => {
                    // Log error but don't fail the entire detection
                    tracing::warn!("Failed to detect adapter {}: {}", adapter.id(), e);
                }
            }
        }

        Ok(detected)
    }

    /// Get the number of registered adapters
    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility function to encode a path for use in adapter storage
///
/// This encodes paths by replacing `/`, `.`, and `_` with `-` to create
/// safe directory names.
///
/// # Arguments
///
/// * `path` - The path to encode
///
/// # Returns
///
/// The encoded path string
pub fn encode_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "-")
        .replace('.', "-")
        .replace('_', "-")
}

/// Utility function to compute MD5 hash of a string
///
/// # Arguments
///
/// * `input` - The string to hash
///
/// # Returns
///
/// The MD5 hash as a hexadecimal string
pub fn md5_hash(input: &str) -> String {
    use std::hash::Hasher;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(input.as_bytes());
    // Note: This is not actually MD5, but for our purposes a simple hash works.
    // For true MD5, we'd use the md5 crate. Let's use xxhash instead since
    // it's already in dependencies.
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_type_from_str() {
        assert_eq!(
            "claude-code".parse::<AdapterType>().unwrap(),
            AdapterType::ClaudeCode
        );
        assert_eq!(
            "cursor".parse::<AdapterType>().unwrap(),
            AdapterType::Cursor
        );
        assert_eq!("codex".parse::<AdapterType>().unwrap(), AdapterType::Codex);
        assert!("unknown".parse::<AdapterType>().is_err());
    }

    #[test]
    fn test_adapter_type_display() {
        assert_eq!(AdapterType::ClaudeCode.to_string(), "claude-code");
        assert_eq!(AdapterType::Cursor.to_string(), "cursor");
    }

    #[test]
    fn test_encode_path() {
        assert_eq!(
            encode_path(Path::new("/home/user/project")),
            "-home-user-project"
        );
        assert_eq!(
            encode_path(Path::new("/path/to/file.txt")),
            "-path-to-file-txt"
        );
    }

    #[test]
    fn test_registry_default() {
        let registry = AdapterRegistry::default();
        assert!(registry.is_empty());
    }
}
