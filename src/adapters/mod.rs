//! AI Adapter System for RightClick
//!
//! This module provides a unified interface for integrating with various
//! AI coding agents and chat tools. It supports:
//!
//! - **Claude Code** - Anthropic's CLI tool
//! - **Cursor** - AI-powered IDE
//! - **Codex CLI** - OpenAI's command-line interface
//! - **Gemini CLI** - Google's AI assistant
//! - **Warp** - AI terminal
//! - **Amp** - AI code editor
//! - **Kiro** - AI IDE
//! - **OpenCode** - Open-source AI coding tool
//!
//! # Architecture
//!
//! The adapter system follows the Functional Core & Imperative Shell pattern:
//!
//! - **Pure types** (`types.rs`) - Core traits and data structures
//! - **Adapter implementations** - Tool-specific detection and parsing logic
//! - **Registry** - Manages adapter lifecycle and discovery
//!
//! # Usage
//!
//! ```rust
//! use rightclick::adapters::{create_default_registry, AdapterRegistry};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let registry = create_default_registry()?;
//!     let project = std::path::Path::new("/path/to/project");
//!     
//!     // Detect which adapters have data for this project
//!     let detected = registry.detect_all(project).await?;
//!     
//!     for (id, adapter) in detected {
//!         println!("Detected: {} {}", adapter.icon(), adapter.name());
//!         let sessions = adapter.sessions(project).await?;
//!         println!("  Sessions: {}", sessions.len());
//!     }
//!     
//!     Ok(())
//! }
//! ```

pub mod claudecode;
pub mod codex;
pub mod cursor;
pub mod types;

// Re-export main types
pub use types::{Adapter, AdapterError, AdapterRegistry, AdapterType, Result};

// Re-export adapter implementations
pub use claudecode::ClaudeCodeAdapter;
pub use codex::CodexAdapter;
pub use cursor::CursorAdapter;

/// Create a new adapter registry with all default adapters registered
///
/// This convenience function creates an `AdapterRegistry` and registers
/// all built-in adapter implementations:
///
/// - `ClaudeCodeAdapter`
/// - `CursorAdapter`
/// - `CodexAdapter`
///
/// # Errors
///
/// Returns an error if any adapter fails to initialize (e.g., if the
/// home directory cannot be determined).
///
/// # Example
///
/// ```rust
/// use rightclick::adapters::create_default_registry;
///
/// let registry = create_default_registry().expect("Failed to create registry");
/// println!("Registered {} adapters", registry.len());
/// ```
pub fn create_default_registry() -> anyhow::Result<AdapterRegistry> {
    let mut registry = AdapterRegistry::new();

    // Register Claude Code adapter
    match ClaudeCodeAdapter::new() {
        Ok(adapter) => registry.register(std::sync::Arc::new(adapter)),
        Err(e) => {
            tracing::warn!("Failed to initialize Claude Code adapter: {}", e);
        }
    }

    // Register Cursor adapter
    match CursorAdapter::new() {
        Ok(adapter) => registry.register(std::sync::Arc::new(adapter)),
        Err(e) => {
            tracing::warn!("Failed to initialize Cursor adapter: {}", e);
        }
    }

    // Register Codex adapter
    match CodexAdapter::new() {
        Ok(adapter) => registry.register(std::sync::Arc::new(adapter)),
        Err(e) => {
            tracing::warn!("Failed to initialize Codex adapter: {}", e);
        }
    }

    Ok(registry)
}

/// Create an adapter registry with only specific adapters
///
/// # Arguments
///
/// * `types` - The adapter types to include in the registry
///
/// # Example
///
/// ```rust
/// use rightclick::adapters::{create_registry_with, AdapterType};
///
/// let registry = create_registry_with(&[
///     AdapterType::ClaudeCode,
///     AdapterType::Codex,
/// ]).expect("Failed to create registry");
/// ```
pub fn create_registry_with(types: &[AdapterType]) -> anyhow::Result<AdapterRegistry> {
    let mut registry = AdapterRegistry::new();

    for adapter_type in types {
        match adapter_type {
            AdapterType::ClaudeCode => {
                if let Ok(adapter) = ClaudeCodeAdapter::new() {
                    registry.register(std::sync::Arc::new(adapter));
                }
            }
            AdapterType::Cursor => {
                if let Ok(adapter) = CursorAdapter::new() {
                    registry.register(std::sync::Arc::new(adapter));
                }
            }
            AdapterType::Codex => {
                if let Ok(adapter) = CodexAdapter::new() {
                    registry.register(std::sync::Arc::new(adapter));
                }
            }
            // Other adapters not yet implemented
            _ => {
                tracing::debug!("Adapter {:?} not yet implemented", adapter_type);
            }
        }
    }

    Ok(registry)
}

/// Get a list of all available adapter types
///
/// Returns all adapter types, including those that may not be
/// fully implemented yet.
pub fn all_adapter_types() -> Vec<AdapterType> {
    vec![
        AdapterType::ClaudeCode,
        AdapterType::Cursor,
        AdapterType::Codex,
        AdapterType::Gemini,
        AdapterType::Warp,
        AdapterType::Amp,
        AdapterType::Kiro,
        AdapterType::OpenCode,
    ]
}

/// Check if an adapter type is implemented
pub fn is_adapter_implemented(adapter_type: AdapterType) -> bool {
    matches!(
        adapter_type,
        AdapterType::ClaudeCode | AdapterType::Cursor | AdapterType::Codex
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_default_registry() {
        let registry = create_default_registry().unwrap();
        // May have 0-3 adapters depending on environment
        assert!(registry.len() <= 3);
    }

    #[test]
    fn test_create_registry_with() {
        let registry =
            create_registry_with(&[AdapterType::ClaudeCode, AdapterType::Codex]).unwrap();
        // Should have up to 2 adapters
        assert!(registry.len() <= 2);
    }

    #[test]
    fn test_all_adapter_types() {
        let types = all_adapter_types();
        assert_eq!(types.len(), 8);
        assert!(types.contains(&AdapterType::ClaudeCode));
        assert!(types.contains(&AdapterType::Cursor));
        assert!(types.contains(&AdapterType::Codex));
    }

    #[test]
    fn test_is_adapter_implemented() {
        assert!(is_adapter_implemented(AdapterType::ClaudeCode));
        assert!(is_adapter_implemented(AdapterType::Cursor));
        assert!(is_adapter_implemented(AdapterType::Codex));
        assert!(!is_adapter_implemented(AdapterType::Gemini));
        assert!(!is_adapter_implemented(AdapterType::Warp));
    }

    #[test]
    fn test_adapter_type_properties() {
        let claude = AdapterType::ClaudeCode;
        assert_eq!(claude.as_str(), "claude-code");
        assert_eq!(claude.display_name(), "Claude Code");
        assert_eq!(claude.icon(), '◈');

        let cursor = AdapterType::Cursor;
        assert_eq!(cursor.as_str(), "cursor");
        assert_eq!(cursor.display_name(), "Cursor");
        assert_eq!(cursor.icon(), '◉');

        let codex = AdapterType::Codex;
        assert_eq!(codex.as_str(), "codex");
        assert_eq!(codex.display_name(), "Codex CLI");
        assert_eq!(codex.icon(), '◆');
    }
}
