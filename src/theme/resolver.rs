//! Theme resolution logic
//!
//! This module handles theme resolution from configuration, project-specific
//! overrides, and environment variables.

use crate::core::models::{Config, Theme, ThemeError};
use crate::theme::builtin::{all_themes, default_theme};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::Path;

/// Global storage for the currently applied theme
///
/// This is stored in a RwLock to allow safe concurrent access and modification.
/// Use `apply_theme()` to set the current theme and `get_current_theme()` to read it.
pub static CURRENT_THEME: Lazy<RwLock<Option<Theme>>> = Lazy::new(|| RwLock::new(None));

/// A resolved theme with metadata about how it was resolved
///
/// This struct contains both the theme itself and information about
/// how it was resolved (source, overrides applied, etc.).
#[derive(Clone, Debug)]
pub struct ResolvedTheme {
    /// The resolved theme
    pub theme: Theme,
    /// The source of the theme resolution
    pub source: ThemeSource,
    /// Any overrides that were applied
    pub overrides: HashMap<String, String>,
    /// The project path that influenced the resolution (if any)
    pub project_path: Option<std::path::PathBuf>,
}

/// Source of theme resolution
#[derive(Clone, Debug, PartialEq)]
pub enum ThemeSource {
    /// Theme came from built-in themes
    BuiltIn,
    /// Theme came from user configuration
    Config,
    /// Theme came from project-specific configuration
    Project,
    /// Theme came from environment variable
    Environment,
    /// Theme is the default fallback
    Default,
}

/// Resolve a theme based on configuration and project path
///
/// This function resolves the theme in the following priority order:
/// 1. Environment variable (`RIGHTCLICK_THEME`)
/// 2. Project-specific configuration (if project_path is provided)
/// 3. User configuration
/// 4. Default theme
///
/// # Arguments
///
/// * `config` - The global configuration
/// * `project_path` - Optional project path for project-specific overrides
///
/// # Example
///
/// ```rust
/// use rightclick::theme::resolve_theme;
/// use rightclick::core::models::Config;
///
/// let config = Config::default();
/// let resolved = resolve_theme(&config, None);
///
/// println!("Using theme: {} from {:?}", resolved.theme.name, resolved.source);
/// ```
pub fn resolve_theme(config: &Config, project_path: Option<&Path>) -> ResolvedTheme {
    // Priority 1: Environment variable
    if let Ok(env_theme) = std::env::var("RIGHTCLICK_THEME") {
        if let Some(theme) = find_theme(&env_theme) {
            return ResolvedTheme {
                theme,
                source: ThemeSource::Environment,
                overrides: HashMap::new(),
                project_path: project_path.map(|p| p.to_path_buf()),
            };
        }
    }

    // Priority 2: Project-specific configuration (if available)
    if let Some(path) = project_path {
        if let Some(project_theme) = resolve_project_theme(path) {
            return ResolvedTheme {
                theme: project_theme,
                source: ThemeSource::Project,
                overrides: HashMap::new(),
                project_path: Some(path.to_path_buf()),
            };
        }
    }

    // Priority 3: User configuration
    let config_theme_name = &config.ui.theme;
    if let Some(theme) = find_theme(config_theme_name) {
        return ResolvedTheme {
            theme,
            source: ThemeSource::Config,
            overrides: HashMap::new(),
            project_path: project_path.map(|p| p.to_path_buf()),
        };
    }

    // Priority 4: Default theme
    ResolvedTheme {
        theme: default_theme(),
        source: ThemeSource::Default,
        overrides: HashMap::new(),
        project_path: project_path.map(|p| p.to_path_buf()),
    }
}

/// Find a theme by name
///
/// Searches all built-in themes for a matching name (case-insensitive).
fn find_theme(name: &str) -> Option<Theme> {
    let name_lower = name.to_lowercase();
    all_themes().into_iter().find(|t| {
        t.name.to_lowercase() == name_lower || t.display_name.to_lowercase() == name_lower
    })
}

/// Resolve a theme from project-specific configuration
///
/// Currently, this looks for a `.rightclick/theme.toml` or `.rightclick/theme.json`
/// file in the project directory. Returns None if no project theme is found.
///
/// # Arguments
///
/// * `project_path` - Path to the project root directory
fn resolve_project_theme(project_path: &Path) -> Option<Theme> {
    let rightclick_dir = project_path.join(".rightclick");

    // Try TOML format first
    let toml_path = rightclick_dir.join("theme.toml");
    if toml_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&toml_path) {
            if let Ok(theme) = parse_theme_from_toml(&content) {
                return Some(theme);
            }
        }
    }

    // Try JSON format
    let json_path = rightclick_dir.join("theme.json");
    if json_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&json_path) {
            if let Ok(theme) = serde_json::from_str::<Theme>(&content) {
                return Some(theme);
            }
        }
    }

    None
}

/// Parse a theme from TOML content
///
/// This is a simplified parser that expects a TOML file with a `name` field
/// referencing a built-in theme, or a complete theme definition.
fn parse_theme_from_toml(content: &str) -> Result<Theme, ThemeError> {
    // First, try to parse as a theme reference (just a name)
    #[derive(serde::Deserialize)]
    struct ThemeRef {
        name: String,
    }

    if let Ok(theme_ref) = toml::from_str::<ThemeRef>(content) {
        if let Some(theme) = find_theme(&theme_ref.name) {
            return Ok(theme);
        }
    }

    // Try to parse as a complete theme
    toml::from_str::<Theme>(content).map_err(|e| ThemeError::ParseError(e.to_string()))
}

/// Get the theme that would be used as default
///
/// This is useful for previewing or resetting to the default theme.
///
/// # Example
///
/// ```rust
/// use rightclick::theme::get_default_theme;
///
/// let theme = get_default_theme();
/// assert_eq!(theme.name, "default");
/// ```
#[allow(dead_code)]
pub fn get_default_theme() -> Theme {
    default_theme()
}

/// Resolve a theme with custom overrides
///
/// Similar to `resolve_theme`, but applies color overrides from the provided map.
/// This is useful for applying user-specific color customizations.
///
/// # Arguments
///
/// * `config` - The global configuration
/// * `project_path` - Optional project path for project-specific overrides
/// * `overrides` - Map of color field names to hex color values
///
/// # Example
///
/// ```rust
/// use rightclick::theme::resolve_theme_with_overrides;
/// use rightclick::core::models::Config;
/// use std::collections::HashMap;
///
/// let config = Config::default();
/// let mut overrides = HashMap::new();
/// overrides.insert("primary".to_string(), "#ff0000".to_string());
///
/// let resolved = resolve_theme_with_overrides(&config, None, overrides);
/// assert_eq!(resolved.overrides.get("primary"), Some(&"#ff0000".to_string()));
/// ```
#[allow(dead_code)]
pub fn resolve_theme_with_overrides(
    config: &Config,
    project_path: Option<&Path>,
    overrides: HashMap<String, String>,
) -> ResolvedTheme {
    let mut resolved = resolve_theme(config, project_path);

    // Apply overrides to the theme
    for (key, value) in &overrides {
        match key.as_str() {
            "primary" => resolved.theme.colors.primary = value.clone(),
            "secondary" => resolved.theme.colors.secondary = value.clone(),
            "success" => resolved.theme.colors.success = value.clone(),
            "warning" => resolved.theme.colors.warning = value.clone(),
            "error" => resolved.theme.colors.error = value.clone(),
            "info" => resolved.theme.colors.info = value.clone(),
            "background" => resolved.theme.colors.background = value.clone(),
            "foreground" => resolved.theme.colors.foreground = value.clone(),
            "muted" => resolved.theme.colors.muted = value.clone(),
            "border" => resolved.theme.colors.border = value.clone(),
            "highlight" => resolved.theme.colors.highlight = value.clone(),
            "cursor" => resolved.theme.colors.cursor = value.clone(),
            "added" => resolved.theme.colors.added = value.clone(),
            "removed" => resolved.theme.colors.removed = value.clone(),
            "modified" => resolved.theme.colors.modified = value.clone(),
            "untracked" => resolved.theme.colors.untracked = value.clone(),
            _ => {}
        }
    }

    resolved.overrides.extend(overrides);
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_theme_default() {
        let config = Config::default();
        let resolved = resolve_theme(&config, None);

        assert_eq!(resolved.theme.name, "default");
        assert_eq!(resolved.source, ThemeSource::Default);
    }

    #[test]
    fn test_find_theme_case_insensitive() {
        assert!(find_theme("Dracula").is_some());
        assert!(find_theme("DRACULA").is_some());
        assert!(find_theme("dracula").is_some());
        assert!(find_theme("Tokyo Night").is_some());
        assert!(find_theme("tokyo-night").is_some());
    }

    #[test]
    fn test_resolve_theme_from_config() {
        let mut config = Config::default();
        config.ui.theme = "dracula".to_string();

        let resolved = resolve_theme(&config, None);
        assert_eq!(resolved.theme.name, "dracula");
        assert_eq!(resolved.source, ThemeSource::Config);
    }

    #[test]
    fn test_get_default_theme() {
        let theme = get_default_theme();
        assert_eq!(theme.name, "default");
    }

    #[test]
    fn test_resolve_theme_with_overrides() {
        let config = Config::default();
        let mut overrides = HashMap::new();
        overrides.insert("primary".to_string(), "#ff0000".to_string());

        let resolved = resolve_theme_with_overrides(&config, None, overrides);

        assert_eq!(resolved.theme.colors.primary, "#ff0000");
        assert!(resolved.overrides.contains_key("primary"));
    }

    #[test]
    fn test_resolved_theme_debug() {
        let config = Config::default();
        let resolved = resolve_theme(&config, None);

        let debug_str = format!("{:?}", resolved);
        assert!(debug_str.contains("theme"));
        assert!(debug_str.contains("source"));
    }

    #[test]
    fn test_theme_source_equality() {
        assert_eq!(ThemeSource::BuiltIn, ThemeSource::BuiltIn);
        assert_ne!(ThemeSource::BuiltIn, ThemeSource::Default);
    }
}
