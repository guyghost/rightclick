//! Git-specific state machine integration
//!
//! This module provides `GitStateMachine`, a wrapper around `StateMachineExecutor`
//! that is specifically tailored for the Git plugin. It maps key events to
//! navigation and actions, and produces `GitCommand`s that the plugin can handle.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────┐
//! │                      GitStateMachine                        │
//! │                    (Git Plugin Adapter)                     │
//! │                                                            │
//! │  handle_key() ──► StateMachineExecutor ──► GitCommand      │
//! │                         │                                  │
//! │                         ▼                                  │
//! │                    Core Logic                              │
//! │              (calculate_navigation,                        │
//! │               check_guard)                                 │
//! └────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Key Mappings
//!
//! | Key         | Action                              |
//! |-------------|-------------------------------------|
//! | j, Down     | Navigate down in list               |
//! | k, Up       | Navigate up in list                 |
//! | h, Left     | Focus sidebar                       |
//! | l, Right    | Focus main pane                     |
//! | g, Home     | Jump to first item                  |
//! | G, End      | Jump to last item                   |
//! | s           | Stage file (if guard passes)        |
//! | u           | Unstage file (if guard passes)      |
//! | H           | Switch to History mode              |
//! | S           | Switch to Status mode               |

use std::path::PathBuf;

use crate::core::models::action::ActionId;
use crate::core::models::navigation::NavDirection;
use crate::core::models::state_machine::{FocusPane, ViewMode};

use super::StateMachineExecutor;

/// Git-specific commands produced by the state machine
///
/// These commands represent the actions that the Git plugin should take
/// in response to keyboard input.
#[derive(Clone, Debug, PartialEq)]
pub enum GitCommand {
    /// Select item at index
    SelectIndex(usize),
    /// Set focus pane
    SetFocus(FocusPane),
    /// Execute an action
    ExecuteAction(ActionId),
    /// Switch view mode
    SwitchMode(ViewMode),
    /// Load commits for history view
    LoadCommits,
    /// Refresh view
    Refresh,
}

/// Git plugin state machine wrapper
///
/// This wraps `StateMachineExecutor` and provides Git-specific key handling.
/// It translates key events into `GitCommand`s that the plugin can handle.
///
/// # Example
///
/// ```ignore
/// use shell::machines::GitStateMachine;
/// use std::path::PathBuf;
///
/// let state_machine = GitStateMachine::new(PathBuf::from("."));
/// state_machine.initialize(10, ViewMode::Status);
///
/// // Handle key press
/// let commands = state_machine.handle_key("j");
/// for cmd in commands {
///     match cmd {
///         GitCommand::SelectIndex(idx) => { /* select file at idx */ }
///         _ => {}
///     }
/// }
/// ```
pub struct GitStateMachine {
    /// Underlying executor
    executor: StateMachineExecutor,
    /// Repository path
    repo_path: PathBuf,
    /// Current view mode for context sync (reserved for future use)
    #[allow(dead_code)]
    current_mode: ViewMode,
}

impl std::fmt::Debug for GitStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitStateMachine")
            .field("executor", &self.executor)
            .field("repo_path", &self.repo_path)
            .field("current_mode", &self.current_mode)
            .finish()
    }
}

impl GitStateMachine {
    /// Create a new git state machine
    ///
    /// # Arguments
    /// * `repo_path` - Path to the git repository
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            executor: StateMachineExecutor::new(),
            repo_path,
            current_mode: ViewMode::Status,
        }
    }

    /// Get the repository path
    pub fn repo_path(&self) -> &PathBuf {
        &self.repo_path
    }

    /// Initialize with data
    ///
    /// Sets up the initial item count and view mode.
    /// This should be called after the plugin loads its initial data.
    ///
    /// # Arguments
    /// * `item_count` - Number of items in the current view
    /// * `view_mode` - Current view mode (Status, History, etc.)
    pub fn initialize(&self, item_count: usize, view_mode: ViewMode) {
        self.executor.set_item_count(item_count);
        self.executor.update_context(|ctx| {
            ctx.view_mode = view_mode;
            ctx.focus_pane = FocusPane::Sidebar;
        });
    }

    /// Handle key event - THIS FIXES THE GIT NAVIGATION ISSUE
    ///
    /// Maps key presses to navigation and actions using the state machine.
    /// Returns a list of commands that the plugin should execute.
    ///
    /// # Key Mappings
    /// - `j`, `Down` → Navigate down in list
    /// - `k`, `Up` → Navigate up in list
    /// - `h`, `Left` → Focus sidebar
    /// - `l`, `Right` → Focus main pane
    /// - `g`, `Home` → Jump to first item
    /// - `G`, `End` → Jump to last item
    /// - `s` → Stage file (if guard passes)
    /// - `u` → Unstage file (if guard passes)
    /// - `H` → Switch to History mode
    /// - `S` → Switch to Status mode
    ///
    /// # Arguments
    /// * `key` - The key that was pressed (e.g., "j", "k", "s")
    ///
    /// # Returns
    /// A vector of commands for the plugin to execute
    pub fn handle_key(&self, key: &str) -> Vec<GitCommand> {
        let mut commands = Vec::new();

        match key {
            "j" | "Down" => {
                let result = self.executor.handle_navigation(NavDirection::Down);
                if let crate::core::models::navigation::NavigationResult::Navigate {
                    index: Some(idx),
                    ..
                } = result
                {
                    commands.push(GitCommand::SelectIndex(idx));
                }
            }
            "k" | "Up" => {
                let result = self.executor.handle_navigation(NavDirection::Up);
                if let crate::core::models::navigation::NavigationResult::Navigate {
                    index: Some(idx),
                    ..
                } = result
                {
                    commands.push(GitCommand::SelectIndex(idx));
                }
            }
            "h" | "Left" => {
                let result = self.executor.handle_navigation(NavDirection::Left);
                if let crate::core::models::navigation::NavigationResult::Navigate {
                    region: crate::core::models::navigation::NavRegion::Sidebar,
                    ..
                } = result
                {
                    self.executor.update_context(|ctx| {
                        ctx.focus_pane = FocusPane::Sidebar;
                    });
                    commands.push(GitCommand::SetFocus(FocusPane::Sidebar));
                }
            }
            "l" | "Right" => {
                let result = self.executor.handle_navigation(NavDirection::Right);
                if let crate::core::models::navigation::NavigationResult::Navigate {
                    region: crate::core::models::navigation::NavRegion::Main,
                    ..
                } = result
                {
                    self.executor.update_context(|ctx| {
                        ctx.focus_pane = FocusPane::Main;
                    });
                    commands.push(GitCommand::SetFocus(FocusPane::Main));
                }
            }
            "g" | "Home" => {
                let result = self.executor.handle_navigation(NavDirection::First);
                if let crate::core::models::navigation::NavigationResult::Navigate {
                    index: Some(idx),
                    ..
                } = result
                {
                    commands.push(GitCommand::SelectIndex(idx));
                }
            }
            "G" | "End" => {
                let result = self.executor.handle_navigation(NavDirection::Last);
                if let crate::core::models::navigation::NavigationResult::Navigate {
                    index: Some(idx),
                    ..
                } = result
                {
                    commands.push(GitCommand::SelectIndex(idx));
                }
            }
            "s" => {
                // Check guard before executing
                if self.executor.can_execute(ActionId::Stage) {
                    commands.push(GitCommand::ExecuteAction(ActionId::Stage));
                }
            }
            "u" => {
                if self.executor.can_execute(ActionId::Unstage) {
                    commands.push(GitCommand::ExecuteAction(ActionId::Unstage));
                }
            }
            "d" => {
                if self.executor.can_execute(ActionId::Diff) {
                    commands.push(GitCommand::ExecuteAction(ActionId::Diff));
                }
            }
            "H" => {
                self.executor.update_context(|ctx| {
                    ctx.view_mode = ViewMode::History;
                    ctx.selected_index = if ctx.item_count > 0 { Some(0) } else { None };
                });
                commands.push(GitCommand::SwitchMode(ViewMode::History));
                commands.push(GitCommand::LoadCommits);
            }
            "S" => {
                self.executor.update_context(|ctx| {
                    ctx.view_mode = ViewMode::Status;
                });
                commands.push(GitCommand::SwitchMode(ViewMode::Status));
            }
            "r" | "R" => {
                commands.push(GitCommand::Refresh);
            }
            _ => {}
        }

        commands
    }

    /// Update after data changes
    ///
    /// Call this when the number of items changes (e.g., after refresh).
    ///
    /// # Arguments
    /// * `count` - New number of items
    pub fn update_items(&self, count: usize) {
        self.executor.set_item_count(count);
    }

    /// Get current selection
    ///
    /// Returns the index of the currently selected item, or None.
    pub fn selected_index(&self) -> Option<usize> {
        self.executor.context().selected_index
    }

    /// Set selected index directly
    ///
    /// Use this when selection is set programmatically.
    ///
    /// # Arguments
    /// * `index` - Index to select, or None to clear
    pub fn set_selected_index(&self, index: Option<usize>) {
        self.executor.set_selected_index(index);
    }

    /// Set focus pane
    ///
    /// # Arguments
    /// * `pane` - The pane to focus
    pub fn set_focus_pane(&self, pane: FocusPane) {
        self.executor.update_context(|ctx| {
            ctx.focus_pane = pane;
        });
    }

    /// Get current focus pane
    pub fn focus_pane(&self) -> FocusPane {
        self.executor.context().focus_pane
    }

    /// Get current view mode
    pub fn view_mode(&self) -> ViewMode {
        self.executor.context().view_mode
    }

    /// Set view mode
    ///
    /// # Arguments
    /// * `mode` - The view mode to set
    pub fn set_view_mode(&self, mode: ViewMode) {
        self.executor.update_context(|ctx| {
            ctx.view_mode = mode;
        });
    }

    /// Check if action is available
    ///
    /// Returns true if the action passes guard checks.
    ///
    /// # Arguments
    /// * `action` - The action to check
    pub fn is_action_available(&self, action: ActionId) -> bool {
        self.executor.can_execute(action)
    }

    /// Get current state
    pub fn current_state(&self) -> crate::core::models::state_machine::ViewState {
        self.executor.current_state()
    }

    /// Execute an action
    ///
    /// Executes the action if it passes guard checks.
    ///
    /// # Arguments
    /// * `action` - The action to execute
    ///
    /// # Returns
    /// The action result
    pub fn execute_action(&self, action: ActionId) -> crate::core::models::action::ActionResult {
        self.executor.execute_action(action)
    }

    /// Get item count
    pub fn item_count(&self) -> usize {
        self.executor.context().item_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_state_machine_new() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        assert_eq!(sm.repo_path(), &PathBuf::from("."));
    }

    #[test]
    fn test_git_state_machine_initialize() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);

        assert_eq!(sm.item_count(), 10);
        assert_eq!(sm.view_mode(), ViewMode::Status);
        assert_eq!(sm.focus_pane(), FocusPane::Sidebar);
    }

    #[test]
    fn test_git_handle_key_j_navigates_down() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);

        // No selection initially - 'j' should select first item
        let commands = sm.handle_key("j");

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], GitCommand::SelectIndex(0));
        assert_eq!(sm.selected_index(), Some(0));
    }

    #[test]
    fn test_git_handle_key_down_arrow_navigates_down() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);

        let commands = sm.handle_key("Down");
        assert_eq!(commands[0], GitCommand::SelectIndex(0));
    }

    #[test]
    fn test_git_handle_key_k_navigates_up() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);
        sm.set_selected_index(Some(5));

        let commands = sm.handle_key("k");

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], GitCommand::SelectIndex(4));
    }

    #[test]
    fn test_git_handle_key_l_focuses_main() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);

        let commands = sm.handle_key("l");

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], GitCommand::SetFocus(FocusPane::Main));
        assert_eq!(sm.focus_pane(), FocusPane::Main);
    }

    #[test]
    fn test_git_handle_key_h_focuses_sidebar() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);
        sm.set_focus_pane(FocusPane::Main);

        let commands = sm.handle_key("h");

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], GitCommand::SetFocus(FocusPane::Sidebar));
        assert_eq!(sm.focus_pane(), FocusPane::Sidebar);
    }

    #[test]
    fn test_git_handle_key_g_jumps_to_first() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);
        sm.set_selected_index(Some(5));

        let commands = sm.handle_key("g");

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], GitCommand::SelectIndex(0));
    }

    #[test]
    fn test_git_handle_key_G_jumps_to_last() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);
        sm.set_selected_index(Some(2));

        let commands = sm.handle_key("G");

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], GitCommand::SelectIndex(9));
    }

    #[test]
    fn test_git_handle_key_s_stage_with_selection() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);
        sm.set_selected_index(Some(0));

        let commands = sm.handle_key("s");

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], GitCommand::ExecuteAction(ActionId::Stage));
    }

    #[test]
    fn test_git_handle_key_s_stage_without_selection() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);
        // No selection

        let commands = sm.handle_key("s");

        // Should not produce any commands - guard blocks it
        assert!(commands.is_empty());
    }

    #[test]
    fn test_git_handle_key_u_unstage_with_selection() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);
        sm.set_selected_index(Some(0));

        let commands = sm.handle_key("u");

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], GitCommand::ExecuteAction(ActionId::Unstage));
    }

    #[test]
    fn test_git_handle_key_u_unstage_without_selection() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);
        // No selection

        let commands = sm.handle_key("u");

        assert!(commands.is_empty());
    }

    #[test]
    fn test_git_handle_key_H_switches_to_history() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);

        let commands = sm.handle_key("H");

        assert!(commands.contains(&GitCommand::SwitchMode(ViewMode::History)));
        assert!(commands.contains(&GitCommand::LoadCommits));
        assert_eq!(sm.view_mode(), ViewMode::History);
    }

    #[test]
    fn test_git_handle_key_S_switches_to_status() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::History);

        let commands = sm.handle_key("S");

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], GitCommand::SwitchMode(ViewMode::Status));
        assert_eq!(sm.view_mode(), ViewMode::Status);
    }

    #[test]
    fn test_git_handle_key_r_refreshes() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);

        let commands = sm.handle_key("r");

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], GitCommand::Refresh);
    }

    #[test]
    fn test_git_update_items() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);

        sm.update_items(20);
        assert_eq!(sm.item_count(), 20);
    }

    #[test]
    fn test_git_set_focus_pane() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);

        sm.set_focus_pane(FocusPane::Main);
        assert_eq!(sm.focus_pane(), FocusPane::Main);
    }

    #[test]
    fn test_git_is_action_available() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);

        // Without selection, Stage should not be available
        assert!(!sm.is_action_available(ActionId::Stage));

        // With selection, Stage should be available
        sm.set_selected_index(Some(0));
        assert!(sm.is_action_available(ActionId::Stage));
    }

    #[test]
    fn test_git_history_mode_navigation() {
        // This test validates the fix for the git navigation bug
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::History);

        // In history mode, j should navigate commits
        let commands = sm.handle_key("j");
        assert_eq!(commands[0], GitCommand::SelectIndex(0));

        // Next j should move to next commit
        let commands = sm.handle_key("j");
        assert_eq!(commands[0], GitCommand::SelectIndex(1));

        // k should move back
        let commands = sm.handle_key("k");
        assert_eq!(commands[0], GitCommand::SelectIndex(0));
    }

    #[test]
    fn test_git_stage_blocked_in_history_mode() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::History);
        sm.set_selected_index(Some(0));

        // In History mode, Stage should be blocked by guard
        assert!(!sm.is_action_available(ActionId::Stage));

        let commands = sm.handle_key("s");
        assert!(commands.is_empty());
    }

    #[test]
    fn test_git_unknown_key() {
        let sm = GitStateMachine::new(PathBuf::from("."));
        sm.initialize(10, ViewMode::Status);

        let commands = sm.handle_key("x");
        assert!(commands.is_empty());
    }
}
