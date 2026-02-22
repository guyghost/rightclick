//! Configuration loading operations.
//!
//! This module handles loading configuration from files, including
/// migration of older config formats and graceful handling of missing files.
use crate::config::paths::config_path;
use crate::core::models::config::{CURRENT_CONFIG_VERSION, Config};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use tracing::{debug, info, warn};

/// Loads configuration from the default config path.
///
/// If the config file doesn't exist, returns a default `Config`.
/// If the file exists but cannot be parsed, returns an error.
///
/// # Migration
///
/// If the loaded config has an older version, it will be migrated to the
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
/// use rightclick::config::load;
///
/// let config = load().expect("Failed to load configuration");
/// ```
pub fn load() -> Result<Config> {
    let path = config_path()?;
    load_from(&path)
}

/// Loads configuration from a specific path.
///
/// If the config file doesn't exist, returns a default `Config`.
/// If the file exists but cannot be parsed, returns an error.
///
/// # Migration
///
/// If the loaded config has an older version, it will be migrated to the
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
/// use std::path::Path;
/// use rightclick::config::load_from;
///
/// let config = load_from(Path::new("/path/to/config.json"))
///     .expect("Failed to load configuration");
/// ```
pub fn load_from(path: &Path) -> Result<Config> {
    if !path.exists() {
        debug!(
            "Config file does not exist, using defaults: {}",
            path.display()
        );
        return Ok(Config::default());
    }

    let contents = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    let mut config: Config = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

    // Migrate if needed
    if config.version != CURRENT_CONFIG_VERSION {
        info!(
            "Migrating config from version {} to {}",
            config.version, CURRENT_CONFIG_VERSION
        );
        config = migrate(config)?;
    }

    debug!("Loaded config from {}", path.display());
    Ok(config)
}

/// Migrates an older config version to the current version.
///
/// This function handles conversion of config data when the format changes.
/// Add new migration branches as the CURRENT_CONFIG_VERSION is incremented.
///
/// # Errors
///
/// Returns an error if the config version is unknown or migration fails.
fn migrate(mut config: Config) -> Result<Config> {
    match config.version {
        0 => {
            // Version 0 -> 1: Initial migration, just set version
            warn!("Migrating from version 0 to {}", CURRENT_CONFIG_VERSION);
            config.version = CURRENT_CONFIG_VERSION;
            Ok(config)
        }
        CURRENT_CONFIG_VERSION => {
            // Already current version
            Ok(config)
        }
        unknown => {
            anyhow::bail!("Unknown config version: {}", unknown);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_returns_default_when_file_missing() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("nonexistent").join("config.json");

        let config = load_from(&path).unwrap();
        assert_eq!(config.version, CURRENT_CONFIG_VERSION);
    }

    #[test]
    fn load_parses_valid_config() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");

        let json = r#"{
            "version": 1,
            "projects": { "list": [] },
            "plugins": {
                "git_status": { "enabled": true, "show_untracked": true, "refresh_interval": 5 },
                "conversations": { "enabled": true, "max_history": 100, "save_context": true },
                "file_browser": { "enabled": true, "show_hidden": false, "sort_by": "name", "ignore_patterns": [] },
                "workspaces": { "enabled": true, "auto_save": true, "show_switcher": true }
            },
            "ui": { "show_clock": true, "theme": "dark", "nerd_fonts_enabled": true, "accent_color": "blue", "compact_mode": false },
            "keymap": { "overrides": {} }
        }"#;

        fs::write(&path, json).unwrap();

        let config = load_from(&path).unwrap();
        assert_eq!(config.version, 1);
        assert!(config.ui.show_clock);
        assert_eq!(config.ui.theme, "dark");
    }

    #[test]
    fn migrate_from_version_0() {
        let old_config = Config {
            version: 0,
            ..Config::default()
        };
        let migrated = migrate(old_config).unwrap();
        assert_eq!(migrated.version, CURRENT_CONFIG_VERSION);
    }

    #[test]
    fn migrate_unknown_version_fails() {
        let bad_config = Config {
            version: 999,
            ..Config::default()
        };
        assert!(migrate(bad_config).is_err());
    }

    #[test]
    fn load_invalid_json_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");

        fs::write(&path, "invalid json {{{").unwrap();

        assert!(load_from(&path).is_err());
    }
}
