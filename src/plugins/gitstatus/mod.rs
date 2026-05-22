//! Git Status Plugin for RightClick
//!
//! This plugin provides a TUI interface for viewing and interacting with
//! git repository status, including:
//!
//! - Viewing staged, unstaged, and untracked files
//! - Viewing diffs for selected files
//! - Viewing commit history
//! - Staging and unstaging files
//! - Keyboard navigation (vim-style: j/k, h/l)
//!
//! # Usage
//!
//! ```rust
//! use rightclick::plugins::gitstatus::{GitStatusPlugin, GitPluginContext};
//! use rightclick::core::models::Config;
//! use std::path::PathBuf;
//!
//! let mut plugin = GitStatusPlugin::new();
//! let ctx = GitPluginContext {
//!     work_dir: PathBuf::from("."),
//!     project_root: PathBuf::from("."),
//!     config: Config::default(),
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

// Re-export public types
pub use plugin::{
    Command, GitPluginContext, GitStatusPlugin, PluginCommand, create_plugin,
    register_default_bindings,
};
pub use render::{render_git_status, render_status_info};
pub use state::{FocusPane, GitModal, PluginState, ViewMode};

/// Plugin metadata
pub const PLUGIN_ID: &str = "git-status";
pub const PLUGIN_NAME: &str = "git";
pub const PLUGIN_ICON: char = 'G';
pub const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Create a new plugin instance with default settings
pub fn create() -> GitStatusPlugin {
    GitStatusPlugin::new()
}

/// Create a new plugin instance with the given configuration
pub fn create_with_config(config: crate::core::models::GitStatusPluginConfig) -> GitStatusPlugin {
    GitStatusPlugin::with_config(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::Plugin;

    #[test]
    fn test_plugin_constants() {
        assert_eq!(PLUGIN_ID, "git-status");
        assert_eq!(PLUGIN_NAME, "git");
        assert_eq!(PLUGIN_ICON, 'G');
    }

    #[test]
    fn test_create_plugin() {
        let plugin = create();
        assert_eq!(plugin.id(), PLUGIN_ID);
        assert_eq!(plugin.name(), PLUGIN_NAME);
    }

    #[test]
    fn test_create_plugin_with_config() {
        let config = crate::core::models::GitStatusPluginConfig::default();
        let plugin = create_with_config(config);
        assert_eq!(plugin.id(), PLUGIN_ID);
    }

    #[test]
    fn test_create_alias() {
        let plugin = create();
        assert_eq!(plugin.id(), "git-status");
        assert_eq!(plugin.name(), "git");
    }

    #[test]
    fn test_view_mode_variants() {
        assert_eq!(ViewMode::default(), ViewMode::Status);
        assert_ne!(ViewMode::Status, ViewMode::Diff);
        assert_ne!(ViewMode::Diff, ViewMode::History);
    }

    #[test]
    fn test_focus_pane_variants() {
        assert_eq!(FocusPane::default(), FocusPane::Sidebar);
        assert_ne!(FocusPane::Sidebar, FocusPane::Main);
    }

    #[test]
    fn test_command_variants() {
        use Command::*;

        // Test that all command variants can be created
        let none: Command = Default::default();
        assert_eq!(none, None);

        let refresh = Refresh;
        assert_eq!(refresh, Refresh);

        let switch_mode = SwitchMode(ViewMode::Diff);
        assert!(matches!(switch_mode, SwitchMode(ViewMode::Diff)));

        let switch_focus = SwitchFocus(FocusPane::Main);
        assert!(matches!(switch_focus, SwitchFocus(FocusPane::Main)));

        let stage = StageFile(std::path::PathBuf::from("test.rs"));
        assert!(matches!(stage, StageFile(_)));

        let unstage = UnstageFile(std::path::PathBuf::from("test.rs"));
        assert!(matches!(unstage, UnstageFile(_)));

        let show_diff = ShowDiff(std::path::PathBuf::from("test.rs"));
        assert!(matches!(show_diff, ShowDiff(_)));

        let commit = OpenCommitDialog;
        assert_eq!(commit, OpenCommitDialog);

        let git_exec = GitExec(vec!["status".to_string()]);
        assert!(matches!(git_exec, GitExec(_)));
    }
}
