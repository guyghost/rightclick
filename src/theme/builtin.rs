//! Built-in themes for RightClick
//!
//! This module defines the default themes that ship with RightClick:
//! - Default (dark theme with blue/purple accents)
//! - Dracula (popular dark theme)
//! - Nord (polar-inspired blue-gray theme)
//! - Tokyo Night (modern dark theme)

use crate::core::models::{ColorPalette, Theme, TokenColors, UiColors};

/// Returns all built-in themes
pub fn all_themes() -> Vec<Theme> {
    vec![
        default_theme(),
        dracula_theme(),
        nord_theme(),
        tokyo_night_theme(),
    ]
}

/// Default dark theme with blue/purple accents
///
/// A balanced dark theme inspired by Tokyo Night but with more neutral
/// base colors. Good for general use.
pub fn default_theme() -> Theme {
    Theme {
        name: "default".to_string(),
        display_name: "Default".to_string(),
        author: Some("RightClick".to_string()),
        description: Some("The default RightClick dark theme".to_string()),
        colors: ColorPalette {
            primary: "#7aa2f7".to_string(),    // Soft blue
            secondary: "#bb9af7".to_string(),  // Soft purple
            success: "#9ece6a".to_string(),    // Soft green
            warning: "#e0af68".to_string(),    // Soft orange
            error: "#f7768e".to_string(),      // Soft red
            info: "#7dcfff".to_string(),       // Cyan
            background: "#1a1b26".to_string(), // Deep blue-black
            foreground: "#c0caf5".to_string(), // Soft white
            muted: "#565f89".to_string(),      // Gray-blue
            border: "#414868".to_string(),     // Dark border
            highlight: "#283457".to_string(),  // Selection blue
            cursor: "#c0caf5".to_string(),     // Match foreground
            added: "#9ece6a".to_string(),      // Git additions - green
            removed: "#f7768e".to_string(),    // Git deletions - red
            modified: "#e0af68".to_string(),   // Git modified - orange
            untracked: "#7aa2f7".to_string(),  // Git untracked - blue
        },
        token_colors: TokenColors {
            comment: "#565f89".to_string(),       // Muted blue
            string: "#9ece6a".to_string(),        // Soft green
            number: "#ff9e64".to_string(),        // Orange
            keyword: "#bb9af7".to_string(),       // Purple
            function: "#7aa2f7".to_string(),      // Blue
            type_name: "#2ac3de".to_string(),     // Cyan
            variable: "#c0caf5".to_string(),      // Foreground
            constant: "#ff9e64".to_string(),      // Orange
            operator: "#89ddff".to_string(),      // Light cyan
            punctuation: "#c0caf5".to_string(),   // Foreground
            documentation: "#73daca".to_string(), // Teal
            macro_name: "#e0af68".to_string(),    // Orange
            namespace: "#7aa2f7".to_string(),     // Blue
            regex: "#b9f27c".to_string(),         // Light green
            escape: "#89ddff".to_string(),        // Light cyan
        },
        ui_colors: UiColors {
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
        },
    }
}

/// Dracula theme
///
/// A popular dark theme with vibrant colors and good contrast.
/// Originally created by Zeno Rocha.
pub fn dracula_theme() -> Theme {
    Theme {
        name: "dracula".to_string(),
        display_name: "Dracula".to_string(),
        author: Some("Dracula Theme".to_string()),
        description: Some("Dark theme with vibrant colors".to_string()),
        colors: ColorPalette {
            primary: "#bd93f9".to_string(),    // Purple
            secondary: "#ff79c6".to_string(),  // Pink
            success: "#50fa7b".to_string(),    // Green
            warning: "#f1fa8c".to_string(),    // Yellow
            error: "#ff5555".to_string(),      // Red
            info: "#8be9fd".to_string(),       // Cyan
            background: "#282a36".to_string(), // Dark background
            foreground: "#f8f8f2".to_string(), // Foreground
            muted: "#6272a4".to_string(),      // Comment
            border: "#44475a".to_string(),     // Selection
            highlight: "#44475a".to_string(),  // Selection
            cursor: "#f8f8f2".to_string(),     // Foreground
            added: "#50fa7b".to_string(),      // Git additions
            removed: "#ff5555".to_string(),    // Git deletions
            modified: "#f1fa8c".to_string(),   // Git modified
            untracked: "#bd93f9".to_string(),  // Git untracked
        },
        token_colors: TokenColors {
            comment: "#6272a4".to_string(),       // Comment color
            string: "#f1fa8c".to_string(),        // Yellow
            number: "#bd93f9".to_string(),        // Purple
            keyword: "#ff79c6".to_string(),       // Pink
            function: "#50fa7b".to_string(),      // Green
            type_name: "#8be9fd".to_string(),     // Cyan
            variable: "#f8f8f2".to_string(),      // Foreground
            constant: "#bd93f9".to_string(),      // Purple
            operator: "#ff79c6".to_string(),      // Pink
            punctuation: "#f8f8f2".to_string(),   // Foreground
            documentation: "#8be9fd".to_string(), // Cyan
            macro_name: "#ffb86c".to_string(),    // Orange
            namespace: "#50fa7b".to_string(),     // Green
            regex: "#f1fa8c".to_string(),         // Yellow
            escape: "#ff79c6".to_string(),        // Pink
        },
        ui_colors: UiColors {
            status_bar_bg: "#191a21".to_string(),
            status_bar_fg: "#f8f8f2".to_string(),
            sidebar_bg: "#191a21".to_string(),
            sidebar_fg: "#f8f8f2".to_string(),
            active_item_bg: "#44475a".to_string(),
            active_item_fg: "#f8f8f2".to_string(),
            inactive_item_bg: "#191a21".to_string(),
            inactive_item_fg: "#6272a4".to_string(),
            popup_bg: "#282a36".to_string(),
            popup_fg: "#f8f8f2".to_string(),
            popup_border: "#44475a".to_string(),
            input_bg: "#44475a".to_string(),
            input_fg: "#f8f8f2".to_string(),
            input_placeholder: "#6272a4".to_string(),
            button_bg: "#bd93f9".to_string(),
            button_fg: "#282a36".to_string(),
            button_hover_bg: "#ff79c6".to_string(),
            button_hover_fg: "#282a36".to_string(),
        },
    }
}

/// Nord theme
///
/// A polar-inspired theme with blue-gray colors.
/// Originally created by Arctic Ice Studio.
pub fn nord_theme() -> Theme {
    Theme {
        name: "nord".to_string(),
        display_name: "Nord".to_string(),
        author: Some("Arctic Ice Studio".to_string()),
        description: Some("Polar-inspired blue-gray theme".to_string()),
        colors: ColorPalette {
            primary: "#88c0d0".to_string(),    // Frost 3
            secondary: "#b48ead".to_string(),  // Aurora 5
            success: "#a3be8c".to_string(),    // Aurora 3
            warning: "#ebcb8b".to_string(),    // Aurora 4
            error: "#bf616a".to_string(),      // Aurora 1
            info: "#81a1c1".to_string(),       // Frost 2
            background: "#2e3440".to_string(), // Polar Night 0
            foreground: "#eceff4".to_string(), // Snow Storm 3
            muted: "#4c566a".to_string(),      // Polar Night 3
            border: "#4c566a".to_string(),     // Polar Night 3
            highlight: "#434c5e".to_string(),  // Polar Night 2
            cursor: "#eceff4".to_string(),     // Snow Storm 3
            added: "#a3be8c".to_string(),      // Aurora 3
            removed: "#bf616a".to_string(),    // Aurora 1
            modified: "#ebcb8b".to_string(),   // Aurora 4
            untracked: "#88c0d0".to_string(),  // Frost 3
        },
        token_colors: TokenColors {
            comment: "#616e88".to_string(),       // Comments
            string: "#a3be8c".to_string(),        // Aurora 3
            number: "#b48ead".to_string(),        // Aurora 5
            keyword: "#81a1c1".to_string(),       // Frost 2
            function: "#88c0d0".to_string(),      // Frost 3
            type_name: "#8fbcbb".to_string(),     // Frost 4
            variable: "#eceff4".to_string(),      // Snow Storm 3
            constant: "#d08770".to_string(),      // Aurora 2
            operator: "#81a1c1".to_string(),      // Frost 2
            punctuation: "#eceff4".to_string(),   // Snow Storm 3
            documentation: "#8fbcbb".to_string(), // Frost 4
            macro_name: "#d08770".to_string(),    // Aurora 2
            namespace: "#88c0d0".to_string(),     // Frost 3
            regex: "#a3be8c".to_string(),         // Aurora 3
            escape: "#ebcb8b".to_string(),        // Aurora 4
        },
        ui_colors: UiColors {
            status_bar_bg: "#3b4252".to_string(), // Polar Night 1
            status_bar_fg: "#eceff4".to_string(),
            sidebar_bg: "#3b4252".to_string(), // Polar Night 1
            sidebar_fg: "#eceff4".to_string(),
            active_item_bg: "#434c5e".to_string(), // Polar Night 2
            active_item_fg: "#eceff4".to_string(),
            inactive_item_bg: "#3b4252".to_string(),
            inactive_item_fg: "#616e88".to_string(),
            popup_bg: "#2e3440".to_string(),
            popup_fg: "#eceff4".to_string(),
            popup_border: "#4c566a".to_string(),
            input_bg: "#3b4252".to_string(),
            input_fg: "#eceff4".to_string(),
            input_placeholder: "#616e88".to_string(),
            button_bg: "#88c0d0".to_string(),
            button_fg: "#2e3440".to_string(),
            button_hover_bg: "#81a1c1".to_string(),
            button_hover_fg: "#2e3440".to_string(),
        },
    }
}

/// Tokyo Night theme
///
/// A modern dark theme with purple/blue accents.
/// Originally created by Enkia.
pub fn tokyo_night_theme() -> Theme {
    Theme {
        name: "tokyo-night".to_string(),
        display_name: "Tokyo Night".to_string(),
        author: Some("Enkia".to_string()),
        description: Some("Modern dark theme with purple/blue accents".to_string()),
        colors: ColorPalette {
            primary: "#7aa2f7".to_string(),    // Blue
            secondary: "#bb9af7".to_string(),  // Purple
            success: "#9ece6a".to_string(),    // Green
            warning: "#e0af68".to_string(),    // Yellow
            error: "#f7768e".to_string(),      // Red
            info: "#7dcfff".to_string(),       // Cyan
            background: "#1a1b26".to_string(), // Dark background
            foreground: "#c0caf5".to_string(), // Foreground
            muted: "#565f89".to_string(),      // Comment
            border: "#414868".to_string(),     // Darker border
            highlight: "#283457".to_string(),  // Visual selection
            cursor: "#c0caf5".to_string(),     // Cursor
            added: "#9ece6a".to_string(),      // Git additions
            removed: "#f7768e".to_string(),    // Git deletions
            modified: "#e0af68".to_string(),   // Git modified
            untracked: "#7aa2f7".to_string(),  // Git untracked
        },
        token_colors: TokenColors {
            comment: "#565f89".to_string(),       // Comment
            string: "#9ece6a".to_string(),        // Green
            number: "#ff9e64".to_string(),        // Orange
            keyword: "#bb9af7".to_string(),       // Purple
            function: "#7aa2f7".to_string(),      // Blue
            type_name: "#2ac3de".to_string(),     // Cyan
            variable: "#c0caf5".to_string(),      // Foreground
            constant: "#ff9e64".to_string(),      // Orange
            operator: "#89ddff".to_string(),      // Light cyan
            punctuation: "#c0caf5".to_string(),   // Foreground
            documentation: "#73daca".to_string(), // Teal
            macro_name: "#e0af68".to_string(),    // Yellow
            namespace: "#7aa2f7".to_string(),     // Blue
            regex: "#b9f27c".to_string(),         // Light green
            escape: "#89ddff".to_string(),        // Light cyan
        },
        ui_colors: UiColors {
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
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme() {
        let theme = default_theme();
        assert_eq!(theme.name, "default");
        assert_eq!(theme.display_name, "Default");
        assert!(theme.description.is_some());
    }

    #[test]
    fn test_dracula_theme() {
        let theme = dracula_theme();
        assert_eq!(theme.name, "dracula");
        assert_eq!(theme.display_name, "Dracula");
        assert_eq!(theme.colors.background, "#282a36");
    }

    #[test]
    fn test_nord_theme() {
        let theme = nord_theme();
        assert_eq!(theme.name, "nord");
        assert_eq!(theme.display_name, "Nord");
        assert_eq!(theme.colors.background, "#2e3440");
    }

    #[test]
    fn test_tokyo_night_theme() {
        let theme = tokyo_night_theme();
        assert_eq!(theme.name, "tokyo-night");
        assert_eq!(theme.display_name, "Tokyo Night");
        assert_eq!(theme.colors.background, "#1a1b26");
    }

    #[test]
    fn test_all_themes_count() {
        let themes = all_themes();
        assert_eq!(themes.len(), 4);
    }

    #[test]
    fn test_theme_colors_are_valid_hex() {
        for theme in all_themes() {
            // Check that all colors start with # and have 7 characters (#RRGGBB)
            assert!(theme.colors.primary.starts_with('#'));
            assert_eq!(theme.colors.primary.len(), 7);
            assert!(theme.colors.background.starts_with('#'));
            assert_eq!(theme.colors.background.len(), 7);
        }
    }
}
