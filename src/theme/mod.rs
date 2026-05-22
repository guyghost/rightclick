//! Theme system for RightClick
//!
//! This module provides theme resolution, built-in themes, and style generation
//! for the TUI interface. It supports both built-in themes and custom user themes.
//!
//! # Usage
//!
//! ```rust
//! use rightclick::theme::{resolve_theme, apply_theme, get_current_theme, list_themes};
//! use rightclick::core::models::Config;
//!
//! // Resolve a theme from config
//! let config = Config::default();
//! let resolved = resolve_theme(&config, None);
//!
//! // Apply the theme globally
//! apply_theme(&resolved.theme);
//!
//! // Get the current theme
//! if let Some(theme) = get_current_theme() {
//!     println!("Current theme: {}", theme.display_name);
//! }
//! ```

mod builtin;
mod resolver;
mod styles;

pub use builtin::{all_themes, default_theme, dracula_theme, nord_theme, tokyo_night_theme};
pub use resolver::{
    CURRENT_THEME, CustomThemeFile, PartialColorPalette, ResolvedTheme, get_default_theme,
    load_custom_themes, resolve_theme, resolve_theme_with_overrides,
};
pub use styles::{
    UiElement, color_from_hex, style_for_git_status, style_for_token, style_for_ui_element,
    style_from_color, style_from_colors, style_with_modifier,
};

use crate::core::models::{Theme, ThemeError};
use std::collections::HashMap;

/// Apply a theme globally
///
/// This sets the current theme that will be used by all UI components.
/// The theme is stored in a global static variable.
///
/// # Example
///
/// ```rust
/// use rightclick::theme::{apply_theme, tokyo_night_theme};
///
/// apply_theme(&tokyo_night_theme());
/// ```
pub fn apply_theme(theme: &Theme) {
    let mut current = CURRENT_THEME.write();
    *current = Some(theme.clone());
}

/// Apply a theme with color overrides
///
/// This creates a modified copy of the theme with the specified color overrides
/// and applies it globally. Overrides should be hex color strings.
///
/// # Arguments
///
/// * `theme` - The base theme to apply
/// * `overrides` - A map of color field names to hex color values
///
/// # Example
///
/// ```rust
/// use rightclick::theme::{apply_theme_with_overrides, tokyo_night_theme};
/// use std::collections::HashMap;
///
/// let mut overrides = HashMap::new();
/// overrides.insert("primary".to_string(), "#ff0000".to_string());
/// overrides.insert("background".to_string(), "#000000".to_string());
///
/// apply_theme_with_overrides(&tokyo_night_theme(), overrides);
/// ```
pub fn apply_theme_with_overrides(theme: &Theme, overrides: HashMap<String, String>) {
    let mut modified_theme = theme.clone();

    for (key, value) in overrides {
        match key.as_str() {
            "primary" => modified_theme.colors.primary = value,
            "secondary" => modified_theme.colors.secondary = value,
            "success" => modified_theme.colors.success = value,
            "warning" => modified_theme.colors.warning = value,
            "error" => modified_theme.colors.error = value,
            "info" => modified_theme.colors.info = value,
            "background" => modified_theme.colors.background = value,
            "foreground" => modified_theme.colors.foreground = value,
            "muted" => modified_theme.colors.muted = value,
            "border" => modified_theme.colors.border = value,
            "highlight" => modified_theme.colors.highlight = value,
            "cursor" => modified_theme.colors.cursor = value,
            "added" => modified_theme.colors.added = value,
            "removed" => modified_theme.colors.removed = value,
            "modified" => modified_theme.colors.modified = value,
            "untracked" => modified_theme.colors.untracked = value,
            _ => {
                // Unknown color key, ignore
            }
        }
    }

    apply_theme(&modified_theme);
}

/// Get the currently applied theme
///
/// Returns `None` if no theme has been applied yet.
///
/// # Example
///
/// ```rust
/// use rightclick::theme::get_current_theme;
///
/// if let Some(theme) = get_current_theme() {
///     println!("Using theme: {}", theme.display_name);
/// }
/// ```
pub fn get_current_theme() -> Option<Theme> {
    CURRENT_THEME.read().clone()
}

/// List all available themes
///
/// Returns a vector of all built-in themes plus any custom themes found
/// in `~/.config/rightclick/themes/*.toml`.
///
/// # Example
///
/// ```rust
/// use rightclick::theme::list_themes;
///
/// for theme in list_themes() {
///     println!("- {} ({})", theme.display_name, theme.name);
/// }
/// ```
pub fn list_themes() -> Vec<Theme> {
    let mut themes = all_themes();
    themes.extend(resolver::load_custom_themes());
    themes
}

/// Load a theme by name
///
/// Searches both built-in and custom themes for a matching name (case-insensitive).
/// Returns an error if the theme is not found.
///
/// # Example
///
/// ```rust
/// use rightclick::theme::load_theme;
///
/// match load_theme("dracula") {
///     Ok(theme) => println!("Loaded: {}", theme.display_name),
///     Err(e) => println!("Error: {}", e),
/// }
/// ```
pub fn load_theme(name: &str) -> Result<Theme, ThemeError> {
    let all = list_themes();
    let name_lower = name.to_lowercase();

    all.into_iter()
        .find(|t| {
            t.name.to_lowercase() == name_lower || t.display_name.to_lowercase() == name_lower
        })
        .ok_or_else(|| ThemeError::NotFound(name.to_string()))
}

/// Check if a theme with the given name exists
///
/// Searches both built-in and custom themes (case-insensitive).
///
/// # Example
///
/// ```rust
/// use rightclick::theme::theme_exists;
///
/// assert!(theme_exists("dracula"));
/// assert!(!theme_exists("nonexistent"));
/// ```
pub fn theme_exists(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    list_themes()
        .iter()
        .any(|t| t.name.to_lowercase() == name_lower || t.display_name.to_lowercase() == name_lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_exists() {
        assert!(theme_exists("default"));
        assert!(theme_exists("dracula"));
        assert!(theme_exists("nord"));
        assert!(theme_exists("tokyo-night"));
        assert!(!theme_exists("nonexistent"));
    }

    #[test]
    fn test_load_theme() {
        let theme = load_theme("dracula").unwrap();
        assert_eq!(theme.name, "dracula");

        let result = load_theme("nonexistent");
        assert!(matches!(result, Err(ThemeError::NotFound(_))));
    }

    #[test]
    fn test_list_themes() {
        let themes = list_themes();
        // At least the 4 built-in themes; may include custom themes from filesystem
        assert!(themes.len() >= 4);
        assert!(themes.iter().any(|t| t.name == "default"));
        assert!(themes.iter().any(|t| t.name == "dracula"));
        assert!(themes.iter().any(|t| t.name == "nord"));
        assert!(themes.iter().any(|t| t.name == "tokyo-night"));
    }

    #[test]
    fn test_apply_and_get_theme() {
        let dracula = dracula_theme();
        apply_theme(&dracula);

        let current = get_current_theme();
        assert!(current.is_some());
        assert_eq!(current.unwrap().name, "dracula");
    }

    #[test]
    fn test_apply_theme_with_overrides() {
        let default = default_theme();
        let mut overrides = HashMap::new();
        overrides.insert("primary".to_string(), "#ff0000".to_string());
        overrides.insert("background".to_string(), "#000000".to_string());

        apply_theme_with_overrides(&default, overrides);

        let current = get_current_theme().unwrap();
        assert_eq!(current.colors.primary, "#ff0000");
        assert_eq!(current.colors.background, "#000000");
        // Other colors should remain unchanged from default
        assert_eq!(current.colors.foreground, default.colors.foreground);
    }
}
