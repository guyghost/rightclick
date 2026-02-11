//! Configuration file path resolution.
//!
//! This module handles resolving paths to configuration files, using the
//! `directories` crate to determine appropriate locations for the current platform.

use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::path::PathBuf;

/// Returns the path to the configuration directory.
///
/// The config directory is located at:
/// - Linux: `~/.config/rightclick/`
/// - macOS: `~/Library/Application Support/com.rightclick.rightclick/`
/// - Windows: `%APPDATA%/rightclick/rightclick/`
///
/// # Errors
///
/// Returns an error if the project directories cannot be determined
/// (e.g., when no home directory exists).
pub fn config_dir() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("com", "rightclick", "rightclick")
        .context("Failed to determine project directories")?;
    Ok(dirs.config_dir().to_path_buf())
}

/// Returns the path to the default configuration file.
///
/// The config file is located at `~/.config/rightclick/config.json` on Unix-like
/// systems and in the appropriate config directory on Windows.
///
/// # Errors
///
/// Returns an error if the project directories cannot be determined.
///
/// # Example
///
/// ```
/// use rightclick::config::config_path;
///
/// let path = config_path().expect("Failed to determine config path");
/// println!("Config file: {}", path.display());
/// ```
pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_path_returns_valid_path() {
        let path = config_path().unwrap();
        assert!(path.ends_with("config.json"));
    }

    #[test]
    fn config_dir_returns_valid_directory() {
        let dir = config_dir().unwrap();
        assert!(dir.to_string_lossy().contains("rightclick"));
    }
}
