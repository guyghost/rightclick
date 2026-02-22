//! Data access layer for RightClick.
//!
//! This module defines repository traits and implementations for persisting and
//! retrieving application data. Repositories abstract the storage mechanism
//! (filesystem, database, etc.) from the use cases.
//!
//! # Design Principles
//!
//! 1. **No business logic** - Repositories only handle data access
//! 2. **Async operations** - All I/O is async for responsiveness
//! 3. **Error handling** - Use `anyhow::Result` for flexible error propagation
//! 4. **Trait-based** - Traits allow for mocking in tests

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tracing::{debug, trace};

use crate::core::models::config::Config;
use crate::state::State;

/// Trait for configuration repository operations.
///
/// This trait abstracts the storage and retrieval of application configuration.
/// Implementations handle the details of where and how the config is stored.
///
/// # Implementations
///
/// - [`FileConfigRepository`] - Stores config as JSON on the filesystem
///
/// # Example
///
/// ```rust,ignore
/// use rightclick::shell::repositories::{ConfigRepository, FileConfigRepository};
///
/// let repo = FileConfigRepository::new("~/.config/rightclick/config.json");
/// let config = repo.load().await?;
/// ```
#[async_trait]
pub trait ConfigRepository: Send + Sync {
    /// Load the configuration from storage.
    ///
    /// Returns the loaded [`Config`]. If no configuration exists, a default
    /// configuration should be returned.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration exists but cannot be read or parsed.
    async fn load(&self) -> Result<Config>;

    /// Save the configuration to storage.
    ///
    /// Persists the given [`Config`] to the underlying storage.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration cannot be written.
    async fn save(&self, config: &Config) -> Result<()>;
}

/// Trait for state repository operations.
///
/// This trait abstracts the storage and retrieval of application state.
/// State is separate from config as it changes during runtime and is
/// persisted across sessions.
///
/// # Implementations
///
/// - [`FileStateRepository`] - Stores state as JSON on the filesystem
///
/// # Example
///
/// ```rust,ignore
/// use rightclick::shell::repositories::{StateRepository, FileStateRepository};
///
/// let repo = FileStateRepository::new("~/.config/rightclick/state.json");
/// let state = repo.load().await?;
/// ```
#[async_trait]
pub trait StateRepository: Send + Sync {
    /// Load the state from storage.
    ///
    /// Returns the loaded [`State`]. If no state exists, a default state
    /// should be returned.
    ///
    /// # Errors
    ///
    /// Returns an error if the state exists but cannot be read or parsed.
    async fn load(&self) -> Result<State>;

    /// Save the state to storage.
    ///
    /// Persists the given [`State`] to the underlying storage.
    ///
    /// # Errors
    ///
    /// Returns an error if the state cannot be written.
    async fn save(&self, state: &State) -> Result<()>;
}

/// File-based implementation of [`ConfigRepository`].
///
/// Stores configuration as a JSON file on the filesystem. The file path
/// is determined at construction time.
///
/// # Example
///
/// ```rust,ignore
/// use rightclick::shell::repositories::FileConfigRepository;
/// use std::path::PathBuf;
///
/// let repo = FileConfigRepository::new(PathBuf::from("/etc/rightclick/config.json"));
/// ```
#[derive(Debug, Clone)]
pub struct FileConfigRepository {
    path: PathBuf,
}

impl FileConfigRepository {
    /// Create a new file-based config repository.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the configuration file
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Get the config file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[async_trait]
impl ConfigRepository for FileConfigRepository {
    async fn load(&self) -> Result<Config> {
        trace!(path = %self.path.display(), "Loading config");

        if !self.path.exists() {
            debug!(path = %self.path.display(), "Config file not found, returning default");
            return Ok(Config::default());
        }

        let content = tokio::fs::read_to_string(&self.path)
            .await
            .with_context(|| format!("Failed to read config from {}", self.path.display()))?;

        let config: Config = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config from {}", self.path.display()))?;

        debug!(path = %self.path.display(), "Config loaded successfully");
        Ok(config)
    }

    async fn save(&self, config: &Config) -> Result<()> {
        trace!(path = %self.path.display(), "Saving config");

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }

        let content = serde_json::to_string_pretty(config).context("Failed to serialize config")?;

        tokio::fs::write(&self.path, content)
            .await
            .with_context(|| format!("Failed to write config to {}", self.path.display()))?;

        debug!(path = %self.path.display(), "Config saved successfully");
        Ok(())
    }
}

/// File-based implementation of [`StateRepository`].
///
/// Stores state as a JSON file on the filesystem. The file path is determined
/// at construction time.
///
/// # Example
///
/// ```rust,ignore
/// use rightclick::shell::repositories::FileStateRepository;
/// use std::path::PathBuf;
///
/// let repo = FileStateRepository::new(PathBuf::from("~/.config/rightclick/state.json"));
/// ```
#[derive(Debug, Clone)]
pub struct FileStateRepository {
    path: PathBuf,
}

impl FileStateRepository {
    /// Create a new file-based state repository.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the state file
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Get the state file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[async_trait]
impl StateRepository for FileStateRepository {
    async fn load(&self) -> Result<State> {
        trace!(path = %self.path.display(), "Loading state");

        if !self.path.exists() {
            debug!(path = %self.path.display(), "State file not found, returning default");
            return Ok(State::default());
        }

        let content = tokio::fs::read_to_string(&self.path)
            .await
            .with_context(|| format!("Failed to read state from {}", self.path.display()))?;

        let state: State = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse state from {}", self.path.display()))?;

        debug!(path = %self.path.display(), "State loaded successfully");
        Ok(state)
    }

    async fn save(&self, state: &State) -> Result<()> {
        trace!(path = %self.path.display(), "Saving state");

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }

        let content = serde_json::to_string_pretty(state).context("Failed to serialize state")?;

        tokio::fs::write(&self.path, content)
            .await
            .with_context(|| format!("Failed to write state to {}", self.path.display()))?;

        debug!(path = %self.path.display(), "State saved successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_config_repository_load_default() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let repo = FileConfigRepository::new(&config_path);
        let config = repo.load().await.unwrap();

        // Should return default config when file doesn't exist
        assert_eq!(
            config.version,
            crate::core::models::config::CURRENT_CONFIG_VERSION
        );
    }

    #[tokio::test]
    async fn test_file_config_repository_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let repo = FileConfigRepository::new(&config_path);

        // Save a config
        let mut config = Config::default();
        config.ui.show_clock = false;
        repo.save(&config).await.unwrap();

        // Load it back
        let loaded = repo.load().await.unwrap();
        assert!(!loaded.ui.show_clock);
    }

    #[tokio::test]
    async fn test_file_state_repository_load_default() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");

        let repo = FileStateRepository::new(&state_path);
        let state = repo.load().await.unwrap();

        // Should return default state when file doesn't exist
        assert_eq!(state.version, crate::state::types::STATE_VERSION);
    }

    #[tokio::test]
    async fn test_file_state_repository_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");

        let repo = FileStateRepository::new(&state_path);

        // Save a state
        let mut state = State::default();
        state.git_graph_enabled = false;
        repo.save(&state).await.unwrap();

        // Load it back
        let loaded = repo.load().await.unwrap();
        assert!(!loaded.git_graph_enabled);
    }
}
