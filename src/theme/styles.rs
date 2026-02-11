//! Style generation from themes
//!
//! This module provides utilities for converting theme colors into
//! ratatui `Style` objects for use in the TUI interface.

use crate::core::models::Theme;
use ratatui::style::{Color, Style};
use std::str::FromStr;

/// UI elements that can be styled
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiElement {
    /// Normal text
    Text,
    /// Muted/secondary text
    MutedText,
    /// Primary accent element
    Primary,
    /// Secondary accent element
    Secondary,
    /// Success/positive indicator
    Success,
    /// Warning/caution indicator
    Warning,
    /// Error/negative indicator
    Error,
    /// Info/neutral indicator
    Info,
    /// Border/line
    Border,
    /// Highlighted/selected item
    Highlight,
    /// Background element
    Background,
    /// Status bar
    StatusBar,
    /// Sidebar
    Sidebar,
    /// Active/selected item
    ActiveItem,
    /// Inactive item
    InactiveItem,
    /// Popup/dialog
    Popup,
    /// Button
    Button,
    /// Button in hover state
    ButtonHover,
    /// Input field
    Input,
    /// Placeholder text in input
    InputPlaceholder,
}

/// Create a style from a hex color string
///
/// Parses the color string and returns a `Style` with the foreground color set.
/// If the color cannot be parsed, falls back to `Color::White`.
///
/// # Arguments
///
/// * `color` - A hex color string (e.g., "#ff0000", "#7aa2f7")
///
/// # Example
///
/// ```rust
/// use rightclick::theme::style_from_color;
/// use ratatui::style::Color;
///
/// let style = style_from_color("#7aa2f7");
/// // style now has foreground color set to a soft blue
/// ```
pub fn style_from_color(color: &str) -> Style {
    Style::default().fg(Color::from_str(color).unwrap_or(Color::White))
}

/// Create a style from a hex color string with a background color
///
/// # Arguments
///
/// * `fg` - Foreground hex color string
/// * `bg` - Background hex color string
///
/// # Example
///
/// ```rust
/// use rightclick::theme::style_from_colors;
///
/// let style = style_from_colors("#c0caf5", "#1a1b26");
/// ```
pub fn style_from_colors(fg: &str, bg: &str) -> Style {
    Style::default()
        .fg(Color::from_str(fg).unwrap_or(Color::White))
        .bg(Color::from_str(bg).unwrap_or(Color::Black))
}

/// Get a style for a syntax highlighting token type
///
/// Returns a `Style` configured with the appropriate color from the theme's
/// token colors. This is useful for syntax highlighting in code blocks.
///
/// # Arguments
///
/// * `theme` - The theme to use for colors
/// * `token_type` - The type of token (e.g., "keyword", "string", "comment")
///
/// # Example
///
/// ```rust
/// use rightclick::theme::{style_for_token, tokyo_night_theme};
///
/// let theme = tokyo_night_theme();
/// let keyword_style = style_for_token(&theme, "keyword");
/// let string_style = style_for_token(&theme, "string");
/// ```
pub fn style_for_token(theme: &Theme, token_type: &str) -> Style {
    let color = match token_type {
        "comment" => &theme.token_colors.comment,
        "string" => &theme.token_colors.string,
        "number" => &theme.token_colors.number,
        "keyword" => &theme.token_colors.keyword,
        "function" => &theme.token_colors.function,
        "type" | "type_name" => &theme.token_colors.type_name,
        "variable" => &theme.token_colors.variable,
        "constant" => &theme.token_colors.constant,
        "operator" => &theme.token_colors.operator,
        "punctuation" => &theme.token_colors.punctuation,
        "documentation" => &theme.token_colors.documentation,
        "macro" | "macro_name" => &theme.token_colors.macro_name,
        "namespace" => &theme.token_colors.namespace,
        "regex" => &theme.token_colors.regex,
        "escape" => &theme.token_colors.escape,
        _ => &theme.colors.foreground, // Default to foreground color
    };

    style_from_color(color)
}

/// Get a style for a UI element
///
/// Returns a `Style` configured with appropriate foreground and background
/// colors from the theme for the specified UI element.
///
/// # Arguments
///
/// * `theme` - The theme to use for colors
/// * `element` - The UI element type
///
/// # Example
///
/// ```rust
/// use rightclick::theme::{style_for_ui_element, UiElement, tokyo_night_theme};
///
/// let theme = tokyo_night_theme();
/// let text_style = style_for_ui_element(&theme, UiElement::Text);
/// let error_style = style_for_ui_element(&theme, UiElement::Error);
/// let button_style = style_for_ui_element(&theme, UiElement::Button);
/// ```
pub fn style_for_ui_element(theme: &Theme, element: UiElement) -> Style {
    match element {
        UiElement::Text => style_from_color(&theme.colors.foreground),
        UiElement::MutedText => style_from_color(&theme.colors.muted),
        UiElement::Primary => style_from_color(&theme.colors.primary),
        UiElement::Secondary => style_from_color(&theme.colors.secondary),
        UiElement::Success => style_from_color(&theme.colors.success),
        UiElement::Warning => style_from_color(&theme.colors.warning),
        UiElement::Error => style_from_color(&theme.colors.error),
        UiElement::Info => style_from_color(&theme.colors.info),
        UiElement::Border => style_from_color(&theme.colors.border),
        UiElement::Highlight => {
            style_from_colors(&theme.colors.foreground, &theme.colors.highlight)
        }
        UiElement::Background => {
            Style::default().bg(Color::from_str(&theme.colors.background).unwrap_or(Color::Black))
        }
        UiElement::StatusBar => style_from_colors(
            &theme.ui_colors.status_bar_fg,
            &theme.ui_colors.status_bar_bg,
        ),
        UiElement::Sidebar => {
            style_from_colors(&theme.ui_colors.sidebar_fg, &theme.ui_colors.sidebar_bg)
        }
        UiElement::ActiveItem => style_from_colors(
            &theme.ui_colors.active_item_fg,
            &theme.ui_colors.active_item_bg,
        ),
        UiElement::InactiveItem => style_from_colors(
            &theme.ui_colors.inactive_item_fg,
            &theme.ui_colors.inactive_item_bg,
        ),
        UiElement::Popup => style_from_colors(&theme.ui_colors.popup_fg, &theme.ui_colors.popup_bg),
        UiElement::Button => {
            style_from_colors(&theme.ui_colors.button_fg, &theme.ui_colors.button_bg)
        }
        UiElement::ButtonHover => style_from_colors(
            &theme.ui_colors.button_hover_fg,
            &theme.ui_colors.button_hover_bg,
        ),
        UiElement::Input => style_from_colors(&theme.ui_colors.input_fg, &theme.ui_colors.input_bg),
        UiElement::InputPlaceholder => style_from_color(&theme.ui_colors.input_placeholder),
    }
}

/// Get a style for a git status indicator
///
/// Returns a style with the appropriate color for a git file status.
///
/// # Arguments
///
/// * `theme` - The theme to use for colors
/// * `status` - The git status string ("added", "modified", "deleted", "untracked")
///
/// # Example
///
/// ```rust
/// use rightclick::theme::{style_for_git_status, tokyo_night_theme};
///
/// let theme = tokyo_night_theme();
/// let added_style = style_for_git_status(&theme, "added");
/// let deleted_style = style_for_git_status(&theme, "deleted");
/// ```
pub fn style_for_git_status(theme: &Theme, status: &str) -> Style {
    let color = match status {
        "added" | "staged" | "new" => &theme.colors.added,
        "modified" | "changed" | "M" => &theme.colors.modified,
        "deleted" | "removed" | "D" => &theme.colors.removed,
        "untracked" | "?" => &theme.colors.untracked,
        _ => &theme.colors.foreground,
    };
    style_from_color(color)
}

/// Get a modifier style (bold, italic, etc.) with a theme color
///
/// # Arguments
///
/// * `theme` - The theme to use for colors
/// * `color_key` - The color to use ("primary", "secondary", "success", etc.)
/// * `modifier` - The modifier to add (bold, italic, underline, etc.)
///
/// # Example
///
/// ```rust
/// use rightclick::theme::style_with_modifier;
/// use ratatui::style::Modifier;
/// use rightclick::theme::tokyo_night_theme;
///
/// let theme = tokyo_night_theme();
/// let bold_primary = style_with_modifier(&theme, "primary", Modifier::BOLD);
/// let italic_secondary = style_with_modifier(&theme, "secondary", Modifier::ITALIC);
/// ```
#[allow(dead_code)]
pub fn style_with_modifier(
    theme: &Theme,
    color_key: &str,
    modifier: ratatui::style::Modifier,
) -> Style {
    let color = match color_key {
        "primary" => &theme.colors.primary,
        "secondary" => &theme.colors.secondary,
        "success" => &theme.colors.success,
        "warning" => &theme.colors.warning,
        "error" => &theme.colors.error,
        "info" => &theme.colors.info,
        "foreground" => &theme.colors.foreground,
        "muted" => &theme.colors.muted,
        "border" => &theme.colors.border,
        _ => &theme.colors.foreground,
    };

    style_from_color(color).add_modifier(modifier)
}

/// Convert a hex color string to a ratatui Color
///
/// Supports 6-digit hex colors (e.g., "#ff0000", "#7aa2f7").
/// Falls back to Color::White if parsing fails.
///
/// # Arguments
///
/// * `hex` - A hex color string
///
/// # Example
///
/// ```rust
/// use rightclick::theme::color_from_hex;
/// use ratatui::style::Color;
///
/// let color = color_from_hex("#7aa2f7");
/// assert!(matches!(color, Color::Rgb(_, _, _)));
/// ```
#[allow(dead_code)]
pub fn color_from_hex(hex: &str) -> Color {
    Color::from_str(hex).unwrap_or(Color::White)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    fn test_theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn test_style_from_color() {
        let style = style_from_color("#ff0000");
        assert_eq!(style.fg, Some(Color::Rgb(255, 0, 0)));
    }

    #[test]
    fn test_style_from_color_invalid() {
        let style = style_from_color("invalid");
        assert_eq!(style.fg, Some(Color::White));
    }

    #[test]
    fn test_style_from_colors() {
        let style = style_from_colors("#ff0000", "#00ff00");
        assert_eq!(style.fg, Some(Color::Rgb(255, 0, 0)));
        assert_eq!(style.bg, Some(Color::Rgb(0, 255, 0)));
    }

    #[test]
    fn test_style_for_token() {
        let theme = test_theme();

        let keyword = style_for_token(&theme, "keyword");
        assert!(keyword.fg.is_some());

        let string = style_for_token(&theme, "string");
        assert!(string.fg.is_some());

        let comment = style_for_token(&theme, "comment");
        assert!(comment.fg.is_some());
    }

    #[test]
    fn test_style_for_ui_element() {
        let theme = test_theme();

        let text = style_for_ui_element(&theme, UiElement::Text);
        assert!(text.fg.is_some());

        let error = style_for_ui_element(&theme, UiElement::Error);
        assert!(error.fg.is_some());

        let button = style_for_ui_element(&theme, UiElement::Button);
        assert!(button.fg.is_some());
        assert!(button.bg.is_some());
    }

    #[test]
    fn test_style_for_git_status() {
        let theme = test_theme();

        let added = style_for_git_status(&theme, "added");
        assert!(added.fg.is_some());

        let modified = style_for_git_status(&theme, "modified");
        assert!(modified.fg.is_some());

        let deleted = style_for_git_status(&theme, "deleted");
        assert!(deleted.fg.is_some());
    }

    #[test]
    fn test_style_with_modifier() {
        let theme = test_theme();

        let bold = style_with_modifier(&theme, "primary", Modifier::BOLD);
        assert!(bold.fg.is_some());
        assert!(bold.add_modifier.contains(Modifier::BOLD));

        let italic = style_with_modifier(&theme, "secondary", Modifier::ITALIC);
        assert!(italic.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn test_color_from_hex() {
        let color = color_from_hex("#7aa2f7");
        assert!(matches!(color, Color::Rgb(_, _, _)));

        let invalid = color_from_hex("invalid");
        assert_eq!(invalid, Color::White);
    }

    #[test]
    fn test_all_ui_elements() {
        let theme = test_theme();

        // Test that all UI elements can be styled without panicking
        let elements = vec![
            UiElement::Text,
            UiElement::MutedText,
            UiElement::Primary,
            UiElement::Secondary,
            UiElement::Success,
            UiElement::Warning,
            UiElement::Error,
            UiElement::Info,
            UiElement::Border,
            UiElement::Highlight,
            UiElement::Background,
            UiElement::StatusBar,
            UiElement::Sidebar,
            UiElement::ActiveItem,
            UiElement::InactiveItem,
            UiElement::Popup,
            UiElement::Button,
            UiElement::ButtonHover,
            UiElement::Input,
            UiElement::InputPlaceholder,
        ];

        for element in elements {
            let style = style_for_ui_element(&theme, element);
            // Just verify it doesn't panic
            let _ = format!("{:?}", style);
        }
    }
}
