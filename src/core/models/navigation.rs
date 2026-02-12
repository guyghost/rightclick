//! Navigation Models - Pure definitions for keyboard navigation
//!
//! This module defines navigation types for focus and keyboard navigation.
//! All types are pure data structures with no side effects.

use super::action::ActionId;
use super::state_machine::FocusPane;

/// A navigable region in the UI
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NavRegion {
    /// Main content area
    Main,
    /// Sidebar/left panel
    #[default]
    Sidebar,
    /// Header/top bar
    Header,
    /// Footer/status bar
    Footer,
    /// Modal/dialog (overlay)
    Modal,
}

impl std::fmt::Display for NavRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NavRegion::Main => write!(f, "Main"),
            NavRegion::Sidebar => write!(f, "Sidebar"),
            NavRegion::Header => write!(f, "Header"),
            NavRegion::Footer => write!(f, "Footer"),
            NavRegion::Modal => write!(f, "Modal"),
        }
    }
}

impl From<FocusPane> for NavRegion {
    fn from(pane: FocusPane) -> Self {
        match pane {
            FocusPane::Sidebar => NavRegion::Sidebar,
            FocusPane::Main => NavRegion::Main,
        }
    }
}

impl From<NavRegion> for FocusPane {
    fn from(region: NavRegion) -> Self {
        match region {
            NavRegion::Main => FocusPane::Main,
            _ => FocusPane::Sidebar,
        }
    }
}

/// Navigation direction
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavDirection {
    /// Move up in list (k, Up arrow)
    Up,
    /// Move down in list (j, Down arrow)
    Down,
    /// Move left / focus sidebar (h, Left arrow)
    Left,
    /// Move right / focus main (l, Right arrow)
    Right,
    /// Tab order (Tab)
    Next,
    /// Reverse tab (Shift+Tab)
    Previous,
    /// Jump to first item (g, Home)
    First,
    /// Jump to last item (G, End)
    Last,
}

impl std::fmt::Display for NavDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NavDirection::Up => write!(f, "Up"),
            NavDirection::Down => write!(f, "Down"),
            NavDirection::Left => write!(f, "Left"),
            NavDirection::Right => write!(f, "Right"),
            NavDirection::Next => write!(f, "Next"),
            NavDirection::Previous => write!(f, "Previous"),
            NavDirection::First => write!(f, "First"),
            NavDirection::Last => write!(f, "Last"),
        }
    }
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

impl Default for NavNode {
    fn default() -> Self {
        Self {
            region: NavRegion::default(),
            focusable: true,
            item_count: 0,
            selected_index: None,
            parent: None,
            children: Vec::new(),
        }
    }
}

impl NavNode {
    /// Create a new navigation node
    pub fn new(region: NavRegion) -> Self {
        Self {
            region,
            ..Default::default()
        }
    }

    /// Create a focusable node with item count
    pub fn with_items(region: NavRegion, item_count: usize) -> Self {
        Self {
            region,
            focusable: true,
            item_count,
            selected_index: if item_count > 0 { Some(0) } else { None },
            parent: None,
            children: Vec::new(),
        }
    }

    /// Builder: set focusable
    #[must_use]
    pub const fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
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
    pub const fn with_selected(mut self, index: Option<usize>) -> Self {
        self.selected_index = index;
        self
    }

    /// Builder: add child node
    #[must_use]
    pub fn with_child(mut self, child: NavNode) -> Self {
        self.children.push(child);
        self
    }

    /// Check if this node has navigable items
    pub const fn has_items(&self) -> bool {
        self.item_count > 0
    }
}

/// Navigation result
#[derive(Clone, Debug, PartialEq)]
pub enum NavigationResult {
    /// Navigate to specific region and item
    Navigate {
        /// Target region
        region: NavRegion,
        /// Target index (if applicable)
        index: Option<usize>,
    },
    /// Stay in current position
    Stay,
    /// Action triggered
    Action(ActionId),
    /// No valid target (at boundary)
    AtBoundary,
}

impl NavigationResult {
    /// Create a navigate result
    pub const fn navigate(region: NavRegion, index: Option<usize>) -> Self {
        NavigationResult::Navigate { region, index }
    }

    /// Create a navigate to sidebar with index
    pub const fn to_sidebar(index: Option<usize>) -> Self {
        Self::navigate(NavRegion::Sidebar, index)
    }

    /// Create a navigate to main result
    pub const fn to_main() -> Self {
        Self::navigate(NavRegion::Main, None)
    }

    /// Check if this is a navigation action
    pub const fn is_navigation(&self) -> bool {
        matches!(self, NavigationResult::Navigate { .. })
    }

    /// Check if at boundary
    pub const fn is_at_boundary(&self) -> bool {
        matches!(self, NavigationResult::AtBoundary)
    }
}

/// Navigation tree for a view
#[derive(Clone, Debug)]
pub struct NavigationTree {
    /// Root node
    pub root: NavNode,
    /// Currently focused region
    pub focused: NavRegion,
    /// Focus history for "back" navigation
    pub history: Vec<NavRegion>,
}

impl Default for NavigationTree {
    fn default() -> Self {
        Self::git_status()
    }
}

impl NavigationTree {
    /// Create a new navigation tree for git status view
    pub fn git_status() -> Self {
        let sidebar = NavNode::with_items(NavRegion::Sidebar, 0);

        let main = NavNode::new(NavRegion::Main)
            .with_item_count(1)
            .with_selected(Some(0));

        let root = NavNode::new(NavRegion::Main)
            .focusable(false)
            .with_child(sidebar)
            .with_child(main);

        Self {
            root,
            focused: NavRegion::Sidebar,
            history: vec![],
        }
    }

    /// Create a navigation tree with item count
    pub fn with_item_count(mut self, count: usize) -> Self {
        if let Some(sidebar) = self
            .root
            .children
            .iter_mut()
            .find(|c| c.region == NavRegion::Sidebar)
        {
            sidebar.item_count = count;
            sidebar.selected_index = if count > 0 { Some(0) } else { None };
        }
        self
    }

    /// Get current focused node
    pub fn focused_node(&self) -> Option<&NavNode> {
        self.root.children.iter().find(|c| c.region == self.focused)
    }

    /// Calculate next focus target
    pub fn navigate(&self, direction: NavDirection) -> NavigationResult {
        match (self.focused, direction) {
            (NavRegion::Sidebar, NavDirection::Right) => {
                NavigationResult::navigate(NavRegion::Main, None)
            }
            (NavRegion::Main, NavDirection::Left) => {
                // Get sidebar selection if available
                let sidebar_idx = self
                    .root
                    .children
                    .iter()
                    .find(|c| c.region == NavRegion::Sidebar)
                    .and_then(|c| c.selected_index);
                NavigationResult::navigate(NavRegion::Sidebar, sidebar_idx)
            }
            (NavRegion::Sidebar, NavDirection::Up) => {
                NavigationResult::Action(ActionId::NavigateUp)
            }
            (NavRegion::Sidebar, NavDirection::Down) => {
                NavigationResult::Action(ActionId::NavigateDown)
            }
            (NavRegion::Main, NavDirection::Up) => NavigationResult::AtBoundary,
            (NavRegion::Main, NavDirection::Down) => NavigationResult::AtBoundary,
            _ => NavigationResult::Stay,
        }
    }

    /// Set focus to a region
    pub fn set_focus(&mut self, region: NavRegion) {
        if self.focused != region {
            self.history.push(self.focused);
            self.focused = region;
        }
    }

    /// Go back to previous focus
    pub fn go_back(&mut self) -> Option<NavRegion> {
        self.history.pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nav_region_default() {
        assert_eq!(NavRegion::default(), NavRegion::Sidebar);
    }

    #[test]
    fn test_nav_region_from_focus_pane() {
        assert_eq!(NavRegion::from(FocusPane::Sidebar), NavRegion::Sidebar);
        assert_eq!(NavRegion::from(FocusPane::Main), NavRegion::Main);
    }

    #[test]
    fn test_focus_pane_from_nav_region() {
        assert_eq!(FocusPane::from(NavRegion::Sidebar), FocusPane::Sidebar);
        assert_eq!(FocusPane::from(NavRegion::Main), FocusPane::Main);
        assert_eq!(FocusPane::from(NavRegion::Header), FocusPane::Sidebar);
    }

    #[test]
    fn test_nav_node_default() {
        let node = NavNode::default();
        assert!(node.focusable);
        assert_eq!(node.item_count, 0);
        assert_eq!(node.selected_index, None);
    }

    #[test]
    fn test_nav_node_with_items() {
        let node = NavNode::with_items(NavRegion::Sidebar, 5);
        assert_eq!(node.item_count, 5);
        assert_eq!(node.selected_index, Some(0));
    }

    #[test]
    fn test_nav_node_with_items_empty() {
        let node = NavNode::with_items(NavRegion::Sidebar, 0);
        assert_eq!(node.item_count, 0);
        assert_eq!(node.selected_index, None);
    }

    #[test]
    fn test_nav_node_builder() {
        let node = NavNode::new(NavRegion::Main)
            .focusable(false)
            .with_item_count(10)
            .with_selected(Some(5));

        assert!(!node.focusable);
        assert_eq!(node.item_count, 10);
        assert_eq!(node.selected_index, Some(5));
    }

    #[test]
    fn test_nav_node_has_items() {
        assert!(NavNode::with_items(NavRegion::Sidebar, 1).has_items());
        assert!(!NavNode::with_items(NavRegion::Sidebar, 0).has_items());
    }

    #[test]
    fn test_navigation_result_helpers() {
        let result = NavigationResult::to_sidebar(Some(5));
        assert!(result.is_navigation());
        assert!(matches!(
            result,
            NavigationResult::Navigate {
                region: NavRegion::Sidebar,
                index: Some(5)
            }
        ));

        let result = NavigationResult::to_main();
        assert!(matches!(
            result,
            NavigationResult::Navigate {
                region: NavRegion::Main,
                index: None
            }
        ));

        assert!(NavigationResult::AtBoundary.is_at_boundary());
    }

    #[test]
    fn test_navigation_tree_default() {
        let tree = NavigationTree::default();
        assert_eq!(tree.focused, NavRegion::Sidebar);
        assert!(tree.history.is_empty());
    }

    #[test]
    fn test_navigation_tree_with_item_count() {
        let tree = NavigationTree::git_status().with_item_count(10);
        let sidebar = tree
            .root
            .children
            .iter()
            .find(|c| c.region == NavRegion::Sidebar);
        assert!(sidebar.is_some());
        let sidebar = sidebar.unwrap();
        assert_eq!(sidebar.item_count, 10);
        assert_eq!(sidebar.selected_index, Some(0));
    }

    #[test]
    fn test_navigation_tree_navigate_right() {
        let tree = NavigationTree::git_status();
        let result = tree.navigate(NavDirection::Right);
        assert!(matches!(
            result,
            NavigationResult::Navigate {
                region: NavRegion::Main,
                ..
            }
        ));
    }

    #[test]
    fn test_navigation_tree_navigate_left() {
        let tree = NavigationTree::git_status();
        let result = tree.navigate(NavDirection::Left);
        // From sidebar, left is at boundary
        assert_eq!(result, NavigationResult::AtBoundary);
    }

    #[test]
    fn test_navigation_tree_navigate_up_down() {
        let tree = NavigationTree::git_status();

        let result = tree.navigate(NavDirection::Up);
        assert!(matches!(
            result,
            NavigationResult::Action(ActionId::NavigateUp)
        ));

        let result = tree.navigate(NavDirection::Down);
        assert!(matches!(
            result,
            NavigationResult::Action(ActionId::NavigateDown)
        ));
    }

    #[test]
    fn test_navigation_tree_set_focus() {
        let mut tree = NavigationTree::git_status();
        tree.set_focus(NavRegion::Main);
        assert_eq!(tree.focused, NavRegion::Main);
        assert_eq!(tree.history, vec![NavRegion::Sidebar]);
    }

    #[test]
    fn test_navigation_tree_go_back() {
        let mut tree = NavigationTree::git_status();
        tree.set_focus(NavRegion::Main);
        let prev = tree.go_back();
        assert_eq!(prev, Some(NavRegion::Sidebar));
    }
}
