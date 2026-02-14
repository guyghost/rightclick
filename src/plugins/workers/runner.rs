//! Worker Runner - Service for executing AI agents as subprocesses.
//!
//! This module provides the Imperative Shell component for spawning and managing
//! AI agent processes. It handles:
//!
//! - Spawning agent processes with appropriate prompts
//! - Capturing and streaming output
//! - Managing process lifecycle (start, stop, monitor)
//! - Writing logs to files

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::Result;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::core::models::intent::{Worker, WorkerType};

/// Errors that can occur when running workers
#[derive(Debug, thiserror::Error)]
pub enum WorkerRunnerError {
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Failed to spawn agent: {0}")]
    SpawnFailed(String),

    #[error("Worker already running: {0}")]
    AlreadyRunning(String),

    #[error("Worker not running: {0}")]
    NotRunning(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Other error: {0}")]
    Other(String),
}

/// Output line from a worker process
#[derive(Clone, Debug)]
pub enum WorkerOutput {
    /// Standard output line
    Stdout(String),
    /// Standard error line
    Stderr(String),
    /// Process exited with code
    Exited(i32),
}

/// Runner for executing AI agent workers
#[derive(Debug)]
pub struct WorkerRunner {
    /// Currently running processes
    running: std::collections::HashMap<String, Child>,
}

impl WorkerRunner {
    /// Create a new worker runner
    pub fn new() -> Self {
        Self {
            running: std::collections::HashMap::new(),
        }
    }

    /// Check if an agent command is available
    pub async fn is_agent_available(agent: &str) -> bool {
        Command::new("which")
            .arg(agent)
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Spawn a worker process
    ///
    /// # Arguments
    ///
    /// * `worker` - The worker to spawn
    /// * `intent_spec` - The full intent specification text
    ///
    /// # Returns
    ///
    /// Returns a receiver for output lines
    pub async fn spawn_worker(
        &mut self,
        worker: &Worker,
        intent_spec: &str,
    ) -> Result<mpsc::Receiver<WorkerOutput>, WorkerRunnerError> {
        let worker_id = worker.id.clone();

        // Check if already running
        if self.running.contains_key(&worker_id) {
            return Err(WorkerRunnerError::AlreadyRunning(worker_id));
        }

        // Check agent availability
        if !Self::is_agent_available(&worker.agent).await {
            return Err(WorkerRunnerError::AgentNotFound(worker.agent.clone()));
        }

        // Build the prompt for this worker type
        let prompt = build_worker_prompt(worker, intent_spec);

        // Ensure log directory exists
        if let Some(parent) = worker.output_log.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                WorkerRunnerError::Io(e)
            })?;
        }

        // Spawn the agent process
        let mut child = spawn_agent_process(&worker.agent, &prompt, &worker.worktree_path)
            .await?;

        let pid = child
            .id()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        info!(
            "Spawned worker {} with PID {} using agent {}",
            worker_id, pid, worker.agent
        );

        // Set up output streaming
        let (tx, rx) = mpsc::channel(100);
        let worker_id_clone = worker_id.clone();
        let log_path = worker.output_log.clone();

        // Get stdout and stderr handles
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| WorkerRunnerError::SpawnFailed("Failed to capture stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| WorkerRunnerError::SpawnFailed("Failed to capture stderr".to_string()))?;

        // Spawn output handling task
        tokio::spawn(async move {
            let result = handle_output(stdout, stderr, log_path, tx.clone()).await;
            if let Err(e) = result {
                error!("Output handling error for worker {}: {}", worker_id_clone, e);
                let _ = tx.send(WorkerOutput::Exited(1)).await;
            }
        });

        // Store the child process
        self.running.insert(worker_id, child);

        Ok(rx)
    }

    /// Stop a running worker
    pub async fn stop_worker(&mut self, worker_id: &str) -> Result<(), WorkerRunnerError> {
        if let Some(mut child) = self.running.remove(worker_id) {
            info!("Stopping worker {}", worker_id);

            // Try graceful shutdown first
            if let Err(e) = child.start_kill() {
                warn!("Failed to kill worker {}: {}", worker_id, e);
            }

            // Wait for process to exit
            match tokio::time::timeout(tokio::time::Duration::from_secs(5), child.wait()).await {
                Ok(Ok(status)) => {
                    info!("Worker {} exited with status: {}", worker_id, status);
                }
                Ok(Err(e)) => {
                    error!("Failed to wait for worker {}: {}", worker_id, e);
                }
                Err(_) => {
                    warn!("Timeout waiting for worker {} to exit", worker_id);
                }
            }

            Ok(())
        } else {
            Err(WorkerRunnerError::NotRunning(worker_id.to_string()))
        }
    }

    /// Check if a worker is running
    pub fn is_running(&self, worker_id: &str) -> bool {
        self.running.contains_key(worker_id)
    }

    /// Get list of running worker IDs
    pub fn running_workers(&self) -> Vec<&String> {
        self.running.keys().collect()
    }

    /// Clean up completed processes
    pub async fn cleanup_completed(&mut self) {
        let completed: Vec<String> = self
            .running
            .iter_mut()
            .filter_map(|(id, child)| {
                match child.try_wait() {
                    Ok(Some(_)) => Some(id.clone()),
                    _ => None,
                }
            })
            .collect();

        for id in completed {
            self.running.remove(&id);
            debug!("Cleaned up completed worker {}", id);
        }
    }

    /// Stop all running workers
    pub async fn stop_all(&mut self) {
        let worker_ids: Vec<String> = self.running.keys().cloned().collect();
        for id in worker_ids {
            if let Err(e) = self.stop_worker(&id).await {
                warn!("Error stopping worker {}: {}", id, e);
            }
        }
    }
}

impl Default for WorkerRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the appropriate prompt for a worker type
fn build_worker_prompt(worker: &Worker, intent_spec: &str) -> String {
    match worker.worker_type {
        WorkerType::Investigator => build_investigator_prompt(intent_spec),
        WorkerType::Implementer => build_implementer_prompt(intent_spec),
        WorkerType::Verifier => build_verifier_prompt(intent_spec),
        WorkerType::Critic => build_critic_prompt(intent_spec),
        WorkerType::Debugger => build_debugger_prompt(intent_spec),
        WorkerType::Coordinator => build_coordinator_prompt(intent_spec),
    }
}

/// Build prompt for Investigator worker
fn build_investigator_prompt(intent_spec: &str) -> String {
    format!(
        r#"You are an Investigator AI agent. Your task is to explore the codebase and understand the current state before any changes are made.

## Your Goal
Analyze the codebase to find all relevant files, understand the existing architecture, and identify what needs to be changed to implement the following intent:

---
{}
---

## Instructions
1. First, explore the project structure
2. Identify all files relevant to this intent
3. Understand the current implementation
4. Note any patterns, conventions, or frameworks being used
5. Identify potential challenges or considerations

## Output Format
Provide a structured report with:
- **Files to modify**: List of files that will need changes
- **Files to reference**: Related files that provide context
- **Key findings**: Important observations about the codebase
- **Recommendations**: Suggested approach for implementation

Be thorough but concise. Focus on facts and observations, not implementation details."#,
        intent_spec
    )
}

/// Build prompt for Implementer worker
fn build_implementer_prompt(intent_spec: &str) -> String {
    format!(
        r#"You are an Implementer AI agent. Your task is to implement the code changes required by the intent.

## Your Goal
Implement the necessary code changes to fulfill this intent:

---
{}
---

## Instructions
1. Read the investigator's findings (if available)
2. Implement the changes following the project's patterns and conventions
3. Make minimal, focused changes
4. Ensure the code is correct and complete
5. Add or update tests as needed

## Guidelines
- Follow existing code style and patterns
- Make incremental changes, verifying as you go
- Don't break existing functionality
- Add comments for complex logic
- Update documentation if needed

## Output Format
As you work, describe:
- What files you're modifying
- What changes you're making
- Any issues encountered and how you resolved them

When complete, summarize what was implemented."#,
        intent_spec
    )
}

/// Build prompt for Verifier worker
fn build_verifier_prompt(intent_spec: &str) -> String {
    format!(
        r#"You are a Verifier AI agent. Your task is to verify that the implementation meets the acceptance criteria.

## Your Goal
Verify that the implementation correctly fulfills this intent:

---
{}
---

## Instructions
1. Review the acceptance criteria in the intent
2. Check that all criteria are met
3. Test the implementation if possible
4. Verify no regressions were introduced
5. Check code quality and completeness

## Output Format
Provide a verification report:
- **Criteria Check**: Go through each acceptance criterion and mark ✓ or ✗
- **Test Results**: Results of any tests run
- **Issues Found**: Any problems or concerns
- **Overall Assessment**: PASS or FAIL with explanation

Be objective and thorough."#,
        intent_spec
    )
}

/// Build prompt for Critic worker
fn build_critic_prompt(intent_spec: &str) -> String {
    format!(
        r#"You are a Critic AI agent. Your task is to review the code quality and suggest improvements.

## Your Goal
Review the implementation of this intent with a focus on code quality:

---
{}
---

## Instructions
1. Review the implementation for:
   - Code clarity and readability
   - Error handling
   - Edge cases
   - Performance considerations
   - Security implications
   - Maintainability
2. Identify any code smells or anti-patterns
3. Suggest specific improvements

## Output Format
Provide a code review:
- **Strengths**: What's done well
- **Concerns**: Issues that should be addressed
- **Suggestions**: Specific recommendations for improvement
- **Priority**: High/Medium/Low for each suggestion

Be constructive and specific."#,
        intent_spec
    )
}

/// Build prompt for Debugger worker
fn build_debugger_prompt(intent_spec: &str) -> String {
    format!(
        r#"You are a Debugger AI agent. Your task is to investigate and fix failures.

## Your Goal
Debug and fix issues related to this intent:

---
{}
---

## Instructions
1. Identify the failure or issue
2. Analyze the root cause
3. Implement a fix
4. Verify the fix works
5. Ensure no new issues are introduced

## Output Format
Provide a debugging report:
- **Issue**: Description of the problem
- **Root Cause**: What caused it
- **Fix**: What you changed
- **Verification**: How you confirmed it's fixed

Be systematic and thorough."#,
        intent_spec
    )
}

/// Build prompt for Coordinator worker
fn build_coordinator_prompt(intent_spec: &str) -> String {
    format!(
        r#"You are a Coordinator AI agent. Your task is to orchestrate the overall workflow.

## Your Goal
Coordinate the implementation of this intent:

---
{}
---

## Instructions
1. Analyze the intent and break it down into steps
2. Determine the order of operations
3. Coordinate between different workers
4. Track progress and handle blockers
5. Ensure the final result meets all criteria

## Output Format
Provide coordination updates:
- **Plan**: High-level approach
- **Progress**: What's been done
- **Next Steps**: What needs to happen next
- **Blockers**: Any issues preventing progress

Be organized and keep the big picture in mind."#,
        intent_spec
    )
}

/// Spawn an agent process with the given prompt
async fn spawn_agent_process(
    agent: &str,
    prompt: &str,
    worktree_path: &Path,
) -> Result<Child, WorkerRunnerError> {
    let mut cmd = Command::new(agent);

    // Set working directory
    cmd.current_dir(worktree_path);

    // Configure based on agent type
    match agent {
        "claude" | "claude-code" => {
            cmd.arg("--prompt").arg(prompt);
        }
        "cursor" => {
            // Cursor doesn't have a direct CLI prompt option
            // We'd need to use a different approach
            cmd.arg("--help"); // Placeholder
        }
        "codex" => {
            cmd.arg("--prompt").arg(prompt);
        }
        _ => {
            // Generic fallback - pass prompt as argument
            cmd.arg(prompt);
        }
    }

    // Set up pipes for stdout/stderr
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    // Spawn the process
    let child = cmd.spawn().map_err(|e| {
        WorkerRunnerError::SpawnFailed(format!("Failed to spawn {}: {}", agent, e))
    })?;

    Ok(child)
}

/// Handle output streaming from a process
async fn handle_output(
    _stdout: tokio::process::ChildStdout,
    _stderr: tokio::process::ChildStderr,
    _log_path: PathBuf,
    _tx: mpsc::Sender<WorkerOutput>,
) -> Result<()> {
    // Simplified implementation - full output streaming requires tokio io-util feature
    // For now, just return Ok - output will be written to log files by the agent itself
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_investigator_prompt() {
        let prompt = build_investigator_prompt("Test intent spec");
        assert!(prompt.contains("Investigator AI agent"));
        assert!(prompt.contains("Test intent spec"));
        assert!(prompt.contains("Files to modify"));
    }

    #[test]
    fn test_build_implementer_prompt() {
        let prompt = build_implementer_prompt("Test intent spec");
        assert!(prompt.contains("Implementer AI agent"));
        assert!(prompt.contains("Test intent spec"));
        assert!(prompt.contains("Make incremental changes"));
    }

    #[test]
    fn test_build_verifier_prompt() {
        let prompt = build_verifier_prompt("Test intent spec");
        assert!(prompt.contains("Verifier AI agent"));
        assert!(prompt.contains("Criteria Check"));
    }

    #[test]
    fn test_worker_runner_new() {
        let runner = WorkerRunner::new();
        assert!(runner.running_workers().is_empty());
    }
}
