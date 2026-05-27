//! Command search palette system for RightClick.
//!
//! This module provides a fuzzy-searchable command search UI for quickly
//! accessing commands and navigating the application. It uses nucleo
//! for fast fuzzy matching and integrates with the theme system for
//! consistent styling.
//!
//! # Example
//!
//! ```rust
//! use rightclick::palette::{Palette, PaletteEntry, Category, PaletteAction};
//! use rightclick::keymap::FocusContext;
//! use crossterm::event::{KeyEvent, KeyCode};
//!
//! // Create a palette with entries
//! let entries = vec![
//!     PaletteEntry::minimal("file.open", "Open File", Category::Navigation)
//!         .with_description("Open a file in the editor")
//!         .with_key("ctrl+o"),
//!     PaletteEntry::minimal("file.save", "Save File", Category::Actions)
//!         .with_description("Save the current file")
//!         .with_key("ctrl+s"),
//! ];
//!
//! let mut palette = Palette::with_entries(entries);
//!
//! // Filter entries
//! palette.filter("open");
//!
//! // Handle keyboard input
//! if let Some(action) = palette.handle_key(KeyEvent::from(KeyCode::Enter)) {
//!     match action {
//!         PaletteAction::Select(entry) => println!("Selected: {}", entry.name),
//!         PaletteAction::Close => println!("Closing palette"),
//!         PaletteAction::ToggleContextMode => println!("Toggling context mode"),
//!     }
//! }
//! ```

pub mod entries;
pub mod fuzzy;
#[allow(clippy::module_inception)]
pub mod palette;

// Re-export commonly used types
pub use entries::{Category, PaletteEntry};
pub use fuzzy::{FuzzyMatcher, MatchResult};
pub use palette::{Palette, PaletteAction};

use crate::core::models::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Renders a palette at the given area.
///
/// This is a convenience function for rendering a palette without
/// needing to call the render method directly.
///
/// # Example
///
/// ```rust
/// use rightclick::palette::{Palette, render_palette};
/// use rightclick::theme::default_theme;
/// use ratatui::layout::Rect;
/// use ratatui::buffer::Buffer;
///
/// let palette = Palette::new();
/// let theme = default_theme();
/// let area = Rect::new(0, 0, 80, 24);
/// let mut buffer = Buffer::empty(area);
///
/// render_palette(&palette, area, &mut buffer, &theme);
/// ```
pub fn render_palette(palette: &Palette, area: Rect, buf: &mut Buffer, theme: &Theme) {
    palette.render(area, buf, theme);
}

/// Creates a standard set of palette entries for the application.
///
/// This provides a default set of commands that can be used across
/// the application. You can extend this with additional entries as needed.
///
/// # Example
///
/// ```rust
/// use rightclick::palette::{Palette, standard_entries};
///
/// let entries = standard_entries();
/// let palette = Palette::with_entries(entries);
/// ```
pub fn standard_entries() -> Vec<PaletteEntry> {
    use crate::keymap::FocusContext;
    use entries::Category;

    vec![
        // Navigation
        PaletteEntry::minimal("file.open", "Open File", Category::Navigation)
            .with_description("Open a file in the editor")
            .with_key("ctrl+o"),
        PaletteEntry::minimal("file.quick_open", "Quick Open", Category::Navigation)
            .with_description("Quickly open a file by name")
            .with_key("ctrl+p"),
        PaletteEntry::minimal("goto.line", "Go to Line", Category::Navigation)
            .with_description("Jump to a specific line number")
            .with_key("ctrl+g"),
        // Actions
        PaletteEntry::minimal("file.save", "Save File", Category::Actions)
            .with_description("Save the current file")
            .with_key("ctrl+s"),
        PaletteEntry::minimal("file.save_all", "Save All Files", Category::Actions)
            .with_description("Save all open files")
            .with_key("ctrl+shift+s"),
        PaletteEntry::minimal("edit.undo", "Undo", Category::Edit)
            .with_description("Undo the last action")
            .with_key("ctrl+z"),
        PaletteEntry::minimal("edit.redo", "Redo", Category::Edit)
            .with_description("Redo the last undone action")
            .with_key("ctrl+shift+z"),
        // Search
        PaletteEntry::minimal("search.find", "Find", Category::Search)
            .with_description("Find text in the current file")
            .with_key("ctrl+f"),
        PaletteEntry::minimal("search.replace", "Replace", Category::Search)
            .with_description("Find and replace text")
            .with_key("ctrl+h"),
        PaletteEntry::minimal("search.find_in_files", "Find in Files", Category::Search)
            .with_description("Search across all files")
            .with_key("ctrl+shift+f"),
        // View
        PaletteEntry::minimal("view.toggle_sidebar", "Toggle Sidebar", Category::View)
            .with_description("Show or hide the sidebar")
            .with_key("ctrl+b"),
        PaletteEntry::minimal("view.toggle_panel", "Toggle Bottom Panel", Category::View)
            .with_description("Show or hide the bottom panel")
            .with_key("ctrl+j"),
        PaletteEntry::minimal("view.command_palette", "Command Search", Category::View)
            .with_description("Search available commands")
            .with_key("ctrl+shift+p"),
        // Git
        PaletteEntry::minimal("git.commit", "Git Commit", Category::Git)
            .with_description("Create a git commit")
            .with_context(FocusContext::GitStatus),
        PaletteEntry::minimal("git.push", "Git Push", Category::Git)
            .with_description("Push commits to remote")
            .with_context(FocusContext::GitStatus),
        PaletteEntry::minimal("git.pull", "Git Pull", Category::Git)
            .with_description("Pull changes from remote")
            .with_context(FocusContext::GitStatus),
        PaletteEntry::minimal("git.status", "Git Status", Category::Git)
            .with_description("Show git status")
            .with_key("ctrl+shift+g"),
        // System
        PaletteEntry::minimal("app.quit", "Quit", Category::System)
            .with_description("Exit the application")
            .with_key("ctrl+q"),
        PaletteEntry::minimal("app.reload", "Reload", Category::System)
            .with_description("Reload the window")
            .with_key("ctrl+r"),
        PaletteEntry::minimal("app.settings", "Settings", Category::System)
            .with_description("Open settings")
            .with_key("ctrl+,"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_entries_not_empty() {
        let entries = standard_entries();
        assert!(!entries.is_empty());
    }

    #[test]
    fn test_standard_entries_name_command_search_consistently() {
        let entries = standard_entries();
        let command_search = entries
            .iter()
            .find(|entry| entry.command_id == "view.command_palette")
            .expect("standard entries should include command search");

        assert_eq!(command_search.name, "Command Search");
        assert_eq!(command_search.description, "Search available commands");
    }

    #[test]
    fn test_render_palette() {
        let palette = Palette::new();
        let theme = Theme::default();
        let area = Rect::new(0, 0, 80, 24);
        let mut buffer = Buffer::empty(area);

        render_palette(&palette, area, &mut buffer, &theme);

        // Just verify it doesn't panic
        assert_eq!(buffer.area, area);
    }
}
