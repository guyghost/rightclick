//! RightClick - A TUI dashboard for AI coding agents

use std::io;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Tabs},
    Terminal,
};
use tracing::{info, warn};

use rightclick::{
    adapters::create_default_registry,
    config,
    core::models::Theme,
    plugin::{Plugin, PluginContext},
    plugins::{conversations, filebrowser, gitstatus, tdmonitor, workspace},
    state,
    theme::{self, resolve_theme},
};

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
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

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
    let adapters = adapter_registry.detect_all(&project_root).await.unwrap_or_default();
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

    // TD Monitor plugin
    if config.plugins.td_monitor.enabled {
        info!("Loading td-monitor plugin");
        let mut plugin = tdmonitor::TDMonitorPlugin::new();
        if let Err(e) = plugin.init(&plugin_ctx).await {
            warn!("Failed to init tdmonitor plugin: {}", e);
        } else {
            plugins.push(Box::new(plugin));
        }
    }

    // Workspace plugin
    if config.plugins.workspace.enabled {
        info!("Loading workspace plugin");
        let mut plugin = workspace::WorkspacePlugin::new();
        if let Err(e) = plugin.init(&plugin_ctx).await {
            warn!("Failed to init workspace plugin: {}", e);
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
    let mut app = App::new(plugins, theme);

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
}

impl App {
    fn new(plugins: Vec<Box<dyn Plugin>>, theme: Theme) -> Self {
        Self {
            plugins,
            theme,
            active_plugin: 0,
            should_quit: false,
        }
    }

    fn handle_event(&mut self, event: crossterm::event::Event) -> Result<()> {
        use crossterm::event::{Event as CEvent, KeyCode, KeyEventKind};

        match event {
            CEvent::Key(key) if key.kind == KeyEventKind::Press => {
                // Global keys first
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        self.should_quit = true;
                        return Ok(());
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        let idx = c.to_digit(10).unwrap() as usize;
                        if idx > 0 && idx <= self.plugins.len() {
                            self.switch_plugin(idx - 1);
                            return Ok(());
                        }
                    }
                    KeyCode::Tab => {
                        self.next_plugin();
                        return Ok(());
                    }
                    KeyCode::BackTab => {
                        self.prev_plugin();
                        return Ok(());
                    }
                    _ => {}
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
                        ctrl: key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL),
                        alt: key.modifiers.contains(crossterm::event::KeyModifiers::ALT),
                        shift: key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT),
                    };
                    let event = rightclick::event::Event::Key { 
                        code: key_code, 
                        modifiers 
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

    fn render(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        terminal.draw(|f| {
            let size = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2), // Header
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
                let msg = Paragraph::new("No plugins loaded.")
                    .alignment(Alignment::Center);
                f.render_widget(msg, chunks[1]);
            }

            // Footer
            self.render_footer(f, chunks[2]);
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
            .map(|p| format!(" [{}] {} ", p.icon(), p.name()))
            .collect();

        let tabs = Tabs::new(tab_titles)
            .block(Block::default().borders(Borders::BOTTOM))
            .select(self.active_plugin)
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White));
        f.render_widget(tabs, area);
    }

    fn render_footer(&self, f: &mut ratatui::Frame, area: Rect) {
        let hints = if let Some(plugin) = self.plugins.get(self.active_plugin) {
            let commands = plugin.commands();
            let cmd_hints: Vec<String> = commands
                .iter()
                .take(4)
                .map(|c| format!("{}: {}", c.key, c.name))
                .collect();
            format!(" q:quit | Tab:switch | {} ", cmd_hints.join(" | "))
        } else {
            " q:quit | Tab:switch ".to_string()
        };

        let footer = Paragraph::new(hints)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(footer, area);
    }
}

/// Main application loop
async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    let tick_rate = std::time::Duration::from_millis(250);

    loop {
        // Draw UI
        app.render(terminal)?;

        // Handle events with timeout
        if crossterm::event::poll(tick_rate)? {
            let event = crossterm::event::read()?;
            app.handle_event(event)?;
        }

        // Update active plugin for async operations
        if let Some(plugin) = app.plugins.get_mut(app.active_plugin) {
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
}
