//! TD Monitor Plugin State
//!
//! This module defines the state structure for the TD Monitor plugin,
//! including task management, view modes, and selection state.

use chrono::{DateTime, Utc};

/// Task status variants
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum TaskStatus {
    /// Task is pending/not started
    #[default]
    Todo,
    /// Task is in progress
    InProgress,
    /// Task is in review
    Review,
    /// Task is completed
    Done,
}

impl TaskStatus {
    /// Get the display name for this status
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Todo => "todo",
            TaskStatus::InProgress => "in-progress",
            TaskStatus::Review => "review",
            TaskStatus::Done => "done",
        }
    }

    /// Get the display icon for this status
    pub fn icon(&self) -> &'static str {
        match self {
            TaskStatus::Todo => "○",
            TaskStatus::InProgress => "◐",
            TaskStatus::Review => "◑",
            TaskStatus::Done => "●",
        }
    }

    /// Parse a status from a database string
    pub fn from_db(status: &str) -> Self {
        match status.to_lowercase().as_str() {
            "todo" | "pending" | "backlog" => TaskStatus::Todo,
            "in_progress" | "in-progress" | "doing" | "active" => TaskStatus::InProgress,
            "review" | "in_review" | "in-review" => TaskStatus::Review,
            "done" | "completed" | "finished" | "closed" => TaskStatus::Done,
            _ => TaskStatus::Todo,
        }
    }
}

/// Task priority levels
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Priority {
    /// Low priority
    Low,
    /// Medium priority (default)
    #[default]
    Medium,
    /// High priority
    High,
    /// Critical priority
    Critical,
}

impl Priority {
    /// Get the display name for this priority
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Low => "low",
            Priority::Medium => "medium",
            Priority::High => "high",
            Priority::Critical => "critical",
        }
    }

    /// Get the display icon for this priority
    pub fn icon(&self) -> &'static str {
        match self {
            Priority::Low => "░░░",
            Priority::Medium => "▒░░",
            Priority::High => "▒▒░",
            Priority::Critical => "▓▓▓",
        }
    }

    /// Parse a priority from a database string
    pub fn from_db(priority: &str) -> Self {
        match priority.to_lowercase().as_str() {
            "low" => Priority::Low,
            "medium" | "normal" | "default" => Priority::Medium,
            "high" => Priority::High,
            "critical" | "urgent" | "blocker" => Priority::Critical,
            _ => Priority::Medium,
        }
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// View mode for the TD Monitor plugin
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// List view with task details
    #[default]
    List,
    /// Board/Kanban view
    Board,
}

impl ViewMode {
    /// Toggle between view modes
    pub fn toggle(&self) -> Self {
        match self {
            ViewMode::List => ViewMode::Board,
            ViewMode::Board => ViewMode::List,
        }
    }
}

/// A task in the TD system
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    /// Unique task identifier
    pub id: String,
    /// Task title
    pub title: String,
    /// Task description (optional)
    pub description: Option<String>,
    /// Current status
    pub status: TaskStatus,
    /// Task priority
    pub priority: Priority,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Tags associated with the task
    pub tags: Vec<String>,
}

impl Task {
    /// Create a new task with the given properties
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            status: TaskStatus::Todo,
            priority: Priority::Medium,
            created_at: now,
            updated_at: now,
            tags: Vec::new(),
        }
    }

    /// Check if this task matches a filter string
    pub fn matches_filter(&self, filter: &str) -> bool {
        let filter_lower = filter.to_lowercase();
        self.title.to_lowercase().contains(&filter_lower)
            || self
                .description
                .as_ref()
                .map(|d| d.to_lowercase().contains(&filter_lower))
                .unwrap_or(false)
            || self.tags.iter().any(|t| t.to_lowercase().contains(&filter_lower))
    }
}

/// An entry in the activity log
#[derive(Clone, Debug, PartialEq)]
pub struct ActivityLogEntry {
    /// Entry timestamp
    pub timestamp: DateTime<Utc>,
    /// Task ID associated with this entry
    pub task_id: Option<String>,
    /// Activity type
    pub activity_type: String,
    /// Activity description
    pub description: String,
}

/// Plugin state containing all mutable data
#[derive(Clone, Debug, Default)]
pub struct PluginState {
    /// All tasks
    pub tasks: Vec<Task>,
    /// Currently selected task index
    pub selected_task: Option<usize>,
    /// Currently focused task (separate from selection)
    pub current_focus: Option<Task>,
    /// Current view mode
    pub view_mode: ViewMode,
    /// Active filter string
    pub filter: Option<String>,
    /// Activity log entries
    pub activity_log: Vec<ActivityLogEntry>,
    /// Sidebar width in columns
    pub sidebar_width: u16,
    /// Loading state
    pub is_loading: bool,
    /// Error message if any
    pub error: Option<String>,
    /// Filter input mode active
    pub filter_input_active: bool,
    /// Current filter input value
    pub filter_input: String,
}

impl PluginState {
    /// Create a new empty state
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            selected_task: None,
            current_focus: None,
            view_mode: ViewMode::default(),
            filter: None,
            activity_log: Vec::new(),
            sidebar_width: 40,
            is_loading: false,
            error: None,
            filter_input_active: false,
            filter_input: String::new(),
        }
    }

    /// Get filtered tasks based on current filter
    pub fn filtered_tasks(&self) -> Vec<&Task> {
        match &self.filter {
            Some(filter) => self
                .tasks
                .iter()
                .filter(|t| t.matches_filter(filter))
                .collect(),
            None => self.tasks.iter().collect(),
        }
    }

    /// Get tasks grouped by status for board view
    pub fn tasks_by_status(&self) -> std::collections::HashMap<TaskStatus, Vec<&Task>> {
        let mut grouped: std::collections::HashMap<TaskStatus, Vec<&Task>> =
            std::collections::HashMap::new();

        for task in self.filtered_tasks() {
            grouped.entry(task.status).or_default().push(task);
        }

        grouped
    }

    /// Get the currently selected task if any
    pub fn selected_task(&self) -> Option<&Task> {
        let filtered = self.filtered_tasks();
        self.selected_task.and_then(|idx| filtered.get(idx).copied())
    }

    /// Get the currently selected task as mutable
    pub fn selected_task_mut(&mut self) -> Option<&mut Task> {
        // This is tricky because we need to find the index in the original vector
        // For now, we'll use the filtered index to find the task ID, then find it in original
        if let Some(selected) = self.selected_task() {
            let id = selected.id.clone();
            self.tasks.iter_mut().find(|t| t.id == id)
        } else {
            None
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        let count = self.filtered_tasks().len();
        if count == 0 {
            self.selected_task = None;
            return;
        }

        match self.selected_task {
            None => self.selected_task = Some(0),
            Some(idx) => {
                self.selected_task = Some((idx + 1).min(count - 1));
            }
        }
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        match self.selected_task {
            None => {
                let count = self.filtered_tasks().len();
                if count > 0 {
                    self.selected_task = Some(count - 1);
                }
            }
            Some(0) => self.selected_task = None,
            Some(idx) => self.selected_task = Some(idx.saturating_sub(1)),
        }
    }

    /// Select the first task
    pub fn select_first(&mut self) {
        if !self.filtered_tasks().is_empty() {
            self.selected_task = Some(0);
        }
    }

    /// Select the last task
    pub fn select_last(&mut self) {
        let count = self.filtered_tasks().len();
        if count > 0 {
            self.selected_task = Some(count - 1);
        }
    }

    /// Set the filter string
    pub fn set_filter(&mut self, filter: Option<String>) {
        self.filter = filter;
        // Reset selection when filter changes
        self.selected_task = None;
    }

    /// Clear the filter
    pub fn clear_filter(&mut self) {
        self.filter = None;
        self.filter_input.clear();
        self.filter_input_active = false;
        self.selected_task = None;
    }

    /// Start filter input mode
    pub fn start_filter_input(&mut self) {
        self.filter_input_active = true;
        self.filter_input.clear();
    }

    /// Apply the current filter input
    pub fn apply_filter_input(&mut self) {
        if self.filter_input.is_empty() {
            self.filter = None;
        } else {
            self.filter = Some(self.filter_input.clone());
        }
        self.filter_input_active = false;
        self.selected_task = None;
    }

    /// Update the current focus task
    pub fn set_focused_task(&mut self, task: Option<Task>) {
        self.current_focus = task;
    }

    /// Set loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
    }

    /// Set error message
    pub fn set_error(&mut self, error: Option<String>) {
        self.error = error;
    }

    /// Clear all state
    pub fn clear(&mut self) {
        self.tasks.clear();
        self.selected_task = None;
        self.current_focus = None;
        self.filter = None;
        self.activity_log.clear();
        self.error = None;
        self.filter_input_active = false;
        self.filter_input.clear();
    }

    /// Get tasks by a specific status
    pub fn tasks_with_status(&self, status: TaskStatus) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| t.status == status)
            .collect()
    }

    /// Get the focused task title for display
    pub fn focused_task_title(&self) -> Option<&str> {
        self.current_focus.as_ref().map(|t| t.title.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status_from_db() {
        assert_eq!(TaskStatus::from_db("todo"), TaskStatus::Todo);
        assert_eq!(TaskStatus::from_db("in-progress"), TaskStatus::InProgress);
        assert_eq!(TaskStatus::from_db("review"), TaskStatus::Review);
        assert_eq!(TaskStatus::from_db("done"), TaskStatus::Done);
        assert_eq!(TaskStatus::from_db("unknown"), TaskStatus::Todo);
    }

    #[test]
    fn test_priority_from_db() {
        assert_eq!(Priority::from_db("low"), Priority::Low);
        assert_eq!(Priority::from_db("medium"), Priority::Medium);
        assert_eq!(Priority::from_db("high"), Priority::High);
        assert_eq!(Priority::from_db("critical"), Priority::Critical);
        assert_eq!(Priority::from_db("unknown"), Priority::Medium);
    }

    #[test]
    fn test_task_matches_filter() {
        let mut task = Task::new("1", "Test Task");
        task.description = Some("A description".to_string());
        task.tags = vec!["bug".to_string(), "urgent".to_string()];

        assert!(task.matches_filter("test"));
        assert!(task.matches_filter("description"));
        assert!(task.matches_filter("bug"));
        assert!(!task.matches_filter("nonexistent"));
    }

    #[test]
    fn test_state_selection() {
        let mut state = PluginState::new();
        state.tasks = vec![
            Task::new("1", "Task 1"),
            Task::new("2", "Task 2"),
            Task::new("3", "Task 3"),
        ];

        assert_eq!(state.selected_task, None);

        state.select_next();
        assert_eq!(state.selected_task, Some(0));

        state.select_next();
        assert_eq!(state.selected_task, Some(1));

        state.select_prev();
        assert_eq!(state.selected_task, Some(0));
    }

    #[test]
    fn test_filtered_tasks() {
        let mut state = PluginState::new();
        state.tasks = vec![
            Task::new("1", "Alpha Task"),
            Task::new("2", "Beta Task"),
            Task::new("3", "Gamma Task"),
        ];

        state.set_filter(Some("alpha".to_string()));
        let filtered = state.filtered_tasks();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "1");
    }

    #[test]
    fn test_view_mode_toggle() {
        let mut mode = ViewMode::List;
        mode = mode.toggle();
        assert!(matches!(mode, ViewMode::Board));
        mode = mode.toggle();
        assert!(matches!(mode, ViewMode::List));
    }
}
