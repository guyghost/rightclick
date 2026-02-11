//! Conversations plugin state management
//!
//! This module defines the state structures for the Conversations plugin,
//! including session management, view state, and navigation.

use crate::adapters::types::{Adapter, AdapterType};
use crate::core::models::conversation::{Message, Role, Session, TokenUsage};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Current view mode for the conversations plugin
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversationView {
    /// List of all sessions
    SessionsList,
    /// Individual conversation view
    Conversation,
    /// Search mode
    Search,
}

impl Default for ConversationView {
    fn default() -> Self {
        ConversationView::SessionsList
    }
}

/// Information about a session with its adapter
#[derive(Clone, Debug)]
pub struct SessionInfo {
    /// The session data
    pub session: Session,
    /// Adapter type that owns this session
    pub adapter_type: AdapterType,
    /// Adapter icon character
    pub adapter_icon: char,
    /// Adapter name
    pub adapter_name: String,
}

impl SessionInfo {
    /// Create new session info from a session and adapter
    pub fn new(session: Session, adapter: &Arc<dyn Adapter>) -> Self {
        Self {
            session,
            adapter_type: adapter.adapter_type(),
            adapter_icon: adapter.icon(),
            adapter_name: adapter.name().to_string(),
        }
    }

    /// Get the session ID
    pub fn id(&self) -> &str {
        &self.session.id
    }

    /// Get the session title/name
    pub fn title(&self) -> &str {
        &self.session.name
    }

    /// Get total tokens if available
    pub fn total_tokens(&self) -> Option<usize> {
        self.session.total_tokens
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.session.message_count
    }
}

/// Navigation state for the sessions list
#[derive(Clone, Debug, Default)]
pub struct ListNavigation {
    /// Currently selected index
    pub selected: usize,
    /// Scroll offset (first visible item)
    pub scroll_offset: usize,
    /// Total number of items
    pub total_items: usize,
    /// Number of visible items
    pub viewport_height: usize,
}

impl ListNavigation {
    /// Create new navigation state
    pub fn new(total_items: usize, viewport_height: usize) -> Self {
        Self {
            selected: 0,
            scroll_offset: 0,
            total_items,
            viewport_height,
        }
    }

    /// Update total items count
    pub fn set_total_items(&mut self, total: usize) {
        self.total_items = total;
        self.selected = self.selected.min(total.saturating_sub(1));
        self.ensure_visible();
    }

    /// Update viewport height
    pub fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = height;
        self.ensure_visible();
    }

    /// Move selection down
    pub fn next(&mut self) {
        if self.selected + 1 < self.total_items {
            self.selected += 1;
            self.ensure_visible();
        }
    }

    /// Move selection up
    pub fn prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.ensure_visible();
        }
    }

    /// Jump to first item
    pub fn first(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Jump to last item
    pub fn last(&mut self) {
        self.selected = self.total_items.saturating_sub(1);
        self.ensure_visible();
    }

    /// Page down
    pub fn page_down(&mut self) {
        let page_size = self.viewport_height.saturating_sub(1);
        self.selected = (self.selected + page_size).min(self.total_items.saturating_sub(1));
        self.ensure_visible();
    }

    /// Page up
    pub fn page_up(&mut self) {
        let page_size = self.viewport_height.saturating_sub(1);
        self.selected = self.selected.saturating_sub(page_size);
        self.ensure_visible();
    }

    /// Ensure the selected item is visible in the viewport
    fn ensure_visible(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + self.viewport_height {
            self.scroll_offset = self.selected.saturating_sub(self.viewport_height.saturating_sub(1));
        }
    }

    /// Get the visible range (start, end) - end is exclusive
    pub fn visible_range(&self) -> (usize, usize) {
        let end = (self.scroll_offset + self.viewport_height).min(self.total_items);
        (self.scroll_offset, end)
    }

    /// Check if an index is the selected one
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected == index
    }

    /// Check if an index is visible
    pub fn is_visible(&self, index: usize) -> bool {
        let (start, end) = self.visible_range();
        index >= start && index < end
    }
}

/// Scroll state for conversation messages
#[derive(Clone, Debug, Default)]
pub struct MessageScroll {
    /// Current scroll position (line offset)
    pub scroll_y: usize,
    /// Total content height in lines
    pub content_height: usize,
    /// Viewport height in lines
    pub viewport_height: usize,
    /// Horizontal scroll for wide content
    pub scroll_x: usize,
}

impl MessageScroll {
    /// Create new scroll state
    pub fn new(viewport_height: usize) -> Self {
        Self {
            scroll_y: 0,
            content_height: 0,
            viewport_height,
            scroll_x: 0,
        }
    }

    /// Set content height
    pub fn set_content_height(&mut self, height: usize) {
        self.content_height = height;
        self.clamp_scroll();
    }

    /// Set viewport height
    pub fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = height;
        self.clamp_scroll();
    }

    /// Scroll down by n lines
    pub fn scroll_down(&mut self, n: usize) {
        self.scroll_y = self.scroll_y.saturating_add(n);
        self.clamp_scroll();
    }

    /// Scroll up by n lines
    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_y = self.scroll_y.saturating_sub(n);
    }

    /// Scroll to top
    pub fn scroll_to_top(&mut self) {
        self.scroll_y = 0;
        self.scroll_x = 0;
    }

    /// Scroll to bottom
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_y = self.content_height.saturating_sub(self.viewport_height);
    }

    /// Scroll right
    pub fn scroll_right(&mut self, n: usize) {
        self.scroll_x = self.scroll_x.saturating_add(n);
    }

    /// Scroll left
    pub fn scroll_left(&mut self, n: usize) {
        self.scroll_x = self.scroll_x.saturating_sub(n);
    }

    /// Clamp scroll values to valid ranges
    fn clamp_scroll(&mut self) {
        let max_scroll = self.content_height.saturating_sub(self.viewport_height);
        self.scroll_y = self.scroll_y.min(max_scroll);
    }

    /// Check if can scroll up
    pub fn can_scroll_up(&self) -> bool {
        self.scroll_y > 0
    }

    /// Check if can scroll down
    pub fn can_scroll_down(&self) -> bool {
        self.scroll_y + self.viewport_height < self.content_height
    }
}

/// Plugin state for conversations
#[derive(Clone, Debug)]
pub struct PluginState {
    /// All loaded sessions from all adapters
    pub sessions: Vec<SessionInfo>,
    /// Current messages in the selected session
    pub messages: Vec<Message>,
    /// Selected session index in the sessions list
    pub selected_session: Option<usize>,
    /// Current view mode
    pub view: ConversationView,
    /// Search query (when in search mode)
    pub search_query: Option<String>,
    /// Set of expanded message IDs
    pub expanded_messages: HashSet<String>,
    /// Navigation state for sessions list
    pub list_nav: ListNavigation,
    /// Scroll state for conversation view
    pub message_scroll: MessageScroll,
    /// Loading state
    pub is_loading: bool,
    /// Error message if any
    pub error: Option<String>,
    /// Adapter filter (None = all adapters)
    pub adapter_filter: Option<AdapterType>,
    /// Total token usage across all visible sessions
    pub total_tokens: Option<TokenUsage>,
}

impl Default for PluginState {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
            messages: Vec::new(),
            selected_session: None,
            view: ConversationView::default(),
            search_query: None,
            expanded_messages: HashSet::new(),
            list_nav: ListNavigation::default(),
            message_scroll: MessageScroll::default(),
            is_loading: false,
            error: None,
            adapter_filter: None,
            total_tokens: None,
        }
    }
}

impl PluginState {
    /// Create new empty state
    pub fn new() -> Self {
        Self::default()
    }

    /// Set sessions and update navigation
    pub fn set_sessions(&mut self, sessions: Vec<SessionInfo>) {
        self.list_nav.set_total_items(sessions.len());
        self.sessions = sessions;
        self.update_total_tokens();
    }

    /// Get the currently selected session info
    pub fn selected_session_info(&self) -> Option<&SessionInfo> {
        self.selected_session.and_then(|idx| self.sessions.get(idx))
    }

    /// Get mutable reference to selected session info
    pub fn selected_session_info_mut(&mut self) -> Option<&mut SessionInfo> {
        self.selected_session.and_then(|idx| self.sessions.get_mut(idx))
    }

    /// Select a session by index
    pub fn select_session(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.selected_session = Some(index);
            self.list_nav.selected = index;
            self.list_nav.ensure_visible();
        }
    }

    /// Clear session selection
    pub fn clear_selection(&mut self) {
        self.selected_session = None;
        self.messages.clear();
    }

    /// Set messages and reset scroll
    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
        self.message_scroll.scroll_to_top();
    }

    /// Toggle message expansion
    pub fn toggle_message_expanded(&mut self, message_id: &str) {
        if self.expanded_messages.contains(message_id) {
            self.expanded_messages.remove(message_id);
        } else {
            self.expanded_messages.insert(message_id.to_string());
        }
    }

    /// Check if a message is expanded
    pub fn is_message_expanded(&self, message_id: &str) -> bool {
        self.expanded_messages.contains(message_id)
    }

    /// Expand all messages
    pub fn expand_all_messages(&mut self) {
        for msg in &self.messages {
            self.expanded_messages.insert(msg.id.clone());
        }
    }

    /// Collapse all messages
    pub fn collapse_all_messages(&mut self) {
        self.expanded_messages.clear();
    }

    /// Set search query and enter search mode
    pub fn start_search(&mut self, query: String) {
        self.search_query = Some(query);
        self.view = ConversationView::Search;
    }

    /// Clear search and return to sessions list
    pub fn clear_search(&mut self) {
        self.search_query = None;
        self.view = ConversationView::SessionsList;
    }

    /// Get filtered sessions based on search query and adapter filter
    pub fn filtered_sessions(&self) -> Vec<&SessionInfo> {
        let mut sessions: Vec<&SessionInfo> = self.sessions.iter().collect();

        // Apply adapter filter
        if let Some(adapter_type) = self.adapter_filter {
            sessions.retain(|s| s.adapter_type == adapter_type);
        }

        // Apply search filter
        if let Some(ref query) = self.search_query {
            let query_lower = query.to_lowercase();
            sessions.retain(|s| {
                s.title().to_lowercase().contains(&query_lower)
                    || s.session.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            });
        }

        sessions
    }

    /// Update total token count across all sessions
    fn update_total_tokens(&mut self) {
        let mut total_input = 0usize;
        let mut total_output = 0usize;
        let mut has_usage = false;

        for session_info in &self.sessions {
            if let Some(tokens) = session_info.total_tokens() {
                // Note: Session only stores total_tokens, not breakdown
                // We approximate by assuming 50/50 split for display purposes
                total_input += tokens / 2;
                total_output += tokens / 2;
                has_usage = true;
            }
        }

        self.total_tokens = if has_usage {
            Some(TokenUsage::new(total_input, total_output))
        } else {
            None
        };
    }

    /// Get total message count across all sessions
    pub fn total_message_count(&self) -> usize {
        self.sessions.iter().map(|s| s.message_count()).sum()
    }

    /// Get adapter counts
    pub fn adapter_counts(&self) -> HashMap<AdapterType, usize> {
        let mut counts = HashMap::new();
        for session in &self.sessions {
            *counts.entry(session.adapter_type).or_insert(0) += 1;
        }
        counts
    }

    /// Enter conversation view mode
    pub fn enter_conversation(&mut self) {
        if self.selected_session.is_some() {
            self.view = ConversationView::Conversation;
        }
    }

    /// Enter sessions list view mode
    pub fn enter_sessions_list(&mut self) {
        self.view = ConversationView::SessionsList;
        self.message_scroll.scroll_to_top();
    }

    /// Set loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
    }

    /// Set error message
    pub fn set_error(&mut self, error: Option<String>) {
        self.error = error;
    }

    /// Get message by index
    pub fn get_message(&self, index: usize) -> Option<&Message> {
        self.messages.get(index)
    }

    /// Count messages by role
    pub fn message_counts_by_role(&self) -> HashMap<Role, usize> {
        let mut counts = HashMap::new();
        for msg in &self.messages {
            *counts.entry(msg.role).or_insert(0) += 1;
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_session(id: &str, name: &str) -> Session {
        Session::new(id, name, "test-adapter")
    }

    #[test]
    fn test_list_navigation() {
        let mut nav = ListNavigation::new(100, 10);
        assert_eq!(nav.selected, 0);
        assert_eq!(nav.scroll_offset, 0);

        nav.next();
        assert_eq!(nav.selected, 1);

        // Scroll down past viewport
        for _ in 0..15 {
            nav.next();
        }
        assert_eq!(nav.selected, 16);
        assert!(nav.scroll_offset > 0);

        nav.prev();
        assert_eq!(nav.selected, 15);

        nav.first();
        assert_eq!(nav.selected, 0);
        assert_eq!(nav.scroll_offset, 0);

        nav.last();
        assert_eq!(nav.selected, 99);
    }

    #[test]
    fn test_list_navigation_page() {
        let mut nav = ListNavigation::new(100, 10);

        nav.page_down();
        assert_eq!(nav.selected, 9);

        nav.page_down();
        assert_eq!(nav.selected, 18);

        nav.page_up();
        assert_eq!(nav.selected, 9);
    }

    #[test]
    fn test_message_scroll() {
        let mut scroll = MessageScroll::new(20);
        scroll.set_content_height(100);

        assert_eq!(scroll.scroll_y, 0);

        scroll.scroll_down(10);
        assert_eq!(scroll.scroll_y, 10);

        scroll.scroll_down(100);
        assert_eq!(scroll.scroll_y, 80); // Clamped to content_height - viewport_height

        scroll.scroll_up(5);
        assert_eq!(scroll.scroll_y, 75);

        scroll.scroll_to_top();
        assert_eq!(scroll.scroll_y, 0);

        scroll.scroll_to_bottom();
        assert_eq!(scroll.scroll_y, 80);
    }

    #[test]
    fn test_plugin_state_sessions() {
        let mut state = PluginState::new();

        let sessions = vec![
            SessionInfo::new(
                create_test_session("1", "Session 1"),
                &MockAdapter::new(AdapterType::ClaudeCode),
            ),
            SessionInfo::new(
                create_test_session("2", "Session 2"),
                &MockAdapter::new(AdapterType::Cursor),
            ),
        ];

        state.set_sessions(sessions);
        assert_eq!(state.sessions.len(), 2);
        assert_eq!(state.list_nav.total_items, 2);

        state.select_session(1);
        assert_eq!(state.selected_session, Some(1));
        assert_eq!(state.list_nav.selected, 1);
    }

    #[test]
    fn test_message_expansion() {
        let mut state = PluginState::new();

        assert!(!state.is_message_expanded("msg1"));

        state.toggle_message_expanded("msg1");
        assert!(state.is_message_expanded("msg1"));

        state.toggle_message_expanded("msg1");
        assert!(!state.is_message_expanded("msg1"));
    }

    #[test]
    fn test_view_transitions() {
        let mut state = PluginState::new();

        assert_eq!(state.view, ConversationView::SessionsList);

        state.enter_conversation();
        // Won't change without a selected session
        assert_eq!(state.view, ConversationView::SessionsList);

        // Set up a session
        let sessions = vec![SessionInfo::new(
            create_test_session("1", "Session 1"),
            &MockAdapter::new(AdapterType::ClaudeCode),
        )];
        state.set_sessions(sessions);
        state.select_session(0);

        state.enter_conversation();
        assert_eq!(state.view, ConversationView::Conversation);

        state.enter_sessions_list();
        assert_eq!(state.view, ConversationView::SessionsList);

        state.start_search("test".to_string());
        assert_eq!(state.view, ConversationView::Search);
        assert_eq!(state.search_query, Some("test".to_string()));

        state.clear_search();
        assert_eq!(state.view, ConversationView::SessionsList);
        assert_eq!(state.search_query, None);
    }

    // Mock adapter for testing
    struct MockAdapter {
        adapter_type: AdapterType,
    }

    impl MockAdapter {
        fn new(adapter_type: AdapterType) -> Self {
            Self { adapter_type }
        }
    }

    impl Adapter for MockAdapter {
        fn id(&self) -> &str {
            "mock"
        }

        fn name(&self) -> &str {
            "Mock Adapter"
        }

        fn icon(&self) -> char {
            '◈'
        }

        fn adapter_type(&self) -> AdapterType {
            self.adapter_type
        }

        fn detect(&self, _project_root: &std::path::Path) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = crate::adapters::types::Result<bool>> + Send + '_>,
        > {
            Box::pin(async { Ok(false) })
        }

        fn sessions(&self, _project_root: &std::path::Path) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = crate::adapters::types::Result<Vec<Session>>> + Send + '_>,
        > {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn messages(&self, _session_id: &str) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = crate::adapters::types::Result<Vec<Message>>> + Send + '_>,
        > {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn usage(&self, _session_id: &str) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = crate::adapters::types::Result<Option<TokenUsage>>> + Send + '_>,
        > {
            Box::pin(async { Ok(None) })
        }
    }
}
