//! Workspace Plugin
//!
//! This module implements the Workspace plugin for RightClick, providing
//! a TUI interface for managing git worktrees and development sessions.

use std::path::PathBuf;

use anyhow::Result;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::core::models::Theme;
use crate::event::Event;
use crate::keymap::registry::KeyBindingRegistry;
use crate::keymap::{Action, FocusContext};

use super::render::{render_workspace, render_workspace_status};
use super::state::{FocusPane, ModalState, PluginState, PreviewTab, ViewMode};
use super::worktree::{AgentLauncher, TmuxManager, WorktreeManager};

/// A command that can be executed by the plugin
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// No operation
    None,
    /// Refresh the view
    Refresh,
    /// Switch to a different view mode
    SwitchMode(ViewMode),
    /// Switch focus to a different pane
    SwitchFocus(FocusPane),
    /// Create a new worktree
    CreateWorktree {
        name: String,
        branch: Option<String>,
    },
    /// Delete a worktree
    DeleteWorktree(PathBuf),
    /// Link a task to the selected worktree
    LinkTask { worktree: String, task_id: String },
    /// Unlink task from the selected worktree
    UnlinkTask(String),
    /// Launch AI agent
    LaunchAgent {
        worktree: String,
        task: Option<String>,
    },
    /// Enter interactive mode for a worktree
    EnterInteractive(PathBuf),
    /// Open merge workflow
    OpenMergeDialog(String),
    /// Show diff for selected worktree
    ShowDiff,
    /// Show task details
    ShowTask,
    /// Open tmux session
    OpenTmuxSession(String),
    /// Execute a shell command
    ShellExec(Vec<String>),
    /// Emit an event
    EmitEvent(Event),
}

impl Default for Command {
    fn default() -> Self {
        Self::None
    }
}

/// Context passed to workspace plugin during initialization
#[derive(Debug, Clone)]
pub struct WorkspacePluginContext {
    /// Project root directory
    pub project_root: PathBuf,
    /// Current configuration
    pub config: crate::core::models::Config,
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

/// The main Workspace plugin struct
#[derive(Debug)]
pub struct WorkspacePlugin {
    /// Plugin state
    state: PluginState,
    /// Repository path (main worktree)
    repo_path: PathBuf,
    /// Whether this plugin is focused
    focused: bool,
    /// Which pane has focus
    focus_pane: FocusPane,
    /// Key binding registry
    #[allow(dead_code)]
    key_bindings: KeyBindingRegistry,
    /// Configuration
    config: crate::core::models::WorkspacePluginConfig,
    /// Worktree manager
    worktree_manager: Option<WorktreeManager>,
    /// Flag to indicate preview content needs update
    pending_preview_update: bool,
}

impl WorkspacePlugin {
    /// Create a new Workspace plugin
    pub fn new() -> Self {
        let mut key_bindings = KeyBindingRegistry::new();
        Self::register_default_bindings(&mut key_bindings);

        Self {
            state: PluginState::new(),
            repo_path: PathBuf::new(),
            focused: false,
            focus_pane: FocusPane::default(),
            key_bindings,
            config: crate::core::models::WorkspacePluginConfig::default(),
            worktree_manager: None,
            pending_preview_update: false,
        }
    }

    /// Create a new plugin with custom configuration
    pub fn with_config(config: crate::core::models::WorkspacePluginConfig) -> Self {
        let mut plugin = Self::new();
        plugin.config = config;
        plugin
    }

    /// Register default key bindings
    fn register_default_bindings(registry: &mut KeyBindingRegistry) {
        // Navigation
        registry.bind("j", Action::NavigateDown, FocusContext::Workspace);
        registry.bind("k", Action::NavigateUp, FocusContext::Workspace);
        registry.bind("h", Action::NavigateLeft, FocusContext::Workspace);
        registry.bind("l", Action::NavigateRight, FocusContext::Workspace);
        registry.bind("tab", Action::Toggle, FocusContext::Workspace);

        // View modes
        registry.bind("v", Action::SwitchView, FocusContext::Workspace);

        // Worktree operations
        registry.bind("n", Action::NewFile, FocusContext::Workspace); // Create worktree
        registry.bind("D", Action::Delete, FocusContext::Workspace); // Delete worktree
        registry.bind("T", Action::LinkTask, FocusContext::Workspace); // Link/unlink task (shift+t)
        registry.bind("a", Action::LaunchAgent, FocusContext::Workspace); // Launch AI agent
        registry.bind("enter", Action::Enter, FocusContext::Workspace); // Enter interactive
        registry.bind("m", Action::Merge, FocusContext::Workspace); // Merge workflow
        registry.bind("r", Action::Refresh, FocusContext::Workspace); // Refresh

        // Note: Preview tab switching uses [ and ] keys (handled in handle_event)
        // to avoid conflict with global navigation keys 1-5

        // Pane switching
        registry.bind("left", Action::NavigateLeft, FocusContext::Workspace);
        registry.bind("right", Action::NavigateRight, FocusContext::Workspace);
    }

    /// Get the plugin ID
    pub fn id(&self) -> &str {
        "workspace"
    }

    /// Get the plugin name
    pub fn name(&self) -> &str {
        "workspace"
    }

    /// Get the plugin icon
    pub fn icon(&self) -> char {
        'W'
    }

    /// Initialize the plugin
    pub async fn init_with_context(&mut self, ctx: &WorkspacePluginContext) -> Result<()> {
        self.repo_path = ctx.project_root.clone();
        self.config = ctx.config.plugins.workspace.clone();

        // Check if this is a git repository
        let git_dir = self.repo_path.join(".git");
        if !git_dir.exists() {
            return Err(anyhow::anyhow!(
                "Not a git repository: {}",
                self.repo_path.display()
            ));
        }

        // Initialize worktree manager
        self.worktree_manager = Some(WorktreeManager::new(self.repo_path.clone()));

        // Initial refresh
        self.refresh().await
    }

    /// Shutdown the plugin
    pub async fn shutdown(&mut self) -> Result<()> {
        // Cleanup if needed
        Ok(())
    }

    /// Refresh the plugin state
    pub async fn refresh(&mut self) -> Result<()> {
        if let Some(ref manager) = self.worktree_manager {
            // Load worktrees
            let worktrees = manager.list_worktrees().await?;
            self.state.worktrees = worktrees;

            // Ensure selection is valid
            if let Some(selected) = self.state.selected {
                if selected >= self.state.worktrees.len() {
                    self.state.selected = if self.state.worktrees.is_empty() {
                        None
                    } else {
                        Some(self.state.worktrees.len() - 1)
                    };
                }
            } else if !self.state.worktrees.is_empty() {
                self.state.selected = Some(0);
            }

            // Load shell sessions
            self.state.shells = TmuxManager::list_sessions().await.unwrap_or_default();

            // Update preview content
            self.update_preview().await?;
        }

        Ok(())
    }

    /// Update preview content based on selection and active tab
    async fn update_preview(&mut self) -> Result<()> {
        match self.state.preview_tab {
            PreviewTab::Diff => {
                self.load_selected_diff().await?;
            }
            PreviewTab::Task => {
                self.load_task_details().await?;
            }
            PreviewTab::Output => {
                // Output is updated by commands
            }
        }
        Ok(())
    }

    /// Load diff for selected worktree
    async fn load_selected_diff(&mut self) -> Result<()> {
        if let Some(ref manager) = self.worktree_manager {
            if let Some(worktree) = self.state.selected_worktree() {
                let diff = manager.get_worktree_diff(&worktree.path).await?;
                self.state.set_diff(diff);
            }
        }
        Ok(())
    }

    /// Load task details for selected worktree
    async fn load_task_details(&mut self) -> Result<()> {
        if let Some(worktree) = self.state.selected_worktree() {
            if let Some(ref task_id) = worktree.linked_task {
                // In a real implementation, this would fetch from TD API
                let content = format!(
                    "Task: {}\n\nStatus: In Progress\n\nThis is a placeholder for task details that would be fetched from the TD (Task Driver) system.",
                    task_id
                );
                self.state.set_task_content(content);
            } else {
                self.state.task_content = None;
            }
        }
        Ok(())
    }

    /// Handle keyboard input events
    pub fn handle_event_internal(&mut self, event: Event) -> Vec<Command> {
        let mut commands = Vec::new();

        match event {
            Event::GitChanged | Event::RefreshNeeded => {
                commands.push(Command::Refresh);
            }
            Event::Key { code, modifiers } => {
                if modifiers.ctrl {
                    // Handle Ctrl+key combinations
                    match code.as_str() {
                        "d" => {
                            // Page down in preview
                        }
                        "u" => {
                            // Page up in preview
                        }
                        _ => {}
                    }
                } else if !modifiers.alt {
                    // Handle simple key presses
                    match code.as_str() {
                        "j" | "Down" => {
                            if self.focus_pane == FocusPane::Sidebar {
                                self.state.select_next();
                                commands.push(Command::ShowDiff);
                            }
                        }
                        "k" | "Up" => {
                            if self.focus_pane == FocusPane::Sidebar {
                                self.state.select_prev();
                                commands.push(Command::ShowDiff);
                            }
                        }
                        "h" | "Left" => {
                            if self.focus_pane == FocusPane::Preview {
                                self.focus_pane = FocusPane::Sidebar;
                                commands.push(Command::SwitchFocus(FocusPane::Sidebar));
                            }
                        }
                        "l" | "Right" => {
                            if self.focus_pane == FocusPane::Sidebar {
                                self.focus_pane = FocusPane::Preview;
                                commands.push(Command::SwitchFocus(FocusPane::Preview));
                            }
                        }
                        "Tab" => {
                            self.focus_pane = match self.focus_pane {
                                FocusPane::Sidebar => FocusPane::Preview,
                                FocusPane::Preview => FocusPane::Sidebar,
                            };
                            commands.push(Command::SwitchFocus(self.focus_pane));
                        }
                        "v" => {
                            // Toggle view mode
                            let new_mode = match self.state.view_mode {
                                ViewMode::List => ViewMode::Kanban,
                                ViewMode::Kanban => ViewMode::List,
                                ViewMode::Interactive => ViewMode::List,
                            };
                            self.state.view_mode = new_mode;
                            commands.push(Command::SwitchMode(new_mode));
                        }
                        "]" => {
                            // Cycle to next preview tab (Output -> Diff -> Task -> Output)
                            let new_tab = match self.state.preview_tab {
                                PreviewTab::Output => PreviewTab::Diff,
                                PreviewTab::Diff => PreviewTab::Task,
                                PreviewTab::Task => PreviewTab::Output,
                            };
                            self.state.preview_tab = new_tab;
                            self.pending_preview_update = true;
                            commands.push(Command::Refresh);
                        }
                        "[" => {
                            // Cycle to previous preview tab (Output <- Diff <- Task <- Output)
                            let new_tab = match self.state.preview_tab {
                                PreviewTab::Output => PreviewTab::Task,
                                PreviewTab::Diff => PreviewTab::Output,
                                PreviewTab::Task => PreviewTab::Diff,
                            };
                            self.state.preview_tab = new_tab;
                            self.pending_preview_update = true;
                            commands.push(Command::Refresh);
                        }
                        "n" => {
                            // Create new worktree
                            self.state.modal_state = ModalState::CreateWorktree;
                        }
                        "D" => {
                            // Delete worktree
                            if self.state.selected_worktree().is_some() {
                                self.state.modal_state = ModalState::DeleteConfirm;
                            }
                        }
                        "T" => {
                            // Link/unlink task (shift+t to avoid conflict with tab navigation)
                            if self.state.selected_worktree().is_some() {
                                self.state.modal_state = ModalState::LinkTask;
                                if let Some(worktree) = self.state.selected_worktree() {
                                    if let Some(ref task_id) = worktree.linked_task {
                                        self.state.task_id_buffer = task_id.clone();
                                    }
                                }
                            }
                        }
                        "a" => {
                            // Launch AI agent
                            if let Some(worktree) = self.state.selected_worktree() {
                                let task = worktree.linked_task.clone();
                                commands.push(Command::LaunchAgent {
                                    worktree: worktree.name.clone(),
                                    task,
                                });
                            }
                        }
                        "Enter" | "o" => {
                            // Enter interactive mode
                            if let Some(worktree) = self.state.selected_worktree() {
                                commands.push(Command::EnterInteractive(worktree.path.clone()));
                            }
                        }
                        "m" => {
                            // Merge workflow
                            if self.state.selected_worktree().is_some() {
                                self.state.modal_state = ModalState::MergeDialog;
                            }
                        }
                        "d" | "f" => {
                            // Show diff
                            commands.push(Command::ShowDiff);
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

        // Handle modal states first
        if self.state.is_modal_open() {
            self.handle_modal_action(action, &mut commands);
            return commands;
        }

        match action {
            Action::NavigateDown => {
                if self.focus_pane == FocusPane::Sidebar {
                    self.state.select_next();
                    commands.push(Command::ShowDiff);
                }
            }
            Action::NavigateUp => {
                if self.focus_pane == FocusPane::Sidebar {
                    self.state.select_prev();
                    commands.push(Command::ShowDiff);
                }
            }
            Action::NavigateLeft => {
                if self.focus_pane == FocusPane::Preview {
                    self.focus_pane = FocusPane::Sidebar;
                    commands.push(Command::SwitchFocus(FocusPane::Sidebar));
                }
            }
            Action::NavigateRight => {
                if self.focus_pane == FocusPane::Sidebar {
                    self.focus_pane = FocusPane::Preview;
                    commands.push(Command::SwitchFocus(FocusPane::Preview));
                }
            }
            Action::Toggle => {
                self.focus_pane = match self.focus_pane {
                    FocusPane::Sidebar => FocusPane::Preview,
                    FocusPane::Preview => FocusPane::Sidebar,
                };
                commands.push(Command::SwitchFocus(self.focus_pane));
            }
            Action::SwitchView => {
                let new_mode = match self.state.view_mode {
                    ViewMode::List => ViewMode::Kanban,
                    ViewMode::Kanban => ViewMode::List,
                    ViewMode::Interactive => ViewMode::List,
                };
                self.state.view_mode = new_mode;
                commands.push(Command::SwitchMode(new_mode));
            }
            Action::SwitchTab(tab) => {
                self.state.preview_tab = match tab {
                    0 => PreviewTab::Output,
                    1 => PreviewTab::Diff,
                    2 => PreviewTab::Task,
                    _ => self.state.preview_tab,
                };
                // Mark preview for update so content is loaded on next update cycle
                self.pending_preview_update = true;
                commands.push(Command::Refresh);
            }
            Action::NewFile => {
                self.state.modal_state = ModalState::CreateWorktree;
            }
            Action::Delete => {
                if self.state.selected_worktree().is_some() {
                    self.state.modal_state = ModalState::DeleteConfirm;
                }
            }
            Action::LinkTask => {
                if self.state.selected_worktree().is_some() {
                    self.state.modal_state = ModalState::LinkTask;
                    // Pre-fill with existing task if any
                    if let Some(worktree) = self.state.selected_worktree() {
                        if let Some(ref task_id) = worktree.linked_task {
                            self.state.task_id_buffer = task_id.clone();
                        }
                    }
                }
            }
            Action::LaunchAgent => {
                if let Some(worktree) = self.state.selected_worktree() {
                    let task = worktree.linked_task.clone();
                    commands.push(Command::LaunchAgent {
                        worktree: worktree.name.clone(),
                        task,
                    });
                }
            }
            Action::Enter => {
                if let Some(worktree) = self.state.selected_worktree() {
                    commands.push(Command::EnterInteractive(worktree.path.clone()));
                }
            }
            Action::Merge => {
                if self.state.selected_worktree().is_some() {
                    self.state.modal_state = ModalState::MergeDialog;
                }
            }
            Action::Refresh => {
                commands.push(Command::Refresh);
            }
            _ => {}
        }

        commands
    }

    /// Handle actions when a modal is open
    fn handle_modal_action(&mut self, action: &Action, commands: &mut Vec<Command>) {
        match self.state.modal_state {
            ModalState::CreateWorktree => {
                match action {
                    Action::Confirm => {
                        // Create worktree
                        if !self.state.new_worktree_name.is_empty() {
                            let branch = if self.state.new_worktree_branch.is_empty() {
                                None
                            } else {
                                Some(self.state.new_worktree_branch.clone())
                            };
                            commands.push(Command::CreateWorktree {
                                name: self.state.new_worktree_name.clone(),
                                branch,
                            });
                            self.state.close_modal();
                        }
                    }
                    Action::Cancel => {
                        self.state.close_modal();
                    }
                    _ => {}
                }
            }
            ModalState::DeleteConfirm => match action {
                Action::Confirm | Action::Delete => {
                    if let Some(worktree) = self.state.selected_worktree() {
                        commands.push(Command::DeleteWorktree(worktree.path.clone()));
                    }
                    self.state.close_modal();
                }
                Action::Cancel => {
                    self.state.close_modal();
                }
                _ => {}
            },
            ModalState::LinkTask => match action {
                Action::Confirm => {
                    if let Some(worktree) = self.state.selected_worktree() {
                        if self.state.task_id_buffer.is_empty() {
                            commands.push(Command::UnlinkTask(worktree.name.clone()));
                        } else {
                            commands.push(Command::LinkTask {
                                worktree: worktree.name.clone(),
                                task_id: self.state.task_id_buffer.clone(),
                            });
                        }
                    }
                    self.state.close_modal();
                }
                Action::Cancel => {
                    self.state.close_modal();
                }
                _ => {}
            },
            ModalState::MergeDialog => match action {
                Action::Cancel => {
                    self.state.close_modal();
                }
                _ => {}
            },
            ModalState::None => {}
        }
    }

    /// Handle character input (for modal text entry)
    pub fn handle_char(&mut self, c: char) -> Vec<Command> {
        if !self.state.is_modal_open() {
            return Vec::new();
        }

        match self.state.modal_state {
            ModalState::CreateWorktree => {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    if self.state.new_worktree_branch.is_empty() {
                        self.state.new_worktree_name.push(c);
                    } else {
                        self.state.new_worktree_branch.push(c);
                    }
                } else if c == ' ' || c == '\t' {
                    // Switch to branch input
                    if self.state.new_worktree_branch.is_empty() {
                        self.state.new_worktree_branch = self.state.new_worktree_name.clone();
                    }
                }
            }
            ModalState::LinkTask => {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    self.state.task_id_buffer.push(c);
                }
            }
            _ => {}
        }

        Vec::new()
    }

    /// Handle backspace
    pub fn handle_backspace(&mut self) {
        if !self.state.is_modal_open() {
            return;
        }

        match self.state.modal_state {
            ModalState::CreateWorktree => {
                if !self.state.new_worktree_branch.is_empty() {
                    self.state.new_worktree_branch.pop();
                } else {
                    self.state.new_worktree_name.pop();
                }
            }
            ModalState::LinkTask => {
                self.state.task_id_buffer.pop();
            }
            _ => {}
        }
    }

    /// Render the plugin UI
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        render_workspace(&self.state, self.focus_pane, self.focused, area, buf, theme);
    }

    /// Get available commands for the command palette
    pub fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::new("create", "Create Worktree", 'n'),
            PluginCommand::new("delete", "Delete Worktree", 'D'),
            PluginCommand::new("link-task", "Link Task", 't'),
            PluginCommand::new("launch-agent", "Launch Agent", 'a'),
            PluginCommand::new("interactive", "Interactive Mode", 'o'),
            PluginCommand::new("merge", "Merge", 'm'),
            PluginCommand::new("refresh", "Refresh", 'r'),
            PluginCommand::new("switch-view", "Switch View", 'v'),
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
        if self.state.is_modal_open() {
            FocusContext::Modal
        } else {
            match self.focus_pane {
                FocusPane::Sidebar => FocusContext::Workspace,
                FocusPane::Preview => match self.state.preview_tab {
                    PreviewTab::Diff => FocusContext::GitDiff,
                    _ => FocusContext::Workspace,
                },
            }
        }
    }

    /// Get status info for the footer
    pub fn status_info(&self, theme: &Theme) -> Vec<ratatui::text::Span<'_>> {
        render_workspace_status(&self.state, theme)
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

    /// Create a new worktree
    pub async fn create_worktree(&mut self, name: &str, branch: Option<&str>) -> Result<PathBuf> {
        if let Some(ref manager) = self.worktree_manager {
            let path = manager.create_worktree(name, branch, None).await?;
            self.state
                .add_output(format!("Created worktree '{}' at {}", name, path.display()));
            self.refresh().await?;
            Ok(path)
        } else {
            Err(anyhow::anyhow!("Worktree manager not initialized"))
        }
    }

    /// Delete a worktree
    pub async fn delete_worktree(&mut self, path: &PathBuf) -> Result<()> {
        if let Some(ref manager) = self.worktree_manager {
            manager.delete_worktree(path, false).await?;
            self.state
                .add_output(format!("Deleted worktree at {}", path.display()));
            self.refresh().await?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Worktree manager not initialized"))
        }
    }

    /// Link a task to a worktree
    pub async fn link_task(&mut self, worktree_name: &str, task_id: &str) -> Result<()> {
        if let Some(worktree) = self.state.get_worktree_mut(worktree_name) {
            worktree.linked_task = Some(task_id.to_string());
            self.state.add_output(format!(
                "Linked task {} to worktree {}",
                task_id, worktree_name
            ));
            self.refresh().await?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Worktree not found: {}", worktree_name))
        }
    }

    /// Unlink task from a worktree
    pub async fn unlink_task(&mut self, worktree_name: &str) -> Result<()> {
        if let Some(worktree) = self.state.get_worktree_mut(worktree_name) {
            worktree.linked_task = None;
            self.state
                .add_output(format!("Unlinked task from worktree {}", worktree_name));
            self.refresh().await?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Worktree not found: {}", worktree_name))
        }
    }

    /// Launch AI agent for a worktree
    pub async fn launch_agent(&mut self, worktree_name: &str, task: Option<&str>) -> Result<()> {
        if let Some(worktree) = self.state.get_worktree_mut(worktree_name) {
            if AgentLauncher::is_kimi_available().await {
                AgentLauncher::launch_agent(&worktree.path, task).await?;
                worktree.agent_running = true;
                self.state
                    .add_output(format!("Launched AI agent for worktree {}", worktree_name));
                Ok(())
            } else {
                Err(anyhow::anyhow!(
                    "Kimi CLI not found. Please install it first."
                ))
            }
        } else {
            Err(anyhow::anyhow!("Worktree not found: {}", worktree_name))
        }
    }

    /// Enter interactive mode for a worktree
    pub async fn enter_interactive(&mut self, path: &PathBuf) -> Result<()> {
        // Create or attach to tmux session
        let session_name = format!(
            "workspace-{}",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
        );

        TmuxManager::create_session(&session_name, path).await?;
        TmuxManager::attach_session(&session_name).await?;

        self.state.view_mode = ViewMode::Interactive;
        self.state
            .add_output(format!("Entered interactive mode for {}", path.display()));

        Ok(())
    }

    /// Switch to a specific preview tab
    pub fn switch_preview_tab(&mut self, tab: PreviewTab) {
        self.state.preview_tab = tab;
    }
}

impl Default for WorkspacePlugin {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Plugin Trait Implementation
// ============================================================================

use crate::plugin::{Command as PluginCmd, Plugin, PluginContext as PluginCtx};
use async_trait::async_trait;

#[async_trait]
impl Plugin for WorkspacePlugin {
    fn id(&self) -> &str {
        "workspace"
    }

    fn name(&self) -> &str {
        "workspaces"
    }

    fn icon(&self) -> char {
        'W'
    }

    async fn init(&mut self, ctx: &PluginCtx) -> anyhow::Result<()> {
        self.repo_path = ctx.project_root.clone();
        self.worktree_manager = Some(WorktreeManager::new(self.repo_path.clone()));

        let _ = self.refresh().await;

        Ok(())
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn handle_event(&mut self, event: crate::event::Event) -> Vec<PluginCmd> {
        let commands = self.handle_event_internal(event);
        commands
            .into_iter()
            .map(|_cmd| PluginCmd::new("workspace", "action"))
            .collect()
    }

    fn render(&self, area: Rect, buf: &mut Buffer, theme: &crate::core::models::Theme) {
        render_workspace(&self.state, self.focus_pane, self.focused, area, buf, theme);
    }

    fn is_focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn commands(&self) -> Vec<crate::plugin::PluginCommand> {
        vec![
            crate::plugin::PluginCommand::with_context(
                "create",
                "Create",
                'n',
                crate::keymap::FocusContext::Workspace,
            ),
            crate::plugin::PluginCommand::with_context(
                "delete",
                "Delete",
                'D',
                crate::keymap::FocusContext::Workspace,
            ),
            crate::plugin::PluginCommand::with_context(
                "link",
                "Link Task",
                'T',
                crate::keymap::FocusContext::Workspace,
            ),
            crate::plugin::PluginCommand::with_context(
                "agent",
                "Launch Agent",
                'a',
                crate::keymap::FocusContext::Workspace,
            ),
            crate::plugin::PluginCommand::with_context(
                "merge",
                "Merge",
                'm',
                crate::keymap::FocusContext::Workspace,
            ),
            crate::plugin::PluginCommand::with_context(
                "prev-tab",
                "Prev",
                '[',
                crate::keymap::FocusContext::Workspace,
            ),
            crate::plugin::PluginCommand::with_context(
                "next-tab",
                "Next",
                ']',
                crate::keymap::FocusContext::Workspace,
            ),
            crate::plugin::PluginCommand::with_context(
                "interactive",
                "Interactive",
                'o',
                crate::keymap::FocusContext::Workspace,
            ),
            crate::plugin::PluginCommand::with_context(
                "refresh",
                "Refresh",
                'r',
                crate::keymap::FocusContext::Workspace,
            ),
        ]
    }

    fn focus_context(&self) -> crate::keymap::FocusContext {
        crate::keymap::FocusContext::Workspace
    }

    async fn update(&mut self) -> anyhow::Result<()> {
        // Update preview content if tab was switched
        if self.pending_preview_update {
            self.pending_preview_update = false;
            self.update_preview().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_id() {
        let plugin = WorkspacePlugin::new();
        assert_eq!(plugin.id(), "workspace");
        assert_eq!(plugin.name(), "workspace");
        assert_eq!(plugin.icon(), 'W');
    }

    #[test]
    fn test_plugin_commands() {
        let plugin = WorkspacePlugin::new();
        let commands = plugin.commands();
        assert!(!commands.is_empty());
        assert!(commands.iter().any(|c| c.id == "create"));
        assert!(commands.iter().any(|c| c.id == "delete"));
    }

    #[test]
    fn test_handle_action_navigation() {
        let mut plugin = WorkspacePlugin::new();

        // Test navigate down
        let cmds = plugin.handle_action(&Action::NavigateDown);
        assert!(cmds.is_empty() || matches!(cmds[0], Command::ShowDiff));

        // Test pane switching
        assert_eq!(plugin.focus_pane, FocusPane::Sidebar);
        let cmds = plugin.handle_action(&Action::NavigateRight);
        assert!(matches!(cmds[0], Command::SwitchFocus(FocusPane::Preview)));
        assert_eq!(plugin.focus_pane, FocusPane::Preview);
    }

    #[test]
    fn test_view_mode() {
        let mut plugin = WorkspacePlugin::new();
        assert_eq!(plugin.view_mode(), ViewMode::List);

        plugin.set_view_mode(ViewMode::Kanban);
        assert_eq!(plugin.view_mode(), ViewMode::Kanban);
    }

    #[test]
    fn test_focus_context() {
        let plugin = WorkspacePlugin::new();
        assert_eq!(plugin.focus_context(), FocusContext::Workspace);
    }

    #[test]
    fn test_modal_handling() {
        let mut plugin = WorkspacePlugin::new();

        // Open create worktree modal
        plugin.handle_action(&Action::NewFile);
        assert!(plugin.state.is_modal_open());
        assert_eq!(plugin.state.modal_state, ModalState::CreateWorktree);

        // Cancel modal
        plugin.handle_action(&Action::Cancel);
        assert!(!plugin.state.is_modal_open());
    }
}
