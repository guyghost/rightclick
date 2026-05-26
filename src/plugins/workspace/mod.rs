//! Workspace Plugin for RightClick
//!
//! This plugin provides a TUI interface for managing git worktrees and associated
//! development sessions, including:
//!
//! - Listing all git worktrees in the repository
//! - Viewing worktree details (branch, path, linked tasks)
//! - Creating new worktrees with 'n'
//! - Deleting worktrees with 'D'
//! - Linking/unlinking TD tasks with 't'
//! - Launching AI agents with 'a'
//! - Entering interactive mode with 'enter'
//! - Merge workflow with 'm'
//!
//! # Usage
//!
//! ```rust
//! use rightclick::plugins::workspace::{WorkspacePlugin, WorkspacePluginContext};
//!
//! let mut plugin = WorkspacePlugin::new();
//! let ctx = WorkspacePluginContext {
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
mod state;
mod worktree;

// Re-export public types
pub use plugin::{Command, PluginCommand, WorkspacePlugin, WorkspacePluginContext};
pub use render::{render_workspace, render_workspace_status};
pub use state::{FocusPane, ModalState, PluginState, PreviewTab, ShellSession, ViewMode, Worktree};
pub use worktree::{AgentLauncher, TmuxManager, WorktreeManager};

/// Plugin metadata
pub const PLUGIN_ID: &str = "workspace";
pub const PLUGIN_NAME: &str = "Workspaces";
pub const PLUGIN_ICON: char = 'W';
pub const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Create a new plugin instance with default settings
pub fn create_plugin() -> WorkspacePlugin {
    WorkspacePlugin::new()
}

/// Create a new plugin instance with the given configuration
pub fn create_plugin_with_config(
    config: crate::core::models::WorkspacePluginConfig,
) -> WorkspacePlugin {
    WorkspacePlugin::with_config(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_constants() {
        assert_eq!(PLUGIN_ID, "workspace");
        assert_eq!(PLUGIN_NAME, "Workspaces");
        assert_eq!(PLUGIN_ICON, 'W');
    }

    #[test]
    fn test_create_plugin() {
        let plugin = create_plugin();
        assert_eq!(plugin.id(), PLUGIN_ID);
        assert_eq!(plugin.name(), PLUGIN_NAME);
    }
}
