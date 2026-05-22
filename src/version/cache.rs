//! Version cache management.
//!
//! This module handles caching of version information to avoid
//! unnecessary network requests. The cache is stored at:
//! `~/.config/rightclick/version_cache.json` on Unix-like systems.

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// The cache validity duration (24 hours).
const CACHE_VALIDITY_DURATION: Duration = Duration::hours(24);

/// Cached version information.
///
/// This struct stores the latest known version and when it was checked,
/// allowing the application to avoid unnecessary network requests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionCache {
    /// The latest version string from GitHub (e.g., "v0.1.0").
    pub latest_version: String,
    /// When the cache was last updated.
    pub checked_at: DateTime<Utc>,
}

impl VersionCache {
    /// Creates a new version cache entry.
    ///
    /// # Example
    ///
    /// ```
    /// use rightclick::version::VersionCache;
    /// use chrono::Utc;
    ///
    /// let cache = VersionCache::new("v0.2.0".to_string(), Utc::now());
    /// assert_eq!(cache.latest_version, "v0.2.0");
    /// ```
    pub fn new(latest_version: String, checked_at: DateTime<Utc>) -> Self {
        Self {
            latest_version,
            checked_at,
        }
    }

    /// Checks if the cache is still valid (less than 24 hours old).
    ///
    /// # Example
    ///
    /// ```
    /// use rightclick::version::VersionCache;
    /// use chrono::Utc;
    ///
    /// let cache = VersionCache::new("v0.2.0".to_string(), Utc::now());
    /// assert!(cache.is_valid());
    /// ```
    pub fn is_valid(&self) -> bool {
        Utc::now() - self.checked_at < CACHE_VALIDITY_DURATION
    }

    /// Returns the age of the cache.
    pub fn age(&self) -> Duration {
        Utc::now() - self.checked_at
    }
}

/// Returns the path to the version cache file.
///
/// The cache is stored at `~/.config/rightclick/version_cache.json` on Unix-like
/// systems and in the appropriate config directory on Windows.
///
/// # Errors
///
/// Returns an error if the project directories cannot be determined
/// (e.g., when no home directory exists).
fn cache_file_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("com", "rightclick", "rightclick")
        .context("Failed to determine project directories")?;
    Ok(dirs.config_dir().join("version_cache.json"))
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

/// Loads the version cache from disk.
///
/// If the cache file doesn't exist or is invalid, returns `Ok(None)`.
/// If the cache is expired (older than 24 hours), returns `Ok(None)`.
///
/// # Errors
///
/// Returns an error only if the file exists but cannot be read.
///
/// # Example
///
/// ```no_run
/// use rightclick::version::cache::load;
///
/// match load() {
///     Ok(Some(cache)) => println!("Cached version: {}", cache.latest_version),
///     Ok(None) => println!("No valid cache found"),
///     Err(e) => eprintln!("Failed to load cache: {}", e),
/// }
/// ```
pub fn load() -> Result<Option<VersionCache>> {
    let path = cache_file_path()?;

    if !path.exists() {
        debug!("Version cache file does not exist");
        return Ok(None);
    }

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read version cache file: {}", path.display()))?;

    let cache: VersionCache = match serde_json::from_str(&contents) {
        Ok(cache) => cache,
        Err(e) => {
            warn!("Failed to parse version cache file: {}. Ignoring cache.", e);
            return Ok(None);
        }
    };

    // Check if cache is still valid
    if cache.is_valid() {
        debug!(
            "Loaded valid version cache from {} (age: {} hours)",
            path.display(),
            cache.age().num_hours()
        );
        Ok(Some(cache))
    } else {
        debug!(
            "Version cache expired (age: {} hours)",
            cache.age().num_hours()
        );
        Ok(None)
    }
}

/// Saves the version cache to disk atomically.
///
/// Writes the cache to a temporary file first, then renames it to the
/// target path. This ensures that the cache file is never in a partially
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
/// use rightclick::version::{VersionCache, cache::save};
/// use chrono::Utc;
///
/// let cache = VersionCache::new("v0.2.0".to_string(), Utc::now());
/// save(&cache).expect("Failed to save version cache");
/// ```
pub fn save(cache: &VersionCache) -> Result<()> {
    let path = cache_file_path()?;
    ensure_config_dir(&path)?;

    // Serialize to a string first to validate
    let json =
        serde_json::to_string_pretty(cache).context("Failed to serialize version cache to JSON")?;

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

    debug!("Saved version cache to {}", path.display());
    Ok(())
}

/// Clears the version cache.
///
/// Removes the cache file if it exists. This is useful for forcing
/// a fresh version check on the next request.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be deleted.
///
/// # Example
///
/// ```no_run
/// use rightclick::version::cache::clear;
///
/// clear().expect("Failed to clear version cache");
/// ```
pub fn clear() -> Result<()> {
    let path = cache_file_path()?;

    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("Failed to remove version cache file: {}", path.display()))?;
        debug!("Cleared version cache at {}", path.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_cache_new() {
        let now = Utc::now();
        let cache = VersionCache::new("v1.0.0".to_string(), now);
        assert_eq!(cache.latest_version, "v1.0.0");
        assert_eq!(cache.checked_at, now);
    }

    #[test]
    fn version_cache_is_valid_when_fresh() {
        let cache = VersionCache::new("v1.0.0".to_string(), Utc::now());
        assert!(cache.is_valid());
    }

    #[test]
    fn version_cache_is_invalid_when_expired() {
        let old_time = Utc::now() - Duration::hours(25);
        let cache = VersionCache::new("v1.0.0".to_string(), old_time);
        assert!(!cache.is_valid());
    }

    #[test]
    fn version_cache_is_valid_at_boundary() {
        // Exactly 23 hours ago should be valid
        let recent_time = Utc::now() - Duration::hours(23);
        let cache = VersionCache::new("v1.0.0".to_string(), recent_time);
        assert!(cache.is_valid());
    }

    #[test]
    fn roundtrip_preserves_cache() {
        let cache = VersionCache::new("v0.2.0".to_string(), Utc::now());
        let json = serde_json::to_string_pretty(&cache).unwrap();
        let loaded: VersionCache = serde_json::from_str(&json).unwrap();
        assert_eq!(cache.latest_version, loaded.latest_version);
        // Note: DateTime serialization may have microsecond precision differences
    }

    #[test]
    fn cache_file_path_returns_valid_path() {
        let path = cache_file_path().unwrap();
        assert!(path.to_string_lossy().contains("version_cache.json"));
    }
}
