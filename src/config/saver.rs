//! Configuration saving operations.
//!
//! This module handles saving configuration to files, including
/// directory creation and atomic file writes to prevent corruption.
use crate::config::paths::config_path;
use crate::core::models::config::Config;
use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;
use tracing::debug;

/// Ensures the parent directory for the given path exists.
///
/// Creates the directory and all parent directories if they don't exist.
fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }
    Ok(())
}

/// Saves configuration to the default config path.
///
/// Writes the config to a temporary file first, then renames it to the
/// target path. This ensures that the config file is never in a partially
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
/// use rightclick::config::{Config, save};
///
/// let config = Config::default();
/// save(&config).expect("Failed to save configuration");
/// ```
pub fn save(config: &Config) -> Result<()> {
    let path = config_path()?;
    save_to(config, &path)
}

/// Saves configuration to a specific path.
///
/// Writes the config to a temporary file first, then renames it to the
/// target path. This ensures that the config file is never in a partially
/// written state.
///
/// # Errors
///
/// Returns an error if:
/// - The parent directory cannot be created
/// - The temporary file cannot be written
/// - The rename operation fails
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use rightclick::config::{Config, save_to};
///
/// let config = Config::default();
/// save_to(&config, Path::new("/path/to/config.json"))
///     .expect("Failed to save configuration");
/// ```
pub fn save_to(config: &Config, path: &Path) -> Result<()> {
    ensure_parent_dir(path)?;

    // Serialize to a string first to validate
    let json =
        serde_json::to_string_pretty(config).context("Failed to serialize config to JSON")?;

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
    fs::rename(&temp_path, path)
        .with_context(|| format!("Failed to rename temp file to: {}", path.display()))?;

    debug!("Saved config to {}", path.display());
    Ok(())
}

/// Saves configuration to the default config path using a method on Config.
///
/// This is a convenience method that allows calling `config.save()` directly.
///
/// # Errors
///
/// Returns an error if saving fails. See [`save`] for details.
pub fn save_config(config: &Config) -> Result<()> {
    save(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::TempDir;
    use crate::config::loader::load_from;

    #[test]
    fn save_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");

        let config = Config::default();
        save_to(&config, &path).unwrap();

        assert!(path.exists());

        let mut contents = String::new();
        fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert!(contents.contains("version"));
        assert!(contents.contains("projects"));
        assert!(contents.contains("plugins"));
    }

    #[test]
    fn save_creates_parent_directories() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir
            .path()
            .join("nested")
            .join("deep")
            .join("config.json");

        let config = Config::default();
        save_to(&config, &path).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn save_overwrites_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");

        fs::write(&path, "old content").unwrap();

        let config = Config::default();
        save_to(&config, &path).unwrap();

        let mut contents = String::new();
        fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert!(contents.contains("version"));
        assert!(!contents.contains("old content"));
    }

    #[test]
    fn roundtrip_preserves_config() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");

        let config = Config::default();
        save_to(&config, &path).unwrap();
        let loaded: Config = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();

       assert_eq!(config, loaded);
    }

    #[test]
    fn save_to_nested_directory() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir
            .path()
            .join("a")
            .join("b")
            .join("c")
            .join("config.json");

        save_to(&Config::default(), &path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn save_and_load_roundtrip_modified_config() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");

        let mut config = Config::default();
        config.ui.compact_mode = true;
        config.ui.theme = "light".to_string();
        config.plugins.git_status.refresh_interval = 10;

        save_to(&config, &path).unwrap();
        let loaded = load_from(&path).unwrap();

        assert_eq!(config, loaded);
        assert!(loaded.ui.compact_mode);
        assert_eq!(loaded.ui.theme, "light");
        assert_eq!(loaded.plugins.git_status.refresh_interval, 10);
    }
}
