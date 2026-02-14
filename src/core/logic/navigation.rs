//! Navigation logic - Pure calculations for keyboard navigation
//!
//! This module provides pure functions for calculating navigation results
//! based on direction and current context.

#[cfg(test)]
use crate::core::models::action::ActionId;
use crate::core::models::navigation::{NavDirection, NavRegion, NavigationResult};
use crate::core::models::state_machine::{FocusPane, StateContext, ViewState};

/// Calculate navigation result (pure function)
///
/// Given a navigation direction and current context, returns the result
/// of the navigation operation.
///
/// # Arguments
/// * `direction` - The direction to navigate
/// * `context` - The current state context
///
/// # Returns
/// * `NavigationResult::Navigate` - Successful navigation to new position
/// * `NavigationResult::Stay` - No movement needed
/// * `NavigationResult::AtBoundary` - Cannot move in that direction
/// * `NavigationResult::Action` - Navigation triggers an action
pub fn calculate_navigation(direction: NavDirection, context: &StateContext) -> NavigationResult {
    match direction {
        NavDirection::Up => navigate_up(context),
        NavDirection::Down => navigate_down(context),
        NavDirection::Left => navigate_left(context),
        NavDirection::Right => navigate_right(context),
        NavDirection::First => navigate_first(context),
        NavDirection::Last => navigate_last(context),
        NavDirection::Next | NavDirection::Previous => NavigationResult::Stay,
    }
}

/// Navigate up in current region (k key, Up arrow)
///
/// In Sidebar: moves selection up in the list
/// In Main: no list navigation
pub fn navigate_up(context: &StateContext) -> NavigationResult {
    match context.focus_pane {
        FocusPane::Sidebar => {
            if let Some(current) = context.selected_index {
                if current > 0 {
                    NavigationResult::Navigate {
                        region: NavRegion::Sidebar,
                        index: Some(current - 1),
                    }
                } else {
                    NavigationResult::AtBoundary
                }
            } else if context.item_count > 0 {
                // No selection: select last item
                NavigationResult::Navigate {
                    region: NavRegion::Sidebar,
                    index: Some(context.item_count - 1),
                }
            } else {
                NavigationResult::AtBoundary
            }
        }
        FocusPane::Main => NavigationResult::Stay,
    }
}

/// Navigate down in current region (j key, Down arrow)
///
/// In Sidebar: moves selection down in the list
/// In Main: no list navigation
pub fn navigate_down(context: &StateContext) -> NavigationResult {
    match context.focus_pane {
        FocusPane::Sidebar => {
            if let Some(current) = context.selected_index {
                if current + 1 < context.item_count {
                    NavigationResult::Navigate {
                        region: NavRegion::Sidebar,
                        index: Some(current + 1),
                    }
                } else {
                    NavigationResult::AtBoundary
                }
            } else if context.item_count > 0 {
                // No selection: select first item
                NavigationResult::Navigate {
                    region: NavRegion::Sidebar,
                    index: Some(0),
                }
            } else {
                NavigationResult::AtBoundary
            }
        }
        FocusPane::Main => NavigationResult::Stay,
    }
}

/// Navigate left - focus to sidebar (h key, Left arrow)
///
/// From Sidebar: at boundary (already there)
/// From Main: navigate to sidebar
pub fn navigate_left(context: &StateContext) -> NavigationResult {
    match context.focus_pane {
        FocusPane::Main => NavigationResult::Navigate {
            region: NavRegion::Sidebar,
            index: context.selected_index,
        },
        FocusPane::Sidebar => NavigationResult::AtBoundary,
    }
}

/// Navigate right - focus to main (l key, Right arrow)
///
/// From Sidebar: navigate to main
/// From Main: at boundary (already there)
pub fn navigate_right(context: &StateContext) -> NavigationResult {
    match context.focus_pane {
        FocusPane::Sidebar => NavigationResult::Navigate {
            region: NavRegion::Main,
            index: None,
        },
        FocusPane::Main => NavigationResult::AtBoundary,
    }
}

/// Navigate to first item (g key, Home)
///
/// Jumps to the first item in the list.
pub fn navigate_first(context: &StateContext) -> NavigationResult {
    if context.item_count > 0 {
        NavigationResult::Navigate {
            region: NavRegion::Sidebar,
            index: Some(0),
        }
    } else {
        NavigationResult::AtBoundary
    }
}

/// Navigate to last item (G key, End)
///
/// Jumps to the last item in the list.
pub fn navigate_last(context: &StateContext) -> NavigationResult {
    if context.item_count > 0 {
        NavigationResult::Navigate {
            region: NavRegion::Sidebar,
            index: Some(context.item_count - 1),
        }
    } else {
        NavigationResult::AtBoundary
    }
}

/// Calculate new state after navigation
///
/// Applies a navigation result to the current view state to produce
/// a new view state.
///
/// # Arguments
/// * `current` - The current view state
/// * `nav_result` - The navigation result to apply
///
/// # Returns
/// The new view state after applying navigation
pub fn apply_navigation(current: ViewState, nav_result: &NavigationResult) -> ViewState {
    match nav_result {
        NavigationResult::Navigate {
            index: Some(idx), ..
        } => ViewState::ItemSelected { index: *idx },
        NavigationResult::Navigate { index: None, .. } => {
            // Focus change without item selection - keep current state
            current
        }
        NavigationResult::AtBoundary => current,
        NavigationResult::Stay => current,
        NavigationResult::Action(_) => current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a basic state context
    fn state_context(
        selected_index: Option<usize>,
        item_count: usize,
        focus_pane: FocusPane,
    ) -> StateContext {
        StateContext {
            selected_index,
            item_count,
            focus_pane,
            view_mode: crate::core::models::state_machine::ViewMode::Status,
            available_actions: vec![],
        }
    }

    #[test]
    fn test_navigate_down_selects_first_when_none_selected() {
        // GIVEN: No selection and items available (git History mode scenario)
        let context = state_context(None, 10, FocusPane::Sidebar);

        // WHEN: Pressing 'j' to navigate down
        let result = navigate_down(&context);

        // THEN: Should select first commit/item
        assert!(matches!(
            result,
            NavigationResult::Navigate { index: Some(0), .. }
        ));
    }

    #[test]
    fn test_navigate_down_selects_first_via_calculate() {
        // GIVEN: No selection and items available
        let context = state_context(None, 10, FocusPane::Sidebar);

        // WHEN: Using calculate_navigation
        let result = calculate_navigation(NavDirection::Down, &context);

        // THEN: Should select first item
        assert!(matches!(
            result,
            NavigationResult::Navigate { index: Some(0), .. }
        ));
    }

    #[test]
    fn test_navigate_up_selects_last_when_none_selected() {
        // GIVEN: No selection and items available
        let context = state_context(None, 10, FocusPane::Sidebar);

        // WHEN: Pressing 'k' to navigate up
        let result = navigate_up(&context);

        // THEN: Should select last item
        assert!(matches!(
            result,
            NavigationResult::Navigate { index: Some(9), .. }
        ));
    }

    #[test]
    fn test_navigate_up_at_boundary() {
        // GIVEN: Selection at top boundary (index 0)
        let context = state_context(Some(0), 10, FocusPane::Sidebar);

        // WHEN: Pressing 'k' to navigate up
        let result = navigate_up(&context);

        // THEN: Should stay at boundary
        assert_eq!(result, NavigationResult::AtBoundary);
    }

    #[test]
    fn test_navigate_down_at_boundary() {
        // GIVEN: Selection at bottom boundary (last item)
        let context = state_context(Some(9), 10, FocusPane::Sidebar);

        // WHEN: Pressing 'j' to navigate down
        let result = navigate_down(&context);

        // THEN: Should stay at boundary
        assert_eq!(result, NavigationResult::AtBoundary);
    }

    #[test]
    fn test_navigate_down_moves_down() {
        // GIVEN: Selection in the middle
        let context = state_context(Some(2), 10, FocusPane::Sidebar);

        // WHEN: Pressing 'j' to navigate down
        let result = navigate_down(&context);

        // THEN: Should move to next item
        assert!(matches!(
            result,
            NavigationResult::Navigate { index: Some(3), .. }
        ));
    }

    #[test]
    fn test_navigate_up_moves_up() {
        // GIVEN: Selection in the middle
        let context = state_context(Some(5), 10, FocusPane::Sidebar);

        // WHEN: Pressing 'k' to navigate up
        let result = navigate_up(&context);

        // THEN: Should move to previous item
        assert!(matches!(
            result,
            NavigationResult::Navigate { index: Some(4), .. }
        ));
    }

    #[test]
    fn test_navigate_right_from_sidebar_to_main() {
        // GIVEN: Focus in sidebar
        let context = state_context(Some(2), 10, FocusPane::Sidebar);

        // WHEN: Pressing 'l' to navigate right
        let result = navigate_right(&context);

        // THEN: Should navigate to Main pane
        assert!(matches!(
            result,
            NavigationResult::Navigate {
                region: NavRegion::Main,
                ..
            }
        ));

        if let NavigationResult::Navigate { index, .. } = result {
            // When moving to main pane, index should be None
            assert!(index.is_none());
        } else {
            panic!("Expected Navigate result");
        }
    }

    #[test]
    fn test_navigate_right_from_main_stays_at_boundary() {
        // GIVEN: Focus already in Main pane
        let context = state_context(None, 10, FocusPane::Main);

        // WHEN: Pressing 'l' to navigate right
        let result = navigate_right(&context);

        // THEN: Should stay at boundary
        assert_eq!(result, NavigationResult::AtBoundary);
    }

    #[test]
    fn test_navigate_left_from_main_to_sidebar() {
        // GIVEN: Focus in Main pane
        let context = state_context(None, 10, FocusPane::Main);

        // WHEN: Pressing 'h' to navigate left
        let result = navigate_left(&context);

        // THEN: Should navigate to Sidebar
        assert!(matches!(
            result,
            NavigationResult::Navigate {
                region: NavRegion::Sidebar,
                ..
            }
        ));
    }

    #[test]
    fn test_navigate_left_from_sidebar_stays_at_boundary() {
        // GIVEN: Focus in Sidebar
        let context = state_context(Some(2), 10, FocusPane::Sidebar);

        // WHEN: Pressing 'h' to navigate left
        let result = navigate_left(&context);

        // THEN: Should stay at boundary
        assert_eq!(result, NavigationResult::AtBoundary);
    }

    #[test]
    fn test_navigate_first_selects_first_item() {
        // GIVEN: Any selection state
        let context = state_context(Some(5), 10, FocusPane::Sidebar);

        // WHEN: Pressing 'g' or Home
        let result = navigate_first(&context);

        // THEN: Should select first item
        assert!(matches!(
            result,
            NavigationResult::Navigate { index: Some(0), .. }
        ));
    }

    #[test]
    fn test_navigate_first_with_no_items() {
        // GIVEN: No items available
        let context = state_context(None, 0, FocusPane::Sidebar);

        // WHEN: Pressing Home
        let result = navigate_first(&context);

        // THEN: Should be at boundary
        assert_eq!(result, NavigationResult::AtBoundary);
    }

    #[test]
    fn test_navigate_last_selects_last_item() {
        // GIVEN: Any selection state
        let context = state_context(Some(2), 10, FocusPane::Sidebar);

        // WHEN: Pressing 'G' or End
        let result = navigate_last(&context);

        // THEN: Should select last item
        assert!(matches!(
            result,
            NavigationResult::Navigate { index: Some(9), .. }
        ));
    }

    #[test]
    fn test_navigate_last_with_no_items() {
        // GIVEN: No items available
        let context = state_context(None, 0, FocusPane::Sidebar);

        // WHEN: Pressing End
        let result = navigate_last(&context);

        // THEN: Should be at boundary
        assert_eq!(result, NavigationResult::AtBoundary);
    }

    #[test]
    fn test_navigate_down_in_main_pane() {
        // GIVEN: Focus in Main pane
        let context = state_context(None, 10, FocusPane::Main);

        // WHEN: Pressing 'j' to navigate down
        let result = navigate_down(&context);

        // THEN: Should stay (no list navigation in main)
        assert_eq!(result, NavigationResult::Stay);
    }

    #[test]
    fn test_navigate_up_in_main_pane() {
        // GIVEN: Focus in Main pane
        let context = state_context(None, 10, FocusPane::Main);

        // WHEN: Pressing 'k' to navigate up
        let result = navigate_up(&context);

        // THEN: Should stay
        assert_eq!(result, NavigationResult::Stay);
    }

    #[test]
    fn test_navigate_down_with_no_items() {
        // GIVEN: No items available
        let context = state_context(None, 0, FocusPane::Sidebar);

        // WHEN: Pressing 'j' to navigate down
        let result = navigate_down(&context);

        // THEN: Should be at boundary
        assert_eq!(result, NavigationResult::AtBoundary);
    }

    #[test]
    fn test_navigate_up_with_no_items() {
        // GIVEN: No items available
        let context = state_context(None, 0, FocusPane::Sidebar);

        // WHEN: Pressing 'k' to navigate up
        let result = navigate_up(&context);

        // THEN: Should be at boundary
        assert_eq!(result, NavigationResult::AtBoundary);
    }

    // Tests for apply_navigation
    #[test]
    fn test_apply_navigation_with_index() {
        // GIVEN: Current state and navigation result with index
        let current = ViewState::Ready;
        let nav_result = NavigationResult::Navigate {
            region: NavRegion::Sidebar,
            index: Some(2),
        };

        // WHEN: Applying navigation
        let new_state = apply_navigation(current, &nav_result);

        // THEN: Should transition to ItemSelected state
        assert_eq!(new_state, ViewState::ItemSelected { index: 2 });
    }

    #[test]
    fn test_apply_navigation_without_index() {
        // GIVEN: Current state and navigation result without index (focus change)
        let current = ViewState::ItemSelected { index: 1 };
        let nav_result = NavigationResult::Navigate {
            region: NavRegion::Main,
            index: None,
        };

        // WHEN: Applying navigation
        let new_state = apply_navigation(current, &nav_result);

        // THEN: Should keep current state (focus change only)
        assert_eq!(new_state, ViewState::ItemSelected { index: 1 });
    }

    #[test]
    fn test_apply_navigation_at_boundary() {
        // GIVEN: Current state and AtBoundary result
        let current = ViewState::ItemSelected { index: 0 };
        let nav_result = NavigationResult::AtBoundary;

        // WHEN: Applying navigation
        let new_state = apply_navigation(current, &nav_result);

        // THEN: Should keep current state
        assert_eq!(new_state, ViewState::ItemSelected { index: 0 });
    }

    #[test]
    fn test_apply_navigation_stay() {
        // GIVEN: Current state and Stay result
        let current = ViewState::Ready;
        let nav_result = NavigationResult::Stay;

        // WHEN: Applying navigation
        let new_state = apply_navigation(current, &nav_result);

        // THEN: Should keep current state
        assert_eq!(new_state, ViewState::Ready);
    }

    #[test]
    fn test_apply_navigation_action() {
        // GIVEN: Current state and Action result
        let current = ViewState::ItemSelected { index: 2 };
        let nav_result = NavigationResult::Action(ActionId::Select);

        // WHEN: Applying navigation
        let new_state = apply_navigation(current, &nav_result);

        // THEN: Should keep current state
        assert_eq!(new_state, ViewState::ItemSelected { index: 2 });
    }

    // Edge cases
    #[test]
    fn test_navigation_with_single_item() {
        // GIVEN: Only one item
        let context = state_context(Some(0), 1, FocusPane::Sidebar);

        // WHEN: Navigating down
        let result = navigate_down(&context);
        assert_eq!(result, NavigationResult::AtBoundary);

        // WHEN: Navigating up
        let result = navigate_up(&context);
        assert_eq!(result, NavigationResult::AtBoundary);
    }

    #[test]
    fn test_navigation_with_exactly_two_items() {
        // GIVEN: Exactly two items
        let context = state_context(Some(0), 2, FocusPane::Sidebar);

        // WHEN: Navigating down from first
        let result = navigate_down(&context);
        assert!(matches!(
            result,
            NavigationResult::Navigate { index: Some(1), .. }
        ));

        // WHEN: Navigating up from second
        let context = state_context(Some(1), 2, FocusPane::Sidebar);
        let result = navigate_up(&context);
        assert!(matches!(
            result,
            NavigationResult::Navigate { index: Some(0), .. }
        ));
    }
}
