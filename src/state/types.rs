//! State data structures for persistent UI state.
//!
//! This module defines the core data types that represent the application's
//! persistent state. All types are serializable and implement `Default` for
//! sensible initial values.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Current state file format version.
///
/// This is incremented when breaking changes are made to the state structure.
/// Migration logic should check this version and handle older formats appropriately.
pub const STATE_VERSION: u32 = 1;

/// The main application state that persists across sessions.
///
/// `State` tracks UI preferences and per-workspace settings that should be
/// restored when the application restarts. It is stored as JSON in the
/// user's config directory.
///
/// # Example
///
/// ```
/// use rightclick::state::State;
///
/// let state = State::default();
/// assert_eq!(state.version, 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct State {
    /// Version of the state file format.
    pub version: u32,

    /// Diff display mode for git operations.
    pub git_diff_mode: DiffMode,

    /// Diff display mode for workspace operations.
    pub workspace_diff_mode: DiffMode,

    /// Whether the git graph visualization is enabled.
    pub git_graph_enabled: bool,

    /// Whether line wrapping is enabled in text views.
    pub line_wrap_enabled: bool,

    /// Map of workdir path to active plugin ID.
    ///
    /// Tracks which plugin was last active for each working directory,
    /// allowing the UI to restore the context when reopening a directory.
    pub active_plugins: HashMap<String, String>,

    /// Per-directory file browser state.
    pub file_browser: HashMap<String, FileBrowserState>,

    /// Per-directory workspace state.
    pub workspace: HashMap<String, WorkspaceState>,

    /// Map of repository path to last active worktree path.
    ///
    /// Used to restore the correct worktree when reopening a repository
    /// that uses git worktrees.
    pub last_worktree: HashMap<String, String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            git_diff_mode: DiffMode::default(),
            workspace_diff_mode: DiffMode::default(),
            git_graph_enabled: true,
            line_wrap_enabled: false,
            active_plugins: HashMap::new(),
            file_browser: HashMap::new(),
            workspace: HashMap::new(),
            last_worktree: HashMap::new(),
        }
    }
}

/// File browser state for a specific working directory.
///
/// Tracks the user's position and expansion state in the file tree,
/// allowing the UI to restore the exact view when returning to a directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileBrowserState {
    /// Path of the currently selected file, relative to the workdir.
    pub selected_file: Option<String>,

    /// List of expanded directory paths, relative to the workdir.
    pub expanded_dirs: Vec<String>,

    /// Vertical scroll offset in the file list.
    pub scroll_offset: usize,
}

impl Default for FileBrowserState {
    fn default() -> Self {
        Self {
            selected_file: None,
            expanded_dirs: Vec::new(),
            scroll_offset: 0,
        }
    }
}

/// Workspace state for a specific working directory.
///
/// Tracks workspace-specific UI preferences like view mode and
/// selected workspace folder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceState {
    /// Name or path of the selected workspace folder.
    pub selected_workspace: Option<String>,

    /// Current view mode for the workspace display.
    pub view_mode: ViewMode,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            selected_workspace: None,
            view_mode: ViewMode::default(),
        }
    }
}

/// Diff display mode for comparing file versions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffMode {
    /// Unified diff format (single column with +/- markers).
    Unified,
    /// Side-by-side diff format (two columns, before and after).
    SideBySide,
}

impl Default for DiffMode {
    fn default() -> Self {
        Self::Unified
    }
}

/// View mode for workspace display.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    /// List view with file/folder names in a column.
    List,
    /// Kanban-style board view for task-oriented workflows.
    Kanban,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self::List
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_default_values() {
        let state = State::default();
        assert_eq!(state.version, STATE_VERSION);
        assert_eq!(state.git_diff_mode, DiffMode::Unified);
        assert_eq!(state.workspace_diff_mode, DiffMode::Unified);
        assert!(state.git_graph_enabled);
        assert!(!state.line_wrap_enabled);
        assert!(state.active_plugins.is_empty());
        assert!(state.file_browser.is_empty());
        assert!(state.workspace.is_empty());
        assert!(state.last_worktree.is_empty());
    }

    #[test]
    fn file_browser_state_default() {
        let fb = FileBrowserState::default();
        assert!(fb.selected_file.is_none());
        assert!(fb.expanded_dirs.is_empty());
        assert_eq!(fb.scroll_offset, 0);
    }

    #[test]
    fn workspace_state_default() {
        let ws = WorkspaceState::default();
        assert!(ws.selected_workspace.is_none());
        assert_eq!(ws.view_mode, ViewMode::List);
    }

    #[test]
    fn diff_mode_serialization() {
        assert_eq!(
            serde_json::to_string(&DiffMode::Unified).unwrap(),
            r#""unified""#
        );
        assert_eq!(
            serde_json::to_string(&DiffMode::SideBySide).unwrap(),
            r#""side_by_side""#
        );
    }

    #[test]
    fn view_mode_serialization() {
        assert_eq!(serde_json::to_string(&ViewMode::List).unwrap(), r#""list""#);
        assert_eq!(
            serde_json::to_string(&ViewMode::Kanban).unwrap(),
            r#""kanban""#
        );
    }

    #[test]
    fn state_roundtrip_serialization() {
        let original = State::default();
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: State = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }
}
