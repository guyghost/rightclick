//! Focus contexts for keyboard shortcuts.
//!
//! Focus contexts define where keyboard input should be routed based on
//! which UI component currently has focus. This allows different shortcuts
//! to have different meanings in different parts of the application.

/// Defines the current focus context for keyboard shortcut routing.
///
/// Each variant represents a distinct UI area or mode where shortcuts
/// may have context-specific behavior. The `Global` context applies
/// everywhere unless overridden by a more specific context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusContext {
    /// Global shortcuts that apply everywhere unless overridden.
    Global,

    /// Git status view is focused.
    GitStatus,

    /// Git diff view is focused.
    GitDiff,

    /// File browser panel is focused.
    FileBrowser,

    /// File browser tree (directory tree) is focused.
    FileBrowserTree,

    /// Conversations list is focused.
    Conversations,

    /// Workspace panel is focused.
    Workspace,

    /// Workspace in interactive mode (e.g., editing).
    WorkspaceInteractive,

    /// A modal dialog is open.
    Modal,

    /// Command palette is open.
    Palette,

    /// Search overlay is open.
    Search,
}

impl FocusContext {
    /// Returns whether this context is a "root" context where 'q' should quit.
    ///
    /// Root contexts are top-level views where pressing 'q' naturally
    /// means "quit this view/quit the application" rather than performing
    /// some other action.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::FocusContext;
    ///
    /// assert!(FocusContext::is_root_context(FocusContext::Global));
    /// assert!(FocusContext::is_root_context(FocusContext::GitStatus));
    /// assert!(!FocusContext::is_root_context(FocusContext::Modal));
    /// assert!(!FocusContext::is_root_context(FocusContext::Palette));
    /// ```
    pub const fn is_root_context(context: Self) -> bool {
        matches!(
            context,
            Self::Global
                | Self::GitStatus
                | Self::GitDiff
                | Self::FileBrowser
                | Self::FileBrowserTree
                | Self::Conversations
                | Self::Workspace
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_contexts_allow_quit() {
        assert!(FocusContext::is_root_context(FocusContext::Global));
        assert!(FocusContext::is_root_context(FocusContext::GitStatus));
        assert!(FocusContext::is_root_context(FocusContext::GitDiff));
        assert!(FocusContext::is_root_context(FocusContext::FileBrowser));
        assert!(FocusContext::is_root_context(FocusContext::FileBrowserTree));
        assert!(FocusContext::is_root_context(FocusContext::Conversations));
        assert!(FocusContext::is_root_context(FocusContext::Workspace));
    }

    #[test]
    fn non_root_contexts_disallow_quit() {
        assert!(!FocusContext::is_root_context(FocusContext::Modal));
        assert!(!FocusContext::is_root_context(FocusContext::Palette));
        assert!(!FocusContext::is_root_context(
            FocusContext::WorkspaceInteractive
        ));
    }

    #[test]
    fn focus_context_equality() {
        assert_eq!(FocusContext::Global, FocusContext::Global);
        assert_ne!(FocusContext::Global, FocusContext::Modal);
    }

    #[test]
    fn focus_context_hashable() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(FocusContext::Global);
        set.insert(FocusContext::Modal);
        set.insert(FocusContext::Global); // Duplicate

        assert_eq!(set.len(), 2);
    }
}
