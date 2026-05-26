//! Plugin registry system for RightClick.
//!
//! This module provides a generic plugin system that allows dynamic registration
//! and management of plugins. Plugins can handle events, render UI components,
//! and expose commands to command palette.
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
//! use rightclick::plugin::{Plugin, PluginContext, Registry, PluginCommand, Command, Category, Diagnostic};
//! use rightclick::core::models::Theme;
//! use rightclick::event::Event;
//! use rightclick::keymap::FocusContext;
//! use ratatui::{layout::Rect, buffer::Buffer};
//! use async_trait::async_trait;
//! use anyhow::Result;
//!
//! #[derive(Debug)]
//! struct ExamplePlugin {
//!     focused: bool,
//!     name: String,
//! }
//!
//! impl ExamplePlugin {
//!     pub fn new() -> Self {
//!         Self {
//!             focused: false,
//!             name: "Example".to_string(),
//!         }
//!     }
//! }
//!
//! #[async_trait]
//! impl Plugin for ExamplePlugin {
//!     fn id(&self) -> &str { "example" }
//!     fn name(&self) -> &str { &self.name }
//!     fn icon(&self) -> char { '?' }
//!
//!     async fn init(&mut self, _ctx: &PluginContext) -> Result<()> {
//!         // Initialize with context
//!         tracing::info!("Initializing {} plugin", self.name);
//!         Ok(())
//!     }
//!
//!     fn shutdown(&mut self) -> Result<()> {
//!         tracing::info!("Shutting down {} plugin", self.name);
//!         Ok(())
//!     }
//!
//!     fn handle_event(&mut self, event: Event) -> Vec<Command> {
//!         match event {
//!             Event::RefreshNeeded => vec![Command::new("example", "refresh")],
//!             _ => vec![],
//!         }
//!     }
//!
//!     fn render(&self, _area: Rect, _buf: &mut Buffer, _theme: &Theme) {
//!         // Render plugin UI
//!     }
//!
//!     fn is_focused(&self) -> bool { self.focused }
//!     fn set_focused(&mut self, focused: bool) { self.focused = focused; }
//!
//!     fn commands(&self) -> Vec<PluginCommand> {
//!         vec![
//!             PluginCommand::minimal(
//!                 "refresh",
//!                 "Refresh",
//!                 Category::System,
//!                 '?',
//!             ),
//!         ]
//!     }
//!
//!     fn focus_context(&self) -> FocusContext {
//!         FocusContext::Global
//!     }
//!
//!     fn start(&mut self) -> Vec<Command> {
//!         vec![]
//!     }
//!
//!     fn consumes_text_input(&self) -> bool {
//!         false
//!     }
//!
//!     fn diagnostics(&self) -> Vec<Diagnostic> {
//!         vec![]
//!     }
//!
//!     async fn update(&mut self) -> Result<()> {
//!         Ok(())
//!     }
//! }
//!
//! ```

use async_trait::async_trait;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::path::Path;
use thiserror::Error;

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
    /// Optional arguments for command
    pub args: Option<String>,
}

impl Command {
    /// Create a new command.
    ///
    /// # Arguments
    ///
    /// * `plugin_id` - The ID of plugin issuing command
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
    /// * `plugin_id` - The ID of plugin issuing command
    /// * `command_id` - The command identifier
    /// * `args` - Arguments for command
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

/// Successful plugin command execution metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginCommandExecution {
    /// Plugin that handled the command.
    pub plugin_id: String,
    /// Command identifier that was executed.
    pub command_id: String,
    /// Display name of the command.
    pub command_name: String,
    /// Commands emitted by the plugin while handling the execution event.
    pub emitted_commands: Vec<Command>,
}

/// Error returned when a plugin command cannot be executed.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PluginCommandError {
    /// The plugin does not expose the requested command identifier.
    #[error("plugin '{plugin_id}' does not expose command '{command_id}'")]
    UnknownCommand {
        /// Plugin that was asked to execute the command.
        plugin_id: String,
        /// Command identifier that could not be found.
        command_id: String,
    },
}

/// Category represents a logical grouping of commands for the command palette.
///
/// Categories allow the command palette to organize commands into sections
/// for better discoverability and user experience.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Category {
    /// Navigation commands (up, down, left, right, page up/down, etc.)
    Navigation = 0,
    /// Action commands (open, create, delete, confirm, cancel, etc.)
    Actions = 1,
    /// View commands (switch view, toggle view, switch tab, etc.)
    View = 2,
    /// Search commands (search, filter, find next/prev, etc.)
    Search = 3,
    /// Edit commands (edit, copy, paste, undo/redo, etc.)
    Edit = 4,
    /// Git commands (stage, unstage, commit, push, pull, fetch, etc.)
    Git = 5,
    /// System commands (quit, refresh, help, settings, etc.)
    System = 6,
}

impl Category {
    /// Returns a display-friendly name for this category.
    pub const fn display_name(&self) -> &'static str {
        match *self {
            Category::Navigation => "Navigation",
            Category::Actions => "Actions",
            Category::View => "View",
            Category::Search => "Search",
            Category::Edit => "Edit",
            Category::Git => "Git",
            Category::System => "System",
        }
    }

    /// Returns the underlying integer representation.
    pub const fn as_usize(&self) -> usize {
        *self as usize
    }
}

/// Diagnostic represents a health or status check result.
///
/// This allows plugins to report their current state for display
/// in status bars or health indicators.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    /// Unique identifier for this diagnostic
    pub id: String,
    /// Status indicator (e.g., "OK", "Warning", "Error", "Loading")
    pub status: String,
    /// Human-readable detail about what the diagnostic means
    pub detail: String,
}

impl Diagnostic {
    /// Create a new diagnostic.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier
    /// * `status` - Status indicator
    /// * `detail` - Human-readable detail
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::plugin::Diagnostic;
    ///
    /// let diag = Diagnostic::new(
    ///     "plugin-state",
    ///     "OK",
    ///     "Plugin is running normally",
    /// );
    /// assert_eq!(diag.id, "plugin-state");
    /// ```
    pub fn new(
        id: impl Into<String>,
        status: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            status: status.into(),
            detail: detail.into(),
        }
    }

    /// Create a new diagnostic with OK status.
    ///
    /// This is a convenience method for creating success diagnostics.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::plugin::Diagnostic;
    ///
    /// let diag = Diagnostic::ok("connection", "Connected to server");
    /// assert_eq!(diag.status, "OK");
    /// ```
    pub fn ok(id: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: "OK".to_string(),
            detail: detail.into(),
        }
    }

    /// Create a new warning diagnostic.
    ///
    /// This is a convenience method for creating warning diagnostics.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::plugin::Diagnostic;
    ///
    /// let diag = Diagnostic::warning("data-stale", "Data may be outdated");
    /// assert_eq!(diag.status, "Warning");
    /// ```
    pub fn warning(id: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: "Warning".to_string(),
            detail: detail.into(),
        }
    }

    /// Create a new error diagnostic.
    ///
    /// This is a convenience method for creating error diagnostics.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::plugin::Diagnostic;
    ///
    /// let diag = Diagnostic::error("connection-failed", "Could not connect to server");
    /// assert_eq!(diag.status, "Error");
    /// ```
    pub fn error(id: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status: "Error".to_string(),
            detail: detail.into(),
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
    /// Full description for palette display
    pub description: String,
    /// Category for organizing commands in palette
    pub category: Category,
    /// Keyboard shortcut character
    pub key: char,
    /// Focus context where this command is available
    pub context: FocusContext,
    /// Priority for footer display (1 = highest priority, 0 = default)
    pub priority: u8,
}

/// A searchable entity exposed by a plugin for global search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSearchEntry {
    /// Stable entry identifier within the plugin.
    pub id: String,
    /// Display title for the search result.
    pub title: String,
    /// Preview text shown below the title.
    pub preview: String,
}

impl PluginCommand {
    /// Create a new plugin command with all fields.
    ///
    /// # Arguments
    ///
    /// * `id` - The command identifier
    /// * `name` - The display name
    /// * `description` - Full description for palette
    /// * `category` - Logical grouping for palette
    /// * `key` - The keyboard shortcut character
    /// * `context` - The focus context where this command is available
    /// * `priority` - Footer display priority (1 = highest)
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::plugin::{PluginCommand, Category};
    /// use rightclick::keymap::FocusContext;
    ///
    /// let cmd = PluginCommand::new(
    ///     "refresh",
    ///     "Refresh",
    ///     "Reload current view data",
    ///     Category::System,
    ///     'r',
    ///     FocusContext::Global,
    ///     0,
    /// );
    /// assert_eq!(cmd.id, "refresh");
    /// assert_eq!(cmd.name, "Refresh");
    /// ```
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        category: Category,
        key: char,
        context: FocusContext,
        priority: u8,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            category,
            key,
            context,
            priority,
        }
    }

    /// Create a new plugin command with minimal fields.
    ///
    /// This is a convenience method for creating commands without
    /// description (empty description, default priority).
    ///
    /// # Arguments
    ///
    /// * `id` - The command identifier
    /// * `name` - The display name
    /// * `category` - Logical grouping for palette
    /// * `key` - The keyboard shortcut character
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::plugin::{PluginCommand, Category};
    /// use rightclick::keymap::FocusContext;
    ///
    /// let cmd = PluginCommand::minimal("refresh", "Refresh", Category::System, 'r');
    /// assert_eq!(cmd.id, "refresh");
    /// assert_eq!(cmd.name, "Refresh");
    /// ```
    pub fn minimal(
        id: impl Into<String>,
        name: impl Into<String>,
        category: Category,
        key: char,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            category,
            key,
            context: FocusContext::Global,
            priority: 0,
        }
    }

    /// Create a new plugin command with specific focus context and priority.
    ///
    /// This is a convenience method for creating commands with
    /// focus-specific context and priority.
    ///
    /// # Arguments
    ///
    /// * `id` - The command identifier
    /// * `name` - The display name
    /// * `description` - Full description for palette
    /// * `category` - Logical grouping for palette
    /// * `key` - The keyboard shortcut character
    /// * `context` - The focus context where this command is available
    /// * `priority` - Footer display priority (1 = highest)
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::plugin::{PluginCommand, Category};
    /// use rightclick::keymap::FocusContext;
    ///
    /// let cmd = PluginCommand::with_priority(
    ///     "stage",
    ///     "Stage",
    ///     "Stage selected file",
    ///     Category::Git,
    ///     's',
    ///     FocusContext::GitStatus,
    ///     1, // High priority
    /// );
    /// assert_eq!(cmd.id, "stage");
    /// assert_eq!(cmd.name, "Stage");
    /// ```
    pub fn with_priority(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        category: Category,
        key: char,
        context: FocusContext,
        priority: u8,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            category,
            key,
            context,
            priority,
        }
    }

    /// Create a new plugin command with a specific focus context.
    ///
    /// This is a convenience method for creating commands with
    /// a specific focus context. Uses empty description, System category,
    /// and default priority.
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
    /// use rightclick::plugin::{PluginCommand, Category};
    /// use rightclick::keymap::FocusContext;
    ///
    /// let cmd = PluginCommand::with_context("refresh", "Refresh", 'r', FocusContext::Global);
    /// assert_eq!(cmd.id, "refresh");
    /// assert_eq!(cmd.context, FocusContext::Global);
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
            description: String::new(),
            category: Category::System,
            key,
            context,
            priority: 0,
        }
    }

    /// Create a new plugin command with context and display description.
    pub fn with_context_description(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        key: char,
        context: FocusContext,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            category: Category::System,
            key,
            context,
            priority: 0,
        }
    }
}

/// A command exposed by a plugin for the command palette.
/// The plugin trait that all plugins must implement.
///
/// This trait defines the interface for plugins to integrate with RightClick.
/// Plugins can handle events, render UI, and expose commands to user.
///
/// # Implementing Plugins
///
/// Plugins must be `Send + Sync` as they may be accessed from multiple threads.
/// The `async_trait` macro is used for async initialization.
///
/// # Examples
///
/// ```
/// use rightclick::plugin::{Plugin, PluginContext, PluginCommand, Command, Category, Diagnostic};
/// use rightclick::core::models::Theme;
/// use rightclick::event::Event;
/// use rightclick::keymap::FocusContext;
/// use ratatui::{layout::Rect, buffer::Buffer};
/// use async_trait::async_trait;
/// use anyhow::Result;
///
/// #[derive(Debug)]
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
///     fn icon(&self) -> char { '?' }
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
///             PluginCommand::minimal(
///                 "refresh",
///                 "Refresh",
///                 Category::System,
///                 '?',
///             ),
///         ]
///     }
///
///     fn focus_context(&self) -> FocusContext {
///         FocusContext::Global
///     }
///
///     fn start(&mut self) -> Vec<Command> {
///         vec![]
///     }
///
///     fn consumes_text_input(&self) -> bool {
///         false
///     }
///
///     fn diagnostics(&self) -> Vec<Diagnostic> {
///         vec![]
///     }
///
///     async fn update(&mut self) -> Result<()> {
///         Ok(())
///     }
/// }
///
/// ```

#[async_trait]
pub trait Plugin: Send + Sync + std::fmt::Debug {
    /// Get unique identifier for this plugin.
    ///
    /// This ID should be unique across all plugins and is used for
    /// routing events and commands.
    fn id(&self) -> &str;

    /// Get human-readable name for this plugin.
    fn name(&self) -> &str;

    /// Get icon character for this plugin.
    fn icon(&self) -> char;

    /// Initialize plugin with the given context.
    ///
    /// This method is called once when plugin is registered.
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

    /// Get a compact status line for the global footer.
    ///
    /// Plugins can override this to expose the most useful current state
    /// without forcing the shell to know plugin-specific internals.
    fn status_line(&self) -> Option<String> {
        None
    }

    /// Reveal or select a filesystem path if the plugin supports file navigation.
    ///
    /// This keeps global search activation generic while allowing file-oriented
    /// plugins to make file results actionable.
    fn reveal_path(&mut self, _path: &Path) -> bool {
        false
    }

    /// Get searchable entries exposed by this plugin.
    ///
    /// Plugins can override this to make domain objects discoverable from the
    /// global search overlay without leaking plugin-specific state to the shell.
    fn search_entries(&self) -> Vec<PluginSearchEntry> {
        Vec::new()
    }

    /// Activate a searchable entry previously returned by [`Plugin::search_entries`].
    ///
    /// Returns `true` when the plugin handled the entry identifier.
    fn activate_search_result(&mut self, _entry_id: &str) -> bool {
        false
    }

    /// Execute a command exposed by this plugin by command identifier.
    ///
    /// The default implementation keeps command activation centralized by
    /// translating command metadata into the same key event path used for
    /// direct keyboard shortcuts.
    fn execute_command(
        &mut self,
        command_id: &str,
    ) -> Result<PluginCommandExecution, PluginCommandError> {
        let plugin_id = self.id().to_string();
        let command = self
            .commands()
            .into_iter()
            .find(|command| command.id == command_id)
            .ok_or_else(|| PluginCommandError::UnknownCommand {
                plugin_id: plugin_id.clone(),
                command_id: command_id.to_string(),
            })?;

        let emitted_commands = self.handle_event(Event::Key {
            code: command.key.to_string(),
            modifiers: crate::event::KeyModifiers {
                shift: command.key.is_ascii_uppercase(),
                ..Default::default()
            },
        });

        Ok(PluginCommandExecution {
            plugin_id,
            command_id: command.id,
            command_name: command.name,
            emitted_commands,
        })
    }

    /// Get the current focus context for this plugin.
    ///
    /// This is used to determine which keyboard shortcuts are active
    /// based on where focus is in the UI.
    fn focus_context(&self) -> FocusContext;

    /// Start the plugin and return initial commands.
    ///
    /// This method is called after `init()` completes and allows plugins
    /// to return commands that should be executed on startup.
    ///
    /// # Returns
    ///
    /// A vector of commands to execute after plugin starts.
    fn start(&mut self) -> Vec<Command> {
        vec![]
    }

    /// Check if this plugin consumes alphanumeric text input.
    ///
    /// Plugins that implement text input (e.g., search boxes, filter inputs)
    /// should return `true` to receive raw character events instead of
    /// having them intercepted as keyboard shortcuts.
    ///
    /// # Returns
    ///
    /// `true` if this plugin wants to consume text input, `false` otherwise.
    fn consumes_text_input(&self) -> bool {
        false
    }

    /// Get diagnostics from this plugin.
    ///
    /// Plugins can implement this to report health/status information.
    /// This allows the application to display plugin status indicators.
    ///
    /// # Returns
    ///
    /// A vector of diagnostic information about the plugin's state.
    fn diagnostics(&self) -> Vec<Diagnostic> {
        vec![]
    }

    /// Update the plugin state asynchronously.
    ///
    /// This method is called regularly by the application loop to allow
    /// plugins to perform async operations like loading data.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if the update failed.
    async fn update(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    #[derive(Debug, Default)]
    struct TestPlugin {
        last_code: Option<String>,
        last_shift: bool,
    }

    #[async_trait]
    impl Plugin for TestPlugin {
        fn id(&self) -> &str {
            "test"
        }

        fn name(&self) -> &str {
            "Test"
        }

        fn icon(&self) -> char {
            'T'
        }

        async fn init(&mut self, _ctx: &PluginContext) -> anyhow::Result<()> {
            Ok(())
        }

        fn shutdown(&mut self) -> anyhow::Result<()> {
            Ok(())
        }

        fn handle_event(&mut self, event: Event) -> Vec<Command> {
            if let Event::Key { code, modifiers } = event {
                self.last_code = Some(code);
                self.last_shift = modifiers.shift;
            }
            Vec::new()
        }

        fn render(&self, _area: Rect, _buf: &mut Buffer, _theme: &Theme) {}

        fn is_focused(&self) -> bool {
            false
        }

        fn set_focused(&mut self, _focused: bool) {}

        fn commands(&self) -> Vec<PluginCommand> {
            vec![PluginCommand::minimal(
                "delete",
                "Delete",
                Category::Actions,
                'D',
            )]
        }

        fn focus_context(&self) -> FocusContext {
            FocusContext::Global
        }
    }

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
        let cmd = PluginCommand::minimal("refresh", "Refresh View", Category::System, '?');
        assert_eq!(cmd.id, "refresh");
        assert_eq!(cmd.name, "Refresh View");
        assert_eq!(cmd.key, '?');
        assert_eq!(cmd.context, FocusContext::Global);
    }

    #[test]
    fn test_plugin_command_with_priority() {
        let cmd = PluginCommand::with_priority(
            "stage",
            "Stage",
            "Stage selected file",
            Category::Git,
            's',
            FocusContext::GitStatus,
            1,
        );
        assert_eq!(cmd.id, "stage");
        assert_eq!(cmd.name, "Stage");
    }

    #[test]
    fn test_plugin_command_with_context_description() {
        let cmd = PluginCommand::with_context_description(
            "refresh",
            "Refresh",
            "Reload current data",
            'r',
            FocusContext::GitStatus,
        );

        assert_eq!(cmd.id, "refresh");
        assert_eq!(cmd.name, "Refresh");
        assert_eq!(cmd.description, "Reload current data");
        assert_eq!(cmd.context, FocusContext::GitStatus);
    }

    #[test]
    fn test_execute_command_dispatches_declared_key_event() {
        let mut plugin = TestPlugin::default();

        let result = plugin.execute_command("delete");

        let execution = result.expect("command should execute");
        assert_eq!(execution.plugin_id, "test");
        assert_eq!(execution.command_id, "delete");
        assert_eq!(execution.command_name, "Delete");
        assert!(execution.emitted_commands.is_empty());
        assert_eq!(plugin.last_code.as_deref(), Some("D"));
        assert!(plugin.last_shift);
    }

    #[test]
    fn test_execute_command_returns_error_for_unknown_command() {
        let mut plugin = TestPlugin::default();

        let result = plugin.execute_command("missing");

        assert_eq!(
            result,
            Err(PluginCommandError::UnknownCommand {
                plugin_id: "test".to_string(),
                command_id: "missing".to_string(),
            })
        );
        assert!(plugin.last_code.is_none());
    }

    #[test]
    fn test_default_reveal_path_is_not_handled() {
        let mut plugin = TestPlugin::default();

        assert!(!plugin.reveal_path(std::path::Path::new("src/main.rs")));
    }

    #[test]
    fn test_default_search_entries_are_empty_and_activation_is_not_handled() {
        let mut plugin = TestPlugin::default();

        assert!(plugin.search_entries().is_empty());
        assert!(!plugin.activate_search_result("anything"));
    }
}
