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

        // Branch operations - authorized when in Branches view with selection
        ActionId::Checkout | ActionId::DeleteBranch => check_requires_selection(ctx, |ctx| {
            if ctx.context.view_mode != ViewMode::Branches {
                return GuardResult::Denied(GuardError::WrongViewMode {
                    current: ctx.context.view_mode,
                    required: ViewMode::Branches,
                });
            }
            GuardResult::Authorized
        }),
        ActionId::CreateBranch => GuardResult::Authorized,

        // Stash operations
        ActionId::StashSave => GuardResult::Authorized,
        ActionId::StashPop | ActionId::StashDrop => check_requires_selection(ctx, |ctx| {
            if ctx.context.view_mode != ViewMode::Stash {
                return GuardResult::Denied(GuardError::WrongViewMode {
                    current: ctx.context.view_mode,
                    required: ViewMode::Stash,
                });
            }
            GuardResult::Authorized
        }),

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
    fn test_checkout_requires_branches_mode() {
        let ctx = full_action_context(
            ActionId::Checkout,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::Status,
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
    fn test_checkout_authorized_in_branches_mode() {
        let ctx = full_action_context(
            ActionId::Checkout,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::Branches,
            FocusPane::Sidebar,
            5,
        );
        let result = check_guard(&ctx);
        assert_eq!(result, GuardResult::Authorized);
    }

    #[test]
    fn test_stash_pop_requires_stash_mode() {
        let ctx = full_action_context(
            ActionId::StashPop,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::Status,
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
    fn test_stash_pop_authorized_in_stash_mode() {
        let ctx = full_action_context(
            ActionId::StashPop,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::Stash,
            FocusPane::Sidebar,
            5,
        );
        let result = check_guard(&ctx);
        assert_eq!(result, GuardResult::Authorized);
    }

    #[test]
    fn test_create_branch_always_authorized() {
        let ctx = action_context(ActionId::CreateBranch, None, ViewMode::Status);
        let result = check_guard(&ctx);
        assert_eq!(result, GuardResult::Authorized);
    }

    #[test]
    fn test_stash_save_always_authorized() {
        let ctx = action_context(ActionId::StashSave, None, ViewMode::Status);
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

    // --- Diff guard ---

    #[test]
    fn test_diff_requires_selection() {
        let ctx = action_context(ActionId::Diff, None, ViewMode::Status);
        assert!(matches!(
            check_guard(&ctx),
            GuardResult::Denied(GuardError::NoSelection)
        ));
    }

    #[test]
    fn test_diff_requires_status_mode() {
        let ctx = full_action_context(
            ActionId::Diff,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::Branches,
            FocusPane::Sidebar,
            5,
        );
        assert!(matches!(
            check_guard(&ctx),
            GuardResult::Denied(GuardError::WrongViewMode { .. })
        ));
    }

    #[test]
    fn test_diff_authorized_when_valid() {
        let ctx = full_action_context(
            ActionId::Diff,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::Status,
            FocusPane::Sidebar,
            5,
        );
        assert_eq!(check_guard(&ctx), GuardResult::Authorized);
    }

    // --- Commit / Push / Pull ---

    #[test]
    fn test_commit_push_pull_always_authorized() {
        for action in [ActionId::Commit, ActionId::Push, ActionId::Pull] {
            let ctx = action_context(action, None, ViewMode::Status);
            assert_eq!(check_guard(&ctx), GuardResult::Authorized);
        }
    }

    // --- DeleteBranch ---

    #[test]
    fn test_delete_branch_requires_selection() {
        let ctx = action_context(ActionId::DeleteBranch, None, ViewMode::Branches);
        assert!(matches!(
            check_guard(&ctx),
            GuardResult::Denied(GuardError::NoSelection)
        ));
    }

    #[test]
    fn test_delete_branch_requires_branches_mode() {
        let ctx = full_action_context(
            ActionId::DeleteBranch,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::Status,
            FocusPane::Sidebar,
            5,
        );
        assert!(matches!(
            check_guard(&ctx),
            GuardResult::Denied(GuardError::WrongViewMode { .. })
        ));
    }

    #[test]
    fn test_delete_branch_authorized_in_branches() {
        let ctx = full_action_context(
            ActionId::DeleteBranch,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::Branches,
            FocusPane::Sidebar,
            5,
        );
        assert_eq!(check_guard(&ctx), GuardResult::Authorized);
    }

    // --- StashDrop ---

    #[test]
    fn test_stash_drop_requires_selection() {
        let ctx = action_context(ActionId::StashDrop, None, ViewMode::Stash);
        assert!(matches!(
            check_guard(&ctx),
            GuardResult::Denied(GuardError::NoSelection)
        ));
    }

    #[test]
    fn test_stash_drop_authorized_in_stash_mode() {
        let ctx = full_action_context(
            ActionId::StashDrop,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::Stash,
            FocusPane::Sidebar,
            5,
        );
        assert_eq!(check_guard(&ctx), GuardResult::Authorized);
    }

    // --- Cancel in various states ---

    #[test]
    fn test_cancel_denied_in_ready() {
        let ctx = full_action_context(
            ActionId::Cancel,
            ViewState::Ready,
            None,
            ViewMode::Status,
            FocusPane::Sidebar,
            5,
        );
        assert!(matches!(
            check_guard(&ctx),
            GuardResult::Denied(GuardError::InvalidState { .. })
        ));
    }

    #[test]
    fn test_cancel_authorized_in_editing() {
        let ctx = full_action_context(
            ActionId::Cancel,
            ViewState::Editing { index: 0 },
            None,
            ViewMode::Status,
            FocusPane::Sidebar,
            5,
        );
        assert_eq!(check_guard(&ctx), GuardResult::Authorized);
    }

    // --- Always-authorized actions ---

    #[test]
    fn test_select_back_switch_always_authorized() {
        for action in [
            ActionId::Select,
            ActionId::Back,
            ActionId::SwitchMode(ViewMode::History),
        ] {
            let ctx = action_context(action, None, ViewMode::Status);
            assert_eq!(check_guard(&ctx), GuardResult::Authorized);
        }
    }

    // --- Unstage edge cases ---

    #[test]
    fn test_unstage_requires_status_mode() {
        let ctx = full_action_context(
            ActionId::Unstage,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::History,
            FocusPane::Sidebar,
            5,
        );
        assert!(matches!(
            check_guard(&ctx),
            GuardResult::Denied(GuardError::WrongViewMode { .. })
        ));
    }

    #[test]
    fn test_unstage_authorized_when_valid() {
        let ctx = full_action_context(
            ActionId::Unstage,
            ViewState::ItemSelected { index: 0 },
            Some(0),
            ViewMode::Status,
            FocusPane::Sidebar,
            5,
        );
        assert_eq!(check_guard(&ctx), GuardResult::Authorized);
    }
}
