//! Conversations Plugin for RightClick
//!
//! This plugin provides a unified interface for viewing and interacting with
//! AI conversation sessions from multiple adapters (Claude, Cursor, Codex, etc.).
//!
//! # Features
//!
//! - **Multi-Adapter Support**: View sessions from Claude Code, Cursor, Codex CLI, and more
//! - **Session Management**: Browse, search, and filter sessions by adapter
//! - **Message Viewing**: Read conversation history with expandable content blocks
//! - **Tool Use Display**: View tool invocations and results
//! - **Keyboard Navigation**: Vim-style keybindings for efficient navigation
//!
//! # Architecture
//!
//! The plugin follows the Functional Core & Imperative Shell pattern:
//!
//! - **`state`** - Pure state management and navigation logic
//! - **`plugin`** - Plugin lifecycle, event handling, and adapter integration
//! - **`render`** - TUI rendering using ratatui
//!
//! # Usage
//!
//! ```rust
//! use rightclick::adapters::create_default_registry;
//! use rightclick::plugins::conversations::{ConversationsPlugin, ConversationsPluginBuilder};
//! use std::sync::Arc;
//! use parking_lot::RwLock;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create adapter registry
//!     let registry = create_default_registry()?;
//!     let registry = Arc::new(RwLock::new(registry));
//!
//!     // Create plugin
//!     let mut plugin = ConversationsPluginBuilder::new()
//!         .adapter_registry(registry)
//!         .project_root("/path/to/project")
//!         .build()?;
//!
//!     // Load sessions
//!     plugin.load_sessions().await?;
//!
//!     // Render
//!     // plugin.render(area, buf);
//!
//!     Ok(())
//! }
//! ```

mod plugin;
mod render;
mod state;

// Re-export main types
pub use plugin::{ConversationsAction, ConversationsPlugin, ConversationsPluginBuilder};
pub use render::ConversationsRenderer;
pub use state::{ConversationView, ListNavigation, MessageScroll, PluginState, SessionInfo};

use std::sync::Arc;

use parking_lot::RwLock;

use crate::adapters::types::Adapter;
use crate::core::models::conversation::Session;

/// Initialize the conversations plugin with default settings
///
/// This is a convenience function for quickly creating a plugin instance.
///
/// # Arguments
///
/// * `adapter_registry` - The registry of AI adapters
///
/// # Example
///
/// ```rust
/// use rightclick::adapters::create_default_registry;
/// use rightclick::plugins::conversations::init;
/// use std::sync::Arc;
/// use parking_lot::RwLock;
///
/// let registry = create_default_registry().unwrap();
/// let plugin = init(Arc::new(RwLock::new(registry)));
/// ```
pub fn init(
    adapter_registry: Arc<RwLock<crate::adapters::AdapterRegistry>>,
) -> ConversationsPlugin {
    ConversationsPlugin::new(adapter_registry)
}

/// Create a builder for the conversations plugin
///
/// Use this for more advanced configuration.
///
/// # Example
///
/// ```rust
/// use rightclick::adapters::create_default_registry;
/// use rightclick::plugins::conversations::builder;
/// use std::sync::Arc;
/// use parking_lot::RwLock;
///
/// let registry = create_default_registry().unwrap();
/// let plugin = builder()
///     .adapter_registry(Arc::new(RwLock::new(registry)))
///     .project_root("/path/to/project")
///     .build()
///     .unwrap();
/// ```
pub fn builder() -> ConversationsPluginBuilder {
    ConversationsPluginBuilder::new()
}

/// Helper function to format a session list item
///
/// Returns a formatted string suitable for display in a list.
///
/// # Arguments
///
/// * `session` - The session to format
/// * `adapter` - The adapter that owns this session
/// * `selected` - Whether this session is selected
///
/// # Returns
///
/// A formatted string with icon, name, and metadata
pub fn format_session_line(session: &Session, adapter: &dyn Adapter, selected: bool) -> String {
    let icon = adapter.icon();
    let name = &session.name;
    let msg_count = session.message_count;

    let date_str = session.updated_at.format("%Y-%m-%d %H:%M").to_string();
    let messages = compact_message_count(msg_count);

    if selected {
        format!("▶ {} {} ({}, {})", icon, name, messages, date_str)
    } else {
        format!("  {} {} ({}, {})", icon, name, messages, date_str)
    }
}

fn compact_message_count(count: usize) -> String {
    if count == 1 {
        "1 msg".to_string()
    } else {
        format!("{} msgs", count)
    }
}

/// Helper function to get the icon for a message role
///
/// # Arguments
///
/// * `role` - The message role
///
/// # Returns
///
/// An emoji character representing the role
pub fn role_icon(role: crate::core::models::conversation::Role) -> &'static str {
    use crate::core::models::conversation::Role;

    match role {
        Role::User => "👤",
        Role::Assistant => "🤖",
        Role::System => "⚙️ ",
        Role::Tool => "🔧",
    }
}

/// Helper function to get the display name for a message role
///
/// # Arguments
///
/// * `role` - The message role
///
/// # Returns
///
/// A human-readable role name
pub fn role_display_name(role: crate::core::models::conversation::Role) -> &'static str {
    use crate::core::models::conversation::Role;

    match role {
        Role::User => "User",
        Role::Assistant => "Assistant",
        Role::System => "System",
        Role::Tool => "Tool",
    }
}

/// Get the style for a message role
///
/// Returns a style with appropriate colors for the role.
///
/// # Arguments
///
/// * `role` - The message role
/// * `theme` - The current theme
///
/// # Returns
///
/// A ratatui `Style` object
pub fn role_style(
    role: crate::core::models::conversation::Role,
    theme: &crate::core::models::Theme,
) -> ratatui::style::Style {
    use crate::core::models::conversation::Role;
    use crate::theme::{UiElement, style_for_ui_element};
    use ratatui::style::Modifier;

    match role {
        Role::User => style_for_ui_element(theme, UiElement::Primary).add_modifier(Modifier::BOLD),
        Role::Assistant => {
            style_for_ui_element(theme, UiElement::Secondary).add_modifier(Modifier::BOLD)
        }
        Role::System => style_for_ui_element(theme, UiElement::Warning),
        Role::Tool => style_for_ui_element(theme, UiElement::Info),
    }
}

/// Default key bindings for the conversations plugin
///
/// Returns a map of key combinations to their actions.
pub fn default_key_bindings() -> Vec<(String, String)> {
    vec![
        ("↑/↓ or j/k".to_string(), "Navigate".to_string()),
        ("Enter/l/o".to_string(), "Open session".to_string()),
        ("Esc/h".to_string(), "Go back".to_string()),
        ("f".to_string(), "Filter".to_string()),
        ("r".to_string(), "Refresh current view".to_string()),
        ("g/G".to_string(), "First/Last".to_string()),
        ("PgUp/PgDn".to_string(), "Page up/down".to_string()),
        ("Space".to_string(), "Toggle expansion".to_string()),
        ("e".to_string(), "Expand all".to_string()),
        ("c".to_string(), "Collapse all".to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::types::AdapterType;
    use crate::core::models::conversation::{Role, Session};

    #[derive(Debug)]
    struct MockAdapter {
        adapter_type: AdapterType,
    }

    #[async_trait::async_trait]
    impl Adapter for MockAdapter {
        fn id(&self) -> &str {
            "mock"
        }

        fn name(&self) -> &str {
            "Mock"
        }

        fn icon(&self) -> char {
            self.adapter_type.icon()
        }

        fn adapter_type(&self) -> AdapterType {
            self.adapter_type
        }

        async fn detect(
            &self,
            _project_root: &std::path::Path,
        ) -> crate::adapters::types::Result<bool> {
            Ok(false)
        }

        async fn sessions(
            &self,
            _project_root: &std::path::Path,
        ) -> crate::adapters::types::Result<Vec<Session>> {
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

    #[test]
    fn test_role_icon() {
        assert_eq!(role_icon(Role::User), "👤");
        assert_eq!(role_icon(Role::Assistant), "🤖");
        assert_eq!(role_icon(Role::System), "⚙️ ");
        assert_eq!(role_icon(Role::Tool), "🔧");
    }

    #[test]
    fn test_role_display_name() {
        assert_eq!(role_display_name(Role::User), "User");
        assert_eq!(role_display_name(Role::Assistant), "Assistant");
        assert_eq!(role_display_name(Role::System), "System");
        assert_eq!(role_display_name(Role::Tool), "Tool");
    }

    #[test]
    fn test_format_session_line() {
        let adapter = MockAdapter {
            adapter_type: AdapterType::ClaudeCode,
        };
        let session = Session::new("test-id", "Test Session", "claude-code");

        let line = format_session_line(&session, &adapter, false);
        assert!(line.contains("Test Session"));
        assert!(line.contains('◈'));

        let selected_line = format_session_line(&session, &adapter, true);
        assert!(selected_line.contains("▶"));
    }

    #[test]
    fn test_format_session_line_uses_singular_message_count() {
        let adapter = MockAdapter {
            adapter_type: AdapterType::ClaudeCode,
        };
        let mut session = Session::new("test-id", "Test Session", "claude-code");
        session.message_count = 1;

        let line = format_session_line(&session, &adapter, false);

        assert!(line.contains("1 msg"));
        assert!(!line.contains("1 msgs"));
    }

    #[test]
    fn test_default_key_bindings() {
        let bindings = default_key_bindings();
        assert!(!bindings.is_empty());
        assert!(bindings.iter().any(|(k, _)| k.contains("↑/↓")));
        assert!(
            bindings
                .iter()
                .any(|(k, v)| k == "Enter/l/o" && v == "Open session")
        );
        assert!(bindings.iter().any(|(k, v)| k == "Esc/h" && v == "Go back"));
        assert!(!bindings.iter().any(|(k, _)| k == "Esc or h"));
        assert!(bindings.iter().any(|(k, v)| k == "f" && v == "Filter"));
        assert!(
            bindings
                .iter()
                .any(|(k, v)| k == "r" && v == "Refresh current view")
        );
    }

    #[test]
    fn test_builder() {
        let registry = Arc::new(RwLock::new(crate::adapters::AdapterRegistry::new()));
        let result = builder()
            .adapter_registry(registry)
            .project_root("/tmp")
            .build();
        assert!(result.is_ok());
    }
}
