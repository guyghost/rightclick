//! Action guards - Pure authorization logic
//!
//! This module provides pure functions for checking whether an action
//! is authorized to execute given the current state and context.

use crate::core::models::action::{ActionContext, ActionId, GuardError, GuardResult};
#[cfg(test)]
use crate::core::models::state_machine::StateContext;
use crate::core::models::state_machine::{FocusPane, ViewMode, ViewState};

/// Check if an action is authorized (pure function)
///
/// This function examines the action, current state, and context to determine
/// if the action should be allowed to proceed.
///
/// # Arguments
/// * `ctx` - The action context containing action, state, and context
///
/// # Returns
/// * `GuardResult::Authorized` - Action is allowed
/// * `GuardResult::Denied(GuardError)` - Action is denied with reason
pub fn check_guard(ctx: &ActionContext) -> GuardResult {
    match ctx.action {
        // Navigation is always allowed
        ActionId::NavigateUp
        | ActionId::NavigateDown
        | ActionId::NavigateLeft
        | ActionId::NavigateRight => GuardResult::Authorized,

        // Actions requiring selection + Status mode + Sidebar focus
        ActionId::Stage | ActionId::Unstage | ActionId::Diff => {
            check_requires_selection(ctx, |ctx| {
                // Check: must be in Status view
                if ctx.context.view_mode != ViewMode::Status {
                    return GuardResult::Denied(GuardError::WrongViewMode {
                        current: ctx.context.view_mode,
                        required: ViewMode::Status,
                    });
                }
                // Check: focus is in sidebar
                if ctx.context.focus_pane != FocusPane::Sidebar {
                    return GuardResult::Denied(GuardError::WrongFocus {
                        current: ctx.context.focus_pane,
                        required: FocusPane::Sidebar,
                    });
                }
                GuardResult::Authorized
            })
        }

        // Commit action - authorized (staged files check is external)
        ActionId::Commit => GuardResult::Authorized,

        // Push/Pull - always authorized
        ActionId::Push | ActionId::Pull => GuardResult::Authorized,

        // View mode switching - always authorized
        ActionId::SwitchMode(_) => GuardResult::Authorized,

        // Refresh - always authorized
        ActionId::Refresh => GuardResult::Authorized,

        // Select and Back - always authorized
        ActionId::Select | ActionId::Back => GuardResult::Authorized,

        // Modal actions only in Modal/Editing state
        ActionId::Confirm | ActionId::Cancel => match ctx.state {
            ViewState::Modal { .. } | ViewState::Editing { .. } => GuardResult::Authorized,
            _ => GuardResult::Denied(GuardError::InvalidState {
                current: ctx.state.clone(),
                action: ctx.action,
            }),
        },
    }
}

/// Helper: check if an action requires a selection
///
/// First checks for basic selection requirements, then delegates to
/// additional checks if selection is valid.
fn check_requires_selection<F>(ctx: &ActionContext, additional_checks: F) -> GuardResult
where
    F: FnOnce(&ActionContext) -> GuardResult,
{
    // Check: must have a selection
    if ctx.context.selected_index.is_none() {
        return GuardResult::Denied(GuardError::NoSelection);
    }

    // Check: must have items available
    if ctx.context.item_count == 0 {
        return GuardResult::Denied(GuardError::InvalidSelection {
            reason: "No items available".to_string(),
        });
    }

    additional_checks(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a basic action context
    fn action_context(
        action: ActionId,
        selected_index: Option<usize>,
        view_mode: ViewMode,
    ) -> ActionContext {
        ActionContext {
            action,
            state: ViewState::Ready,
            context: StateContext {
                selected_index,
                view_mode,
                focus_pane: FocusPane::Sidebar,
                item_count: 5,
                available_actions: vec![],
            },
        }
    }

    /// Helper to create a context with full control
    fn full_action_context(
        action: ActionId,
        state: ViewState,
        selected_index: Option<usize>,
        view_mode: ViewMode,
        focus_pane: FocusPane,
        item_count: usize,
    ) -> ActionContext {
        ActionContext {
            action,
            state,
            context: StateContext {
                selected_index,
                view_mode,
                focus_pane,
                item_count,
                available_actions: vec![],
            },
        }
    }

    #[test]
    fn test_stage_requires_selection() {
        let ctx = action_context(ActionId::Stage, None, ViewMode::Status);
        let result = check_guard(&ctx);
        assert!(matches!(
            result,
            GuardResult::Denied(GuardError::NoSelection)
        ));
    }

    #[test]
    fn test_stage_requires_status_mode() {
        let ctx = full_action_context(
            ActionId::Stage,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::History,
            FocusPane::Sidebar,
            5,
        );
        let result = check_guard(&ctx);
        assert!(matches!(
            result,
            GuardResult::Denied(GuardError::WrongViewMode { .. })
        ));
    }

    #[test]
    fn test_stage_requires_sidebar_focus() {
        let ctx = full_action_context(
            ActionId::Stage,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::Status,
            FocusPane::Main,
            5,
        );
        let result = check_guard(&ctx);
        assert!(matches!(
            result,
            GuardResult::Denied(GuardError::WrongFocus { .. })
        ));
    }

    #[test]
    fn test_stage_authorized_when_valid() {
        let ctx = full_action_context(
            ActionId::Stage,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::Status,
            FocusPane::Sidebar,
            5,
        );
        let result = check_guard(&ctx);
        assert_eq!(result, GuardResult::Authorized);
    }

    #[test]
    fn test_unstage_requires_selection() {
        let ctx = action_context(ActionId::Unstage, None, ViewMode::Status);
        let result = check_guard(&ctx);
        assert!(matches!(
            result,
            GuardResult::Denied(GuardError::NoSelection)
        ));
    }

    #[test]
    fn test_navigation_always_authorized() {
        for action in [
            ActionId::NavigateUp,
            ActionId::NavigateDown,
            ActionId::NavigateLeft,
            ActionId::NavigateRight,
        ] {
            let ctx = full_action_context(
                action,
                ViewState::Ready,
                None,
                ViewMode::Status,
                FocusPane::Sidebar,
                0,
            );
            let result = check_guard(&ctx);
            assert_eq!(result, GuardResult::Authorized);
        }
    }

    #[test]
    fn test_refresh_always_authorized() {
        let ctx = action_context(ActionId::Refresh, None, ViewMode::Status);
        let result = check_guard(&ctx);
        assert_eq!(result, GuardResult::Authorized);
    }

    #[test]
    fn test_confirm_requires_modal_state() {
        let ctx = full_action_context(
            ActionId::Confirm,
            ViewState::Ready,
            None,
            ViewMode::Status,
            FocusPane::Sidebar,
            5,
        );
        let result = check_guard(&ctx);
        assert!(matches!(
            result,
            GuardResult::Denied(GuardError::InvalidState { .. })
        ));
    }

    #[test]
    fn test_confirm_authorized_in_modal_state() {
        let ctx = full_action_context(
            ActionId::Confirm,
            ViewState::Modal {
                parent: Box::new(ViewState::Ready),
            },
            None,
            ViewMode::Status,
            FocusPane::Sidebar,
            5,
        );
        let result = check_guard(&ctx);
        assert_eq!(result, GuardResult::Authorized);
    }

    #[test]
    fn test_invalid_selection_when_no_items() {
        let ctx = full_action_context(
            ActionId::Stage,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::Status,
            FocusPane::Sidebar,
            0,
        );
        let result = check_guard(&ctx);
        assert!(matches!(
            result,
            GuardResult::Denied(GuardError::InvalidSelection { .. })
        ));
    }
}
