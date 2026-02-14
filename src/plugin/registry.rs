//! Plugin registry for managing plugin lifecycle.
//!
//! This module provides the [`Registry`] struct which manages plugin
//! registration, initialization, and shutdown. It maintains a collection
//! of active plugins and tracks any that are unavailable.

use std::collections::HashMap;

use anyhow::Result;
use tracing::{debug, error, info, warn};

use crate::core::models::Config;

use super::{Plugin, PluginContext};

/// A registry for managing plugins.
///
/// The registry maintains a collection of active plugins and provides
/// methods for registration, initialization, and shutdown. It also
/// tracks plugins that are unavailable (e.g., due to missing dependencies).
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use std::sync::Arc;
/// use rightclick::plugin::{Registry, PluginContext};
/// use rightclick::core::models::Config;
/// use rightclick::event::Dispatcher;
///
/// # tokio_test::block_on(async {
/// let ctx = PluginContext::new(
///     PathBuf::from("/work"),
///     PathBuf::from("/project"),
///     PathBuf::from("~/.config/rightclick"),
///     Config::default(),
///     Arc::new(Dispatcher::new()),
///     tracing::info_span!("plugin"),
/// );
///
/// let mut registry = Registry::new(ctx);
/// // Register plugins...
/// # })
/// ```
#[derive(Debug)]
pub struct Registry {
    /// Active plugins managed by this registry
    plugins: Vec<Box<dyn Plugin>>,

    /// Plugins that are unavailable, mapped by ID to reason
    unavailable: HashMap<String, String>,

    /// Shared context for all plugins
    ctx: PluginContext,
}

impl Registry {
    /// Create a new plugin registry.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The plugin context to share with all registered plugins
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use std::sync::Arc;
    /// use rightclick::plugin::{Registry, PluginContext};
    /// use rightclick::core::models::Config;
    /// use rightclick::event::Dispatcher;
    ///
    /// let ctx = PluginContext::new(
    ///     PathBuf::from("/work"),
    ///     PathBuf::from("/project"),
    ///     PathBuf::from("~/.config/rightclick"),
    ///     Config::default(),
    ///     Arc::new(Dispatcher::new()),
    ///     tracing::info_span!("plugin"),
    /// );
    ///
    /// let registry = Registry::new(ctx);
    /// assert!(registry.plugins().is_empty());
    /// ```
    pub fn new(ctx: PluginContext) -> Self {
        Self {
            plugins: Vec::new(),
            unavailable: HashMap::new(),
            ctx,
        }
    }

    /// Register a new plugin with the registry.
    ///
    /// This method adds a plugin to the registry. The plugin is not
    /// initialized until [`Registry::init_all`] is called.
    ///
    /// # Arguments
    ///
    /// * `plugin` - The plugin to register
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the plugin was registered successfully.
    /// Returns an error if a plugin with the same ID is already registered.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::plugin::{Registry, PluginContext, Plugin, PluginCommand, Command};
    /// use rightclick::core::models::Theme;
    /// use rightclick::event::Event;
    /// use rightclick::keymap::FocusContext;
    /// use ratatui::{layout::Rect, buffer::Buffer};
    /// use async_trait::async_trait;
    /// # use std::path::PathBuf;
    /// # use std::sync::Arc;
    /// # use rightclick::core::models::Config;
    /// # use rightclick::event::Dispatcher;
    ///
    /// # struct DummyPlugin;
    /// # #[async_trait]
    /// # impl Plugin for DummyPlugin {
    /// #     fn id(&self) -> &str { "dummy" }
    /// #     fn name(&self) -> &str { "Dummy" }
    /// #     fn icon(&self) -> char { 'D' }
    /// #     async fn init(&mut self, _ctx: &PluginContext) -> anyhow::Result<()> { Ok(()) }
    /// #     fn shutdown(&mut self) -> anyhow::Result<()> { Ok(()) }
    /// #     fn handle_event(&mut self, _event: Event) -> Vec<Command> { vec![] }
    /// #     fn render(&self, _area: Rect, _buf: &mut Buffer, _theme: &Theme) {}
    /// #     fn is_focused(&self) -> bool { false }
    /// #     fn set_focused(&mut self, _focused: bool) {}
    /// #     fn commands(&self) -> Vec<PluginCommand> { vec![] }
    /// #     fn focus_context(&self) -> FocusContext { FocusContext::Global }
    /// # }
    ///
    /// # tokio_test::block_on(async {
    /// # let ctx = PluginContext::new(
    /// #     PathBuf::from("/work"),
    /// #     PathBuf::from("/project"),
    /// #     PathBuf::from("~/.config/rightclick"),
    /// #     Config::default(),
    /// #     Arc::new(Dispatcher::new()),
    /// #     tracing::info_span!("plugin"),
    /// # );
    /// # let mut registry = Registry::new(ctx);
    /// let plugin = DummyPlugin;
    /// registry.register(Box::new(plugin)).unwrap();
    /// # })
    /// ```
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> Result<()> {
        let id = plugin.id().to_string();

        // Check for duplicate IDs
        if self.plugins.iter().any(|p| p.id() == id) {
            return Err(anyhow::anyhow!(
                "Plugin with ID '{}' is already registered",
                id
            ));
        }

        debug!(plugin_id = %id, "Registering plugin");
        self.plugins.push(plugin);
        info!(plugin_id = %id, "Plugin registered successfully");

        Ok(())
    }

    /// Get all registered plugins.
    ///
    /// # Returns
    ///
    /// A slice of all registered plugins.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rightclick::plugin::{Registry, PluginContext};
    /// # use std::path::PathBuf;
    /// # use std::sync::Arc;
    /// # use rightclick::core::models::Config;
    /// # use rightclick::event::Dispatcher;
    /// # let ctx = PluginContext::new(
    /// #     PathBuf::from("/work"),
    /// #     PathBuf::from("/project"),
    /// #     PathBuf::from("~/.config/rightclick"),
    /// #     Config::default(),
    /// #     Arc::new(Dispatcher::new()),
    /// #     tracing::info_span!("plugin"),
    /// # );
    /// # let registry = Registry::new(ctx);
    /// let plugins = registry.plugins();
    /// ```
    pub fn plugins(&self) -> &[Box<dyn Plugin>] {
        &self.plugins
    }

    /// Get all registered plugins mutably.
    pub fn plugins_mut(&mut self) -> &mut [Box<dyn Plugin>] {
        &mut self.plugins
    }

    /// Get a plugin by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The plugin identifier
    ///
    /// # Returns
    ///
    /// Returns `Some(&dyn Plugin)` if found, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rightclick::plugin::{Registry, PluginContext};
    /// # use std::path::PathBuf;
    /// # use std::sync::Arc;
    /// # use rightclick::core::models::Config;
    /// # use rightclick::event::Dispatcher;
    /// # let ctx = PluginContext::new(
    /// #     PathBuf::from("/work"),
    /// #     PathBuf::from("/project"),
    /// #     PathBuf::from("~/.config/rightclick"),
    /// #     Config::default(),
    /// #     Arc::new(Dispatcher::new()),
    /// #     tracing::info_span!("plugin"),
    /// # );
    /// # let registry = Registry::new(ctx);
    /// if let Some(plugin) = registry.get("workspace") {
    ///     println!("Found plugin: {}", plugin.name());
    /// }
    /// ```
    pub fn get(&self, id: &str) -> Option<&dyn Plugin> {
        self.plugins
            .iter()
            .find(|p| p.id() == id)
            .map(|p| p.as_ref())
    }

    /// Get a mutable reference to a plugin by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The plugin identifier
    ///
    /// # Returns
    ///
    /// Returns `Some(&mut dyn Plugin)` if found, `None` otherwise.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut (dyn Plugin + '_)> {
        self.plugins
            .iter_mut()
            .find(|p| p.id() == id)
            .map(|p| p.as_mut() as &mut (dyn Plugin + '_))
    }

    /// Get all unavailable plugins.
    ///
    /// # Returns
    ///
    /// A map of plugin IDs to the reason they are unavailable.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rightclick::plugin::{Registry, PluginContext};
    /// # use std::path::PathBuf;
    /// # use std::sync::Arc;
    /// # use rightclick::core::models::Config;
    /// # use rightclick::event::Dispatcher;
    /// # let ctx = PluginContext::new(
    /// #     PathBuf::from("/work"),
    /// #     PathBuf::from("/project"),
    /// #     PathBuf::from("~/.config/rightclick"),
    /// #     Config::default(),
    /// #     Arc::new(Dispatcher::new()),
    /// #     tracing::info_span!("plugin"),
    /// # );
    /// # let registry = Registry::new(ctx);
    /// for (id, reason) in registry.unavailable() {
    ///     eprintln!("Plugin {} unavailable: {}", id, reason);
    /// }
    /// ```
    pub fn unavailable(&self) -> &HashMap<String, String> {
        &self.unavailable
    }

    /// Mark a plugin as unavailable.
    ///
    /// This is used when a plugin cannot be loaded due to missing
    /// dependencies or other issues.
    ///
    /// # Arguments
    ///
    /// * `id` - The plugin identifier
    /// * `reason` - The reason the plugin is unavailable
    pub fn mark_unavailable(&mut self, id: impl Into<String>, reason: impl Into<String>) {
        let id = id.into();
        let reason = reason.into();
        warn!(plugin_id = %id, reason = %reason, "Plugin marked as unavailable");
        self.unavailable.insert(id, reason);
    }

    /// Initialize all registered plugins.
    ///
    /// This method calls [`Plugin::init`] on each registered plugin.
    /// If a plugin fails to initialize, it is removed from the registry
    /// and added to the unavailable list.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if all plugins were initialized successfully.
    /// Returns an error if any plugin failed to initialize.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rightclick::plugin::{Registry, PluginContext};
    /// # use std::path::PathBuf;
    /// # use std::sync::Arc;
    /// # use rightclick::core::models::Config;
    /// # use rightclick::event::Dispatcher;
    /// # tokio_test::block_on(async {
    /// # let ctx = PluginContext::new(
    /// #     PathBuf::from("/work"),
    /// #     PathBuf::from("/project"),
    /// #     PathBuf::from("~/.config/rightclick"),
    /// #     Config::default(),
    /// #     Arc::new(Dispatcher::new()),
    /// #     tracing::info_span!("plugin"),
    /// # );
    /// # let mut registry = Registry::new(ctx);
    /// registry.init_all().await.unwrap();
    /// # })
    /// ```
    pub async fn init_all(&mut self) -> Result<()> {
        info!(count = self.plugins.len(), "Initializing all plugins");

        let mut failed = Vec::new();

        for (index, plugin) in self.plugins.iter_mut().enumerate() {
            let id = plugin.id().to_string();
            debug!(plugin_id = %id, "Initializing plugin");

            let _span = tracing::span!(parent: &self.ctx.logger, tracing::Level::INFO, "plugin_init", plugin_id = %id);
            let _enter = _span.enter();

            match plugin.init(&self.ctx).await {
                Ok(()) => {
                    info!(plugin_id = %id, "Plugin initialized successfully");
                }
                Err(e) => {
                    error!(plugin_id = %id, error = %e, "Plugin initialization failed");
                    failed.push((index, id, e.to_string()));
                }
            }
        }

        // Remove failed plugins in reverse order to maintain index validity
        for (index, id, reason) in failed.into_iter().rev() {
            self.plugins.remove(index);
            self.unavailable.insert(id, reason);
        }

        info!(
            active = self.plugins.len(),
            unavailable = self.unavailable.len(),
            "Plugin initialization complete"
        );

        Ok(())
    }

    /// Shutdown all registered plugins.
    ///
    /// This method calls [`Plugin::shutdown`] on each registered plugin.
    /// Errors during shutdown are logged but do not prevent other plugins
    /// from shutting down.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rightclick::plugin::{Registry, PluginContext};
    /// # use std::path::PathBuf;
    /// # use std::sync::Arc;
    /// # use rightclick::core::models::Config;
    /// # use rightclick::event::Dispatcher;
    /// # tokio_test::block_on(async {
    /// # let ctx = PluginContext::new(
    /// #     PathBuf::from("/work"),
    /// #     PathBuf::from("/project"),
    /// #     PathBuf::from("~/.config/rightclick"),
    /// #     Config::default(),
    /// #     Arc::new(Dispatcher::new()),
    /// #     tracing::info_span!("plugin"),
    /// # );
    /// # let mut registry = Registry::new(ctx);
    /// # registry.init_all().await.unwrap();
    /// registry.shutdown_all();
    /// # })
    /// ```
    pub fn shutdown_all(&mut self) {
        info!(count = self.plugins.len(), "Shutting down all plugins");

        for plugin in &mut self.plugins {
            let id = plugin.id().to_string();
            debug!(plugin_id = %id, "Shutting down plugin");

            if let Err(e) = plugin.shutdown() {
                error!(plugin_id = %id, error = %e, "Plugin shutdown failed");
            } else {
                info!(plugin_id = %id, "Plugin shut down successfully");
            }
        }

        // Clear the plugins list
        self.plugins.clear();

        info!("All plugins shut down");
    }

    /// Update the configuration for all plugins.
    ///
    /// This method updates the shared context configuration. Plugins
    /// can access the updated configuration on the next operation.
    ///
    /// # Arguments
    ///
    /// * `config` - The new configuration
    ///
    /// # Examples
    ///
    /// ```
    /// # use rightclick::plugin::{Registry, PluginContext};
    /// # use rightclick::core::models::Config;
    /// # use std::path::PathBuf;
    /// # use std::sync::Arc;
    /// # use rightclick::event::Dispatcher;
    /// # let ctx = PluginContext::new(
    /// #     PathBuf::from("/work"),
    /// #     PathBuf::from("/project"),
    /// #     PathBuf::from("~/.config/rightclick"),
    /// #     Config::default(),
    /// #     Arc::new(Dispatcher::new()),
    /// #     tracing::info_span!("plugin"),
    /// # );
    /// # let mut registry = Registry::new(ctx);
    /// let new_config = Config::default();
    /// registry.update_config(new_config);
    /// ```
    pub fn update_config(&mut self, config: Config) {
        debug!("Updating plugin registry configuration");
        self.ctx.config = config;
    }

    /// Get the number of registered plugins.
    ///
    /// # Returns
    ///
    /// The number of active plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Check if the registry is empty.
    ///
    /// # Returns
    ///
    /// `true` if no plugins are registered, `false` otherwise.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Get the shared plugin context.
    ///
    /// # Returns
    ///
    /// A reference to the shared plugin context.
    pub fn context(&self) -> &PluginContext {
        &self.ctx
    }

    /// Get a mutable reference to the shared plugin context.
    ///
    /// # Returns
    ///
    /// A mutable reference to the shared plugin context.
    pub fn context_mut(&mut self) -> &mut PluginContext {
        &mut self.ctx
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        if !self.plugins.is_empty() {
            self.shutdown_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::Theme;
    use crate::event::Event;
    use crate::keymap::FocusContext;
    use crate::plugin::{Command, Plugin, PluginCommand, PluginContext};
    use async_trait::async_trait;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[derive(Debug)]
    struct TestPlugin {
        id: String,
        name: String,
        focused: bool,
        init_called: bool,
        shutdown_called: bool,
        should_fail_init: bool,
    }

    impl TestPlugin {
        fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                name: name.into(),
                focused: false,
                init_called: false,
                shutdown_called: false,
                should_fail_init: false,
            }
        }

        fn with_init_failure(mut self) -> Self {
            self.should_fail_init = true;
            self
        }
    }

    #[async_trait]
    impl Plugin for TestPlugin {
        fn id(&self) -> &str {
            &self.id
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn icon(&self) -> char {
            'T'
        }

        async fn init(&mut self, _ctx: &PluginContext) -> Result<()> {
            self.init_called = true;
            if self.should_fail_init {
                return Err(anyhow::anyhow!("Init failed"));
            }
            Ok(())
        }

        fn shutdown(&mut self) -> Result<()> {
            self.shutdown_called = true;
            Ok(())
        }

        fn handle_event(&mut self, _event: Event) -> Vec<Command> {
            vec![]
        }

        fn render(&self, _area: Rect, _buf: &mut Buffer, _theme: &Theme) {}

        fn is_focused(&self) -> bool {
            self.focused
        }

        fn set_focused(&mut self, focused: bool) {
            self.focused = focused;
        }

        fn commands(&self) -> Vec<PluginCommand> {
            vec![]
        }

        fn focus_context(&self) -> FocusContext {
            FocusContext::Global
        }
    }

    fn create_test_context() -> PluginContext {
        PluginContext::new(
            PathBuf::from("/work"),
            PathBuf::from("/project"),
            PathBuf::from("~/.config/rightclick"),
            Config::default(),
            Arc::new(crate::event::Dispatcher::new()),
            tracing::info_span!("test"),
        )
    }

    #[test]
    fn test_new_registry() {
        let ctx = create_test_context();
        let registry = Registry::new(ctx);
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_register_plugin() {
        let ctx = create_test_context();
        let mut registry = Registry::new(ctx);
        let plugin = TestPlugin::new("test", "Test Plugin");

        registry.register(Box::new(plugin)).unwrap();
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_register_duplicate_id() {
        let ctx = create_test_context();
        let mut registry = Registry::new(ctx);

        let plugin1 = TestPlugin::new("test", "Test Plugin 1");
        let plugin2 = TestPlugin::new("test", "Test Plugin 2");

        registry.register(Box::new(plugin1)).unwrap();
        let result = registry.register(Box::new(plugin2));

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("already registered")
        );
    }

    #[test]
    fn test_get_plugin() {
        let ctx = create_test_context();
        let mut registry = Registry::new(ctx);
        let plugin = TestPlugin::new("test", "Test Plugin");

        registry.register(Box::new(plugin)).unwrap();

        let found = registry.get("test");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name(), "Test Plugin");

        let not_found = registry.get("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_get_mut_plugin() {
        let ctx = create_test_context();
        let mut registry = Registry::new(ctx);
        let plugin = TestPlugin::new("test", "Test Plugin");

        registry.register(Box::new(plugin)).unwrap();

        let found = registry.get_mut("test");
        assert!(found.is_some());
        found.unwrap().set_focused(true);

        let found = registry.get("test");
        assert!(found.unwrap().is_focused());
    }

    #[test]
    fn test_mark_unavailable() {
        let ctx = create_test_context();
        let mut registry = Registry::new(ctx);

        registry.mark_unavailable("missing", "Not installed");
        assert_eq!(registry.unavailable().len(), 1);
        assert_eq!(
            registry.unavailable().get("missing").unwrap(),
            "Not installed"
        );
    }

    #[tokio::test]
    async fn test_init_all() {
        let ctx = create_test_context();
        let mut registry = Registry::new(ctx);
        let plugin = TestPlugin::new("test", "Test Plugin");

        registry.register(Box::new(plugin)).unwrap();
        registry.init_all().await.unwrap();

        assert_eq!(registry.len(), 1);
        assert!(registry.unavailable().is_empty());
    }

    #[tokio::test]
    async fn test_init_all_with_failure() {
        let ctx = create_test_context();
        let mut registry = Registry::new(ctx);

        let plugin1 = TestPlugin::new("success", "Success Plugin");
        let plugin2 = TestPlugin::new("failure", "Failure Plugin").with_init_failure();

        registry.register(Box::new(plugin1)).unwrap();
        registry.register(Box::new(plugin2)).unwrap();

        registry.init_all().await.unwrap();

        assert_eq!(registry.len(), 1);
        assert_eq!(registry.unavailable().len(), 1);
        assert!(registry.unavailable().contains_key("failure"));
    }

    #[tokio::test]
    async fn test_shutdown_all() {
        let ctx = create_test_context();
        let mut registry = Registry::new(ctx);
        let plugin = TestPlugin::new("test", "Test Plugin");

        registry.register(Box::new(plugin)).unwrap();
        registry.init_all().await.unwrap();

        registry.shutdown_all();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_update_config() {
        let ctx = create_test_context();
        let mut registry = Registry::new(ctx);

        let new_config = Config::default();
        registry.update_config(new_config);

        // Just verify it doesn't panic
    }

    #[test]
    fn test_plugins_slice() {
        let ctx = create_test_context();
        let mut registry = Registry::new(ctx);
        let plugin = TestPlugin::new("test", "Test Plugin");

        registry.register(Box::new(plugin)).unwrap();

        let plugins = registry.plugins();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].id(), "test");
    }

    #[test]
    fn test_context_access() {
        let ctx = create_test_context();
        let registry = Registry::new(ctx);

        assert_eq!(registry.context().work_dir, PathBuf::from("/work"));
    }
}
