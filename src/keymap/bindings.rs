//! Key binding definitions and actions.
//!
//! This module defines the `Action` enum representing all possible outcomes
//! of a keyboard shortcut, and the `Binding` struct that associates a key
//! with a command in a specific context.

use std::any::Any;

use super::context::FocusContext;

/// Actions that can be triggered by keyboard shortcuts.
///
/// Each variant represents a specific operation the application should
/// perform in response to a key press. Custom actions allow plugins and
/// extensions to define their own behavior.
#[derive(Debug)]
pub enum Action {
    /// Quit the current view or the application.
    Quit,

    /// Refresh the current view or data.
    Refresh,

    /// Switch to a different plugin/view.
    SwitchPlugin(String),

    /// Open the command palette.
    OpenPalette,

    /// Open the help view.
    OpenHelp,

    /// Navigate up in a list or tree.
    NavigateUp,

    /// Navigate down in a list or tree.
    NavigateDown,

    /// Navigate left (e.g., collapse tree, go back).
    NavigateLeft,

    /// Navigate right (e.g., expand tree, go forward).
    NavigateRight,

    /// Navigate to the first item in a list or view.
    NavigateFirst,

    /// Navigate to the last item in a list or view.
    NavigateLast,

    /// Select the current item.
    Select,

    /// Go back to the previous view.
    Back,

    /// Open the currently focused item.
    Open,

    /// Delete the currently focused item.
    Delete,

    /// Create a new item.
    Create,

    /// Edit the currently focused item.
    Edit,

    /// Copy the currently focused item.
    Copy,

    /// Paste from clipboard.
    Paste,

    /// Open global search.
    Search,

    /// Filter the current view.
    Filter,

    /// Toggle a boolean state (e.g., checkbox).
    Toggle,

    /// Expand the current item (e.g., tree node).
    Expand,

    /// Collapse the current item (e.g., tree node).
    Collapse,

    /// Stage the current item (e.g., in git).
    Stage,

    /// Unstage the current item (e.g., in git).
    Unstage,

    /// Commit changes.
    Commit,

    /// Push changes.
    Push,

    /// Pull changes.
    Pull,

    /// Fetch changes.
    Fetch,

    /// Show context menu for the current item.
    ContextMenu,

    /// Create a new worktree/file.
    NewFile,

    /// Link/unlink a task to the current item.
    LinkTask,

    /// Launch an AI agent for the current worktree.
    LaunchAgent,

    /// Enter interactive mode for the current item.
    Enter,

    /// Open merge workflow for the current worktree.
    Merge,

    /// Switch between different views (list, kanban, etc.).
    SwitchView,

    /// Switch to a specific tab by index.
    SwitchTab(usize),

    /// Confirm the current action (modal dialogs).
    Confirm,

    /// Cancel the current action (modal dialogs).
    Cancel,

    /// Custom action for extensibility.
    ///
    /// This variant allows plugins and external code to define their own
    /// actions. The boxed `Any` value should be downcast to the expected
    /// type by the handler.
    Custom(Box<dyn Any + Send + Sync>),
}

impl Action {
    /// Returns a human-readable description of the action.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::Action;
    ///
    /// assert_eq!(Action::Quit.description(), "Quit");
    /// assert_eq!(Action::Refresh.description(), "Refresh");
    /// ```
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Quit => "Quit",
            Self::Refresh => "Refresh",
            Self::SwitchPlugin(_) => "Switch Plugin",
            Self::OpenPalette => "Open Command Palette",
            Self::OpenHelp => "Open Help",
            Self::NavigateUp => "Navigate Up",
            Self::NavigateDown => "Navigate Down",
            Self::NavigateLeft => "Navigate Left",
            Self::NavigateRight => "Navigate Right",
            Self::NavigateFirst => "Navigate First",
            Self::NavigateLast => "Navigate Last",
            Self::Select => "Select",
            Self::Back => "Back",
            Self::Open => "Open",
            Self::Delete => "Delete",
            Self::Create => "Create",
            Self::Edit => "Edit",
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::Search => "Global Search",
            Self::Filter => "Filter",
            Self::Toggle => "Toggle",
            Self::Expand => "Expand",
            Self::Collapse => "Collapse",
            Self::Stage => "Stage",
            Self::Unstage => "Unstage",
            Self::Commit => "Commit",
            Self::Push => "Push",
            Self::Pull => "Pull",
            Self::Fetch => "Fetch",
            Self::ContextMenu => "Context Menu",
            Self::NewFile => "New File/Worktree",
            Self::LinkTask => "Link Task",
            Self::LaunchAgent => "Launch Agent",
            Self::Enter => "Enter Interactive Mode",
            Self::Merge => "Merge",
            Self::SwitchView => "Switch View",
            Self::SwitchTab(_) => "Switch Tab",
            Self::Confirm => "Confirm",
            Self::Cancel => "Cancel",
            Self::Custom(_) => "Custom Action",
        }
    }

    /// Returns true if this action should quit the application or view.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::Action;
    ///
    /// assert!(Action::Quit.is_quit());
    /// assert!(!Action::Refresh.is_quit());
    /// ```
    pub const fn is_quit(&self) -> bool {
        matches!(self, Self::Quit)
    }
}

/// A key binding associates a key combination with a command.
///
/// Bindings are context-specific, meaning the same key can trigger
/// different actions depending on which UI component has focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The key combination (e.g., "q", "ctrl+c", "enter", "escape").
    pub key: String,

    /// The ID of the command to execute when this binding is triggered.
    pub command_id: String,

    /// The context in which this binding is active.
    pub context: FocusContext,
}

impl Binding {
    /// Creates a new key binding.
    ///
    /// # Arguments
    ///
    /// * `key` - The key combination string
    /// * `command_id` - The command identifier
    /// * `context` - The focus context for this binding
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::{Binding, FocusContext};
    ///
    /// let binding = Binding::new("q", "app.quit", FocusContext::Global);
    /// assert_eq!(binding.key, "q");
    /// assert_eq!(binding.command_id, "app.quit");
    /// ```
    pub fn new<K: Into<String>, C: Into<String>>(
        key: K,
        command_id: C,
        context: FocusContext,
    ) -> Self {
        Self {
            key: key.into(),
            command_id: command_id.into(),
            context,
        }
    }

    /// Creates a global binding that applies in all contexts.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::Binding;
    ///
    /// let binding = Binding::global("escape", "app.back");
    /// assert_eq!(binding.key, "escape");
    /// assert_eq!(binding.context as i32, 0); // Global is first variant
    /// ```
    pub fn global<K: Into<String>, C: Into<String>>(key: K, command_id: C) -> Self {
        Self::new(key, command_id, FocusContext::Global)
    }
}

/// A wrapper around crossterm key events for easier handling.
///
/// This type provides direct access to the key code and modifiers
/// for plugins that handle raw key events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBinding {
    /// The key code (e.g., KeyCode::Char('a'), KeyCode::Enter)
    pub code: crossterm::event::KeyCode,
    /// The key modifiers (e.g., KeyModifiers::CONTROL)
    pub modifiers: crossterm::event::KeyModifiers,
}

impl From<crossterm::event::KeyEvent> for KeyBinding {
    fn from(event: crossterm::event::KeyEvent) -> Self {
        Self {
            code: event.code,
            modifiers: event.modifiers,
        }
    }
}

/// Type alias for focus context (used by some plugins)
pub type KeyContext = FocusContext;

/// Handler function type for key events
pub type KeyHandler = fn(KeyBinding) -> Option<Action>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_description() {
        assert_eq!(Action::Quit.description(), "Quit");
        assert_eq!(Action::Refresh.description(), "Refresh");
        assert_eq!(Action::OpenPalette.description(), "Open Command Palette");
        assert_eq!(Action::Search.description(), "Global Search");
    }

    #[test]
    fn action_is_quit() {
        assert!(Action::Quit.is_quit());
        assert!(!Action::Refresh.is_quit());
        assert!(!Action::SwitchPlugin("test".to_string()).is_quit());
    }

    #[test]
    fn binding_new() {
        let binding = Binding::new("q", "app.quit", FocusContext::Global);
        assert_eq!(binding.key, "q");
        assert_eq!(binding.command_id, "app.quit");
        assert_eq!(binding.context, FocusContext::Global);
    }

    #[test]
    fn binding_global() {
        let binding = Binding::global("escape", "app.back");
        assert_eq!(binding.key, "escape");
        assert_eq!(binding.command_id, "app.back");
        assert_eq!(binding.context, FocusContext::Global);
    }

    #[test]
    fn binding_context_specific() {
        let binding = Binding::new("enter", "git.stage", FocusContext::GitStatus);
        assert_eq!(binding.key, "enter");
        assert_eq!(binding.command_id, "git.stage");
        assert_eq!(binding.context, FocusContext::GitStatus);
    }

    #[test]
    fn action_custom() {
        let custom_data = "my custom data".to_string();
        let action = Action::Custom(Box::new(custom_data));
        assert_eq!(action.description(), "Custom Action");
        assert!(!action.is_quit());
    }
}
