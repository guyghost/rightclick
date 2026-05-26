//! Workers Plugin for RightClick
//!
//! This plugin provides a TUI interface for managing AI agent workflows (Intents)
//! and the workers that execute them. It implements an Intent-based development
//! workflow inspired by Augment Code's Intent feature.
//!
//! # Features
//!
//! - **Intents**: Living specifications that evolve as workers complete tasks
//! - **Workers**: Specialized AI agents (Investigator, Implementer, Verifier, etc.)
//! - **Coordinator**: Orchestrates worker execution with dependency management
//! - **Live Output**: Real-time streaming of worker output
//!
//! # Usage
//!
//! ```rust
//! use rightclick::plugins::workers::{WorkersPlugin, WorkersPluginContext};
//!
//! let mut plugin = WorkersPlugin::new();
//! let ctx = WorkersPluginContext {
//!     project_root: std::path::PathBuf::from("."),
//!     config: rightclick::core::models::Config::default(),
//! };
//!
//! // Initialize the plugin
//! # tokio_test::block_on(async {
//! plugin.init_with_context(&ctx).await.unwrap();
//! # });
//! ```

mod plugin;
mod render;
mod runner;
mod state;

// Re-export public types
pub use plugin::{Command, PluginCommand, WorkersPlugin, WorkersPluginContext};
pub use render::{render_kanban, render_workers, render_workers_status};
pub use runner::{WorkerRunner, WorkerRunnerError};
pub use state::{
    FocusPane, IntentEntry, KanbanState, ModalState, PluginState, PreviewTab, ViewMode, WorkerEntry,
};

/// Plugin metadata
pub const PLUGIN_ID: &str = "workers";
pub const PLUGIN_NAME: &str = "Workers";
pub const PLUGIN_ICON: char = '🤖';
pub const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default intents directory relative to project root
pub const DEFAULT_INTENTS_DIR: &str = ".rightclick/intents";

/// Default logs directory relative to project root
pub const DEFAULT_LOGS_DIR: &str = ".rightclick/logs";

/// Create a new plugin instance with default settings
pub fn create_plugin() -> WorkersPlugin {
    WorkersPlugin::new()
}

/// Create a new plugin instance with the given configuration
pub fn create_plugin_with_config(
    config: crate::core::models::WorkersPluginConfig,
) -> WorkersPlugin {
    WorkersPlugin::with_config(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_constants() {
        assert_eq!(PLUGIN_ID, "workers");
        assert_eq!(PLUGIN_NAME, "Workers");
        assert_eq!(PLUGIN_ICON, '🤖');
    }

    #[test]
    fn test_create_plugin() {
        let plugin = create_plugin();
        assert_eq!(plugin.id(), PLUGIN_ID);
        assert_eq!(plugin.name(), PLUGIN_NAME);
    }
}
