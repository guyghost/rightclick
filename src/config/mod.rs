//! Configuration management for RightClick.
//!
//! This module provides functionality for loading and saving application
//! configuration from/to JSON files. The configuration is stored at:
//! `~/.config/rightclick/config.json` on Unix-like systems.
//!
//! # Example
//!
//! ```no_run
//! use rightclick::config::{load, save, Config};
//!
//! // Load configuration (or use defaults if file doesn't exist)
//! let config = load().expect("Failed to load configuration");
//!
//! // Modify configuration...
//!
//! // Save configuration
//! save(&config).expect("Failed to save configuration");
//! ```

pub mod loader;
pub mod paths;
pub mod saver;

// Re-export commonly used types and functions
pub use crate::core::models::config::Config;
pub use loader::{load, load_from};
pub use paths::{config_dir, config_path};
pub use saver::{save, save_config, save_to};

use anyhow::Result;
use std::path::Path;

/// Returns a default configuration.
///
/// This is a convenience function that creates a new `Config` with all
/// default values set.
///
/// # Example
///
/// ```
/// use rightclick::config::default;
///
/// let config = default();
/// assert_eq!(config.version, 1);
/// ```
pub fn default() -> Config {
    Config::default()
}

/// Configuration operations trait.
///
/// This trait provides methods for loading and saving configuration
/// that can be implemented by the `Config` type.
pub trait ConfigOps {
    /// Load configuration from the default path.
    ///
    /// Returns default configuration if the file doesn't exist.
    fn load() -> Result<Config>;

    /// Load configuration from a specific path.
    ///
    /// Returns default configuration if the file doesn't exist.
    fn load_from(path: &Path) -> Result<Config>;

    /// Save configuration to the default path.
    fn save(&self) -> Result<()>;

    /// Save configuration to a specific path.
    fn save_to(&self, path: &Path) -> Result<()>;
}

impl ConfigOps for Config {
    fn load() -> Result<Config> {
        loader::load()
    }

    fn load_from(path: &Path) -> Result<Config> {
        loader::load_from(path)
    }

    fn save(&self) -> Result<()> {
        saver::save(self)
    }

    fn save_to(&self, path: &Path) -> Result<()> {
        saver::save_to(self, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_returns_valid_config() {
        let config = default();
        assert_eq!(config.version, 1);
        assert!(config.ui.show_clock);
    }

    #[test]
    fn config_load_and_save() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");

        // Save a config
        let config = Config::default();
        config.save_to(&path).unwrap();

        // Load it back
        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(config, loaded);
    }

    #[test]
    fn config_trait_methods() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");

        // Use trait methods
        let config = Config::default();
        ConfigOps::save_to(&config, &path).unwrap();

        let loaded = <Config as ConfigOps>::load_from(&path).unwrap();
        assert_eq!(config, loaded);
    }
}
