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

pub mod amp;
pub mod claudecode;
pub mod codex;
pub mod cursor;
pub mod gemini;
pub mod kiro;
pub mod opencode;
pub mod types;
pub mod warp;

// Re-export main types
pub use types::{
    Adapter, AdapterError, AdapterPolicy, AdapterRegistry, AdapterType, DEFAULT_ADAPTER_ORDER,
    EXPERIMENTAL_ADAPTERS, Result,
};

// Re-export adapter implementations
pub use amp::AmpAdapter;
pub use claudecode::ClaudeCodeAdapter;
pub use codex::CodexAdapter;
pub use cursor::CursorAdapter;
pub use gemini::GeminiAdapter;
pub use kiro::KiroAdapter;
pub use opencode::OpenCodeAdapter;
pub use warp::WarpAdapter;

/// Create a new adapter registry with all default adapters registered
///
/// This convenience function creates an `AdapterRegistry` and registers
/// production-ready built-in adapter implementations:
///
/// - `ClaudeCodeAdapter`
/// - `CursorAdapter`
/// - `CodexAdapter`
/// - `OpenCodeAdapter`
///
/// Adapters with unknown or unavailable local storage formats are kept in the
/// codebase for development, but are not registered by default.
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
    create_registry_with_policy(&AdapterPolicy::production_default())
}

/// Create a registry from an explicit adapter policy.
pub fn create_registry_with_policy(policy: &AdapterPolicy) -> anyhow::Result<AdapterRegistry> {
    let mut registry = AdapterRegistry::new();

    for adapter_type in policy.enabled() {
        register_adapter_type(&mut registry, *adapter_type);
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
    create_registry_with_policy(&AdapterPolicy::production_from(types))
}

fn register_adapter_type(registry: &mut AdapterRegistry, adapter_type: AdapterType) {
    match adapter_type {
        AdapterType::ClaudeCode => match ClaudeCodeAdapter::new() {
            Ok(adapter) => registry.register(std::sync::Arc::new(adapter)),
            Err(e) => tracing::warn!("Failed to initialize Claude Code adapter: {}", e),
        },
        AdapterType::Cursor => match CursorAdapter::new() {
            Ok(adapter) => registry.register(std::sync::Arc::new(adapter)),
            Err(e) => tracing::warn!("Failed to initialize Cursor adapter: {}", e),
        },
        AdapterType::Codex => match CodexAdapter::new() {
            Ok(adapter) => registry.register(std::sync::Arc::new(adapter)),
            Err(e) => tracing::warn!("Failed to initialize Codex adapter: {}", e),
        },
        AdapterType::OpenCode => match OpenCodeAdapter::new() {
            Ok(adapter) => registry.register(std::sync::Arc::new(adapter)),
            Err(e) => tracing::warn!("Failed to initialize OpenCode adapter: {}", e),
        },
        AdapterType::Gemini | AdapterType::Kiro | AdapterType::Warp | AdapterType::Amp => {}
    }
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
        AdapterType::ClaudeCode | AdapterType::Cursor | AdapterType::Codex | AdapterType::OpenCode
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_default_registry() {
        let registry = create_default_registry().unwrap();
        // Only production-ready adapters are registered by default.
        assert_eq!(registry.len(), 4);
    }

    #[test]
    fn test_create_registry_with() {
        let registry =
            create_registry_with(&[AdapterType::ClaudeCode, AdapterType::Codex]).unwrap();
        // Should have up to 2 adapters
        assert!(registry.len() <= 2);
    }

    #[test]
    fn test_create_registry_with_new_adapters() {
        let registry = create_registry_with(&[
            AdapterType::OpenCode,
            AdapterType::Gemini,
            AdapterType::Kiro,
            AdapterType::Warp,
            AdapterType::Amp,
        ])
        .unwrap();
        // Experimental/stub adapters are not registered for production use.
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_all_adapter_types() {
        let types = all_adapter_types();
        assert_eq!(types.len(), 8);
        assert!(types.contains(&AdapterType::ClaudeCode));
        assert!(types.contains(&AdapterType::Cursor));
        assert!(types.contains(&AdapterType::Codex));
        assert!(types.contains(&AdapterType::Gemini));
        assert!(types.contains(&AdapterType::Warp));
        assert!(types.contains(&AdapterType::Amp));
        assert!(types.contains(&AdapterType::Kiro));
        assert!(types.contains(&AdapterType::OpenCode));
    }

    #[test]
    fn test_is_adapter_implemented() {
        assert!(is_adapter_implemented(AdapterType::ClaudeCode));
        assert!(is_adapter_implemented(AdapterType::Cursor));
        assert!(is_adapter_implemented(AdapterType::Codex));
        assert!(!is_adapter_implemented(AdapterType::Gemini));
        assert!(!is_adapter_implemented(AdapterType::Warp));
        assert!(!is_adapter_implemented(AdapterType::Amp));
        assert!(!is_adapter_implemented(AdapterType::Kiro));
        assert!(is_adapter_implemented(AdapterType::OpenCode));
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

        let gemini = AdapterType::Gemini;
        assert_eq!(gemini.as_str(), "gemini");
        assert_eq!(gemini.display_name(), "Gemini CLI");
        assert_eq!(gemini.icon(), '◊');

        let warp = AdapterType::Warp;
        assert_eq!(warp.as_str(), "warp");
        assert_eq!(warp.display_name(), "Warp");
        assert_eq!(warp.icon(), '◐');

        let amp = AdapterType::Amp;
        assert_eq!(amp.as_str(), "amp");
        assert_eq!(amp.display_name(), "Amp");
        assert_eq!(amp.icon(), '◎');

        let kiro = AdapterType::Kiro;
        assert_eq!(kiro.as_str(), "kiro");
        assert_eq!(kiro.display_name(), "Kiro");
        assert_eq!(kiro.icon(), '○');

        let opencode = AdapterType::OpenCode;
        assert_eq!(opencode.as_str(), "opencode");
        assert_eq!(opencode.display_name(), "OpenCode");
        assert_eq!(opencode.icon(), '◑');
    }
}
