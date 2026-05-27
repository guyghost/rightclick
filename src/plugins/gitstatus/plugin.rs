//! Git Status Plugin
//!
//! This module implements the Git Status plugin for RightClick, providing
//! a TUI interface for viewing and interacting with git repository status.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use anyhow::Result;
use async_trait::async_trait;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::core::models::{Config, FileStatus, RepoStatus, Theme};
use crate::event::Event;
use crate::keymap::FocusContext;
use crate::keymap::registry::KeyBindingRegistry;
use crate::plugin::{Command as PluginCommandTrait, Plugin, PluginContext};
use crate::shell::machines::{GitCommand, GitStateMachine};
use crate::shell::services_full::{CliGitService, GitService};

use super::render::{render_git_status, render_status_info};
use super::state::{FocusPane, GitModal, PluginState, ViewMode};

/// A command that can be executed by the plugin
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Command {
    /// No operation
    #[default]
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
    /// Execute a commit with the given message
    ExecuteCommit(String),
    /// Load branches list
    LoadBranches,
    /// Checkout a branch
    CheckoutBranch(String),
    /// Create a new branch
    CreateBranch(String),
    /// Delete a branch
    DeleteBranch(String),
    /// Push to remote
    PushToRemote,
    /// Pull from remote
    PullFromRemote,
    /// Load stashes list
    LoadStashes,
    /// Save to stash (optional message)
    StashSave(Option<String>),
    /// Pop stash at index
    StashPop(usize),
    /// Drop stash at index
    StashDrop(usize),
    /// Execute a git command
    GitExec(Vec<String>),
    /// Emit an event
    EmitEvent(Event),
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
    /// Path and status of file to load diff for (auto-preview on selection change)
    pending_diff_path: Option<(PathBuf, FileStatus)>,
    /// Commands queued from input handling and executed in update()
    pending_commands: VecDeque<Command>,
}

impl GitStatusPlugin {
    /// Create a new Git Status plugin
    pub fn new() -> Self {
        Self {
            state: PluginState::default(),
            pending_commit_hash: None,
            pending_diff_path: None,
            pending_commands: VecDeque::new(),
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
            pending_diff_path: None,
            pending_commands: VecDeque::new(),
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
        "Git Status"
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
        // If a modal is active, delegate key handling to modal
        if self.state.modal_active {
            return self.handle_modal_key(key);
        }

        // Commit dialog
        if key == "c" {
            return vec![Command::OpenCommitDialog];
        }

        // Branches view
        if key == "B" {
            return vec![
                Command::LoadBranches,
                Command::SwitchMode(ViewMode::Branches),
            ];
        }

        // Stash view
        if key == "Z" {
            return vec![Command::LoadStashes, Command::SwitchMode(ViewMode::Stash)];
        }

        // Push to remote
        if key == "P" {
            return vec![Command::PushToRemote];
        }

        // Pull from remote
        if key == "p" && self.state.view_mode != ViewMode::Status {
            // 'p' in Status mode is handled by state machine (no-op)
            // In other modes, it pulls
        }

        // Clipboard: copy commit hash or file path
        if key == "y" {
            let text = match self.state.view_mode {
                ViewMode::History => self.state.selected_commit().map(|c| c.hash.clone()),
                ViewMode::Branches => self.state.selected_branch().map(|b| b.name.clone()),
                ViewMode::Stash => self
                    .state
                    .selected_stash()
                    .map(|s| format!("stash@{{{}}}: {}", s.index, s.message)),
                _ => self.state.selected_file().map(|f| f.path.clone()),
            };
            if let Some(text) = text {
                if crate::shell::clipboard::copy_to_clipboard(&text).is_ok() {
                    return vec![Command::EmitEvent(Event::Notification {
                        message: format!("Copied: {}", text),
                        level: crate::event::NotificationEventLevel::Success,
                    })];
                }
            }
            return vec![];
        }

        // Branch-specific keys when in Branches mode
        if self.state.view_mode == ViewMode::Branches {
            return self.handle_branch_key(key);
        }

        // Stash-specific keys when in Stash mode
        if self.state.view_mode == ViewMode::Stash {
            return self.handle_stash_key(key);
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
                        self.queue_selected_file_diff();
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

        commands
    }

    /// Handle key presses when a modal is active
    fn handle_modal_key(&mut self, key: &str) -> Vec<Command> {
        match key {
            "Escape" => {
                self.state.close_modal();
                vec![Command::Refresh]
            }
            "Enter" | "d" | "D" => {
                // Extract modal action before closing.
                let modal = self.state.active_modal.clone();
                match modal {
                    Some(GitModal::DeleteBranch { name }) => {
                        self.state.close_modal();
                        vec![Command::DeleteBranch(name)]
                    }
                    Some(GitModal::DropStash { index }) => {
                        self.state.close_modal();
                        vec![Command::StashDrop(index)]
                    }
                    _ if key == "Enter" => {
                        self.state.close_modal();
                        vec![Command::Refresh]
                    }
                    _ => vec![],
                }
            }
            _ => vec![],
        }
    }

    /// Handle keys in Branches view mode
    fn handle_branch_key(&mut self, key: &str) -> Vec<Command> {
        match key {
            "j" | "Down" => {
                self.state.select_next_branch();
                vec![Command::Refresh]
            }
            "k" | "Up" => {
                self.state.select_prev_branch();
                vec![Command::Refresh]
            }
            "Enter" => {
                if let Some(branch) = self.state.selected_branch() {
                    if !branch.is_current {
                        let name = branch.name.clone();
                        return vec![Command::CheckoutBranch(name)];
                    }
                }
                vec![]
            }
            "n" => {
                self.state.open_modal(GitModal::CreateBranch);
                vec![Command::Refresh]
            }
            "d" => {
                if let Some(branch) = self.state.selected_branch() {
                    if !branch.is_current {
                        let name = branch.name.clone();
                        self.state.open_modal(GitModal::DeleteBranch { name });
                        return vec![Command::Refresh];
                    }
                }
                vec![]
            }
            "g" | "Home" => {
                if !self.state.branches.is_empty() {
                    self.state.selected_branch = Some(0);
                }
                vec![Command::Refresh]
            }
            "G" | "End" => {
                if !self.state.branches.is_empty() {
                    self.state.selected_branch = Some(self.state.branches.len() - 1);
                }
                vec![Command::Refresh]
            }
            "S" => vec![Command::SwitchMode(ViewMode::Status)],
            "H" => vec![Command::SwitchMode(ViewMode::History), Command::LoadCommits],
            "r" | "R" => vec![Command::LoadBranches],
            _ => vec![],
        }
    }

    /// Handle keys in Stash view mode
    fn handle_stash_key(&mut self, key: &str) -> Vec<Command> {
        match key {
            "j" | "Down" => {
                self.state.select_next_stash();
                vec![Command::Refresh]
            }
            "k" | "Up" => {
                self.state.select_prev_stash();
                vec![Command::Refresh]
            }
            "Enter" => {
                if let Some(stash) = self.state.selected_stash() {
                    let idx = stash.index;
                    return vec![Command::StashPop(idx)];
                }
                vec![]
            }
            "s" => {
                vec![Command::StashSave(None)]
            }
            "d" => {
                if let Some(stash) = self.state.selected_stash() {
                    let idx = stash.index;
                    self.state.open_modal(GitModal::DropStash { index: idx });
                    return vec![Command::Refresh];
                }
                vec![]
            }
            "g" | "Home" => {
                if !self.state.stashes.is_empty() {
                    self.state.selected_stash = Some(0);
                }
                vec![Command::Refresh]
            }
            "G" | "End" => {
                if !self.state.stashes.is_empty() {
                    self.state.selected_stash = Some(self.state.stashes.len() - 1);
                }
                vec![Command::Refresh]
            }
            "S" => vec![Command::SwitchMode(ViewMode::Status)],
            "H" => vec![Command::SwitchMode(ViewMode::History), Command::LoadCommits],
            "r" | "R" => vec![Command::LoadStashes],
            _ => vec![],
        }
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

                // Ensure appropriate selection when switching modes
                match mode {
                    ViewMode::History => {
                        if self.state.selected_commit.is_none() && !self.state.commits.is_empty() {
                            self.state.selected_commit = Some(0);
                        }
                        self.state.selected_file = None;
                    }
                    ViewMode::Branches => {
                        if self.state.selected_branch.is_none() && !self.state.branches.is_empty() {
                            self.state.selected_branch = Some(0);
                        }
                    }
                    ViewMode::Stash => {
                        if self.state.selected_stash.is_none() && !self.state.stashes.is_empty() {
                            self.state.selected_stash = Some(0);
                        }
                    }
                    ViewMode::Status | ViewMode::Diff => {
                        if self.state.selected_file.is_none() && !self.state.files.is_empty() {
                            self.state.selected_file = Some(0);
                        }
                    }
                }

                self.validate_selections();
                if matches!(mode, ViewMode::Status | ViewMode::Diff) {
                    self.ensure_file_selection();
                    self.queue_selected_file_diff();
                } else {
                    self.pending_diff_path = None;
                }
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
            Command::ExecuteCommit(message) => {
                match self.git_service.commit(&self.repo_path, &message).await {
                    Ok(()) => {
                        self.refresh().await?;
                        events.push(Event::GitChanged);
                        events.push(Event::Notification {
                            message: "Commit created successfully".to_string(),
                            level: crate::event::NotificationEventLevel::Success,
                        });
                    }
                    Err(e) => {
                        self.state.open_modal(GitModal::Error {
                            message: format!("Commit failed: {}", e),
                        });
                        events.push(Event::RefreshNeeded);
                    }
                }
            }
            Command::LoadBranches => {
                match self.git_service.branches(&self.repo_path).await {
                    Ok(branches) => {
                        self.state.branches = branches;
                        if self.state.selected_branch.is_none() && !self.state.branches.is_empty() {
                            self.state.selected_branch = Some(0);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load branches: {}", e);
                    }
                }
                events.push(Event::RefreshNeeded);
            }
            Command::CheckoutBranch(name) => {
                match self.git_service.checkout(&self.repo_path, &name).await {
                    Ok(()) => {
                        self.refresh().await?;
                        // Reload branches to update current marker
                        if let Ok(branches) = self.git_service.branches(&self.repo_path).await {
                            self.state.branches = branches;
                        }
                        events.push(Event::GitChanged);
                        events.push(Event::Notification {
                            message: format!("Switched to branch '{}'", name),
                            level: crate::event::NotificationEventLevel::Success,
                        });
                    }
                    Err(e) => {
                        self.state.open_modal(GitModal::Error {
                            message: format!("Checkout failed: {}", e),
                        });
                        events.push(Event::RefreshNeeded);
                    }
                }
            }
            Command::CreateBranch(name) => {
                match self.git_service.create_branch(&self.repo_path, &name).await {
                    Ok(()) => {
                        // Reload branches
                        if let Ok(branches) = self.git_service.branches(&self.repo_path).await {
                            self.state.branches = branches;
                        }
                        events.push(Event::GitChanged);
                        events.push(Event::Notification {
                            message: format!("Created branch '{}'", name),
                            level: crate::event::NotificationEventLevel::Success,
                        });
                    }
                    Err(e) => {
                        self.state.open_modal(GitModal::Error {
                            message: format!("Create branch failed: {}", e),
                        });
                        events.push(Event::RefreshNeeded);
                    }
                }
            }
            Command::DeleteBranch(name) => {
                match self.git_service.delete_branch(&self.repo_path, &name).await {
                    Ok(()) => {
                        // Reload branches
                        if let Ok(branches) = self.git_service.branches(&self.repo_path).await {
                            self.state.branches = branches;
                            // Reset selection if out of bounds
                            if let Some(idx) = self.state.selected_branch {
                                if idx >= self.state.branches.len() {
                                    self.state.selected_branch = if self.state.branches.is_empty() {
                                        None
                                    } else {
                                        Some(self.state.branches.len() - 1)
                                    };
                                }
                            }
                        }
                        events.push(Event::GitChanged);
                        events.push(Event::Notification {
                            message: format!("Deleted branch '{}'", name),
                            level: crate::event::NotificationEventLevel::Success,
                        });
                    }
                    Err(e) => {
                        self.state.open_modal(GitModal::Error {
                            message: format!("Delete branch failed: {}", e),
                        });
                        events.push(Event::RefreshNeeded);
                    }
                }
            }
            Command::PushToRemote => {
                let branch = self.state.branch.clone();
                match self
                    .git_service
                    .push(&self.repo_path, "origin", &branch)
                    .await
                {
                    Ok(()) => {
                        self.refresh().await?;
                        events.push(Event::GitChanged);
                        events.push(Event::Notification {
                            message: format!("Pushed to origin/{}", branch),
                            level: crate::event::NotificationEventLevel::Success,
                        });
                    }
                    Err(e) => {
                        self.state.open_modal(GitModal::Error {
                            message: format!("Push failed: {}", e),
                        });
                        events.push(Event::RefreshNeeded);
                    }
                }
            }
            Command::PullFromRemote => {
                let branch = self.state.branch.clone();
                match self
                    .git_service
                    .pull(&self.repo_path, "origin", &branch)
                    .await
                {
                    Ok(()) => {
                        self.refresh().await?;
                        events.push(Event::GitChanged);
                        events.push(Event::Notification {
                            message: format!("Pulled from origin/{}", branch),
                            level: crate::event::NotificationEventLevel::Success,
                        });
                    }
                    Err(e) => {
                        self.state.open_modal(GitModal::Error {
                            message: format!("Pull failed: {}", e),
                        });
                        events.push(Event::RefreshNeeded);
                    }
                }
            }
            Command::LoadStashes => {
                match self.git_service.stash_list(&self.repo_path).await {
                    Ok(stashes) => {
                        self.state.stashes = stashes;
                        if self.state.selected_stash.is_none() && !self.state.stashes.is_empty() {
                            self.state.selected_stash = Some(0);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load stashes: {}", e);
                    }
                }
                events.push(Event::RefreshNeeded);
            }
            Command::StashSave(message) => {
                match self
                    .git_service
                    .stash_save(&self.repo_path, message.as_deref())
                    .await
                {
                    Ok(()) => {
                        self.refresh().await?;
                        // Reload stashes
                        if let Ok(stashes) = self.git_service.stash_list(&self.repo_path).await {
                            self.state.stashes = stashes;
                        }
                        events.push(Event::GitChanged);
                        events.push(Event::Notification {
                            message: "Changes stashed".to_string(),
                            level: crate::event::NotificationEventLevel::Success,
                        });
                    }
                    Err(e) => {
                        self.state.open_modal(GitModal::Error {
                            message: format!("Stash failed: {}", e),
                        });
                        events.push(Event::RefreshNeeded);
                    }
                }
            }
            Command::StashPop(index) => {
                match self.git_service.stash_pop(&self.repo_path, index).await {
                    Ok(()) => {
                        self.refresh().await?;
                        // Reload stashes
                        if let Ok(stashes) = self.git_service.stash_list(&self.repo_path).await {
                            self.state.stashes = stashes;
                            if let Some(idx) = self.state.selected_stash {
                                if idx >= self.state.stashes.len() {
                                    self.state.selected_stash = if self.state.stashes.is_empty() {
                                        None
                                    } else {
                                        Some(self.state.stashes.len() - 1)
                                    };
                                }
                            }
                        }
                        events.push(Event::GitChanged);
                        events.push(Event::Notification {
                            message: "Stash applied and dropped".to_string(),
                            level: crate::event::NotificationEventLevel::Success,
                        });
                    }
                    Err(e) => {
                        self.state.open_modal(GitModal::Error {
                            message: format!("Stash pop failed: {}", e),
                        });
                        events.push(Event::RefreshNeeded);
                    }
                }
            }
            Command::StashDrop(index) => {
                match self.git_service.stash_drop(&self.repo_path, index).await {
                    Ok(()) => {
                        // Reload stashes
                        if let Ok(stashes) = self.git_service.stash_list(&self.repo_path).await {
                            self.state.stashes = stashes;
                            if let Some(idx) = self.state.selected_stash {
                                if idx >= self.state.stashes.len() {
                                    self.state.selected_stash = if self.state.stashes.is_empty() {
                                        None
                                    } else {
                                        Some(self.state.stashes.len() - 1)
                                    };
                                }
                            }
                        }
                        events.push(Event::Notification {
                            message: format!("Dropped stash@{{{}}}", index),
                            level: crate::event::NotificationEventLevel::Success,
                        });
                    }
                    Err(e) => {
                        self.state.open_modal(GitModal::Error {
                            message: format!("Stash drop failed: {}", e),
                        });
                        events.push(Event::RefreshNeeded);
                    }
                }
            }
            Command::None
            | Command::OpenCommitDialog
            | Command::GitExec(_)
            | Command::EmitEvent(_) => {}
        }

        Ok(events)
    }

    /// Refresh the repository status
    async fn refresh(&mut self) -> Result<()> {
        let status = self.fetch_repo_status().await?;
        self.apply_repo_status(status);
        Ok(())
    }

    fn apply_repo_status(&mut self, status: RepoStatus) {
        self.state.update_status(status);
        self.validate_selections();
        self.ensure_file_selection();
        self.queue_selected_file_diff();
        self.sync_state_machine();
    }

    fn ensure_file_selection(&mut self) {
        if matches!(self.state.view_mode, ViewMode::Status | ViewMode::Diff)
            && self.state.selected_file.is_none()
            && !self.state.files.is_empty()
        {
            self.state.selected_file = Some(0);
        }
    }

    fn queue_selected_file_diff(&mut self) {
        if let Some(file) = self.state.selected_file() {
            self.pending_diff_path = Some((PathBuf::from(&file.path), file.status));
        } else {
            self.pending_diff_path = None;
            self.state.diff = None;
        }
    }

    /// Fetch repository status
    async fn fetch_repo_status(&self) -> Result<RepoStatus> {
        self.git_service.status(&self.repo_path).await
    }

    /// Load diff for a file based on its status
    async fn load_diff(&mut self, path: &Path) -> Result<()> {
        // Determine the file status for the correct diff command
        let status = self
            .state
            .selected_file()
            .map(|f| f.status)
            .unwrap_or(FileStatus::Modified);
        let diff = self
            .git_service
            .diff_file(&self.repo_path, path, status)
            .await?;
        self.state.diff = Some(diff);
        Ok(())
    }

    /// Stage a file
    async fn stage_file(&self, path: &Path) -> Result<()> {
        self.git_service.stage(&self.repo_path, path).await
    }

    /// Unstage a file
    async fn unstage_file(&self, path: &Path) -> Result<()> {
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

        // Validate selections after loading commits
        self.validate_selections();

        // Update state machine with new commit count
        if self.state.view_mode == ViewMode::History {
            self.state_machine.update_items(self.state.commits.len());
            self.state_machine
                .set_selected_index(self.state.selected_commit);
        }

        // Load details for selected commit (if any)
        if let Some(commit) = self.state.selected_commit().cloned() {
            self.load_commit_details(&commit.hash).await?;
        }

        Ok(())
    }

    /// Load commit details
    async fn load_commit_details(&mut self, hash: &str) -> Result<()> {
        // Load file list with stats
        let files = self
            .git_service
            .commit_details(&self.repo_path, hash)
            .await?;
        self.state.commit_files = files;

        // Load full diff with patch content
        let diff = self.git_service.commit_diff(&self.repo_path, hash).await?;
        self.state.commit_diff = Some(diff);

        Ok(())
    }

    fn sync_state_machine(&self) {
        // Determine item count based on view mode and available data
        let item_count = match self.state.view_mode {
            ViewMode::History => self.state.commits.len(),
            ViewMode::Branches => self.state.branches.len(),
            ViewMode::Stash => self.state.stashes.len(),
            _ => self.state.files.len(),
        };

        self.state_machine
            .initialize(item_count, self.state.view_mode);

        // Set selected index based on view mode
        let selected_index = match self.state.view_mode {
            ViewMode::History => self.state.selected_commit,
            ViewMode::Branches => self.state.selected_branch,
            ViewMode::Stash => self.state.selected_stash,
            _ => self.state.selected_file,
        };
        self.state_machine.set_selected_index(selected_index);
        self.state_machine.set_focus_pane(self.state.focus_pane);

        tracing::debug!(
            "Synced state machine: view_mode={:?}, item_count={}, selected_index={:?}, focus_pane={:?}",
            self.state.view_mode,
            item_count,
            selected_index,
            self.state.focus_pane
        );
    }

    /// Ensure selections are valid after data changes
    fn validate_selections(&mut self) {
        // Validate file selection
        if let Some(idx) = self.state.selected_file {
            if idx >= self.state.files.len() {
                self.state.selected_file = if self.state.files.is_empty() {
                    None
                } else {
                    Some(self.state.files.len() - 1)
                };
            }
        }

        // Validate commit selection
        if let Some(idx) = self.state.selected_commit {
            if idx >= self.state.commits.len() {
                self.state.selected_commit = if self.state.commits.is_empty() {
                    None
                } else {
                    Some(self.state.commits.len() - 1)
                };
            }
        }

        // Validate branch selection
        if let Some(idx) = self.state.selected_branch {
            if idx >= self.state.branches.len() {
                self.state.selected_branch = if self.state.branches.is_empty() {
                    None
                } else {
                    Some(self.state.branches.len() - 1)
                };
            }
        }

        // Validate stash selection
        if let Some(idx) = self.state.selected_stash {
            if idx >= self.state.stashes.len() {
                self.state.selected_stash = if self.state.stashes.is_empty() {
                    None
                } else {
                    Some(self.state.stashes.len() - 1)
                };
            }
        }

        // Ensure at least one selection when appropriate
        if self.state.view_mode == ViewMode::History
            && self.state.selected_commit.is_none()
            && !self.state.commits.is_empty()
        {
            self.state.selected_commit = Some(0);
        }
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
            self.apply_repo_status(repo_status);
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
        for cmd in commands {
            self.pending_commands.push_back(cmd);
        }
        Vec::new()
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
        // Build contextual shortcuts based on view mode
        let mut commands = vec![
            // Navigation
            crate::plugin::PluginCommand::with_context_description(
                "nav-down",
                "Down",
                "Move selection down",
                'j',
                self.focus_context(),
            ),
            crate::plugin::PluginCommand::with_context_description(
                "nav-up",
                "Up",
                "Move selection up",
                'k',
                self.focus_context(),
            ),
        ];

        if self.state.view_mode == ViewMode::History {
            // History mode shortcuts
            commands.extend(vec![
                crate::plugin::PluginCommand::with_context_description(
                    "status",
                    "Status",
                    "Switch to status view",
                    'S',
                    FocusContext::GitStatus,
                )
                .with_footer_priority(5),
                crate::plugin::PluginCommand::with_context_description(
                    "history",
                    "History",
                    "Show commit history",
                    'H',
                    FocusContext::GitStatus,
                ),
            ]);
        } else if self.state.view_mode == ViewMode::Branches {
            commands.extend(vec![
                crate::plugin::PluginCommand::with_context_description(
                    "create-branch",
                    "New Branch",
                    "Create a new branch",
                    'n',
                    FocusContext::GitStatus,
                )
                .with_footer_priority(5),
                crate::plugin::PluginCommand::with_context_description(
                    "delete-branch",
                    "Delete Branch",
                    "Delete the selected branch",
                    'd',
                    FocusContext::GitStatus,
                )
                .with_footer_priority(3),
                crate::plugin::PluginCommand::with_context_description(
                    "status",
                    "Status",
                    "Switch to status view",
                    'S',
                    FocusContext::GitStatus,
                )
                .with_footer_priority(4),
                crate::plugin::PluginCommand::with_context_description(
                    "history",
                    "History",
                    "Show commit history",
                    'H',
                    FocusContext::GitStatus,
                ),
            ]);
        } else if self.state.view_mode == ViewMode::Stash {
            commands.extend(vec![
                crate::plugin::PluginCommand::with_context_description(
                    "save-stash",
                    "Save Stash",
                    "Stash current changes",
                    's',
                    FocusContext::GitStatus,
                )
                .with_footer_priority(5),
                crate::plugin::PluginCommand::with_context_description(
                    "drop-stash",
                    "Drop Stash",
                    "Drop the selected stash",
                    'd',
                    FocusContext::GitStatus,
                )
                .with_footer_priority(3),
                crate::plugin::PluginCommand::with_context_description(
                    "status",
                    "Status",
                    "Switch to status view",
                    'S',
                    FocusContext::GitStatus,
                )
                .with_footer_priority(4),
                crate::plugin::PluginCommand::with_context_description(
                    "history",
                    "History",
                    "Show commit history",
                    'H',
                    FocusContext::GitStatus,
                ),
            ]);
        } else {
            // Status/Diff mode shortcuts
            if !self.state.files.is_empty() {
                commands.extend(vec![
                    crate::plugin::PluginCommand::with_context_description(
                        "stage",
                        "Stage",
                        "Stage the selected file",
                        's',
                        FocusContext::GitStatus,
                    )
                    .with_footer_priority(6),
                    crate::plugin::PluginCommand::with_context_description(
                        "unstage",
                        "Unstage",
                        "Unstage the selected file",
                        'u',
                        FocusContext::GitStatus,
                    )
                    .with_footer_priority(5),
                    crate::plugin::PluginCommand::with_context_description(
                        "diff",
                        "Diff",
                        "Toggle file diff view",
                        'd',
                        FocusContext::GitStatus,
                    )
                    .with_footer_priority(3),
                ]);
            }
            commands.extend(vec![
                crate::plugin::PluginCommand::with_context_description(
                    "history",
                    "History",
                    "Show commit history",
                    'H',
                    FocusContext::GitStatus,
                ),
                crate::plugin::PluginCommand::with_context_description(
                    "commit",
                    "Commit",
                    "Create a commit from staged changes",
                    'c',
                    FocusContext::GitStatus,
                )
                .with_footer_priority(4),
            ]);
        }

        // Common shortcuts
        commands.extend(vec![
            crate::plugin::PluginCommand::with_context_description(
                "refresh",
                "Refresh Git Status",
                "Reload repository status from disk",
                'r',
                FocusContext::GitStatus,
            )
            .with_footer_priority(1),
            crate::plugin::PluginCommand::with_context_description(
                "branches",
                "Branches",
                "Show local branches",
                'B',
                FocusContext::GitStatus,
            ),
            crate::plugin::PluginCommand::with_context_description(
                "stash",
                "Stash",
                "Show git stashes",
                'Z',
                FocusContext::GitStatus,
            ),
            crate::plugin::PluginCommand::with_context_description(
                "push",
                "Push",
                "Push the current branch",
                'P',
                FocusContext::GitStatus,
            )
            .with_footer_priority(2),
        ]);

        commands
    }

    fn status_line(&self) -> Option<String> {
        Some(self.status_info())
    }

    fn focus_context(&self) -> FocusContext {
        self.focus_context()
    }

    fn consumes_text_input(&self) -> bool {
        self.state.modal_active
    }

    async fn update(&mut self) -> Result<()> {
        while let Some(cmd) = self.pending_commands.pop_front() {
            if let Err(e) = self.execute_internal(cmd).await {
                tracing::warn!("Failed to execute git command: {}", e);
            }
        }

        // Load commit details if there's a pending hash
        if let Some(hash) = self.pending_commit_hash.take() {
            if let Err(e) = self.load_commit_details(&hash).await {
                tracing::warn!("Failed to load commit details: {}", e);
            }
        }

        // Load diff for the selected file if pending
        if let Some((path, status)) = self.pending_diff_path.take() {
            match self
                .git_service
                .diff_file(&self.repo_path, &path, status)
                .await
            {
                Ok(diff) => {
                    self.state.diff = Some(diff);
                }
                Err(e) => {
                    tracing::debug!("Failed to load diff for {}: {}", path.display(), e);
                    self.state.diff = None;
                }
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
    use crate::core::models::{Branch, Commit, FileChange, FileStatus, Stash};
    use chrono::Utc;

    fn test_commit(hash: &str) -> Commit {
        Commit::new(hash, "subject", "author", Utc::now())
    }

    fn repo_status_with_unstaged(path: &str) -> RepoStatus {
        RepoStatus {
            branch: "main".to_string(),
            is_dirty: true,
            unstaged: vec![FileChange::new(path, FileStatus::Modified)],
            ..RepoStatus::default()
        }
    }

    fn test_branch(name: &str, is_current: bool) -> Branch {
        Branch {
            name: name.to_string(),
            full_name: format!("refs/heads/{}", name),
            is_current,
            is_remote: false,
            remote: None,
            commit_hash: "abc123".to_string(),
            upstream: None,
            ahead: None,
            behind: None,
        }
    }

    fn test_stash(index: usize, message: &str) -> Stash {
        Stash {
            index,
            message: message.to_string(),
            commit_hash: "def456".to_string(),
            date: None,
            branch: None,
        }
    }

    #[test]
    fn test_plugin_new() {
        let plugin = GitStatusPlugin::new();
        assert_eq!(plugin.id(), "git-status");
        assert_eq!(plugin.name(), "Git Status");
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

        // Check that we have navigation commands
        let has_nav = commands
            .iter()
            .any(|c| matches!(c.id.as_str(), "nav-down" | "nav-up"));
        let refresh_command = commands
            .iter()
            .find(|command| command.id == "refresh")
            .expect("git status refresh command");

        assert!(has_nav);
        assert_eq!(refresh_command.name, "Refresh Git Status");
        assert_eq!(
            refresh_command.description,
            "Reload repository status from disk"
        );
        assert_eq!(refresh_command.key, 'r');
    }

    #[test]
    fn test_status_mode_commands_prioritize_footer_actions() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.files = vec![FileChange::new("src/main.rs", FileStatus::Modified)];

        let commands = plugin.commands();

        let prioritized: Vec<(&str, u8)> = commands
            .iter()
            .filter(|command| command.priority > 0)
            .map(|command| (command.id.as_str(), command.priority))
            .collect();

        assert_eq!(
            prioritized,
            vec![
                ("stage", 6),
                ("unstage", 5),
                ("diff", 3),
                ("commit", 4),
                ("refresh", 1),
                ("push", 2)
            ]
        );
    }

    #[test]
    fn test_handle_key_navigation() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state_machine.initialize(3, ViewMode::Status);

        // Test j key (next)
        let commands = plugin.handle_key("j");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], Command::Refresh);

        // Test k key (prev)
        let commands = plugin.handle_key("k");
        assert!(commands.is_empty() || (commands.len() == 1 && commands[0] == Command::Refresh));

        // Test h key (sidebar focus)
        let commands = plugin.handle_key("h");
        assert!(commands.is_empty());
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
    fn test_history_navigation_uses_state_machine() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::History;
        plugin.state.commits = vec![
            test_commit("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            test_commit("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            test_commit("cccccccccccccccccccccccccccccccccccccccc"),
        ];
        plugin.sync_state_machine();

        let commands = plugin.handle_key("j");
        assert_eq!(commands, vec![Command::Refresh]);
        assert_eq!(plugin.state.selected_commit, Some(0));
        assert_eq!(
            plugin.pending_commit_hash.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );

        let commands = plugin.handle_key("j");
        assert_eq!(commands, vec![Command::Refresh]);
        assert_eq!(plugin.state.selected_commit, Some(1));
        assert_eq!(
            plugin.pending_commit_hash.as_deref(),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );

        let commands = plugin.handle_key("k");
        assert_eq!(commands, vec![Command::Refresh]);
        assert_eq!(plugin.state.selected_commit, Some(0));
        assert_eq!(
            plugin.pending_commit_hash.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
    }

    #[test]
    fn test_stage_without_selection_is_blocked_by_guard() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Status;
        plugin.state.files = vec![FileChange::new("src/main.rs", FileStatus::Modified)];
        plugin.state.selected_file = None;
        plugin.sync_state_machine();

        let commands = plugin.handle_key("s");
        assert!(commands.is_empty());
    }

    #[test]
    fn test_apply_repo_status_selects_first_file_and_queues_diff() {
        let mut plugin = GitStatusPlugin::new();

        plugin.apply_repo_status(repo_status_with_unstaged("src/main.rs"));

        assert_eq!(plugin.state.selected_file, Some(0));
        assert_eq!(
            plugin.pending_diff_path,
            Some((PathBuf::from("src/main.rs"), FileStatus::Modified))
        );
        assert_eq!(plugin.state_machine.selected_index(), Some(0));
    }

    #[tokio::test]
    async fn test_switch_to_status_queues_selected_file_diff_after_validation() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::History;
        plugin.state.files = vec![FileChange::new("src/lib.rs", FileStatus::Modified)];
        plugin.state.selected_file = Some(99);

        plugin
            .execute_internal(Command::SwitchMode(ViewMode::Status))
            .await
            .unwrap();

        assert_eq!(plugin.state.selected_file, Some(0));
        assert_eq!(
            plugin.pending_diff_path,
            Some((PathBuf::from("src/lib.rs"), FileStatus::Modified))
        );
        assert_eq!(plugin.state_machine.selected_index(), Some(0));
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
        let ctx = PluginContext::new(
            PathBuf::from("."),
            PathBuf::from("."),
            PathBuf::from("."),
            Config::default(),
            std::sync::Arc::new(crate::event::Dispatcher::new()),
            tracing::info_span!("test"),
        );

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

    #[test]
    fn test_handle_key_b_loads_branches() {
        let mut plugin = GitStatusPlugin::new();
        let commands = plugin.handle_key("B");
        assert!(commands.contains(&Command::LoadBranches));
        assert!(commands.contains(&Command::SwitchMode(ViewMode::Branches)));
    }

    #[test]
    fn test_handle_key_z_loads_stashes() {
        let mut plugin = GitStatusPlugin::new();
        let commands = plugin.handle_key("Z");
        assert!(commands.contains(&Command::LoadStashes));
        assert!(commands.contains(&Command::SwitchMode(ViewMode::Stash)));
    }

    #[test]
    fn test_handle_key_p_push() {
        let mut plugin = GitStatusPlugin::new();
        let commands = plugin.handle_key("P");
        assert_eq!(commands, vec![Command::PushToRemote]);
    }

    #[test]
    fn test_branch_navigation() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Branches;
        plugin.state.branches = vec![
            test_branch("main", true),
            test_branch("feature", false),
            test_branch("develop", false),
        ];

        // Navigate down
        let commands = plugin.handle_key("j");
        assert_eq!(commands, vec![Command::Refresh]);
        assert_eq!(plugin.state.selected_branch, Some(0));

        let commands = plugin.handle_key("j");
        assert_eq!(commands, vec![Command::Refresh]);
        assert_eq!(plugin.state.selected_branch, Some(1));

        // Navigate up
        let commands = plugin.handle_key("k");
        assert_eq!(commands, vec![Command::Refresh]);
        assert_eq!(plugin.state.selected_branch, Some(0));

        // Jump to last
        plugin.handle_key("G");
        assert_eq!(plugin.state.selected_branch, Some(2));

        // Jump to first
        plugin.handle_key("g");
        assert_eq!(plugin.state.selected_branch, Some(0));
    }

    #[test]
    fn test_branch_checkout_current_blocked() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Branches;
        plugin.state.branches = vec![test_branch("main", true)];
        plugin.state.selected_branch = Some(0);

        // Enter on current branch should do nothing
        let commands = plugin.handle_key("Enter");
        assert!(commands.is_empty());
    }

    #[test]
    fn test_branch_checkout_non_current() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Branches;
        plugin.state.branches = vec![test_branch("feature", false)];
        plugin.state.selected_branch = Some(0);

        let commands = plugin.handle_key("Enter");
        assert_eq!(
            commands,
            vec![Command::CheckoutBranch("feature".to_string())]
        );
    }

    #[test]
    fn test_branch_delete_opens_modal() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Branches;
        plugin.state.branches = vec![test_branch("feature", false)];
        plugin.state.selected_branch = Some(0);

        let commands = plugin.handle_key("d");
        assert_eq!(commands, vec![Command::Refresh]);
        assert!(plugin.state.modal_active);
        assert_eq!(
            plugin.state.active_modal,
            Some(GitModal::DeleteBranch {
                name: "feature".to_string()
            })
        );
    }

    #[test]
    fn test_stash_navigation() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Stash;
        plugin.state.stashes = vec![test_stash(0, "WIP: feature"), test_stash(1, "Backup")];

        let commands = plugin.handle_key("j");
        assert_eq!(commands, vec![Command::Refresh]);
        assert_eq!(plugin.state.selected_stash, Some(0));

        let commands = plugin.handle_key("j");
        assert_eq!(commands, vec![Command::Refresh]);
        assert_eq!(plugin.state.selected_stash, Some(1));
    }

    #[test]
    fn test_stash_pop() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Stash;
        plugin.state.stashes = vec![test_stash(0, "WIP")];
        plugin.state.selected_stash = Some(0);

        let commands = plugin.handle_key("Enter");
        assert_eq!(commands, vec![Command::StashPop(0)]);
    }

    #[test]
    fn test_stash_save() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Stash;

        let commands = plugin.handle_key("s");
        assert_eq!(commands, vec![Command::StashSave(None)]);
    }

    #[test]
    fn test_stash_drop_opens_modal() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Stash;
        plugin.state.stashes = vec![test_stash(0, "WIP")];
        plugin.state.selected_stash = Some(0);

        let commands = plugin.handle_key("d");
        assert_eq!(commands, vec![Command::Refresh]);
        assert!(plugin.state.modal_active);
        assert_eq!(
            plugin.state.active_modal,
            Some(GitModal::DropStash { index: 0 })
        );
    }

    #[test]
    fn test_modal_escape_closes() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.open_modal(GitModal::CommitMessage);
        assert!(plugin.state.modal_active);

        let commands = plugin.handle_key("Escape");
        assert_eq!(commands, vec![Command::Refresh]);
        assert!(!plugin.state.modal_active);
    }

    #[test]
    fn test_modal_enter_confirms_delete_branch() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.open_modal(GitModal::DeleteBranch {
            name: "old-branch".to_string(),
        });

        let commands = plugin.handle_key("Enter");
        assert_eq!(
            commands,
            vec![Command::DeleteBranch("old-branch".to_string())]
        );
        assert!(!plugin.state.modal_active);
    }

    #[test]
    fn test_modal_d_confirms_delete_branch() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.open_modal(GitModal::DeleteBranch {
            name: "old-branch".to_string(),
        });

        let commands = plugin.handle_key("d");
        assert_eq!(
            commands,
            vec![Command::DeleteBranch("old-branch".to_string())]
        );
        assert!(!plugin.state.modal_active);
    }

    #[test]
    fn test_modal_enter_confirms_drop_stash() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.open_modal(GitModal::DropStash { index: 2 });

        let commands = plugin.handle_key("Enter");
        assert_eq!(commands, vec![Command::StashDrop(2)]);
        assert!(!plugin.state.modal_active);
    }

    #[test]
    fn test_modal_d_confirms_drop_stash() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.open_modal(GitModal::DropStash { index: 2 });

        let commands = plugin.handle_key("D");
        assert_eq!(commands, vec![Command::StashDrop(2)]);
        assert!(!plugin.state.modal_active);
    }

    #[test]
    fn test_modal_d_does_not_confirm_text_modal() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.open_modal(GitModal::CommitMessage);

        let commands = plugin.handle_key("d");
        assert!(commands.is_empty());
        assert!(plugin.state.modal_active);
    }

    #[test]
    fn test_consumes_text_input_when_modal_active() {
        let mut plugin = GitStatusPlugin::new();
        assert!(!plugin.consumes_text_input());

        plugin.state.open_modal(GitModal::CommitMessage);
        assert!(plugin.consumes_text_input());

        plugin.state.close_modal();
        assert!(!plugin.consumes_text_input());
    }

    #[test]
    fn test_plugin_commands_include_branches_stash_push() {
        let plugin = GitStatusPlugin::new();
        let commands = plugin.commands();

        let has_branches = commands.iter().any(|c| c.id == "branches");
        let has_stash = commands.iter().any(|c| c.id == "stash");
        let has_push = commands.iter().any(|c| c.id == "push");

        assert!(has_branches);
        assert!(has_stash);
        assert!(has_push);
    }

    #[test]
    fn test_branch_mode_commands_include_branch_actions() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Branches;

        let commands = plugin.commands();

        assert!(
            commands
                .iter()
                .any(|c| c.id == "create-branch" && c.name == "New Branch" && c.key == 'n')
        );
        assert!(
            commands
                .iter()
                .any(|c| c.id == "delete-branch" && c.name == "Delete Branch" && c.key == 'd')
        );
        assert!(
            commands
                .iter()
                .any(|c| c.id == "status" && c.name == "Status" && c.key == 'S')
        );
    }

    #[test]
    fn test_stash_mode_commands_include_stash_actions() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Stash;

        let commands = plugin.commands();

        assert!(
            commands
                .iter()
                .any(|c| c.id == "save-stash" && c.name == "Save Stash" && c.key == 's')
        );
        assert!(
            commands
                .iter()
                .any(|c| c.id == "drop-stash" && c.name == "Drop Stash" && c.key == 'd')
        );
        assert!(
            commands
                .iter()
                .any(|c| c.id == "status" && c.name == "Status" && c.key == 'S')
        );
    }

    #[test]
    fn test_execute_delete_branch_command_uses_declared_shortcut() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Branches;
        plugin.state.branches = vec![test_branch("feature", false)];
        plugin.state.selected_branch = Some(0);

        let execution = plugin
            .execute_command("delete-branch")
            .expect("delete branch command should execute");

        assert_eq!(execution.command_name, "Delete Branch");
        assert!(execution.emitted_commands.is_empty());
        assert_eq!(plugin.pending_commands.pop_front(), Some(Command::Refresh));
        assert_eq!(
            plugin.state.active_modal,
            Some(GitModal::DeleteBranch {
                name: "feature".to_string()
            })
        );
    }

    #[test]
    fn test_execute_refresh_command_uses_declared_shortcut() {
        let mut plugin = GitStatusPlugin::new();

        let execution = plugin
            .execute_command("refresh")
            .expect("refresh command should execute");

        assert_eq!(execution.command_name, "Refresh Git Status");
        assert!(execution.emitted_commands.is_empty());
        assert_eq!(plugin.pending_commands.pop_front(), Some(Command::Refresh));
    }

    #[test]
    fn test_execute_save_stash_command_uses_declared_shortcut() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Stash;

        let execution = plugin
            .execute_command("save-stash")
            .expect("save stash command should execute");

        assert_eq!(execution.command_name, "Save Stash");
        assert!(execution.emitted_commands.is_empty());
        assert_eq!(
            plugin.pending_commands.pop_front(),
            Some(Command::StashSave(None))
        );
    }

    #[test]
    fn test_branch_mode_switch_to_status() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Branches;

        let commands = plugin.handle_key("S");
        assert_eq!(commands, vec![Command::SwitchMode(ViewMode::Status)]);
    }

    #[test]
    fn test_stash_mode_switch_to_history() {
        let mut plugin = GitStatusPlugin::new();
        plugin.state.view_mode = ViewMode::Stash;

        let commands = plugin.handle_key("H");
        assert!(commands.contains(&Command::SwitchMode(ViewMode::History)));
        assert!(commands.contains(&Command::LoadCommits));
    }
}
