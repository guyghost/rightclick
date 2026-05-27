//! Workers Plugin
//!
//! This module implements the Workers plugin for RightClick, providing
//! a TUI interface for managing AI agent workflows (Intents).

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use anyhow::Result;
use ratatui::{buffer::Buffer, layout::Rect};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::core::logic::{build_intent_from_spec, generate_default_spec, parse_spec_document};
use crate::core::models::Theme;
use crate::core::models::intent::{
    Intent, IntentStatus, Worker, WorkerId, WorkerStatus, WorkerType,
};
use crate::event::Event;
use crate::keymap::registry::KeyBindingRegistry;
use crate::keymap::{Action, FocusContext};

use super::render::{render_workers, render_workers_status};
use super::runner::{WorkerOutput, WorkerRunner};
use super::state::{FocusPane, ModalState, PluginState, PreviewTab, ViewMode};

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
    /// Create a new intent
    CreateIntent { title: String },
    /// Delete an intent
    DeleteIntent(String),
    /// Run workers for selected intent
    RunWorkers(String),
    /// Stop all running workers
    StopWorkers,
    /// Open intent spec for editing
    OpenIntent(String),
    /// Show intent details
    ShowIntent(String),
    /// Switch preview tab
    SwitchTab(PreviewTab),
    /// Emit an event
    EmitEvent(Event),
}

/// Context passed to workers plugin during initialization
#[derive(Debug, Clone)]
pub struct WorkersPluginContext {
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

/// The main Workers plugin struct
#[derive(Debug)]
pub struct WorkersPlugin {
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
    config: crate::core::models::WorkersPluginConfig,
    /// Worker runner
    worker_runner: WorkerRunner,
    /// Output receivers for running workers
    output_receivers: HashMap<String, mpsc::Receiver<WorkerOutput>>,
    /// Commands queued from input handling and executed in update()
    pending_commands: VecDeque<Command>,
}

impl WorkersPlugin {
    /// Create a new Workers plugin
    pub fn new() -> Self {
        let mut key_bindings = KeyBindingRegistry::new();
        Self::register_default_bindings(&mut key_bindings);

        let intents_dir = PathBuf::from(super::DEFAULT_INTENTS_DIR);
        let logs_dir = PathBuf::from(super::DEFAULT_LOGS_DIR);

        Self {
            state: PluginState::new(intents_dir, logs_dir),
            repo_path: PathBuf::new(),
            focused: false,
            focus_pane: FocusPane::default(),
            key_bindings,
            config: crate::core::models::WorkersPluginConfig::default(),
            worker_runner: WorkerRunner::new(),
            output_receivers: HashMap::new(),
            pending_commands: VecDeque::new(),
        }
    }

    /// Create a new plugin with custom configuration
    pub fn with_config(config: crate::core::models::WorkersPluginConfig) -> Self {
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

        // Intent operations
        registry.bind("n", Action::NewFile, FocusContext::Workspace); // Create intent
        registry.bind("D", Action::Delete, FocusContext::Workspace); // Delete intent
        registry.bind("f", Action::Filter, FocusContext::Workspace); // Refresh intents
        registry.bind("r", Action::Refresh, FocusContext::Workspace); // Run workers
        // Stop workers is handled directly by the plugin because Action has no Stop variant.
        registry.bind("enter", Action::Enter, FocusContext::Workspace); // Open intent

        // Note: Preview tab switching uses [ and ] keys (handled in handle_event)
        // to avoid conflict with global navigation keys 1-5

        // Pane switching
        registry.bind("left", Action::NavigateLeft, FocusContext::Workspace);
        registry.bind("right", Action::NavigateRight, FocusContext::Workspace);
    }

    /// Get the plugin ID.
    pub fn id(&self) -> &str {
        super::PLUGIN_ID
    }

    /// Get the plugin name
    pub fn name(&self) -> &str {
        "Workers"
    }

    /// Get the plugin icon
    pub fn icon(&self) -> char {
        '🤖'
    }

    /// Initialize the plugin
    pub async fn init_with_context(&mut self, ctx: &WorkersPluginContext) -> Result<()> {
        self.repo_path = ctx.project_root.clone();
        self.config = ctx.config.plugins.workers.clone();

        self.state.intents_dir = PathBuf::from(&self.config.intents_dir);
        self.state.logs_dir = PathBuf::from(&self.config.logs_dir);

        // Update paths to be absolute
        self.state.intents_dir = self.repo_path.join(&self.state.intents_dir);
        self.state.logs_dir = self.repo_path.join(&self.state.logs_dir);

        // Ensure directories exist
        tokio::fs::create_dir_all(&self.state.intents_dir)
            .await
            .ok();
        tokio::fs::create_dir_all(&self.state.logs_dir).await.ok();

        // Initial load
        self.load_intents().await?;

        Ok(())
    }

    /// Load all intents from the intents directory
    async fn load_intents(&mut self) -> Result<()> {
        self.state.intents.clear();

        let mut entries = tokio::fs::read_dir(&self.state.intents_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                match self.load_intent_from_file(&path).await {
                    Ok(intent) => {
                        self.state.add_intent(intent);
                    }
                    Err(e) => {
                        warn!("Failed to load intent from {:?}: {}", path, e);
                    }
                }
            }
        }

        // Sort intents by creation date (newest first)
        self.state
            .intents
            .sort_by(|a, b| b.intent.created_at.cmp(&a.intent.created_at));

        // Select first intent if none selected
        if self.state.selected_intent.is_none() && !self.state.intents.is_empty() {
            self.state.selected_intent = Some(0);
        }

        info!("Loaded {} intents", self.state.intents.len());
        Ok(())
    }

    /// Load a single intent from a file
    async fn load_intent_from_file(&self, path: &PathBuf) -> Result<Intent> {
        let content = tokio::fs::read_to_string(path).await?;
        let doc = parse_spec_document(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse spec: {}", e))?;
        let intent = build_intent_from_spec(doc, path.clone())
            .map_err(|e| anyhow::anyhow!("Failed to build intent: {}", e))?;
        Ok(intent)
    }

    /// Shutdown the plugin
    pub async fn shutdown(&mut self) -> Result<()> {
        // Stop all running workers
        self.worker_runner.stop_all().await;
        Ok(())
    }

    /// Refresh the plugin state
    pub async fn refresh(&mut self) -> Result<()> {
        self.load_intents().await?;
        self.worker_runner.cleanup_completed().await;
        Ok(())
    }

    /// Create a new intent
    pub async fn create_intent(&mut self, title: &str) -> Result<Intent> {
        // Generate slug from title
        let slug = title
            .to_lowercase()
            .replace(' ', "-")
            .replace(|c: char| !c.is_alphanumeric() && c != '-', "");

        let spec_path = self.state.intents_dir.join(format!("{}.md", slug));

        // Check if file already exists
        if spec_path.exists() {
            return Err(anyhow::anyhow!("Intent '{}' already exists", slug));
        }

        // Generate spec content
        let now = chrono::Utc::now().to_rfc3339();
        let spec_content = generate_default_spec(title, &now);

        // Write file
        tokio::fs::write(&spec_path, spec_content).await?;

        // Load the new intent
        let intent = self.load_intent_from_file(&spec_path).await?;
        self.state.add_intent(intent.clone());

        info!("Created new intent: {} at {:?}", title, spec_path);
        Ok(intent)
    }

    /// Delete an intent
    pub async fn delete_intent(&mut self, intent_id: &str) -> Result<()> {
        if let Some(intent) = self.state.remove_intent(intent_id) {
            // Delete the spec file
            tokio::fs::remove_file(&intent.spec_path).await?;
            info!("Deleted intent: {}", intent_id);
        }
        Ok(())
    }

    /// Run workers for an intent
    pub async fn run_workers(&mut self, intent_id: &str) -> Result<()> {
        let intent = self
            .state
            .get_intent(intent_id)
            .ok_or_else(|| anyhow::anyhow!("Intent not found: {}", intent_id))?
            .intent
            .clone();

        // Read the spec content
        let spec_content = tokio::fs::read_to_string(&intent.spec_path).await?;

        // Create default workers if none exist
        if self.state.get_intent_workers(intent_id).is_empty() {
            self.create_default_workers(intent_id).await?;
        }

        // Start pending workers
        let workers_to_start: Vec<Worker> = self
            .state
            .workers
            .values()
            .filter(|e| {
                e.worker.intent_id == intent_id
                    && e.worker.status == WorkerStatus::Pending
                    && e.worker.can_start(&self.get_completed_worker_ids())
            })
            .map(|e| e.worker.clone())
            .collect();

        for worker in workers_to_start {
            self.start_worker(&worker, &spec_content).await?;
        }

        // Update intent status
        if let Some(entry) = self.state.get_intent_mut(intent_id) {
            entry.intent.status = IntentStatus::InProgress;
        }

        Ok(())
    }

    /// Create default workers for an intent (sequential workflow)
    async fn create_default_workers(&mut self, intent_id: &str) -> Result<()> {
        let intent = self
            .state
            .get_intent(intent_id)
            .ok_or_else(|| anyhow::anyhow!("Intent not found: {}", intent_id))?
            .intent
            .clone();

        let now = chrono::Utc::now().to_rfc3339();
        let branch = sanitize_branch_name(&intent.title);
        let agent = self.config.default_agent.clone();

        // Create workers in sequence: Investigator -> Implementer -> Verifier
        let investigator = Worker::new(
            "investigate",
            WorkerType::Investigator,
            intent_id,
            self.repo_path.clone(),
            &branch,
            agent.clone(),
            self.state
                .logs_dir
                .join(format!("{}-investigator.log", intent_id)),
            &now,
        );

        let implementer = Worker::new(
            "implement",
            WorkerType::Implementer,
            intent_id,
            self.repo_path.clone(),
            &branch,
            agent.clone(),
            self.state
                .logs_dir
                .join(format!("{}-implementer.log", intent_id)),
            &now,
        )
        .depends_on(investigator.id.clone());

        let verifier = Worker::new(
            "verify",
            WorkerType::Verifier,
            intent_id,
            self.repo_path.clone(),
            &branch,
            agent,
            self.state
                .logs_dir
                .join(format!("{}-verifier.log", intent_id)),
            &now,
        )
        .depends_on(implementer.id.clone());

        self.state.add_worker(investigator);
        self.state.add_worker(implementer);
        self.state.add_worker(verifier);

        info!("Created default workers for intent: {}", intent_id);
        Ok(())
    }

    /// Start a single worker
    async fn start_worker(&mut self, worker: &Worker, spec_content: &str) -> Result<()> {
        let rx = self
            .worker_runner
            .spawn_worker(worker, spec_content)
            .await?;
        self.output_receivers.insert(worker.id.clone(), rx);

        // Update worker status
        if let Some(entry) = self.state.get_worker_mut(&worker.id) {
            entry.worker.mark_running();
            entry.streaming = true;
        }

        info!("Started worker: {} ({:?})", worker.id, worker.worker_type);
        Ok(())
    }

    /// Get IDs of completed workers
    fn get_completed_worker_ids(&self) -> Vec<WorkerId> {
        self.state
            .workers
            .values()
            .filter(|e| e.worker.status == WorkerStatus::Completed)
            .map(|e| e.worker.id.clone())
            .collect()
    }

    /// Stop all running workers
    pub async fn stop_workers(&mut self) -> Result<()> {
        self.worker_runner.stop_all().await;
        self.output_receivers.clear();
        Ok(())
    }

    /// Process output from running workers
    async fn process_worker_output(&mut self) {
        let mut completed_workers = Vec::new();
        let mut workers_to_start = Vec::new();

        // Collect output first to avoid borrow issues
        let output: Vec<(String, Vec<WorkerOutput>)> = self
            .output_receivers
            .iter_mut()
            .map(|(id, rx)| {
                let mut outputs = Vec::new();
                while let Ok(output) = rx.try_recv() {
                    outputs.push(output);
                }
                (id.clone(), outputs)
            })
            .collect();

        for (worker_id, outputs) in output {
            for output in outputs {
                match output {
                    WorkerOutput::Stdout(line) => {
                        if let Some(entry) = self.state.get_worker_mut(&worker_id) {
                            entry.append_output(line);
                        }
                    }
                    WorkerOutput::Stderr(line) => {
                        if let Some(entry) = self.state.get_worker_mut(&worker_id) {
                            entry.append_output(format!("[stderr] {}", line));
                        }
                    }
                    WorkerOutput::Exited(code) => {
                        let now = chrono::Utc::now().to_rfc3339();
                        if let Some(entry) = self.state.get_worker_mut(&worker_id) {
                            entry.streaming = false;
                            if code == 0 {
                                entry.worker.mark_completed(&now);
                            } else {
                                entry.worker.mark_failed(&now, code);
                            }
                        }
                        completed_workers.push(worker_id.clone());

                        // Collect intent IDs to start next workers
                        if let Some(intent_id) = self.state.get_intent_id_for_worker(&worker_id) {
                            workers_to_start.push(intent_id);
                        }
                    }
                }
            }
        }

        // Clean up completed receivers
        for worker_id in completed_workers {
            self.output_receivers.remove(&worker_id);
            self.worker_runner.mark_completed(&worker_id);
        }

        // Start next workers after releasing borrows
        for intent_id in workers_to_start {
            if let Err(e) = self.run_workers(&intent_id).await {
                warn!("Failed to start next workers: {}", e);
            }
        }
    }

    async fn process_pending_commands(&mut self) {
        while let Some(command) = self.pending_commands.pop_front() {
            let result = match command {
                Command::None
                | Command::SwitchMode(_)
                | Command::SwitchFocus(_)
                | Command::SwitchTab(_)
                | Command::ShowIntent(_)
                | Command::EmitEvent(_) => Ok(()),
                Command::Refresh => self.refresh().await,
                Command::CreateIntent { title } => self.create_intent(&title).await.map(|_| ()),
                Command::DeleteIntent(intent_id) => self.delete_intent(&intent_id).await,
                Command::RunWorkers(intent_id) => self.run_workers(&intent_id).await,
                Command::StopWorkers => self.stop_workers().await,
                Command::OpenIntent(intent_id) => {
                    if let Some(idx) = self
                        .state
                        .intents
                        .iter()
                        .position(|intent| intent.intent.id == intent_id)
                    {
                        self.state.selected_intent = Some(idx);
                        self.state.preview_tab = PreviewTab::Spec;
                    }
                    Ok(())
                }
            };

            if let Err(e) = result {
                warn!("Failed to execute workers command: {}", e);
            }
        }
    }

    /// Handle keyboard input events
    pub fn handle_event_internal(&mut self, event: Event) -> Vec<Command> {
        let mut commands = Vec::new();

        match event {
            Event::GitChanged | Event::RefreshNeeded => {
                commands.push(Command::Refresh);
            }
            Event::Key { code, modifiers } => {
                if self.state.is_modal_open() && !modifiers.ctrl && !modifiers.alt {
                    match self.state.modal_state {
                        ModalState::CreateIntent => match code.as_str() {
                            "Esc" => self.state.close_modal(),
                            "Backspace" => self.handle_backspace(),
                            "Enter" => {
                                self.confirm_create_intent(&mut commands);
                            }
                            _ => {
                                if code.len() == 1 {
                                    if let Some(c) = code.chars().next() {
                                        self.handle_char(c);
                                    }
                                }
                            }
                        },
                        ModalState::DeleteConfirm => match code.as_str() {
                            "Esc" => self.state.close_modal(),
                            "Enter" | "Delete" | "D" => {
                                if let Some(intent) = self.state.selected_intent() {
                                    commands.push(Command::DeleteIntent(intent.intent.id.clone()));
                                }
                                self.state.close_modal();
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                    return commands;
                }

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
                            if self.state.view_mode == ViewMode::Kanban {
                                let groups = self.state.workers_by_status();
                                let col = self.state.kanban_state.focused_column;
                                let count = match col {
                                    0 => groups.pending.len(),
                                    1 => groups.running.len(),
                                    2 => groups.completed.len(),
                                    3 => groups.failed.len(),
                                    _ => 0,
                                };
                                self.state.kanban_state.next_row(count);
                            } else if self.focus_pane == FocusPane::Sidebar {
                                self.state.select_next();
                            }
                        }
                        "k" | "Up" => {
                            if self.state.view_mode == ViewMode::Kanban {
                                self.state.kanban_state.prev_row();
                            } else if self.focus_pane == FocusPane::Sidebar {
                                self.state.select_prev();
                            }
                        }
                        "h" | "Left" => {
                            if self.state.view_mode == ViewMode::Kanban {
                                self.state.kanban_state.prev_column();
                            } else if self.focus_pane == FocusPane::Preview {
                                self.focus_pane = FocusPane::Sidebar;
                                commands.push(Command::SwitchFocus(FocusPane::Sidebar));
                            }
                        }
                        "l" | "Right" => {
                            if self.state.view_mode == ViewMode::Kanban {
                                self.state.kanban_state.next_column();
                            } else if self.focus_pane == FocusPane::Sidebar {
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
                                ViewMode::Kanban => ViewMode::Detail,
                                ViewMode::Detail => ViewMode::List,
                            };
                            self.state.view_mode = new_mode;
                            commands.push(Command::SwitchMode(new_mode));
                        }
                        "]" => {
                            // Cycle to next preview tab (Spec -> Output -> Criteria -> Spec)
                            let new_tab = match self.state.preview_tab {
                                PreviewTab::Spec => PreviewTab::Output,
                                PreviewTab::Output => PreviewTab::Criteria,
                                PreviewTab::Criteria => PreviewTab::Spec,
                            };
                            self.state.preview_tab = new_tab;
                            commands.push(Command::Refresh);
                        }
                        "[" => {
                            // Cycle to previous preview tab (Spec <- Output <- Criteria <- Spec)
                            let new_tab = match self.state.preview_tab {
                                PreviewTab::Spec => PreviewTab::Criteria,
                                PreviewTab::Output => PreviewTab::Spec,
                                PreviewTab::Criteria => PreviewTab::Output,
                            };
                            self.state.preview_tab = new_tab;
                            commands.push(Command::Refresh);
                        }
                        "n" => {
                            // Create new intent
                            self.state.modal_state = ModalState::CreateIntent;
                        }
                        "f" => {
                            // Refresh intents from disk
                            commands.push(Command::Refresh);
                        }
                        "D" => {
                            // Delete intent
                            if self.state.selected_intent().is_some() {
                                self.state.modal_state = ModalState::DeleteConfirm;
                            }
                        }
                        "r" => {
                            // Run workers
                            if let Some(intent) = self.state.selected_intent() {
                                commands.push(Command::RunWorkers(intent.intent.id.clone()));
                            }
                        }
                        "s" => {
                            // Stop workers
                            commands.push(Command::StopWorkers);
                        }
                        "Enter" => {
                            if self.state.view_mode == ViewMode::Kanban {
                                // Transition worker status in kanban view
                                self.kanban_transition_worker();
                            } else {
                                // Open intent
                                if let Some(intent) = self.state.selected_intent() {
                                    commands.push(Command::OpenIntent(intent.intent.id.clone()));
                                }
                            }
                        }
                        "o" => {
                            // Open intent, including from kanban where Enter is reserved
                            // for worker status transitions.
                            if let Some(intent) = self.state.selected_intent() {
                                commands.push(Command::OpenIntent(intent.intent.id.clone()));
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

    /// Transition the focused worker's status in kanban view
    /// Pending -> Running, Failed -> Pending
    fn kanban_transition_worker(&mut self) {
        let groups = self.state.workers_by_status();
        let col = self.state.kanban_state.focused_column;
        let row = self.state.kanban_state.focused_row;

        let worker_id = match col {
            0 => groups.pending.get(row).cloned(),
            3 => groups.failed.get(row).cloned(),
            _ => None,
        };

        if let Some(id) = worker_id {
            if let Some(entry) = self.state.get_worker_mut(&id) {
                match entry.worker.status {
                    WorkerStatus::Pending => {
                        entry.worker.mark_running();
                    }
                    WorkerStatus::Failed => {
                        entry.worker.status = WorkerStatus::Pending;
                        entry.worker.exit_code = None;
                        entry.worker.completed_at = None;
                    }
                    _ => {}
                }
            }
        }
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
                }
            }
            Action::NavigateUp => {
                if self.focus_pane == FocusPane::Sidebar {
                    self.state.select_prev();
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
                    ViewMode::Kanban => ViewMode::Detail,
                    ViewMode::Detail => ViewMode::List,
                };
                self.state.view_mode = new_mode;
                commands.push(Command::SwitchMode(new_mode));
            }
            Action::SwitchTab(tab) => {
                self.state.preview_tab = match tab {
                    0 => PreviewTab::Spec,
                    1 => PreviewTab::Output,
                    2 => PreviewTab::Criteria,
                    _ => self.state.preview_tab,
                };
                commands.push(Command::Refresh);
            }
            Action::NewFile => {
                self.state.modal_state = ModalState::CreateIntent;
            }
            Action::Delete => {
                if self.state.selected_intent().is_some() {
                    self.state.modal_state = ModalState::DeleteConfirm;
                }
            }
            Action::Refresh => {
                if let Some(intent) = self.state.selected_intent() {
                    commands.push(Command::RunWorkers(intent.intent.id.clone()));
                }
            }
            Action::Filter => {
                commands.push(Command::Refresh);
            }
            Action::Enter => {
                if let Some(intent) = self.state.selected_intent() {
                    commands.push(Command::OpenIntent(intent.intent.id.clone()));
                }
            }
            _ => {}
        }

        commands
    }

    /// Handle actions when a modal is open
    fn handle_modal_action(&mut self, action: &Action, commands: &mut Vec<Command>) {
        match self.state.modal_state {
            ModalState::CreateIntent => match action {
                Action::Confirm => {
                    self.confirm_create_intent(commands);
                }
                Action::Cancel => {
                    self.state.close_modal();
                }
                _ => {}
            },
            ModalState::DeleteConfirm => match action {
                Action::Confirm | Action::Delete => {
                    if let Some(intent) = self.state.selected_intent() {
                        commands.push(Command::DeleteIntent(intent.intent.id.clone()));
                    }
                    self.state.close_modal();
                }
                Action::Cancel => {
                    self.state.close_modal();
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn confirm_create_intent(&mut self, commands: &mut Vec<Command>) {
        let title = self.state.new_intent_title.trim().to_string();
        if title.is_empty() {
            return;
        }

        commands.push(Command::CreateIntent { title });
        self.state.close_modal();
    }

    /// Handle character input (for modal text entry)
    pub fn handle_char(&mut self, c: char) -> Vec<Command> {
        if !self.state.is_modal_open() {
            return Vec::new();
        }

        if self.state.modal_state == ModalState::CreateIntent
            && (c.is_alphanumeric() || c.is_whitespace() || c == '-' || c == '_')
        {
            self.state.new_intent_title.push(c);
        }

        Vec::new()
    }

    /// Handle backspace
    pub fn handle_backspace(&mut self) {
        if !self.state.is_modal_open() {
            return;
        }

        if self.state.modal_state == ModalState::CreateIntent {
            self.state.new_intent_title.pop();
        }
    }

    /// Render the plugin UI
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        render_workers(&self.state, self.focus_pane, self.focused, area, buf, theme);
    }

    /// Get available commands for the command palette
    pub fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::new("create", "New Intent", 'n'),
            PluginCommand::new("delete", "Delete", 'D'),
            PluginCommand::new("run", "Run Workers", 'r'),
            PluginCommand::new("stop", "Stop Workers", 's'),
            PluginCommand::new("open", "Open Intent", 'o'),
            PluginCommand::new("refresh", "Refresh intents", 'f'),
            PluginCommand::new("switch-view", "Switch View", 'v'),
            PluginCommand::new("prev-tab", "Previous Tab", '['),
            PluginCommand::new("next-tab", "Next Tab", ']'),
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
                FocusPane::Preview => FocusContext::Workspace,
            }
        }
    }

    /// Get status info for the footer
    pub fn status_info<'a>(&'a self, theme: &'a Theme) -> Vec<ratatui::text::Span<'a>> {
        render_workers_status(&self.state, theme)
    }

    /// Get a reference to the plugin state
    pub fn state(&self) -> &PluginState {
        &self.state
    }

    /// Get a mutable reference to the plugin state
    pub fn state_mut(&mut self) -> &mut PluginState {
        &mut self.state
    }
}

impl Default for WorkersPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Sanitize a string for use as a git branch name
fn sanitize_branch_name(name: &str) -> String {
    name.to_lowercase()
        .replace(' ', "-")
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "")
        .trim_matches('-')
        .to_string()
}

// Add helper method to PluginState
impl super::state::PluginState {
    /// Get the intent ID for a given worker ID
    fn get_intent_id_for_worker(&self, worker_id: &str) -> Option<String> {
        self.workers
            .get(worker_id)
            .map(|e| e.worker.intent_id.clone())
    }
}

fn workers_status_line(state: &PluginState) -> String {
    if state.intents.is_empty() {
        return format!(
            "No intents | n New | f Refresh intents | / Global search | ? Help | specs: {}",
            state.intents_dir.display()
        );
    }

    let running = state.running_workers_count();
    let completed = state.completed_workers_count();
    let failed = state
        .workers
        .values()
        .filter(|entry| {
            matches!(
                entry.worker.status,
                WorkerStatus::Failed | WorkerStatus::Cancelled
            )
        })
        .count();

    if running > 0 || failed > 0 {
        format!(
            "{} | {} | {} running | {} failed",
            count_label(state.intents.len(), "intent"),
            count_label(state.workers.len(), "worker"),
            running,
            failed
        )
    } else {
        format!(
            "{} | {} | {} completed",
            count_label(state.intents.len(), "intent"),
            count_label(state.workers.len(), "worker"),
            completed
        )
    }
}

fn count_label(count: usize, label: &str) -> String {
    if count == 1 {
        format!("1 {}", label)
    } else {
        format!("{} {}s", count, label)
    }
}

// ============================================================================
// Plugin Trait Implementation
// ============================================================================

use crate::plugin::{Command as PluginCmd, Plugin, PluginContext as PluginCtx};
use async_trait::async_trait;

#[async_trait]
impl Plugin for WorkersPlugin {
    fn id(&self) -> &str {
        super::PLUGIN_ID
    }

    fn name(&self) -> &str {
        "Workers"
    }

    fn icon(&self) -> char {
        '🤖'
    }

    async fn init(&mut self, ctx: &PluginCtx) -> anyhow::Result<()> {
        let plugin_ctx = WorkersPluginContext {
            project_root: ctx.project_root.clone(),
            config: ctx.config.clone(),
        };
        self.init_with_context(&plugin_ctx).await
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        // Note: Can't use async here, but we can trigger cleanup
        Ok(())
    }

    fn handle_event(&mut self, event: crate::event::Event) -> Vec<PluginCmd> {
        let commands = self.handle_event_internal(event);
        for command in commands {
            self.pending_commands.push_back(command);
        }
        Vec::new()
    }

    fn render(&self, area: Rect, buf: &mut Buffer, theme: &crate::core::models::Theme) {
        render_workers(&self.state, self.focus_pane, self.focused, area, buf, theme);
    }

    fn is_focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn commands(&self) -> Vec<crate::plugin::PluginCommand> {
        vec![
            crate::plugin::PluginCommand::with_context_description(
                "create",
                "New Intent",
                "Create a new implementation intent",
                'n',
                crate::keymap::FocusContext::Workspace,
            )
            .with_footer_priority(1),
            crate::plugin::PluginCommand::with_context_description(
                "delete",
                "Delete",
                "Delete the selected intent",
                'D',
                crate::keymap::FocusContext::Workspace,
            ),
            crate::plugin::PluginCommand::with_context_description(
                "run",
                "Run Workers",
                "Run workers for the selected intent",
                'r',
                crate::keymap::FocusContext::Workspace,
            )
            .with_footer_priority(4),
            crate::plugin::PluginCommand::with_context_description(
                "stop",
                "Stop Workers",
                "Stop running workers",
                's',
                crate::keymap::FocusContext::Workspace,
            )
            .with_footer_priority(2),
            crate::plugin::PluginCommand::with_context_description(
                "open",
                "Open Intent",
                "Open the selected intent",
                'o',
                crate::keymap::FocusContext::Workspace,
            )
            .with_footer_priority(3),
            crate::plugin::PluginCommand::with_context_description(
                "switch-view",
                "Switch View",
                "Switch workers view mode",
                'v',
                crate::keymap::FocusContext::Workspace,
            ),
            crate::plugin::PluginCommand::with_context_description(
                "prev-tab",
                "Previous Tab",
                "Switch to the previous tab",
                '[',
                crate::keymap::FocusContext::Workspace,
            ),
            crate::plugin::PluginCommand::with_context_description(
                "next-tab",
                "Next Tab",
                "Switch to the next tab",
                ']',
                crate::keymap::FocusContext::Workspace,
            ),
            crate::plugin::PluginCommand::with_context_description(
                "refresh",
                "Refresh intents",
                "Reload worker state",
                'f',
                crate::keymap::FocusContext::Workspace,
            ),
        ]
    }

    fn status_line(&self) -> Option<String> {
        Some(workers_status_line(&self.state))
    }

    fn search_entries(&self) -> Vec<crate::plugin::PluginSearchEntry> {
        self.state
            .intents
            .iter()
            .map(|entry| {
                let intent = &entry.intent;
                crate::plugin::PluginSearchEntry {
                    id: intent.id.clone(),
                    title: intent.title.clone(),
                    preview: format!(
                        "{} {} | {}% complete | {} workers | {}",
                        intent.status.icon(),
                        intent.status,
                        entry.completion_percentage(),
                        entry.worker_ids.len(),
                        intent.spec_path.display()
                    ),
                }
            })
            .collect()
    }

    fn activate_search_result(&mut self, entry_id: &str) -> bool {
        let Some(index) = self
            .state
            .intents
            .iter()
            .position(|entry| entry.intent.id == entry_id)
        else {
            return false;
        };

        self.state.selected_intent = Some(index);
        self.state.selected_worker = None;
        self.state.view_mode = ViewMode::List;
        self.state.preview_tab = PreviewTab::Spec;
        self.focus_pane = FocusPane::Preview;
        self.state.modal_state = ModalState::None;
        true
    }

    fn focus_context(&self) -> crate::keymap::FocusContext {
        crate::keymap::FocusContext::Workspace
    }

    async fn update(&mut self) -> anyhow::Result<()> {
        self.process_pending_commands().await;

        // Process worker output
        self.process_worker_output().await;

        // Cleanup completed workers
        self.worker_runner.cleanup_completed().await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_id() {
        let plugin = WorkersPlugin::new();
        assert_eq!(plugin.id(), "workers");
        assert_eq!(plugin.name(), "Workers");
        assert_eq!(plugin.icon(), '🤖');
    }

    #[test]
    fn test_plugin_commands() {
        let plugin = WorkersPlugin::new();
        let commands = plugin.commands();
        assert!(!commands.is_empty());
        assert!(commands.iter().any(|c| c.id == "create"));
        assert!(commands.iter().any(|c| c.id == "run"));

        let open_command = commands
            .iter()
            .find(|command| command.id == "open")
            .expect("workers open command");
        assert_eq!(open_command.name, "Open Intent");
        assert_eq!(open_command.key, 'o');

        let switch_view_command = commands
            .iter()
            .find(|command| command.id == "switch-view")
            .expect("workers switch view command");
        assert_eq!(switch_view_command.name, "Switch View");
        assert_eq!(switch_view_command.key, 'v');

        let previous_tab_command = commands
            .iter()
            .find(|command| command.id == "prev-tab")
            .expect("workers previous tab command");
        assert_eq!(previous_tab_command.name, "Previous Tab");
        assert_eq!(previous_tab_command.key, '[');

        let next_tab_command = commands
            .iter()
            .find(|command| command.id == "next-tab")
            .expect("workers next tab command");
        assert_eq!(next_tab_command.name, "Next Tab");
        assert_eq!(next_tab_command.key, ']');

        let trait_commands = <WorkersPlugin as crate::plugin::Plugin>::commands(&plugin);
        let prioritized: Vec<(&str, u8)> = trait_commands
            .iter()
            .filter(|command| command.priority > 0)
            .map(|command| (command.id.as_str(), command.priority))
            .collect();
        assert_eq!(
            prioritized,
            vec![("create", 1), ("run", 4), ("stop", 2), ("open", 3)]
        );
    }

    #[test]
    fn test_stop_key_emits_stop_workers_command() {
        let mut plugin = WorkersPlugin::new();

        let commands = plugin.handle_event_internal(Event::Key {
            code: "s".to_string(),
            modifiers: crate::event::KeyModifiers::default(),
        });

        assert_eq!(commands, vec![Command::StopWorkers]);
    }

    #[test]
    fn test_refresh_key_emits_refresh_command() {
        let mut plugin = WorkersPlugin::new();

        let commands = plugin.handle_event_internal(Event::Key {
            code: "f".to_string(),
            modifiers: crate::event::KeyModifiers::default(),
        });

        assert_eq!(commands, vec![Command::Refresh]);
    }

    #[test]
    fn test_execute_stop_command_uses_declared_shortcut() {
        let mut plugin = WorkersPlugin::new();

        let execution = plugin
            .execute_command("stop")
            .expect("stop command should execute");

        assert_eq!(execution.command_name, "Stop Workers");
        assert_eq!(
            plugin.pending_commands.pop_front(),
            Some(Command::StopWorkers)
        );
    }

    #[test]
    fn test_execute_refresh_command_uses_declared_shortcut() {
        let mut plugin = WorkersPlugin::new();

        let execution = plugin
            .execute_command("refresh")
            .expect("refresh command should execute");

        assert_eq!(execution.command_name, "Refresh intents");
        assert_eq!(plugin.pending_commands.pop_front(), Some(Command::Refresh));
    }

    #[test]
    fn test_execute_open_command_opens_intent_from_kanban_view() {
        let mut plugin = WorkersPlugin::new();
        let mut intent = Intent::new(
            "Kanban open polish",
            PathBuf::from(".rightclick/intents/kanban-open.md"),
            "2026-05-26T10:00:00Z",
        );
        intent.id = "intent-kanban-open".to_string();
        plugin.state.add_intent(intent);
        plugin.state.selected_intent = Some(0);
        plugin.state.view_mode = ViewMode::Kanban;

        let execution = plugin
            .execute_command("open")
            .expect("open command should execute");

        assert_eq!(execution.command_name, "Open Intent");
        assert_eq!(
            plugin.pending_commands.pop_front(),
            Some(Command::OpenIntent("intent-kanban-open".to_string()))
        );
    }

    #[test]
    fn test_create_intent_enter_keeps_modal_open_when_title_is_blank() {
        let mut plugin = WorkersPlugin::new();
        plugin.state.modal_state = ModalState::CreateIntent;
        plugin.state.new_intent_title = "   ".to_string();

        let commands = plugin.handle_event_internal(Event::Key {
            code: "Enter".to_string(),
            modifiers: crate::event::KeyModifiers::default(),
        });

        assert!(commands.is_empty());
        assert_eq!(plugin.state.modal_state, ModalState::CreateIntent);
        assert_eq!(plugin.state.new_intent_title, "   ");
    }

    #[test]
    fn test_create_intent_enter_trims_title_before_creating() {
        let mut plugin = WorkersPlugin::new();
        plugin.state.modal_state = ModalState::CreateIntent;
        plugin.state.new_intent_title = "  Polish worker UX  ".to_string();

        let commands = plugin.handle_event_internal(Event::Key {
            code: "Enter".to_string(),
            modifiers: crate::event::KeyModifiers::default(),
        });

        assert_eq!(
            commands,
            vec![Command::CreateIntent {
                title: "Polish worker UX".to_string()
            }]
        );
        assert_eq!(plugin.state.modal_state, ModalState::None);
        assert!(plugin.state.new_intent_title.is_empty());
    }

    #[test]
    fn test_create_intent_action_keeps_modal_open_when_title_is_blank() {
        let mut plugin = WorkersPlugin::new();
        plugin.state.modal_state = ModalState::CreateIntent;
        plugin.state.new_intent_title = String::new();

        let commands = plugin.handle_action(&Action::Confirm);

        assert!(commands.is_empty());
        assert_eq!(plugin.state.modal_state, ModalState::CreateIntent);
    }

    #[test]
    fn test_search_entries_include_intents() {
        let mut plugin = WorkersPlugin::new();
        let mut intent = Intent::new(
            "Improve search polish",
            PathBuf::from(".rightclick/intents/search-polish.md"),
            "2026-05-26T10:00:00Z",
        );
        intent.id = "intent-search-polish".to_string();
        intent.status = IntentStatus::Ready;
        plugin.state.add_intent(intent);

        let entries = plugin.search_entries();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "intent-search-polish");
        assert_eq!(entries[0].title, "Improve search polish");
        assert!(entries[0].preview.contains("ready"));
        assert!(entries[0].preview.contains("search-polish.md"));
    }

    #[test]
    fn test_activate_search_result_selects_intent() {
        let mut plugin = WorkersPlugin::new();
        let mut first = Intent::new(
            "First",
            PathBuf::from(".rightclick/intents/first.md"),
            "2026-05-26T10:00:00Z",
        );
        first.id = "intent-first".to_string();
        let mut second = Intent::new(
            "Second",
            PathBuf::from(".rightclick/intents/second.md"),
            "2026-05-26T10:00:00Z",
        );
        second.id = "intent-second".to_string();
        plugin.state.add_intent(first);
        plugin.state.add_intent(second);
        plugin.state.view_mode = ViewMode::Kanban;
        plugin.state.preview_tab = PreviewTab::Output;
        plugin.focus_pane = FocusPane::Sidebar;

        assert!(plugin.activate_search_result("intent-second"));

        assert_eq!(plugin.state.selected_intent, Some(1));
        assert_eq!(plugin.state.view_mode, ViewMode::List);
        assert_eq!(plugin.state.preview_tab, PreviewTab::Spec);
        assert_eq!(plugin.focus_pane, FocusPane::Preview);
    }

    #[test]
    fn test_status_line_points_to_specs_when_empty() {
        let plugin = WorkersPlugin::new();

        assert_eq!(
            plugin.status_line(),
            Some(
                "No intents | n New | f Refresh intents | / Global search | ? Help | specs: .rightclick/intents"
                    .to_string()
            )
        );
    }

    #[test]
    fn test_status_line_summarizes_worker_outcomes() {
        let mut plugin = WorkersPlugin::new();
        let mut intent = Intent::new(
            "Ship workers polish",
            PathBuf::from(".rightclick/intents/workers-polish.md"),
            "2026-05-26T10:00:00Z",
        );
        intent.id = "intent-workers-polish".to_string();
        plugin.state.add_intent(intent);

        let mut running = Worker::new(
            "implement",
            WorkerType::Implementer,
            "intent-workers-polish",
            PathBuf::from("/repo/w1"),
            "branch",
            "claude",
            PathBuf::from("/repo/log1"),
            "2026-05-26T10:00:00Z",
        );
        running.mark_running();
        let mut failed = Worker::new(
            "verify",
            WorkerType::Verifier,
            "intent-workers-polish",
            PathBuf::from("/repo/w2"),
            "branch",
            "claude",
            PathBuf::from("/repo/log2"),
            "2026-05-26T10:00:00Z",
        );
        failed.mark_failed("2026-05-26T11:00:00Z", 1);

        plugin.state.add_worker(running);
        plugin.state.add_worker(failed);

        assert_eq!(
            plugin.status_line(),
            Some("1 intent | 2 workers | 1 running | 1 failed".to_string())
        );
    }

    #[test]
    fn test_status_line_uses_singular_worker_count() {
        let mut plugin = WorkersPlugin::new();
        let mut intent = Intent::new(
            "Ship workers polish",
            PathBuf::from(".rightclick/intents/workers-polish.md"),
            "2026-05-26T10:00:00Z",
        );
        intent.id = "intent-workers-polish".to_string();
        plugin.state.add_intent(intent);

        let mut worker = Worker::new(
            "verify",
            WorkerType::Verifier,
            "intent-workers-polish",
            PathBuf::from("/repo/w1"),
            "branch",
            "claude",
            PathBuf::from("/repo/log1"),
            "2026-05-26T10:00:00Z",
        );
        worker.mark_completed("2026-05-26T11:00:00Z");
        plugin.state.add_worker(worker);

        assert_eq!(
            plugin.status_line(),
            Some("1 intent | 1 worker | 1 completed".to_string())
        );
    }

    #[test]
    fn test_sanitize_branch_name() {
        assert_eq!(sanitize_branch_name("Hello World"), "hello-world");
        assert_eq!(sanitize_branch_name("Feature: Auth!"), "feature-auth");
        assert_eq!(sanitize_branch_name("  spaces  "), "spaces");
    }
}
