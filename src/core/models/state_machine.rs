//! State Machine Models - Pure definitions
//!
//! This module defines the state machine types for view state management.
//! All types are pure data structures with no side effects.

use super::action::ActionId;

/// The plugin's view mode determines what content is displayed
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViewMode {
    /// Show working directory status (staged/unstaged files)
    Status,
    /// Show diff for selected file
    Diff,
    /// Show commit history
    History,
}

impl Default for ViewMode {
    fn default() -> Self {
        // Default to History to show commits on startup (lazygit-style)
        Self::History
    }
}

impl std::fmt::Display for ViewMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewMode::Status => write!(f, "Status"),
            ViewMode::Diff => write!(f, "Diff"),
            ViewMode::History => write!(f, "History"),
        }
    }
}

/// Which pane has keyboard focus
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FocusPane {
    /// Sidebar with file list
    #[default]
    Sidebar,
    /// Main content area (diff or history)
    Main,
}

impl std::fmt::Display for FocusPane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FocusPane::Sidebar => write!(f, "Sidebar"),
            FocusPane::Main => write!(f, "Main"),
        }
    }
}

/// A state in the state machine
#[derive(Clone, Debug, PartialEq)]
pub enum ViewState {
    /// Initial/loading state
    Initial,
    /// Ready for user interaction (no selection)
    Ready,
    /// Item selected, details visible
    ItemSelected {
        /// Index of selected item
        index: usize,
    },
    /// Editing/acting on an item
    Editing {
        /// Index of item being edited
        index: usize,
    },
    /// Modal/dialog open
    Modal {
        /// Parent state to return to
        parent: Box<ViewState>,
    },
    /// Error state
    Error {
        /// Error message
        message: String,
        /// Previous state to return to
        previous: Box<ViewState>,
    },
}

impl Default for ViewState {
    fn default() -> Self {
        Self::Initial
    }
}

impl ViewState {
    /// Check if this state has an item selected
    pub const fn has_selection(&self) -> bool {
        matches!(
            self,
            ViewState::ItemSelected { .. } | ViewState::Editing { .. }
        )
    }

    /// Get the selected index if any
    pub const fn selected_index(&self) -> Option<usize> {
        match self {
            ViewState::ItemSelected { index } | ViewState::Editing { index } => Some(*index),
            _ => None,
        }
    }

    /// Check if in modal state
    pub const fn is_modal(&self) -> bool {
        matches!(self, ViewState::Modal { .. })
    }

    /// Check if in error state
    pub const fn is_error(&self) -> bool {
        matches!(self, ViewState::Error { .. })
    }

    /// Create an item selected state
    pub const fn selected(index: usize) -> Self {
        ViewState::ItemSelected { index }
    }

    /// Create an editing state
    pub const fn editing(index: usize) -> Self {
        ViewState::Editing { index }
    }

    /// Create a modal state
    pub fn modal(parent: ViewState) -> Self {
        ViewState::Modal {
            parent: Box::new(parent),
        }
    }

    /// Create an error state
    pub fn error(message: impl Into<String>, previous: ViewState) -> Self {
        ViewState::Error {
            message: message.into(),
            previous: Box::new(previous),
        }
    }
}

/// State context containing additional data
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StateContext {
    /// Current focus pane
    pub focus_pane: FocusPane,
    /// Current view mode
    pub view_mode: ViewMode,
    /// Number of items available for selection
    pub item_count: usize,
    /// Selected item index (if any)
    pub selected_index: Option<usize>,
    /// Available actions in current state (populated by state machine)
    pub available_actions: Vec<ActionId>,
}

impl StateContext {
    /// Create a new context with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create context for git status view
    pub fn for_status(item_count: usize) -> Self {
        Self {
            focus_pane: FocusPane::Sidebar,
            view_mode: ViewMode::Status,
            item_count,
            selected_index: None,
            available_actions: Vec::new(),
        }
    }

    /// Create context for git history view
    pub fn for_history(item_count: usize) -> Self {
        Self {
            focus_pane: FocusPane::Sidebar,
            view_mode: ViewMode::History,
            item_count,
            selected_index: if item_count > 0 { Some(0) } else { None },
            available_actions: Vec::new(),
        }
    }

    /// Check if there's a valid selection
    pub const fn has_valid_selection(&self) -> bool {
        match self.selected_index {
            Some(idx) => idx < self.item_count,
            None => false,
        }
    }

    /// Builder: set focus pane
    #[must_use]
    pub const fn with_focus_pane(mut self, pane: FocusPane) -> Self {
        self.focus_pane = pane;
        self
    }

    /// Builder: set view mode
    #[must_use]
    pub const fn with_view_mode(mut self, mode: ViewMode) -> Self {
        self.view_mode = mode;
        self
    }

    /// Builder: set item count
    #[must_use]
    pub const fn with_item_count(mut self, count: usize) -> Self {
        self.item_count = count;
        self
    }

    /// Builder: set selected index
    #[must_use]
    pub const fn with_selected_index(mut self, index: Option<usize>) -> Self {
        self.selected_index = index;
        self
    }
}

/// Transition result
#[derive(Clone, Debug, PartialEq)]
pub enum TransitionResult {
    /// Transition successful, new state
    Success(ViewState),
    /// Transition denied by guard
    Denied {
        /// Reason for denial
        reason: String,
        /// Current state (unchanged)
        current: ViewState,
    },
    /// No transition needed (already in target state)
    NoOp,
}

/// State machine definition (pure data)
#[derive(Clone, Debug)]
pub struct StateMachine {
    /// Current state
    pub current: ViewState,
    /// Context
    pub context: StateContext,
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl StateMachine {
    /// Create a new state machine in Initial state
    pub fn new() -> Self {
        Self {
            current: ViewState::Initial,
            context: StateContext::default(),
        }
    }

    /// Create a state machine with specific context
    pub fn with_context(context: StateContext) -> Self {
        Self {
            current: ViewState::Initial,
            context,
        }
    }

    /// Create a ready state machine
    pub fn ready() -> Self {
        Self {
            current: ViewState::Ready,
            context: StateContext::default(),
        }
    }

    /// Get available actions for current state
    pub fn available_actions(&self) -> Vec<ActionId> {
        match &self.current {
            ViewState::Initial => vec![ActionId::Refresh, ActionId::NavigateDown],
            ViewState::Ready => vec![
                ActionId::NavigateUp,
                ActionId::NavigateDown,
                ActionId::NavigateLeft,
                ActionId::NavigateRight,
                ActionId::Select,
                ActionId::Refresh,
                ActionId::SwitchMode(ViewMode::History),
                ActionId::SwitchMode(ViewMode::Status),
            ],
            ViewState::ItemSelected { .. } => vec![
                ActionId::NavigateUp,
                ActionId::NavigateDown,
                ActionId::NavigateLeft,
                ActionId::NavigateRight,
                ActionId::Select,
                ActionId::Back,
                ActionId::Refresh,
                ActionId::Stage,
                ActionId::Unstage,
                ActionId::Diff,
            ],
            ViewState::Editing { .. } => vec![ActionId::Confirm, ActionId::Cancel],
            ViewState::Modal { .. } => vec![
                ActionId::Confirm,
                ActionId::Cancel,
                ActionId::NavigateUp,
                ActionId::NavigateDown,
            ],
            ViewState::Error { .. } => vec![ActionId::Confirm, ActionId::Back],
        }
    }

    /// Check if an action is available in current state
    pub fn can_execute(&self, action: ActionId) -> bool {
        self.available_actions().contains(&action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_mode_default() {
        assert_eq!(ViewMode::default(), ViewMode::Status);
    }

    #[test]
    fn test_focus_pane_default() {
        assert_eq!(FocusPane::default(), FocusPane::Sidebar);
    }

    #[test]
    fn test_view_state_default() {
        assert_eq!(ViewState::default(), ViewState::Initial);
    }

    #[test]
    fn test_view_state_has_selection() {
        assert!(!ViewState::Initial.has_selection());
        assert!(!ViewState::Ready.has_selection());
        assert!(ViewState::selected(0).has_selection());
        assert!(ViewState::editing(0).has_selection());
        assert!(!ViewState::modal(ViewState::Ready).has_selection());
    }

    #[test]
    fn test_view_state_selected_index() {
        assert_eq!(ViewState::Initial.selected_index(), None);
        assert_eq!(ViewState::Ready.selected_index(), None);
        assert_eq!(ViewState::selected(5).selected_index(), Some(5));
        assert_eq!(ViewState::editing(3).selected_index(), Some(3));
    }

    #[test]
    fn test_view_state_is_modal() {
        assert!(!ViewState::Ready.is_modal());
        assert!(ViewState::modal(ViewState::Ready).is_modal());
    }

    #[test]
    fn test_view_state_is_error() {
        assert!(!ViewState::Ready.is_error());
        assert!(ViewState::error("test", ViewState::Ready).is_error());
    }

    #[test]
    fn test_state_context_for_status() {
        let ctx = StateContext::for_status(5);
        assert_eq!(ctx.focus_pane, FocusPane::Sidebar);
        assert_eq!(ctx.view_mode, ViewMode::Status);
        assert_eq!(ctx.item_count, 5);
        assert_eq!(ctx.selected_index, None);
    }

    #[test]
    fn test_state_context_for_history() {
        let ctx = StateContext::for_history(10);
        assert_eq!(ctx.focus_pane, FocusPane::Sidebar);
        assert_eq!(ctx.view_mode, ViewMode::History);
        assert_eq!(ctx.item_count, 10);
        assert_eq!(ctx.selected_index, Some(0));
    }

    #[test]
    fn test_state_context_for_history_empty() {
        let ctx = StateContext::for_history(0);
        assert_eq!(ctx.selected_index, None);
    }

    #[test]
    fn test_state_context_has_valid_selection() {
        let ctx = StateContext::for_status(5).with_selected_index(Some(2));
        assert!(ctx.has_valid_selection());

        let ctx = StateContext::for_status(5).with_selected_index(Some(10));
        assert!(!ctx.has_valid_selection());

        let ctx = StateContext::for_status(5);
        assert!(!ctx.has_valid_selection());
    }

    #[test]
    fn test_state_context_builder() {
        let ctx = StateContext::new()
            .with_focus_pane(FocusPane::Main)
            .with_view_mode(ViewMode::Diff)
            .with_item_count(3)
            .with_selected_index(Some(1));

        assert_eq!(ctx.focus_pane, FocusPane::Main);
        assert_eq!(ctx.view_mode, ViewMode::Diff);
        assert_eq!(ctx.item_count, 3);
        assert_eq!(ctx.selected_index, Some(1));
    }

    #[test]
    fn test_state_machine_new() {
        let sm = StateMachine::new();
        assert_eq!(sm.current, ViewState::Initial);
        assert_eq!(sm.context.focus_pane, FocusPane::Sidebar);
    }

    #[test]
    fn test_state_machine_available_actions_initial() {
        let sm = StateMachine::new();
        let actions = sm.available_actions();
        assert!(actions.contains(&ActionId::Refresh));
        assert!(actions.contains(&ActionId::NavigateDown));
    }

    #[test]
    fn test_state_machine_available_actions_ready() {
        let sm = StateMachine::ready();
        let actions = sm.available_actions();
        assert!(actions.contains(&ActionId::NavigateUp));
        assert!(actions.contains(&ActionId::NavigateDown));
        assert!(actions.contains(&ActionId::Select));
    }

    #[test]
    fn test_state_machine_available_actions_selected() {
        let mut sm = StateMachine::new();
        sm.current = ViewState::selected(0);
        let actions = sm.available_actions();
        assert!(actions.contains(&ActionId::Stage));
        assert!(actions.contains(&ActionId::Unstage));
        assert!(actions.contains(&ActionId::Diff));
    }

    #[test]
    fn test_state_machine_can_execute() {
        let sm = StateMachine::new();
        assert!(sm.can_execute(ActionId::Refresh));
        assert!(!sm.can_execute(ActionId::Stage));
    }
}
