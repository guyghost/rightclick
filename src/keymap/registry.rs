//! Keymap registry for managing commands and bindings.
//!
//! The registry stores all available commands and their associated key bindings.
//! It supports context-aware lookup, allowing different shortcuts in different
//! UI contexts, and user overrides for customization.

use std::collections::HashMap;

use super::bindings::{Action, Binding};
use super::context::FocusContext;
use super::sequence::{KeySequenceBuffer, SequenceResult};

/// A command that can be triggered by a keyboard shortcut.
///
/// Commands encapsulate the metadata (name, description) and the handler
/// function that produces an `Action` when executed.
pub struct Command {
    /// Unique identifier for this command (e.g., "app.quit").
    pub id: String,

    /// Human-readable name for display in help and palette.
    pub name: String,

    /// Optional description providing more details about the command.
    pub description: Option<String>,

    /// The handler function that produces an action when the command is invoked.
    ///
    /// This is stored in a `Box` to allow different closures and function
    /// types to be stored in the registry. The `Send + Sync` bounds ensure
    /// thread safety for use in async contexts.
    pub handler: Box<dyn Fn() -> Action + Send + Sync>,
}

impl Command {
    /// Creates a new command with the given id, name, and handler.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique command identifier
    /// * `name` - Human-readable name
    /// * `handler` - Function that produces an Action
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::{Command, Action};
    ///
    /// let cmd = Command::new("app.quit", "Quit", || Action::Quit);
    /// assert_eq!(cmd.id, "app.quit");
    /// assert_eq!(cmd.name, "Quit");
    /// ```
    pub fn new<I: Into<String>, N: Into<String>, F>(id: I, name: N, handler: F) -> Self
    where
        F: Fn() -> Action + Send + Sync + 'static,
    {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            handler: Box::new(handler),
        }
    }

    /// Sets the description for this command.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::{Command, Action};
    ///
    /// let cmd = Command::new("app.quit", "Quit", || Action::Quit)
    ///     .with_description("Exit the application");
    /// assert_eq!(cmd.description, Some("Exit the application".to_string()));
    /// ```
    pub fn with_description<D: Into<String>>(mut self, description: D) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Executes the command handler and returns the resulting action.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::{Command, Action};
    ///
    /// let cmd = Command::new("app.refresh", "Refresh", || Action::Refresh);
    /// let action = cmd.execute();
    /// assert!(matches!(action, Action::Refresh));
    /// ```
    pub fn execute(&self) -> Action {
        (self.handler)()
    }
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Command")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("description", &self.description)
            .field("handler", &"<function>")
            .finish()
    }
}

/// Registry for commands and key bindings.
///
/// The registry maintains mappings between:
/// - Command IDs and their command definitions
/// - Focus contexts and the bindings active in each context
/// - User overrides for key customization
#[derive(Debug)]
pub struct Registry {
    /// Map of command IDs to their command definitions.
    commands: HashMap<String, Command>,

    /// Map of focus contexts to their active bindings.
    bindings: HashMap<FocusContext, Vec<Binding>>,

    /// User-defined key overrides mapping key strings to command IDs.
    /// These take precedence over default bindings.
    user_overrides: HashMap<String, String>,

    /// Buffer for multi-key sequence detection (e.g., `gg`, `dd`).
    sequence_buffer: KeySequenceBuffer,
}

/// Type alias for backward compatibility with plugins
pub type KeyBindingRegistry = Registry;

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    /// Creates a new, empty keymap registry.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::Registry;
    ///
    /// let registry = Registry::new();
    /// ```
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            bindings: HashMap::new(),
            user_overrides: HashMap::new(),
            sequence_buffer: KeySequenceBuffer::new(),
        }
    }

    /// Creates a new registry with all default key bindings registered.
    ///
    /// This is the recommended way to initialize the registry for the application.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::Registry;
    ///
    /// let registry = Registry::with_defaults();
    /// ```
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register_default_commands();
        registry.register_default_bindings();
        registry
    }

    /// Registers a command in the registry.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to register
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::{Registry, Command, Action};
    ///
    /// let mut registry = Registry::new();
    /// let cmd = Command::new("app.quit", "Quit", || Action::Quit);
    /// registry.register_command(cmd);
    /// ```
    pub fn register_command(&mut self, command: Command) {
        self.commands.insert(command.id.clone(), command);
    }

    /// Registers a key binding in the registry.
    ///
    /// # Arguments
    ///
    /// * `binding` - The binding to register
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::{Registry, Binding, FocusContext};
    ///
    /// let mut registry = Registry::new();
    /// let binding = Binding::new("q", "app.quit", FocusContext::Global);
    /// registry.register_binding(binding);
    /// ```
    pub fn register_binding(&mut self, binding: Binding) {
        self.bindings
            .entry(binding.context)
            .or_default()
            .push(binding);
    }

    /// Registers a key binding with the given key, action, and context.
    ///
    /// This is a convenience method that creates a `Binding` and registers it.
    ///
    /// # Arguments
    ///
    /// * `key` - The key combination string (e.g., "j", "ctrl+c")
    /// * `action` - The action to associate with this key
    /// * `context` - The focus context for this binding
    pub fn bind(&mut self, key: &str, action: Action, context: FocusContext) {
        // Convert action to a command id
        let command_id = format!("action.{:?}", action).to_lowercase();
        // Register the command if not already present
        if !self.commands.contains_key(&command_id) {
            let cmd = Command::new(&command_id, &command_id, move || {
                Action::Custom(Box::new(()))
            });
            self.register_command(cmd);
        }
        let binding = Binding::new(key, command_id, context);
        self.register_binding(binding);
    }

    /// Sets a user override for a key binding.
    ///
    /// User overrides take precedence over default bindings.
    ///
    /// # Arguments
    ///
    /// * `key` - The key combination to override
    /// * `command_id` - The command ID to bind to this key
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::Registry;
    ///
    /// let mut registry = Registry::new();
    /// registry.set_user_override("q".to_string(), "app.refresh".to_string());
    /// ```
    pub fn set_user_override(&mut self, key: String, command_id: String) {
        self.user_overrides.insert(key, command_id);
    }

    /// Removes a user override for a key.
    ///
    /// # Arguments
    ///
    /// * `key` - The key combination to remove the override for
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::Registry;
    ///
    /// let mut registry = Registry::new();
    /// registry.set_user_override("q".to_string(), "app.refresh".to_string());
    /// registry.remove_user_override("q");
    /// ```
    pub fn remove_user_override(&mut self, key: &str) {
        self.user_overrides.remove(key);
    }

    /// Clears all user overrides.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::Registry;
    ///
    /// let mut registry = Registry::new();
    /// registry.set_user_override("q".to_string(), "app.refresh".to_string());
    /// registry.clear_user_overrides();
    /// ```
    pub fn clear_user_overrides(&mut self) {
        self.user_overrides.clear();
    }

    /// Handles a key press in the given context.
    ///
    /// This method looks up the key in the following order:
    /// 1. User overrides (highest priority)
    /// 2. Context-specific bindings
    /// 3. Global bindings (lowest priority)
    ///
    /// # Arguments
    ///
    /// * `key` - The key combination that was pressed
    /// * `context` - The current focus context
    ///
    /// # Returns
    ///
    /// The action to execute, or `None` if no binding was found.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::{Registry, Command, Binding, Action, FocusContext};
    ///
    /// let mut registry = Registry::new();
    /// registry.register_command(Command::new("app.quit", "Quit", || Action::Quit));
    /// registry.register_binding(Binding::new("q", "app.quit", FocusContext::Global));
    ///
    /// let action = registry.handle("q", FocusContext::Global);
    /// assert!(action.is_some());
    /// ```
    pub fn handle(&self, key: &str, context: FocusContext) -> Option<Action> {
        // First, check user overrides
        if let Some(command_id) = self.user_overrides.get(key) {
            if let Some(command) = self.commands.get(command_id) {
                return Some(command.execute());
            }
        }

        // Then, check context-specific bindings
        if let Some(bindings) = self.bindings.get(&context) {
            for binding in bindings {
                if binding.key == key {
                    if let Some(command) = self.commands.get(&binding.command_id) {
                        return Some(command.execute());
                    }
                }
            }
        }

        // Finally, check global bindings
        if context != FocusContext::Global {
            if let Some(bindings) = self.bindings.get(&FocusContext::Global) {
                for binding in bindings {
                    if binding.key == key {
                        if let Some(command) = self.commands.get(&binding.command_id) {
                            return Some(command.execute());
                        }
                    }
                }
            }
        }

        None
    }

    /// Handles a key press with multi-key sequence support.
    ///
    /// This method first feeds the key into the sequence buffer. If the
    /// buffer resolves a sequence, the corresponding command is executed.
    /// If the key passes through (not part of any sequence), normal
    /// [`handle`](Self::handle) is called as a fallback.
    ///
    /// # Arguments
    ///
    /// * `key` - The key combination that was pressed
    /// * `context` - The current focus context
    ///
    /// # Returns
    ///
    /// A tuple of `(Action, Option<usize>)` where the second element is
    /// an optional count prefix, or `None` if no binding was found.
    pub fn handle_with_sequence(
        &mut self,
        key: &str,
        context: FocusContext,
    ) -> Option<(Action, Option<usize>)> {
        match self.sequence_buffer.feed(key, context) {
            SequenceResult::Resolved { command_id, count } => self
                .commands
                .get(&command_id)
                .map(|command| (command.execute(), count)),
            SequenceResult::PassThrough { key, count } => {
                self.handle(&key, context).map(|action| (action, count))
            }
            SequenceResult::Pending { .. } => None,
            SequenceResult::Timeout { key } => {
                self.handle(&key, context).map(|action| (action, None))
            }
        }
    }

    /// Checks for sequence timeout and returns the resolved action if timed out.
    ///
    /// This should be called from the application's tick loop.
    ///
    /// # Returns
    ///
    /// A tuple of `(Action, Option<usize>)` if the timed-out key maps to
    /// a command, or `None` otherwise.
    pub fn check_sequence_timeout(&mut self) -> Option<(Action, Option<usize>)> {
        if let Some(result) = self.sequence_buffer.check_timeout() {
            match result {
                SequenceResult::Timeout { key } => {
                    // We don't have context here, so try Global
                    self.handle(&key, FocusContext::Global)
                        .map(|action| (action, None))
                }
                _ => None,
            }
        } else {
            None
        }
    }

    /// Returns the current pending display from the sequence buffer.
    ///
    /// This can be used to show the user what keys have been typed so far
    /// in the footer or status bar.
    pub fn pending_sequence_display(&self) -> Option<String> {
        self.sequence_buffer.pending_display()
    }

    /// Returns a mutable reference to the sequence buffer.
    ///
    /// This allows callers to register custom sequences or configure
    /// the buffer directly.
    pub fn sequence_buffer_mut(&mut self) -> &mut KeySequenceBuffer {
        &mut self.sequence_buffer
    }

    /// Returns a reference to a command by its ID.
    ///
    /// # Arguments
    ///
    /// * `command_id` - The command identifier
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::{Registry, Command, Action};
    ///
    /// let mut registry = Registry::new();
    /// registry.register_command(Command::new("app.quit", "Quit", || Action::Quit));
    ///
    /// let cmd = registry.get_command("app.quit");
    /// assert!(cmd.is_some());
    /// ```
    pub fn get_command(&self, command_id: &str) -> Option<&Command> {
        self.commands.get(command_id)
    }

    /// Returns all bindings for a given context.
    ///
    /// # Arguments
    ///
    /// * `context` - The focus context
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::{Registry, Binding, FocusContext};
    ///
    /// let mut registry = Registry::new();
    /// registry.register_binding(Binding::new("q", "app.quit", FocusContext::Global));
    ///
    /// let bindings = registry.get_bindings(FocusContext::Global);
    /// assert_eq!(bindings.len(), 1);
    /// ```
    pub fn get_bindings(&self, context: FocusContext) -> &[Binding] {
        self.bindings
            .get(&context)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Returns all registered commands.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::{Registry, Command, Action};
    ///
    /// let mut registry = Registry::new();
    /// registry.register_command(Command::new("app.quit", "Quit", || Action::Quit));
    ///
    /// let commands = registry.commands();
    /// assert_eq!(commands.len(), 1);
    /// ```
    pub fn commands(&self) -> &HashMap<String, Command> {
        &self.commands
    }

    /// Returns all user overrides.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::keymap::Registry;
    ///
    /// let mut registry = Registry::new();
    /// registry.set_user_override("q".to_string(), "app.refresh".to_string());
    ///
    /// let overrides = registry.user_overrides();
    /// assert_eq!(overrides.len(), 1);
    /// ```
    pub fn user_overrides(&self) -> &HashMap<String, String> {
        &self.user_overrides
    }

    /// Registers all default application commands.
    fn register_default_commands(&mut self) {
        // Application commands
        self.register_command(
            Command::new("app.quit", "Quit", || Action::Quit)
                .with_description("Exit the application"),
        );
        self.register_command(
            Command::new("app.refresh", "Refresh", || Action::Refresh)
                .with_description("Refresh the current view"),
        );
        self.register_command(
            Command::new("app.palette", "Command Palette", || Action::OpenPalette)
                .with_description("Open the command palette"),
        );
        self.register_command(
            Command::new("app.help", "Help", || Action::OpenHelp)
                .with_description("Open the help view"),
        );
        self.register_command(
            Command::new("app.back", "Back", || Action::Back)
                .with_description("Go back to the previous view"),
        );
        self.register_command(
            Command::new("app.search", "Search", || Action::Search)
                .with_description("Search in the current view"),
        );
        self.register_command(
            Command::new("app.filter", "Filter", || Action::Filter)
                .with_description("Filter the current view"),
        );
        self.register_command(
            Command::new("app.context_menu", "Context Menu", || Action::ContextMenu)
                .with_description("Show context menu"),
        );

        // Navigation commands
        self.register_command(
            Command::new("nav.up", "Navigate Up", || Action::NavigateUp)
                .with_description("Move cursor up"),
        );
        self.register_command(
            Command::new("nav.down", "Navigate Down", || Action::NavigateDown)
                .with_description("Move cursor down"),
        );
        self.register_command(
            Command::new("nav.left", "Navigate Left", || Action::NavigateLeft)
                .with_description("Move cursor left or collapse"),
        );
        self.register_command(
            Command::new("nav.right", "Navigate Right", || Action::NavigateRight)
                .with_description("Move cursor right or expand"),
        );
        self.register_command(
            Command::new("nav.select", "Select", || Action::Select)
                .with_description("Select the current item"),
        );
        self.register_command(
            Command::new("nav.first", "Navigate First", || Action::NavigateFirst)
                .with_description("Navigate to the first item"),
        );
        self.register_command(
            Command::new("nav.last", "Navigate Last", || Action::NavigateLast)
                .with_description("Navigate to the last item"),
        );

        // Clipboard operations
        self.register_command(
            Command::new("clipboard.copy", "Copy to Clipboard", || Action::Copy)
                .with_description("Copy the current item to clipboard"),
        );

        // File operations
        self.register_command(
            Command::new("file.open", "Open", || Action::Open)
                .with_description("Open the selected file"),
        );
        self.register_command(
            Command::new("file.create", "Create", || Action::Create)
                .with_description("Create a new file"),
        );
        self.register_command(
            Command::new("file.delete", "Delete", || Action::Delete)
                .with_description("Delete the selected file"),
        );
        self.register_command(
            Command::new("file.edit", "Edit", || Action::Edit)
                .with_description("Edit the selected file"),
        );
        self.register_command(
            Command::new("file.copy", "Copy", || Action::Copy)
                .with_description("Copy the selected file"),
        );
        self.register_command(
            Command::new("file.paste", "Paste", || Action::Paste)
                .with_description("Paste from clipboard"),
        );

        // Tree operations
        self.register_command(
            Command::new("tree.expand", "Expand", || Action::Expand)
                .with_description("Expand the current node"),
        );
        self.register_command(
            Command::new("tree.collapse", "Collapse", || Action::Collapse)
                .with_description("Collapse the current node"),
        );
        self.register_command(
            Command::new("tree.toggle", "Toggle", || Action::Toggle)
                .with_description("Toggle the current node"),
        );

        // Git commands
        self.register_command(
            Command::new("git.stage", "Stage", || Action::Stage).with_description("Stage changes"),
        );
        self.register_command(
            Command::new("git.unstage", "Unstage", || Action::Unstage)
                .with_description("Unstage changes"),
        );
        self.register_command(
            Command::new("git.commit", "Commit", || Action::Commit)
                .with_description("Commit staged changes"),
        );
        self.register_command(
            Command::new("git.push", "Push", || Action::Push).with_description("Push commits"),
        );
        self.register_command(
            Command::new("git.pull", "Pull", || Action::Pull).with_description("Pull changes"),
        );
        self.register_command(
            Command::new("git.fetch", "Fetch", || Action::Fetch).with_description("Fetch changes"),
        );

        // Plugin switching
        self.register_command(Command::new(
            "plugin.git_status",
            "Switch to Git Status",
            || Action::SwitchPlugin("git_status".to_string()),
        ));
        self.register_command(Command::new(
            "plugin.file_browser",
            "Switch to File Browser",
            || Action::SwitchPlugin("file_browser".to_string()),
        ));
        self.register_command(Command::new(
            "plugin.conversations",
            "Switch to Conversations",
            || Action::SwitchPlugin("conversations".to_string()),
        ));
        self.register_command(Command::new(
            "plugin.workspace",
            "Switch to Workspace",
            || Action::SwitchPlugin("workspace".to_string()),
        ));
    }

    /// Registers all default key bindings.
    fn register_default_bindings(&mut self) {
        // Global bindings
        self.register_binding(Binding::new("q", "app.quit", FocusContext::Global));
        self.register_binding(Binding::new("ctrl+c", "app.quit", FocusContext::Global));
        self.register_binding(Binding::new("r", "app.refresh", FocusContext::Global));
        self.register_binding(Binding::new("ctrl+r", "app.refresh", FocusContext::Global));
        self.register_binding(Binding::new(":", "app.palette", FocusContext::Global));
        self.register_binding(Binding::new("?", "app.help", FocusContext::Global));
        self.register_binding(Binding::new("escape", "app.back", FocusContext::Global));
        self.register_binding(Binding::new("/", "app.search", FocusContext::Global));
        self.register_binding(Binding::new("f", "app.filter", FocusContext::Global));
        self.register_binding(Binding::new("e", "app.context_menu", FocusContext::Global));

        // Navigation bindings (available in most contexts)
        self.register_binding(Binding::new("k", "nav.up", FocusContext::Global));
        self.register_binding(Binding::new("j", "nav.down", FocusContext::Global));
        self.register_binding(Binding::new("h", "nav.left", FocusContext::Global));
        self.register_binding(Binding::new("l", "nav.right", FocusContext::Global));
        self.register_binding(Binding::new("up", "nav.up", FocusContext::Global));
        self.register_binding(Binding::new("down", "nav.down", FocusContext::Global));
        self.register_binding(Binding::new("left", "nav.left", FocusContext::Global));
        self.register_binding(Binding::new("right", "nav.right", FocusContext::Global));
        self.register_binding(Binding::new("enter", "nav.select", FocusContext::Global));
        self.register_binding(Binding::new("tab", "nav.right", FocusContext::Global));
        self.register_binding(Binding::new("shift+tab", "nav.left", FocusContext::Global));

        // File browser specific
        self.register_binding(Binding::new("o", "file.open", FocusContext::FileBrowser));
        self.register_binding(Binding::new("n", "file.create", FocusContext::FileBrowser));
        self.register_binding(Binding::new("d", "file.delete", FocusContext::FileBrowser));
        self.register_binding(Binding::new("y", "file.copy", FocusContext::FileBrowser));
        self.register_binding(Binding::new("p", "file.paste", FocusContext::FileBrowser));

        // Tree operations
        self.register_binding(Binding::new(
            "o",
            "tree.expand",
            FocusContext::FileBrowserTree,
        ));
        self.register_binding(Binding::new(
            "c",
            "tree.collapse",
            FocusContext::FileBrowserTree,
        ));
        self.register_binding(Binding::new(
            "space",
            "tree.toggle",
            FocusContext::FileBrowserTree,
        ));

        // Git status specific
        self.register_binding(Binding::new("s", "git.stage", FocusContext::GitStatus));
        self.register_binding(Binding::new("u", "git.unstage", FocusContext::GitStatus));
        self.register_binding(Binding::new("c", "git.commit", FocusContext::GitStatus));
        self.register_binding(Binding::new("P", "git.push", FocusContext::GitStatus));
        self.register_binding(Binding::new("L", "git.pull", FocusContext::GitStatus));
        self.register_binding(Binding::new("F", "git.fetch", FocusContext::GitStatus));

        // Git diff specific
        self.register_binding(Binding::new("s", "git.stage", FocusContext::GitDiff));
        self.register_binding(Binding::new("u", "git.unstage", FocusContext::GitDiff));

        // Workspace interactive mode
        self.register_binding(Binding::new(
            "ctrl+s",
            "file.edit",
            FocusContext::WorkspaceInteractive,
        ));
        self.register_binding(Binding::new(
            "ctrl+o",
            "file.open",
            FocusContext::WorkspaceInteractive,
        ));

        // Modal specific
        self.register_binding(Binding::new("escape", "app.back", FocusContext::Modal));
        self.register_binding(Binding::new("enter", "nav.select", FocusContext::Modal));

        // Plugin switching (numeric keys)
        self.register_binding(Binding::new("1", "plugin.git_status", FocusContext::Global));
        self.register_binding(Binding::new(
            "2",
            "plugin.file_browser",
            FocusContext::Global,
        ));
        self.register_binding(Binding::new(
            "3",
            "plugin.conversations",
            FocusContext::Global,
        ));
        self.register_binding(Binding::new("4", "plugin.workspace", FocusContext::Global));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_new() {
        let cmd = Command::new("test.cmd", "Test Command", || Action::Refresh);
        assert_eq!(cmd.id, "test.cmd");
        assert_eq!(cmd.name, "Test Command");
        assert!(cmd.description.is_none());
    }

    #[test]
    fn command_with_description() {
        let cmd = Command::new("test.cmd", "Test Command", || Action::Refresh)
            .with_description("A test command");
        assert_eq!(cmd.description, Some("A test command".to_string()));
    }

    #[test]
    fn command_execute() {
        let cmd = Command::new("test.quit", "Quit", || Action::Quit);
        let action = cmd.execute();
        assert!(matches!(action, Action::Quit));
    }

    #[test]
    fn registry_new_is_empty() {
        let registry = Registry::new();
        assert!(registry.commands().is_empty());
        assert!(registry.user_overrides().is_empty());
    }

    #[test]
    fn registry_register_command() {
        let mut registry = Registry::new();
        registry.register_command(Command::new("app.quit", "Quit", || Action::Quit));
        assert_eq!(registry.commands().len(), 1);
        assert!(registry.get_command("app.quit").is_some());
    }

    #[test]
    fn registry_register_binding() {
        let mut registry = Registry::new();
        registry.register_binding(Binding::new("q", "app.quit", FocusContext::Global));
        let bindings = registry.get_bindings(FocusContext::Global);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].key, "q");
    }

    #[test]
    fn registry_handle_global_binding() {
        let mut registry = Registry::new();
        registry.register_command(Command::new("app.quit", "Quit", || Action::Quit));
        registry.register_binding(Binding::new("q", "app.quit", FocusContext::Global));

        let action = registry.handle("q", FocusContext::Global);
        assert!(action.is_some());
        assert!(matches!(action.unwrap(), Action::Quit));
    }

    #[test]
    fn registry_handle_context_specific() {
        let mut registry = Registry::new();
        registry.register_command(Command::new("git.stage", "Stage", || Action::Stage));
        registry.register_binding(Binding::new("s", "git.stage", FocusContext::GitStatus));

        // Should match in GitStatus context
        let action = registry.handle("s", FocusContext::GitStatus);
        assert!(action.is_some());

        // Should not match in other contexts
        let action = registry.handle("s", FocusContext::FileBrowser);
        assert!(action.is_none());
    }

    #[test]
    fn registry_handle_user_override() {
        let mut registry = Registry::new();
        registry.register_command(Command::new("app.quit", "Quit", || Action::Quit));
        registry.register_command(Command::new("app.refresh", "Refresh", || Action::Refresh));
        registry.register_binding(Binding::new("q", "app.quit", FocusContext::Global));
        registry.set_user_override("q".to_string(), "app.refresh".to_string());

        let action = registry.handle("q", FocusContext::Global);
        assert!(action.is_some());
        assert!(matches!(action.unwrap(), Action::Refresh));
    }

    #[test]
    fn registry_handle_unknown_key() {
        let registry = Registry::new();
        let action = registry.handle("unknown_key", FocusContext::Global);
        assert!(action.is_none());
    }

    #[test]
    fn registry_with_defaults_has_commands() {
        let registry = Registry::with_defaults();
        assert!(!registry.commands().is_empty());
        assert!(registry.get_command("app.quit").is_some());
        assert!(registry.get_command("app.refresh").is_some());
    }

    #[test]
    fn registry_with_defaults_has_bindings() {
        let registry = Registry::with_defaults();
        let global_bindings = registry.get_bindings(FocusContext::Global);
        assert!(!global_bindings.is_empty());
    }

    #[test]
    fn registry_remove_user_override() {
        let mut registry = Registry::new();
        registry.set_user_override("q".to_string(), "app.refresh".to_string());
        assert_eq!(registry.user_overrides().len(), 1);

        registry.remove_user_override("q");
        assert!(registry.user_overrides().is_empty());
    }

    #[test]
    fn registry_clear_user_overrides() {
        let mut registry = Registry::new();
        registry.set_user_override("q".to_string(), "app.refresh".to_string());
        registry.set_user_override("r".to_string(), "app.quit".to_string());
        assert_eq!(registry.user_overrides().len(), 2);

        registry.clear_user_overrides();
        assert!(registry.user_overrides().is_empty());
    }

    #[test]
    fn registry_get_command_returns_none_for_unknown() {
        let registry = Registry::new();
        assert!(registry.get_command("unknown").is_none());
    }

    #[test]
    fn registry_get_bindings_empty_for_unknown_context() {
        let registry = Registry::new();
        let bindings = registry.get_bindings(FocusContext::Modal);
        assert!(bindings.is_empty());
    }
}
