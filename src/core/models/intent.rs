//! Intent, Worker, and Task models for the Workers plugin.
//!
//! This module provides pure data structures for managing AI agent workflows
//! following the Functional Core & Imperative Shell architecture.
//!
//! All types in this module are serializable and deserializable for persistence.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Unique identifier for intents
pub type IntentId = String;

/// Unique identifier for workers
pub type WorkerId = String;

/// Unique identifier for tasks
pub type TaskId = String;

/// An Intent represents a high-level specification for AI agents to implement.
///
/// Intents are "living specs" that evolve as workers complete tasks.
/// They are persisted as Markdown files with YAML frontmatter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Intent {
    /// Unique identifier (UUID)
    pub id: IntentId,

    /// Human-readable title
    pub title: String,

    /// Detailed description (markdown)
    pub description: String,

    /// Current lifecycle status
    pub status: IntentStatus,

    /// Path to the spec file
    pub spec_path: PathBuf,

    /// IDs of workers associated with this intent
    pub workers: Vec<WorkerId>,

    /// Acceptance criteria for completion
    pub acceptance_criteria: Vec<Criterion>,

    /// Additional metadata (tags, priority, etc.)
    pub metadata: HashMap<String, String>,

    /// Creation timestamp (ISO 8601)
    pub created_at: String,

    /// Last update timestamp (ISO 8601)
    pub updated_at: String,
}

impl Intent {
    /// Create a new intent with the given title and spec path.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::core::models::intent::Intent;
    /// use std::path::PathBuf;
    ///
    /// let intent = Intent::new(
    ///     "Add JWT authentication",
    ///     PathBuf::from(".rightclick/intents/auth-jwt.md"),
    ///     "2026-02-14T10:00:00Z",
    /// );
    /// assert_eq!(intent.title, "Add JWT authentication");
    /// assert_eq!(intent.status.to_string(), "draft");
    /// ```
    pub fn new(title: impl Into<String>, spec_path: PathBuf, now: impl Into<String>) -> Self {
        let title = title.into();
        let now = now.into();
        Self {
            id: format!("intent-{}", uuid::Uuid::new_v4()),
            title,
            description: String::new(),
            status: IntentStatus::Draft,
            spec_path,
            workers: Vec::new(),
            acceptance_criteria: Vec::new(),
            metadata: HashMap::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Add an acceptance criterion.
    pub fn with_criterion(mut self, description: impl Into<String>) -> Self {
        self.acceptance_criteria.push(Criterion {
            description: description.into(),
            completed: false,
        });
        self
    }

    /// Add a worker to this intent.
    pub fn add_worker(&mut self, worker_id: WorkerId) {
        if !self.workers.contains(&worker_id) {
            self.workers.push(worker_id);
        }
    }

    /// Remove a worker from this intent.
    pub fn remove_worker(&mut self, worker_id: &str) {
        self.workers.retain(|id| id != worker_id);
    }

    /// Check if all acceptance criteria are met.
    pub fn is_complete(&self) -> bool {
        if self.acceptance_criteria.is_empty() {
            return false;
        }
        self.acceptance_criteria.iter().all(|c| c.completed)
    }

    /// Get completion percentage (0-100).
    pub fn completion_percentage(&self) -> u8 {
        if self.acceptance_criteria.is_empty() {
            return 0;
        }
        let completed = self
            .acceptance_criteria
            .iter()
            .filter(|c| c.completed)
            .count();
        ((completed as f32 / self.acceptance_criteria.len() as f32) * 100.0) as u8
    }

    /// Update the status based on workers and criteria.
    pub fn update_status(&mut self) {
        let all_workers_done = self.workers.is_empty()
            || self.workers.iter().all(|_| {
                // This would check worker status in real implementation
                // For now, we assume workers are tracked separately
                true
            });

        self.status = match (self.status, all_workers_done, self.is_complete()) {
            (IntentStatus::Draft, _, _) => IntentStatus::Draft,
            (IntentStatus::Ready, _, _) => IntentStatus::Ready,
            (IntentStatus::InProgress, true, true) => IntentStatus::Completed,
            (IntentStatus::InProgress, _, _) => IntentStatus::InProgress,
            (IntentStatus::Completed, _, _) => IntentStatus::Completed,
            (IntentStatus::Failed, _, _) => IntentStatus::Failed,
        };
    }
}

/// Lifecycle status of an intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentStatus {
    /// Initial state, being drafted
    Draft,
    /// Ready to start execution
    Ready,
    /// Workers are actively processing
    InProgress,
    /// All criteria met
    Completed,
    /// Execution failed
    Failed,
}

impl IntentStatus {
    /// Get display icon for this status.
    pub const fn icon(&self) -> &'static str {
        match self {
            IntentStatus::Draft => "📝",
            IntentStatus::Ready => "⏳",
            IntentStatus::InProgress => "🔄",
            IntentStatus::Completed => "✅",
            IntentStatus::Failed => "❌",
        }
    }

    /// Check if this status represents a terminal state.
    pub const fn is_terminal(&self) -> bool {
        matches!(self, IntentStatus::Completed | IntentStatus::Failed)
    }

    /// Check if workers can be started in this state.
    pub const fn can_start(&self) -> bool {
        matches!(self, IntentStatus::Ready | IntentStatus::InProgress)
    }
}

impl std::fmt::Display for IntentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntentStatus::Draft => write!(f, "draft"),
            IntentStatus::Ready => write!(f, "ready"),
            IntentStatus::InProgress => write!(f, "in_progress"),
            IntentStatus::Completed => write!(f, "completed"),
            IntentStatus::Failed => write!(f, "failed"),
        }
    }
}

/// A single acceptance criterion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Criterion {
    /// Description of what must be true
    pub description: String,
    /// Whether this criterion is satisfied
    pub completed: bool,
}

impl Criterion {
    /// Create a new criterion.
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            completed: false,
        }
    }

    /// Mark as completed.
    pub fn complete(&mut self) {
        self.completed = true;
    }

    /// Mark as incomplete.
    pub fn uncomplete(&mut self) {
        self.completed = false;
    }
}

/// A Worker represents an AI agent instance working on a specific task.
///
/// Workers are spawned by the coordinator to execute parts of an intent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worker {
    /// Unique identifier (UUID)
    pub id: WorkerId,

    /// Human-readable name
    pub name: String,

    /// Type of worker (determines behavior)
    pub worker_type: WorkerType,

    /// Current execution status
    pub status: WorkerStatus,

    /// ID of the parent intent
    pub intent_id: IntentId,

    /// Optional specific task ID
    pub task_id: Option<TaskId>,

    /// Path to the worktree where the agent runs
    pub worktree_path: PathBuf,

    /// Git branch name
    pub branch: String,

    /// Which agent to use (claude, cursor, codex, etc.)
    pub agent: String,

    /// Path to the output log file
    pub output_log: PathBuf,

    /// IDs of workers that must complete before this one starts
    pub depends_on: Vec<WorkerId>,

    /// Creation timestamp (ISO 8601)
    pub created_at: String,

    /// Completion timestamp (ISO 8601)
    pub completed_at: Option<String>,

    /// Exit code if the worker has finished
    pub exit_code: Option<i32>,
}

impl Worker {
    /// Create a new worker.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::core::models::intent::{Worker, WorkerType};
    /// use std::path::PathBuf;
    ///
    /// let worker = Worker::new(
    ///     "investigate-auth",
    ///     WorkerType::Investigator,
    ///     "intent-123",
    ///     PathBuf::from("/repo/workers/auth"),
    ///     "feature/auth",
    ///     "claude",
    ///     PathBuf::from("/repo/.rightclick/logs/auth.log"),
    ///     "2026-02-14T10:00:00Z",
    /// );
    /// assert_eq!(worker.worker_type, WorkerType::Investigator);
    /// assert_eq!(worker.status.to_string(), "pending");
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: impl Into<String>,
        worker_type: WorkerType,
        intent_id: impl Into<String>,
        worktree_path: PathBuf,
        branch: impl Into<String>,
        agent: impl Into<String>,
        output_log: PathBuf,
        now: impl Into<String>,
    ) -> Self {
        Self {
            id: format!("worker-{}", uuid::Uuid::new_v4()),
            name: name.into(),
            worker_type,
            status: WorkerStatus::Pending,
            intent_id: intent_id.into(),
            task_id: None,
            worktree_path,
            branch: branch.into(),
            agent: agent.into(),
            output_log,
            depends_on: Vec::new(),
            created_at: now.into(),
            completed_at: None,
            exit_code: None,
        }
    }

    /// Add a dependency on another worker.
    pub fn depends_on(mut self, worker_id: impl Into<String>) -> Self {
        self.depends_on.push(worker_id.into());
        self
    }

    /// Check if this worker can start (all dependencies satisfied).
    pub fn can_start(&self, completed_workers: &[WorkerId]) -> bool {
        self.depends_on
            .iter()
            .all(|dep| completed_workers.contains(dep))
    }

    /// Mark as running.
    pub fn mark_running(&mut self) {
        self.status = WorkerStatus::Running;
    }

    /// Mark as completed.
    pub fn mark_completed(&mut self, now: impl Into<String>) {
        self.status = WorkerStatus::Completed;
        self.completed_at = Some(now.into());
        self.exit_code = Some(0);
    }

    /// Mark as failed.
    pub fn mark_failed(&mut self, now: impl Into<String>, exit_code: i32) {
        self.status = WorkerStatus::Failed;
        self.completed_at = Some(now.into());
        self.exit_code = Some(exit_code);
    }

    /// Get the display icon for this worker's status.
    pub fn status_icon(&self) -> &'static str {
        self.status.icon()
    }

    /// Get the display icon for this worker's type.
    pub fn type_icon(&self) -> &'static str {
        self.worker_type.icon()
    }
}

/// Type of worker determining its specialized behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerType {
    /// Explores codebase to find relevant files and context
    Investigator,
    /// Implements the actual code changes
    Implementer,
    /// Verifies implementation against acceptance criteria
    Verifier,
    /// Reviews code quality and best practices
    Critic,
    /// Debugs failures and errors
    Debugger,
    /// Orchestrates other workers (coordinator)
    Coordinator,
}

impl WorkerType {
    /// Get display icon for this worker type.
    pub const fn icon(&self) -> &'static str {
        match self {
            WorkerType::Investigator => "🔍",
            WorkerType::Implementer => "🔨",
            WorkerType::Verifier => "✓",
            WorkerType::Critic => "👁",
            WorkerType::Debugger => "🐛",
            WorkerType::Coordinator => "🎯",
        }
    }

    /// Get description of what this worker type does.
    pub const fn description(&self) -> &'static str {
        match self {
            WorkerType::Investigator => "Explores codebase to find relevant files and context",
            WorkerType::Implementer => "Implements code changes based on findings",
            WorkerType::Verifier => "Verifies implementation meets acceptance criteria",
            WorkerType::Critic => "Reviews code quality and suggests improvements",
            WorkerType::Debugger => "Investigates and fixes failures",
            WorkerType::Coordinator => "Orchestrates the workflow of other workers",
        }
    }

    /// Get the typical order in the workflow.
    pub const fn execution_order(&self) -> u8 {
        match self {
            WorkerType::Coordinator => 0,
            WorkerType::Investigator => 1,
            WorkerType::Implementer => 2,
            WorkerType::Critic => 3,
            WorkerType::Verifier => 4,
            WorkerType::Debugger => 5,
        }
    }
}

impl std::fmt::Display for WorkerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerType::Investigator => write!(f, "investigator"),
            WorkerType::Implementer => write!(f, "implementer"),
            WorkerType::Verifier => write!(f, "verifier"),
            WorkerType::Critic => write!(f, "critic"),
            WorkerType::Debugger => write!(f, "debugger"),
            WorkerType::Coordinator => write!(f, "coordinator"),
        }
    }
}

/// Execution status of a worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    /// Waiting for dependencies
    Pending,
    /// Currently executing
    Running,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed,
    /// Paused by user
    Paused,
    /// Cancelled by user
    Cancelled,
}

impl WorkerStatus {
    /// Get display icon for this status.
    pub const fn icon(&self) -> &'static str {
        match self {
            WorkerStatus::Pending => "⏳",
            WorkerStatus::Running => "🔄",
            WorkerStatus::Completed => "✅",
            WorkerStatus::Failed => "❌",
            WorkerStatus::Paused => "⏸️",
            WorkerStatus::Cancelled => "🚫",
        }
    }

    /// Check if this is a terminal state.
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            WorkerStatus::Completed | WorkerStatus::Failed | WorkerStatus::Cancelled
        )
    }

    /// Check if the worker is actively running.
    pub const fn is_active(&self) -> bool {
        matches!(self, WorkerStatus::Running)
    }
}

impl std::fmt::Display for WorkerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerStatus::Pending => write!(f, "pending"),
            WorkerStatus::Running => write!(f, "running"),
            WorkerStatus::Completed => write!(f, "completed"),
            WorkerStatus::Failed => write!(f, "failed"),
            WorkerStatus::Paused => write!(f, "paused"),
            WorkerStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A Task represents a unit of work within an intent.
///
/// Tasks are created by decomposing an intent and assigned to workers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier
    pub id: TaskId,

    /// Human-readable title
    pub title: String,

    /// Detailed description
    pub description: String,

    /// Parent intent ID
    pub intent_id: IntentId,

    /// Assigned worker ID (if any)
    pub worker_id: Option<WorkerId>,

    /// Task status
    pub status: TaskStatus,

    /// Expected worker type
    pub required_worker_type: WorkerType,

    /// Estimated complexity (story points)
    pub complexity: Option<u8>,
}

/// Status of a task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Not started
    Todo,
    /// In progress
    InProgress,
    /// Under review
    InReview,
    /// Done
    Done,
    /// Blocked
    Blocked,
}

impl TaskStatus {
    /// Get display icon for this status.
    pub const fn icon(&self) -> &'static str {
        match self {
            TaskStatus::Todo => "⬜",
            TaskStatus::InProgress => "🔄",
            TaskStatus::InReview => "👀",
            TaskStatus::Done => "✅",
            TaskStatus::Blocked => "🚧",
        }
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Todo => write!(f, "todo"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::InReview => write!(f, "in_review"),
            TaskStatus::Done => write!(f, "done"),
            TaskStatus::Blocked => write!(f, "blocked"),
        }
    }
}

/// Spec frontmatter structure for YAML parsing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecFrontmatter {
    pub id: Option<String>,
    pub status: Option<IntentStatus>,
    pub created: Option<String>,
    pub updated: Option<String>,
    #[serde(default)]
    pub workers: Vec<WorkerSpec>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

/// Worker specification in frontmatter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerSpec {
    pub id: String,
    #[serde(rename = "type")]
    pub worker_type: WorkerType,
    pub status: WorkerStatus,
    pub agent: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Complete spec document (frontmatter + content).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecDocument {
    pub frontmatter: SpecFrontmatter,
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_new() {
        let intent = Intent::new(
            "Test Intent",
            PathBuf::from("test.md"),
            "2026-02-14T10:00:00Z",
        );
        assert_eq!(intent.title, "Test Intent");
        assert_eq!(intent.status, IntentStatus::Draft);
        assert!(intent.workers.is_empty());
    }

    #[test]
    fn test_intent_completion_percentage() {
        let mut intent = Intent::new("Test", PathBuf::from("test.md"), "2026-02-14T10:00:00Z");
        assert_eq!(intent.completion_percentage(), 0);

        intent
            .acceptance_criteria
            .push(Criterion::new("Criterion 1"));
        intent
            .acceptance_criteria
            .push(Criterion::new("Criterion 2"));
        assert_eq!(intent.completion_percentage(), 0);

        intent.acceptance_criteria[0].complete();
        assert_eq!(intent.completion_percentage(), 50);

        intent.acceptance_criteria[1].complete();
        assert_eq!(intent.completion_percentage(), 100);
    }

    #[test]
    fn test_worker_dependencies() {
        let worker1 = Worker::new(
            "worker1",
            WorkerType::Investigator,
            "intent-1",
            PathBuf::from("/repo/w1"),
            "branch1",
            "claude",
            PathBuf::from("/repo/log1"),
            "2026-02-14T10:00:00Z",
        );

        let worker2 = Worker::new(
            "worker2",
            WorkerType::Implementer,
            "intent-1",
            PathBuf::from("/repo/w2"),
            "branch2",
            "claude",
            PathBuf::from("/repo/log2"),
            "2026-02-14T10:00:00Z",
        )
        .depends_on(worker1.id.clone());

        assert!(!worker2.can_start(&[]));
        assert!(worker2.can_start(std::slice::from_ref(&worker1.id)));
    }

    #[test]
    fn test_worker_status_transitions() {
        let mut worker = Worker::new(
            "test",
            WorkerType::Investigator,
            "intent-1",
            PathBuf::from("/repo/w"),
            "branch",
            "claude",
            PathBuf::from("/repo/log"),
            "2026-02-14T10:00:00Z",
        );

        assert_eq!(worker.status, WorkerStatus::Pending);

        worker.mark_running();
        assert_eq!(worker.status, WorkerStatus::Running);

        worker.mark_completed("2026-02-14T11:00:00Z");
        assert_eq!(worker.status, WorkerStatus::Completed);
        assert_eq!(worker.exit_code, Some(0));
    }

    #[test]
    fn test_worker_type_order() {
        assert_eq!(WorkerType::Coordinator.execution_order(), 0);
        assert_eq!(WorkerType::Investigator.execution_order(), 1);
        assert_eq!(WorkerType::Implementer.execution_order(), 2);
        assert_eq!(WorkerType::Verifier.execution_order(), 4);
    }

    #[test]
    fn test_intent_status_display() {
        assert_eq!(IntentStatus::Draft.to_string(), "draft");
        assert_eq!(IntentStatus::InProgress.to_string(), "in_progress");
        assert_eq!(IntentStatus::Completed.to_string(), "completed");
    }

    #[test]
    fn test_worker_status_icons() {
        assert_eq!(WorkerStatus::Pending.icon(), "⏳");
        assert_eq!(WorkerStatus::Running.icon(), "🔄");
        assert_eq!(WorkerStatus::Completed.icon(), "✅");
        assert_eq!(WorkerStatus::Failed.icon(), "❌");
    }
}
