//! Workspace Plugin State
//!
//! This module defines the state structure for the Workspace plugin,
//! including view modes, worktrees, shell sessions, and selection state.

use std::path::PathBuf;

/// A worktree entry representing a git worktree
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    /// Worktree name (directory name)
    pub name: String,
    /// Full path to the worktree
    pub path: PathBuf,
    /// Current branch name
    pub branch: String,
    /// Whether this is the main worktree
    pub is_main: bool,
    /// Linked TD task ID (if any)
    pub linked_task: Option<String>,
    /// Whether an AI agent is currently running
    pub agent_running: bool,
    /// Whether the worktree has uncommitted changes
    pub is_dirty: bool,
    /// Last commit message
    pub last_commit: Option<String>,
}

impl Worktree {
    /// Create a new worktree entry
    pub fn new(name: impl Into<String>, path: PathBuf, branch: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path,
            branch: branch.into(),
            is_main: false,
            linked_task: None,
            agent_running: false,
            is_dirty: false,
            last_commit: None,
        }
    }

    /// Mark this worktree as the main worktree
    pub fn with_main(mut self, is_main: bool) -> Self {
        self.is_main = is_main;
        self
    }

    /// Link a TD task to this worktree
    pub fn with_task(mut self, task_id: impl Into<String>) -> Self {
        self.linked_task = Some(task_id.into());
        self
    }

    /// Set agent running state
    pub fn with_agent_running(mut self, running: bool) -> Self {
        self.agent_running = running;
        self
    }

    /// Get status indicator characters
    pub fn status_icons(&self) -> String {
        let mut icons = String::new();
        if self.is_dirty {
            icons.push('*');
        }
        if self.agent_running {
            icons.push('🤖');
        }
        if self.linked_task.is_some() {
            icons.push('📋');
        }
        icons
    }
}

/// A shell session associated with a worktree
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellSession {
    /// Session ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Tmux session name
    pub tmux_session: String,
    /// Associated worktree name (if any)
    pub worktree_name: Option<String>,
    /// Whether the session is currently active
    pub is_active: bool,
}

impl ShellSession {
    /// Create a new shell session
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        tmux_session: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            tmux_session: tmux_session.into(),
            worktree_name: None,
            is_active: false,
        }
    }

    /// Associate with a worktree
    pub fn with_worktree(mut self, name: impl Into<String>) -> Self {
        self.worktree_name = Some(name.into());
        self
    }
}

/// View mode for the workspace display
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    /// List view showing all worktrees
    List,
    /// Kanban view organized by status
    Kanban,
    /// Interactive mode with terminal
    Interactive,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self::List
    }
}

/// Preview tab in the right pane
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewTab {
    /// Command output
    Output,
    /// Git diff
    Diff,
    /// Task details
    Task,
}

impl Default for PreviewTab {
    fn default() -> Self {
        Self::Output
    }
}

/// Which pane has keyboard focus
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusPane {
    /// Worktree list pane
    Sidebar,
    /// Preview pane
    Preview,
}

impl Default for FocusPane {
    fn default() -> Self {
        Self::Sidebar
    }
}

/// Plugin state containing all mutable data
#[derive(Clone, Debug, Default)]
pub struct PluginState {
    /// All git worktrees
    pub worktrees: Vec<Worktree>,
    /// Active shell sessions
    pub shells: Vec<ShellSession>,
    /// Currently selected worktree index
    pub selected: Option<usize>,
    /// Current view mode
    pub view_mode: ViewMode,
    /// Current preview tab
    pub preview_tab: PreviewTab,
    /// Sidebar width in columns
    pub sidebar_width: u16,
    /// Command output text
    pub output_text: String,
    /// Current diff content
    pub diff_content: Option<String>,
    /// Task details content
    pub task_content: Option<String>,
    /// Whether a command is running
    pub command_running: bool,
    /// New worktree name buffer (for creation dialog)
    pub new_worktree_name: String,
    /// New worktree branch buffer
    pub new_worktree_branch: String,
    /// Task ID buffer (for linking)
    pub task_id_buffer: String,
    /// Modal state
    pub modal_state: ModalState,
}

/// Modal dialog state
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ModalState {
    /// No modal open
    #[default]
    None,
    /// Create worktree dialog
    CreateWorktree,
    /// Delete confirmation dialog
    DeleteConfirm,
    /// Link task dialog
    LinkTask,
    /// Merge workflow dialog
    MergeDialog,
}

impl PluginState {
    /// Create a new empty state
    pub fn new() -> Self {
        Self {
            worktrees: Vec::new(),
            shells: Vec::new(),
            selected: None,
            view_mode: ViewMode::default(),
            preview_tab: PreviewTab::default(),
            sidebar_width: 40,
            output_text: String::new(),
            diff_content: None,
            task_content: None,
            command_running: false,
            new_worktree_name: String::new(),
            new_worktree_branch: String::new(),
            task_id_buffer: String::new(),
            modal_state: ModalState::None,
        }
    }

    /// Get the currently selected worktree if any
    pub fn selected_worktree(&self) -> Option<&Worktree> {
        self.selected.and_then(|idx| self.worktrees.get(idx))
    }

    /// Get a mutable reference to the selected worktree
    pub fn selected_worktree_mut(&mut self) -> Option<&mut Worktree> {
        self.selected.and_then(|idx| self.worktrees.get_mut(idx))
    }

    /// Get worktree by name
    pub fn get_worktree(&self, name: &str) -> Option<&Worktree> {
        self.worktrees.iter().find(|w| w.name == name)
    }

    /// Get mutable worktree by name
    pub fn get_worktree_mut(&mut self, name: &str) -> Option<&mut Worktree> {
        self.worktrees.iter_mut().find(|w| w.name == name)
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.worktrees.is_empty() {
            self.selected = None;
            return;
        }

        match self.selected {
            None => self.selected = Some(0),
            Some(idx) => {
                self.selected = Some((idx + 1).min(self.worktrees.len() - 1));
            }
        }
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        match self.selected {
            None => {
                if !self.worktrees.is_empty() {
                    self.selected = Some(self.worktrees.len() - 1);
                }
            }
            Some(0) => self.selected = None,
            Some(idx) => self.selected = Some(idx.saturating_sub(1)),
        }
    }

    /// Clear all state
    pub fn clear(&mut self) {
        self.worktrees.clear();
        self.shells.clear();
        self.selected = None;
        self.output_text.clear();
        self.diff_content = None;
        self.task_content = None;
        self.command_running = false;
        self.modal_state = ModalState::None;
    }

    /// Add output text
    pub fn add_output(&mut self, text: impl Into<String>) {
        let text = text.into();
        if !self.output_text.is_empty() {
            self.output_text.push('\n');
        }
        self.output_text.push_str(&text);
    }

    /// Clear output
    pub fn clear_output(&mut self) {
        self.output_text.clear();
    }

    /// Set diff content
    pub fn set_diff(&mut self, diff: impl Into<String>) {
        self.diff_content = Some(diff.into());
    }

    /// Set task content
    pub fn set_task_content(&mut self, content: impl Into<String>) {
        self.task_content = Some(content.into());
    }

    /// Get worktrees grouped by status for kanban view
    pub fn worktrees_by_status(&self) -> WorktreeGroups {
        let mut active = Vec::new();
        let mut waiting = Vec::new();
        let mut done = Vec::new();

        for (idx, worktree) in self.worktrees.iter().enumerate() {
            if worktree.is_dirty {
                active.push(idx);
            } else if worktree.linked_task.is_some() {
                waiting.push(idx);
            } else {
                done.push(idx);
            }
        }

        WorktreeGroups {
            active,
            waiting,
            done,
        }
    }

    /// Check if a modal is open
    pub fn is_modal_open(&self) -> bool {
        self.modal_state != ModalState::None
    }

    /// Close any open modal
    pub fn close_modal(&mut self) {
        self.modal_state = ModalState::None;
        self.new_worktree_name.clear();
        self.new_worktree_branch.clear();
        self.task_id_buffer.clear();
    }
}

/// Grouped worktree indices for kanban view
#[derive(Clone, Debug)]
pub struct WorktreeGroups {
    /// Active worktrees (with changes)
    pub active: Vec<usize>,
    /// Waiting worktrees (linked to tasks but clean)
    pub waiting: Vec<usize>,
    /// Done worktrees (clean, no linked task)
    pub done: Vec<usize>,
}

impl WorktreeGroups {
    /// Create empty groups
    pub fn new() -> Self {
        Self {
            active: Vec::new(),
            waiting: Vec::new(),
            done: Vec::new(),
        }
    }
}

impl Default for WorktreeGroups {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_mode_default() {
        assert_eq!(ViewMode::default(), ViewMode::List);
    }

    #[test]
    fn test_preview_tab_default() {
        assert_eq!(PreviewTab::default(), PreviewTab::Output);
    }

    #[test]
    fn test_focus_pane_default() {
        assert_eq!(FocusPane::default(), FocusPane::Sidebar);
    }

    #[test]
    fn test_worktree_new() {
        let worktree = Worktree::new("feature", PathBuf::from("/repo/feature"), "feature-branch");
        assert_eq!(worktree.name, "feature");
        assert_eq!(worktree.branch, "feature-branch");
        assert!(!worktree.is_main);
    }

    #[test]
    fn test_worktree_with_main() {
        let worktree = Worktree::new("main", PathBuf::from("/repo"), "main").with_main(true);
        assert!(worktree.is_main);
    }

    #[test]
    fn test_worktree_with_task() {
        let worktree = Worktree::new("feature", PathBuf::from("/repo/feature"), "feature")
            .with_task("TD-123");
        assert_eq!(worktree.linked_task, Some("TD-123".to_string()));
    }

    #[test]
    fn test_worktree_status_icons() {
        let worktree = Worktree::new("feature", PathBuf::from("/repo/feature"), "feature")
            .with_task("TD-123")
            .with_agent_running(true);
        let icons = worktree.status_icons();
        assert!(icons.contains('🤖'));
        assert!(icons.contains('📋'));
    }

    #[test]
    fn test_shell_session_new() {
        let session = ShellSession::new("1", "main", "repo-main");
        assert_eq!(session.id, "1");
        assert_eq!(session.name, "main");
        assert_eq!(session.tmux_session, "repo-main");
    }

    #[test]
    fn test_state_new() {
        let state = PluginState::new();
        assert!(state.worktrees.is_empty());
        assert!(state.shells.is_empty());
        assert_eq!(state.selected, None);
        assert_eq!(state.view_mode, ViewMode::List);
        assert_eq!(state.sidebar_width, 40);
    }

    #[test]
    fn test_selection() {
        let mut state = PluginState::new();
        state.worktrees = vec![
            Worktree::new("main", PathBuf::from("/repo"), "main"),
            Worktree::new("feature1", PathBuf::from("/repo/feature1"), "feature1"),
            Worktree::new("feature2", PathBuf::from("/repo/feature2"), "feature2"),
        ];

        // Initially no selection
        assert_eq!(state.selected, None);

        // Select next
        state.select_next();
        assert_eq!(state.selected, Some(0));

        // Select next
        state.select_next();
        assert_eq!(state.selected, Some(1));

        // Select prev
        state.select_prev();
        assert_eq!(state.selected, Some(0));

        // Select prev from first item
        state.select_prev();
        assert_eq!(state.selected, None);
    }

    #[test]
    fn test_add_output() {
        let mut state = PluginState::new();
        state.add_output("Line 1");
        state.add_output("Line 2");
        assert_eq!(state.output_text, "Line 1\nLine 2");
    }

    #[test]
    fn test_worktrees_by_status() {
        let mut state = PluginState::new();
        state.worktrees = vec![
            Worktree::new("main", PathBuf::from("/repo"), "main"),
            Worktree::new("dirty", PathBuf::from("/repo/dirty"), "dirty").with_task("TD-1"),
            Worktree::new("clean-task", PathBuf::from("/repo/clean"), "clean").with_task("TD-2"),
            Worktree::new("clean", PathBuf::from("/repo/clean2"), "clean2"),
        ];

        // Mark one as dirty
        state.worktrees[1].is_dirty = true;

        let groups = state.worktrees_by_status();
        assert_eq!(groups.active.len(), 1);
        assert_eq!(groups.waiting.len(), 1);
        assert_eq!(groups.done.len(), 2);
    }

    #[test]
    fn test_modal_state() {
        let mut state = PluginState::new();
        assert!(!state.is_modal_open());

        state.modal_state = ModalState::CreateWorktree;
        assert!(state.is_modal_open());

        state.close_modal();
        assert!(!state.is_modal_open());
        assert_eq!(state.modal_state, ModalState::None);
    }
}
