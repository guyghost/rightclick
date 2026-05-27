//! Palette entry types for the command search system.
//!
//! This module defines the data structures for palette entries, including
//! categories, context associations, and entry metadata.

use crate::keymap::FocusContext;

/// Command categories for grouping palette entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// Navigation commands (go to file, switch tab, etc.)
    Navigation,
    /// Action commands (save, close, execute, etc.)
    Actions,
    /// View commands (toggle sidebar, zoom, etc.)
    View,
    /// Search commands (find, replace, etc.)
    Search,
    /// Edit commands (cut, copy, paste, etc.)
    Edit,
    /// Git commands (commit, push, diff, etc.)
    Git,
    /// System commands (quit, reload, settings, etc.)
    System,
}

impl Category {
    /// Returns the display name for this category.
    pub fn display_name(&self) -> &'static str {
        match self {
            Category::Navigation => "Navigation",
            Category::Actions => "Actions",
            Category::View => "View",
            Category::Search => "Search",
            Category::Edit => "Edit",
            Category::Git => "Git",
            Category::System => "System",
        }
    }

    /// Returns a short prefix for this category.
    pub fn prefix(&self) -> &'static str {
        match self {
            Category::Navigation => "NAV",
            Category::Actions => "ACT",
            Category::View => "VIEW",
            Category::Search => "SRCH",
            Category::Edit => "EDIT",
            Category::Git => "GIT",
            Category::System => "SYS",
        }
    }

    /// Returns the sort order for this category (lower = first).
    pub fn sort_order(&self) -> u8 {
        match self {
            Category::Navigation => 0,
            Category::Actions => 1,
            Category::Edit => 2,
            Category::Search => 3,
            Category::View => 4,
            Category::Git => 5,
            Category::System => 6,
        }
    }
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// A single entry in command search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    /// Unique key for this entry (used for keybinding display like "ctrl+s")
    pub key: String,
    /// Command identifier for execution
    pub command_id: String,
    /// Display name for this entry
    pub name: String,
    /// Optional description of what this command does
    pub description: String,
    /// Category for grouping
    pub category: Category,
    /// Context where this entry is available
    pub context: FocusContext,
}

impl PaletteEntry {
    /// Creates a new palette entry with all fields.
    pub fn new(
        key: impl Into<String>,
        command_id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        category: Category,
        context: FocusContext,
    ) -> Self {
        Self {
            key: key.into(),
            command_id: command_id.into(),
            name: name.into(),
            description: description.into(),
            category,
            context,
        }
    }

    /// Creates a new palette entry with minimal fields.
    pub fn minimal(
        command_id: impl Into<String>,
        name: impl Into<String>,
        category: Category,
    ) -> Self {
        Self {
            key: String::new(),
            command_id: command_id.into(),
            name: name.into(),
            description: String::new(),
            category,
            context: FocusContext::Global,
        }
    }

    /// Sets the key for this entry.
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }

    /// Sets the description for this entry.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Sets the context for this entry.
    pub fn with_context(mut self, context: FocusContext) -> Self {
        self.context = context;
        self
    }

    /// Returns the search text for this entry (used for fuzzy matching).
    pub fn search_text(&self) -> String {
        format!(
            "{} {} {} {} {}",
            self.name,
            self.description,
            self.category.display_name(),
            self.key,
            self.command_id
        )
    }

    /// Returns true if this entry is available in the given context.
    pub fn is_available_in(&self, context: FocusContext, show_all: bool) -> bool {
        if show_all {
            return true;
        }
        self.context == FocusContext::Global || self.context == context
    }
}

impl Default for PaletteEntry {
    fn default() -> Self {
        Self {
            key: String::new(),
            command_id: String::new(),
            name: String::new(),
            description: String::new(),
            category: Category::Actions,
            context: FocusContext::Global,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_display_name() {
        assert_eq!(Category::Navigation.display_name(), "Navigation");
        assert_eq!(Category::Git.display_name(), "Git");
        assert_eq!(Category::System.display_name(), "System");
    }

    #[test]
    fn test_category_prefix() {
        assert_eq!(Category::Navigation.prefix(), "NAV");
        assert_eq!(Category::Git.prefix(), "GIT");
        assert_eq!(Category::System.prefix(), "SYS");
    }

    #[test]
    fn test_category_sort_order() {
        assert!(Category::Navigation.sort_order() < Category::System.sort_order());
        assert!(Category::Actions.sort_order() < Category::Git.sort_order());
    }

    #[test]
    fn test_palette_entry_new() {
        let entry = PaletteEntry::new(
            "ctrl+s",
            "file.save",
            "Save File",
            "Save the current file",
            Category::Actions,
            FocusContext::Global,
        );

        assert_eq!(entry.key, "ctrl+s");
        assert_eq!(entry.command_id, "file.save");
        assert_eq!(entry.name, "Save File");
        assert_eq!(entry.description, "Save the current file");
        assert_eq!(entry.category, Category::Actions);
        assert_eq!(entry.context, FocusContext::Global);
    }

    #[test]
    fn test_palette_entry_minimal() {
        let entry = PaletteEntry::minimal("file.open", "Open File", Category::Navigation);

        assert_eq!(entry.command_id, "file.open");
        assert_eq!(entry.name, "Open File");
        assert_eq!(entry.category, Category::Navigation);
        assert_eq!(entry.context, FocusContext::Global);
        assert!(entry.key.is_empty());
        assert!(entry.description.is_empty());
    }

    #[test]
    fn test_palette_entry_builder_methods() {
        let entry = PaletteEntry::minimal("test", "Test", Category::Actions)
            .with_key("ctrl+t")
            .with_description("A test command")
            .with_context(FocusContext::Workspace);

        assert_eq!(entry.key, "ctrl+t");
        assert_eq!(entry.description, "A test command");
        assert_eq!(entry.context, FocusContext::Workspace);
    }

    #[test]
    fn test_search_text() {
        let entry = PaletteEntry::new(
            "ctrl+s",
            "file.save",
            "Save File",
            "Save the current file",
            Category::Actions,
            FocusContext::Global,
        );

        let search = entry.search_text();
        assert!(search.contains("Save File"));
        assert!(search.contains("Save the current file"));
        assert!(search.contains("Actions"));
        assert!(search.contains("ctrl+s"));
        assert!(search.contains("file.save"));
    }

    #[test]
    fn test_is_available_in() {
        let global_entry = PaletteEntry::minimal("test", "Test", Category::Actions)
            .with_context(FocusContext::Global);

        let git_entry = PaletteEntry::minimal("git.commit", "Commit", Category::Git)
            .with_context(FocusContext::GitStatus);

        // Global entries are available everywhere
        assert!(global_entry.is_available_in(FocusContext::Workspace, false));
        assert!(global_entry.is_available_in(FocusContext::GitStatus, false));

        // Context-specific entries are only available in their context
        assert!(!git_entry.is_available_in(FocusContext::Workspace, false));
        assert!(git_entry.is_available_in(FocusContext::GitStatus, false));

        // When show_all is true, all entries are available
        assert!(git_entry.is_available_in(FocusContext::Workspace, true));
    }

    #[test]
    fn test_default_palette_entry() {
        let entry = PaletteEntry::default();
        assert!(entry.key.is_empty());
        assert!(entry.command_id.is_empty());
        assert!(entry.name.is_empty());
        assert!(entry.description.is_empty());
        assert_eq!(entry.category, Category::Actions);
        assert_eq!(entry.context, FocusContext::Global);
    }
}
