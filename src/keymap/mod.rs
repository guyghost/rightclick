//! Keyboard shortcut system for RightClick.
//!
//! This module provides a complete keymap registry for managing keyboard
//! shortcuts across different focus contexts in the application. It supports:
//!
//! - Context-aware key bindings (different shortcuts in different UI areas)
//! - User-customizable key overrides
//! - A comprehensive set of default bindings
//! - Extensible command and action system
//!
//! # Architecture
//!
//! The keymap system consists of:
//!
//! - [`Registry`] - The central registry storing commands and bindings
//! - [`Command`] - Individual commands with metadata and handlers
//! - [`Binding`] - Key-to-command mappings for specific contexts
//! - [`FocusContext`] - Enumeration of UI focus areas
//! - [`Action`] - The outcomes that commands produce
//!
//! # Example Usage
//!
//! ```
//! use rightclick::keymap::{Registry, Command, Binding, Action, FocusContext};
//!
//! // Create a registry with default bindings
//! let mut registry = Registry::with_defaults();
//!
//! // Register a custom command
//! registry.register_command(Command::new(
//!     "custom.action",
//!     "Custom Action",
//!     || Action::Custom(Box::new("data"))
//! ));
//!
//! // Register a custom binding
//! registry.register_binding(Binding::new(
//!     "ctrl+x",
//!     "custom.action",
//!     FocusContext::Global
//! ));
//!
//! // Handle a key press
//! if let Some(action) = registry.handle("q", FocusContext::Global) {
//!     match action {
//!         Action::Quit => println!("Quitting..."),
//!         _ => println!("Other action"),
//!     }
//! }
//! ```

pub mod bindings;
pub mod context;
pub mod registry;
pub mod sequence;

// Re-export commonly used types
pub use bindings::{Action, Binding, KeyBinding, KeyContext, KeyHandler};
pub use context::FocusContext;
pub use registry::{Command, Registry};
pub use sequence::{KeySequenceBuffer, SequenceDefinition, SequenceResult, SequenceState};

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn full_workflow() {
        // Create registry with defaults
        let mut registry = Registry::with_defaults();

        // Verify default commands exist
        assert!(registry.get_command("app.quit").is_some());
        assert!(registry.get_command("app.refresh").is_some());

        // Handle a default binding
        let action = registry.handle("q", FocusContext::Global);
        assert!(action.is_some());
        assert!(matches!(action.unwrap(), Action::Quit));

        // Add a custom command
        registry.register_command(Command::new("my.custom", "Custom", || Action::Refresh));

        // Add a custom binding
        registry.register_binding(Binding::new("ctrl+m", "my.custom", FocusContext::Global));

        // Handle the custom binding
        let action = registry.handle("ctrl+m", FocusContext::Global);
        assert!(action.is_some());
        assert!(matches!(action.unwrap(), Action::Refresh));

        // Set a user override
        registry.set_user_override("q".to_string(), "app.refresh".to_string());

        // Now 'q' should trigger refresh instead of quit
        let action = registry.handle("q", FocusContext::Global);
        assert!(action.is_some());
        assert!(matches!(action.unwrap(), Action::Refresh));

        // Remove the override
        registry.remove_user_override("q");

        // Now 'q' should trigger quit again
        let action = registry.handle("q", FocusContext::Global);
        assert!(action.is_some());
        assert!(matches!(action.unwrap(), Action::Quit));
    }

    #[test]
    fn context_isolation() {
        let mut registry = Registry::new();

        // Register same key for different contexts
        registry.register_command(Command::new("action.a", "Action A", || Action::Open));
        registry.register_command(Command::new("action.b", "Action B", || Action::Create));

        registry.register_binding(Binding::new("x", "action.a", FocusContext::GitStatus));
        registry.register_binding(Binding::new("x", "action.b", FocusContext::FileBrowser));

        // Same key, different contexts should produce different actions
        let action_git = registry.handle("x", FocusContext::GitStatus);
        let action_file = registry.handle("x", FocusContext::FileBrowser);

        assert!(matches!(action_git.unwrap(), Action::Open));
        assert!(matches!(action_file.unwrap(), Action::Create));
    }

    #[test]
    fn global_fallback() {
        let mut registry = Registry::new();

        registry.register_command(Command::new("app.quit", "Quit", || Action::Quit));
        registry.register_binding(Binding::new("q", "app.quit", FocusContext::Global));

        // Global binding should work in any context
        let contexts = [
            FocusContext::GitStatus,
            FocusContext::FileBrowser,
            FocusContext::Conversations,
            FocusContext::Workspace,
        ];

        for context in contexts {
            let action = registry.handle("q", context);
            assert!(
                matches!(action.unwrap(), Action::Quit),
                "Global binding should work in {:?}",
                context
            );
        }
    }

    #[test]
    fn context_override_global() {
        let mut registry = Registry::new();

        // Register global binding
        registry.register_command(Command::new("action.global", "Global", || Action::Open));
        registry.register_binding(Binding::new("x", "action.global", FocusContext::Global));

        // Register context-specific override
        registry.register_command(Command::new("action.local", "Local", || Action::Create));
        registry.register_binding(Binding::new("x", "action.local", FocusContext::GitStatus));

        // Context-specific should take precedence
        let action = registry.handle("x", FocusContext::GitStatus);
        assert!(matches!(action.unwrap(), Action::Create));

        // Other contexts should use global
        let action = registry.handle("x", FocusContext::FileBrowser);
        assert!(matches!(action.unwrap(), Action::Open));
    }

    #[test]
    fn is_root_context_behavior() {
        // In root contexts, 'q' should quit
        let root_contexts = [
            FocusContext::Global,
            FocusContext::GitStatus,
            FocusContext::FileBrowser,
            FocusContext::Conversations,
            FocusContext::Workspace,
        ];

        for context in root_contexts {
            assert!(
                FocusContext::is_root_context(context),
                "{:?} should be a root context",
                context
            );
        }

        // In non-root contexts, 'q' should not quit (may do something else)
        let non_root_contexts = [
            FocusContext::Modal,
            FocusContext::Palette,
            FocusContext::WorkspaceInteractive,
        ];

        for context in non_root_contexts {
            assert!(
                !FocusContext::is_root_context(context),
                "{:?} should not be a root context",
                context
            );
        }
    }

    #[test]
    fn plugin_switching_via_registry() {
        let registry = Registry::with_defaults();

        // Test numeric plugin switching
        let action = registry.handle("1", FocusContext::Global);
        assert!(matches!(action, Some(Action::SwitchPlugin(ref p)) if p == "git_status"));

        let action = registry.handle("2", FocusContext::Global);
        assert!(matches!(action, Some(Action::SwitchPlugin(ref p)) if p == "file_browser"));

        let action = registry.handle("3", FocusContext::Global);
        assert!(matches!(action, Some(Action::SwitchPlugin(ref p)) if p == "conversations"));

        let action = registry.handle("4", FocusContext::Global);
        assert!(matches!(action, Some(Action::SwitchPlugin(ref p)) if p == "workspace"));
    }
}
