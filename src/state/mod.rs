//! Persistent state management for RightClick.
//!
//! This module provides thread-safe access to application state that persists
//! across sessions. State is stored in JSON format at:
//! - `~/.config/rightclick/state.json` on Unix
//! - `%APPDATA%\rightclick\rightclick\state.json` on Windows
//!
//! # Architecture
//!
//! The state system uses a global singleton pattern with `RwLock` for
//! thread-safe access. The [`with_state`] and [`with_state_mut`] functions
//! provide safe read and write access to the global state.
//!
//! # Usage
//!
//! ```no_run
//! use rightclick::state::{init, with_state, with_state_mut};
//!
//! // Initialize once at application startup
//! init().expect("Failed to initialize state");
//!
//! // Read state
//! let diff_mode = with_state(|s| s.git_diff_mode);
//!
//! // Modify state (automatically persisted)
//! with_state_mut(|s| {
//!     s.git_diff_mode = rightclick::state::DiffMode::SideBySide;
//! });
//! ```

use crate::state::persistence::{load, save};
// Types are re-exported below
use anyhow::{Context, Result};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use tracing::{debug, error};

pub mod persistence;
pub mod types;

// Re-export commonly used types
pub use types::{DiffMode, FileBrowserState, State, ViewMode, WorkspaceState};

/// Global state singleton.
///
/// This is initialized once by calling [`init`] and provides thread-safe
/// access to the application state throughout the program lifetime.
static GLOBAL_STATE: OnceCell<RwLock<State>> = OnceCell::new();

/// Error type for state operations.
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    /// State has not been initialized.
    #[error("State not initialized. Call init() before accessing state")]
    NotInitialized,

    /// Failed to load state from disk.
    #[error("Failed to load state: {0}")]
    LoadError(#[from] anyhow::Error),
}

/// Initializes the global state.
///
/// This function must be called before any other state operations.
/// It loads the state from disk if it exists, or creates a default
/// state if it doesn't.
///
/// # Errors
///
/// Returns an error if:
/// - The state file exists but cannot be read or parsed
/// - State has already been initialized
///
/// # Example
///
/// ```no_run
/// use rightclick::state::init;
///
/// fn main() -> anyhow::Result<()> {
///     init()?;
///     // Now state operations can be used
///     Ok(())
/// }
/// ```
pub fn init() -> Result<()> {
    let state = load().context("Failed to load state from disk")?;

    GLOBAL_STATE
        .set(RwLock::new(state))
        .map_err(|_| anyhow::anyhow!("State already initialized"))?;

    debug!("Global state initialized");
    Ok(())
}

/// Provides read-only access to the global state.
///
/// The provided closure receives an immutable reference to the state
/// and can return any type. This function blocks if another thread
/// is writing to the state.
///
/// # Panics
///
/// Panics if [`init`] has not been called.
///
/// # Example
///
/// ```no_run
/// use rightclick::state::{init, with_state, DiffMode};
///
/// init().unwrap();
///
/// // Get a copy of the diff mode
/// let mode = with_state(|s| s.git_diff_mode);
///
/// // Check if a plugin is active for a workdir
/// let is_active = with_state(|s| {
///     s.active_plugins.get("/home/user/project").map(|p| p == "git")
/// });
/// ```
pub fn with_state<T, F>(f: F) -> T
where
    F: FnOnce(&State) -> T,
{
    let state = GLOBAL_STATE
        .get()
        .expect("State not initialized. Call init() first");
    let guard = state.read();
    f(&guard)
}

/// Provides mutable access to the global state.
///
/// The provided closure receives a mutable reference to the state
/// and can return any type. This function blocks if any other thread
/// is reading or writing the state. The state is automatically
/// persisted to disk after the closure completes successfully.
///
/// # Panics
///
/// Panics if [`init`] has not been called.
///
/// # Example
///
/// ```no_run
/// use rightclick::state::{init, with_state_mut, DiffMode};
///
/// init().unwrap();
///
/// // Change the diff mode
/// with_state_mut(|s| {
///     s.git_diff_mode = DiffMode::SideBySide;
/// });
///
/// // Set an active plugin for a workdir
/// with_state_mut(|s| {
///     s.active_plugins.insert(
///         "/home/user/project".to_string(),
///         "git".to_string()
///     );
/// });
/// ```
pub fn with_state_mut<T, F>(f: F) -> T
where
    F: FnOnce(&mut State) -> T,
{
    let state = GLOBAL_STATE
        .get()
        .expect("State not initialized. Call init() first");
    let mut guard = state.write();
    let result = f(&mut guard);

    // Persist the state after mutation
    let state_ref: &State = &guard;
    if let Err(e) = save(state_ref) {
        error!("Failed to persist state: {}", e);
        // We don't return the error here because the mutation already happened.
        // The caller can choose to handle this or not.
    }

    result
}

/// Forces a save of the current state to disk.
///
/// This is typically not needed as [`with_state_mut`] automatically
/// saves after modifications. Use this if you've made changes through
/// other means or want to ensure the state is flushed to disk.
///
/// # Errors
///
/// Returns an error if the state cannot be saved to disk.
///
/// # Panics
///
/// Panics if [`init`] has not been called.
pub fn force_save() -> Result<()> {
    with_state(|s| save(s))
}

/// Returns a copy of the current state.
///
/// This is a convenience function equivalent to `with_state(|s| s.clone())`.
///
/// # Panics
///
/// Panics if [`init`] has not been called.
pub fn get_state() -> State {
    with_state(|s| s.clone())
}

/// Replaces the entire state with a new value.
///
/// This is a convenience function that replaces the entire state
/// and persists it to disk. Use with caution as it overwrites all state.
///
/// # Panics
///
/// Panics if [`init`] has not been called.
pub fn set_state(new_state: State) {
    with_state_mut(|s| {
        *s = new_state;
    });
}

/// Resets the state to default values.
///
/// This clears all persistent state and returns to default values.
/// The cleared state is immediately saved to disk.
///
/// # Panics
///
/// Panics if [`init`] has not been called.
pub fn reset() {
    with_state_mut(|s| {
        *s = State::default();
    });
}

/// Gets the file browser state for a specific working directory.
///
/// Returns `None` if no state exists for the given workdir.
///
/// # Panics
///
/// Panics if [`init`] has not been called.
pub fn get_file_browser_state(workdir: &str) -> Option<FileBrowserState> {
    with_state(|s| s.file_browser.get(workdir).cloned())
}

/// Sets the file browser state for a specific working directory.
///
/// # Panics
///
/// Panics if [`init`] has not been called.
pub fn set_file_browser_state(workdir: &str, fb_state: FileBrowserState) {
    with_state_mut(|s| {
        s.file_browser.insert(workdir.to_string(), fb_state);
    });
}

/// Gets the workspace state for a specific working directory.
///
/// Returns `None` if no state exists for the given workdir.
///
/// # Panics
///
/// Panics if [`init`] has not been called.
pub fn get_workspace_state(workdir: &str) -> Option<WorkspaceState> {
    with_state(|s| s.workspace.get(workdir).cloned())
}

/// Sets the workspace state for a specific working directory.
///
/// # Panics
///
/// Panics if [`init`] has not been called.
pub fn set_workspace_state(workdir: &str, ws_state: WorkspaceState) {
    with_state_mut(|s| {
        s.workspace.insert(workdir.to_string(), ws_state);
    });
}

/// Gets the active plugin for a specific working directory.
///
/// Returns `None` if no plugin is active for the given workdir.
///
/// # Panics
///
/// Panics if [`init`] has not been called.
pub fn get_active_plugin(workdir: &str) -> Option<String> {
    with_state(|s| s.active_plugins.get(workdir).cloned())
}

/// Sets the active plugin for a specific working directory.
///
/// # Panics
///
/// Panics if [`init`] has not been called.
pub fn set_active_plugin(workdir: &str, plugin_id: &str) {
    with_state_mut(|s| {
        s.active_plugins
            .insert(workdir.to_string(), plugin_id.to_string());
    });
}

/// Gets the last active worktree for a repository.
///
/// Returns `None` if no worktree is recorded for the given repo path.
///
/// # Panics
///
/// Panics if [`init`] has not been called.
pub fn get_last_worktree(repo: &str) -> Option<String> {
    with_state(|s| s.last_worktree.get(repo).cloned())
}

/// Sets the last active worktree for a repository.
///
/// # Panics
///
/// Panics if [`init`] has not been called.
pub fn set_last_worktree(repo: &str, worktree: &str) {
    with_state_mut(|s| {
        s.last_worktree
            .insert(repo.to_string(), worktree.to_string());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() {
        // Reset the global state for testing
        // Note: This is a hack for testing only. In real code, init() should only be called once.
        let _ = GLOBAL_STATE.set(RwLock::new(State::default()));
    }

    #[test]
    fn test_with_state_read() {
        setup();
        let version = with_state(|s| s.version);
        assert_eq!(version, 1);
    }

    #[test]
    fn test_with_state_mut_write() {
        setup();
        with_state_mut(|s| {
            s.git_graph_enabled = false;
        });
        let enabled = with_state(|s| s.git_graph_enabled);
        assert!(!enabled);
    }

    #[test]
    fn test_active_plugin_helpers() {
        setup();
        let workdir = "/home/user/project";
        let plugin = "git";

        assert!(get_active_plugin(workdir).is_none());

        set_active_plugin(workdir, plugin);
        assert_eq!(get_active_plugin(workdir), Some(plugin.to_string()));
    }

    #[test]
    fn test_file_browser_helpers() {
        setup();
        let workdir = "/home/user/project";
        let fb_state = FileBrowserState {
            selected_file: Some("src/main.rs".to_string()),
            expanded_dirs: vec!["src".to_string()],
            scroll_offset: 10,
        };

        assert!(get_file_browser_state(workdir).is_none());

        set_file_browser_state(workdir, fb_state.clone());
        let retrieved = get_file_browser_state(workdir).unwrap();
        assert_eq!(retrieved.selected_file, fb_state.selected_file);
    }

    #[test]
    fn test_workspace_helpers() {
        setup();
        let workdir = "/home/user/project";
        let ws_state = WorkspaceState {
            selected_workspace: Some("backend".to_string()),
            view_mode: ViewMode::Kanban,
        };

        assert!(get_workspace_state(workdir).is_none());

        set_workspace_state(workdir, ws_state.clone());
        let retrieved = get_workspace_state(workdir).unwrap();
        assert_eq!(retrieved.view_mode, ViewMode::Kanban);
    }

    #[test]
    fn test_last_worktree_helpers() {
        setup();
        let repo = "/home/user/repo";
        let worktree = "/home/user/repo-feature-branch";

        assert!(get_last_worktree(repo).is_none());

        set_last_worktree(repo, worktree);
        assert_eq!(get_last_worktree(repo), Some(worktree.to_string()));
    }

    #[test]
    fn test_thread_safety() {
        setup();
        use std::thread;

        let handles: Vec<_> = (0..10)
            .map(|i| {
                thread::spawn(move || {
                    let workdir = format!("/project/{}", i);
                    set_active_plugin(&workdir, &format!("plugin-{}", i));
                    get_active_plugin(&workdir)
                })
            })
            .collect();

        for (i, handle) in handles.into_iter().enumerate() {
            let result = handle.join().unwrap();
            assert_eq!(result, Some(format!("plugin-{}", i)));
        }
    }
}
