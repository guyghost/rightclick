//! Action models and guards - Pure definitions
//!
//! This module defines action identifiers, guard errors, and action context
//! for the state machine. All types are pure data with no side effects.

use super::state_machine::{FocusPane, StateContext, ViewMode, ViewState};

/// Unique action identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionId {
    // Navigation
    /// Navigate up in current list
    NavigateUp,
    /// Navigate down in current list
    NavigateDown,
    /// Navigate left (focus sidebar)
    NavigateLeft,
    /// Navigate right (focus main pane)
    NavigateRight,
    /// Select current item
    Select,
    /// Go back / cancel selection
    Back,

    // View actions
    /// Refresh current view
    Refresh,
    /// Switch to different view mode
    SwitchMode(ViewMode),

    // Git-specific
    /// Stage selected file
    Stage,
    /// Unstage selected file
    Unstage,
    /// Show diff for selected file/commit
    Diff,
    /// Create a commit
    Commit,
    /// Push to remote
    Push,
    /// Pull from remote
    Pull,

    // Modal
    /// Confirm action in modal
    Confirm,
    /// Cancel action in modal
    Cancel,
}

impl std::fmt::Display for ActionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionId::NavigateUp => write!(f, "Navigate Up"),
            ActionId::NavigateDown => write!(f, "Navigate Down"),
            ActionId::NavigateLeft => write!(f, "Navigate Left"),
            ActionId::NavigateRight => write!(f, "Navigate Right"),
            ActionId::Select => write!(f, "Select"),
            ActionId::Back => write!(f, "Back"),
            ActionId::Refresh => write!(f, "Refresh"),
            ActionId::SwitchMode(mode) => write!(f, "Switch to {:?}", mode),
            ActionId::Stage => write!(f, "Stage"),
            ActionId::Unstage => write!(f, "Unstage"),
            ActionId::Diff => write!(f, "Diff"),
            ActionId::Commit => write!(f, "Commit"),
            ActionId::Push => write!(f, "Push"),
            ActionId::Pull => write!(f, "Pull"),
            ActionId::Confirm => write!(f, "Confirm"),
            ActionId::Cancel => write!(f, "Cancel"),
        }
    }
}

/// Guard error - why an action was denied
#[derive(Clone, Debug, PartialEq)]
pub enum GuardError {
    /// No item selected
    NoSelection,
    /// Item selected but action not applicable
    InvalidSelection { reason: String },
    /// Wrong view mode for this action
    WrongViewMode {
        current: ViewMode,
        required: ViewMode,
    },
    /// Wrong focus pane for this action
    WrongFocus {
        current: FocusPane,
        required: FocusPane,
    },
    /// State doesn't allow this action
    InvalidState {
        current: ViewState,
        action: ActionId,
    },
    /// Custom guard failed
    Custom { message: String },
}

impl std::fmt::Display for GuardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GuardError::NoSelection => write!(f, "No item selected"),
            GuardError::InvalidSelection { reason } => write!(f, "Invalid selection: {}", reason),
            GuardError::WrongViewMode { current, required } => {
                write!(
                    f,
                    "Action requires {:?} mode, currently in {:?}",
                    required, current
                )
            }
            GuardError::WrongFocus { current, required } => {
                write!(
                    f,
                    "Action requires {:?} focus, currently in {:?}",
                    required, current
                )
            }
            GuardError::InvalidState { current, action } => {
                write!(f, "Cannot {:?} in {:?} state", action, current)
            }
            GuardError::Custom { message } => write!(f, "{}", message),
        }
    }
}

impl std::error::Error for GuardError {}

/// Guard check result
#[derive(Clone, Debug, PartialEq)]
pub enum GuardResult {
    /// Action is authorized
    Authorized,
    /// Action is denied with reason
    Denied(GuardError),
}

/// Action with context for guard checking
#[derive(Clone, Debug)]
pub struct ActionContext {
    /// Action to execute
    pub action: ActionId,
    /// Current view state
    pub state: ViewState,
    /// Current context
    pub context: StateContext,
}

impl ActionContext {
    /// Create a new action context
    pub const fn new(action: ActionId, state: ViewState, context: StateContext) -> Self {
        Self {
            action,
            state,
            context,
        }
    }
}

/// Action execution result
#[derive(Clone, Debug, PartialEq)]
pub enum ActionResult {
    /// Action succeeded
    Success,
    /// Action succeeded with new state
    SuccessWithState(ViewState),
    /// Action denied by guard
    Denied(GuardError),
    /// Action failed during execution
    Failed { error: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_id_display() {
        assert_eq!(ActionId::NavigateUp.to_string(), "Navigate Up");
        assert_eq!(ActionId::Stage.to_string(), "Stage");
        assert_eq!(
            ActionId::SwitchMode(ViewMode::Status).to_string(),
            "Switch to Status"
        );
    }

    #[test]
    fn test_guard_error_display() {
        let err = GuardError::NoSelection;
        assert_eq!(err.to_string(), "No item selected");

        let err = GuardError::InvalidSelection {
            reason: "File not found".to_string(),
        };
        assert_eq!(err.to_string(), "Invalid selection: File not found");

        let err = GuardError::WrongViewMode {
            current: ViewMode::History,
            required: ViewMode::Status,
        };
        assert!(err.to_string().contains("Status"));
        assert!(err.to_string().contains("History"));
    }

    #[test]
    fn test_action_context_new() {
        let ctx = ActionContext::new(ActionId::Stage, ViewState::Ready, StateContext::default());
        assert_eq!(ctx.action, ActionId::Stage);
    }

    #[test]
    fn test_guard_result_equality() {
        assert_eq!(GuardResult::Authorized, GuardResult::Authorized);
        assert_ne!(
            GuardResult::Authorized,
            GuardResult::Denied(GuardError::NoSelection)
        );
    }
}
