//! State machine executor - Imperative shell for state management
//!
//! This module provides the `StateMachineExecutor` which wraps the pure `StateMachine`
//! from the core and adds:
//!
//! - Thread-safe access via `Arc<Mutex<StateMachine>>`
//! - Callbacks for state changes and action execution
//! - Side effect coordination
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    StateMachineExecutor                      │
//! │                     (Imperative Shell)                       │
//! │                                                             │
//! │  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐│
//! │  │ on_state_change │  │   on_action     │  │ Arc<Mutex>   ││
//! │  │    callback     │  │    callback     │  │ StateMachine ││
//! │  └────────┬────────┘  └────────┬────────┘  └──────┬───────┘│
//! │           │                    │                   │        │
//! │           └────────────────────┼───────────────────┘        │
//! │                                │                            │
//! │                                ▼                            │
//! │           ┌────────────────────────────────────┐            │
//! │           │        Core (Pure Functions)       │            │
//! │           │  - check_guard()                   │            │
//! │           │  - calculate_navigation()          │            │
//! │           │  - apply_navigation()              │            │
//! │           └────────────────────────────────────┘            │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # FC&IS Compliance
//!
//! The executor follows the Functional Core & Imperative Shell pattern:
//! - All logic is delegated to core functions (`check_guard`, `calculate_navigation`)
//! - Side effects (callbacks, state mutations) are isolated in the shell
//! - Core is never aware of the executor's existence

mod git_state_machine;

use parking_lot::Mutex;
use std::sync::Arc;

use crate::core::logic::guards::check_guard;
use crate::core::logic::navigation::{apply_navigation, calculate_navigation};
use crate::core::models::action::{ActionContext, ActionId, ActionResult, GuardResult};
use crate::core::models::navigation::{NavDirection, NavigationResult};
use crate::core::models::state_machine::{StateContext, StateMachine, ViewState};

pub use git_state_machine::{GitCommand, GitStateMachine};

type StateChangeCallback = Box<dyn Fn(&ViewState, &ViewState) + Send + Sync>;

/// Executor for state machines - handles side effects
///
/// This is the imperative shell that wraps the pure state machine
/// and coordinates side effects (callbacks, state persistence).
///
/// # Thread Safety
///
/// The executor uses `Arc<Mutex<StateMachine>>` for thread-safe interior
/// mutability. This allows the executor to be shared across threads while
/// maintaining safe access to the underlying state.
///
/// # Example
///
/// ```ignore
/// use shell::machines::StateMachineExecutor;
/// use core::models::navigation::NavDirection;
///
/// let executor = StateMachineExecutor::new();
/// executor.set_item_count(10);
///
/// // Set up state change callback
/// executor.on_state_change(|old, new| {
///     println!("State changed: {:?} -> {:?}", old, new);
/// });
///
/// // Navigate
/// let result = executor.handle_navigation(NavDirection::Down);
/// ```
/// Executor for state machines - handles side effects
///
/// This is the imperative shell that wraps the pure state machine
/// and coordinates side effects (callbacks, state persistence).
///
/// # Thread Safety
///
/// The executor uses `Arc<Mutex<StateMachine>>` for thread-safe interior
/// mutability. This allows the executor to be shared across threads while
/// maintaining safe access to the underlying state.
///
/// # Example
///
/// ```ignore
/// use shell::machines::StateMachineExecutor;
/// use core::models::navigation::NavDirection;
///
/// let executor = StateMachineExecutor::new();
/// executor.set_item_count(10);
///
/// // Set up state change callback
/// executor.on_state_change(|old, new| {
///     println!("State changed: {:?} -> {:?}", old, new);
/// });
///
/// // Navigate
/// let result = executor.handle_navigation(NavDirection::Down);
/// ```
pub struct StateMachineExecutor {
    /// The state machine (wrapped for interior mutability)
    machine: Arc<Mutex<StateMachine>>,
    /// Callback for state changes: (old_state, new_state)
    on_state_change: Option<StateChangeCallback>,
    /// Callback for action execution
    on_action: Option<Box<dyn Fn(ActionId) + Send + Sync>>,
}

impl std::fmt::Debug for StateMachineExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateMachineExecutor")
            .field("machine", &"<StateMachine>")
            .field("has_state_change_callback", &self.on_state_change.is_some())
            .field("has_action_callback", &self.on_action.is_some())
            .finish()
    }
}

impl StateMachineExecutor {
    /// Create a new executor with initial state
    ///
    /// The executor starts with the state machine in `ViewState::Initial`
    /// and default context.
    pub fn new() -> Self {
        Self {
            machine: Arc::new(Mutex::new(StateMachine::new())),
            on_state_change: None,
            on_action: None,
        }
    }

    /// Create an executor with a specific state machine
    pub fn with_machine(machine: StateMachine) -> Self {
        Self {
            machine: Arc::new(Mutex::new(machine)),
            on_state_change: None,
            on_action: None,
        }
    }

    /// Set state change callback
    ///
    /// The callback is invoked whenever the view state transitions.
    /// It receives both the old and new states for comparison.
    ///
    /// # Arguments
    /// * `callback` - Function called on state change with (old_state, new_state)
    pub fn on_state_change<F>(&mut self, callback: F)
    where
        F: Fn(&ViewState, &ViewState) + Send + Sync + 'static,
    {
        self.on_state_change = Some(Box::new(callback));
    }

    /// Set action callback
    ///
    /// The callback is invoked whenever an action is successfully executed
    /// (i.e., passes guard checks).
    ///
    /// # Arguments
    /// * `callback` - Function called when action executes
    pub fn on_action<F>(&mut self, callback: F)
    where
        F: Fn(ActionId) + Send + Sync + 'static,
    {
        self.on_action = Some(Box::new(callback));
    }

    /// Get current state (read-only)
    ///
    /// Returns a clone of the current view state.
    pub fn current_state(&self) -> ViewState {
        let machine = self.machine.lock();
        machine.current.clone()
    }

    /// Get current context
    ///
    /// Returns a clone of the current state context.
    pub fn context(&self) -> StateContext {
        let machine = self.machine.lock();
        machine.context.clone()
    }

    /// Update context (e.g., when data changes)
    ///
    /// Provides mutable access to the context via a closure.
    ///
    /// # Arguments
    /// * `updater` - Closure that modifies the context
    ///
    /// # Example
    ///
    /// ```ignore
    /// executor.update_context(|ctx| {
    ///     ctx.view_mode = ViewMode::History;
    ///     ctx.focus_pane = FocusPane::Sidebar;
    /// });
    /// ```
    pub fn update_context<F>(&self, updater: F)
    where
        F: FnOnce(&mut StateContext),
    {
        let mut machine = self.machine.lock();
        updater(&mut machine.context);
    }

    /// Update item count (triggers state recalculation if needed)
    ///
    /// When the item count changes, this method also validates that
    /// the selected index is still within bounds. If not, it adjusts
    /// the selection appropriately.
    ///
    /// # Arguments
    /// * `count` - New number of items available
    pub fn set_item_count(&self, count: usize) {
        let mut machine = self.machine.lock();
        machine.context.item_count = count;

        // Validate selected index is still valid
        if let Some(idx) = machine.context.selected_index {
            if idx >= count && count > 0 {
                // Selection is out of bounds, move to last item
                machine.context.selected_index = Some(count - 1);
            } else if count == 0 {
                // No items, clear selection
                machine.context.selected_index = None;
            }
        }
    }

    /// Set selected index directly
    ///
    /// Updates the selected index and transitions the state accordingly.
    /// This is used when selection is set programmatically (not via navigation).
    ///
    /// # Arguments
    /// * `index` - New selection index, or None to clear selection
    pub fn set_selected_index(&self, index: Option<usize>) {
        let mut machine = self.machine.lock();
        let old_state = machine.current.clone();

        machine.context.selected_index = index;

        // Update state based on selection
        let new_state = match index {
            Some(idx) => ViewState::ItemSelected { index: idx },
            None => ViewState::Ready,
        };

        machine.current = new_state.clone();

        // Trigger callback if state changed
        if new_state != old_state {
            drop(machine); // Release lock before callback
            if let Some(ref callback) = self.on_state_change {
                callback(&old_state, &new_state);
            }
        }
    }

    /// Handle navigation key (side effect: may trigger callbacks)
    ///
    /// Calculates the navigation result using the core logic, applies it,
    /// and triggers callbacks if the state changes.
    ///
    /// # Arguments
    /// * `direction` - Navigation direction (Up, Down, Left, Right, etc.)
    ///
    /// # Returns
    /// The navigation result indicating what happened
    pub fn handle_navigation(&self, direction: NavDirection) -> NavigationResult {
        // Calculate navigation using core logic
        let (result, new_state, old_state, new_index) = {
            let machine = self.machine.lock();
            let result = calculate_navigation(direction, &machine.context);
            let new_state = apply_navigation(machine.current.clone(), &result);

            // Extract new index from navigation result
            let new_index = match &result {
                NavigationResult::Navigate {
                    index: Some(idx), ..
                } => Some(*idx),
                _ => None,
            };

            (result, new_state, machine.current.clone(), new_index)
        };

        // Apply changes and trigger callback if state changed
        if new_state != old_state {
            {
                let mut machine = self.machine.lock();
                machine.current = new_state.clone();

                // Update selected index from navigation result
                if let Some(idx) = new_index {
                    machine.context.selected_index = Some(idx);
                }

                // Update focus pane for left/right navigation
                // Note: Focus pane changes are handled by the caller (GitStateMachine)
            }

            // Trigger callback
            if let Some(ref callback) = self.on_state_change {
                callback(&old_state, &new_state);
            }
        }

        result
    }

    /// Execute an action with guard check (side effect: may trigger callbacks)
    ///
    /// First checks if the action is authorized using the guard logic,
    /// then executes it if authorized.
    ///
    /// # Arguments
    /// * `action` - The action to execute
    ///
    /// # Returns
    /// * `ActionResult::Success` - Action executed successfully
    /// * `ActionResult::Denied(GuardError)` - Action blocked by guard
    /// * `ActionResult::Failed` - Action failed during execution
    pub fn execute_action(&self, action: ActionId) -> ActionResult {
        // Build action context and check guard
        let guard_result = {
            let machine = self.machine.lock();
            let action_ctx =
                ActionContext::new(action, machine.current.clone(), machine.context.clone());
            check_guard(&action_ctx)
        };

        match guard_result {
            GuardResult::Denied(error) => ActionResult::Denied(error),
            GuardResult::Authorized => {
                // Guard passed, trigger action callback
                if let Some(ref callback) = self.on_action {
                    callback(action);
                }

                // Update state if needed
                self.update_state_after_action(action);

                ActionResult::Success
            }
        }
    }

    /// Update state after successful action
    ///
    /// Some actions cause state transitions (e.g., Select transitions to ItemSelected).
    fn update_state_after_action(&self, action: ActionId) {
        let (old_state, new_state) = {
            let mut machine = self.machine.lock();
            let old_state = machine.current.clone();

            let new_state = match action {
                ActionId::Select => {
                    if let Some(idx) = machine.context.selected_index {
                        ViewState::ItemSelected { index: idx }
                    } else {
                        machine.current.clone()
                    }
                }
                ActionId::Stage | ActionId::Unstage => {
                    // Stay in selected state after staging/unstaging
                    machine.current.clone()
                }
                ActionId::Confirm | ActionId::Cancel => {
                    // Modal actions - transition depends on current state
                    match &machine.current {
                        ViewState::Modal { parent } => (**parent).clone(),
                        ViewState::Editing { .. } => ViewState::Ready,
                        _ => machine.current.clone(),
                    }
                }
                _ => machine.current.clone(),
            };

            if new_state != old_state {
                machine.current = new_state.clone();
            }

            (old_state, new_state)
        };

        // Trigger callback if state changed
        if new_state != old_state {
            if let Some(ref callback) = self.on_state_change {
                callback(&old_state, &new_state);
            }
        }
    }

    /// Check if an action can be executed (dry run)
    ///
    /// Returns true if the action passes all guard checks.
    /// This is useful for UI to show enabled/disabled states.
    ///
    /// # Arguments
    /// * `action` - The action to check
    ///
    /// # Returns
    /// true if the action is authorized, false otherwise
    pub fn can_execute(&self, action: ActionId) -> bool {
        let machine = self.machine.lock();
        let action_ctx =
            ActionContext::new(action, machine.current.clone(), machine.context.clone());

        matches!(check_guard(&action_ctx), GuardResult::Authorized)
    }

    /// Get list of available actions in current state
    ///
    /// Returns the list of actions that are available in the current
    /// view state (not accounting for guard checks).
    pub fn available_actions(&self) -> Vec<ActionId> {
        let machine = self.machine.lock();
        machine.available_actions()
    }

    /// Get the underlying Arc for sharing across threads
    ///
    /// This allows cloning the executor's internal machine reference
    /// for use in async contexts.
    pub fn machine_arc(&self) -> Arc<Mutex<StateMachine>> {
        Arc::clone(&self.machine)
    }
}

impl Default for StateMachineExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::state_machine::{FocusPane, ViewMode};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_executor_new() {
        let executor = StateMachineExecutor::new();
        assert_eq!(executor.current_state(), ViewState::Initial);
    }

    #[test]
    fn test_executor_set_item_count() {
        let executor = StateMachineExecutor::new();
        executor.set_item_count(10);

        let context = executor.context();
        assert_eq!(context.item_count, 10);
    }

    #[test]
    fn test_executor_set_item_count_validates_selection() {
        let executor = StateMachineExecutor::new();
        executor.set_item_count(10);
        executor.set_selected_index(Some(8));

        // Reduce count below current selection
        executor.set_item_count(5);

        // Selection should be adjusted to last item
        let context = executor.context();
        assert_eq!(context.selected_index, Some(4));
    }

    #[test]
    fn test_executor_set_item_count_clears_selection_when_zero() {
        let executor = StateMachineExecutor::new();
        executor.set_item_count(10);
        executor.set_selected_index(Some(5));

        // Set count to 0
        executor.set_item_count(0);

        // Selection should be cleared
        let context = executor.context();
        assert_eq!(context.selected_index, None);
    }

    #[test]
    fn test_executor_set_selected_index() {
        let executor = StateMachineExecutor::new();
        executor.set_item_count(10);
        executor.set_selected_index(Some(3));

        assert_eq!(
            executor.current_state(),
            ViewState::ItemSelected { index: 3 }
        );
        assert_eq!(executor.context().selected_index, Some(3));
    }

    #[test]
    fn test_executor_set_selected_index_none() {
        let executor = StateMachineExecutor::new();
        executor.set_item_count(10);
        executor.set_selected_index(Some(3));
        executor.set_selected_index(None);

        assert_eq!(executor.current_state(), ViewState::Ready);
        assert_eq!(executor.context().selected_index, None);
    }

    #[test]
    fn test_executor_navigation_callback() {
        let executor = StateMachineExecutor::new();
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let mut executor = executor;
        executor.on_state_change(move |_old, _new| {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
        });

        executor.set_item_count(10);
        executor.handle_navigation(NavDirection::Down);

        // Callback should have been triggered for state change
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_executor_navigation_down_selects_first() {
        let executor = StateMachineExecutor::new();
        executor.set_item_count(10);

        let result = executor.handle_navigation(NavDirection::Down);

        // Should select first item
        assert!(matches!(
            result,
            NavigationResult::Navigate { index: Some(0), .. }
        ));
        assert_eq!(executor.context().selected_index, Some(0));
    }

    #[test]
    fn test_executor_navigation_down_moves_down() {
        let executor = StateMachineExecutor::new();
        executor.set_item_count(10);
        executor.set_selected_index(Some(2));

        let result = executor.handle_navigation(NavDirection::Down);

        // Should move to next item
        assert!(matches!(
            result,
            NavigationResult::Navigate { index: Some(3), .. }
        ));
        assert_eq!(executor.context().selected_index, Some(3));
    }

    #[test]
    fn test_executor_navigation_up_moves_up() {
        let executor = StateMachineExecutor::new();
        executor.set_item_count(10);
        executor.set_selected_index(Some(5));

        let result = executor.handle_navigation(NavDirection::Up);

        // Should move to previous item
        assert!(matches!(
            result,
            NavigationResult::Navigate { index: Some(4), .. }
        ));
        assert_eq!(executor.context().selected_index, Some(4));
    }

    #[test]
    fn test_executor_can_execute_stage_without_selection() {
        let executor = StateMachineExecutor::new();
        executor.set_item_count(5);
        executor.update_context(|ctx| {
            ctx.view_mode = ViewMode::Status;
            ctx.focus_pane = FocusPane::Sidebar;
        });

        // Without selection, Stage should not be executable
        assert!(!executor.can_execute(ActionId::Stage));
    }

    #[test]
    fn test_executor_can_execute_stage_with_selection() {
        let executor = StateMachineExecutor::new();
        executor.set_item_count(5);
        executor.set_selected_index(Some(2));
        executor.update_context(|ctx| {
            ctx.view_mode = ViewMode::Status;
            ctx.focus_pane = FocusPane::Sidebar;
        });

        // With selection, Stage should be executable
        assert!(executor.can_execute(ActionId::Stage));
    }

    #[test]
    fn test_executor_execute_action_denied() {
        let executor = StateMachineExecutor::new();
        executor.set_item_count(5);
        executor.update_context(|ctx| {
            ctx.view_mode = ViewMode::Status;
            ctx.focus_pane = FocusPane::Sidebar;
        });

        // No selection - should be denied
        let result = executor.execute_action(ActionId::Stage);
        assert!(matches!(result, ActionResult::Denied(_)));
    }

    #[test]
    fn test_executor_execute_action_success() {
        let executor = StateMachineExecutor::new();
        executor.set_item_count(5);
        executor.set_selected_index(Some(0));
        executor.update_context(|ctx| {
            ctx.view_mode = ViewMode::Status;
            ctx.focus_pane = FocusPane::Sidebar;
        });

        // With selection - should succeed
        let result = executor.execute_action(ActionId::Stage);
        assert_eq!(result, ActionResult::Success);
    }

    #[test]
    fn test_executor_action_callback() {
        let executor = StateMachineExecutor::new();
        let action_called = Arc::new(AtomicUsize::new(0));
        let action_called_clone = action_called.clone();

        let mut executor = executor;
        executor.on_action(move |_action| {
            action_called_clone.fetch_add(1, Ordering::SeqCst);
        });

        executor.set_item_count(5);
        executor.set_selected_index(Some(0));
        executor.update_context(|ctx| {
            ctx.view_mode = ViewMode::Status;
            ctx.focus_pane = FocusPane::Sidebar;
        });

        executor.execute_action(ActionId::Stage);

        assert_eq!(action_called.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_executor_available_actions() {
        let executor = StateMachineExecutor::new();
        let actions = executor.available_actions();

        assert!(actions.contains(&ActionId::Refresh));
        assert!(actions.contains(&ActionId::NavigateDown));
    }

    #[test]
    fn test_executor_update_context() {
        let executor = StateMachineExecutor::new();
        executor.update_context(|ctx| {
            ctx.view_mode = ViewMode::History;
            ctx.focus_pane = FocusPane::Main;
        });

        let context = executor.context();
        assert_eq!(context.view_mode, ViewMode::History);
        assert_eq!(context.focus_pane, FocusPane::Main);
    }
}
