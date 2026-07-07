//! RightClick - A TUI dashboard for AI coding agents

use std::io;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};
use tracing::{info, warn};

use rightclick::palette::fuzzy::fuzzy_match_simple;
use rightclick::{
    adapters::create_default_registry,
    config,
    core::models::{Config, Theme},
    event::{Dispatcher, Event, Topic},
    plugin::{
        Plugin, PluginCommandError, PluginCommandExecution, PluginContext, PluginSearchEntry,
    },
    plugins::{conversations, filebrowser, gitstatus, workers, workspace},
    search::{
        SearchOverlayAction, SearchOverlayState, SearchScope, render_search_overlay, search_files,
    },
    settings::SettingsModal,
    state,
    theme::{self, resolve_theme},
    ui::{Footer, Header, NotificationManager, compact_global_search_hint},
};

#[derive(Debug)]
enum SearchCommandError {
    InvalidRoute(String),
    PluginUnavailable(String),
    PluginCommand(PluginCommandError),
}

impl SearchCommandError {
    fn notification_message(&self) -> String {
        match self {
            Self::InvalidRoute(route) => format!("Invalid command route: {}", route),
            Self::PluginUnavailable(plugin_id) => {
                format!("Command plugin not loaded: {}", plugin_id)
            }
            Self::PluginCommand(error) => error.to_string(),
        }
    }
}

/// Command-line arguments
#[derive(Parser, Debug)]
#[command(name = "rightclick")]
#[command(about = "A TUI dashboard for AI coding agents")]
#[command(version = version())]
struct Args {
    /// Project root directory
    #[arg(short, long, default_value = ".")]
    project: PathBuf,

    /// Path to config file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

/// Get version string
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Setup logging
fn setup_logging(debug: bool) -> Result<()> {
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

    let filter = if debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("warn")
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(io::stderr))
        .init();

    Ok(())
}

/// Main entry point
#[tokio::main]
async fn main() -> Result<()> {
    // Parse arguments
    let args = Args::parse();

    // Setup logging
    setup_logging(args.debug)?;

    info!("RightClick v{} starting up", version());

    // Initialize state system
    state::init()?;

    // Load configuration
    let config = if let Some(config_path) = args.config {
        config::load_from(&config_path)?
    } else {
        config::load().unwrap_or_default()
    };

    // Resolve and apply theme
    let resolved = resolve_theme(&config, None);
    let theme = resolved.theme.clone();
    theme::apply_theme(&theme);

    // Resolve project paths
    let work_dir = std::env::current_dir()?.join(&args.project);
    let work_dir = work_dir.canonicalize().unwrap_or(work_dir);
    let project_root = work_dir.clone();

    info!("Work directory: {}", work_dir.display());

    // Detect adapters for conversations plugin
    let adapter_registry = create_default_registry()?;
    let adapters = adapter_registry
        .detect_all(&project_root)
        .await
        .unwrap_or_default();
    info!("Detected {} adapters", adapters.len());

    // Shared event bus for both plugin context and the App shell.
    let event_bus = std::sync::Arc::new(rightclick::event::Dispatcher::new());

    // Create plugin context
    let plugin_ctx = PluginContext {
        work_dir: work_dir.clone(),
        project_root: project_root.clone(),
        config_dir: config::config_dir().unwrap_or_else(|_| PathBuf::from("~/.config/rightclick")),
        config: config.clone(),
        adapters: adapters.into(),
        event_bus: event_bus.clone(),
        logger: tracing::info_span!("plugin"),
    };

    // Create and initialize plugins
    let mut plugins: Vec<Box<dyn Plugin>> = Vec::new();

    // Git Status plugin
    if config.plugins.git_status.enabled {
        info!("Loading git-status plugin");
        let mut plugin = gitstatus::GitStatusPlugin::new();
        if let Err(e) = plugin.init(&plugin_ctx).await {
            warn!("Failed to init gitstatus plugin: {}", e);
        } else {
            plugins.push(Box::new(plugin));
        }
    }

    // File Browser plugin
    if config.plugins.file_browser.enabled {
        info!("Loading file-browser plugin");
        let mut plugin = filebrowser::FileBrowserPlugin::new(work_dir.clone());
        if let Err(e) = plugin.init(&plugin_ctx).await {
            warn!("Failed to init filebrowser plugin: {}", e);
        } else {
            plugins.push(Box::new(plugin));
        }
    }

    // Conversations plugin
    if config.plugins.conversations.enabled {
        info!("Loading conversations plugin");
        let adapter_registry = std::sync::Arc::new(parking_lot::RwLock::new(adapter_registry));
        let mut plugin = conversations::ConversationsPlugin::new(adapter_registry);
        if let Err(e) = plugin.init(&plugin_ctx).await {
            warn!("Failed to init conversations plugin: {}", e);
        } else {
            plugins.push(Box::new(plugin));
        }
    }

    // Workspaces plugin
    if config.plugins.workspace.enabled {
        info!("Loading workspaces plugin");
        let mut plugin = workspace::WorkspacePlugin::new();
        if let Err(e) = plugin.init(&plugin_ctx).await {
            warn!("Failed to init workspace plugin: {}", e);
        } else {
            plugins.push(Box::new(plugin));
        }
    }

    // Workers plugin
    if config.plugins.workers.enabled {
        info!("Loading workers plugin");
        let mut plugin = workers::WorkersPlugin::new();
        if let Err(e) = plugin.init(&plugin_ctx).await {
            warn!("Failed to init workers plugin: {}", e);
        } else {
            plugins.push(Box::new(plugin));
        }
    }

    // If no plugins loaded, add defaults
    if plugins.is_empty() {
        info!("Loading default plugins");
        let mut plugin = gitstatus::GitStatusPlugin::new();
        let _ = plugin.init(&plugin_ctx).await;
        plugins.push(Box::new(plugin));

        let mut plugin = filebrowser::FileBrowserPlugin::new(work_dir.clone());
        let _ = plugin.init(&plugin_ctx).await;
        plugins.push(Box::new(plugin));

        let mut plugin = workspace::WorkspacePlugin::new();
        let _ = plugin.init(&plugin_ctx).await;
        plugins.push(Box::new(plugin));

        let mut plugin = workers::WorkersPlugin::new();
        let _ = plugin.init(&plugin_ctx).await;
        plugins.push(Box::new(plugin));
    }

    info!("Loaded {} plugins", plugins.len());

    // Set first plugin as focused
    if !plugins.is_empty() {
        plugins[0].set_focused(true);
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new(plugins, theme, work_dir.clone(), config, event_bus);

    // Run main loop
    let result = run_app(&mut terminal, &mut app).await;

    // Shutdown plugins
    for plugin in app.plugins.iter_mut() {
        let _ = plugin.shutdown();
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

/// Application state
struct App {
    plugins: Vec<Box<dyn Plugin>>,
    theme: Theme,
    active_plugin: usize,
    should_quit: bool,
    show_help: bool,
    notifications: NotificationManager,
    search: SearchOverlayState,
    work_dir: PathBuf,
    /// Authoritative application config. Edited via the settings modal and
    /// persisted back to `config.json` on save.
    config: Config,
    /// Shared event bus. Used to broadcast `Event::ConfigChanged` after a
    /// settings save so plugins can live-reload.
    event_bus: std::sync::Arc<Dispatcher>,
    /// Whether the settings modal is currently open.
    settings_open: bool,
    /// The settings modal state, present when open.
    settings: Option<SettingsModal>,
}

impl App {
    fn new(
        plugins: Vec<Box<dyn Plugin>>,
        theme: Theme,
        work_dir: PathBuf,
        config: Config,
        event_bus: std::sync::Arc<Dispatcher>,
    ) -> Self {
        Self {
            plugins,
            theme,
            active_plugin: 0,
            should_quit: false,
            show_help: false,
            notifications: NotificationManager::new(),
            search: SearchOverlayState::new(),
            work_dir,
            config,
            event_bus,
            settings_open: false,
            settings: None,
        }
    }

    /// Toggle the settings modal open/closed.
    fn toggle_settings(&mut self) {
        if self.settings_open {
            self.close_settings();
        } else {
            self.settings = Some(SettingsModal::from_config(&self.config));
            self.settings_open = true;
        }
    }

    /// Close the settings modal, discarding any unapplied edits.
    fn close_settings(&mut self) {
        self.settings = None;
        self.settings_open = false;
    }

    /// Persist the edited settings: build the new config from the modal,
    /// save it to disk, apply it to live plugins, and broadcast the change.
    fn save_settings(&mut self) {
        let modal = match self.settings.take() {
            Some(modal) => modal,
            None => {
                self.settings_open = false;
                return;
            }
        };
        let new_config = modal.into_config(&self.config);
        // Persist to config.json.
        if let Err(e) = config::save(&new_config) {
            warn!("Failed to save config: {}", e);
            self.notifications
                .error(format!("Failed to save settings: {e}"));
            self.settings_open = false;
            return;
        }
        // Apply to live plugins.
        self.config = new_config.clone();
        for plugin in &mut self.plugins {
            plugin.apply_config(&new_config);
        }
        // Broadcast so any other listeners (and future event-driven plugins)
        // can react.
        self.event_bus
            .publish(Topic::ConfigChange, Event::ConfigChanged);
        self.settings_open = false;
        self.notifications.success("Settings saved");
    }

    async fn handle_event(&mut self, event: crossterm::event::Event) -> Result<()> {
        use crossterm::event::{Event as CEvent, KeyCode, KeyEventKind};

        match event {
            CEvent::Key(key) if key.kind == KeyEventKind::Press => {
                // Ctrl+C always quits regardless of any mode
                if is_ctrl_c_quit_key(&key) {
                    self.should_quit = true;
                    return Ok(());
                }
                if is_ctrl_r_refresh_key(&key) {
                    if let Some(plugin) = self.plugins.get_mut(self.active_plugin) {
                        plugin.handle_event(rightclick::event::Event::RefreshNeeded);
                    }
                    return Ok(());
                }

                // Ctrl+, toggles the settings modal.
                if is_ctrl_comma_key(&key) {
                    self.toggle_settings();
                    return Ok(());
                }

                // While the settings modal is open it captures all input.
                if self.settings_open {
                    if let Some(modal) = self.settings.as_mut() {
                        match modal.handle_key(key) {
                            rightclick::settings::SettingsAction::Save => {
                                self.save_settings();
                            }
                            rightclick::settings::SettingsAction::Cancel => {
                                self.close_settings();
                            }
                            rightclick::settings::SettingsAction::Handled
                            | rightclick::settings::SettingsAction::Ignored => {}
                        }
                    }
                    return Ok(());
                }

                // If search overlay is visible, route all input to it
                if self.search.visible {
                    let key_code = key_code_to_string(key.code);
                    let ctrl = key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL);
                    let action = self.search.handle_key(&key_code, ctrl);
                    match action {
                        SearchOverlayAction::QueryChanged | SearchOverlayAction::ScopeChanged => {
                            self.run_search().await;
                        }
                        SearchOverlayAction::ResultSelected => {
                            self.activate_selected_search_result();
                            // Close overlay on result selection
                            self.search.close();
                        }
                        _ => {}
                    }
                    return Ok(());
                }

                let active_plugin_id = self
                    .plugins
                    .get(self.active_plugin)
                    .map(|p| p.id())
                    .unwrap_or("");
                let active_uses_tab_for_panes = plugin_uses_tab_for_panes(active_plugin_id);

                // Check if the active plugin is consuming text input (e.g., modal with text field)
                let consumes_text = self
                    .plugins
                    .get(self.active_plugin)
                    .map(|p| p.consumes_text_input())
                    .unwrap_or(false);

                // When a plugin is consuming text input, skip global shortcuts
                // that would conflict with typing (q, digits, etc.)
                if !consumes_text {
                    // Global keys first
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                            self.should_quit = true;
                            return Ok(());
                        }
                        KeyCode::Char('/') => {
                            self.search.open();
                            return Ok(());
                        }
                        KeyCode::Char(':') => {
                            self.search.open_with_scope(SearchScope::Commands);
                            return Ok(());
                        }
                        KeyCode::Char('?') => {
                            self.show_help = !self.show_help;
                            return Ok(());
                        }
                        KeyCode::Char(c) => {
                            // Always use digits 1-9 for global plugin navigation
                            if let Some(idx) = plugin_shortcut_index(c) {
                                if idx < self.plugins.len() {
                                    self.switch_plugin(idx);
                                    return Ok(());
                                }
                            }
                        }
                        KeyCode::Tab
                            if !active_uses_tab_for_panes
                                || key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            self.next_plugin();
                            return Ok(());
                        }
                        KeyCode::BackTab
                            if !active_uses_tab_for_panes
                                || key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            self.prev_plugin();
                            return Ok(());
                        }
                        _ => {}
                    }
                }

                // Send key event to active plugin
                if let Some(plugin) = self.plugins.get_mut(self.active_plugin) {
                    let key_code = match key.code {
                        KeyCode::Char(c) => c.to_string(),
                        KeyCode::Up => "Up".to_string(),
                        KeyCode::Down => "Down".to_string(),
                        KeyCode::Left => "Left".to_string(),
                        KeyCode::Right => "Right".to_string(),
                        KeyCode::Enter => "Enter".to_string(),
                        KeyCode::Esc => "Esc".to_string(),
                        KeyCode::Backspace => "Backspace".to_string(),
                        KeyCode::Tab => "Tab".to_string(),
                        KeyCode::BackTab => "BackTab".to_string(),
                        KeyCode::Home => "Home".to_string(),
                        KeyCode::End => "End".to_string(),
                        KeyCode::PageUp => "PageUp".to_string(),
                        KeyCode::PageDown => "PageDown".to_string(),
                        KeyCode::Delete => "Delete".to_string(),
                        KeyCode::Insert => "Insert".to_string(),
                        KeyCode::F(n) => format!("F{}", n),
                        _ => key.code.to_string(),
                    };
                    let modifiers = rightclick::event::KeyModifiers {
                        ctrl: key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL),
                        alt: key.modifiers.contains(crossterm::event::KeyModifiers::ALT),
                        shift: key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::SHIFT),
                    };
                    let event = rightclick::event::Event::Key {
                        code: key_code,
                        modifiers,
                    };
                    let _commands = plugin.handle_event(event);
                }
            }
            CEvent::Resize(_, _) => {
                // Terminal resized, will redraw automatically
            }
            _ => {}
        }

        Ok(())
    }

    fn switch_plugin(&mut self, idx: usize) {
        if idx >= self.plugins.len() || idx == self.active_plugin {
            return;
        }

        // Unfocus current
        if let Some(plugin) = self.plugins.get_mut(self.active_plugin) {
            plugin.set_focused(false);
        }

        // Focus new
        self.active_plugin = idx;
        if let Some(plugin) = self.plugins.get_mut(self.active_plugin) {
            plugin.set_focused(true);
        }
    }

    fn next_plugin(&mut self) {
        if self.plugins.len() <= 1 {
            return;
        }
        let next = (self.active_plugin + 1) % self.plugins.len();
        self.switch_plugin(next);
    }

    fn prev_plugin(&mut self) {
        if self.plugins.len() <= 1 {
            return;
        }
        let prev = if self.active_plugin == 0 {
            self.plugins.len() - 1
        } else {
            self.active_plugin - 1
        };
        self.switch_plugin(prev);
    }

    /// Run search based on current query and scope
    async fn run_search(&mut self) {
        let query = self.search.query().to_string();
        if query.is_empty() {
            self.search.set_results(Vec::new());
            return;
        }

        let scope = self.search.scope;
        let mut all_results = Vec::new();

        // File content search (async ripgrep subprocess)
        if matches!(scope, SearchScope::All | SearchScope::Files) {
            let file_results = search_files(&query, &self.work_dir, 30).await;
            all_results.extend(file_results);
        }

        // Command search (synchronous, always available)
        if matches!(scope, SearchScope::All | SearchScope::Commands) {
            let cmd_results = self.search_plugin_commands(&query, 20);
            all_results.extend(cmd_results);
        }

        // Plugin entry search (synchronous fuzzy match)
        if matches!(scope, SearchScope::All | SearchScope::Items) {
            let plugin_results = self.plugins.iter().flat_map(|plugin| {
                search_plugin_entries(
                    plugin.id(),
                    plugin.name(),
                    &plugin.search_entries(),
                    &query,
                    20,
                )
            });
            all_results.extend(plugin_results);
        }

        // Sort all results by score descending
        all_results.sort_by_key(|r| std::cmp::Reverse(r.score));
        all_results.truncate(50);
        self.search.set_results(all_results);
    }

    fn activate_selected_search_result(&mut self) {
        use rightclick::search::SearchResultKind;

        let Some(result) = self.search.selected_result().cloned() else {
            return;
        };

        match result.kind {
            SearchResultKind::FileContent { path, line, .. } => {
                let result_path = PathBuf::from(&path);
                if let Some(plugin_idx) = self.plugins.iter_mut().position(|plugin| {
                    plugin.id() == "file_browser" && plugin.reveal_path(&result_path)
                }) {
                    self.switch_plugin(plugin_idx);
                    self.notifications.info(format!("Opened {}:{}", path, line));
                } else {
                    self.notifications
                        .warning(format!("File no longer in browser: {}", path));
                }
            }
            SearchResultKind::Conversation { id } => {
                let Some((plugin_id, entry_id)) = id.split_once(':') else {
                    self.notifications
                        .warning(format!("Invalid conversation route: {}", id));
                    return;
                };

                if let Some(plugin_idx) = self.plugins.iter_mut().position(|plugin| {
                    plugin.id() == plugin_id && plugin.activate_search_result(entry_id)
                }) {
                    self.switch_plugin(plugin_idx);
                    self.notifications.info(format!("Opened {}", result.title));
                } else {
                    self.notifications.warning(format!(
                        "Conversation no longer in {}: {}",
                        plugin_id, result.title
                    ));
                }
            }
            SearchResultKind::PluginEntry {
                plugin_id,
                entry_id,
            } => {
                if let Some(plugin_idx) = self.plugins.iter_mut().position(|plugin| {
                    plugin.id() == plugin_id && plugin.activate_search_result(&entry_id)
                }) {
                    self.switch_plugin(plugin_idx);
                    self.notifications.info(format!("Opened {}", result.title));
                } else {
                    self.notifications
                        .warning(format!("Item no longer in {}: {}", plugin_id, result.title));
                }
            }
            SearchResultKind::Command { id } => match self.execute_search_command(&id) {
                Ok(execution) => {
                    if let Some(plugin_idx) = self
                        .plugins
                        .iter()
                        .position(|plugin| plugin.id() == execution.plugin_id.as_str())
                    {
                        self.switch_plugin(plugin_idx);
                    }
                    self.notifications
                        .info(format!("Command: {}", result.title));
                }
                Err(error) => {
                    self.notifications.warning(error.notification_message());
                }
            },
        }
    }

    fn search_plugin_commands(
        &self,
        query: &str,
        max_results: usize,
    ) -> Vec<rightclick::search::SearchResult> {
        use rightclick::search::{SearchResult, SearchResultKind};

        let mut results = Vec::new();
        for plugin in &self.plugins {
            for command in plugin.commands() {
                let full_id = format!("{}:{}", plugin.id(), command.id);
                let name_score = fuzzy_match_simple(&command.name, query).unwrap_or(0);
                let desc_score = fuzzy_match_simple(&command.description, query).unwrap_or(0);
                let id_score = fuzzy_match_simple(&command.id, query).unwrap_or(0);
                let full_id_score = fuzzy_match_simple(&full_id, query).unwrap_or(0);
                let key_score = fuzzy_match_simple(&command.key.to_string(), query).unwrap_or(0);
                let plugin_id_score = fuzzy_match_simple(plugin.id(), query).unwrap_or(0);
                let plugin_name_score = fuzzy_match_simple(plugin.name(), query).unwrap_or(0);
                let category_score =
                    fuzzy_match_simple(command.category.display_name(), query).unwrap_or(0);
                let title = format!("{}: {}", plugin.name(), command.name);
                let title_score = fuzzy_match_simple(&title, query).unwrap_or(0);
                let score = name_score
                    .max(desc_score)
                    .max(id_score)
                    .max(full_id_score)
                    .max(key_score)
                    .max(plugin_id_score)
                    .max(plugin_name_score)
                    .max(category_score)
                    .max(title_score);
                if score == 0 {
                    continue;
                }

                results.push(SearchResult {
                    kind: SearchResultKind::Command { id: full_id },
                    title,
                    preview: format_command_search_preview(plugin.id(), &command),
                    score,
                });
            }
        }

        // App-level system commands exposed via the palette (app.*). These are
        // routed by the shell, not by a plugin, so they are added separately.
        for entry in rightclick::palette::standard_entries()
            .into_iter()
            .filter(|e| e.command_id.starts_with("app."))
        {
            let name_score = fuzzy_match_simple(&entry.name, query).unwrap_or(0);
            let desc_score = fuzzy_match_simple(&entry.description, query).unwrap_or(0);
            let id_score = fuzzy_match_simple(&entry.command_id, query).unwrap_or(0);
            let key_score = fuzzy_match_simple(&entry.key, query).unwrap_or(0);
            let score = name_score.max(desc_score).max(id_score).max(key_score);
            if score == 0 {
                continue;
            }
            results.push(SearchResult {
                kind: SearchResultKind::Command {
                    id: entry.command_id.clone(),
                },
                title: entry.name,
                preview: entry.description,
                score,
            });
        }

        results.sort_by_key(|r| std::cmp::Reverse(r.score));
        results.truncate(max_results);
        results
    }

    fn execute_search_command(
        &mut self,
        id: &str,
    ) -> Result<PluginCommandExecution, SearchCommandError> {
        // App-level commands (app.*) are handled by the shell directly.
        if let Some(app_cmd) = id.strip_prefix("app.") {
            return self
                .execute_app_command(app_cmd)
                .map(|_| PluginCommandExecution {
                    plugin_id: "app".to_string(),
                    command_id: id.to_string(),
                    command_name: id.to_string(),
                    emitted_commands: Vec::new(),
                })
                .map_err(SearchCommandError::PluginCommand);
        }

        let Some((plugin_id, command_id)) = id.split_once(':') else {
            return Err(SearchCommandError::InvalidRoute(id.to_string()));
        };

        let Some(plugin) = self
            .plugins
            .iter_mut()
            .find(|plugin| plugin.id() == plugin_id)
        else {
            return Err(SearchCommandError::PluginUnavailable(plugin_id.to_string()));
        };

        plugin
            .execute_command(command_id)
            .map_err(SearchCommandError::PluginCommand)
    }

    /// Execute an app-level command (`app.<name>`).
    ///
    /// Handles the small set of system commands that the palette exposes with
    /// the `app.` prefix: `quit`, `refresh`, and `settings`.
    fn execute_app_command(&mut self, name: &str) -> Result<(), PluginCommandError> {
        match name {
            "quit" => {
                self.should_quit = true;
                Ok(())
            }
            "refresh" => {
                if let Some(plugin) = self.plugins.get_mut(self.active_plugin) {
                    plugin.handle_event(rightclick::event::Event::RefreshNeeded);
                }
                Ok(())
            }
            "settings" => {
                self.toggle_settings();
                Ok(())
            }
            other => Err(PluginCommandError::UnknownCommand {
                plugin_id: "app".to_string(),
                command_id: other.to_string(),
            }),
        }
    }

    fn render(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        terminal.draw(|f| {
            let size = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Header
                    Constraint::Min(0),    // Content
                    Constraint::Length(1), // Footer
                ])
                .split(size);

            // Header with tabs
            self.render_header(f, chunks[0]);

            // Content - render active plugin
            if let Some(plugin) = self.plugins.get(self.active_plugin) {
                plugin.render(chunks[1], f.buffer_mut(), &self.theme);
            } else {
                let msg = Paragraph::new(no_plugins_empty_message(chunks[1].width))
                    .alignment(Alignment::Center);
                f.render_widget(msg, chunks[1]);
            }

            // Footer
            self.render_footer(f, chunks[2]);

            // Search overlay (on top of content)
            render_search_overlay(&self.search, size, f.buffer_mut(), &self.theme);

            // Help overlay
            if self.show_help {
                self.render_help_overlay(size, f.buffer_mut());
            }

            // Settings modal overlay
            if self.settings_open {
                if let Some(modal) = &self.settings {
                    modal.render(size, f.buffer_mut(), &self.theme);
                }
            }

            // Notification toasts (overlay on top of everything)
            self.notifications.render(size, f.buffer_mut(), &self.theme);
        })?;

        Ok(())
    }

    fn render_header(&self, f: &mut ratatui::Frame, area: Rect) {
        let tab_titles: Vec<String> = self
            .plugins
            .iter()
            .enumerate()
            .map(|(idx, p)| format!(" {} {} {} ", idx + 1, p.icon(), p.name()))
            .collect();

        let subtitle = self.work_dir.display().to_string();
        let header = build_app_header(subtitle, tab_titles, self.active_plugin);
        header.render(area, f.buffer_mut(), &self.theme);
    }

    fn render_footer(&self, f: &mut ratatui::Frame, area: Rect) {
        if let Some(plugin) = self.plugins.get(self.active_plugin) {
            let status = plugin
                .status_line()
                .unwrap_or_else(|| format!("{} ready", plugin.name()));
            let footer = Footer::new(status).with_hints(self.footer_hints(plugin.as_ref()));
            footer.render(area, f.buffer_mut(), &self.theme);
        } else {
            let footer =
                Footer::new(no_plugins_footer_status()).with_hints(no_plugins_footer_hints());
            footer.render(area, f.buffer_mut(), &self.theme);
        }
    }

    fn footer_hints(&self, plugin: &dyn Plugin) -> Vec<(String, String)> {
        build_footer_hints(plugin.id(), &plugin.commands())
    }

    fn render_help_overlay(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        if area.width < 30 || area.height < 10 {
            Paragraph::new(compact_help_overlay_text(area.width))
                .alignment(Alignment::Center)
                .render(area, buf);
            return;
        }

        let width = area.width.saturating_mul(3).saturating_div(5).clamp(30, 80);
        let height = area
            .height
            .saturating_mul(2)
            .saturating_div(3)
            .clamp(10, 28);
        let popup = Rect::new(
            area.x.saturating_add(area.width.saturating_sub(width) / 2),
            area.y
                .saturating_add(area.height.saturating_sub(height) / 2),
            width,
            height,
        );

        Clear.render(popup, buf);

        let block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(crate::theme::style_for_ui_element(
                &self.theme,
                crate::theme::UiElement::Primary,
            ));
        let inner = block.inner(popup);
        block.render(popup, buf);

        let lines = if let Some(plugin) = self.plugins.get(self.active_plugin) {
            let status = plugin
                .status_line()
                .unwrap_or_else(|| format!("{} ready", plugin.name()));
            build_help_lines(plugin.id(), plugin.name(), &plugin.commands(), &status)
        } else {
            build_no_plugins_help_lines()
        };

        let rendered: Vec<Line> = visible_help_lines(lines, inner.height)
            .into_iter()
            .map(|line| {
                if line.ends_with(':') {
                    Line::from(Span::styled(
                        line,
                        Style::default().add_modifier(Modifier::BOLD),
                    ))
                } else {
                    Line::from(line)
                }
            })
            .collect();

        Paragraph::new(rendered)
            .alignment(Alignment::Left)
            .render(inner, buf);
    }
}

fn build_app_header(subtitle: String, tab_titles: Vec<String>, active_plugin: usize) -> Header {
    let header = Header::new("RightClick").with_subtitle(subtitle);
    if tab_titles.is_empty() {
        header
    } else {
        header.with_tabs(tab_titles, active_plugin)
    }
}

#[derive(Clone, Copy)]
struct ShortcutHint {
    key: &'static str,
    label: &'static str,
}

impl ShortcutHint {
    const fn new(key: &'static str, label: &'static str) -> Self {
        Self { key, label }
    }

    fn inline_line(self) -> String {
        if self.key == ":" {
            format!("{} {}", self.key, self.label)
        } else {
            format!("{}: {}", self.key, self.label)
        }
    }

    fn help_line(self) -> String {
        format!("  {}", self.inline_line())
    }

    fn footer_pair(self) -> (String, String) {
        (self.key.to_string(), self.label.to_string())
    }
}

const GLOBAL_SEARCH_SHORTCUT: ShortcutHint = ShortcutHint::new("/", "Global search");
const COMMAND_SEARCH_SHORTCUT: ShortcutHint = ShortcutHint::new(":", "Command search");
const TOGGLE_HELP_SHORTCUT: ShortcutHint = ShortcutHint::new("?", "Toggle help");
const QUIT_SHORTCUT: ShortcutHint = ShortcutHint::new("q/Ctrl+C", "Quit");
const NO_PLUGINS_FOOTER_SHORTCUTS: [ShortcutHint; 4] = [
    TOGGLE_HELP_SHORTCUT,
    GLOBAL_SEARCH_SHORTCUT,
    COMMAND_SEARCH_SHORTCUT,
    QUIT_SHORTCUT,
];
const NO_PLUGINS_HELP_SHORTCUTS: [ShortcutHint; 4] = [
    GLOBAL_SEARCH_SHORTCUT,
    COMMAND_SEARCH_SHORTCUT,
    TOGGLE_HELP_SHORTCUT,
    QUIT_SHORTCUT,
];

fn search_plugin_entries(
    plugin_id: &str,
    plugin_name: &str,
    entries: &[PluginSearchEntry],
    query: &str,
    max_results: usize,
) -> Vec<rightclick::search::SearchResult> {
    use rightclick::search::{SearchResult, SearchResultKind};

    if query.is_empty() {
        return Vec::new();
    }

    let mut results: Vec<SearchResult> = entries
        .iter()
        .filter_map(|entry| {
            let title_score = fuzzy_match_simple(&entry.title, query).unwrap_or(0);
            let preview_score = fuzzy_match_simple(&entry.preview, query).unwrap_or(0);
            let id_score = fuzzy_match_simple(&entry.id, query).unwrap_or(0);
            let score = title_score.max(preview_score).max(id_score);
            if score == 0 {
                return None;
            }

            Some(SearchResult {
                kind: SearchResultKind::PluginEntry {
                    plugin_id: plugin_id.to_string(),
                    entry_id: entry.id.clone(),
                },
                title: format!("{}: {}", plugin_name, entry.title),
                preview: entry.preview.clone(),
                score,
            })
        })
        .collect();

    results.sort_by_key(|r| std::cmp::Reverse(r.score));
    results.truncate(max_results);
    results
}

fn build_help_lines(
    plugin_id: &str,
    plugin_name: &str,
    commands: &[rightclick::plugin::PluginCommand],
    status: &str,
) -> Vec<String> {
    let mut lines = vec![
        plugin_name.to_string(),
        status.to_string(),
        String::new(),
        "Plugin commands:".to_string(),
    ];

    let mut seen = std::collections::HashSet::new();
    for command in commands {
        let key = command.key.to_string();
        if seen.insert((key.clone(), command.name.clone())) {
            lines.push(format_command_help_line(&key, command));
        }
    }
    if seen.is_empty() {
        lines.push("  No view-specific plugin commands".to_string());
    }

    lines.extend([String::new(), "Global shortcuts:".to_string()]);
    lines.extend(global_shortcut_help_lines(plugin_id));

    lines
}

fn global_shortcut_help_lines(plugin_id: &str) -> Vec<String> {
    let mut lines = vec![
        GLOBAL_SEARCH_SHORTCUT.help_line(),
        COMMAND_SEARCH_SHORTCUT.help_line(),
        TOGGLE_HELP_SHORTCUT.help_line(),
        ShortcutHint::new("j/k or ↑/↓", "Navigate items").help_line(),
        ShortcutHint::new("Enter", "Select").help_line(),
        ShortcutHint::new("Ctrl+R", "Refresh current view").help_line(),
    ];

    if plugin_uses_tab_for_panes(plugin_id) {
        lines.extend([
            ShortcutHint::new("Tab", "Switch pane").help_line(),
            ShortcutHint::new("Shift+Tab", "Previous pane").help_line(),
            ShortcutHint::new("Ctrl+Tab", "Next plugin").help_line(),
            ShortcutHint::new("Ctrl+Shift+Tab", "Previous plugin").help_line(),
        ]);
    } else {
        lines.extend([
            ShortcutHint::new("Tab", "Next plugin").help_line(),
            ShortcutHint::new("Shift+Tab", "Previous plugin").help_line(),
        ]);
    }
    lines.extend([
        ShortcutHint::new("1-9", "Jump to plugin").help_line(),
        ShortcutHint::new("Esc", "Back/close").help_line(),
        QUIT_SHORTCUT.help_line(),
    ]);

    lines
}

fn visible_help_lines(lines: Vec<String>, max_height: u16) -> Vec<String> {
    let max_lines = max_height as usize;
    if lines.len() <= max_lines {
        return lines;
    }
    if max_lines == 0 {
        return Vec::new();
    }

    let hidden_count = lines.len().saturating_sub(max_lines.saturating_sub(1));
    let mut visible: Vec<String> = lines.into_iter().take(max_lines).collect();
    visible[max_lines - 1] = hidden_help_line_label(hidden_count);
    visible
}

fn hidden_help_line_label(hidden_count: usize) -> String {
    match hidden_count {
        1 => "  ... 1 more help line".to_string(),
        count => format!("  ... {count} more help lines"),
    }
}

fn compact_help_overlay_text(width: u16) -> &'static str {
    match width {
        0 => "",
        1..=7 => "?",
        8..=12 => "? Help",
        13..=22 => "? Help  Esc",
        23..=29 => "? Help  / Search  Esc",
        30..=39 => "? Help  / Search  : Cmds  Esc",
        40..=52 => "? Toggle help  / Global  : Commands  Esc",
        53..=60 => "? Toggle help  / Global search  : Command search  Esc",
        _ => "? Toggle help  / Global search  : Command search  Esc  q Quit",
    }
}

fn build_no_plugins_help_lines() -> Vec<String> {
    let mut lines = vec![
        "No plugins loaded".to_string(),
        "RightClick is running without an active plugin.".to_string(),
        String::new(),
        "Global shortcuts:".to_string(),
    ];
    lines.extend(no_plugins_global_shortcut_help_lines());
    lines.extend([
        String::new(),
        "Diagnostics:".to_string(),
        "  bash scripts/dev.sh doctor".to_string(),
        "  RUST_LOG=debug bash scripts/dev.sh run".to_string(),
        "  Check configuration if this state persists.".to_string(),
    ]);
    lines
}

fn no_plugins_global_shortcut_help_lines() -> Vec<String> {
    NO_PLUGINS_HELP_SHORTCUTS
        .iter()
        .copied()
        .map(ShortcutHint::help_line)
        .collect()
}

fn build_footer_hints(
    plugin_id: &str,
    commands: &[rightclick::plugin::PluginCommand],
) -> Vec<(String, String)> {
    let mut hints: Vec<(String, String)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let tab_label = if plugin_uses_tab_for_panes(plugin_id) {
        "Pane"
    } else {
        "Switch"
    };

    let mut global_hints = vec![
        ShortcutHint::new("Tab", tab_label),
        ShortcutHint::new("Enter", "Select"),
        ShortcutHint::new("Ctrl+R", "Refresh current view"),
        GLOBAL_SEARCH_SHORTCUT,
        COMMAND_SEARCH_SHORTCUT,
        TOGGLE_HELP_SHORTCUT,
        QUIT_SHORTCUT,
        ShortcutHint::new("1-9", "Focus plugin"),
    ];
    if plugin_uses_tab_for_panes(plugin_id) {
        global_hints.insert(1, ShortcutHint::new("Ctrl+Tab", "Next plugin"));
    }

    for hint in global_hints {
        if seen.insert(hint.key.to_string()) {
            hints.push(hint.footer_pair());
        }
    }

    let mut prioritized_commands: Vec<&rightclick::plugin::PluginCommand> =
        commands.iter().collect();
    prioritized_commands.sort_by_key(|c| std::cmp::Reverse(c.priority));

    for command in prioritized_commands {
        let key = command.key.to_string();
        if matches!(key.as_str(), "j" | "k") {
            continue;
        }
        if seen.insert(key.clone()) {
            hints.push((key, command.name.clone()));
        }
    }

    hints
}

fn plugin_uses_tab_for_panes(plugin_id: &str) -> bool {
    matches!(plugin_id, "git-status" | "workspace" | "workers")
}

fn no_plugins_empty_message(width: u16) -> String {
    let mut lines = vec![
        "No plugins loaded".to_string(),
        String::new(),
        "RightClick is running without an active plugin.".to_string(),
        String::new(),
        fit_no_plugins_line(&TOGGLE_HELP_SHORTCUT.inline_line(), width),
    ];

    if let Some(search_hint) = no_plugins_search_hint(width) {
        lines.push(search_hint.to_string());
    }

    lines.extend([
        fit_no_plugins_line(&QUIT_SHORTCUT.inline_line(), width),
        String::new(),
        "Diagnostics:".to_string(),
        fit_no_plugins_line("bash scripts/dev.sh doctor", width),
        fit_no_plugins_line("RUST_LOG=debug bash scripts/dev.sh run", width),
        "Check configuration if this persists.".to_string(),
    ]);

    lines.join("\n")
}

fn no_plugins_search_hint(width: u16) -> Option<&'static str> {
    compact_global_search_hint(width)
}

fn fit_no_plugins_line(line: &str, width: u16) -> String {
    let max_width = width as usize;
    if line.len() <= max_width {
        return line.to_string();
    }

    if max_width <= 2 {
        return ".".repeat(max_width);
    }

    format!("{}..", &line[..max_width - 2])
}

fn no_plugins_footer_status() -> &'static str {
    "No plugins loaded"
}

fn no_plugins_footer_hints() -> Vec<(String, String)> {
    NO_PLUGINS_FOOTER_SHORTCUTS
        .iter()
        .copied()
        .map(ShortcutHint::footer_pair)
        .collect()
}

fn format_command_help_line(key: &str, command: &rightclick::plugin::PluginCommand) -> String {
    if command.description.is_empty() {
        format!("  {}: {}", key, command.name)
    } else {
        format!("  {}: {} - {}", key, command.name, command.description)
    }
}

fn format_command_search_preview(
    plugin_id: &str,
    command: &rightclick::plugin::PluginCommand,
) -> String {
    let full_id = format!("{}:{}", plugin_id, command.id);
    if command.description.is_empty() {
        format!(
            "Shortcut: {} | Category: {} | ID: {}",
            command.key,
            command.category.display_name(),
            full_id
        )
    } else {
        format!(
            "Shortcut: {} | Category: {} | ID: {} | {}",
            command.key,
            command.category.display_name(),
            full_id,
            command.description
        )
    }
}

fn is_ctrl_c_quit_key(key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::{KeyCode, KeyModifiers};

    matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
        && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn is_ctrl_r_refresh_key(key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::{KeyCode, KeyModifiers};

    matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R'))
        && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn is_ctrl_comma_key(key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::{KeyCode, KeyModifiers};

    matches!(key.code, KeyCode::Char(',')) && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn plugin_shortcut_index(c: char) -> Option<usize> {
    c.to_digit(10)
        .and_then(|digit| digit.checked_sub(1))
        .map(|idx| idx as usize)
}

/// Convert a crossterm KeyCode to a string representation
fn key_code_to_string(code: crossterm::event::KeyCode) -> String {
    use crossterm::event::KeyCode;
    match code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "BackTab".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Insert => "Insert".to_string(),
        KeyCode::F(n) => format!("F{}", n),
        _ => code.to_string(),
    }
}

/// Main application loop
async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let tick_rate = std::time::Duration::from_millis(250);

    loop {
        // Draw UI
        app.render(terminal)?;

        // Handle events with timeout
        if crossterm::event::poll(tick_rate)? {
            let event = crossterm::event::read()?;
            app.handle_event(event).await?;
        }

        // Update plugins for async operations, even when they are not focused.
        for plugin in app.plugins.iter_mut() {
            let _ = plugin.update().await;
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!version().is_empty());
    }

    #[test]
    fn test_build_app_header_keeps_title_without_plugin_tabs() {
        let header = build_app_header("/tmp/rightclick".to_string(), Vec::new(), 0);

        assert_eq!(header.title, "RightClick");
        assert_eq!(header.subtitle, Some("/tmp/rightclick".to_string()));
        assert!(header.tabs.is_empty());
    }

    #[test]
    fn test_build_app_header_includes_plugin_tabs() {
        let header = build_app_header(
            "/tmp/rightclick".to_string(),
            vec![" 1 G Git ".to_string(), " 2 W Workspace ".to_string()],
            1,
        );

        assert_eq!(header.title, "RightClick");
        assert_eq!(header.tabs.len(), 2);
        assert_eq!(header.active_tab, 1);
    }

    #[test]
    fn test_shortcut_hint_formats_command_search_without_extra_colon() {
        assert_eq!(GLOBAL_SEARCH_SHORTCUT.help_line(), "  /: Global search");
        assert_eq!(COMMAND_SEARCH_SHORTCUT.help_line(), "  : Command search");
        assert_eq!(TOGGLE_HELP_SHORTCUT.inline_line(), "?: Toggle help");
    }

    #[test]
    fn test_build_help_lines_includes_plugin_and_global_shortcuts() {
        let commands = vec![rightclick::plugin::PluginCommand::with_context(
            "refresh",
            "Refresh",
            'r',
            rightclick::keymap::FocusContext::Global,
        )];

        let lines = build_help_lines("git-status", "Git Status", &commands, "3 files changed");

        assert!(lines.iter().any(|line| line.contains("Git Status")));
        assert!(lines.iter().any(|line| line.contains("r")));
        assert!(lines.iter().any(|line| line.contains("Refresh")));
        assert!(lines.iter().any(|line| line.contains("/")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("/") && line.contains("Global search"))
        );
        assert!(lines.contains(&"  : Command search".to_string()));
        assert!(lines.iter().any(|line| line == "  ?: Toggle help"));
        assert!(lines.contains(&"  j/k or ↑/↓: Navigate items".to_string()));
        assert!(lines.contains(&"  Enter: Select".to_string()));
        assert!(lines.contains(&"  Ctrl+R: Refresh current view".to_string()));
        assert!(!lines.iter().any(|line| line.contains("Toggle this help")));
        assert!(lines.contains(&"  Tab: Switch pane".to_string()));
        assert!(lines.contains(&"  Shift+Tab: Previous pane".to_string()));
        assert!(lines.contains(&"  Ctrl+Tab: Next plugin".to_string()));
        assert!(lines.contains(&"  Ctrl+Shift+Tab: Previous plugin".to_string()));
        assert!(!lines.iter().any(|line| line.contains("plugin/pane")));
        assert!(lines.contains(&"  Esc: Back/close".to_string()));
        assert!(
            !lines
                .iter()
                .any(|line| line.contains("Switch to previous plugin or pane"))
        );
        assert!(
            !lines
                .iter()
                .any(|line| line.contains("Back or close active view"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("q/Ctrl+C") && line.contains("Quit"))
        );
        assert!(lines.contains(&"  q/Ctrl+C: Quit".to_string()));
        assert!(lines.iter().any(|line| line.contains("3 files changed")));

        let global_index = lines
            .iter()
            .position(|line| line == "Global shortcuts:")
            .expect("global shortcuts section should be present");
        let plugin_index = lines
            .iter()
            .position(|line| line == "Plugin commands:")
            .expect("plugin commands section should be present");
        assert!(plugin_index < global_index);
    }

    #[test]
    fn test_build_help_lines_surfaces_plugin_commands_before_generic_shortcuts() {
        let commands = vec![rightclick::plugin::PluginCommand::with_context_description(
            "commit",
            "Commit",
            "Create a git commit",
            'c',
            rightclick::keymap::FocusContext::Global,
        )];

        let lines = build_help_lines("git-status", "Git Status", &commands, "ready");

        assert_eq!(lines[0], "Git Status");
        assert_eq!(lines[1], "ready");
        assert_eq!(lines[3], "Plugin commands:");
        assert_eq!(lines[4], "  c: Commit - Create a git commit");
        assert!(
            lines
                .iter()
                .position(|line| line == "Plugin commands:")
                .unwrap()
                < lines
                    .iter()
                    .position(|line| line == "Global shortcuts:")
                    .unwrap()
        );
    }

    #[test]
    fn test_build_help_lines_includes_command_descriptions() {
        let commands = vec![rightclick::plugin::PluginCommand::with_context_description(
            "refresh",
            "Refresh",
            "Reload repository state",
            'r',
            rightclick::keymap::FocusContext::Global,
        )];

        let lines = build_help_lines("git-status", "Git Status", &commands, "ready");

        assert!(
            lines
                .iter()
                .any(|line| line == "  r: Refresh - Reload repository state")
        );
    }

    #[test]
    fn test_build_help_lines_describes_empty_plugin_commands() {
        let lines = build_help_lines("workers", "Workers", &[], "Workers ready");

        assert!(lines.iter().any(|line| line.contains("Plugin commands:")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("No view-specific plugin commands"))
        );
        assert!(
            !lines
                .iter()
                .any(|line| line.contains("No plugin commands available in this view"))
        );
        assert!(lines.iter().any(|line| line.contains("Workers ready")));
        assert!(lines.iter().any(|line| line.contains("Global shortcuts:")));
    }

    #[test]
    fn test_build_help_lines_labels_tab_as_plugin_switch_for_simple_plugins() {
        let lines = build_help_lines("conversations", "Conversations", &[], "ready");

        assert!(lines.contains(&"  Tab: Next plugin".to_string()));
        assert!(lines.contains(&"  Shift+Tab: Previous plugin".to_string()));
        assert!(!lines.iter().any(|line| line == "  Ctrl+Tab: Next plugin"));
        assert!(!lines.iter().any(|line| line.contains("plugin/pane")));
    }

    #[test]
    fn test_visible_help_lines_keeps_short_help_unchanged() {
        let lines = vec!["Help".to_string(), "  ?  Toggle help".to_string()];

        assert_eq!(visible_help_lines(lines.clone(), 3), lines);
    }

    #[test]
    fn test_visible_help_lines_marks_truncated_help() {
        let lines = vec![
            "Help".to_string(),
            "Plugin commands:".to_string(),
            "  r: Refresh".to_string(),
            "Global shortcuts:".to_string(),
        ];

        assert_eq!(
            visible_help_lines(lines, 3),
            vec![
                "Help".to_string(),
                "Plugin commands:".to_string(),
                "  ... 2 more help lines".to_string()
            ]
        );
    }

    #[test]
    fn test_visible_help_lines_counts_hidden_lines() {
        let lines = vec![
            "Help".to_string(),
            "Plugin commands:".to_string(),
            "  r: Refresh".to_string(),
            "  c: Commit".to_string(),
            "Global shortcuts:".to_string(),
            "  q/Ctrl+C: Quit".to_string(),
        ];

        assert_eq!(
            visible_help_lines(lines, 3),
            vec![
                "Help".to_string(),
                "Plugin commands:".to_string(),
                "  ... 4 more help lines".to_string()
            ]
        );
    }

    #[test]
    fn test_hidden_help_line_label_uses_singular_and_plural() {
        assert_eq!(hidden_help_line_label(1), "  ... 1 more help line");
        assert_eq!(hidden_help_line_label(3), "  ... 3 more help lines");
    }

    #[test]
    fn test_visible_help_lines_handles_zero_height() {
        let lines = vec!["Help".to_string()];

        assert!(visible_help_lines(lines, 0).is_empty());
    }

    #[test]
    fn test_compact_help_overlay_text_fits_width() {
        for width in 0..=80 {
            assert!(compact_help_overlay_text(width).len() <= width as usize);
        }
        assert_eq!(compact_help_overlay_text(0), "");
        assert_eq!(compact_help_overlay_text(7), "?");
        assert_eq!(compact_help_overlay_text(12), "? Help");
        assert_eq!(compact_help_overlay_text(18), "? Help  Esc");
        assert_eq!(compact_help_overlay_text(25), "? Help  / Search  Esc");
        assert_eq!(
            compact_help_overlay_text(30),
            "? Help  / Search  : Cmds  Esc"
        );
        assert_eq!(
            compact_help_overlay_text(40),
            "? Toggle help  / Global  : Commands  Esc"
        );
        assert_eq!(
            compact_help_overlay_text(53),
            "? Toggle help  / Global search  : Command search  Esc"
        );
        assert_eq!(
            compact_help_overlay_text(61),
            "? Toggle help  / Global search  : Command search  Esc  q Quit"
        );
    }

    #[test]
    fn test_help_overlay_renders_compact_text_in_small_area() {
        let app = App::new(
            Vec::new(),
            Theme::default(),
            PathBuf::from("/tmp/rightclick"),
            Config::default(),
            std::sync::Arc::new(Dispatcher::new()),
        );
        let area = Rect::new(0, 0, 26, 3);
        let mut buf = ratatui::buffer::Buffer::empty(area);

        app.render_help_overlay(area, &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("? Help"));
        assert!(content.contains("Esc"));
    }

    #[test]
    fn test_help_overlay_renders_inside_offset_area_near_u16_max() {
        let app = App::new(
            Vec::new(),
            Theme::default(),
            PathBuf::from("/tmp/rightclick"),
            Config::default(),
            std::sync::Arc::new(Dispatcher::new()),
        );
        let area = Rect::new(u16::MAX - 80, u16::MAX - 40, 80, 40);
        let mut buf = ratatui::buffer::Buffer::empty(area);

        app.render_help_overlay(area, &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Help"));
    }

    #[test]
    fn test_build_no_plugins_help_lines_points_to_global_actions() {
        let lines = build_no_plugins_help_lines();

        assert!(lines.iter().any(|line| line.contains("No plugins loaded")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("/") && line.contains("Global search"))
        );
        assert!(lines.contains(&"  : Command search".to_string()));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("?") && line.contains("Toggle help"))
        );
        assert!(!lines.iter().any(|line| line.contains("Toggle this help")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("q/Ctrl+C") && line.contains("Quit"))
        );
        assert!(lines.contains(&"  q/Ctrl+C: Quit".to_string()));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("bash scripts/dev.sh doctor"))
        );
        assert!(lines.contains(&"  RUST_LOG=debug bash scripts/dev.sh run".to_string()));
        assert!(lines.iter().any(|line| line.contains("configuration")));
    }

    #[test]
    fn test_command_search_preview_includes_shortcut_with_description() {
        let command = rightclick::plugin::PluginCommand::with_context_description(
            "refresh",
            "Refresh",
            "Reload repository state",
            'r',
            rightclick::keymap::FocusContext::Global,
        );

        assert_eq!(
            format_command_search_preview("git-status", &command),
            "Shortcut: r | Category: System | ID: git-status:refresh | Reload repository state"
        );
    }

    #[test]
    fn test_command_search_preview_falls_back_to_shortcut_label() {
        let command = rightclick::plugin::PluginCommand::with_context(
            "refresh",
            "Refresh",
            'r',
            rightclick::keymap::FocusContext::Global,
        );

        assert_eq!(
            format_command_search_preview("git-status", &command),
            "Shortcut: r | Category: System | ID: git-status:refresh"
        );
    }

    #[test]
    fn test_build_footer_hints_prioritizes_global_shortcuts() {
        let commands = vec![
            rightclick::plugin::PluginCommand::with_context(
                "refresh",
                "Refresh",
                'r',
                rightclick::keymap::FocusContext::Global,
            ),
            rightclick::plugin::PluginCommand::with_context(
                "down",
                "Down",
                'j',
                rightclick::keymap::FocusContext::Global,
            ),
        ];

        let hints = build_footer_hints("workspace", &commands);

        assert_eq!(
            &hints[..8],
            &[
                ("Tab".to_string(), "Pane".to_string()),
                ("Ctrl+Tab".to_string(), "Next plugin".to_string()),
                ("Enter".to_string(), "Select".to_string()),
                ("Ctrl+R".to_string(), "Refresh current view".to_string()),
                ("/".to_string(), "Global search".to_string()),
                (":".to_string(), "Command search".to_string()),
                ("?".to_string(), "Toggle help".to_string()),
                ("q/Ctrl+C".to_string(), "Quit".to_string()),
            ]
        );
        assert!(hints.contains(&("1-9".to_string(), "Focus plugin".to_string())));
        assert!(hints.contains(&("r".to_string(), "Refresh".to_string())));
        assert!(!hints.iter().any(|(key, _)| key == "j"));
    }

    #[test]
    fn test_build_footer_hints_uses_command_priority() {
        let commands = vec![
            rightclick::plugin::PluginCommand::with_context(
                "secondary",
                "Secondary",
                's',
                rightclick::keymap::FocusContext::Global,
            ),
            rightclick::plugin::PluginCommand::with_priority(
                "primary",
                "Primary",
                "Primary action",
                rightclick::plugin::Category::System,
                'p',
                rightclick::keymap::FocusContext::Global,
                1,
            ),
        ];

        let hints = build_footer_hints("workspace", &commands);

        let plugin_hints = &hints[9..];
        assert_eq!(plugin_hints[0], ("p".to_string(), "Primary".to_string()));
        assert_eq!(plugin_hints[1], ("s".to_string(), "Secondary".to_string()));
    }

    #[test]
    fn test_build_footer_hints_orders_higher_priorities_first() {
        let commands = vec![
            rightclick::plugin::PluginCommand::with_priority(
                "low",
                "Low",
                "Lower priority",
                rightclick::plugin::Category::System,
                'l',
                rightclick::keymap::FocusContext::Global,
                1,
            ),
            rightclick::plugin::PluginCommand::with_priority(
                "high",
                "High",
                "Higher priority",
                rightclick::plugin::Category::System,
                'h',
                rightclick::keymap::FocusContext::Global,
                5,
            ),
        ];

        let hints = build_footer_hints("workspace", &commands);

        let plugin_hints = &hints[9..];
        assert_eq!(plugin_hints[0], ("h".to_string(), "High".to_string()));
        assert_eq!(plugin_hints[1], ("l".to_string(), "Low".to_string()));
    }

    #[test]
    fn test_build_footer_hints_keeps_all_unique_plugin_actions() {
        let commands = vec![
            rightclick::plugin::PluginCommand::with_context(
                "one",
                "One",
                'a',
                rightclick::keymap::FocusContext::Global,
            ),
            rightclick::plugin::PluginCommand::with_context(
                "two",
                "Two",
                'b',
                rightclick::keymap::FocusContext::Global,
            ),
            rightclick::plugin::PluginCommand::with_context(
                "three",
                "Three",
                'c',
                rightclick::keymap::FocusContext::Global,
            ),
        ];

        let hints = build_footer_hints("workspace", &commands);

        assert!(hints.contains(&("a".to_string(), "One".to_string())));
        assert!(hints.contains(&("b".to_string(), "Two".to_string())));
        assert!(hints.contains(&("c".to_string(), "Three".to_string())));
    }

    #[test]
    fn test_build_footer_hints_labels_git_tab_as_pane() {
        let hints = build_footer_hints("git-status", &[]);
        assert_eq!(hints[0], ("Tab".to_string(), "Pane".to_string()));
        assert_eq!(
            hints[1],
            ("Ctrl+Tab".to_string(), "Next plugin".to_string())
        );
    }

    #[test]
    fn test_build_footer_hints_labels_workers_tab_as_pane() {
        let hints = build_footer_hints("workers", &[]);
        assert_eq!(hints[0], ("Tab".to_string(), "Pane".to_string()));
        assert_eq!(
            hints[1],
            ("Ctrl+Tab".to_string(), "Next plugin".to_string())
        );
    }

    #[test]
    fn test_build_footer_hints_surfaces_workers_refresh() {
        let plugin = workers::WorkersPlugin::new();
        let commands = <workers::WorkersPlugin as Plugin>::commands(&plugin);
        let hints = build_footer_hints(plugin.id(), &commands);

        // After the workers command reshuffle the reload-intents command moves
        // from `f` to `r`, the `f` reload entry is removed, and "Run Workers"
        // moves to `R`. Global `Ctrl+R` ("Refresh current view") is a distinct
        // key, so the plugin-scoped `r` hint still surfaces here.
        assert!(hints.contains(&("r".to_string(), "Reload Intents".to_string())));
        // TODO(merge): the lead described this as "Refresh/Reload Intents". If
        // the descriptors branch keeps the label "Reload Intents" this passes
        // as-is; if it renames to "Refresh Intents", update the assertion above.
        assert!(!hints.iter().any(|(key, _)| key == "f"));
    }

    #[test]
    fn test_build_footer_hints_surfaces_workspace_refresh() {
        let plugin = workspace::WorkspacePlugin::new();
        let commands = <workspace::WorkspacePlugin as Plugin>::commands(&plugin);
        let hints = build_footer_hints(plugin.id(), &commands);

        assert!(hints.contains(&("r".to_string(), "Refresh Worktrees".to_string())));
    }

    #[test]
    fn test_build_footer_hints_omits_ctrl_tab_when_tab_switches_plugins() {
        let hints = build_footer_hints("conversations", &[]);
        assert_eq!(hints[0], ("Tab".to_string(), "Switch".to_string()));
        assert!(!hints.iter().any(|(key, _)| key == "Ctrl+Tab"));
    }

    #[test]
    fn test_plugin_uses_tab_for_panes_matches_pane_plugins() {
        assert!(plugin_uses_tab_for_panes("git-status"));
        assert!(plugin_uses_tab_for_panes("workspace"));
        assert!(plugin_uses_tab_for_panes("workers"));
        assert!(!plugin_uses_tab_for_panes("conversations"));
    }

    #[tokio::test]
    async fn test_workers_tab_keys_are_routed_to_plugin_panes() {
        use crossterm::event::{Event as CEvent, KeyCode, KeyEvent, KeyModifiers};

        let plugins: Vec<Box<dyn rightclick::plugin::Plugin>> = vec![
            Box::new(workers::WorkersPlugin::new()),
            Box::new(workspace::WorkspacePlugin::new()),
        ];
        let mut app = App::new(
            plugins,
            Theme::default(),
            PathBuf::from("/tmp/rightclick"),
            Config::default(),
            std::sync::Arc::new(Dispatcher::new()),
        );

        app.handle_event(CEvent::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)))
            .await
            .unwrap();
        assert_eq!(app.active_plugin, 0);

        app.handle_event(CEvent::Key(KeyEvent::new(
            KeyCode::BackTab,
            KeyModifiers::SHIFT,
        )))
        .await
        .unwrap();
        assert_eq!(app.active_plugin, 0);
    }

    #[tokio::test]
    async fn test_ctrl_tab_still_switches_from_pane_plugins() {
        use crossterm::event::{Event as CEvent, KeyCode, KeyEvent, KeyModifiers};

        let plugins: Vec<Box<dyn rightclick::plugin::Plugin>> = vec![
            Box::new(workers::WorkersPlugin::new()),
            Box::new(workspace::WorkspacePlugin::new()),
        ];
        let mut app = App::new(
            plugins,
            Theme::default(),
            PathBuf::from("/tmp/rightclick"),
            Config::default(),
            std::sync::Arc::new(Dispatcher::new()),
        );

        app.handle_event(CEvent::Key(KeyEvent::new(
            KeyCode::Tab,
            KeyModifiers::CONTROL,
        )))
        .await
        .unwrap();

        assert_eq!(app.active_plugin, 1);
    }

    #[derive(Debug)]
    struct RecordingPlugin {
        id: &'static str,
        name: &'static str,
        commands: Vec<rightclick::plugin::PluginCommand>,
        events: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        focused: bool,
    }

    #[async_trait::async_trait]
    impl Plugin for RecordingPlugin {
        fn id(&self) -> &str {
            self.id
        }

        fn name(&self) -> &str {
            self.name
        }

        fn icon(&self) -> char {
            'R'
        }

        async fn init(&mut self, _ctx: &PluginContext) -> anyhow::Result<()> {
            Ok(())
        }

        fn shutdown(&mut self) -> anyhow::Result<()> {
            Ok(())
        }

        fn handle_event(
            &mut self,
            event: rightclick::event::Event,
        ) -> Vec<rightclick::plugin::Command> {
            let label = match event {
                rightclick::event::Event::RefreshNeeded => "refresh".to_string(),
                rightclick::event::Event::Key { code, modifiers } => {
                    format!("key:{code}:ctrl={}", modifiers.ctrl)
                }
                _ => "other".to_string(),
            };
            self.events.lock().unwrap().push(label);
            Vec::new()
        }

        fn render(&self, _area: Rect, _buf: &mut ratatui::buffer::Buffer, _theme: &Theme) {}

        fn is_focused(&self) -> bool {
            self.focused
        }

        fn set_focused(&mut self, focused: bool) {
            self.focused = focused;
        }

        fn commands(&self) -> Vec<rightclick::plugin::PluginCommand> {
            self.commands.clone()
        }

        fn focus_context(&self) -> rightclick::keymap::FocusContext {
            rightclick::keymap::FocusContext::Global
        }
    }

    #[tokio::test]
    async fn test_ctrl_r_requests_active_plugin_refresh() {
        use crossterm::event::{Event as CEvent, KeyCode, KeyEvent, KeyModifiers};

        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let plugins: Vec<Box<dyn rightclick::plugin::Plugin>> = vec![Box::new(RecordingPlugin {
            id: "recording",
            name: "Recording",
            commands: Vec::new(),
            events: events.clone(),
            focused: false,
        })];
        let mut app = App::new(
            plugins,
            Theme::default(),
            PathBuf::from("/tmp/rightclick"),
            Config::default(),
            std::sync::Arc::new(Dispatcher::new()),
        );

        app.handle_event(CEvent::Key(KeyEvent::new(
            KeyCode::Char('r'),
            KeyModifiers::CONTROL,
        )))
        .await
        .unwrap();

        assert_eq!(*events.lock().unwrap(), vec!["refresh".to_string()]);
    }

    #[test]
    fn test_no_plugins_empty_state_points_to_global_actions() {
        let message = no_plugins_empty_message(80);
        let hints = no_plugins_footer_hints();

        assert!(message.contains("No plugins loaded"));
        assert!(message.contains("RightClick is running without an active plugin."));
        assert!(message.contains("?: Toggle help"));
        assert!(message.contains("/: Global search  |  : Command search"));
        assert!(!message.contains("/: Global search\n: Command search"));
        assert!(message.contains("q/Ctrl+C: Quit"));
        assert!(message.contains("Diagnostics:"));
        assert!(message.contains("bash scripts/dev.sh doctor"));
        assert!(message.contains("RUST_LOG=debug bash scripts/dev.sh run"));
        assert!(message.contains("Check configuration"));
        assert_eq!(no_plugins_footer_status(), "No plugins loaded");
        assert_eq!(
            hints,
            vec![
                ("?".to_string(), "Toggle help".to_string()),
                ("/".to_string(), "Global search".to_string()),
                (":".to_string(), "Command search".to_string()),
                ("q/Ctrl+C".to_string(), "Quit".to_string()),
            ]
        );
    }

    #[test]
    fn test_no_plugins_search_hint_compacts_for_narrow_widths() {
        assert_eq!(no_plugins_search_hint(1), None);
        assert_eq!(no_plugins_search_hint(2), Some("/:"));
        assert_eq!(no_plugins_search_hint(9), Some("/: Search"));
        assert_eq!(no_plugins_search_hint(20), Some("/: Search  |  : Cmds"));
        assert_eq!(no_plugins_search_hint(24), Some("/: Search  |  : Commands"));
        assert_eq!(
            no_plugins_search_hint(80),
            Some(rightclick::ui::GLOBAL_SEARCH_HINT)
        );
    }

    #[test]
    fn test_no_plugins_empty_state_omits_search_hint_when_too_narrow() {
        let message = no_plugins_empty_message(1);

        assert!(message.contains("No plugins loaded"));
        assert!(!message.contains("Global search"));
        assert!(!message.contains("/:"));
        assert!(message.contains("Diagnostics:"));
    }

    #[test]
    fn test_no_plugins_diagnostics_truncate_for_narrow_widths() {
        let message = no_plugins_empty_message(20);

        assert!(message.contains("bash scripts/dev.s.."));
        assert!(message.contains("RUST_LOG=debug bas.."));
        assert!(!message.contains("bash scripts/dev.sh doctor"));
        assert!(!message.contains("RUST_LOG=debug bash scripts/dev.sh run"));
    }

    #[test]
    fn test_search_plugin_entries_routes_to_plugin_and_entry_id() {
        let entries = vec![rightclick::plugin::PluginSearchEntry {
            id: "session-1".to_string(),
            title: "Investigate render bug".to_string(),
            preview: "Mock Adapter | 4 messages".to_string(),
        }];

        let results =
            search_plugin_entries("conversations", "Conversations", &entries, "render", 10);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Conversations: Investigate render bug");
        assert!(matches!(
            &results[0].kind,
            rightclick::search::SearchResultKind::PluginEntry { plugin_id, entry_id }
                if plugin_id == "conversations" && entry_id == "session-1"
        ));
    }

    #[test]
    fn test_search_plugin_commands_matches_visible_plugin_title() {
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let commands = vec![rightclick::plugin::PluginCommand::with_context_description(
            "refresh",
            "Refresh Data",
            "Reload current plugin data",
            'r',
            rightclick::keymap::FocusContext::Global,
        )];
        let plugins: Vec<Box<dyn rightclick::plugin::Plugin>> = vec![Box::new(RecordingPlugin {
            id: "target",
            name: "Target",
            commands,
            events,
            focused: false,
        })];
        let app = App::new(
            plugins,
            Theme::default(),
            PathBuf::from("/tmp/rightclick"),
            Config::default(),
            std::sync::Arc::new(Dispatcher::new()),
        );

        let results = app.search_plugin_commands("target refresh", 10);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Target: Refresh Data");
        assert!(matches!(
            &results[0].kind,
            rightclick::search::SearchResultKind::Command { id } if id == "target:refresh"
        ));
    }

    #[test]
    fn test_search_plugin_commands_matches_shortcut_key() {
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let commands = vec![rightclick::plugin::PluginCommand::with_context_description(
            "rebuild",
            "Build Project",
            "Compile workspace",
            'z',
            rightclick::keymap::FocusContext::Global,
        )];
        let plugins: Vec<Box<dyn rightclick::plugin::Plugin>> = vec![Box::new(RecordingPlugin {
            id: "builder",
            name: "Builder",
            commands,
            events,
            focused: false,
        })];
        let app = App::new(
            plugins,
            Theme::default(),
            PathBuf::from("/tmp/rightclick"),
            Config::default(),
            std::sync::Arc::new(Dispatcher::new()),
        );

        let results = app.search_plugin_commands("z", 10);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Builder: Build Project");
        assert_eq!(
            results[0].preview,
            "Shortcut: z | Category: System | ID: builder:rebuild | Compile workspace"
        );
    }

    #[test]
    fn test_search_plugin_commands_matches_category() {
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let commands = vec![rightclick::plugin::PluginCommand::new(
            "sync",
            "Sync Changes",
            "Fetch remote updates",
            rightclick::plugin::Category::Git,
            's',
            rightclick::keymap::FocusContext::Global,
            0,
        )];
        let plugins: Vec<Box<dyn rightclick::plugin::Plugin>> = vec![Box::new(RecordingPlugin {
            id: "repository",
            name: "Repository",
            commands,
            events,
            focused: false,
        })];
        let app = App::new(
            plugins,
            Theme::default(),
            PathBuf::from("/tmp/rightclick"),
            Config::default(),
            std::sync::Arc::new(Dispatcher::new()),
        );

        let results = app.search_plugin_commands("git", 10);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Repository: Sync Changes");
        assert_eq!(
            results[0].preview,
            "Shortcut: s | Category: Git | ID: repository:sync | Fetch remote updates"
        );
    }

    #[test]
    fn test_search_plugin_commands_matches_full_command_id() {
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let commands = vec![rightclick::plugin::PluginCommand::with_context_description(
            "open-pr",
            "Open Pull Request",
            "Open review in browser",
            'o',
            rightclick::keymap::FocusContext::Global,
        )];
        let plugins: Vec<Box<dyn rightclick::plugin::Plugin>> = vec![Box::new(RecordingPlugin {
            id: "repo_tools",
            name: "Review Tools",
            commands,
            events,
            focused: false,
        })];
        let app = App::new(
            plugins,
            Theme::default(),
            PathBuf::from("/tmp/rightclick"),
            Config::default(),
            std::sync::Arc::new(Dispatcher::new()),
        );

        let results = app.search_plugin_commands("repo_tools:open-pr", 10);

        assert_eq!(results.len(), 1);
        assert!(matches!(
            &results[0].kind,
            rightclick::search::SearchResultKind::Command { id } if id == "repo_tools:open-pr"
        ));
        assert_eq!(
            results[0].preview,
            "Shortcut: o | Category: System | ID: repo_tools:open-pr | Open review in browser"
        );
    }

    #[test]
    fn test_command_search_activation_switches_to_command_plugin() {
        let source_events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let target_events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let target_commands = vec![rightclick::plugin::PluginCommand::with_context_description(
            "refresh",
            "Refresh Target",
            "Reload target plugin data",
            'r',
            rightclick::keymap::FocusContext::Global,
        )];
        let plugins: Vec<Box<dyn rightclick::plugin::Plugin>> = vec![
            Box::new(RecordingPlugin {
                id: "source",
                name: "Source",
                commands: Vec::new(),
                events: source_events.clone(),
                focused: false,
            }),
            Box::new(RecordingPlugin {
                id: "target",
                name: "Target",
                commands: target_commands,
                events: target_events.clone(),
                focused: false,
            }),
        ];
        let mut app = App::new(
            plugins,
            Theme::default(),
            PathBuf::from("/tmp/rightclick"),
            Config::default(),
            std::sync::Arc::new(Dispatcher::new()),
        );

        let search_result = rightclick::search::SearchResult {
            kind: rightclick::search::SearchResultKind::Command {
                id: "target:refresh".to_string(),
            },
            title: "Target: Refresh Target".to_string(),
            preview:
                "Shortcut: r | Category: System | ID: target:refresh | Reload target plugin data"
                    .to_string(),
            score: 100,
        };
        app.search.set_results(vec![search_result]);

        app.activate_selected_search_result();

        assert_eq!(app.active_plugin, 1);
        assert!(app.plugins[1].is_focused());
        assert!(source_events.lock().unwrap().is_empty());
        assert_eq!(
            *target_events.lock().unwrap(),
            vec!["key:r:ctrl=false".to_string()]
        );
        let active_notifications = app.notifications.active();
        assert_eq!(active_notifications.len(), 1);
        assert_eq!(
            active_notifications[0].message,
            "Command: Target: Refresh Target"
        );
    }

    #[test]
    fn test_stale_file_search_result_reports_browser_context() {
        let mut app = App::new(
            Vec::new(),
            Theme::default(),
            PathBuf::from("/tmp/rightclick"),
            Config::default(),
            std::sync::Arc::new(Dispatcher::new()),
        );
        app.search
            .set_results(vec![rightclick::search::SearchResult {
                kind: rightclick::search::SearchResultKind::FileContent {
                    path: "/tmp/rightclick/src/main.rs".to_string(),
                    line: 12,
                    column: 1,
                },
                title: "src/main.rs".to_string(),
                preview: "fn main() {}".to_string(),
                score: 100,
            }]);

        app.activate_selected_search_result();

        let active_notifications = app.notifications.active();
        assert_eq!(active_notifications.len(), 1);
        assert_eq!(
            active_notifications[0].message,
            "File no longer in browser: /tmp/rightclick/src/main.rs"
        );
    }

    #[test]
    fn test_stale_plugin_search_result_reports_plugin_context() {
        let mut app = App::new(
            Vec::new(),
            Theme::default(),
            PathBuf::from("/tmp/rightclick"),
            Config::default(),
            std::sync::Arc::new(Dispatcher::new()),
        );
        app.search
            .set_results(vec![rightclick::search::SearchResult {
                kind: rightclick::search::SearchResultKind::PluginEntry {
                    plugin_id: "workspace".to_string(),
                    entry_id: "wt-1".to_string(),
                },
                title: "feature/work".to_string(),
                preview: "worktree".to_string(),
                score: 100,
            }]);

        app.activate_selected_search_result();

        let active_notifications = app.notifications.active();
        assert_eq!(active_notifications.len(), 1);
        assert_eq!(
            active_notifications[0].message,
            "Item no longer in workspace: feature/work"
        );
    }

    #[test]
    fn test_command_search_unloaded_plugin_message_names_command_plugin() {
        let error = SearchCommandError::PluginUnavailable("workspace".to_string());

        assert_eq!(
            error.notification_message(),
            "Command plugin not loaded: workspace"
        );
    }

    #[test]
    fn test_ctrl_c_quit_key_accepts_lowercase_and_shifted_forms() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let ctrl_shift_c = KeyEvent::new(
            KeyCode::Char('C'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        let plain_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);

        assert!(is_ctrl_c_quit_key(&ctrl_c));
        assert!(is_ctrl_c_quit_key(&ctrl_shift_c));
        assert!(!is_ctrl_c_quit_key(&plain_c));
    }

    #[test]
    fn test_ctrl_r_refresh_key_accepts_lowercase_and_shifted_forms() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let ctrl_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        let ctrl_shift_r = KeyEvent::new(
            KeyCode::Char('R'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        let plain_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);

        assert!(is_ctrl_r_refresh_key(&ctrl_r));
        assert!(is_ctrl_r_refresh_key(&ctrl_shift_r));
        assert!(!is_ctrl_r_refresh_key(&plain_r));
    }

    #[test]
    fn test_plugin_shortcut_index_maps_digits_without_panics() {
        assert_eq!(plugin_shortcut_index('1'), Some(0));
        assert_eq!(plugin_shortcut_index('9'), Some(8));
        assert_eq!(plugin_shortcut_index('0'), None);
        assert_eq!(plugin_shortcut_index('a'), None);
    }
}
