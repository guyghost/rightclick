//! Theme resolution logic
//!
//! This module handles theme resolution from configuration, project-specific
//! overrides, and environment variables. It also supports loading custom themes
//! from `~/.config/rightclick/themes/*.toml`.

use crate::core::models::{ColorPalette, Config, Theme, ThemeError};
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

/// A partial color palette for theme overrides (all fields optional)
///
/// When loading a custom theme file, users only need to specify the colors
/// they want to override. Any unspecified colors will be inherited from the
/// base theme (if `extends` is used) or from the default theme.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PartialColorPalette {
    pub primary: Option<String>,
    pub secondary: Option<String>,
    pub success: Option<String>,
    pub warning: Option<String>,
    pub error: Option<String>,
    pub info: Option<String>,
    pub background: Option<String>,
    pub foreground: Option<String>,
    pub muted: Option<String>,
    pub border: Option<String>,
    pub highlight: Option<String>,
    pub cursor: Option<String>,
    pub added: Option<String>,
    pub removed: Option<String>,
    pub modified: Option<String>,
    pub untracked: Option<String>,
}

/// TOML format for custom theme files
///
/// Custom theme files are placed in `~/.config/rightclick/themes/` and use
/// the `.toml` extension. A minimal theme file needs only a `name` field.
/// Use `extends` to base a custom theme on a built-in one and override
/// specific colors.
///
/// # Example TOML
///
/// ```toml
/// name = "my-theme"
/// display_name = "My Custom Theme"
/// extends = "dracula"
///
/// [colors]
/// primary = "#ff0000"
/// background = "#000000"
/// ```
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CustomThemeFile {
    /// Unique identifier for the theme
    pub name: String,
    /// Human-readable display name (defaults to `name` if not specified)
    #[serde(default)]
    pub display_name: Option<String>,
    /// Optional base theme to extend (must be a built-in theme name)
    pub extends: Option<String>,
    /// Color overrides (only specified colors will be changed)
    #[serde(default)]
    pub colors: PartialColorPalette,
}

/// Load custom themes from `~/.config/rightclick/themes/*.toml`
///
/// Scans the user's config directory for custom theme TOML files and
/// loads them. Invalid files are logged as warnings and skipped.
///
/// Returns an empty vector if the themes directory does not exist or
/// if no valid theme files are found.
pub fn load_custom_themes() -> Vec<Theme> {
    load_custom_themes_from_dir(None)
}

/// Load custom themes from a specific directory (or the default config dir if None)
///
/// This is the inner implementation that allows specifying a custom directory,
/// primarily for testing purposes.
fn load_custom_themes_from_dir(dir_override: Option<&Path>) -> Vec<Theme> {
    let themes_dir = match dir_override {
        Some(dir) => dir.to_path_buf(),
        None => {
            let config_dir = dirs::config_dir().map(|d| d.join("rightclick").join("themes"));
            let Some(dir) = config_dir else {
                return vec![];
            };
            dir
        }
    };

    if !themes_dir.exists() {
        return vec![];
    }

    let mut themes = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&themes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "toml").unwrap_or(false) {
                match load_custom_theme_file(&path) {
                    Ok(theme) => themes.push(theme),
                    Err(e) => {
                        tracing::warn!("Failed to load custom theme {:?}: {}", path, e);
                    }
                }
            }
        }
    }

    themes
}

/// Load and parse a single custom theme TOML file
fn load_custom_theme_file(path: &Path) -> Result<Theme, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    parse_custom_theme(&content)
}

/// Parse a custom theme from a TOML string
///
/// If the theme specifies `extends`, the named built-in theme is used as
/// the base. Otherwise, `Theme::default()` is used. Then any colors
/// specified in the `[colors]` section are merged over the base.
fn parse_custom_theme(content: &str) -> Result<Theme, Box<dyn std::error::Error>> {
    let custom: CustomThemeFile = toml::from_str(content)?;

    // Start with base theme if extends is specified
    let mut theme = if let Some(ref base_name) = custom.extends {
        all_themes()
            .into_iter()
            .find(|t| {
                t.name == *base_name || t.display_name.to_lowercase() == base_name.to_lowercase()
            })
            .unwrap_or_default()
    } else {
        Theme::default()
    };

    // Override name
    theme.name = custom.name.clone();
    theme.display_name = custom.display_name.unwrap_or(custom.name);

    // Merge partial colors over the base
    merge_partial_colors(&mut theme.colors, &custom.colors);

    Ok(theme)
}

/// Merge a partial color palette into a full color palette
///
/// Only fields that are `Some` in the partial palette will overwrite
/// the corresponding field in the target palette.
fn merge_partial_colors(target: &mut ColorPalette, partial: &PartialColorPalette) {
    if let Some(ref v) = partial.primary {
        target.primary = v.clone();
    }
    if let Some(ref v) = partial.secondary {
        target.secondary = v.clone();
    }
    if let Some(ref v) = partial.success {
        target.success = v.clone();
    }
    if let Some(ref v) = partial.warning {
        target.warning = v.clone();
    }
    if let Some(ref v) = partial.error {
        target.error = v.clone();
    }
    if let Some(ref v) = partial.info {
        target.info = v.clone();
    }
    if let Some(ref v) = partial.background {
        target.background = v.clone();
    }
    if let Some(ref v) = partial.foreground {
        target.foreground = v.clone();
    }
    if let Some(ref v) = partial.muted {
        target.muted = v.clone();
    }
    if let Some(ref v) = partial.border {
        target.border = v.clone();
    }
    if let Some(ref v) = partial.highlight {
        target.highlight = v.clone();
    }
    if let Some(ref v) = partial.cursor {
        target.cursor = v.clone();
    }
    if let Some(ref v) = partial.added {
        target.added = v.clone();
    }
    if let Some(ref v) = partial.removed {
        target.removed = v.clone();
    }
    if let Some(ref v) = partial.modified {
        target.modified = v.clone();
    }
    if let Some(ref v) = partial.untracked {
        target.untracked = v.clone();
    }
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

    // --- Custom theme tests ---

    #[test]
    fn test_parse_complete_custom_theme() {
        let toml_str = r##"
name = "my-complete"
display_name = "My Complete Theme"

[colors]
primary = "#ff0000"
secondary = "#00ff00"
success = "#0000ff"
warning = "#ffff00"
error = "#ff00ff"
info = "#00ffff"
background = "#111111"
foreground = "#eeeeee"
muted = "#888888"
border = "#444444"
highlight = "#555555"
cursor = "#ffffff"
added = "#aabbcc"
removed = "#ccbbaa"
modified = "#ddeeff"
untracked = "#ffeedd"
"##;

        let theme = parse_custom_theme(toml_str).unwrap();
        assert_eq!(theme.name, "my-complete");
        assert_eq!(theme.display_name, "My Complete Theme");
        assert_eq!(theme.colors.primary, "#ff0000");
        assert_eq!(theme.colors.secondary, "#00ff00");
        assert_eq!(theme.colors.success, "#0000ff");
        assert_eq!(theme.colors.warning, "#ffff00");
        assert_eq!(theme.colors.error, "#ff00ff");
        assert_eq!(theme.colors.info, "#00ffff");
        assert_eq!(theme.colors.background, "#111111");
        assert_eq!(theme.colors.foreground, "#eeeeee");
        assert_eq!(theme.colors.muted, "#888888");
        assert_eq!(theme.colors.border, "#444444");
        assert_eq!(theme.colors.highlight, "#555555");
        assert_eq!(theme.colors.cursor, "#ffffff");
        assert_eq!(theme.colors.added, "#aabbcc");
        assert_eq!(theme.colors.removed, "#ccbbaa");
        assert_eq!(theme.colors.modified, "#ddeeff");
        assert_eq!(theme.colors.untracked, "#ffeedd");
    }

    #[test]
    fn test_parse_theme_extends_dracula_with_override() {
        let toml_str = r##"
name = "my-dracula"
extends = "dracula"

[colors]
primary = "#ff0000"
"##;

        let theme = parse_custom_theme(toml_str).unwrap();
        assert_eq!(theme.name, "my-dracula");
        // The overridden color
        assert_eq!(theme.colors.primary, "#ff0000");
        // Inherited from dracula base
        let dracula = crate::theme::builtin::dracula_theme();
        assert_eq!(theme.colors.background, dracula.colors.background);
        assert_eq!(theme.colors.foreground, dracula.colors.foreground);
        assert_eq!(theme.colors.secondary, dracula.colors.secondary);
        assert_eq!(theme.colors.error, dracula.colors.error);
    }

    #[test]
    fn test_parse_theme_missing_optional_fields() {
        let toml_str = r#"
name = "minimal"
"#;

        let theme = parse_custom_theme(toml_str).unwrap();
        assert_eq!(theme.name, "minimal");
        // display_name defaults to name when not specified
        assert_eq!(theme.display_name, "minimal");
        // Colors should be from Theme::default()
        let default_colors = ColorPalette::default();
        assert_eq!(theme.colors.primary, default_colors.primary);
        assert_eq!(theme.colors.background, default_colors.background);
    }

    #[test]
    fn test_parse_invalid_toml() {
        let toml_str = r#"
this is not valid toml [[[
"#;

        let result = parse_custom_theme(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_toml_missing_required_name() {
        let toml_str = r#"
display_name = "No Name Field"
"#;

        let result = parse_custom_theme(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_custom_themes_nonexistent_directory() {
        let nonexistent = Path::new("/tmp/rightclick_test_nonexistent_dir_12345");
        let themes = load_custom_themes_from_dir(Some(nonexistent));
        assert!(themes.is_empty());
    }

    #[test]
    fn test_load_custom_themes_from_tempdir() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let themes_dir = tmp_dir.path();

        // Write a valid theme file
        let theme_content = r##"
name = "test-custom"
display_name = "Test Custom Theme"
extends = "nord"

[colors]
primary = "#abcdef"
"##;
        std::fs::write(themes_dir.join("test-custom.toml"), theme_content).unwrap();

        // Write a non-toml file (should be ignored)
        std::fs::write(themes_dir.join("readme.txt"), "not a theme").unwrap();

        let themes = load_custom_themes_from_dir(Some(themes_dir));
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].name, "test-custom");
        assert_eq!(themes[0].display_name, "Test Custom Theme");
        assert_eq!(themes[0].colors.primary, "#abcdef");
        // Inherited from nord
        let nord = crate::theme::builtin::nord_theme();
        assert_eq!(themes[0].colors.background, nord.colors.background);
    }

    #[test]
    fn test_load_custom_themes_skips_invalid_files() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let themes_dir = tmp_dir.path();

        // Write an invalid theme file
        std::fs::write(themes_dir.join("bad.toml"), "not valid [[[ toml").unwrap();

        // Write a valid theme file
        let valid = r#"
name = "good-theme"
"#;
        std::fs::write(themes_dir.join("good.toml"), valid).unwrap();

        let themes = load_custom_themes_from_dir(Some(themes_dir));
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].name, "good-theme");
    }

    #[test]
    fn test_load_custom_themes_empty_directory() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let themes = load_custom_themes_from_dir(Some(tmp_dir.path()));
        assert!(themes.is_empty());
    }

    #[test]
    fn test_parse_theme_extends_unknown_base_falls_back_to_default() {
        let toml_str = r##"
name = "bad-base"
extends = "nonexistent-theme"

[colors]
primary = "#123456"
"##;

        let theme = parse_custom_theme(toml_str).unwrap();
        assert_eq!(theme.name, "bad-base");
        assert_eq!(theme.colors.primary, "#123456");
        // Should fall back to Theme::default() colors for non-overridden fields
        let default_colors = ColorPalette::default();
        assert_eq!(theme.colors.background, default_colors.background);
    }

    #[test]
    fn test_merge_partial_colors_selective() {
        let mut palette = ColorPalette::default();
        let original_secondary = palette.secondary.clone();

        let partial = PartialColorPalette {
            primary: Some("#aaaaaa".to_string()),
            ..Default::default()
        };

        merge_partial_colors(&mut palette, &partial);
        assert_eq!(palette.primary, "#aaaaaa");
        // secondary should be untouched
        assert_eq!(palette.secondary, original_secondary);
    }
}
