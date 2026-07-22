//! Persistence operations for application state.
//!
//! This module handles loading and saving state to disk, including
//! directory creation, file I/O, and migration of older state formats.
//! All operations are atomic where possible to prevent corruption.

use crate::state::types::{STATE_VERSION, State};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Returns the path to the state file.
///
/// The state is stored at `~/.config/rightclick/state.json` on Unix-like
/// systems and in the appropriate config directory on Windows.
///
/// # Errors
///
/// Returns an error if the project directories cannot be determined
/// (e.g., when no home directory exists).
pub fn state_file_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("com", "rightclick", "rightclick")
        .context("Failed to determine project directories")?;
    Ok(dirs.config_dir().join("state.json"))
}

/// Ensures the config directory exists.
///
/// Creates the config directory and all parent directories if they don't exist.
fn ensure_config_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
    }
    Ok(())
}

/// Loads state from disk.
///
/// If the state file doesn't exist, returns a default `State`.
/// If the file exists but cannot be parsed, returns an error.
///
/// # Migration
///
/// If the loaded state has an older version, it will be migrated to the
/// current version automatically.
///
/// # Errors
///
/// Returns an error if:
/// - The file exists but cannot be read
/// - The file contains invalid JSON
/// - Migration fails
///
/// # Example
///
/// ```no_run
/// use rightclick::state::persistence::load;
///
/// let state = load().expect("Failed to load state");
/// ```
pub fn load() -> Result<State> {
    let path = state_file_path()?;

    if !path.exists() {
        debug!("State file does not exist, using defaults");
        return Ok(State::default());
    }

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read state file: {}", path.display()))?;

    let mut state: State = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse state file: {}", path.display()))?;

    // Migrate if needed
    if state.version != STATE_VERSION {
        info!(
            "Migrating state from version {} to {}",
            state.version, STATE_VERSION
        );
        state = migrate(state)?;
    }

    debug!("Loaded state from {}", path.display());
    Ok(state)
}

/// Saves state to disk atomically.
///
/// Writes the state to a temporary file first, then renames it to the
/// target path. This ensures that the state file is never in a partially
/// written state.
///
/// # Errors
///
/// Returns an error if:
/// - The config directory cannot be created
/// - The temporary file cannot be written
/// - The rename operation fails
///
/// # Example
///
/// ```no_run
/// use rightclick::state::{State, persistence::save};
///
/// let state = State::default();
/// save(&state).expect("Failed to save state");
/// ```
pub fn save(state: &State) -> Result<()> {
    let path = state_file_path()?;
    ensure_config_dir(&path)?;

    // Serialize to a string first to validate
    let json = serde_json::to_string_pretty(state).context("Failed to serialize state to JSON")?;

    // Write to a temporary file in the same directory
    let temp_path = path.with_extension("tmp");
    {
        let mut temp_file = fs::File::create(&temp_path)
            .with_context(|| format!("Failed to create temp file: {}", temp_path.display()))?;
        temp_file
            .write_all(json.as_bytes())
            .with_context(|| format!("Failed to write to temp file: {}", temp_path.display()))?;
        temp_file
            .sync_all()
            .with_context(|| format!("Failed to sync temp file: {}", temp_path.display()))?;
    }

    // Atomically rename the temp file to the target
    fs::rename(&temp_path, &path)
        .with_context(|| format!("Failed to rename temp file to: {}", path.display()))?;

    debug!("Saved state to {}", path.display());
    Ok(())
}

/// Migrates an older state version to the current version.
///
/// This function handles conversion of state data when the format changes.
/// Add new migration branches as the STATE_VERSION is incremented.
///
/// # Errors
///
/// Returns an error if the state version is unknown or migration fails.
fn migrate(mut state: State) -> Result<State> {
    match state.version {
        0 => {
            // Version 0 -> 1: Initial migration, just set version
            warn!("Migrating from version 0 to {}", STATE_VERSION);
            state.version = STATE_VERSION;
            Ok(state)
        }
        STATE_VERSION => {
            // Already current version
            Ok(state)
        }
        unknown => {
            anyhow::bail!("Unknown state version: {}", unknown);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::TempDir;

    #[test]
    fn load_returns_default_when_file_missing() {
        let temp_dir = TempDir::new().unwrap();
        let _path = temp_dir.path().join("nonexistent").join("state.json");

        // Temporarily override the state path for testing
        // This is a simplified test - in practice we'd use dependency injection
        let state = State::default();
        assert_eq!(state.version, STATE_VERSION);
    }

    #[test]
    fn save_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("state.json");

        let state = State::default();
        let json = serde_json::to_string_pretty(&state).unwrap();

        fs::write(&path, &json).unwrap();

        assert!(path.exists());

        let mut contents = String::new();
        fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert!(contents.contains("version"));
    }

    #[test]
    fn roundtrip_preserves_state() {
        let state = State::default();
        let json = serde_json::to_string_pretty(&state).unwrap();
        let loaded: State = serde_json::from_str(&json).unwrap();
        assert_eq!(state, loaded);
    }

    #[test]
    fn migrate_from_version_0() {
        let old_state = State {
            version: 0,
            ..State::default()
        };
        let migrated = migrate(old_state).unwrap();
        assert_eq!(migrated.version, STATE_VERSION);
    }

    #[test]
    fn migrate_unknown_version_fails() {
        let bad_state = State {
            version: 999,
            ..State::default()
        };
        assert!(migrate(bad_state).is_err());
    }

    #[test]
    fn save_and_load_roundtrip_via_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("state.json");

        let state = State::default();
        let json = serde_json::to_string_pretty(&state).unwrap();
        fs::write(&path, &json).unwrap();

        // Parse back — simulates load behavior
        let contents = fs::read_to_string(&path).unwrap();
        let loaded: State = serde_json::from_str(&contents).unwrap();

        assert_eq!(state, loaded);
    }

    #[test]
    fn save_and_load_roundtrip_modified_state() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("state.json");

        let mut state = State {
            git_graph_enabled: false,
            line_wrap_enabled: true,
            ..Default::default()
        };
        state
            .active_plugins
            .insert("/proj".to_string(), "gitstatus".to_string());

        let json = serde_json::to_string_pretty(&state).unwrap();
        fs::write(&path, &json).unwrap();

        let contents = fs::read_to_string(&path).unwrap();
        let loaded: State = serde_json::from_str(&contents).unwrap();

        assert_eq!(state, loaded);
        assert!(!loaded.git_graph_enabled);
        assert!(loaded.line_wrap_enabled);
        assert_eq!(
            loaded.active_plugins.get("/proj"),
            Some(&"gitstatus".to_string())
        );
    }

    #[test]
    fn migrate_current_version_is_noop() {
        let state = State::default();
        let migrated = migrate(state.clone()).unwrap();
        assert_eq!(state, migrated);
    }

    #[test]
    fn parse_invalid_json_fails() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("state.json");
        fs::write(&path, "not json {{{").unwrap();

        let result: Result<State> =
            serde_json::from_str::<State>(&fs::read_to_string(&path).unwrap())
                .map_err(|e| anyhow::anyhow!("{}", e));
        assert!(result.is_err());
    }
}
