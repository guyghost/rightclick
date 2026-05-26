//! Conversations plugin implementation
//!
//! This module provides the main plugin struct and its implementation for
//! managing AI conversation sessions across multiple adapters.

use crate::adapters::types::{Adapter, AdapterRegistry};
// Message and Session imported from state module
use crate::event::{Event, Topic};
use crate::keymap::KeyBinding;
use crate::plugins::conversations::render::ConversationsRenderer;
use crate::plugins::conversations::state::{ConversationView, PluginState, SessionInfo};
use crate::theme::get_current_theme;
use anyhow::Result;
use parking_lot::RwLock;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// The Conversations plugin manages AI conversation sessions
///
/// This plugin provides a unified interface for viewing and interacting
/// with conversation sessions from multiple AI adapters (Claude, Cursor,
/// Codex, etc.).
#[derive(Debug)]
pub struct ConversationsPlugin {
    /// Plugin state
    state: PluginState,
    /// Adapter registry for accessing AI adapters
    adapter_registry: Arc<RwLock<AdapterRegistry>>,
    /// Map of adapter IDs to adapters
    adapters: HashMap<String, Arc<dyn Adapter>>,
    /// Whether the plugin is currently focused
    focused: bool,
    /// Project root path
    project_root: Option<std::path::PathBuf>,
    /// Renderer for the UI
    renderer: ConversationsRenderer,
    /// Event dispatcher for publishing events
    event_dispatcher: Option<crate::event::EventDispatcher>,
    /// Defer refresh/load operations to async update()
    pending_refresh: bool,
    pending_load_messages: bool,
}

impl ConversationsPlugin {
    /// Create a new Conversations plugin
    ///
    /// # Arguments
    ///
    /// * `adapter_registry` - The registry of AI adapters
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::adapters::create_default_registry;
    /// use rightclick::plugins::conversations::ConversationsPlugin;
    /// use std::sync::Arc;
    /// use parking_lot::RwLock;
    ///
    /// let registry = create_default_registry().unwrap();
    /// let plugin = ConversationsPlugin::new(Arc::new(RwLock::new(registry)));
    /// ```
    pub fn new(adapter_registry: Arc<RwLock<AdapterRegistry>>) -> Self {
        Self {
            state: PluginState::new(),
            adapter_registry,
            adapters: HashMap::new(),
            focused: false,
            project_root: None,
            renderer: ConversationsRenderer::new(),
            event_dispatcher: None,
            pending_refresh: false,
            pending_load_messages: false,
        }
    }

    /// Set the event dispatcher
    pub fn with_event_dispatcher(mut self, dispatcher: crate::event::EventDispatcher) -> Self {
        self.event_dispatcher = Some(dispatcher);
        self
    }

    /// Set the project root path
    pub fn set_project_root(&mut self, path: impl AsRef<Path>) {
        self.project_root = Some(path.as_ref().to_path_buf());
    }

    /// Get the plugin name
    pub fn name(&self) -> &'static str {
        "Conversations"
    }

    /// Get the plugin ID
    pub fn id(&self) -> &'static str {
        "conversations"
    }

    /// Check if the plugin is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set the focused state
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if focused {
            debug!("Conversations plugin focused");
        }
    }

    /// Get a reference to the plugin state
    pub fn state(&self) -> &PluginState {
        &self.state
    }

    /// Get a mutable reference to the plugin state
    pub fn state_mut(&mut self) -> &mut PluginState {
        &mut self.state
    }

    fn set_selected_session_from_filtered_index(&mut self) {
        let selected_id = self
            .state
            .filtered_sessions()
            .get(self.state.list_nav.selected)
            .map(|s| s.session.id.clone());

        self.state.selected_session = selected_id.and_then(|id| {
            self.state
                .sessions
                .iter()
                .position(|session| session.session.id == id)
        });
    }

    fn refresh_search_navigation(&mut self) {
        let filtered_count = self.state.filtered_sessions().len();
        self.state.list_nav.set_total_items(filtered_count);

        if filtered_count == 0 {
            self.state.selected_session = None;
            return;
        }

        if self.state.list_nav.selected >= filtered_count {
            self.state.list_nav.selected = filtered_count - 1;
        }

        self.set_selected_session_from_filtered_index();
    }

    fn append_search_char(&mut self, c: char) {
        let mut query = self.state.search_query.clone().unwrap_or_default();
        query.push(c);
        self.state.search_query = Some(query);
        self.refresh_search_navigation();
    }

    fn pop_search_char(&mut self) {
        if let Some(mut query) = self.state.search_query.clone() {
            query.pop();
            self.state.search_query = Some(query);
            self.refresh_search_navigation();
        }
    }

    /// Load sessions from all adapters
    ///
    /// This async method queries all registered adapters for sessions
    /// related to the current project and populates the state.
    pub async fn load_sessions(&mut self) -> Result<()> {
        self.state.set_loading(true);
        self.state.set_error(None);

        let project_root = match &self.project_root {
            Some(path) => path.clone(),
            None => {
                self.state.set_loading(false);
                self.state
                    .set_error(Some("No project root set".to_string()));
                return Ok(());
            }
        };

        let adapters = {
            let registry = self.adapter_registry.read();
            registry.adapters().to_vec()
        };
        let mut detected = HashMap::new();
        for adapter in adapters {
            match adapter.detect(&project_root).await {
                Ok(true) => {
                    detected.insert(adapter.id().to_string(), adapter);
                }
                Ok(false) => {}
                Err(e) => {
                    warn!("Failed to detect adapter {}: {}", adapter.id(), e);
                }
            }
        }

        debug!("Detected {} adapters with data", detected.len());

        let mut all_sessions: Vec<SessionInfo> = Vec::new();

        for (adapter_id, adapter) in detected {
            debug!("Loading sessions from adapter: {}", adapter_id);

            match adapter.sessions(&project_root).await {
                Ok(sessions) => {
                    info!("Loaded {} sessions from {}", sessions.len(), adapter.name());

                    for session in sessions {
                        all_sessions.push(SessionInfo::new(session, &adapter));
                    }

                    // Store adapter reference
                    self.adapters.insert(adapter_id.clone(), adapter.clone());
                }
                Err(e) => {
                    warn!("Failed to load sessions from {}: {}", adapter.name(), e);
                }
            }
        }

        // Sort sessions by updated_at (newest first)
        all_sessions.sort_by(|a, b| b.session.updated_at.cmp(&a.session.updated_at));

        self.state.set_sessions(all_sessions);
        self.state.set_loading(false);

        // Publish event if we have a dispatcher
        if let Some(ref dispatcher) = self.event_dispatcher {
            dispatcher.publish(Topic::AdapterWatch, Event::SessionUpdate);
        }

        Ok(())
    }

    /// Load messages for the currently selected session
    pub async fn load_selected_session_messages(&mut self) -> Result<()> {
        let session_info = match self.state.selected_session_info() {
            Some(info) => info.clone(),
            None => return Ok(()),
        };

        let adapter = match self.adapters.get(&session_info.session.adapter_id) {
            Some(adapter) => adapter.clone(),
            None => {
                // Try to find adapter in registry
                let registry = self.adapter_registry.read();
                match registry.get(&session_info.session.adapter_id) {
                    Some(adapter) => adapter.clone(),
                    None => {
                        self.state.set_error(Some(format!(
                            "Adapter {} not found",
                            session_info.session.adapter_id
                        )));
                        return Ok(());
                    }
                }
            }
        };

        self.state.set_loading(true);

        match adapter.messages(&session_info.session.id).await {
            Ok(messages) => {
                self.state.set_messages(messages);
                self.state.set_error(None);
            }
            Err(e) => {
                error!("Failed to load messages: {}", e);
                self.state
                    .set_error(Some(format!("Failed to load messages: {}", e)));
            }
        }

        self.state.set_loading(false);
        Ok(())
    }

    /// Refresh the current view
    pub async fn refresh(&mut self) -> Result<()> {
        match self.state.view {
            ConversationView::SessionsList | ConversationView::Search => {
                self.load_sessions().await?;
            }
            ConversationView::Conversation => {
                self.load_selected_session_messages().await?;
            }
        }
        Ok(())
    }

    /// Handle a key event
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        let binding = KeyBinding::from(key);

        match self.state.view {
            ConversationView::SessionsList => self.handle_sessions_list_key(binding),
            ConversationView::Conversation => self.handle_conversation_key(binding),
            ConversationView::Search => self.handle_search_key(binding),
        }
    }

    /// Handle key in sessions list view
    fn handle_sessions_list_key(&mut self, binding: KeyBinding) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};

        match (binding.code, binding.modifiers) {
            // Navigation
            (KeyCode::Down, KeyModifiers::NONE) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                self.state.list_nav.next();
                self.state.selected_session = Some(self.state.list_nav.selected);
                true
            }
            (KeyCode::Up, KeyModifiers::NONE) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                self.state.list_nav.prev();
                self.state.selected_session = Some(self.state.list_nav.selected);
                true
            }
            (KeyCode::PageDown, _) | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                self.state.list_nav.page_down();
                self.state.selected_session = Some(self.state.list_nav.selected);
                true
            }
            (KeyCode::PageUp, _) | (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.state.list_nav.page_up();
                self.state.selected_session = Some(self.state.list_nav.selected);
                true
            }
            (KeyCode::Home, _) | (KeyCode::Char('g'), KeyModifiers::NONE) => {
                self.state.list_nav.first();
                self.state.selected_session = Some(self.state.list_nav.selected);
                true
            }
            (KeyCode::End, _) | (KeyCode::Char('G'), KeyModifiers::SHIFT) => {
                self.state.list_nav.last();
                self.state.selected_session = Some(self.state.list_nav.selected);
                true
            }

            // Enter conversation view
            (KeyCode::Enter, KeyModifiers::NONE) | (KeyCode::Char('l'), KeyModifiers::NONE) => {
                if self.state.selected_session.is_some() {
                    // We'll need to load messages async, so signal this
                    return false; // Let caller handle async
                }
                true
            }

            // Filter
            (KeyCode::Char('f'), KeyModifiers::NONE) => {
                self.state.start_search(String::new());
                true
            }

            // Refresh
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                false // Signal to refresh
            }

            _ => false,
        }
    }

    /// Handle key in conversation view
    fn handle_conversation_key(&mut self, binding: KeyBinding) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};

        match (binding.code, binding.modifiers) {
            // Navigation
            (KeyCode::Down, KeyModifiers::NONE) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                self.state.message_scroll.scroll_down(3);
                true
            }
            (KeyCode::Up, KeyModifiers::NONE) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                self.state.message_scroll.scroll_up(3);
                true
            }
            (KeyCode::PageDown, _) | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                self.state
                    .message_scroll
                    .scroll_down(self.state.message_scroll.viewport_height);
                true
            }
            (KeyCode::PageUp, _) | (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.state
                    .message_scroll
                    .scroll_up(self.state.message_scroll.viewport_height);
                true
            }
            (KeyCode::Home, _) | (KeyCode::Char('g'), KeyModifiers::NONE) => {
                self.state.message_scroll.scroll_to_top();
                true
            }
            (KeyCode::End, _) | (KeyCode::Char('G'), KeyModifiers::SHIFT) => {
                self.state.message_scroll.scroll_to_bottom();
                true
            }

            // Go back to sessions list
            (KeyCode::Esc, KeyModifiers::NONE)
            | (KeyCode::Char('h'), KeyModifiers::NONE)
            | (KeyCode::Left, KeyModifiers::NONE)
            | (KeyCode::Backspace, KeyModifiers::NONE) => {
                self.state.enter_sessions_list();
                true
            }

            // Filter sessions without first leaving the conversation
            (KeyCode::Char('f'), KeyModifiers::NONE) => {
                self.state.start_search(String::new());
                self.refresh_search_navigation();
                true
            }

            // Refresh messages for the selected session
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                self.pending_refresh = true;
                true
            }

            // Toggle message expansion
            (KeyCode::Char(' '), KeyModifiers::NONE) => {
                if let Some(msg) = self.state.get_message(self.state.message_scroll.scroll_y) {
                    let msg_id = msg.id.clone();
                    self.state.toggle_message_expanded(&msg_id);
                }
                true
            }

            // Expand all messages
            (KeyCode::Char('e'), KeyModifiers::NONE) => {
                self.state.expand_all_messages();
                true
            }

            // Collapse all messages
            (KeyCode::Char('c'), KeyModifiers::NONE) => {
                self.state.collapse_all_messages();
                true
            }

            // Copy message content to clipboard
            (KeyCode::Char('y'), KeyModifiers::NONE) => {
                if let Some(msg) = self.state.get_message(self.state.message_scroll.scroll_y) {
                    let _ = crate::shell::clipboard::copy_to_clipboard(&msg.content);
                }
                true
            }

            _ => false,
        }
    }

    /// Handle key in search view
    fn handle_search_key(&mut self, binding: KeyBinding) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};

        match (binding.code, binding.modifiers) {
            // Exit search
            (KeyCode::Esc, KeyModifiers::NONE) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.state.clear_search();
                true
            }

            // Execute search (Enter)
            (KeyCode::Enter, KeyModifiers::NONE) => {
                // Filter is already applied in real-time
                true
            }

            _ => false,
        }
    }

    /// Handle an action that requires async processing
    pub async fn handle_action(&mut self, action: ConversationsAction) -> Result<()> {
        match action {
            ConversationsAction::EnterSession => {
                self.state.enter_conversation();
                self.load_selected_session_messages().await?;
            }
            ConversationsAction::Refresh => {
                self.refresh().await?;
            }
            ConversationsAction::LoadSessions => {
                self.load_sessions().await?;
            }
            ConversationsAction::ToggleMessageExpansion(msg_id) => {
                self.state.toggle_message_expanded(&msg_id);
            }
            ConversationsAction::UpdateSearch(query) => {
                if query.is_empty() {
                    self.state.search_query = None;
                } else {
                    self.state.search_query = Some(query);
                }
            }
        }
        Ok(())
    }

    /// Render the plugin UI
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = get_current_theme().unwrap_or_default();
        self.renderer
            .render(&self.state, area, buf, &theme, self.focused);
    }

    /// Get key hints for the current view
    pub fn key_hints(&self) -> Vec<(&'static str, &'static str)> {
        match self.state.view {
            ConversationView::SessionsList => vec![
                ("↑/↓", "Navigate"),
                ("Enter/l/o", "Open"),
                ("f", "Filter"),
                ("r", "Refresh"),
                ("Ctrl+d/u", "Page"),
                ("g/G", "First/Last"),
            ],
            ConversationView::Conversation => vec![
                ("↑/↓", "Scroll"),
                ("Ctrl+d/u", "Page"),
                ("Esc/h", "Back"),
                ("f", "Filter"),
                ("r", "Refresh"),
                ("Space", "Expand"),
                ("e/c", "Expand/Collapse All"),
            ],
            ConversationView::Search => {
                vec![("Esc", "Clear"), ("Enter", "Search"), ("Type", "Filter")]
            }
        }
    }

    /// Get the current view title
    pub fn view_title(&self) -> String {
        match self.state.view {
            ConversationView::SessionsList => {
                if let Some(ref query) = self.state.search_query {
                    format!("Sessions (filter: '{}')", query)
                } else {
                    format!(
                        "Sessions ({})",
                        conversation_count_label(self.state.sessions.len(), "session")
                    )
                }
            }
            ConversationView::Conversation => {
                if let Some(session) = self.state.selected_session_info() {
                    format!("{} - {}", session.adapter_icon, session.title())
                } else {
                    "Conversation".to_string()
                }
            }
            ConversationView::Search => "Search Sessions".to_string(),
        }
    }

    /// Get adapter filter
    pub fn adapter_filter(&self) -> Option<crate::adapters::types::AdapterType> {
        self.state.adapter_filter
    }

    /// Set adapter filter
    pub fn set_adapter_filter(&mut self, filter: Option<crate::adapters::types::AdapterType>) {
        self.state.adapter_filter = filter;
    }

    /// Update viewport dimensions
    pub fn update_dimensions(&mut self, sessions_height: u16, messages_height: u16) {
        self.state
            .list_nav
            .set_viewport_height(sessions_height as usize);
        self.state
            .message_scroll
            .set_viewport_height(messages_height as usize);
    }
}

/// Actions that can be performed by the conversations plugin
#[derive(Debug, Clone)]
pub enum ConversationsAction {
    /// Enter the currently selected session
    EnterSession,
    /// Refresh the current view
    Refresh,
    /// Load all sessions
    LoadSessions,
    /// Toggle expansion of a specific message
    ToggleMessageExpansion(String),
    /// Update the search query
    UpdateSearch(String),
}

/// Builder for ConversationsPlugin
pub struct ConversationsPluginBuilder {
    adapter_registry: Option<Arc<RwLock<AdapterRegistry>>>,
    project_root: Option<std::path::PathBuf>,
    event_dispatcher: Option<crate::event::EventDispatcher>,
}

impl ConversationsPluginBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            adapter_registry: None,
            project_root: None,
            event_dispatcher: None,
        }
    }

    /// Set the adapter registry
    pub fn adapter_registry(mut self, registry: Arc<RwLock<AdapterRegistry>>) -> Self {
        self.adapter_registry = Some(registry);
        self
    }

    /// Set the project root
    pub fn project_root(mut self, path: impl AsRef<Path>) -> Self {
        self.project_root = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set the event dispatcher
    pub fn event_dispatcher(mut self, dispatcher: crate::event::EventDispatcher) -> Self {
        self.event_dispatcher = Some(dispatcher);
        self
    }

    /// Build the plugin
    pub fn build(self) -> Result<ConversationsPlugin> {
        let registry = self
            .adapter_registry
            .ok_or_else(|| anyhow::anyhow!("Adapter registry is required"))?;

        let mut plugin = ConversationsPlugin::new(registry);

        if let Some(path) = self.project_root {
            plugin.set_project_root(path);
        }

        if let Some(dispatcher) = self.event_dispatcher {
            plugin.event_dispatcher = Some(dispatcher);
        }

        Ok(plugin)
    }
}

impl Default for ConversationsPluginBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Plugin Trait Implementation
// ============================================================================

use crate::plugin::{Command as PluginCmd, Plugin, PluginContext};
use async_trait::async_trait;

#[async_trait]
impl Plugin for ConversationsPlugin {
    fn id(&self) -> &str {
        "conversations"
    }

    fn name(&self) -> &str {
        "Conversations"
    }

    fn icon(&self) -> char {
        'C'
    }

    async fn init(&mut self, ctx: &PluginContext) -> anyhow::Result<()> {
        self.project_root = Some(ctx.project_root.clone());
        // Note: adapters are loaded from the registry, not from context

        if let Err(e) = self.load_sessions().await {
            warn!("Failed to load sessions: {}", e);
        }

        Ok(())
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn handle_event(&mut self, event: crate::event::Event) -> Vec<PluginCmd> {
        match event {
            crate::event::Event::Key { code, modifiers } => {
                // Handle Ctrl+key combinations
                if modifiers.ctrl {
                    match code.as_str() {
                        "d" => {
                            // Page down - varies by view
                            match self.state.view {
                                ConversationView::SessionsList | ConversationView::Search => {
                                    self.state.list_nav.page_down();
                                    self.state.selected_session =
                                        Some(self.state.list_nav.selected);
                                }
                                ConversationView::Conversation => {
                                    self.state
                                        .message_scroll
                                        .scroll_down(self.state.message_scroll.viewport_height);
                                }
                            }
                        }
                        "u" => {
                            // Page up - varies by view
                            match self.state.view {
                                ConversationView::SessionsList | ConversationView::Search => {
                                    self.state.list_nav.page_up();
                                    self.state.selected_session =
                                        Some(self.state.list_nav.selected);
                                }
                                ConversationView::Conversation => {
                                    self.state
                                        .message_scroll
                                        .scroll_up(self.state.message_scroll.viewport_height);
                                }
                            }
                        }
                        "c" => {
                            // Exit search mode
                            if self.state.view == ConversationView::Search {
                                self.state.clear_search();
                                self.state
                                    .list_nav
                                    .set_total_items(self.state.sessions.len());
                                if self.state.list_nav.selected < self.state.sessions.len() {
                                    self.state.selected_session =
                                        Some(self.state.list_nav.selected);
                                }
                            }
                        }
                        _ => {}
                    }
                } else if !modifiers.alt {
                    // Handle simple key presses
                    match self.state.view {
                        ConversationView::SessionsList => match code.as_str() {
                            "j" | "Down" => {
                                self.state.list_nav.next();
                                self.state.selected_session = Some(self.state.list_nav.selected);
                            }
                            "k" | "Up" => {
                                self.state.list_nav.prev();
                                self.state.selected_session = Some(self.state.list_nav.selected);
                            }
                            "g" | "Home" => {
                                self.state.list_nav.first();
                                self.state.selected_session = Some(self.state.list_nav.selected);
                            }
                            "G" | "End" => {
                                self.state.list_nav.last();
                                self.state.selected_session = Some(self.state.list_nav.selected);
                            }
                            "Enter" | "l" | "o" => {
                                if self.state.selected_session.is_some() {
                                    self.state.enter_conversation();
                                    self.pending_load_messages = true;
                                }
                            }
                            "f" => {
                                self.state.start_search(String::new());
                                self.refresh_search_navigation();
                            }
                            "r" => {
                                self.pending_refresh = true;
                            }
                            _ => {}
                        },
                        ConversationView::Conversation => {
                            match code.as_str() {
                                "j" | "Down" => {
                                    self.state.message_scroll.scroll_down(3);
                                }
                                "k" | "Up" => {
                                    self.state.message_scroll.scroll_up(3);
                                }
                                "g" | "Home" => {
                                    self.state.message_scroll.scroll_to_top();
                                }
                                "G" | "End" => {
                                    self.state.message_scroll.scroll_to_bottom();
                                }
                                "h" | "Left" | "Esc" | "Backspace" => {
                                    self.state.enter_sessions_list();
                                }
                                "f" => {
                                    self.state.start_search(String::new());
                                    self.refresh_search_navigation();
                                }
                                "r" => {
                                    self.pending_refresh = true;
                                }
                                " " => {
                                    // Toggle message expansion
                                    if let Some(msg) =
                                        self.state.get_message(self.state.message_scroll.scroll_y)
                                    {
                                        let msg_id = msg.id.clone();
                                        self.state.toggle_message_expanded(&msg_id);
                                    }
                                }
                                "e" => {
                                    self.state.expand_all_messages();
                                }
                                "c" => {
                                    self.state.collapse_all_messages();
                                }
                                _ => {}
                            }
                        }
                        ConversationView::Search => {
                            match code.as_str() {
                                "Esc" => {
                                    self.state.clear_search();
                                    self.state
                                        .list_nav
                                        .set_total_items(self.state.sessions.len());
                                    if self.state.list_nav.selected < self.state.sessions.len() {
                                        self.state.selected_session =
                                            Some(self.state.list_nav.selected);
                                    }
                                }
                                "Enter" => {
                                    // Filter is already applied in real-time
                                }
                                "Backspace" => {
                                    self.pop_search_char();
                                }
                                "j" | "Down" => {
                                    self.state.list_nav.next();
                                    self.set_selected_session_from_filtered_index();
                                }
                                "k" | "Up" => {
                                    self.state.list_nav.prev();
                                    self.set_selected_session_from_filtered_index();
                                }
                                _ => {
                                    if code.len() == 1 {
                                        let ch = code.chars().next().unwrap_or_default();
                                        if !ch.is_control() {
                                            self.append_search_char(ch);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            crate::event::Event::RefreshNeeded => {
                self.pending_refresh = true;
            }
            _ => {}
        }

        Vec::new()
    }

    fn render(&self, area: Rect, buf: &mut Buffer, theme: &crate::core::models::Theme) {
        let current_theme = get_current_theme().unwrap_or_else(|| theme.clone());
        let renderer = ConversationsRenderer::new();
        renderer.render(&self.state, area, buf, &current_theme, self.focused);
    }

    fn is_focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn commands(&self) -> Vec<crate::plugin::PluginCommand> {
        vec![
            crate::plugin::PluginCommand::with_context_description(
                "refresh",
                "Refresh",
                "Reload sessions from detected adapters",
                'r',
                crate::keymap::FocusContext::Conversations,
            )
            .with_footer_priority(3),
            crate::plugin::PluginCommand::with_context_description(
                "search",
                "Filter",
                "Filter loaded sessions",
                'f',
                crate::keymap::FocusContext::Conversations,
            )
            .with_footer_priority(4),
            crate::plugin::PluginCommand::with_context_description(
                "expand",
                "Expand All",
                "Expand messages in the current conversation",
                'e',
                crate::keymap::FocusContext::Conversations,
            )
            .with_footer_priority(2),
            crate::plugin::PluginCommand::with_context_description(
                "collapse",
                "Collapse All",
                "Collapse messages in the current conversation",
                'c',
                crate::keymap::FocusContext::Conversations,
            )
            .with_footer_priority(1),
            crate::plugin::PluginCommand::with_context_description(
                "open",
                "Open",
                "Open the selected session",
                'o',
                crate::keymap::FocusContext::Conversations,
            )
            .with_footer_priority(5),
            crate::plugin::PluginCommand::with_context_description(
                "back",
                "Back",
                "Return to the sessions list",
                'h',
                crate::keymap::FocusContext::Conversations,
            ),
        ]
    }

    fn status_line(&self) -> Option<String> {
        Some(conversations_status_line(&self.state))
    }

    fn search_entries(&self) -> Vec<crate::plugin::PluginSearchEntry> {
        self.state
            .sessions
            .iter()
            .map(|session_info| {
                let mut preview_parts = vec![
                    session_info.adapter_name.clone(),
                    conversation_count_label(session_info.message_count(), "message"),
                ];
                if let Some(total_tokens) = session_info.total_tokens() {
                    preview_parts.push(conversation_count_label(total_tokens, "token"));
                }

                crate::plugin::PluginSearchEntry {
                    id: session_info.session.id.clone(),
                    title: session_info.title().to_string(),
                    preview: preview_parts.join(" | "),
                }
            })
            .collect()
    }

    fn activate_search_result(&mut self, entry_id: &str) -> bool {
        let Some(index) = self
            .state
            .sessions
            .iter()
            .position(|session_info| session_info.session.id == entry_id)
        else {
            return false;
        };

        self.state.select_session(index);
        self.state.enter_conversation();
        self.pending_load_messages = true;
        true
    }

    fn focus_context(&self) -> crate::keymap::FocusContext {
        crate::keymap::FocusContext::Conversations
    }

    async fn update(&mut self) -> anyhow::Result<()> {
        if self.pending_refresh {
            self.pending_refresh = false;
            if let Err(e) = self.refresh().await {
                warn!("Conversations refresh failed: {}", e);
            }
        }

        if self.pending_load_messages {
            self.pending_load_messages = false;
            if let Err(e) = self.load_selected_session_messages().await {
                warn!("Failed to load conversation messages: {}", e);
            }
        }

        Ok(())
    }
}

fn conversations_status_line(state: &PluginState) -> String {
    if state.is_loading {
        return "Loading conversations".to_string();
    }

    if let Some(error) = &state.error {
        return format!("Conversations error: {}", error);
    }

    if state.sessions.is_empty() {
        return "No sessions | r Refresh detected adapters | f Filter sessions | / Global search"
            .to_string();
    }

    let visible = state.filtered_sessions().len();
    let total = state.sessions.len();
    let messages = state.total_message_count();

    if state.search_query.is_some() || state.adapter_filter.is_some() {
        let mut status = format!(
            "{} visible | {} total | {}",
            conversation_count_label(visible, "session"),
            conversation_count_label(total, "session"),
            conversation_count_label(messages, "message")
        );
        if let Some(adapter_type) = state.adapter_filter {
            status.push_str(&format!(" | adapter: {}", adapter_type.display_name()));
        }
        if state.search_query.is_some() {
            status.push_str(" | Esc: Clear");
        }
        status
    } else {
        format!(
            "{} | {}",
            conversation_count_label(total, "session"),
            conversation_count_label(messages, "message")
        )
    }
}

fn conversation_count_label(count: usize, label: &str) -> String {
    if count == 1 {
        format!("1 {}", label)
    } else {
        format!("{} {}s", count, label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_creation() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let plugin = ConversationsPlugin::new(registry);

        assert_eq!(plugin.name(), "Conversations");
        assert_eq!(plugin.id(), "conversations");
        assert!(!plugin.is_focused());
    }

    #[test]
    fn test_plugin_focus() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let mut plugin = ConversationsPlugin::new(registry);

        plugin.set_focused(true);
        assert!(plugin.is_focused());

        plugin.set_focused(false);
        assert!(!plugin.is_focused());
    }

    #[test]
    fn test_view_transitions() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let mut plugin = ConversationsPlugin::new(registry);

        assert_eq!(plugin.state().view, ConversationView::SessionsList);

        // Can't enter conversation without a session selected
        plugin.state_mut().enter_conversation();
        assert_eq!(plugin.state().view, ConversationView::SessionsList);
    }

    #[test]
    fn test_key_hints() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let plugin = ConversationsPlugin::new(registry);

        let hints = plugin.key_hints();
        assert!(!hints.is_empty());
        assert!(hints.contains(&("f", "Filter")));
        assert!(
            !hints
                .iter()
                .any(|(key, label)| *key == "/" && *label == "Search")
        );
    }

    #[test]
    fn test_filter_key_enters_session_search() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let mut plugin = ConversationsPlugin::new(registry);

        plugin.handle_event(crate::event::Event::Key {
            code: "f".to_string(),
            modifiers: crate::event::KeyModifiers::default(),
        });

        assert_eq!(plugin.state().view, ConversationView::Search);
        assert_eq!(plugin.state().search_query, Some(String::new()));
    }

    #[test]
    fn test_filter_key_enters_search_from_conversation() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let mut plugin = ConversationsPlugin::new(registry);
        plugin.state_mut().view = ConversationView::Conversation;

        plugin.handle_event(crate::event::Event::Key {
            code: "f".to_string(),
            modifiers: crate::event::KeyModifiers::default(),
        });

        assert_eq!(plugin.state().view, ConversationView::Search);
        assert_eq!(plugin.state().search_query, Some(String::new()));
    }

    #[test]
    fn test_refresh_key_refreshes_current_conversation() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let mut plugin = ConversationsPlugin::new(registry);
        plugin.state_mut().view = ConversationView::Conversation;

        plugin.handle_event(crate::event::Event::Key {
            code: "r".to_string(),
            modifiers: crate::event::KeyModifiers::default(),
        });

        assert!(plugin.pending_refresh);
    }

    #[test]
    fn test_commands_use_filter_key_to_avoid_global_search_conflict() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let plugin = ConversationsPlugin::new(registry);

        let commands = plugin.commands();
        let filter_command = commands
            .iter()
            .find(|command| command.id == "search")
            .expect("conversations filter command");

        assert_eq!(filter_command.name, "Filter");
        assert_eq!(filter_command.key, 'f');
        assert!(commands.iter().any(|command| command.id == "expand"
            && command.name == "Expand All"
            && command.key == 'e'));
        assert!(commands.iter().any(|command| command.id == "collapse"
            && command.name == "Collapse All"
            && command.key == 'c'));
        assert!(!commands.iter().any(|command| command.key == '/'));

        let prioritized: Vec<(&str, u8)> = commands
            .iter()
            .filter(|command| command.priority > 0)
            .map(|command| (command.id.as_str(), command.priority))
            .collect();
        assert_eq!(
            prioritized,
            vec![
                ("refresh", 3),
                ("search", 4),
                ("expand", 2),
                ("collapse", 1),
                ("open", 5)
            ]
        );
    }

    #[test]
    fn test_execute_collapse_command_uses_declared_shortcut() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let mut plugin = ConversationsPlugin::new(registry);
        plugin.state_mut().view = ConversationView::Conversation;
        plugin.state_mut().set_messages(vec![
            crate::core::models::conversation::Message::user("msg-1", "First"),
            crate::core::models::conversation::Message::assistant("msg-2", "Second"),
        ]);
        plugin.state_mut().expand_all_messages();

        let execution = plugin
            .execute_command("collapse")
            .expect("collapse command should execute");

        assert_eq!(execution.command_name, "Collapse All");
        assert!(plugin.state().expanded_messages.is_empty());
    }

    #[test]
    fn test_view_title() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let adapter: Arc<dyn Adapter> = Arc::new(TestAdapter);
        let mut plugin = ConversationsPlugin::new(registry);
        plugin.state_mut().set_sessions(vec![SessionInfo::new(
            crate::core::models::conversation::Session::new(
                "session-1",
                "Title polish",
                "test-adapter",
            ),
            &adapter,
        )]);

        let title = plugin.view_title();
        assert_eq!(title, "Sessions (1 session)");
    }

    #[test]
    fn test_search_entries_include_loaded_sessions() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let adapter: Arc<dyn Adapter> = Arc::new(TestAdapter);
        let mut plugin = ConversationsPlugin::new(registry);
        plugin.state_mut().set_sessions(vec![SessionInfo::new(
            crate::core::models::conversation::Session::new(
                "session-1",
                "Investigate render bug",
                "test-adapter",
            ),
            &adapter,
        )]);

        let entries = plugin.search_entries();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "session-1");
        assert!(entries[0].title.contains("Investigate render bug"));
        assert!(entries[0].preview.contains("Mock Adapter"));
    }

    #[test]
    fn test_search_entries_use_singular_message_and_token_counts() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let adapter: Arc<dyn Adapter> = Arc::new(TestAdapter);
        let mut session = crate::core::models::conversation::Session::new(
            "session-1",
            "Single message",
            "test-adapter",
        );
        session.message_count = 1;
        session.total_tokens = Some(1);
        let mut plugin = ConversationsPlugin::new(registry);
        plugin
            .state_mut()
            .set_sessions(vec![SessionInfo::new(session, &adapter)]);

        let entries = plugin.search_entries();

        assert_eq!(entries.len(), 1);
        assert!(entries[0].preview.contains("1 message"));
        assert!(entries[0].preview.contains("1 token"));
        assert!(!entries[0].preview.contains("1 messages"));
        assert!(!entries[0].preview.contains("1 tokens"));
    }

    #[test]
    fn test_activate_search_result_selects_session_and_enters_conversation() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let adapter: Arc<dyn Adapter> = Arc::new(TestAdapter);
        let mut plugin = ConversationsPlugin::new(registry);
        plugin.state_mut().set_sessions(vec![
            SessionInfo::new(
                crate::core::models::conversation::Session::new(
                    "session-1",
                    "First",
                    "test-adapter",
                ),
                &adapter,
            ),
            SessionInfo::new(
                crate::core::models::conversation::Session::new(
                    "session-2",
                    "Second",
                    "test-adapter",
                ),
                &adapter,
            ),
        ]);

        assert!(plugin.activate_search_result("session-2"));

        assert_eq!(plugin.state().selected_session, Some(1));
        assert_eq!(plugin.state().view, ConversationView::Conversation);
    }

    #[test]
    fn test_status_line_points_to_refresh_when_empty() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let plugin = ConversationsPlugin::new(registry);

        assert_eq!(
            plugin.status_line(),
            Some(
                "No sessions | r Refresh detected adapters | f Filter sessions | / Global search"
                    .to_string()
            )
        );
    }

    #[test]
    fn test_status_line_summarizes_filtered_sessions() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let adapter: Arc<dyn Adapter> = Arc::new(TestAdapter);
        let mut first = crate::core::models::conversation::Session::new(
            "session-1",
            "Render bug",
            "test-adapter",
        );
        first.message_count = 3;
        let mut second = crate::core::models::conversation::Session::new(
            "session-2",
            "CLI polish",
            "test-adapter",
        );
        second.message_count = 5;
        let mut plugin = ConversationsPlugin::new(registry);
        plugin.state_mut().set_sessions(vec![
            SessionInfo::new(first, &adapter),
            SessionInfo::new(second, &adapter),
        ]);
        plugin.state_mut().start_search("render".to_string());

        assert_eq!(
            plugin.status_line(),
            Some("1 session visible | 2 sessions total | 8 messages | Esc: Clear".to_string())
        );
    }

    #[test]
    fn test_status_line_names_adapter_filter() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let adapter: Arc<dyn Adapter> = Arc::new(TestAdapter);
        let mut first =
            crate::core::models::conversation::Session::new("session-1", "Render bug", "codex");
        first.message_count = 3;
        let mut second =
            crate::core::models::conversation::Session::new("session-2", "CLI polish", "codex");
        second.message_count = 5;
        let mut plugin = ConversationsPlugin::new(registry);
        plugin.state_mut().set_sessions(vec![
            SessionInfo::new(first, &adapter),
            SessionInfo::new(second, &adapter),
        ]);
        plugin.set_adapter_filter(Some(crate::adapters::types::AdapterType::Codex));

        assert_eq!(
            plugin.status_line(),
            Some(
                "2 sessions visible | 2 sessions total | 8 messages | adapter: Codex CLI"
                    .to_string()
            )
        );
    }

    #[test]
    fn test_status_line_uses_singular_session_and_message_counts() {
        let registry = Arc::new(RwLock::new(AdapterRegistry::new()));
        let adapter: Arc<dyn Adapter> = Arc::new(TestAdapter);
        let mut session =
            crate::core::models::conversation::Session::new("session-1", "Render bug", "codex");
        session.message_count = 1;
        let mut plugin = ConversationsPlugin::new(registry);
        plugin
            .state_mut()
            .set_sessions(vec![SessionInfo::new(session, &adapter)]);

        assert_eq!(
            plugin.status_line(),
            Some("1 session | 1 message".to_string())
        );
    }

    #[derive(Debug)]
    struct TestAdapter;

    #[async_trait::async_trait]
    impl Adapter for TestAdapter {
        fn id(&self) -> &str {
            "test-adapter"
        }

        fn name(&self) -> &str {
            "Mock Adapter"
        }

        fn icon(&self) -> char {
            'T'
        }

        fn adapter_type(&self) -> crate::adapters::types::AdapterType {
            crate::adapters::types::AdapterType::Codex
        }

        async fn detect(
            &self,
            _project_root: &std::path::Path,
        ) -> crate::adapters::types::Result<bool> {
            Ok(true)
        }

        async fn sessions(
            &self,
            _project_root: &std::path::Path,
        ) -> crate::adapters::types::Result<Vec<crate::core::models::conversation::Session>>
        {
            Ok(Vec::new())
        }

        async fn messages(
            &self,
            _session_id: &str,
        ) -> crate::adapters::types::Result<Vec<crate::core::models::conversation::Message>>
        {
            Ok(Vec::new())
        }

        async fn usage(
            &self,
            _session_id: &str,
        ) -> crate::adapters::types::Result<Option<crate::core::models::conversation::TokenUsage>>
        {
            Ok(None)
        }
    }
}
