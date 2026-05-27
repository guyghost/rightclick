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
    core::models::Theme,
    plugin::{
        Plugin, PluginCommandError, PluginCommandExecution, PluginContext, PluginSearchEntry,
    },
    plugins::{conversations, filebrowser, gitstatus, workers, workspace},
    search::{
        SearchOverlayAction, SearchOverlayState, SearchScope, render_search_overlay, search_files,
    },
    state,
    theme::{self, resolve_theme},
    ui::{Footer, Header, NotificationManager},
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
            Self::PluginUnavailable(plugin_id) => format!("Plugin not available: {}", plugin_id),
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

    // Create plugin context
    let plugin_ctx = PluginContext {
        work_dir: work_dir.clone(),
        project_root: project_root.clone(),
        config_dir: config::config_dir().unwrap_or_else(|_| PathBuf::from("~/.config/rightclick")),
        config: config.clone(),
        adapters: adapters.into(),
        event_bus: std::sync::Arc::new(rightclick::event::Dispatcher::new()),
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
    let mut app = App::new(plugins, theme, work_dir.clone());

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
}

impl App {
    fn new(plugins: Vec<Box<dyn Plugin>>, theme: Theme, work_dir: PathBuf) -> Self {
        Self {
            plugins,
            theme,
            active_plugin: 0,
            should_quit: false,
            show_help: false,
            notifications: NotificationManager::new(),
            search: SearchOverlayState::new(),
            work_dir,
        }
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
        all_results.sort_by(|a, b| b.score.cmp(&a.score));
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
                        .warning(format!("File not available: {}", path));
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
                    self.notifications
                        .warning(format!("Conversation not available: {}", result.title));
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
                        .warning(format!("Item not available: {}", result.title));
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
                let name_score = fuzzy_match_simple(&command.name, query).unwrap_or(0);
                let desc_score = fuzzy_match_simple(&command.description, query).unwrap_or(0);
                let id_score = fuzzy_match_simple(&command.id, query).unwrap_or(0);
                let title = format!("{}: {}", plugin.name(), command.name);
                let title_score = fuzzy_match_simple(&title, query).unwrap_or(0);
                let score = name_score.max(desc_score).max(id_score).max(title_score);
                if score == 0 {
                    continue;
                }

                results.push(SearchResult {
                    kind: SearchResultKind::Command {
                        id: format!("{}:{}", plugin.id(), command.id),
                    },
                    title,
                    preview: format_command_search_preview(&command),
                    score,
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(max_results);
        results
    }

    fn execute_search_command(
        &mut self,
        id: &str,
    ) -> Result<PluginCommandExecution, SearchCommandError> {
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
                let msg = Paragraph::new(no_plugins_empty_message()).alignment(Alignment::Center);
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

    results.sort_by(|a, b| b.score.cmp(&a.score));
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
        "Global shortcuts:".to_string(),
        "  /: Global search".to_string(),
        "  :: Command search".to_string(),
        "  ?: Toggle help".to_string(),
        "  j/k or ↑/↓: Navigate items".to_string(),
        "  Enter: Select".to_string(),
        "  Ctrl+R: Refresh current view".to_string(),
    ];
    if plugin_uses_tab_for_panes(plugin_id) {
        lines.extend([
            "  Tab: Switch pane".to_string(),
            "  Shift+Tab: Previous pane".to_string(),
            "  Ctrl+Tab: Next plugin".to_string(),
            "  Ctrl+Shift+Tab: Previous plugin".to_string(),
        ]);
    } else {
        lines.extend([
            "  Tab: Next plugin".to_string(),
            "  Shift+Tab: Previous plugin".to_string(),
        ]);
    }
    lines.extend([
        "  1-9: Jump to plugin".to_string(),
        "  Esc: Back/close".to_string(),
        "  q/Ctrl+C: Quit".to_string(),
        String::new(),
        "Plugin commands:".to_string(),
    ]);

    let mut seen = std::collections::HashSet::new();
    for command in commands {
        let key = command.key.to_string();
        if seen.insert((key.clone(), command.name.clone())) {
            lines.push(format_command_help_line(&key, command));
        }
    }
    if seen.is_empty() {
        lines.push("  No plugin commands available in this view".to_string());
    }

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

    let mut visible: Vec<String> = lines.into_iter().take(max_lines).collect();
    visible[max_lines - 1] = "  ... more help hidden".to_string();
    visible
}

fn compact_help_overlay_text(width: u16) -> &'static str {
    match width {
        0 => "",
        1..=7 => "?",
        8..=12 => "? Toggle",
        13..=22 => "? Toggle help",
        23..=34 => "? Toggle help  / Global",
        35..=40 => "? Toggle help  / Global  : Commands",
        41..=55 => "? Toggle help  / Global  : Command search",
        _ => "? Toggle help  / Global search  : Command search  q Quit",
    }
}

fn build_no_plugins_help_lines() -> Vec<String> {
    vec![
        "No plugins loaded".to_string(),
        "RightClick is running without an active plugin.".to_string(),
        String::new(),
        "Global shortcuts:".to_string(),
        "  /: Global search".to_string(),
        "  :: Command search".to_string(),
        "  ?: Toggle help".to_string(),
        "  q/Ctrl+C: Quit".to_string(),
        String::new(),
        "Diagnostics:".to_string(),
        "  bash scripts/dev.sh doctor".to_string(),
        "  RUST_LOG=debug bash scripts/dev.sh run".to_string(),
        "  Check configuration if this state persists.".to_string(),
    ]
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
        ("Tab", tab_label),
        ("Enter", "Select"),
        ("Ctrl+R", "Refresh current view"),
        ("/", "Global search"),
        (":", "Command search"),
        ("?", "Toggle help"),
        ("q/Ctrl+C", "Quit"),
        ("1-9", "Focus plugin"),
    ];
    if plugin_uses_tab_for_panes(plugin_id) {
        global_hints.insert(1, ("Ctrl+Tab", "Next plugin"));
    }

    for (key, label) in global_hints {
        if seen.insert(key.to_string()) {
            hints.push((key.to_string(), label.to_string()));
        }
    }

    let mut prioritized_commands: Vec<&rightclick::plugin::PluginCommand> =
        commands.iter().collect();
    prioritized_commands.sort_by(|left, right| right.priority.cmp(&left.priority));

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

fn no_plugins_empty_message() -> &'static str {
    "No plugins loaded\n\n?: Toggle help\n/: Global search\n:: Command search\nq/Ctrl+C: Quit\n\nDiagnostics:\nbash scripts/dev.sh doctor\nRUST_LOG=debug bash scripts/dev.sh run\nCheck configuration if this persists."
}

fn no_plugins_footer_status() -> &'static str {
    "No plugins loaded"
}

fn no_plugins_footer_hints() -> Vec<(String, String)> {
    vec![
        ("?".to_string(), "Toggle help".to_string()),
        ("/".to_string(), "Global search".to_string()),
        (":".to_string(), "Command search".to_string()),
        ("q/Ctrl+C".to_string(), "Quit".to_string()),
    ]
}

fn format_command_help_line(key: &str, command: &rightclick::plugin::PluginCommand) -> String {
    if command.description.is_empty() {
        format!("  {}: {}", key, command.name)
    } else {
        format!("  {}: {} - {}", key, command.name, command.description)
    }
}

fn format_command_search_preview(command: &rightclick::plugin::PluginCommand) -> String {
    if command.description.is_empty() {
        format!("Shortcut: {}", command.key)
    } else {
        format!("Shortcut: {} | {}", command.key, command.description)
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
        assert!(lines.contains(&"  :: Command search".to_string()));
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
        assert!(global_index < plugin_index);
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
                "  ... more help hidden".to_string()
            ]
        );
    }

    #[test]
    fn test_visible_help_lines_handles_zero_height() {
        let lines = vec!["Help".to_string()];

        assert!(visible_help_lines(lines, 0).is_empty());
    }

    #[test]
    fn test_compact_help_overlay_text_fits_width() {
        for width in 0..=48 {
            assert!(compact_help_overlay_text(width).len() <= width as usize);
        }
        assert_eq!(compact_help_overlay_text(0), "");
        assert_eq!(compact_help_overlay_text(7), "?");
        assert_eq!(compact_help_overlay_text(12), "? Toggle");
        assert_eq!(compact_help_overlay_text(18), "? Toggle help");
        assert_eq!(compact_help_overlay_text(25), "? Toggle help  / Global");
        assert_eq!(
            compact_help_overlay_text(35),
            "? Toggle help  / Global  : Commands"
        );
        assert_eq!(
            compact_help_overlay_text(41),
            "? Toggle help  / Global  : Command search"
        );
        assert_eq!(
            compact_help_overlay_text(56),
            "? Toggle help  / Global search  : Command search  q Quit"
        );
    }

    #[test]
    fn test_help_overlay_renders_compact_text_in_small_area() {
        let app = App::new(
            Vec::new(),
            Theme::default(),
            PathBuf::from("/tmp/rightclick"),
        );
        let area = Rect::new(0, 0, 26, 3);
        let mut buf = ratatui::buffer::Buffer::empty(area);

        app.render_help_overlay(area, &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("? Toggle help"));
        assert!(content.contains("/ Global"));
    }

    #[test]
    fn test_help_overlay_renders_inside_offset_area_near_u16_max() {
        let app = App::new(
            Vec::new(),
            Theme::default(),
            PathBuf::from("/tmp/rightclick"),
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
        assert!(lines.contains(&"  :: Command search".to_string()));
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
            format_command_search_preview(&command),
            "Shortcut: r | Reload repository state"
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

        assert_eq!(format_command_search_preview(&command), "Shortcut: r");
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

        assert!(hints.contains(&("f".to_string(), "Refresh Intents".to_string())));
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
        let mut app = App::new(plugins, Theme::default(), PathBuf::from("/tmp/rightclick"));

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
        let mut app = App::new(plugins, Theme::default(), PathBuf::from("/tmp/rightclick"));

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
        let mut app = App::new(plugins, Theme::default(), PathBuf::from("/tmp/rightclick"));

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
        let message = no_plugins_empty_message();
        let hints = no_plugins_footer_hints();

        assert!(message.contains("No plugins loaded"));
        assert!(message.contains("?: Toggle help"));
        assert!(message.contains("/: Global search"));
        assert!(message.contains(":: Command search"));
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
        let app = App::new(plugins, Theme::default(), PathBuf::from("/tmp/rightclick"));

        let results = app.search_plugin_commands("target refresh", 10);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Target: Refresh Data");
        assert!(matches!(
            &results[0].kind,
            rightclick::search::SearchResultKind::Command { id } if id == "target:refresh"
        ));
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
        let mut app = App::new(plugins, Theme::default(), PathBuf::from("/tmp/rightclick"));

        app.search
            .set_results(vec![rightclick::search::SearchResult {
                kind: rightclick::search::SearchResultKind::Command {
                    id: "target:refresh".to_string(),
                },
                title: "Target: Refresh Target".to_string(),
                preview: "Shortcut: r | Reload target plugin data".to_string(),
                score: 100,
            }]);

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
