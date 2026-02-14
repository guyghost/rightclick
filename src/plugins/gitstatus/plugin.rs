//! Git Status Plugin
//!
//! This module implements the Git Status plugin for RightClick, providing
//! a TUI interface for viewing and interacting with git repository status.

use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::core::models::{RepoStatus, Theme, Config};
use crate::event::Event;
use crate::keymap::registry::KeyBindingRegistry;
use crate::keymap::FocusContext;
use crate::plugin::{Command as PluginCommandTrait, Plugin, PluginContext};
use crate::shell::machines::{GitCommand, GitStateMachine};
use crate::shell::services_full::{CliGitService, GitService};

use super::render::{render_git_status, render_status_info};
use super::state::{FocusPane, PluginState, ViewMode};

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
    /// Stage the selected file
    StageFile(PathBuf),
    /// Unstage the selected file
    UnstageFile(PathBuf),
    /// Show diff for the selected file
    ShowDiff(PathBuf),
    /// Open commit dialog
    OpenCommitDialog,
    /// Load commits for history view
    LoadCommits,
    /// Load details for selected commit
    LoadCommitDetails(String),
    /// Select next commit
    NextCommit,
    /// Select previous commit
    PrevCommit,
    /// Execute a git command
    GitExec(Vec<String>),
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
    /// Key binding
    pub key: char,
    /// Context for this command
    pub context: FocusContext,
}

/// Context provided during plugin initialization
#[derive(Debug, Clone)]
pub struct GitPluginContext {
    /// Working directory
    pub work_dir: PathBuf,
    /// Project root path
    pub project_root: PathBuf,
    /// Plugin configuration
    pub config: Config,
}

/// Git Status plugin implementation
#[derive(Debug)]
pub struct GitStatusPlugin {
    /// Plugin state
    state: PluginState,
    /// Repository path
    repo_path: PathBuf,
    /// Whether the plugin is focused
    focused: bool,
    /// Git service
    git_service: CliGitService,
    /// Plugin configuration
    config: Option<Config>,
    /// State machine handling navigation and guards
    state_machine: GitStateMachine,
    /// Hash of commit to load details for
    pending_commit_hash: Option<String>,
}

impl GitStatusPlugin {
    /// Create a new Git Status plugin
    pub fn new() -> Self {
        Self {
            state: PluginState::default(),
            pending_commit_hash: None,
            repo_path: PathBuf::new(),
            focused: false,
            git_service: CliGitService::new(),
            config: None,
            state_machine: GitStateMachine::new(PathBuf::new()),
        }
    }

    /// Create a new plugin with a custom git service (for testing)
    pub fn with_git_service(git_service: CliGitService) -> Self {
        Self {
            state: PluginState::default(),
            pending_commit_hash: None,
            repo_path: PathBuf::new(),
            focused: false,
            git_service,
            config: None,
            state_machine: GitStateMachine::new(PathBuf::new()),
        }
    }

    /// Create a new plugin with configuration
    pub fn with_config(config: crate::core::models::GitStatusPluginConfig) -> Self {
        let plugin = Self::new();
        // Store the show_untracked setting in the state if needed
        // For now, just create with default settings
        let _ = config;
        plugin
    }

    /// Get the plugin ID
    pub fn plugin_id(&self) -> &'static str {
        "git-status"
    }

    /// Get the plugin name
    pub fn plugin_name(&self) -> &'static str {
        "git"
    }

    /// Get the plugin icon
    pub fn plugin_icon(&self) -> char {
        'G'
    }

    /// Initialize the plugin with the given context
    pub async fn init_with_context(&mut self, ctx: &GitPluginContext) -> Result<()> {
        self.repo_path = ctx.project_root.clone();
        self.config = Some(ctx.config.clone());
        self.state_machine = GitStateMachine::new(self.repo_path.clone());
        self.refresh().await?;
        Ok(())
    }

    /// Check if the plugin is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set the focused state
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Get the current focus context
    pub fn focus_context(&self) -> FocusContext {
        match self.state.focus_pane {
            FocusPane::Sidebar => FocusContext::GitStatus,
            FocusPane::Main => FocusContext::GitDiff,
        }
    }

    /// Get available commands for the command palette
    pub fn available_commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand {
                id: "stage".to_string(),
                name: "Stage".to_string(),
                key: 's',
                context: FocusContext::GitStatus,
            },
            PluginCommand {
                id: "unstage".to_string(),
                name: "Unstage".to_string(),
                key: 'u',
                context: FocusContext::GitStatus,
            },
            PluginCommand {
                id: "diff".to_string(),
                name: "Diff".to_string(),
                key: 'd',
                context: FocusContext::GitStatus,
            },
            PluginCommand {
                id: "commit".to_string(),
                name: "Commit".to_string(),
                key: 'c',
                context: FocusContext::GitStatus,
            },
        ]
    }

    /// Handle an event and return any commands to execute
    pub fn handle_event_internal(&mut self, event: Event) -> Vec<Command> {
        let mut commands = Vec::new();

        match event {
            Event::RefreshNeeded => {
                commands.push(Command::Refresh);
            }
            Event::Key { code, modifiers } => {
                // Handle key events
                if modifiers.ctrl {
                    // Handle Ctrl+key combinations
                } else if modifiers.alt {
                    // Handle Alt+key combinations  
                } else {
                    // Handle simple key presses
                    commands.extend(self.handle_key(&code));
                }
            }
            _ => {}
        }

        commands
    }

    /// Handle a key press
    fn handle_key(&mut self, key: &str) -> Vec<Command> {
        if key == "c" {
            return vec![Command::OpenCommitDialog];
        }

        let git_commands = self.state_machine.handle_key(key);
        let mut commands = Vec::new();

        for cmd in git_commands {
            match cmd {
                GitCommand::SelectIndex(idx) => {
                    if self.state.view_mode == ViewMode::History {
                        self.state.selected_commit = Some(idx);
                        if let Some(commit) = self.state.selected_commit() {
                            self.pending_commit_hash = Some(commit.hash.clone());
                        }
                    } else {
                        self.state.selected_file = Some(idx);
                    }
                    commands.push(Command::Refresh);
                }
                GitCommand::SetFocus(pane) => {
                    self.state.focus_pane = pane;
                    commands.push(Command::Refresh);
                }
                GitCommand::ExecuteAction(crate::core::models::action::ActionId::Stage) => {
                    if let Some(file) = self.state.selected_file_path() {
                        commands.push(Command::StageFile(file));
                    }
                }
                GitCommand::ExecuteAction(crate::core::models::action::ActionId::Unstage) => {
                    if let Some(file) = self.state.selected_file_path() {
                        commands.push(Command::UnstageFile(file));
                    }
                }
                GitCommand::ExecuteAction(crate::core::models::action::ActionId::Diff) => {
                    if let Some(file) = self.state.selected_file_path() {
                        commands.push(Command::ShowDiff(file));
                    }
                }
                GitCommand::ExecuteAction(_) => {}
                GitCommand::SwitchMode(mode) => {
                    commands.push(Command::SwitchMode(mode));
                }
                GitCommand::LoadCommits => {
                    commands.push(Command::LoadCommits);
                }
                GitCommand::Refresh => {
                    commands.push(Command::Refresh);
                }
            }
        }

        if key == "s" && commands.is_empty() {
            if let Some(file) = self.state.selected_file_path() {
                commands.push(Command::StageFile(file));
            }
        } else if key == "u" && commands.is_empty() {
            if let Some(file) = self.state.selected_file_path() {
                commands.push(Command::UnstageFile(file));
            }
        } else if key == "d" && commands.is_empty() {
            if let Some(file) = self.state.selected_file_path() {
                commands.push(Command::ShowDiff(file));
            }
        } else if key == "r" && commands.is_empty() {
            commands.push(Command::Refresh);
        }

        commands
    }

    /// Execute a command
    pub async fn execute_internal(&mut self, command: Command) -> Result<Vec<Event>> {
        let mut events = Vec::new();

        match command {
            Command::Refresh => {
                self.refresh().await?;
                events.push(Event::RefreshNeeded);
            }
            Command::StageFile(path) => {
                self.stage_file(&path).await?;
                self.refresh().await?;
                events.push(Event::GitChanged);
            }
            Command::UnstageFile(path) => {
                self.unstage_file(&path).await?;
                self.refresh().await?;
                events.push(Event::GitChanged);
            }
            Command::ShowDiff(path) => {
                self.load_diff(&path).await?;
                self.state.view_mode = ViewMode::Diff;
                self.sync_state_machine();
                events.push(Event::RefreshNeeded);
            }
            Command::SwitchMode(mode) => {
                self.state.view_mode = mode;
                self.sync_state_machine();
                events.push(Event::RefreshNeeded);
            }
            Command::SwitchFocus(pane) => {
                self.state.focus_pane = pane;
                self.state_machine.set_focus_pane(pane);
                events.push(Event::RefreshNeeded);
            }
            Command::LoadCommits => {
                self.load_commits().await?;
                events.push(Event::RefreshNeeded);
            }
            Command::LoadCommitDetails(hash) => {
                self.load_commit_details(&hash).await?;
                events.push(Event::RefreshNeeded);
            }
            Command::NextCommit => {
                self.state.select_next_commit();
                if let Some(commit) = self.state.selected_commit().cloned() {
                    self.load_commit_details(&commit.hash).await?;
                }
                events.push(Event::RefreshNeeded);
            }
            Command::PrevCommit => {
                self.state.select_prev_commit();
                if let Some(commit) = self.state.selected_commit().cloned() {
                    self.load_commit_details(&commit.hash).await?;
                }
                events.push(Event::RefreshNeeded);
            }
            _ => {}
        }

        Ok(events)
    }

    /// Refresh the repository status
    async fn refresh(&mut self) -> Result<()> {
        let status = self.fetch_repo_status().await?;
        self.state.update_status(status);
        self.sync_state_machine();
        Ok(())
    }

    /// Fetch repository status
    async fn fetch_repo_status(&self) -> Result<RepoStatus> {
        self.git_service.status(&self.repo_path).await
    }

    /// Load diff for a file
    async fn load_diff(&mut self, path: &PathBuf) -> Result<()> {
        let diff = self.git_service.diff(&self.repo_path, path).await?;
        self.state.diff = Some(diff);
        Ok(())
    }

    /// Stage a file
    async fn stage_file(&self, path: &PathBuf) -> Result<()> {
        self.git_service.stage(&self.repo_path, path).await
    }

    /// Unstage a file
    async fn unstage_file(&self, path: &PathBuf) -> Result<()> {
        self.git_service.unstage(&self.repo_path, path).await
    }

    /// Render the plugin
    pub fn render_internal(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        render_git_status(&self.state, area, buf, theme, self.focused);
    }

    /// Get status info for the footer
    pub fn status_info(&self) -> String {
        render_status_info(&self.state)
    }

    /// Get a reference to the plugin state
    pub fn state(&self) -> &PluginState {
        &self.state
    }

    /// Get a mutable reference to the plugin state
    pub fn state_mut(&mut self) -> &mut PluginState {
        &mut self.state
    }

    /// Load commits for history view
    async fn load_commits(&mut self) -> Result<()> {
        tracing::debug!("Loading commits from {:?}", self.repo_path);
        let commits = self.git_service.commits(&self.repo_path, 100).await?;
        tracing::debug!("Loaded {} commits", commits.len());
        self.state.commits = commits;
        self.state.selected_commit = if self.state.commits.is_empty() {
            None
        } else {
            Some(0)
        };
        self.state_machine.update_items(self.state.commits.len());
        self.state_machine.set_selected_index(self.state.selected_commit);

        // Load details for first commit
        let first_hash = self.state.selected_commit().map(|c| c.hash.clone());
        if let Some(hash) = first_hash {
            self.load_commit_details(&hash).await?;
        }

        Ok(())
    }

    /// Load commit details
    async fn load_commit_details(&mut self, hash: &str) -> Result<()> {
        let files = self.git_service.commit_details(&self.repo_path, hash).await?;
        self.state.commit_files = files;
        Ok(())
    }

    fn sync_state_machine(&self) {
        let item_count = if self.state.view_mode == ViewMode::History {
            self.state.commits.len()
        } else {
            self.state.files.len()
        };
        self.state_machine.initialize(item_count, self.state.view_mode);
        self.state_machine
            .set_selected_index(if self.state.view_mode == ViewMode::History {
                self.state.selected_commit
            } else {
                self.state.selected_file
            });
        self.state_machine.set_focus_pane(self.state.focus_pane);
    }
}

impl Default for GitStatusPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for GitStatusPlugin {
    fn id(&self) -> &str {
        self.plugin_id()
    }

    fn name(&self) -> &str {
        self.plugin_name()
    }

    fn icon(&self) -> char {
        self.plugin_icon()
    }

    async fn init(&mut self, ctx: &PluginContext) -> Result<()> {
        self.repo_path = ctx.project_root.clone();
        self.state_machine = GitStateMachine::new(self.repo_path.clone());
        tracing::debug!("Git plugin initialized with repo: {:?}", self.repo_path);
        if let Ok(repo_status) = self.fetch_repo_status().await {
            self.state.update_status(repo_status);
            self.sync_state_machine();
        }
        // Load commits for history view (lazygit-style)
        if let Err(e) = self.load_commits().await {
            tracing::warn!("Failed to load commits: {}", e);
        }
        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        // Clean up if needed
        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Vec<PluginCommandTrait> {
        let commands = self.handle_event_internal(event);
        // Convert internal commands to plugin commands
        commands
            .into_iter()
            .filter_map(|cmd| match cmd {
                Command::Refresh => Some(PluginCommandTrait::new("git-status", "refresh")),
                Command::StageFile(path) => {
                    Some(PluginCommandTrait::with_args("git-status", "stage", path.to_string_lossy()))
                }
                Command::UnstageFile(path) => Some(PluginCommandTrait::with_args(
                    "git-status",
                    "unstage",
                    path.to_string_lossy(),
                )),
                Command::ShowDiff(path) => {
                    Some(PluginCommandTrait::with_args("git-status", "diff", path.to_string_lossy()))
                }
                Command::OpenCommitDialog => Some(PluginCommandTrait::new("git-status", "commit")),
                _ => None,
            })
            .collect()
    }

    fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        self.render_internal(area, buf, theme);
    }

    fn is_focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn commands(&self) -> Vec<crate::plugin::PluginCommand> {
        vec![
            crate::plugin::PluginCommand::with_context("stage", "Stage", 's', FocusContext::GitStatus),
            crate::plugin::PluginCommand::with_context("unstage", "Unstage", 'u', FocusContext::GitStatus),
            crate::plugin::PluginCommand::with_context("diff", "Diff", 'd', FocusContext::GitStatus),
            crate::plugin::PluginCommand::with_context("commit", "Commit", 'c', FocusContext::GitStatus),
        ]
    }

    fn focus_context(&self) -> FocusContext {
        self.focus_context()
    }

    async fn update(&mut self) -> Result<()> {
        // Load commit details if there's a pending hash
        if let Some(hash) = self.pending_commit_hash.take() {
            if let Err(e) = self.load_commit_details(&hash).await {
                tracing::warn!("Failed to load commit details: {}", e);
            }
        }
        Ok(())
    }
}

/// Create a new Git Status plugin
pub fn create_plugin() -> GitStatusPlugin {
    GitStatusPlugin::new()
}

/// Register default key bindings for this plugin
pub fn register_default_bindings(registry: &mut KeyBindingRegistry) {
    use crate::keymap::Binding;

    registry.register_binding(Binding {
        key: "s".to_string(),
        command_id: "git-status.stage".to_string(),
        context: FocusContext::GitStatus,
    });

    registry.register_binding(Binding {
        key: "u".to_string(),
        command_id: "git-status.unstage".to_string(),
        context: FocusContext::GitStatus,
    });

    registry.register_binding(Binding {
        key: "d".to_string(),
        command_id: "git-status.diff".to_string(),
        context: FocusContext::GitStatus,
    });

    registry.register_binding(Binding {
        key: "c".to_string(),
        command_id: "git-status.commit".to_string(),
        context: FocusContext::GitStatus,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_new() {
        let plugin = GitStatusPlugin::new();
        assert_eq!(plugin.id(), "git-status");
        assert_eq!(plugin.name(), "git");
        assert_eq!(plugin.icon(), 'G');
    }

    #[test]
    fn test_default_command() {
        let cmd: Command = Default::default();
        assert_eq!(cmd, Command::None);
    }

    #[test]
    fn test_plugin_commands() {
        let plugin = GitStatusPlugin::new();
        let commands = plugin.commands();
        assert!(!commands.is_empty());
        
        // Check that we have the expected commands
        let has_stage = commands.iter().any(|c| c.id == "stage");
        let has_unstage = commands.iter().any(|c| c.id == "unstage");
        let has_diff = commands.iter().any(|c| c.id == "diff");
        let has_commit = commands.iter().any(|c| c.id == "commit");
        
        assert!(has_stage);
        assert!(has_unstage);
        assert!(has_diff);
        assert!(has_commit);
    }

    #[test]
    fn test_handle_key_navigation() {
        let mut plugin = GitStatusPlugin::new();
        
        // Test j key (next)
        let commands = plugin.handle_key("j");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], Command::Refresh);
        
        // Test k key (prev)
        let commands = plugin.handle_key("k");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], Command::Refresh);
        
        // Test h key (sidebar focus)
        let commands = plugin.handle_key("h");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], Command::Refresh);
        assert_eq!(plugin.state.focus_pane, FocusPane::Sidebar);
        
        // Test l key (main focus)
        let commands = plugin.handle_key("l");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], Command::Refresh);
        assert_eq!(plugin.state.focus_pane, FocusPane::Main);
    }

    #[test]
    fn test_handle_key_actions() {
        let mut plugin = GitStatusPlugin::new();
        
        // Test r key (refresh)
        let commands = plugin.handle_key("r");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], Command::Refresh);
        
        // Test c key (commit dialog)
        let commands = plugin.handle_key("c");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], Command::OpenCommitDialog);
    }

    #[test]
    fn test_focus_context() {
        let mut plugin = GitStatusPlugin::new();
        
        // Default is sidebar
        assert_eq!(plugin.focus_context(), FocusContext::GitStatus);
        
        // Switch to main
        plugin.state.focus_pane = FocusPane::Main;
        assert_eq!(plugin.focus_context(), FocusContext::GitDiff);
    }

    #[tokio::test]
    async fn test_plugin_init() {
        let mut plugin = GitStatusPlugin::new();
        let ctx = PluginContext {
            project_root: PathBuf::from("."),
            config: Config::default(),
        };
        
        // Should not panic
        let result = plugin.init(&ctx).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_plugin_shutdown() {
        let mut plugin = GitStatusPlugin::new();
        let result = plugin.shutdown();
        assert!(result.is_ok());
    }

    #[test]
    fn test_available_commands() {
        let plugin = GitStatusPlugin::new();
        let commands = plugin.available_commands();
        
        assert_eq!(commands.len(), 4);
        
        let stage = commands.iter().find(|c| c.id == "stage").unwrap();
        assert_eq!(stage.key, 's');
        assert_eq!(stage.context, FocusContext::GitStatus);
        
        let unstage = commands.iter().find(|c| c.id == "unstage").unwrap();
        assert_eq!(unstage.key, 'u');
    }
}
