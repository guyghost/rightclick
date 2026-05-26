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
    plugin::{Plugin, PluginCommandError, PluginCommandExecution, PluginContext},
    plugins::{conversations, filebrowser, gitstatus, workers, workspace},
    search::{
        SearchOverlayAction, SearchOverlayState, SearchScope, render_search_overlay,
        search_conversations, search_files,
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
                if key.code == KeyCode::Char('c')
                    && key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL)
                {
                    self.should_quit = true;
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
                let active_is_workspace = active_plugin_id == "workspace";
                let active_is_gitstatus = active_plugin_id == "git-status";

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
                        KeyCode::Char('?') => {
                            self.show_help = !self.show_help;
                            return Ok(());
                        }
                        KeyCode::Char(c) if c.is_ascii_digit() => {
                            // Always use digits 1-9 for global plugin navigation
                            let idx = c.to_digit(10).unwrap() as usize;
                            if idx > 0 && idx <= self.plugins.len() {
                                self.switch_plugin(idx - 1);
                                return Ok(());
                            }
                        }
                        KeyCode::Tab
                            if (!active_is_workspace && !active_is_gitstatus)
                                || key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            self.next_plugin();
                            return Ok(());
                        }
                        KeyCode::BackTab
                            if (!active_is_workspace && !active_is_gitstatus)
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

        // Conversation search (synchronous fuzzy match)
        if matches!(scope, SearchScope::All | SearchScope::Conversations) {
            let conversations: Vec<(String, String)> = self
                .plugins
                .iter()
                .filter(|p| p.id() == "conversations")
                .flat_map(|p| {
                    p.commands()
                        .into_iter()
                        .map(|c| (c.name.to_string(), c.description.to_string()))
                })
                .collect();
            if !conversations.is_empty() {
                let conv_results = search_conversations(&query, &conversations, 20);
                all_results.extend(conv_results);
            }
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
                self.notifications.info(format!("Conversation: {}", id));
            }
            SearchResultKind::Command { id } => match self.execute_search_command(&id) {
                Ok(execution) => {
                    self.notifications
                        .info(format!("Command: {}", execution.command_name));
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
                let score = name_score.max(desc_score).max(id_score);
                if score == 0 {
                    continue;
                }

                results.push(SearchResult {
                    kind: SearchResultKind::Command {
                        id: format!("{}:{}", plugin.id(), command.id),
                    },
                    title: format!("{}: {}", plugin.name(), command.name),
                    preview: if command.description.is_empty() {
                        format!("Shortcut '{}'", command.key)
                    } else {
                        command.description.clone()
                    },
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
                let msg = Paragraph::new("No plugins loaded.");
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
        if self.plugins.is_empty() {
            return;
        }

        let tab_titles: Vec<String> = self
            .plugins
            .iter()
            .enumerate()
            .map(|(idx, p)| format!(" {} {} {} ", idx + 1, p.icon(), p.name()))
            .collect();

        let subtitle = self.work_dir.display().to_string();
        let header = Header::new("RightClick")
            .with_subtitle(subtitle)
            .with_tabs(tab_titles, self.active_plugin);
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
            let footer = Footer::new("No plugin loaded").with_hints(vec![
                ("q".to_string(), "Quit".to_string()),
                ("/".to_string(), "Search".to_string()),
            ]);
            footer.render(area, f.buffer_mut(), &self.theme);
        }
    }

    fn footer_hints(&self, plugin: &dyn Plugin) -> Vec<(String, String)> {
        let mut hints: Vec<(String, String)> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for command in plugin.commands() {
            let key = command.key.to_string();
            if matches!(key.as_str(), "j" | "k") {
                continue;
            }
            if seen.insert(key.clone()) {
                hints.push((key, command.name));
            }
            if hints.len() >= 4 {
                break;
            }
        }

        let tab_label = if plugin.id() == "git-status" {
            "Pane"
        } else {
            "Switch"
        };
        for (key, label) in [
            ("Tab", tab_label),
            ("1-9", "Go"),
            ("/", "Search"),
            ("q", "Quit"),
        ] {
            if seen.insert(key.to_string()) {
                hints.push((key.to_string(), label.to_string()));
            }
        }

        hints
    }

    fn render_help_overlay(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.width < 30 || area.height < 10 {
            return;
        }

        let width = area.width.saturating_mul(3).saturating_div(5).clamp(30, 80);
        let height = area
            .height
            .saturating_mul(2)
            .saturating_div(3)
            .clamp(10, 28);
        let popup = Rect::new(
            area.x + area.width.saturating_sub(width) / 2,
            area.y + area.height.saturating_sub(height) / 2,
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

        let Some(plugin) = self.plugins.get(self.active_plugin) else {
            return;
        };
        let status = plugin
            .status_line()
            .unwrap_or_else(|| format!("{} ready", plugin.name()));
        let lines = build_help_lines(plugin.name(), &plugin.commands(), &status);
        let rendered: Vec<Line> = lines
            .into_iter()
            .take(inner.height as usize)
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

fn build_help_lines(
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
            lines.push(format!("  {:<8} {}", key, command.name));
        }
    }

    lines.extend([
        String::new(),
        "Global shortcuts:".to_string(),
        "  /        Search files, commands, conversations".to_string(),
        "  ?        Toggle this help".to_string(),
        "  Tab      Switch plugin or pane".to_string(),
        "  1-9      Jump to plugin".to_string(),
        "  q        Quit".to_string(),
    ]);

    lines
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
    fn test_build_help_lines_includes_plugin_and_global_shortcuts() {
        let commands = vec![rightclick::plugin::PluginCommand::with_context(
            "refresh",
            "Refresh",
            'r',
            rightclick::keymap::FocusContext::Global,
        )];

        let lines = build_help_lines("Git Status", &commands, "3 files changed");

        assert!(lines.iter().any(|line| line.contains("Git Status")));
        assert!(lines.iter().any(|line| line.contains("r")));
        assert!(lines.iter().any(|line| line.contains("Refresh")));
        assert!(lines.iter().any(|line| line.contains("/")));
        assert!(lines.iter().any(|line| line.contains("3 files changed")));
    }
}
