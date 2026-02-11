//! TD Monitor Plugin for RightClick
//!
//! This plugin provides a TUI interface for viewing and interacting with
//! TD (Task Driver) tasks, including:
//!
//! - Viewing task list with status indicators
//! - Board/Kanban view for task management
//! - Activity log for tracking task changes
//! - Filtering tasks with '/'
//! - Reviewing tasks with 'r'
//! - Creating new tasks with 'n'
//! - Updating task status
//!
//! # TD Integration
//!
//! The plugin integrates with TD by:
//! - Checking if the 'td' binary is available
//! - Reading from the TD SQLite database at `.td/db.sqlite3`
//! - Executing TD CLI commands for task operations
//!
//! # Usage
//!
//! ```rust
//! use rightclick::plugins::tdmonitor::{TDMonitorPlugin, TDMonitorPluginContext};
//!
//! let mut plugin = TDMonitorPlugin::new();
//! let ctx = TDMonitorPluginContext {
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

// Re-export public types
pub use plugin::{Command, PluginCommand, TDMonitorPluginContext, TDMonitorPlugin};
pub use render::{
    render_activity_log, render_focused_task_header, render_not_available, render_status_info,
    render_td_monitor,
};
pub use state::{ActivityLogEntry, PluginState, Priority, Task, TaskStatus, ViewMode};

/// Plugin metadata
pub const PLUGIN_ID: &str = "td-monitor";
pub const PLUGIN_NAME: &str = "td";
pub const PLUGIN_ICON: char = 'T';
pub const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Create a new plugin instance with default settings
pub fn create_plugin() -> TDMonitorPlugin {
    TDMonitorPlugin::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_constants() {
        assert_eq!(PLUGIN_ID, "td-monitor");
        assert_eq!(PLUGIN_NAME, "td");
        assert_eq!(PLUGIN_ICON, 'T');
    }

    #[test]
    fn test_create_plugin() {
        let plugin = create_plugin();
        assert_eq!(plugin.id(), PLUGIN_ID);
        assert_eq!(plugin.name(), PLUGIN_NAME);
    }

    #[test]
    fn test_task_status_parsing() {
        assert_eq!(TaskStatus::from_db("todo"), TaskStatus::Todo);
        assert_eq!(TaskStatus::from_db("in-progress"), TaskStatus::InProgress);
        assert_eq!(TaskStatus::from_db("review"), TaskStatus::Review);
        assert_eq!(TaskStatus::from_db("done"), TaskStatus::Done);
    }

    #[test]
    fn test_priority_parsing() {
        assert_eq!(Priority::from_db("low"), Priority::Low);
        assert_eq!(Priority::from_db("medium"), Priority::Medium);
        assert_eq!(Priority::from_db("high"), Priority::High);
        assert_eq!(Priority::from_db("critical"), Priority::Critical);
    }

    #[test]
    fn test_task_creation() {
        let task = Task::new("task-1", "Test Task");
        assert_eq!(task.id, "task-1");
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.status, TaskStatus::Todo);
        assert_eq!(task.priority, Priority::Medium);
    }

    #[test]
    fn test_task_filtering() {
        let mut task = Task::new("1", "Important Bug Fix");
        task.description = Some("Fix the critical issue".to_string());

        assert!(task.matches_filter("bug"));
        assert!(task.matches_filter("critical"));
        assert!(task.matches_filter("Important"));
        assert!(!task.matches_filter("feature"));
    }

    #[test]
    fn test_view_mode_toggle() {
        let mut mode = ViewMode::List;
        assert!(matches!(mode, ViewMode::List));

        mode = mode.toggle();
        assert!(matches!(mode, ViewMode::Board));

        mode = mode.toggle();
        assert!(matches!(mode, ViewMode::List));
    }

    #[test]
    fn test_plugin_state_selection() {
        let mut state = PluginState::new();
        state.tasks = vec![
            Task::new("1", "Task 1"),
            Task::new("2", "Task 2"),
            Task::new("3", "Task 3"),
        ];

        assert_eq!(state.selected_task, None);

        state.select_next();
        assert_eq!(state.selected_task, Some(0));

        state.select_next();
        assert_eq!(state.selected_task, Some(1));

        state.select_prev();
        assert_eq!(state.selected_task, Some(0));

        state.select_last();
        assert_eq!(state.selected_task, Some(2));

        state.select_first();
        assert_eq!(state.selected_task, Some(0));
    }

    #[test]
    fn test_plugin_state_filter() {
        let mut state = PluginState::new();
        state.tasks = vec![
            Task::new("1", "Alpha Task"),
            Task::new("2", "Beta Task"),
            Task::new("3", "Gamma Task"),
        ];

        // Test no filter
        assert_eq!(state.filtered_tasks().len(), 3);

        // Test with filter
        state.set_filter(Some("alpha".to_string()));
        let filtered = state.filtered_tasks();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "1");

        // Test clear filter
        state.clear_filter();
        assert_eq!(state.filter, None);
        assert_eq!(state.filtered_tasks().len(), 3);
    }

    #[test]
    fn test_plugin_command() {
        let cmd = PluginCommand::new("test", "Test Command", 't');
        assert_eq!(cmd.id, "test");
        assert_eq!(cmd.name, "Test Command");
        assert_eq!(cmd.key, 't');
    }

    #[test]
    fn test_command_variants() {
        let cmd1 = Command::None;
        let cmd2 = Command::Refresh;
        let cmd3 = Command::SwitchMode(ViewMode::List);
        let cmd4 = Command::CreateTask;
        let cmd5 = Command::UpdateStatus("task-1".to_string(), TaskStatus::Done);

        assert_ne!(std::mem::discriminant(&cmd1), std::mem::discriminant(&cmd2));
        assert_ne!(std::mem::discriminant(&cmd2), std::mem::discriminant(&cmd3));
        assert_ne!(std::mem::discriminant(&cmd3), std::mem::discriminant(&cmd4));
        assert_ne!(std::mem::discriminant(&cmd4), std::mem::discriminant(&cmd5));
    }
}
