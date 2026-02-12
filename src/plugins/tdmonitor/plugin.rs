//! TD Monitor Plugin
//!
//! This module implements the TD Monitor plugin for RightClick, providing
//! a TUI interface for viewing and interacting with TD (Task Driver) tasks.
//!
//! TD is a task management system that stores tasks in a SQLite database
//! at `.td/db.sqlite3` in the project root.

use std::path::PathBuf;
use std::process::Command as ProcessCommand;

use anyhow::{Context, Result};
use chrono::DateTime;
use ratatui::{buffer::Buffer, layout::Rect};
use rusqlite::{params, Connection};

use crate::core::models::Theme;
use crate::event::Event;
use crate::keymap::registry::KeyBindingRegistry;
use crate::keymap::{Action, FocusContext};

use super::render::{render_not_available, render_td_monitor, render_status_info};
use super::state::{ActivityLogEntry, PluginState, Priority, Task, TaskStatus, ViewMode};

/// A command that can be executed by the plugin
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// No operation
    None,
    /// Refresh the view
    Refresh,
    /// Switch to a different view mode
    SwitchMode(ViewMode),
    /// Create a new task
    CreateTask,
    /// Edit the selected task
    EditTask(String),
    /// Update task status
    UpdateStatus(String, TaskStatus),
    /// Review the selected task
    ReviewTask(String),
    /// Set the focused task
    SetFocus(String),
    /// Clear the focused task
    ClearFocus,
    /// Execute a TD command
    TDExec(Vec<String>),
    /// Emit an event
    EmitEvent(Event),
}

impl Default for Command {
    fn default() -> Self {
        Self::None
    }
}

/// Plugin command for the command palette
#[derive(Debug, Clone)]
pub struct PluginCommand {
    /// Command ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Keyboard shortcut character
    pub key: char,
}

impl PluginCommand {
    /// Create a new plugin command
    pub fn new(id: impl Into<String>, name: impl Into<String>, key: char) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            key,
        }
    }
}

/// Context passed to TD monitor plugin during initialization
#[derive(Debug, Clone)]
pub struct TDMonitorPluginContext {
    /// Project root directory
    pub project_root: PathBuf,
    /// Current configuration
    pub config: crate::core::models::Config,
}

/// The main TD Monitor plugin struct
#[derive(Debug)]
pub struct TDMonitorPlugin {
    /// Plugin state
    state: PluginState,
    /// Working directory (project root)
    work_dir: PathBuf,
    /// Whether this plugin is focused
    focused: bool,
    /// Whether TD binary is available
    td_available: bool,
    /// Path to TD database
    db_path: Option<PathBuf>,
    /// Key binding registry
    #[allow(dead_code)]
    key_bindings: KeyBindingRegistry,
}

impl TDMonitorPlugin {
    /// Create a new TD Monitor plugin
    pub fn new() -> Self {
        let mut key_bindings = KeyBindingRegistry::new();
        Self::register_default_bindings(&mut key_bindings);

        Self {
            state: PluginState::new(),
            work_dir: PathBuf::new(),
            focused: false,
            td_available: false,
            db_path: None,
            key_bindings,
        }
    }

    /// Register default key bindings
    fn register_default_bindings(registry: &mut KeyBindingRegistry) {
        // Navigation
        registry.bind("j", Action::NavigateDown, FocusContext::TDMonitor);
        registry.bind("k", Action::NavigateUp, FocusContext::TDMonitor);
        registry.bind("g", Action::NavigateLeft, FocusContext::TDMonitor);
        registry.bind("G", Action::NavigateRight, FocusContext::TDMonitor);

        // View switching
        registry.bind("v", Action::Toggle, FocusContext::TDMonitor);
        registry.bind("t", Action::SwitchPlugin("td-monitor-board".to_string()), FocusContext::TDMonitor);

        // Task operations
        registry.bind("n", Action::Create, FocusContext::TDMonitor);
        registry.bind("e", Action::Edit, FocusContext::TDMonitor);
        registry.bind("r", Action::Custom(Box::new("review".to_string())), FocusContext::TDMonitor);
        registry.bind("f", Action::Filter, FocusContext::TDMonitor);
        registry.bind("c", Action::Custom(Box::new("cycle-status".to_string())), FocusContext::TDMonitor);
        registry.bind("enter", Action::Select, FocusContext::TDMonitor);
        registry.bind("space", Action::Toggle, FocusContext::TDMonitor);

        // Refresh
        registry.bind("R", Action::Refresh, FocusContext::TDMonitor);
    }

    /// Get the plugin ID
    pub fn id(&self) -> &str {
        "td-monitor"
    }

    /// Get the plugin name
    pub fn name(&self) -> &str {
        "td"
    }

    /// Get the plugin icon
    pub fn icon(&self) -> char {
        'T'
    }

    /// Check if TD binary is available
    fn check_td_available() -> bool {
        ProcessCommand::new("td")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Find the TD database path
    fn find_db_path(work_dir: &PathBuf) -> Option<PathBuf> {
        // Check for .td/db.sqlite3 in work directory
        let db_path = work_dir.join(".td").join("db.sqlite3");
        if db_path.exists() {
            return Some(db_path);
        }

        // Check for TD_HOME environment variable
        if let Ok(td_home) = std::env::var("TD_HOME") {
            let db_path = PathBuf::from(td_home).join("db.sqlite3");
            if db_path.exists() {
                return Some(db_path);
            }
        }

        // Check in home directory
        if let Some(home) = dirs::home_dir() {
            let db_path = home.join(".td").join("db.sqlite3");
            if db_path.exists() {
                return Some(db_path);
            }
        }

        None
    }

    /// Initialize the plugin
    pub async fn init_with_context(&mut self, ctx: &TDMonitorPluginContext) -> Result<()> {
        self.work_dir = ctx.project_root.clone();

        // Check if TD is available
        self.td_available = Self::check_td_available();

        // Find database path
        self.db_path = Self::find_db_path(&self.work_dir);

        // Initial refresh if available
        if self.td_available || self.db_path.is_some() {
            self.refresh().await?;
        }

        Ok(())
    }

    /// Shutdown the plugin
    pub async fn shutdown(&mut self) -> Result<()> {
        // Cleanup if needed
        Ok(())
    }

    /// Refresh the plugin state from the database
    pub async fn refresh(&mut self) -> Result<()> {
        self.state.set_loading(true);
        self.state.set_error(None);

        let result = self.load_tasks_from_db().await;

        self.state.set_loading(false);

        match result {
            Ok(()) => {
                // Load activity log after tasks
                if let Err(e) = self.load_activity_log().await {
                    tracing::warn!("Failed to load activity log: {}", e);
                }
                Ok(())
            }
            Err(e) => {
                self.state.set_error(Some(e.to_string()));
                Err(e)
            }
        }
    }

    /// Load tasks from the TD database
    async fn load_tasks_from_db(&mut self) -> Result<()> {
        let db_path = match &self.db_path {
            Some(path) => path.clone(),
            None => {
                // Try to find db again
                if let Some(path) = Self::find_db_path(&self.work_dir) {
                    self.db_path = Some(path.clone());
                    path
                } else {
                    return Err(anyhow::anyhow!("TD database not found"));
                }
            }
        };

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open TD database at {:?}", db_path))?;

        // Query tasks from the database
        // The schema may vary, this is a common TD schema
        let mut stmt = conn.prepare(
            "SELECT 
                id, title, description, status, priority, 
                created_at, updated_at, tags
             FROM tasks
             ORDER BY updated_at DESC"
        )?;

        let tasks = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let description: Option<String> = row.get(2)?;
            let status_str: String = row.get(3).unwrap_or_else(|_| "todo".to_string());
            let priority_str: String = row.get(4).unwrap_or_else(|_| "medium".to_string());
            let created_at_str: String = row.get(5)?;
            let updated_at_str: String = row.get(6)?;
            let tags_str: Option<String> = row.get(7)?;

            let status = TaskStatus::from_db(&status_str);
            let priority = Priority::from_db(&priority_str);

            // Parse timestamps (TD typically uses ISO 8601 or Unix timestamp)
            let created_at = parse_datetime(&created_at_str).unwrap_or_else(|_| chrono::Utc::now());
            let updated_at = parse_datetime(&updated_at_str).unwrap_or_else(|_| chrono::Utc::now());

            // Parse tags (comma-separated)
            let tags = tags_str
                .map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
                .unwrap_or_default();

            Ok(Task {
                id,
                title,
                description,
                status,
                priority,
                created_at,
                updated_at,
                tags,
            })
        })?;

        self.state.tasks.clear();
        for task in tasks {
            match task {
                Ok(t) => self.state.tasks.push(t),
                Err(e) => tracing::warn!("Failed to parse task: {}", e),
            }
        }

        // Load current focus from database
        self.load_focused_task(&conn)?;

        Ok(())
    }

    /// Load the currently focused task
    fn load_focused_task(&mut self, conn: &Connection) -> Result<()> {
        // TD may have a separate table for current focus or a flag on tasks
        let result: Result<String, _> = conn.query_row(
            "SELECT task_id FROM current_focus LIMIT 1",
            [],
            |row| row.get(0),
        );

        if let Ok(task_id) = result {
            if let Some(task) = self.state.tasks.iter().find(|t| t.id == task_id).cloned() {
                self.state.set_focused_task(Some(task));
            }
        }

        Ok(())
    }

    /// Load activity log from the database
    async fn load_activity_log(&mut self) -> Result<()> {
        let db_path = match &self.db_path {
            Some(path) => path.clone(),
            None => return Ok(()),
        };

        let conn = Connection::open(&db_path)?;

        // Query activity log
        let mut stmt = conn.prepare(
            "SELECT 
                timestamp, task_id, activity_type, description
             FROM activity_log
             ORDER BY timestamp DESC
             LIMIT 50"
        )?;

        let entries = stmt.query_map([], |row| {
            let timestamp_str: String = row.get(0)?;
            let task_id: Option<String> = row.get(1)?;
            let activity_type: String = row.get(2)?;
            let description: String = row.get(3)?;

            let timestamp = parse_datetime(&timestamp_str).unwrap_or_else(|_| chrono::Utc::now());

            Ok(ActivityLogEntry {
                timestamp,
                task_id,
                activity_type,
                description,
            })
        })?;

        self.state.activity_log.clear();
        for entry in entries {
            match entry {
                Ok(e) => self.state.activity_log.push(e),
                Err(e) => tracing::warn!("Failed to parse activity entry: {}", e),
            }
        }

        Ok(())
    }

    /// Handle keyboard input events
    pub fn handle_event_internal(&mut self, event: Event) -> Vec<Command> {
        let mut commands = Vec::new();

        match event {
            Event::RefreshNeeded => {
                commands.push(Command::Refresh);
            }
            Event::Key { code, modifiers } => {
                if !modifiers.ctrl && !modifiers.alt {
                    match code.as_str() {
                        "j" | "Down" => {
                            self.state.select_next();
                        }
                        "k" | "Up" => {
                            self.state.select_prev();
                        }
                        "r" => {
                            commands.push(Command::Refresh);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        commands
    }

    /// Handle an action from key bindings
    pub fn handle_action(&mut self, action: &Action) -> Vec<Command> {
        let mut commands = Vec::new();

        match action {
            Action::NavigateDown => {
                self.state.select_next();
            }
            Action::NavigateUp => {
                self.state.select_prev();
            }
            Action::NavigateLeft => {
                self.state.select_first();
            }
            Action::NavigateRight => {
                self.state.select_last();
            }
            Action::Toggle => {
                // Toggle view mode
                self.state.view_mode = self.state.view_mode.toggle();
                commands.push(Command::SwitchMode(self.state.view_mode));
            }
            Action::Filter => {
                // Start filter input
                self.state.start_filter_input();
            }
            Action::Create => {
                // Create new task
                commands.push(Command::CreateTask);
            }
            Action::Edit => {
                // Edit selected task
                if let Some(task) = self.state.selected_task() {
                    commands.push(Command::EditTask(task.id.clone()));
                }
            }
            Action::Select => {
                // Set as focused task
                if let Some(task) = self.state.selected_task() {
                    commands.push(Command::SetFocus(task.id.clone()));
                }
            }
            Action::Refresh => {
                commands.push(Command::Refresh);
            }
            Action::Custom(data) => {
                if let Some(s) = data.downcast_ref::<String>() {
                    match s.as_str() {
                        "review" => {
                            if let Some(task) = self.state.selected_task() {
                                commands.push(Command::ReviewTask(task.id.clone()));
                            }
                        }
                        "cycle-status" => {
                            if let Some(task) = self.state.selected_task() {
                                let next_status = match task.status {
                                    TaskStatus::Todo => TaskStatus::InProgress,
                                    TaskStatus::InProgress => TaskStatus::Review,
                                    TaskStatus::Review => TaskStatus::Done,
                                    TaskStatus::Done => TaskStatus::Todo,
                                };
                                commands.push(Command::UpdateStatus(task.id.clone(), next_status));
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        commands
    }

    /// Handle filter input character
    pub fn handle_filter_input(&mut self, c: char) {
        if self.state.filter_input_active {
            self.state.filter_input.push(c);
        }
    }

    /// Handle filter input backspace
    pub fn handle_filter_backspace(&mut self) {
        if self.state.filter_input_active {
            self.state.filter_input.pop();
        }
    }

    /// Submit filter input
    pub fn submit_filter(&mut self) {
        self.state.apply_filter_input();
    }

    /// Cancel filter input
    pub fn cancel_filter(&mut self) {
        self.state.clear_filter();
    }

    /// Check if in filter input mode
    pub fn is_filter_input_active(&self) -> bool {
        self.state.filter_input_active
    }

    /// Render the plugin UI
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if !self.td_available && self.db_path.is_none() {
            render_not_available(area, buf, theme);
        } else {
            render_td_monitor(&self.state, self.focused, area, buf, theme);
        }
    }

    /// Get available commands for the command palette
    pub fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::new("new", "New Task", 'n'),
            PluginCommand::new("edit", "Edit Task", 'e'),
            PluginCommand::new("review", "Review", 'r'),
            PluginCommand::new("filter", "Filter", '/'),
            PluginCommand::new("toggle-view", "Toggle View", 'v'),
            PluginCommand::new("refresh", "Refresh", 'R'),
        ]
    }

    /// Set focus state
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Check if plugin is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Get the current focus context
    pub fn focus_context(&self) -> FocusContext {
        FocusContext::TDMonitor
    }

    /// Get status info for the footer
    pub fn status_info(&self, theme: &Theme) -> Vec<ratatui::text::Span<'_>> {
        render_status_info(&self.state, theme)
    }

    /// Get a reference to the plugin state
    pub fn state(&self) -> &PluginState {
        &self.state
    }

    /// Get a mutable reference to the plugin state
    pub fn state_mut(&mut self) -> &mut PluginState {
        &mut self.state
    }

    /// Set view mode
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.state.view_mode = mode;
    }

    /// Get the current view mode
    pub fn view_mode(&self) -> ViewMode {
        self.state.view_mode
    }

    /// Check if TD is available
    pub fn is_td_available(&self) -> bool {
        self.td_available || self.db_path.is_some()
    }

    /// Get the path to the TD database
    pub fn db_path(&self) -> Option<&PathBuf> {
        self.db_path.as_ref()
    }

    /// Create a new task using TD CLI
    pub async fn create_task(&mut self, title: &str) -> Result<()> {
        if !self.td_available {
            return Err(anyhow::anyhow!("TD CLI not available"));
        }

        let output = ProcessCommand::new("td")
            .arg("create")
            .arg(title)
            .current_dir(&self.work_dir)
            .output()
            .context("Failed to execute td create")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("TD create failed: {}", stderr));
        }

        self.refresh().await
    }

    /// Update task status using TD CLI or direct database update
    pub async fn update_task_status(&mut self, task_id: &str, status: TaskStatus) -> Result<()> {
        // Try TD CLI first
        if self.td_available {
            let status_str = status.as_str();
            let output = ProcessCommand::new("td")
                .arg("update")
                .arg(task_id)
                .arg("--status")
                .arg(status_str)
                .current_dir(&self.work_dir)
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    return self.refresh().await;
                }
            }
        }

        // Fallback to direct database update
        if let Some(ref db_path) = self.db_path {
            let conn = Connection::open(db_path)?;
            conn.execute(
                "UPDATE tasks SET status = ?, updated_at = ? WHERE id = ?",
                params![
                    status.as_str(),
                    chrono::Utc::now().to_rfc3339(),
                    task_id
                ],
            )?;
            return self.refresh().await;
        }

        Err(anyhow::anyhow!("Cannot update task: no database or CLI available"))
    }

    /// Set the focused task
    pub async fn set_focused_task(&mut self, task_id: Option<&str>) -> Result<()> {
        // Update local state
        if let Some(id) = task_id {
            if let Some(task) = self.state.tasks.iter().find(|t| t.id == id).cloned() {
                self.state.set_focused_task(Some(task));
            }
        } else {
            self.state.set_focused_task(None);
        }

        // Try to persist to database
        if let Some(ref db_path) = self.db_path {
            let conn = Connection::open(db_path)?;
            
            // Create current_focus table if it doesn't exist
            conn.execute(
                "CREATE TABLE IF NOT EXISTS current_focus (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    task_id TEXT
                )",
                [],
            )?;

            // Update or insert focus
            conn.execute(
                "INSERT INTO current_focus (id, task_id) VALUES (1, ?)
                 ON CONFLICT(id) DO UPDATE SET task_id = excluded.task_id",
                params![task_id],
            )?;
        }

        Ok(())
    }

    /// Get the currently focused task
    pub fn focused_task(&self) -> Option<&Task> {
        self.state.current_focus.as_ref()
    }
}

impl Default for TDMonitorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a datetime string from various formats
fn parse_datetime(s: &str) -> Result<DateTime<chrono::Utc>> {
    // Try RFC 3339 / ISO 8601 first
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&chrono::Utc));
    }

    // Try other common formats
    let formats = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d",
    ];

    for format in &formats {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, format) {
            return Ok(DateTime::from_naive_utc_and_offset(naive, chrono::Utc));
        }
    }

    // Try parsing as Unix timestamp
    if let Ok(ts) = s.parse::<i64>() {
        if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
            return Ok(dt);
        }
    }

    Err(anyhow::anyhow!("Unable to parse datetime: {}", s))
}

// ============================================================================
// Plugin Trait Implementation
// ============================================================================

use crate::plugin::{Plugin, PluginContext as PluginCtx, Command as PluginCmd};
use async_trait::async_trait;

#[async_trait]
impl Plugin for TDMonitorPlugin {
    fn id(&self) -> &str {
        "td-monitor"
    }

    fn name(&self) -> &str {
        "td"
    }

    fn icon(&self) -> char {
        'T'
    }

    async fn init(&mut self, ctx: &PluginCtx) -> anyhow::Result<()> {
        self.work_dir = ctx.project_root.clone();
        self.td_available = Self::check_td_available();
        self.db_path = Self::find_db_path(&self.work_dir);
        
        if self.td_available || self.db_path.is_some() {
            let _ = self.refresh().await;
        }
        
        Ok(())
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn handle_event(&mut self, event: crate::event::Event) -> Vec<PluginCmd> {
        let commands = self.handle_event_internal(event);
        commands.into_iter().map(|_cmd| PluginCmd::new("td-monitor", "action")).collect()
    }

    fn render(&self, area: Rect, buf: &mut Buffer, theme: &crate::core::models::Theme) {
        if !self.td_available && self.db_path.is_none() {
            render_not_available(area, buf, theme);
        } else {
            render_td_monitor(&self.state, self.focused, area, buf, theme);
        }
    }

    fn is_focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn commands(&self) -> Vec<crate::plugin::PluginCommand> {
        vec![
            crate::plugin::PluginCommand::with_context("new", "New Task", 'n', crate::keymap::FocusContext::TDMonitor),
            crate::plugin::PluginCommand::with_context("edit", "Edit Task", 'e', crate::keymap::FocusContext::TDMonitor),
            crate::plugin::PluginCommand::with_context("review", "Review", 'r', crate::keymap::FocusContext::TDMonitor),
            crate::plugin::PluginCommand::with_context("filter", "Filter", '/', crate::keymap::FocusContext::TDMonitor),
            crate::plugin::PluginCommand::with_context("toggle-view", "Toggle View", 'v', crate::keymap::FocusContext::TDMonitor),
            crate::plugin::PluginCommand::with_context("refresh", "Refresh", 'R', crate::keymap::FocusContext::TDMonitor),
        ]
    }

    fn focus_context(&self) -> crate::keymap::FocusContext {
        crate::keymap::FocusContext::TDMonitor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_id() {
        let plugin = TDMonitorPlugin::new();
        assert_eq!(plugin.id(), "td-monitor");
        assert_eq!(plugin.name(), "td");
        assert_eq!(plugin.icon(), 'T');
    }

    #[test]
    fn test_plugin_commands() {
        let plugin = TDMonitorPlugin::new();
        let commands = plugin.commands();
        assert!(!commands.is_empty());
        assert!(commands.iter().any(|c| c.id == "new"));
        assert!(commands.iter().any(|c| c.id == "edit"));
    }

    #[test]
    fn test_handle_action_navigation() {
        let mut plugin = TDMonitorPlugin::new();

        // Test navigate down
        let cmds = plugin.handle_action(&Action::NavigateDown);
        assert!(cmds.is_empty());

        // Test toggle view
        let cmds = plugin.handle_action(&Action::Toggle);
        assert!(matches!(cmds[0], Command::SwitchMode(ViewMode::Board)));
    }

    #[test]
    fn test_view_mode() {
        let mut plugin = TDMonitorPlugin::new();
        assert!(matches!(plugin.view_mode(), ViewMode::List));

        plugin.set_view_mode(ViewMode::Board);
        assert!(matches!(plugin.view_mode(), ViewMode::Board));
    }

    #[test]
    fn test_focus_context() {
        let plugin = TDMonitorPlugin::new();
        assert_eq!(plugin.focus_context(), FocusContext::TDMonitor);
    }

    #[test]
    fn test_parse_datetime() {
        // RFC 3339
        let dt = parse_datetime("2024-01-15T10:30:00Z").unwrap();
        assert_eq!(dt.year(), 2024);

        // Simple format
        let dt = parse_datetime("2024-01-15 10:30:00").unwrap();
        assert_eq!(dt.year(), 2024);
    }
}
