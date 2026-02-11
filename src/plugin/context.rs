//! Plugin context for initialization and runtime access.
//!
//! This module defines the context passed to plugins during initialization,
//! providing access to configuration, adapters, and the event bus.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::adapters::Adapter;
use crate::core::models::Config;
use crate::event::Dispatcher;

/// Context passed to plugins during initialization and runtime.
///
/// This struct provides plugins with access to shared resources and
/// configuration needed for their operation.
#[derive(Debug)]
pub struct PluginContext {
    /// Working directory where the application was started
    pub work_dir: PathBuf,

    /// Project root directory (detected from git or config)
    pub project_root: PathBuf,

    /// Configuration directory (e.g., ~/.config/rightclick)
    pub config_dir: PathBuf,

    /// Current application configuration
    pub config: Config,

    /// Registered AI adapters for accessing session data
    pub adapters: Arc<HashMap<String, Arc<dyn Adapter>>>,

    /// Event bus dispatcher for pub/sub communication
    pub event_bus: Arc<Dispatcher>,

    /// Tracing span for structured logging
    pub logger: tracing::Span,
}

impl PluginContext {
    /// Create a new plugin context with the required fields.
    ///
    /// # Arguments
    ///
    /// * `work_dir` - The working directory
    /// * `project_root` - The project root directory
    /// * `config_dir` - The configuration directory
    /// * `config` - The application configuration
    /// * `event_bus` - The event dispatcher
    /// * `logger` - The tracing span for logging
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use std::sync::Arc;
    /// use rightclick::plugin::PluginContext;
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
    /// ```
    pub fn new(
        work_dir: PathBuf,
        project_root: PathBuf,
        config_dir: PathBuf,
        config: Config,
        event_bus: Arc<Dispatcher>,
        logger: tracing::Span,
    ) -> Self {
        Self {
            work_dir,
            project_root,
            config_dir,
            config,
            adapters: Arc::<HashMap<String, Arc<dyn Adapter>>>::new(HashMap::new()),
            event_bus,
            logger,
        }
    }

    /// Create a new plugin context with adapters.
    ///
    /// # Arguments
    ///
    /// * `work_dir` - The working directory
    /// * `project_root` - The project root directory
    /// * `config_dir` - The configuration directory
    /// * `config` - The application configuration
    /// * `adapters` - Map of adapter ID to adapter instances
    /// * `event_bus` - The event dispatcher
    /// * `logger` - The tracing span for logging
    pub fn with_adapters(
        work_dir: PathBuf,
        project_root: PathBuf,
        config_dir: PathBuf,
        config: Config,
        adapters: Arc<HashMap<String, Arc<dyn Adapter>>>,
        event_bus: Arc<Dispatcher>,
        logger: tracing::Span,
    ) -> Self {
        Self {
            work_dir,
            project_root,
            config_dir,
            config,
            adapters,
            event_bus,
            logger,
        }
    }

    /// Get an adapter by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The adapter identifier
    ///
    /// # Returns
    ///
    /// Returns `Some(&dyn Adapter)` if found, `None` otherwise.
    pub fn get_adapter(&self, id: &str) -> Option<&dyn Adapter> {
        self.adapters.get(id).map(|a| a.as_ref())
    }
    
    /// Set adapters for this context.
    ///
    /// # Arguments
    ///
    /// * `adapters` - Map of adapter ID to adapter instances wrapped in Arc
    pub fn set_adapters(&mut self, adapters: Arc<HashMap<String, Arc<dyn Adapter>>>) {
        self.adapters = adapters;
    }

    /// Check if an adapter is available.
    ///
    /// # Arguments
    ///
    /// * `id` - The adapter identifier
    ///
    /// # Returns
    ///
    /// Returns `true` if the adapter is registered.
    pub fn has_adapter(&self, id: &str) -> bool {
        self.adapters.contains_key(id)
    }

    /// Publish an event to the event bus.
    ///
    /// This is a convenience method that publishes to `Topic::All`.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to publish
    pub fn emit(&self, event: crate::event::Event) {
        self.event_bus.publish_all(event);
    }

    /// Get the plugin-specific configuration from the global config.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The configuration type to extract
    ///
    /// # Returns
    ///
    /// Returns the plugin configuration if available.
    pub fn plugin_config<T: Clone + 'static>(&self, extractor: impl FnOnce(&Config) -> &T) -> T {
        extractor(&self.config).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_context() -> PluginContext {
        PluginContext::new(
            PathBuf::from("/work"),
            PathBuf::from("/project"),
            PathBuf::from("~/.config/rightclick"),
            Config::default(),
            Arc::new(Dispatcher::new()),
            tracing::info_span!("test"),
        )
    }

    #[test]
    fn test_context_creation() {
        let ctx = create_test_context();
        assert_eq!(ctx.work_dir, PathBuf::from("/work"));
        assert_eq!(ctx.project_root, PathBuf::from("/project"));
        assert_eq!(ctx.config_dir, PathBuf::from("~/.config/rightclick"));
    }

    #[test]
    fn test_context_with_adapters() {
        let adapters: HashMap<String, Box<dyn Adapter>> = HashMap::new();
        let ctx = PluginContext::with_adapters(
            PathBuf::from("/work"),
            PathBuf::from("/project"),
            PathBuf::from("~/.config/rightclick"),
            Config::default(),
            adapters,
            Arc::new(Dispatcher::new()),
            tracing::info_span!("test"),
        );
        assert!(ctx.adapters.is_empty());
    }

    #[test]
    fn test_get_adapter_none() {
        let ctx = create_test_context();
        assert!(ctx.get_adapter("nonexistent").is_none());
    }

    #[test]
    fn test_has_adapter() {
        let ctx = create_test_context();
        assert!(!ctx.has_adapter("nonexistent"));
    }

    #[test]
    fn test_plugin_config_extraction() {
        let ctx = create_test_context();
        let ui_config = ctx.plugin_config(|c| &c.ui);
        assert!(ui_config.show_clock);
    }
}
