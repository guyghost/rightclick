//! Version checking logic.
//!
//! This module provides functionality for checking the current application
//! version and comparing it against the latest release on GitHub.

use crate::version::cache::{self, VersionCache};
use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use std::env;
use tracing::{debug, info};

#[allow(dead_code)]
/// The GitHub API URL for the latest release.
const GITHUB_API_URL: &str = "https://api.github.com/repos/guyghost/rightclick/releases/latest";

#[allow(dead_code)]
/// The application name for the User-Agent header.
const APP_NAME: &str = "rightclick";

#[allow(dead_code)]
/// Response structure from GitHub's releases API.
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    /// The tag name of the release (e.g., "v0.1.0").
    tag_name: String,
    /// The release name/title.
    #[allow(dead_code)]
    name: Option<String>,
    /// Whether this is a draft release.
    #[allow(dead_code)]
    draft: bool,
    /// Whether this is a prerelease.
    #[allow(dead_code)]
    prerelease: bool,
}

/// Information about the current and latest versions.
///
/// This struct provides a complete picture of version status,
/// including whether an update is available.
#[derive(Debug, Clone, PartialEq)]
pub struct VersionInfo {
    /// The current version of the application (e.g., "0.1.0").
    pub current: String,
    /// The latest version available on GitHub, if known.
    pub latest: Option<String>,
    /// Whether an update is available (latest > current).
    pub update_available: bool,
    /// When the version was last checked, if ever.
    pub checked_at: Option<chrono::DateTime<Utc>>,
}

impl VersionInfo {
    /// Creates a new VersionInfo with the current version only.
    fn new(current: String) -> Self {
        Self {
            current,
            latest: None,
            update_available: false,
            checked_at: None,
        }
    }

    /// Creates a VersionInfo from a cache entry.
    fn from_cache(current: String, cache: &VersionCache) -> Self {
        let update_available = is_newer(&cache.latest_version, &current);
        Self {
            current,
            latest: Some(cache.latest_version.clone()),
            update_available,
            checked_at: Some(cache.checked_at),
        }
    }

    #[allow(dead_code)]
    /// Creates a VersionInfo from a fetched latest version.
    fn from_fetched(current: String, latest: String, checked_at: chrono::DateTime<Utc>) -> Self {
        let update_available = is_newer(&latest, &current);
        Self {
            current,
            latest: Some(latest),
            update_available,
            checked_at: Some(checked_at),
        }
    }
}

/// Returns the current application version.
///
/// This is set at compile time from the Cargo.toml version field.
///
/// # Example
///
/// ```
/// use rightclick::version::current_version;
///
/// let version = current_version();
/// assert!(!version.is_empty());
/// ```
pub fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Checks the latest version from GitHub releases.
///
/// Makes an HTTP request to the GitHub API to fetch the latest release
/// information. Returns `None` if the request fails or the response
/// cannot be parsed.
///
/// This function requires the `reqwest` feature to be enabled.
///
/// # Errors
///
/// Returns an error if:
/// - The HTTP request fails
/// - The response cannot be parsed as JSON
/// - The network is unreachable
///
/// # Example
///
/// ```no_run
/// use rightclick::version::check_latest;
///
/// # async fn example() -> anyhow::Result<()> {
/// match check_latest().await? {
///     Some(version) => println!("Latest version: {}", version),
///     None => println!("Could not determine latest version"),
/// }
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "reqwest")]
pub async fn check_latest() -> Result<Option<String>> {
    debug!("Checking latest version from GitHub: {}", GITHUB_API_URL);

    let client = reqwest::Client::builder()
        .user_agent(format!(
            "{}/{} (github.com/guyghost/rightclick)",
            APP_NAME,
            current_version()
        ))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("Failed to build HTTP client")?;

    let response = client
        .get(GITHUB_API_URL)
        .send()
        .await
        .context("Failed to fetch latest release from GitHub")?;

    if !response.status().is_success() {
        let status = response.status();
        warn!("GitHub API returned error status: {}", status);
        anyhow::bail!("GitHub API returned error: {}", status);
    }

    let release: GitHubRelease = response
        .json()
        .await
        .context("Failed to parse GitHub release response")?;

    debug!("Latest version from GitHub: {}", release.tag_name);

    Ok(Some(release.tag_name))
}

/// Stub implementation when reqwest is not available.
#[cfg(not(feature = "reqwest"))]
pub async fn check_latest() -> Result<Option<String>> {
    debug!("Version checking disabled (reqwest feature not enabled)");
    Ok(None)
}

/// Checks if an update is available.
///
/// Compares the current version against the latest version from GitHub.
/// Returns `true` if a newer version is available.
///
/// This function respects the cache and will only make a network request
/// if the cache is expired or missing.
///
/// # Errors
///
/// Returns an error if the version check fails and no valid cache exists.
///
/// # Example
///
/// ```no_run
/// use rightclick::version::update_available;
///
/// # async fn example() -> anyhow::Result<()> {
/// if update_available().await? {
///     println!("An update is available!");
/// }
/// # Ok(())
/// # }
/// ```
pub async fn update_available() -> Result<bool> {
    let info = get_version_info().await?;
    Ok(info.update_available)
}

/// Gets full version information with caching.
///
/// This function returns comprehensive version information, including:
/// - Current version (from compile-time)
/// - Latest version (from cache or GitHub API)
/// - Whether an update is available
/// - When the version was last checked
///
/// The function uses caching to avoid unnecessary network requests.
/// The cache is valid for 24 hours.
///
/// # Errors
///
/// Returns an error if:
/// - The cache cannot be read or written
/// - The network request fails and no valid cache exists
///
/// # Example
///
/// ```no_run
/// use rightclick::version::get_version_info;
///
/// # async fn example() -> anyhow::Result<()> {
/// let info = get_version_info().await?;
/// println!("Current: {}", info.current);
/// if let Some(latest) = info.latest {
///     println!("Latest: {}", latest);
/// }
/// if info.update_available {
///     println!("Update available!");
/// }
/// # Ok(())
/// # }
/// ```
pub async fn get_version_info() -> Result<VersionInfo> {
    let current = current_version();

    // Try to load from cache first
    match cache::load()? {
        Some(cached) => {
            info!(
                "Using cached version info (cached {} hours ago)",
                cached.age().num_hours()
            );
            Ok(VersionInfo::from_cache(current, &cached))
        }
        None => {
            // No valid cache, fetch from GitHub
            debug!("No valid cache, fetching from GitHub");

            #[cfg(feature = "reqwest")]
            {
                match check_latest().await {
                    Ok(Some(latest)) => {
                        let checked_at = Utc::now();
                        let cache = VersionCache::new(latest.clone(), checked_at);

                        // Save to cache (best effort)
                        if let Err(e) = cache::save(&cache) {
                            warn!("Failed to save version cache: {}", e);
                        }

                        Ok(VersionInfo::from_fetched(current, latest, checked_at))
                    }
                    Ok(None) => {
                        // No version info available
                        debug!("Could not determine latest version");
                        Ok(VersionInfo::new(current))
                    }
                    Err(e) => {
                        warn!("Failed to check latest version: {}", e);
                        // Return current info without latest
                        Ok(VersionInfo::new(current))
                    }
                }
            }

            #[cfg(not(feature = "reqwest"))]
            {
                Ok(VersionInfo::new(current))
            }
        }
    }
}

/// Compares two version strings.
///
/// Returns true if `candidate` is newer than `current`.
/// Handles semver-style versions (e.g., "v0.1.0" vs "0.1.0").
///
/// This is a simple implementation that strips 'v' prefixes and
/// compares version components numerically.
fn is_newer(candidate: &str, current: &str) -> bool {
    let candidate = normalize_version(candidate);
    let current = normalize_version(current);

    // Parse version components
    let candidate_parts: Vec<u32> = candidate
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    let current_parts: Vec<u32> = current.split('.').filter_map(|s| s.parse().ok()).collect();

    // Compare component by component
    let max_len = candidate_parts.len().max(current_parts.len());
    for i in 0..max_len {
        let c = candidate_parts.get(i).copied().unwrap_or(0);
        let cur = current_parts.get(i).copied().unwrap_or(0);

        if c > cur {
            return true;
        }
        if c < cur {
            return false;
        }
    }

    // Versions are equal
    false
}

/// Normalizes a version string by removing the 'v' prefix.
fn normalize_version(version: &str) -> &str {
    version.strip_prefix('v').unwrap_or(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_returns_non_empty() {
        let version = current_version();
        assert!(!version.is_empty());
        // Should match the pattern of a version string
        assert!(version.contains('.') || version.chars().next().unwrap().is_ascii_digit());
    }

    #[test]
    fn version_info_new() {
        let info = VersionInfo::new("0.1.0".to_string());
        assert_eq!(info.current, "0.1.0");
        assert!(info.latest.is_none());
        assert!(!info.update_available);
        assert!(info.checked_at.is_none());
    }

    #[test]
    fn version_info_from_cache() {
        let cache = VersionCache::new("v0.2.0".to_string(), Utc::now());
        let info = VersionInfo::from_cache("0.1.0".to_string(), &cache);
        assert_eq!(info.current, "0.1.0");
        assert_eq!(info.latest, Some("v0.2.0".to_string()));
        assert!(info.update_available);
        assert!(info.checked_at.is_some());
    }

    #[test]
    fn version_info_from_fetched() {
        let checked_at = Utc::now();
        let info = VersionInfo::from_fetched("0.1.0".to_string(), "v0.2.0".to_string(), checked_at);
        assert_eq!(info.current, "0.1.0");
        assert_eq!(info.latest, Some("v0.2.0".to_string()));
        assert!(info.update_available);
        assert_eq!(info.checked_at, Some(checked_at));
    }

    #[test]
    fn is_newer_basic() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
    }

    #[test]
    fn is_newer_with_v_prefix() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("0.2.0", "v0.1.0"));
        assert!(is_newer("v0.2.0", "v0.1.0"));
    }

    #[test]
    fn is_newer_patch_versions() {
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.1"));
    }

    #[test]
    fn is_newer_major_versions() {
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.9.9", "1.0.0"));
    }

    #[test]
    fn is_newer_different_length() {
        assert!(is_newer("0.1.0.1", "0.1.0"));
        assert!(!is_newer("0.1", "0.1.0"));
    }

    #[test]
    fn normalize_version_strips_v() {
        assert_eq!(normalize_version("v0.1.0"), "0.1.0");
        assert_eq!(normalize_version("0.1.0"), "0.1.0");
    }

    #[test]
    fn normalize_version_handles_multiple_v() {
        // Only strips leading v
        assert_eq!(normalize_version("v0.1.0-beta"), "0.1.0-beta");
    }
}
