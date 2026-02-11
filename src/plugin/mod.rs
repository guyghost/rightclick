//! Plugin registry system for RightClick.
//!
//! This module provides a generic plugin system that allows dynamic registration
//! and management of plugins. Plugins can handle events, render UI components,
//! and expose commands to the command palette.
//!
//! # Architecture
//!
//! The plugin system consists of:
//! - [`Plugin`] trait - The interface all plugins must implement
//! - [`Registry`] - Manages plugin registration and lifecycle
//! - [`PluginContext`] - Provides plugins with access to shared resources
//!
//! # Examples
//!
//! ```
//! use rightclick::plugin::{Plugin, PluginContext, Registry, PluginCommand, Command};
//! use rightclick::core::models::Theme;
//! use rightclick::event::Event;
//! use rightclick::keymap::FocusContext;
//! use ratatui::{layout::Rect, buffer::Buffer};
//! use async_trait::async_trait;
//! use anyhow::Result;
//!
//! struct MyPlugin {
//!     focused: bool,
//! }
//!
//! #[async_trait]
//! impl Plugin for MyPlugin {
//!     fn id(&self) -> &str { "my-plugin" }
//!     fn name(&self) -> &str { "My Plugin" }
//!     fn icon(&self) -> char { '⚡' }
//!
//!     async fn init(&mut self, _ctx: &PluginContext) -> Result<()> { Ok(()) }
//!     fn shutdown(&mut self) -> Result<()> { Ok(()) }
//!
//!     fn handle_event(&mut self, _event: Event) -> Vec<Command> { vec![] }
//!     fn render(&self, _area: Rect, _buf: &mut Buffer, _theme: &Theme) {}
//!
//!     fn is_focused(&self) -> bool { self.focused }
//!     fn set_focused(&mut self, focused: bool) { self.focused = focused; }
//!
//!     fn commands(&self) -> Vec<PluginCommand> { vec![] }
//!     fn focus_context(&self) -> FocusContext { FocusContext::Global }
//! }
//! ```

use async_trait::async_trait;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::core::models::Theme;
use crate::event::Event;
use crate::keymap::FocusContext;

pub mod context;
pub mod registry;

pub use context::PluginContext;
pub use registry::Registry;

/// A command that can be executed by a plugin.
///
/// Commands are returned by plugins in response to events and are
/// processed by the application to perform actions.
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    /// The plugin that issued this command
    pub plugin_id: String,
    /// The command identifier
    pub command_id: String,
    /// Optional arguments for the command
    pub args: Option<String>,
}

impl Command {
    /// Create a new command.
    ///
    /// # Arguments
    ///
    /// * `plugin_id` - The ID of the plugin issuing the command
    /// * `command_id` - The command identifier
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::plugin::Command;
    ///
    /// let cmd = Command::new("workspace", "refresh");
    /// assert_eq!(cmd.plugin_id, "workspace");
    /// assert_eq!(cmd.command_id, "refresh");
    /// ```
    pub fn new(plugin_id: impl Into<String>, command_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            command_id: command_id.into(),
            args: None,
        }
    }

    /// Create a new command with arguments.
    ///
    /// # Arguments
    ///
    /// * `plugin_id` - The ID of the plugin issuing the command
    /// * `command_id` - The command identifier
    /// * `args` - Arguments for the command
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::plugin::Command;
    ///
    /// let cmd = Command::with_args("workspace", "create", "feature-branch");
    /// assert_eq!(cmd.args, Some("feature-branch".to_string()));
    /// ```
    pub fn with_args(
        plugin_id: impl Into<String>,
        command_id: impl Into<String>,
        args: impl Into<String>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            command_id: command_id.into(),
            args: Some(args.into()),
        }
    }
}

/// A command exposed by a plugin for the command palette.
///
/// These commands appear in the command palette and can be triggered
/// by the user with a keyboard shortcut.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginCommand {
    /// Command identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Keyboard shortcut character
    pub key: char,
    /// Focus context where this command is available
    pub context: FocusContext,
}

impl PluginCommand {
    /// Create a new plugin command.
    ///
    /// # Arguments
    ///
    /// * `id` - The command identifier
    /// * `name` - The display name
    /// * `key` - The keyboard shortcut character
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::plugin::PluginCommand;
    /// use rightclick::keymap::FocusContext;
    ///
    /// let cmd = PluginCommand::new("refresh", "Refresh View", 'r');
    /// assert_eq!(cmd.id, "refresh");
    /// assert_eq!(cmd.name, "Refresh View");
    /// assert_eq!(cmd.key, 'r');
    /// assert_eq!(cmd.context, FocusContext::Global);
    /// ```
    pub fn new(id: impl Into<String>, name: impl Into<String>, key: char) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            key,
            context: FocusContext::Global,
        }
    }

    /// Create a new plugin command with a specific focus context.
    ///
    /// # Arguments
    ///
    /// * `id` - The command identifier
    /// * `name` - The display name
    /// * `key` - The keyboard shortcut character
    /// * `context` - The focus context where this command is available
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::plugin::PluginCommand;
    /// use rightclick::keymap::FocusContext;
    ///
    /// let cmd = PluginCommand::with_context("delete", "Delete", 'd', FocusContext::Workspace);
    /// assert_eq!(cmd.context, FocusContext::Workspace);
    /// ```
    pub fn with_context(
        id: impl Into<String>,
        name: impl Into<String>,
        key: char,
        context: FocusContext,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            key,
            context,
        }
    }
}

/// The plugin trait that all plugins must implement.
///
/// This trait defines the interface for plugins to integrate with RightClick.
/// Plugins can handle events, render UI, and expose commands to the user.
///
/// # Implementing Plugins
///
/// Plugins must be `Send + Sync` as they may be accessed from multiple threads.
/// The `async_trait` macro is used for async initialization.
///
/// # Examples
///
/// ```
/// use rightclick::plugin::{Plugin, PluginContext, PluginCommand, Command};
/// use rightclick::core::models::Theme;
/// use rightclick::event::Event;
/// use rightclick::keymap::FocusContext;
/// use ratatui::{layout::Rect, buffer::Buffer};
/// use async_trait::async_trait;
/// use anyhow::Result;
///
/// pub struct ExamplePlugin {
///     focused: bool,
///     name: String,
/// }
///
/// impl ExamplePlugin {
///     pub fn new() -> Self {
///         Self {
///             focused: false,
///             name: "Example".to_string(),
///         }
///     }
/// }
///
/// #[async_trait]
/// impl Plugin for ExamplePlugin {
///     fn id(&self) -> &str { "example" }
///     fn name(&self) -> &str { &self.name }
///     fn icon(&self) -> char { '📦' }
///
///     async fn init(&mut self, ctx: &PluginContext) -> Result<()> {
///         // Initialize with context
///         tracing::info!("Initializing {} plugin", self.name);
///         Ok(())
///     }
///
///     fn shutdown(&mut self) -> Result<()> {
///         tracing::info!("Shutting down {} plugin", self.name);
///         Ok(())
///     }
///
///     fn handle_event(&mut self, event: Event) -> Vec<Command> {
///         match event {
///             Event::RefreshNeeded => vec![Command::new("example", "refresh")],
///             _ => vec![],
///         }
///     }
///
///     fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
///         // Render plugin UI
///     }
///
///     fn is_focused(&self) -> bool { self.focused }
///     fn set_focused(&mut self, focused: bool) { self.focused = focused; }
///
///     fn commands(&self) -> Vec<PluginCommand> {
///         vec![
///             PluginCommand::new("refresh", "Refresh", 'r'),
///         ]
///     }
///
///     fn focus_context(&self) -> FocusContext {
///         FocusContext::Global
///     }
/// }
/// ```
#[async_trait]
pub trait Plugin: Send + Sync + std::fmt::Debug {
    /// Get the unique identifier for this plugin.
    ///
    /// This ID should be unique across all plugins and is used for
    /// routing events and commands.
    fn id(&self) -> &str;

    /// Get the human-readable name for this plugin.
    fn name(&self) -> &str;

    /// Get the icon character for this plugin.
    fn icon(&self) -> char;

    /// Initialize the plugin with the given context.
    ///
    /// This method is called once when the plugin is registered.
    /// It should perform any necessary setup, such as loading state
    /// or connecting to external resources.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The plugin context providing access to shared resources
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful initialization, or an error if
    /// initialization failed.
    async fn init(&mut self, ctx: &PluginContext) -> anyhow::Result<()>;

    /// Shutdown the plugin.
    ///
    /// This method is called when the application is shutting down or
    /// when the plugin is being unregistered. It should perform any
    /// necessary cleanup.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful shutdown, or an error if cleanup failed.
    fn shutdown(&mut self) -> anyhow::Result<()>;

    /// Handle an event and return any commands to execute.
    ///
    /// This method is called when an event is dispatched to the plugin.
    /// The plugin can process the event and optionally return commands
    /// to be executed by the application.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to handle
    ///
    /// # Returns
    ///
    /// A vector of commands to execute, or an empty vector if no action is needed.
    fn handle_event(&mut self, event: Event) -> Vec<Command>;

    /// Render the plugin UI.
    ///
    /// This method is called during the render phase to draw the plugin's
    /// UI within the given area using the provided buffer and theme.
    ///
    /// # Arguments
    ///
    /// * `area` - The rectangular area to render within
    /// * `buf` - The buffer to draw to
    /// * `theme` - The current theme for styling
    fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme);

    /// Check if this plugin is currently focused.
    fn is_focused(&self) -> bool;

    /// Set the focus state of this plugin.
    ///
    /// # Arguments
    ///
    /// * `focused` - `true` to focus the plugin, `false` to unfocus
    fn set_focused(&mut self, focused: bool);

    /// Get the list of commands exposed by this plugin.
    ///
    /// These commands appear in the command palette and can be triggered
    /// by the user.
    fn commands(&self) -> Vec<PluginCommand>;

    /// Get the current focus context for this plugin.
    ///
    /// This is used to determine which keyboard shortcuts are active
    /// based on where focus is in the UI.
    fn focus_context(&self) -> FocusContext;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_new() {
        let cmd = Command::new("workspace", "refresh");
        assert_eq!(cmd.plugin_id, "workspace");
        assert_eq!(cmd.command_id, "refresh");
        assert!(cmd.args.is_none());
    }

    #[test]
    fn test_command_with_args() {
        let cmd = Command::with_args("workspace", "create", "feature");
        assert_eq!(cmd.plugin_id, "workspace");
        assert_eq!(cmd.command_id, "create");
        assert_eq!(cmd.args, Some("feature".to_string()));
    }

    #[test]
    fn test_plugin_command_new() {
        let cmd = PluginCommand::new("refresh", "Refresh View", 'r');
        assert_eq!(cmd.id, "refresh");
        assert_eq!(cmd.name, "Refresh View");
        assert_eq!(cmd.key, 'r');
        assert_eq!(cmd.context, FocusContext::Global);
    }

    #[test]
    fn test_plugin_command_with_context() {
        let cmd = PluginCommand::with_context("delete", "Delete", 'd', FocusContext::Workspace);
        assert_eq!(cmd.id, "delete");
        assert_eq!(cmd.name, "Delete");
        assert_eq!(cmd.key, 'd');
        assert_eq!(cmd.context, FocusContext::Workspace);
    }
}
