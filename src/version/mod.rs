//! Version checking and update notifications.
//!
//! This module provides functionality for checking the current application
//! version against the latest release on GitHub. It includes caching to
//! avoid unnecessary network requests.
//!
//! # Example
//!
//! ```no_run
//! use rightclick::version::{get_version_info, update_available};
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Check if an update is available
//! if update_available().await? {
//!     println!("A new version is available!");
//! }
//!
//! // Get full version information
//! let info = get_version_info().await?;
//! println!("Current version: {}", info.current);
//! if let Some(latest) = &info.latest {
//!     println!("Latest version: {}", latest);
//! }
//! # Ok(())
//! # }
//! ```

pub mod cache;
pub mod checker;

// Re-export commonly used types and functions
pub use cache::VersionCache;
pub use checker::{VersionInfo, check_latest, current_version, get_version_info, update_available};

use anyhow::Result;

/// Loads the version cache from disk.
///
/// This is a convenience wrapper around [`cache::load`].
///
/// # Example
///
/// ```no_run
/// use rightclick::version::load_cache;
///
/// match load_cache() {
///     Ok(Some(cache)) => println!("Cached version: {}", cache.latest_version),
///     Ok(None) => println!("No valid cache"),
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
pub fn load_cache() -> Result<Option<VersionCache>> {
    cache::load()
}

/// Saves the version cache to disk.
///
/// This is a convenience wrapper around [`cache::save`].
///
/// # Example
///
/// ```no_run
/// use rightclick::version::{VersionCache, save_cache};
/// use chrono::Utc;
///
/// let cache = VersionCache::new("v0.2.0".to_string(), Utc::now());
/// save_cache(&cache).expect("Failed to save cache");
/// ```
pub fn save_cache(cache: &VersionCache) -> Result<()> {
    cache::save(cache)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn version_cache_creation() {
        let cache = VersionCache::new("v1.0.0".to_string(), Utc::now());
        assert_eq!(cache.latest_version, "v1.0.0");
    }

    #[test]
    fn version_info_creation() {
        let current = current_version();
        assert!(!current.is_empty());
    }
}
