//! Theme types for RightClick
//!
//! This module defines color palettes, themes, and related data structures
//! used for styling the TUI interface.

use serde::{Deserialize, Serialize};

/// A complete color palette for the application
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorPalette {
    /// Primary accent color (used for important elements)
    pub primary: String,
    /// Secondary accent color
    pub secondary: String,
    /// Success/positive color
    pub success: String,
    /// Warning/caution color
    pub warning: String,
    /// Error/negative color
    pub error: String,
    /// Info/neutral accent color
    pub info: String,
    /// Background color
    pub background: String,
    /// Foreground/text color
    pub foreground: String,
    /// Muted/subtle text color
    pub muted: String,
    /// Border/line color
    pub border: String,
    /// Highlight/selection background
    pub highlight: String,
    /// Cursor color
    pub cursor: String,
    /// Color for added/inserted content (e.g., git additions)
    pub added: String,
    /// Color for removed/deleted content (e.g., git deletions)
    pub removed: String,
    /// Color for modified/changed content
    pub modified: String,
    /// Color for untracked/new content
    pub untracked: String,
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self {
            primary: "#7aa2f7".to_string(),
            secondary: "#bb9af7".to_string(),
            success: "#9ece6a".to_string(),
            warning: "#e0af68".to_string(),
            error: "#f7768e".to_string(),
            info: "#7dcfff".to_string(),
            background: "#1a1b26".to_string(),
            foreground: "#c0caf5".to_string(),
            muted: "#565f89".to_string(),
            border: "#414868".to_string(),
            highlight: "#283457".to_string(),
            cursor: "#c0caf5".to_string(),
            added: "#9ece6a".to_string(),
            removed: "#f7768e".to_string(),
            modified: "#e0af68".to_string(),
            untracked: "#7aa2f7".to_string(),
        }
    }
}

/// A complete theme definition
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    /// Unique identifier for the theme (e.g., "tokyo-night", "gruvbox")
    pub name: String,
    /// Human-readable display name
    pub display_name: String,
    /// Author/creator of the theme
    pub author: Option<String>,
    /// Theme description
    pub description: Option<String>,
    /// Color palette
    pub colors: ColorPalette,
    /// Semantic token colors (e.g., for syntax highlighting)
    pub token_colors: TokenColors,
    /// UI-specific color overrides
    pub ui_colors: UiColors,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            display_name: "Default".to_string(),
            author: None,
            description: Some("The default RightClick theme".to_string()),
            colors: ColorPalette::default(),
            token_colors: TokenColors::default(),
            ui_colors: UiColors::default(),
        }
    }
}

/// Syntax highlighting token colors
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TokenColors {
    /// Comments (single-line, multi-line, doc)
    pub comment: String,
    /// String literals
    pub string: String,
    /// Numbers
    pub number: String,
    /// Keywords (if, else, while, etc.)
    pub keyword: String,
    /// Function names (when being called)
    pub function: String,
    /// Type names (structs, enums, etc.)
    pub type_name: String,
    /// Variable names
    pub variable: String,
    /// Constants
    pub constant: String,
    /// Operators (+, -, =, etc.)
    pub operator: String,
    /// Punctuation (brackets, commas, etc.)
    pub punctuation: String,
    /// Documentation markup
    pub documentation: String,
    /// Macros/annotations
    pub macro_name: String,
    /// Module/namespace names
    pub namespace: String,
    /// Regular expression literals
    pub regex: String,
    /// Escape sequences in strings
    pub escape: String,
}

impl Default for TokenColors {
    fn default() -> Self {
        Self {
            comment: "#565f89".to_string(),
            string: "#9ece6a".to_string(),
            number: "#ff9e64".to_string(),
            keyword: "#bb9af7".to_string(),
            function: "#7aa2f7".to_string(),
            type_name: "#2ac3de".to_string(),
            variable: "#c0caf5".to_string(),
            constant: "#ff9e64".to_string(),
            operator: "#89ddff".to_string(),
            punctuation: "#c0caf5".to_string(),
            documentation: "#73daca".to_string(),
            macro_name: "#e0af68".to_string(),
            namespace: "#7aa2f7".to_string(),
            regex: "#b9f27c".to_string(),
            escape: "#89ddff".to_string(),
        }
    }
}

/// UI-specific color overrides
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiColors {
    /// Status bar background
    pub status_bar_bg: String,
    /// Status bar foreground
    pub status_bar_fg: String,
    /// Sidebar background
    pub sidebar_bg: String,
    /// Sidebar foreground
    pub sidebar_fg: String,
    /// Active item background
    pub active_item_bg: String,
    /// Active item foreground
    pub active_item_fg: String,
    /// Inactive item background
    pub inactive_item_bg: String,
    /// Inactive item foreground
    pub inactive_item_fg: String,
    /// Popup/dialog background
    pub popup_bg: String,
    /// Popup/dialog foreground
    pub popup_fg: String,
    /// Popup border
    pub popup_border: String,
    /// Input field background
    pub input_bg: String,
    /// Input field foreground
    pub input_fg: String,
    /// Input field placeholder
    pub input_placeholder: String,
    /// Button background
    pub button_bg: String,
    /// Button foreground
    pub button_fg: String,
    /// Button background when hovered
    pub button_hover_bg: String,
    /// Button foreground when hovered
    pub button_hover_fg: String,
}

impl Default for UiColors {
    fn default() -> Self {
        Self {
            status_bar_bg: "#16161e".to_string(),
            status_bar_fg: "#a9b1d6".to_string(),
            sidebar_bg: "#16161e".to_string(),
            sidebar_fg: "#a9b1d6".to_string(),
            active_item_bg: "#283457".to_string(),
            active_item_fg: "#c0caf5".to_string(),
            inactive_item_bg: "#16161e".to_string(),
            inactive_item_fg: "#565f89".to_string(),
            popup_bg: "#1a1b26".to_string(),
            popup_fg: "#c0caf5".to_string(),
            popup_border: "#414868".to_string(),
            input_bg: "#24283b".to_string(),
            input_fg: "#c0caf5".to_string(),
            input_placeholder: "#565f89".to_string(),
            button_bg: "#7aa2f7".to_string(),
            button_fg: "#1a1b26".to_string(),
            button_hover_bg: "#bb9af7".to_string(),
            button_hover_fg: "#1a1b26".to_string(),
        }
    }
}

/// A lightweight theme reference (name only, for storage)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThemeRef {
    /// Theme identifier
    pub name: String,
}

/// Errors that can occur when working with themes
#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    /// Theme not found
    #[error("theme not found: {0}")]
    NotFound(String),
    /// Invalid color format
    #[error("invalid color format: {0}")]
    InvalidColor(String),
    /// Failed to parse theme file
    #[error("failed to parse theme: {0}")]
    ParseError(String),
    /// Theme validation failed
    #[error("theme validation failed: {0}")]
    ValidationError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_color_palette() {
        let palette = ColorPalette::default();
        assert_eq!(palette.primary, "#7aa2f7");
        assert_eq!(palette.background, "#1a1b26");
        assert_eq!(palette.success, "#9ece6a");
    }

    #[test]
    fn test_default_theme() {
        let theme = Theme::default();
        assert_eq!(theme.name, "default");
        assert_eq!(theme.display_name, "Default");
        assert!(theme.description.is_some());
    }

    #[test]
    fn test_token_colors() {
        let tokens = TokenColors::default();
        assert_eq!(tokens.keyword, "#bb9af7");
        assert_eq!(tokens.string, "#9ece6a");
        assert_eq!(tokens.comment, "#565f89");
    }

    #[test]
    fn test_ui_colors() {
        let ui = UiColors::default();
        assert_eq!(ui.status_bar_bg, "#16161e");
        assert_eq!(ui.popup_bg, "#1a1b26");
    }
}
