//! Workers Plugin State
//!
//! This module defines the state structure for the Workers plugin,
//! including intents, workers, view modes, and selection state.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::models::intent::{Intent, IntentId, IntentStatus, Worker, WorkerId, WorkerStatus};

/// An intent entry with computed properties
#[derive(Clone, Debug)]
pub struct IntentEntry {
    /// The intent data
    pub intent: Intent,
    /// Whether this intent is expanded in the UI
    pub expanded: bool,
    /// Workers associated with this intent
    pub worker_ids: Vec<WorkerId>,
}

impl IntentEntry {
    /// Create a new intent entry
    pub fn new(intent: Intent) -> Self {
        let worker_ids = intent.workers.clone();
        Self {
            intent,
            expanded: false,
            worker_ids,
        }
    }

    /// Get completion percentage
    pub fn completion_percentage(&self) -> u8 {
        self.intent.completion_percentage()
    }

    /// Check if all workers are terminal
    pub fn all_workers_terminal(&self, workers: &HashMap<WorkerId, Worker>) -> bool {
        self.worker_ids.iter().all(|id| {
            workers
                .get(id)
                .map(|w| w.status.is_terminal())
                .unwrap_or(true)
        })
    }

    /// Get status icon
    pub fn status_icon(&self) -> &'static str {
        self.intent.status.icon()
    }
}

/// A worker entry with runtime information
#[derive(Clone, Debug)]
pub struct WorkerEntry {
    /// The worker data
    pub worker: Worker,
    /// Live output lines (last N lines)
    pub output_lines: Vec<String>,
    /// Whether output is being streamed
    pub streaming: bool,
}

impl WorkerEntry {
    /// Create a new worker entry
    pub fn new(worker: Worker) -> Self {
        Self {
            worker,
            output_lines: Vec::new(),
            streaming: false,
        }
    }

    /// Append output line
    pub fn append_output(&mut self, line: String) {
        self.output_lines.push(line);
        // Keep only last 1000 lines
        if self.output_lines.len() > 1000 {
            self.output_lines.remove(0);
        }
    }

    /// Get the last N output lines
    pub fn last_output(&self, n: usize) -> &[String] {
        let start = self.output_lines.len().saturating_sub(n);
        &self.output_lines[start..]
    }

    /// Get status icon
    pub fn status_icon(&self) -> &'static str {
        self.worker.status_icon()
    }

    /// Get type icon
    pub fn type_icon(&self) -> &'static str {
        self.worker.type_icon()
    }
}

/// View mode for the workers display
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    /// List view showing all intents
    List,
    /// Kanban view organized by status
    Kanban,
    /// Detail view for selected intent
    Detail,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self::List
    }
}

/// Preview tab in the right pane
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewTab {
    /// Intent spec content
    Spec,
    /// Worker output
    Output,
    /// Acceptance criteria
    Criteria,
}

impl Default for PreviewTab {
    fn default() -> Self {
        Self::Spec
    }
}

/// Which pane has keyboard focus
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusPane {
    /// Intent list pane
    Sidebar,
    /// Preview/detail pane
    Preview,
}

impl Default for FocusPane {
    fn default() -> Self {
        Self::Sidebar
    }
}

/// Modal dialog state
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ModalState {
    /// No modal open
    #[default]
    None,
    /// Create intent dialog
    CreateIntent,
    /// Delete confirmation dialog
    DeleteConfirm,
    /// Run workers confirmation
    RunConfirm,
    /// Stop workers confirmation
    StopConfirm,
}

/// Plugin state containing all mutable data
#[derive(Clone, Debug, Default)]
pub struct PluginState {
    /// All intents
    pub intents: Vec<IntentEntry>,
    /// All workers by ID
    pub workers: HashMap<WorkerId, WorkerEntry>,
    /// Currently selected intent index
    pub selected_intent: Option<usize>,
    /// Currently selected worker ID (if viewing workers)
    pub selected_worker: Option<WorkerId>,
    /// Current view mode
    pub view_mode: ViewMode,
    /// Current preview tab
    pub preview_tab: PreviewTab,
    /// Sidebar width in columns
    pub sidebar_width: u16,
    /// Whether a command is running
    pub command_running: bool,
    /// Modal state
    pub modal_state: ModalState,
    /// New intent title buffer (for creation dialog)
    pub new_intent_title: String,
    /// Output text buffer
    pub output_text: String,
    /// Intents directory path
    pub intents_dir: PathBuf,
    /// Logs directory path
    pub logs_dir: PathBuf,
}

impl PluginState {
    /// Create a new empty state
    pub fn new(intents_dir: PathBuf, logs_dir: PathBuf) -> Self {
        Self {
            intents: Vec::new(),
            workers: HashMap::new(),
            selected_intent: None,
            selected_worker: None,
            view_mode: ViewMode::default(),
            preview_tab: PreviewTab::default(),
            sidebar_width: 40,
            command_running: false,
            modal_state: ModalState::None,
            new_intent_title: String::new(),
            output_text: String::new(),
            intents_dir,
            logs_dir,
        }
    }

    /// Get the currently selected intent if any
    pub fn selected_intent(&self) -> Option<&IntentEntry> {
        self.selected_intent.and_then(|idx| self.intents.get(idx))
    }

    /// Get a mutable reference to the selected intent
    pub fn selected_intent_mut(&mut self) -> Option<&mut IntentEntry> {
        self.selected_intent.and_then(|idx| self.intents.get_mut(idx))
    }

    /// Get intent by ID
    pub fn get_intent(&self, id: &str) -> Option<&IntentEntry> {
        self.intents.iter().find(|e| e.intent.id == id)
    }

    /// Get mutable intent by ID
    pub fn get_intent_mut(&mut self, id: &str) -> Option<&mut IntentEntry> {
        self.intents.iter_mut().find(|e| e.intent.id == id)
    }

    /// Get worker by ID
    pub fn get_worker(&self, id: &str) -> Option<&WorkerEntry> {
        self.workers.get(id)
    }

    /// Get mutable worker by ID
    pub fn get_worker_mut(&mut self, id: &str) -> Option<&mut WorkerEntry> {
        self.workers.get_mut(id)
    }

    /// Add an intent
    pub fn add_intent(&mut self, intent: Intent) {
        self.intents.push(IntentEntry::new(intent));
    }

    /// Remove an intent by ID
    pub fn remove_intent(&mut self, id: &str) -> Option<Intent> {
        if let Some(pos) = self.intents.iter().position(|e| e.intent.id == id) {
            let entry = self.intents.remove(pos);
            // Clean up selection
            if let Some(selected) = self.selected_intent {
                if selected >= self.intents.len() {
                    self.selected_intent = if self.intents.is_empty() {
                        None
                    } else {
                        Some(self.intents.len() - 1)
                    };
                } else if selected == pos {
                    self.selected_intent = None;
                }
            }
            Some(entry.intent)
        } else {
            None
        }
    }

    /// Add a worker
    pub fn add_worker(&mut self, worker: Worker) {
        let intent_id = worker.intent_id.clone();
        let worker_id = worker.id.clone();
        self.workers.insert(worker_id.clone(), WorkerEntry::new(worker));

        // Associate with intent
        if let Some(intent) = self.get_intent_mut(&intent_id) {
            intent.worker_ids.push(worker_id);
        }
    }

    /// Remove a worker by ID
    pub fn remove_worker(&mut self, id: &str) -> Option<Worker> {
        if let Some(entry) = self.workers.remove(id) {
            // Remove from intent's worker list
            for intent in &mut self.intents {
                intent.worker_ids.retain(|wid| wid != id);
            }
            Some(entry.worker)
        } else {
            None
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.intents.is_empty() {
            self.selected_intent = None;
            return;
        }

        match self.selected_intent {
            None => self.selected_intent = Some(0),
            Some(idx) => {
                self.selected_intent = Some((idx + 1).min(self.intents.len() - 1));
            }
        }
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        match self.selected_intent {
            None => {
                if !self.intents.is_empty() {
                    self.selected_intent = Some(self.intents.len() - 1);
                }
            }
            Some(0) => self.selected_intent = None,
            Some(idx) => self.selected_intent = Some(idx.saturating_sub(1)),
        }
    }

    /// Clear all state
    pub fn clear(&mut self) {
        self.intents.clear();
        self.workers.clear();
        self.selected_intent = None;
        self.selected_worker = None;
        self.output_text.clear();
        self.command_running = false;
        self.modal_state = ModalState::None;
    }

    /// Add output text
    pub fn add_output(&mut self, text: impl Into<String>) {
        let text = text.into();
        if !self.output_text.is_empty() {
            self.output_text.push('\n');
        }
        self.output_text.push_str(&text);
    }

    /// Clear output
    pub fn clear_output(&mut self) {
        self.output_text.clear();
    }

    /// Get intents grouped by status for kanban view
    pub fn intents_by_status(
        &self,
    ) -> IntentGroups {
        let mut draft = Vec::new();
        let mut ready = Vec::new();
        let mut in_progress = Vec::new();
        let mut completed = Vec::new();
        let mut failed = Vec::new();

        for (idx, entry) in self.intents.iter().enumerate() {
            match entry.intent.status {
                IntentStatus::Draft => draft.push(idx),
                IntentStatus::Ready => ready.push(idx),
                IntentStatus::InProgress => in_progress.push(idx),
                IntentStatus::Completed => completed.push(idx),
                IntentStatus::Failed => failed.push(idx),
            }
        }

        IntentGroups {
            draft,
            ready,
            in_progress,
            completed,
            failed,
        }
    }

    /// Get workers grouped by status
    pub fn workers_by_status(
        &self,
    ) -> WorkerGroups {
        let mut pending = Vec::new();
        let mut running = Vec::new();
        let mut completed = Vec::new();
        let mut failed = Vec::new();
        let mut paused = Vec::new();

        for (id, entry) in &self.workers {
            match entry.worker.status {
                WorkerStatus::Pending => pending.push(id.clone()),
                WorkerStatus::Running => running.push(id.clone()),
                WorkerStatus::Completed => completed.push(id.clone()),
                WorkerStatus::Failed => failed.push(id.clone()),
                WorkerStatus::Paused => paused.push(id.clone()),
                WorkerStatus::Cancelled => failed.push(id.clone()),
            }
        }

        WorkerGroups {
            pending,
            running,
            completed,
            failed,
            paused,
        }
    }

    /// Check if a modal is open
    pub fn is_modal_open(&self) -> bool {
        self.modal_state != ModalState::None
    }

    /// Close any open modal
    pub fn close_modal(&mut self) {
        self.modal_state = ModalState::None;
        self.new_intent_title.clear();
    }

    /// Get running workers count
    pub fn running_workers_count(&self) -> usize {
        self.workers
            .values()
            .filter(|e| e.worker.status == WorkerStatus::Running)
            .count()
    }

    /// Get completed workers count
    pub fn completed_workers_count(&self) -> usize {
        self.workers
            .values()
            .filter(|e| e.worker.status == WorkerStatus::Completed)
            .count()
    }

    /// Check if any workers are running
    pub fn has_running_workers(&self) -> bool {
        self.workers
            .values()
            .any(|e| e.worker.status == WorkerStatus::Running)
    }

    /// Get workers for a specific intent
    pub fn get_intent_workers(
        &self,
        intent_id: &str,
    ) -> Vec<&WorkerEntry> {
        self.workers
            .values()
            .filter(|e| e.worker.intent_id == intent_id)
            .collect()
    }

    /// Get next pending worker that can start (dependencies satisfied)
    pub fn get_runnable_worker(&self) -> Option<&Worker> {
        let completed: Vec<_> = self
            .workers
            .values()
            .filter(|e| e.worker.status == WorkerStatus::Completed)
            .map(|e| e.worker.id.clone())
            .collect();

        self.workers
            .values()
            .filter(|e| e.worker.status == WorkerStatus::Pending)
            .find(|e| e.worker.can_start(&completed))
            .map(|e| &e.worker)
    }
}

/// Grouped intent indices for kanban view
#[derive(Clone, Debug, Default)]
pub struct IntentGroups {
    /// Draft intents
    pub draft: Vec<usize>,
    /// Ready intents
    pub ready: Vec<usize>,
    /// In-progress intents
    pub in_progress: Vec<usize>,
    /// Completed intents
    pub completed: Vec<usize>,
    /// Failed intents
    pub failed: Vec<usize>,
}

/// Grouped worker IDs by status
#[derive(Clone, Debug, Default)]
pub struct WorkerGroups {
    /// Pending workers
    pub pending: Vec<WorkerId>,
    /// Running workers
    pub running: Vec<WorkerId>,
    /// Completed workers
    pub completed: Vec<WorkerId>,
    /// Failed workers
    pub failed: Vec<WorkerId>,
    /// Paused workers
    pub paused: Vec<WorkerId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::intent::{WorkerType, WorkerStatus};

    #[test]
    fn test_view_mode_default() {
        assert_eq!(ViewMode::default(), ViewMode::List);
    }

    #[test]
    fn test_preview_tab_default() {
        assert_eq!(PreviewTab::default(), PreviewTab::Spec);
    }

    #[test]
    fn test_focus_pane_default() {
        assert_eq!(FocusPane::default(), FocusPane::Sidebar);
    }

    #[test]
    fn test_state_new() {
        let state = PluginState::new(
            PathBuf::from(".rightclick/intents"),
            PathBuf::from(".rightclick/logs"),
        );
        assert!(state.intents.is_empty());
        assert!(state.workers.is_empty());
        assert_eq!(state.selected_intent, None);
        assert_eq!(state.sidebar_width, 40);
    }

    #[test]
    fn test_selection() {
        let mut state = PluginState::new(PathBuf::from("intents"), PathBuf::from("logs"));

        // Add some mock intents
        state.intents.push(IntentEntry::new(Intent::new(
            "Intent 1",
            PathBuf::from("i1.md"),
            "2026-02-14T10:00:00Z",
        )));
        state.intents.push(IntentEntry::new(Intent::new(
            "Intent 2",
            PathBuf::from("i2.md"),
            "2026-02-14T10:00:00Z",
        )));
        state.intents.push(IntentEntry::new(Intent::new(
            "Intent 3",
            PathBuf::from("i3.md"),
            "2026-02-14T10:00:00Z",
        )));

        // Initially no selection
        assert_eq!(state.selected_intent, None);

        // Select next
        state.select_next();
        assert_eq!(state.selected_intent, Some(0));

        // Select next
        state.select_next();
        assert_eq!(state.selected_intent, Some(1));

        // Select prev
        state.select_prev();
        assert_eq!(state.selected_intent, Some(0));
    }

    #[test]
    fn test_add_and_remove_intent() {
        let mut state = PluginState::new(PathBuf::from("intents"), PathBuf::from("logs"));

        let intent = Intent::new("Test", PathBuf::from("test.md"), "2026-02-14T10:00:00Z");
        let id = intent.id.clone();

        state.add_intent(intent);
        assert_eq!(state.intents.len(), 1);

        let removed = state.remove_intent(&id);
        assert!(removed.is_some());
        assert!(state.intents.is_empty());
    }

    #[test]
    fn test_worker_entry_output() {
        let worker = Worker::new(
            "test",
            WorkerType::Investigator,
            "intent-1",
            PathBuf::from("/repo/w"),
            "branch",
            "claude",
            PathBuf::from("/repo/log"),
            "2026-02-14T10:00:00Z",
        );

        let mut entry = WorkerEntry::new(worker);

        entry.append_output("Line 1".to_string());
        entry.append_output("Line 2".to_string());
        entry.append_output("Line 3".to_string());

        assert_eq!(entry.last_output(2).len(), 2);
        assert_eq!(entry.last_output(2)[0], "Line 2");
        assert_eq!(entry.last_output(2)[1], "Line 3");
    }

    #[test]
    fn test_intent_groups() {
        let mut state = PluginState::new(PathBuf::from("intents"), PathBuf::from("logs"));

        let mut intent1 = Intent::new("Draft", PathBuf::from("d.md"), "2026-02-14T10:00:00Z");
        intent1.status = IntentStatus::Draft;

        let mut intent2 = Intent::new("Ready", PathBuf::from("r.md"), "2026-02-14T10:00:00Z");
        intent2.status = IntentStatus::Ready;

        let mut intent3 = Intent::new("In Progress", PathBuf::from("p.md"), "2026-02-14T10:00:00Z");
        intent3.status = IntentStatus::InProgress;

        state.add_intent(intent1);
        state.add_intent(intent2);
        state.add_intent(intent3);

        let groups = state.intents_by_status();
        assert_eq!(groups.draft.len(), 1);
        assert_eq!(groups.ready.len(), 1);
        assert_eq!(groups.in_progress.len(), 1);
        assert!(groups.completed.is_empty());
    }

    #[test]
    fn test_modal_state() {
        let mut state = PluginState::new(PathBuf::from("intents"), PathBuf::from("logs"));
        assert!(!state.is_modal_open());

        state.modal_state = ModalState::CreateIntent;
        assert!(state.is_modal_open());

        state.close_modal();
        assert!(!state.is_modal_open());
    }
}
