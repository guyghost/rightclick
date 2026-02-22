# OpenSpec: State Manager avec Autorisation d'Actions et Navigation Clavier

## Metadata

```yaml
spec_version: "1.0"
feature_id: "state-manager"
author: "system"
created: "2025-01-12"
status: complete
tdd: true
priority: high
```

## 1. Objective

Créer un gestionnaire de state centralisé qui :
1. **Garantit** que les actions ne sont exécutées que lorsqu'elles sont autorisées (guards)
2. **Facilite** la navigation au clavier avec un système de focus cohérent
3. **Résout** le problème de navigation dans l'historique git (onglet git - navigation entre commits impossible)

## 2. Problem Analysis

### 2.1 Current Issues

| Issue | Location | Description |
|-------|----------|-------------|
| No action guards | `plugin.rs` | Actions like `StageFile` execute without checking if a file is selected |
| Broken commit navigation | `state.rs`, `plugin.rs` | `j/k` keys don't navigate commits properly in History mode |
| Focus management scattered | Multiple files | Focus logic is split between `FocusPane`, `FocusContext`, and plugin state |
| No centralized navigation | N/A | Each plugin implements its own navigation logic |
| State mutations uncontrolled | `plugin.rs` | State changes happen directly without validation |

### 2.2 Root Cause

The current architecture has:
- **State** (`PluginState`) mixed with **behavior** (navigation logic)
- **Actions** executed without checking preconditions
- **Navigation** hardcoded per plugin without a consistent system

## 3. Proposed Architecture (FC&IS)

```
src/
├── core/
│   ├── models/
│   │   ├── state_machine.rs    # State machine definitions (pure)
│   │   ├── navigation.rs       # Navigation tree & focus models (pure)
│   │   └── action.rs           # Action & guard definitions (pure)
│   └── logic/
│       ├── state_machine.rs    # Pure state transitions
│       ├── guards.rs           # Action authorization logic
│       └── navigation.rs       # Navigation calculations (next/prev/focus)
│
└── shell/
    └── machines/
        ├── mod.rs              # State machine executor (imperative)
        └── git_state_machine.rs # Git-specific state machine instance
```

**Rule: Shell calls Core. Core NEVER calls Shell. Core IGNORES Shell exists.**

## 4. Functional Core (Pure)

### 4.1 Core Models

#### `core/models/state_machine.rs`

```rust
//! State Machine Models - Pure definitions

use crate::core::models::action::{Action, ActionResult, GuardError};

/// A state in the state machine
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViewState {
    /// Initial/loading state
    Initial,
    /// Ready for user interaction
    Ready,
    /// Item selected, details visible
    ItemSelected { index: usize },
    /// Editing/acting on an item
    Editing { index: usize },
    /// Modal/dialog open
    Modal { parent: Box<ViewState> },
    /// Error state
    Error { message: String, previous: Box<ViewState> },
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
    /// Available actions in current state
    pub available_actions: Vec<ActionId>,
}

/// Transition result
#[derive(Clone, Debug, PartialEq)]
pub enum TransitionResult {
    /// Transition successful, new state
    Success(ViewState),
    /// Transition denied by guard
    Denied { reason: String, current: ViewState },
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

impl StateMachine {
    /// Create a new state machine in Initial state
    pub fn new() -> Self {
        Self {
            current: ViewState::Initial,
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
            ],
            ViewState::ItemSelected { .. } => vec![
                ActionId::NavigateUp,
                ActionId::NavigateDown,
                ActionId::NavigateLeft,
                ViewState::Editing { index } => vec![
                    ActionId::Confirm,
                    ActionId::Cancel,
                ],
                ViewState::Modal { .. } => vec![
                    ActionId::Confirm,
                    ActionId::Cancel,
                    ActionId::NavigateUp,
                    ActionId::NavigateDown,
                ],
                ViewState::Error { .. } => vec![
                    ActionId::Confirm, // Acknowledge error
                    ActionId::Back,
                ],
            }
        }
    }

    /// Check if an action is available in current state
    pub fn can_execute(&self, action: ActionId) -> bool {
        self.available_actions().contains(&action)
    }
}
```

#### `core/models/navigation.rs`

```rust
//! Navigation Models - Pure definitions for keyboard navigation

use crate::core::models::state_machine::FocusPane;

/// A navigable region in the UI
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NavRegion {
    /// Main content area
    Main,
    /// Sidebar/left panel
    Sidebar,
    /// Header/top bar
    Header,
    /// Footer/status bar
    Footer,
    /// Modal/dialog (overlay)
    Modal,
}

/// Navigation direction
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavDirection {
    Up,
    Down,
    Left,
    Right,
    Next,    // Tab order
    Previous, // Shift+Tab
    First,   // Home key
    Last,    // End key
}

/// Navigation tree node
#[derive(Clone, Debug, PartialEq)]
pub struct NavNode {
    /// Region identifier
    pub region: NavRegion,
    /// Whether this node can receive focus
    pub focusable: bool,
    /// Number of items in this region (for list navigation)
    pub item_count: usize,
    /// Currently selected item (if any)
    pub selected_index: Option<usize>,
    /// Parent node (for hierarchical navigation)
    pub parent: Option<Box<NavNode>>,
    /// Child nodes
    pub children: Vec<NavNode>,
}

/// Navigation result
#[derive(Clone, Debug, PartialEq)]
pub enum NavigationResult {
    /// Navigate to specific region and item
    Navigate { region: NavRegion, index: Option<usize> },
    /// Stay in current position
    Stay,
    /// Action triggered
    Action(ActionId),
    /// No valid target (at boundary)
    AtBoundary,
}

/// Navigation tree for a view
#[derive(Clone, Debug)]
pub struct NavigationTree {
    /// Root node
    pub root: NavNode,
    /// Currently focused node
    pub focused: NavRegion,
    /// Focus history for "back" navigation
    pub history: Vec<NavRegion>,
}

impl NavigationTree {
    /// Create a new navigation tree for git status view
    pub fn git_status() -> Self {
        let sidebar = NavNode {
            region: NavRegion::Sidebar,
            focusable: true,
            item_count: 0, // Set at runtime
            selected_index: None,
            parent: None,
            children: vec![],
        };

        let main = NavNode {
            region: NavRegion::Main,
            focusable: true,
            item_count: 1, // Single content area
            selected_index: Some(0),
            parent: None,
            children: vec![],
        };

        let root = NavNode {
            region: NavRegion::Main, // Root placeholder
            focusable: false,
            item_count: 0,
            selected_index: None,
            parent: None,
            children: vec![sidebar, main],
        };

        Self {
            root,
            focused: NavRegion::Sidebar,
            history: vec![],
        }
    }

    /// Calculate next focus target
    pub fn navigate(&self, direction: NavDirection) -> NavigationResult {
        // Pure function: given current state and direction, return result
        match (self.focused, direction) {
            (NavRegion::Sidebar, NavDirection::Right) => {
                NavigationResult::Navigate { 
                    region: NavRegion::Main, 
                    index: None 
                }
            }
            (NavRegion::Main, NavDirection::Left) => {
                NavigationResult::Navigate { 
                    region: NavRegion::Sidebar, 
                    index: None 
                }
            }
            (NavRegion::Sidebar, NavDirection::Up) => {
                // Navigate up in sidebar list
                NavigationResult::Action(ActionId::NavigateUp)
            }
            (NavRegion::Sidebar, NavDirection::Down) => {
                // Navigate down in sidebar list
                NavigationResult::Action(ActionId::NavigateDown)
            }
            _ => NavigationResult::Stay,
        }
    }
}
```

#### `core/models/action.rs`

```rust
//! Action models and guards - Pure definitions

/// Unique action identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionId {
    // Navigation
    NavigateUp,
    NavigateDown,
    NavigateLeft,
    NavigateRight,
    Select,
    Back,
    
    // View actions
    Refresh,
    SwitchMode(ViewMode),
    
    // Git-specific
    Stage,
    Unstage,
    Diff,
    Commit,
    Push,
    Pull,
    
    // Modal
    Confirm,
    Cancel,
}

/// Guard error - why an action was denied
#[derive(Clone, Debug, PartialEq)]
pub enum GuardError {
    /// No item selected
    NoSelection,
    /// Item selected but action not applicable
    InvalidSelection { reason: String },
    /// Wrong view mode for this action
    WrongViewMode { current: ViewMode, required: ViewMode },
    /// Wrong focus pane for this action
    WrongFocus { current: FocusPane, required: FocusPane },
    /// State doesn't allow this action
    InvalidState { current: ViewState, action: ActionId },
    /// Custom guard failed
    Custom { message: String },
}

impl std::fmt::Display for GuardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GuardError::NoSelection => write!(f, "No item selected"),
            GuardError::InvalidSelection { reason } => write!(f, "Invalid selection: {}", reason),
            GuardError::WrongViewMode { current, required } => {
                write!(f, "Action requires {:?} mode, currently in {:?}", required, current)
            }
            GuardError::WrongFocus { current, required } => {
                write!(f, "Action requires {:?} focus, currently in {:?}", required, current)
            }
            GuardError::InvalidState { current, action } => {
                write!(f, "Cannot {:?} in {:?} state", action, current)
            }
            GuardError::Custom { message } => write!(f, "{}", message),
        }
    }
}

/// Guard check result
#[derive(Clone, Debug, PartialEq)]
pub enum GuardResult {
    /// Action is authorized
    Authorized,
    /// Action is denied with reason
    Denied(GuardError),
}

/// Action with context
#[derive(Clone, Debug)]
pub struct ActionContext {
    /// Action to execute
    pub action: ActionId,
    /// Current state
    pub state: ViewState,
    /// Current context
    pub context: StateContext,
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
    /// Action failed
    Failed { error: String },
}
```

### 4.2 Core Logic

#### `core/logic/guards.rs`

```rust
//! Action guards - Pure authorization logic

use crate::core::models::action::{ActionContext, ActionId, GuardError, GuardResult};
use crate::core::models::state_machine::{StateContext, ViewState, ViewMode, FocusPane};

/// Check if an action is authorized (pure function)
pub fn check_guard(ctx: &ActionContext) -> GuardResult {
    match ctx.action {
        // Navigation is always allowed
        ActionId::NavigateUp 
        | ActionId::NavigateDown 
        | ActionId::NavigateLeft 
        | ActionId::NavigateRight => GuardResult::Authorized,
        
        // Actions requiring selection
        ActionId::Stage | ActionId::Unstage | ActionId::Diff => {
            check_requires_selection(ctx, |ctx| {
                // Additional check: must be in Status view
                if ctx.context.view_mode != ViewMode::Status {
                    return GuardResult::Denied(GuardError::WrongViewMode {
                        current: ctx.context.view_mode,
                        required: ViewMode::Status,
                    });
                }
                // Check focus is in sidebar
                if ctx.context.focus_pane != FocusPane::Sidebar {
                    return GuardResult::Denied(GuardError::WrongFocus {
                        current: ctx.context.focus_pane,
                        required: FocusPane::Sidebar,
                    });
                }
                GuardResult::Authorized
            })
        }
        
        // Commit action requires items staged
        ActionId::Commit => {
            // Additional logic: check if there are staged files
            // This would need context about staged files
            GuardResult::Authorized
        }
        
        // View mode switching
        ActionId::SwitchMode(mode) => {
            // Always allow mode switching
            GuardResult::Authorized
        }
        
        // Refresh always allowed
        ActionId::Refresh => GuardResult::Authorized,
        
        // Modal actions only in modal state
        ActionId::Confirm | ActionId::Cancel => {
            match ctx.state {
                ViewState::Modal { .. } | ViewState::Editing { .. } => GuardResult::Authorized,
                _ => GuardResult::Denied(GuardError::InvalidState {
                    current: ctx.state.clone(),
                    action: ctx.action,
                }),
            }
        }
        
        _ => GuardResult::Authorized,
    }
}

/// Helper: check if an action requires a selection
fn check_requires_selection<F>(ctx: &ActionContext, additional_checks: F) -> GuardResult 
where 
    F: FnOnce(&ActionContext) -> GuardResult 
{
    if ctx.context.selected_index.is_none() {
        return GuardResult::Denied(GuardError::NoSelection);
    }
    
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

    #[test]
    fn test_stage_requires_selection() {
        let ctx = ActionContext {
            action: ActionId::Stage,
            state: ViewState::Ready,
            context: StateContext {
                selected_index: None,
                view_mode: ViewMode::Status,
                focus_pane: FocusPane::Sidebar,
                ..Default::default()
            },
        };
        
        let result = check_guard(&ctx);
        assert!(matches!(result, GuardResult::Denied(GuardError::NoSelection)));
    }

    #[test]
    fn test_stage_requires_status_mode() {
        let ctx = ActionContext {
            action: ActionId::Stage,
            state: ViewState::ItemSelected { index: 0 },
            context: StateContext {
                selected_index: Some(0),
                view_mode: ViewMode::History,
                focus_pane: FocusPane::Sidebar,
                item_count: 5,
                ..Default::default()
            },
        };
        
        let result = check_guard(&ctx);
        assert!(matches!(result, GuardResult::Denied(GuardError::WrongViewMode { .. })));
    }

    #[test]
    fn test_stage_authorized_when_valid() {
        let ctx = ActionContext {
            action: ActionId::Stage,
            state: ViewState::ItemSelected { index: 0 },
            context: StateContext {
                selected_index: Some(0),
                view_mode: ViewMode::Status,
                focus_pane: FocusPane::Sidebar,
                item_count: 5,
                ..Default::default()
            },
        };
        
        let result = check_guard(&ctx);
        assert_eq!(result, GuardResult::Authorized);
    }
}
```

#### `core/logic/navigation.rs`

```rust
//! Navigation logic - Pure calculations for keyboard navigation

use crate::core::models::navigation::{NavDirection, NavigationResult, NavRegion};
use crate::core::models::state_machine::{StateContext, ViewState, ViewMode};
use crate::core::models::action::ActionId;

/// Calculate navigation result (pure function)
pub fn calculate_navigation(
    direction: NavDirection,
    context: &StateContext,
) -> NavigationResult {
    match direction {
        NavDirection::Up => navigate_up(context),
        NavDirection::Down => navigate_down(context),
        NavDirection::Left => navigate_left(context),
        NavDirection::Right => navigate_right(context),
        NavDirection::First => navigate_first(context),
        NavDirection::Last => navigate_last(context),
        _ => NavigationResult::Stay,
    }
}

/// Navigate up in current region
fn navigate_up(context: &StateContext) -> NavigationResult {
    match context.focus_pane {
        FocusPane::Sidebar => {
            // Navigate up in list
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
                // Select last item if nothing selected
                NavigationResult::Navigate {
                    region: NavRegion::Sidebar,
                    index: Some(context.item_count - 1),
                }
            } else {
                NavigationResult::AtBoundary
            }
        }
        FocusPane::Main => {
            // In main pane, up could scroll or navigate
            NavigationResult::Stay
        }
    }
}

/// Navigate down in current region
fn navigate_down(context: &StateContext) -> NavigationResult {
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
                // Select first item if nothing selected
                NavigationResult::Navigate {
                    region: NavRegion::Sidebar,
                    index: Some(0),
                }
            } else {
                NavigationResult::AtBoundary
            }
        }
        FocusPane::Main => {
            NavigationResult::Stay
        }
    }
}

/// Navigate left (focus to sidebar)
fn navigate_left(context: &StateContext) -> NavigationResult {
    match context.focus_pane {
        FocusPane::Main => NavigationResult::Navigate {
            region: NavRegion::Sidebar,
            index: context.selected_index,
        },
        FocusPane::Sidebar => NavigationResult::AtBoundary,
    }
}

/// Navigate right (focus to main)
fn navigate_right(context: &StateContext) -> NavigationResult {
    match context.focus_pane {
        FocusPane::Sidebar => NavigationResult::Navigate {
            region: NavRegion::Main,
            index: None,
        },
        FocusPane::Main => NavigationResult::AtBoundary,
    }
}

/// Navigate to first item
fn navigate_first(context: &StateContext) -> NavigationResult {
    if context.item_count > 0 {
        NavigationResult::Navigate {
            region: NavRegion::Sidebar,
            index: Some(0),
        }
    } else {
        NavigationResult::AtBoundary
    }
}

/// Navigate to last item
fn navigate_last(context: &StateContext) -> NavigationResult {
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
pub fn apply_navigation(
    current: ViewState,
    nav_result: &NavigationResult,
) -> ViewState {
    match nav_result {
        NavigationResult::Navigate { index: Some(idx), .. } => {
            ViewState::ItemSelected { index: *idx }
        }
        NavigationResult::Navigate { index: None, .. } => {
            // Focus change without item selection
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

    #[test]
    fn test_navigate_down_selects_first() {
        let context = StateContext {
            selected_index: None,
            item_count: 5,
            focus_pane: FocusPane::Sidebar,
            ..Default::default()
        };
        
        let result = navigate_down(&context);
        
        assert!(matches!(result, NavigationResult::Navigate { index: Some(0), .. }));
    }

    #[test]
    fn test_navigate_down_moves_down() {
        let context = StateContext {
            selected_index: Some(2),
            item_count: 5,
            focus_pane: FocusPane::Sidebar,
            ..Default::default()
        };
        
        let result = navigate_down(&context);
        
        assert!(matches!(result, NavigationResult::Navigate { index: Some(3), .. }));
    }

    #[test]
    fn test_navigate_down_at_boundary() {
        let context = StateContext {
            selected_index: Some(4),
            item_count: 5,
            focus_pane: FocusPane::Sidebar,
            ..Default::default()
        };
        
        let result = navigate_down(&context);
        
        assert_eq!(result, NavigationResult::AtBoundary);
    }

    #[test]
    fn test_navigate_right_from_sidebar() {
        let context = StateContext {
            selected_index: Some(2),
            focus_pane: FocusPane::Sidebar,
            ..Default::default()
        };
        
        let result = navigate_right(&context);
        
        assert!(matches!(result, NavigationResult::Navigate { 
            region: NavRegion::Main, 
            .. 
        }));
    }
}
```

## 5. Imperative Shell (I/O, Side Effects)

### 5.1 State Machine Executor

#### `shell/machines/mod.rs`

```rust
//! State machine executor - Imperative shell for state management

use std::sync::{Arc, Mutex};

use crate::core::logic::guards::check_guard;
use crate::core::logic::navigation::{apply_navigation, calculate_navigation};
use crate::core::models::action::{ActionContext, ActionId, ActionResult, GuardResult};
use crate::core::models::navigation::{NavDirection, NavigationResult, NavRegion};
use crate::core::models::state_machine::{StateContext, StateMachine, ViewState};

/// Executor for state machines - handles side effects
pub struct StateMachineExecutor {
    /// The state machine (wrapped for interior mutability)
    machine: Arc<Mutex<StateMachine>>,
    /// Callback for state changes
    on_state_change: Option<Box<dyn Fn(&ViewState, &ViewState) + Send>>,
    /// Callback for action execution
    on_action: Option<Box<dyn Fn(ActionId) + Send>>,
}

impl StateMachineExecutor {
    /// Create a new executor with initial state
    pub fn new() -> Self {
        Self {
            machine: Arc::new(Mutex::new(StateMachine::new())),
            on_state_change: None,
            on_action: None,
        }
    }

    /// Set state change callback
    pub fn on_state_change<F>(&mut self, callback: F)
    where
        F: Fn(&ViewState, &ViewState) + Send + 'static,
    {
        self.on_state_change = Some(Box::new(callback));
    }

    /// Set action callback
    pub fn on_action<F>(&mut self, callback: F)
    where
        F: Fn(ActionId) + Send + 'static,
    {
        self.on_action = Some(Box::new(callback));
    }

    /// Get current state (read-only)
    pub fn current_state(&self) -> ViewState {
        let machine = self.machine.lock().unwrap();
        machine.current.clone()
    }

    /// Get current context
    pub fn context(&self) -> StateContext {
        let machine = self.machine.lock().unwrap();
        machine.context.clone()
    }

    /// Update context (e.g., when data changes)
    pub fn update_context<F>(&self, updater: F)
    where
        F: FnOnce(&mut StateContext),
    {
        let mut machine = self.machine.lock().unwrap();
        updater(&mut machine.context);
    }

    /// Update item count (triggers state recalculation if needed)
    pub fn set_item_count(&self, count: usize) {
        let mut machine = self.machine.lock().unwrap();
        machine.context.item_count = count;
        
        // Validate selected index is still valid
        if let Some(idx) = machine.context.selected_index {
            if idx >= count && count > 0 {
                machine.context.selected_index = Some(count - 1);
            } else if count == 0 {
                machine.context.selected_index = None;
            }
        }
    }

    /// Set selected index directly
    pub fn set_selected_index(&self, index: Option<usize>) {
        let mut machine = self.machine.lock().unwrap();
        machine.context.selected_index = index;
        
        // Update state based on selection
        machine.current = match index {
            Some(idx) => ViewState::ItemSelected { index: idx },
            None => ViewState::Ready,
        };
    }

    /// Handle navigation key (side effect: may trigger callbacks)
    pub fn handle_navigation(&self, direction: NavDirection) -> NavigationResult {
        let machine = self.machine.lock().unwrap();
        let result = calculate_navigation(direction, &machine.context);
        
        // Apply navigation result
        let new_state = apply_navigation(machine.current.clone(), &result);
        
        if new_state != machine.current {
            let old_state = machine.current.clone();
            drop(machine); // Release lock before callback
            
            // Update state
            {
                let mut machine = self.machine.lock().unwrap();
                machine.current = new_state.clone();
                
                // Update selected index from navigation result
                if let NavigationResult::Navigate { index: Some(idx), .. } = &result {
                    machine.context.selected_index = Some(*idx);
                }
            }
            
            // Trigger callback
            if let Some(ref callback) = self.on_state_change {
                callback(&old_state, &new_state);
            }
        }
        
        result
    }

    /// Execute an action with guard check (side effect: may trigger callbacks)
    pub fn execute_action(&self, action: ActionId) -> ActionResult {
        let machine = self.machine.lock().unwrap();
        
        // Build action context
        let action_ctx = ActionContext {
            action,
            state: machine.current.clone(),
            context: machine.context.clone(),
        };
        
        // Check guard
        let guard_result = check_guard(&action_ctx);
        
        match guard_result {
            GuardResult::Denied(error) => {
                return ActionResult::Denied(error);
            }
            GuardResult::Authorized => {
                // Guard passed, proceed with execution
                drop(machine);
                
                // Trigger action callback
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
    fn update_state_after_action(&self, action: ActionId) {
        let mut machine = self.machine.lock().unwrap();
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
                // Stay in selected state
                machine.current.clone()
            }
            _ => machine.current.clone(),
        };
        
        if new_state != old_state {
            machine.current = new_state.clone();
            drop(machine);
            
            if let Some(ref callback) = self.on_state_change {
                callback(&old_state, &new_state);
            }
        }
    }

    /// Check if an action can be executed (dry run)
    pub fn can_execute(&self, action: ActionId) -> bool {
        let machine = self.machine.lock().unwrap();
        let action_ctx = ActionContext {
            action,
            state: machine.current.clone(),
            context: machine.context.clone(),
        };
        
        matches!(check_guard(&action_ctx), GuardResult::Authorized)
    }

    /// Get list of available actions in current state
    pub fn available_actions(&self) -> Vec<ActionId> {
        let machine = self.machine.lock().unwrap();
        machine.available_actions()
    }
}

impl Default for StateMachineExecutor {
    fn default() -> Self {
        Self::new()
    }
}
```

### 5.2 Git Plugin Integration

#### `shell/machines/git_state_machine.rs`

```rust
//! Git-specific state machine integration

use std::path::PathBuf;

use crate::core::models::action::ActionId;
use crate::core::models::navigation::NavDirection;
use crate::core::models::state_machine::{FocusPane, StateContext, ViewMode, ViewState};
use crate::event::Event;
use crate::keymap::FocusContext;
use crate::plugin::Command as PluginCommandTrait;
use crate::shell::machines::StateMachineExecutor;

/// Git plugin state machine wrapper
pub struct GitStateMachine {
    /// Underlying executor
    executor: StateMachineExecutor,
    /// Repository path
    repo_path: PathBuf,
    /// Current view mode for context sync
    current_mode: ViewMode,
}

impl GitStateMachine {
    /// Create a new git state machine
    pub fn new(repo_path: PathBuf) -> Self {
        let executor = StateMachineExecutor::new();
        
        // Set up callbacks
        // ...
        
        Self {
            executor,
            repo_path,
            current_mode: ViewMode::Status,
        }
    }

    /// Initialize with data
    pub fn initialize(&self, item_count: usize, view_mode: ViewMode) {
        self.executor.set_item_count(item_count);
        self.executor.update_context(|ctx| {
            ctx.view_mode = view_mode;
            ctx.focus_pane = FocusPane::Sidebar;
        });
    }

    /// Handle key event - THIS FIXES THE GIT NAVIGATION ISSUE
    pub fn handle_key(&self, key: &str) -> Vec<GitCommand> {
        let mut commands = Vec::new();
        
        // Map keys to navigation or actions
        match key {
            "j" | "Down" => {
                let result = self.executor.handle_navigation(NavDirection::Down);
                if let crate::core::models::navigation::NavigationResult::Navigate { index: Some(idx), .. } = result {
                    commands.push(GitCommand::SelectIndex(idx));
                }
            }
            "k" | "Up" => {
                let result = self.executor.handle_navigation(NavDirection::Up);
                if let crate::core::models::navigation::NavigationResult::Navigate { index: Some(idx), .. } = result {
                    commands.push(GitCommand::SelectIndex(idx));
                }
            }
            "h" | "Left" => {
                self.executor.handle_navigation(NavDirection::Left);
                commands.push(GitCommand::SetFocus(FocusPane::Sidebar));
            }
            "l" | "Right" => {
                self.executor.handle_navigation(NavDirection::Right);
                commands.push(GitCommand::SetFocus(FocusPane::Main));
            }
            "g" | "Home" => {
                let result = self.executor.handle_navigation(NavDirection::First);
                if let crate::core::models::navigation::NavigationResult::Navigate { index: Some(idx), .. } = result {
                    commands.push(GitCommand::SelectIndex(idx));
                }
            }
            "G" | "End" => {
                let result = self.executor.handle_navigation(NavDirection::Last);
                if let crate::core::models::navigation::NavigationResult::Navigate { index: Some(idx), .. } = result {
                    commands.push(GitCommand::SelectIndex(idx));
                }
            }
            "s" => {
                // Check guard before executing
                if self.executor.can_execute(ActionId::Stage) {
                    commands.push(GitCommand::ExecuteAction(ActionId::Stage));
                }
            }
            "u" => {
                if self.executor.can_execute(ActionId::Unstage) {
                    commands.push(GitCommand::ExecuteAction(ActionId::Unstage));
                }
            }
            "H" => {
                self.executor.update_context(|ctx| {
                    ctx.view_mode = ViewMode::History;
                });
                commands.push(GitCommand::SwitchMode(ViewMode::History));
                commands.push(GitCommand::LoadCommits);
            }
            "S" => {
                self.executor.update_context(|ctx| {
                    ctx.view_mode = ViewMode::Status;
                });
                commands.push(GitCommand::SwitchMode(ViewMode::Status));
            }
            _ => {}
        }
        
        commands
    }

    /// Update after data changes
    pub fn update_items(&self, count: usize) {
        self.executor.set_item_count(count);
    }

    /// Get current selection
    pub fn selected_index(&self) -> Option<usize> {
        self.executor.context().selected_index
    }

    /// Set focus pane
    pub fn set_focus_pane(&self, pane: FocusPane) {
        self.executor.update_context(|ctx| {
            ctx.focus_pane = pane;
        });
    }

    /// Get current focus pane
    pub fn focus_pane(&self) -> FocusPane {
        self.executor.context().focus_pane
    }

    /// Check if action is available
    pub fn is_action_available(&self, action: ActionId) -> bool {
        self.executor.can_execute(action)
    }
}

/// Git-specific commands produced by the state machine
#[derive(Debug, Clone, PartialEq)]
pub enum GitCommand {
    /// Select item at index
    SelectIndex(usize),
    /// Set focus pane
    SetFocus(FocusPane),
    /// Execute an action
    ExecuteAction(ActionId),
    /// Switch view mode
    SwitchMode(ViewMode),
    /// Load commits for history view
    LoadCommits,
    /// Refresh view
    Refresh,
}
```

## 6. Integration with Git Plugin

### 6.1 Modified GitStatusPlugin

```rust
//! Simplified GitStatusPlugin showing integration

pub struct GitStatusPlugin {
    /// Plugin state (kept for rendering)
    state: PluginState,
    /// State machine for navigation and guards
    state_machine: GitStateMachine,
    /// Repository path
    repo_path: PathBuf,
    /// Whether the plugin is focused
    focused: bool,
    /// Git service
    git_service: CliGitService,
}

impl GitStatusPlugin {
    /// Handle a key press using state machine
    fn handle_key(&mut self, key: &str) -> Vec<Command> {
        // Delegate to state machine
        let git_commands = self.state_machine.handle_key(key);
        
        // Convert GitCommand to Command
        let mut commands = Vec::new();
        for cmd in git_commands {
            match cmd {
                GitCommand::SelectIndex(idx) => {
                    // Update state
                    if self.state.view_mode == ViewMode::History {
                        self.state.selected_commit = Some(idx);
                        // Load commit details
                        if let Some(commit) = self.state.commits.get(idx) {
                            commands.push(Command::LoadCommitDetails(commit.hash.clone()));
                        }
                    } else {
                        self.state.selected_file = Some(idx);
                    }
                    commands.push(Command::Refresh);
                }
                GitCommand::SetFocus(pane) => {
                    self.state.focus_pane = pane;
                    commands.push(Command::Refresh);
                }
                GitCommand::ExecuteAction(action) => {
                    match action {
                        ActionId::Stage => {
                            if let Some(path) = self.state.selected_file_path() {
                                commands.push(Command::StageFile(path));
                            }
                        }
                        ActionId::Unstage => {
                            if let Some(path) = self.state.selected_file_path() {
                                commands.push(Command::UnstageFile(path));
                            }
                        }
                        _ => {}
                    }
                }
                GitCommand::SwitchMode(mode) => {
                    self.state.view_mode = mode;
                    commands.push(Command::SwitchMode(mode));
                }
                GitCommand::LoadCommits => {
                    commands.push(Command::LoadCommits);
                }
                GitCommand::Refresh => {
                    commands.push(Command::Refresh);
                }
            }
        }
        
        commands
    }

    /// After loading data, sync with state machine
    async fn refresh(&mut self) -> Result<()> {
        let status = self.fetch_repo_status().await?;
        self.state.update_status(status);
        
        // Sync state machine with data
        match self.state.view_mode {
            ViewMode::History => {
                self.state_machine.update_items(self.state.commits.len());
            }
            _ => {
                self.state_machine.update_items(self.state.files.len());
            }
        }
        
        Ok(())
    }

    /// After loading commits
    async fn load_commits(&mut self) -> Result<()> {
        let commits = self.git_service.commits(&self.repo_path, 100).await?;
        self.state.commits = commits;
        
        // IMPORTANT: Update state machine with item count
        self.state_machine.update_items(self.state.commits.len());
        
        // Set initial selection
        if !self.state.commits.is_empty() {
            self.state.selected_commit = Some(0);
            self.state_machine.set_selected_index(Some(0));
            
            // Load details for first commit
            if let Some(commit) = self.state.commits.first() {
                self.load_commit_details(&commit.hash).await?;
            }
        }
        
        Ok(())
    }
}
```

## 7. Test Scenarios

### 7.1 Unit Tests (Core - Pure)

```rust
// core/logic/guards.rs tests
#[test]
fn test_stage_guard_requires_selection() {
    let ctx = action_context(ActionId::Stage, None, ViewMode::Status);
    assert!(matches!(
        check_guard(&ctx),
        GuardResult::Denied(GuardError::NoSelection)
    ));
}

#[test]
fn test_stage_guard_requires_status_mode() {
    let ctx = action_context(ActionId::Stage, Some(0), ViewMode::History);
    assert!(matches!(
        check_guard(&ctx),
        GuardResult::Denied(GuardError::WrongViewMode { .. })
    ));
}

// core/logic/navigation.rs tests
#[test]
fn test_navigate_down_from_none_selects_first() {
    let context = StateContext {
        selected_index: None,
        item_count: 5,
        focus_pane: FocusPane::Sidebar,
        ..Default::default()
    };
    
    let result = calculate_navigation(NavDirection::Down, &context);
    
    assert!(matches!(
        result,
        NavigationResult::Navigate { index: Some(0), .. }
    ));
}

#[test]
fn test_navigate_down_at_boundary() {
    let context = StateContext {
        selected_index: Some(4),
        item_count: 5,
        focus_pane: FocusPane::Sidebar,
        ..Default::default()
    };
    
    let result = calculate_navigation(NavDirection::Down, &context);
    
    assert_eq!(result, NavigationResult::AtBoundary);
}
```

### 7.2 Integration Tests (Shell)

```rust
// shell/machines/ tests
#[test]
fn test_state_machine_navigation_triggers_callback() {
    let executor = StateMachineExecutor::new();
    let states = Arc::new(Mutex::new(Vec::new()));
    
    let states_clone = states.clone();
    executor.on_state_change(move |old, new| {
        states_clone.lock().unwrap().push((old.clone(), new.clone()));
    });
    
    executor.set_item_count(5);
    executor.handle_navigation(NavDirection::Down);
    
    let recorded = states.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert!(matches!(recorded[0].1, ViewState::ItemSelected { index: 0 }));
}

#[test]
fn test_action_guard_blocks_unauthorized() {
    let executor = StateMachineExecutor::new();
    executor.set_item_count(0); // No items
    
    let result = executor.execute_action(ActionId::Stage);
    
    assert!(matches!(result, ActionResult::Denied(..)));
}
```

### 7.3 E2E Tests (Git Navigation Fix)

```rust
// Test the specific git navigation fix
#[tokio::test]
async fn test_git_history_navigation_with_j_k() {
    // Given: Git plugin in history mode with commits loaded
    let mut plugin = create_test_plugin_with_commits(10).await;
    plugin.state.view_mode = ViewMode::History;
    plugin.state_machine.initialize(10, ViewMode::History);
    
    // When: Press 'j' to navigate down
    let commands = plugin.handle_key("j");
    
    // Then: Should select first commit
    assert_eq!(plugin.state.selected_commit, Some(0));
    assert!(commands.iter().any(|c| matches!(c, Command::LoadCommitDetails(..))));
    
    // When: Press 'j' again
    let commands = plugin.handle_key("j");
    
    // Then: Should select second commit
    assert_eq!(plugin.state.selected_commit, Some(1));
}

#[tokio::test]
async fn test_git_stage_guard_no_selection() {
    // Given: Git plugin with no selection
    let mut plugin = GitStatusPlugin::new();
    plugin.state.files = vec![file_change("test.rs", FileStatus::Modified)];
    plugin.state_machine.initialize(1, ViewMode::Status);
    // No selection made
    
    // When: Try to stage
    let result = plugin.state_machine.executor.execute_action(ActionId::Stage);
    
    // Then: Should be denied
    assert!(matches!(result, ActionResult::Denied(GuardError::NoSelection)));
}
```

## 8. Implementation Checklist

### Phase 1: Core Models
- [x] Create `core/models/state_machine.rs` with `ViewState`, `StateContext`, `StateMachine`
- [x] Create `core/models/navigation.rs` with `NavRegion`, `NavDirection`, `NavigationTree`
- [x] Create `core/models/action.rs` with `ActionId`, `GuardError`, `GuardResult`

### Phase 2: Core Logic
- [x] Implement `core/logic/guards.rs` with authorization rules
- [x] Implement `core/logic/navigation.rs` with navigation calculations

### Phase 3: Shell
- [x] Create `shell/machines/mod.rs` with `StateMachineExecutor`
- [x] Create `shell/machines/git_state_machine.rs` with git-specific integration

### Phase 4: Integration
- [x] Modify `GitStatusPlugin` to use `GitStateMachine`
- [x] Wire up callbacks for state changes
- [x] Update key handling to use state machine

### Phase 5: Testing
- [x] Unit tests for guards
- [x] Unit tests for navigation
- [x] Integration tests for executor
- [x] E2E tests for git navigation fix (plugin-level tests in gitstatus/plugin.rs)

## 9. Files Modified

| File | Change | Reason |
|------|--------|--------|
| `src/core/models/mod.rs` | Add modules | Export new models |
| `src/core/models/state_machine.rs` | **NEW** | State machine definitions |
| `src/core/models/navigation.rs` | **NEW** | Navigation models |
| `src/core/models/action.rs` | **NEW** | Action & guard models |
| `src/core/logic/mod.rs` | Add modules | Export new logic |
| `src/core/logic/guards.rs` | **NEW** | Authorization logic |
| `src/core/logic/navigation.rs` | **NEW** | Navigation logic |
| `src/shell/machines/mod.rs` | **NEW** | State machine executor |
| `src/shell/machines/git_state_machine.rs` | **NEW** | Git integration |
| `src/shell/mod.rs` | Add module | Export machines |
| `src/plugins/gitstatus/plugin.rs` | Modify | Integrate state machine |
| `src/plugins/gitstatus/state.rs` | Modify | Sync with state machine |

## 10. Success Criteria

1. **Action Guards**: `Stage` action returns `Denied(NoSelection)` when no file is selected
2. **Git Navigation**: Pressing `j/k` navigates through commits in History mode
3. **Focus Management**: `h/l` switches focus between sidebar and main pane
4. **State Consistency**: State machine and plugin state are always synchronized
5. **Test Coverage**: >80% for core logic, >70% for shell

## 11. Notes

### Why This Architecture?

- **Pure Core**: Navigation and authorization are deterministic, testable, and cacheable
- **Imperative Shell**: I/O (callbacks, state persistence) is isolated and observable
- **FC&IS Compliance**: Core never calls Shell; Shell orchestrates Core + side effects

### Specific Fix for Git Navigation

The bug was that `j/k` keys in History mode didn't properly update the selection index. The fix:

1. State machine tracks `item_count` and `selected_index` separately from view state
2. Navigation logic is pure: given direction + context → result
3. Executor applies result and triggers callbacks
4. Git plugin receives `SelectIndex(n)` command and updates view state

This decouples "what should happen" (Core) from "how it happens" (Shell).
