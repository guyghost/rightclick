//! Git Status Plugin State
//!
//! This module defines the state structure for the Git Status plugin,
//! including view modes, focus panes, and selection state.

use crate::core::models::{Commit, Diff, FileChange, FileDiff, FileStatus, RepoStatus};

/// The plugin's view mode determines what content is displayed
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    /// Show working directory status (staged/unstaged files)
    Status,
    /// Show diff for selected file
    Diff,
    /// Show commit history
    History,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self::History
    }
}

/// Which pane has keyboard focus
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusPane {
    /// Sidebar with file list
    Sidebar,
    /// Main content area (diff or history)
    Main,
}

impl Default for FocusPane {
    fn default() -> Self {
        Self::Sidebar
    }
}

/// Plugin state containing all mutable data
#[derive(Clone, Debug, Default)]
pub struct PluginState {
    /// All file changes (staged, unstaged, untracked)
    pub files: Vec<FileChange>,
    /// Recent commits for history view
    pub commits: Vec<Commit>,
    /// Currently selected file index in sidebar
    pub selected_file: Option<usize>,
    /// Currently selected commit index in history
    pub selected_commit: Option<usize>,
    /// Current diff being displayed
    pub diff: Option<Diff>,
    /// Current view mode
    pub view_mode: ViewMode,
    /// Which pane has focus
    pub focus_pane: FocusPane,
    /// Sidebar width in columns
    pub sidebar_width: u16,
    /// Current branch name
    pub branch: String,
    /// Number of commits ahead of upstream
    pub ahead: usize,
    /// Number of commits behind upstream
    pub behind: usize,
    /// Whether the repo has uncommitted changes
    pub is_dirty: bool,
    /// Changed files for the selected commit
    pub commit_files: Vec<FileDiff>,
}

impl PluginState {
    /// Create a new empty state
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            commits: Vec::new(),
            selected_file: None,
            selected_commit: None,
            diff: None,
            view_mode: ViewMode::default(),
            focus_pane: FocusPane::default(),
            sidebar_width: 35,
            branch: String::new(),
            ahead: 0,
            behind: 0,
            is_dirty: false,
            commit_files: Vec::new(),
        }
    }

    /// Get staged files
    pub fn staged_files(&self) -> Vec<&FileChange> {
        self.files
            .iter()
            .filter(|f| f.status == FileStatus::Staged)
            .collect()
    }

    /// Get unstaged files
    pub fn unstaged_files(&self) -> Vec<&FileChange> {
        self.files
            .iter()
            .filter(|f| f.status == FileStatus::Modified || f.status == FileStatus::Deleted)
            .collect()
    }

    /// Get untracked files
    pub fn untracked_files(&self) -> Vec<&FileChange> {
        self.files
            .iter()
            .filter(|f| f.status == FileStatus::Untracked)
            .collect()
    }

    /// Get the currently selected file if any
    pub fn selected_file(&self) -> Option<&FileChange> {
        self.selected_file.and_then(|idx| self.files.get(idx))
    }

    /// Get the path of the currently selected file if any
    pub fn selected_file_path(&self) -> Option<std::path::PathBuf> {
        self.selected_file().map(|f| std::path::PathBuf::from(&f.path))
    }

    /// Get the currently selected commit if any
    pub fn selected_commit(&self) -> Option<&Commit> {
        self.selected_commit.and_then(|idx| self.commits.get(idx))
    }

    /// Move selection down in the sidebar
    pub fn select_next_file(&mut self) {
        if self.files.is_empty() {
            self.selected_file = None;
            return;
        }

        match self.selected_file {
            None => self.selected_file = Some(0),
            Some(idx) => {
                self.selected_file = Some((idx + 1).min(self.files.len() - 1));
            }
        }
    }

    /// Move selection up in the sidebar
    pub fn select_prev_file(&mut self) {
        match self.selected_file {
            None => {
                if !self.files.is_empty() {
                    self.selected_file = Some(self.files.len() - 1);
                }
            }
            Some(0) => self.selected_file = None,
            Some(idx) => self.selected_file = Some(idx.saturating_sub(1)),
        }
    }

    /// Move selection down in commit history
    pub fn select_next_commit(&mut self) {
        if self.commits.is_empty() {
            self.selected_commit = None;
            return;
        }

        match self.selected_commit {
            None => self.selected_commit = Some(0),
            Some(idx) => {
                self.selected_commit = Some((idx + 1).min(self.commits.len() - 1));
            }
        }
    }

    /// Move selection up in commit history
    pub fn select_prev_commit(&mut self) {
        match self.selected_commit {
            None => {
                if !self.commits.is_empty() {
                    self.selected_commit = Some(self.commits.len() - 1);
                }
            }
            Some(0) => self.selected_commit = None,
            Some(idx) => self.selected_commit = Some(idx.saturating_sub(1)),
        }
    }

    /// Clear all state
    pub fn clear(&mut self) {
        self.files.clear();
        self.commits.clear();
        self.selected_file = None;
        self.selected_commit = None;
        self.diff = None;
        self.branch.clear();
        self.ahead = 0;
        self.behind = 0;
        self.is_dirty = false;
        self.commit_files.clear();
    }

    /// Update state from repo status
    pub fn update_from_repo_status(&mut self, status: &RepoStatus) {
        self.files.clear();
        self.files.extend(status.staged.clone());
        self.files.extend(status.unstaged.clone());
        self.files.extend(status.untracked.clone());
        self.files.extend(status.conflicted.clone());
        self.branch = status.branch.clone();
        self.ahead = status.ahead;
        self.behind = status.behind;
        self.is_dirty = status.is_dirty;
        self.commit_files.clear();

        // Reset selection if out of bounds
        if let Some(idx) = self.selected_file {
            if idx >= self.files.len() {
                self.selected_file = if self.files.is_empty() {
                    None
                } else {
                    Some(self.files.len() - 1)
                };
            }
        }
    }

    /// Update state from repo status (alias for update_from_repo_status)
    pub fn update_status(&mut self, status: RepoStatus) {
        self.update_from_repo_status(&status);
    }

    /// Alias for select_next_file
    pub fn next_file(&mut self) {
        self.select_next_file();
    }

    /// Alias for select_prev_file
    pub fn prev_file(&mut self) {
        self.select_prev_file();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_mode_default() {
        assert_eq!(ViewMode::default(), ViewMode::Status);
    }

    #[test]
    fn test_focus_pane_default() {
        assert_eq!(FocusPane::default(), FocusPane::Sidebar);
    }

    #[test]
    fn test_state_new() {
        let state = PluginState::new();
        assert!(state.files.is_empty());
        assert!(state.commits.is_empty());
        assert_eq!(state.selected_file, None);
        assert_eq!(state.view_mode, ViewMode::Status);
        assert_eq!(state.sidebar_width, 35);
    }

    #[test]
    fn test_file_selection() {
        let mut state = PluginState::new();
        state.files = vec![
            FileChange::new("a.rs", FileStatus::Modified),
            FileChange::new("b.rs", FileStatus::Staged),
            FileChange::new("c.rs", FileStatus::Untracked),
        ];

        // Initially no selection
        assert_eq!(state.selected_file, None);

        // Select next
        state.select_next_file();
        assert_eq!(state.selected_file, Some(0));

        // Select next
        state.select_next_file();
        assert_eq!(state.selected_file, Some(1));

        // Select prev
        state.select_prev_file();
        assert_eq!(state.selected_file, Some(0));

        // Select prev from first item
        state.select_prev_file();
        assert_eq!(state.selected_file, None);
    }

    #[test]
    fn test_file_filters() {
        let mut state = PluginState::new();
        state.files = vec![
            FileChange::new("staged.rs", FileStatus::Staged),
            FileChange::new("modified.rs", FileStatus::Modified),
            FileChange::new("untracked.rs", FileStatus::Untracked),
        ];

        assert_eq!(state.staged_files().len(), 1);
        assert_eq!(state.staged_files()[0].path, "staged.rs");

        assert_eq!(state.unstaged_files().len(), 1);
        assert_eq!(state.unstaged_files()[0].path, "modified.rs");

        assert_eq!(state.untracked_files().len(), 1);
        assert_eq!(state.untracked_files()[0].path, "untracked.rs");
    }

    #[test]
    fn test_clear() {
        let mut state = PluginState::new();
        state
            .files
            .push(FileChange::new("test.rs", FileStatus::Modified));
        state.branch = "main".to_string();
        state.ahead = 1;
        state.behind = 2;

        state.clear();

        assert!(state.files.is_empty());
        assert!(state.branch.is_empty());
        assert_eq!(state.ahead, 0);
        assert_eq!(state.behind, 0);
    }

    #[test]
    fn test_selected_file_path() {
        let mut state = PluginState::new();
        state.files = vec![
            FileChange::new("test.rs", FileStatus::Modified),
            FileChange::new("other.rs", FileStatus::Staged),
        ];
        state.selected_file = Some(0);

        let path = state.selected_file_path();
        assert!(path.is_some());
        assert_eq!(path.unwrap().to_str(), Some("test.rs"));
    }

    #[test]
    fn test_update_status() {
        let mut state = PluginState::new();
        let status = RepoStatus {
            branch: "main".to_string(),
            head: "abc123".to_string(),
            is_dirty: true,
            staged: vec![FileChange::new("staged.rs", FileStatus::Staged)],
            unstaged: vec![FileChange::new("modified.rs", FileStatus::Modified)],
            untracked: vec![FileChange::new("new.rs", FileStatus::Untracked)],
            conflicted: vec![],
            ahead: 2,
            behind: 1,
            state: None,
        };

        state.update_status(status);

        assert_eq!(state.branch, "main");
        assert_eq!(state.files.len(), 3);
        assert_eq!(state.ahead, 2);
        assert_eq!(state.behind, 1);
    }
}
