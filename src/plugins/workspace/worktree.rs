//! Worktree Operations
//!
//! This module provides git worktree operations and tmux integration
//! for managing development environments.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::process::Command;

use super::state::{ShellSession, Worktree};

/// Manages git worktree operations
#[derive(Debug)]
pub struct WorktreeManager {
    /// Repository root path (main worktree)
    repo_path: PathBuf,
}

impl WorktreeManager {
    /// Create a new worktree manager
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    /// List all worktrees in the repository
    pub async fn list_worktrees(&self) -> Result<Vec<Worktree>> {
        let output = Command::new("git")
            .args(["-C", &self.repo_path.to_string_lossy(), "worktree", "list", "--porcelain"])
            .output()
            .await
            .context("Failed to execute git worktree list")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("git worktree list failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut worktrees = Vec::new();
        let mut current_worktree: Option<WorktreeBuilder> = None;

        for line in stdout.lines() {
            if line.is_empty() {
                // End of worktree entry
                if let Some(builder) = current_worktree.take() {
                    worktrees.push(builder.build()?);
                }
                continue;
            }

            if line.starts_with("worktree ") {
                // New worktree entry
                if let Some(builder) = current_worktree.take() {
                    worktrees.push(builder.build()?);
                }
                let path = line.strip_prefix("worktree ").unwrap_or("");
                current_worktree = Some(WorktreeBuilder::new(path));
            } else if let Some(ref mut builder) = current_worktree {
                if line.starts_with("branch ") {
                    let branch = line.strip_prefix("branch ").unwrap_or("");
                    // Extract short branch name from refs/heads/...
                    let branch = branch.strip_prefix("refs/heads/").unwrap_or(branch);
                    builder.branch(branch);
                } else if line == "bare" {
                    builder.is_bare(true);
                } else if line == "detached" {
                    builder.is_detached(true);
                } else if line.starts_with("HEAD ") {
                    let head = line.strip_prefix("HEAD ").unwrap_or("");
                    builder.head(head);
                }
            }
        }

        // Don't forget the last entry
        if let Some(builder) = current_worktree {
            worktrees.push(builder.build()?);
        }

        // Enhance worktree info
        for worktree in &mut worktrees {
            // Check if dirty
            worktree.is_dirty = self.is_worktree_dirty(&worktree.path).await.unwrap_or(false);

            // Get last commit
            worktree.last_commit = self.get_last_commit(&worktree.path).await.ok();

            // Detect if main worktree
            worktree.is_main = worktree.path == self.repo_path;
        }

        Ok(worktrees)
    }

    /// Create a new worktree
    pub async fn create_worktree(
        &self,
        name: &str,
        branch: Option<&str>,
        base_branch: Option<&str>,
    ) -> Result<PathBuf> {
        let worktree_path = self.repo_path.parent().unwrap_or(&self.repo_path).join(name);

        // Check if path already exists
        if worktree_path.exists() {
            return Err(anyhow::anyhow!(
                "Worktree path already exists: {}",
                worktree_path.display()
            ));
        }

        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.repo_path)
            .arg("worktree")
            .arg("add");

        // If branch is specified, use it
        if let Some(branch_name) = branch {
            // Check if branch exists
            let branch_exists = self.branch_exists(branch_name).await?;

            if branch_exists {
                cmd.arg("-B").arg(branch_name);
            } else {
                // Create new branch from base or current
                if let Some(base) = base_branch {
                    cmd.arg("-b").arg(branch_name).arg(base);
                } else {
                    cmd.arg("-b").arg(branch_name);
                }
            }
        }

        cmd.arg(&worktree_path);

        let output = cmd
            .output()
            .await
            .context("Failed to execute git worktree add")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to create worktree: {}", stderr));
        }

        Ok(worktree_path)
    }

    /// Delete a worktree
    pub async fn delete_worktree(&self, path: &Path, force: bool) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.repo_path)
            .arg("worktree")
            .arg("remove");

        if force {
            cmd.arg("--force");
        }

        cmd.arg(path);

        let output = cmd
            .output()
            .await
            .context("Failed to execute git worktree remove")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to delete worktree: {}", stderr));
        }

        Ok(())
    }

    /// Prune worktree metadata
    pub async fn prune_worktrees(&self) -> Result<()> {
        let output = Command::new("git")
            .args(["-C", &self.repo_path.to_string_lossy(), "worktree", "prune"])
            .output()
            .await
            .context("Failed to execute git worktree prune")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to prune worktrees: {}", stderr));
        }

        Ok(())
    }

    /// Check if a worktree has uncommitted changes
    async fn is_worktree_dirty(&self, path: &Path) -> Result<bool> {
        let output = Command::new("git")
            .args([
                "-C",
                &path.to_string_lossy(),
                "status",
                "--porcelain",
                "--untracked-files=no",
            ])
            .output()
            .await
            .context("Failed to check worktree status")?;

        if !output.status.success() {
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(!stdout.trim().is_empty())
    }

    /// Get the last commit message for a worktree
    async fn get_last_commit(&self, path: &Path) -> Result<String> {
        let output = Command::new("git")
            .args([
                "-C",
                &path.to_string_lossy(),
                "log",
                "-1",
                "--pretty=format:%s",
            ])
            .output()
            .await
            .context("Failed to get last commit")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to get last commit"));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Check if a branch exists
    async fn branch_exists(&self, branch: &str) -> Result<bool> {
        let output = Command::new("git")
            .args([
                "-C",
                &self.repo_path.to_string_lossy(),
                "branch",
                "--list",
                branch,
            ])
            .output()
            .await
            .context("Failed to list branches")?;

        if !output.status.success() {
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(!stdout.trim().is_empty())
    }

    /// Get diff for worktree
    pub async fn get_worktree_diff(&self, path: &Path) -> Result<String> {
        let output = Command::new("git")
            .args(["-C", &path.to_string_lossy(), "diff", "HEAD"])
            .output()
            .await
            .context("Failed to get worktree diff")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to get diff: {}", stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Get worktree status summary
    pub async fn get_status_summary(&self, path: &Path) -> Result<WorktreeStatus> {
        let output = Command::new("git")
            .args([
                "-C",
                &path.to_string_lossy(),
                "status",
                "--porcelain",
            ])
            .output()
            .await
            .context("Failed to get status summary")?;

        if !output.status.success() {
            return Ok(WorktreeStatus::default());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut staged = 0;
        let mut unstaged = 0;
        let mut untracked = 0;

        for line in stdout.lines() {
            if line.len() < 2 {
                continue;
            }
            let index_status = line.chars().next().unwrap();
            let worktree_status = line.chars().nth(1).unwrap();

            match index_status {
                '?' => untracked += 1,
                ' ' => {
                    if worktree_status != ' ' {
                        unstaged += 1;
                    }
                }
                _ => staged += 1,
            }

            if worktree_status != ' ' && worktree_status != '?' {
                unstaged += 1;
            }
        }

        Ok(WorktreeStatus {
            staged,
            unstaged,
            untracked,
            is_dirty: staged > 0 || unstaged > 0,
        })
    }
}

/// Status summary for a worktree
#[derive(Clone, Copy, Debug, Default)]
pub struct WorktreeStatus {
    /// Number of staged changes
    pub staged: usize,
    /// Number of unstaged changes
    pub unstaged: usize,
    /// Number of untracked files
    pub untracked: usize,
    /// Whether the worktree has any changes
    pub is_dirty: bool,
}

/// Builder for constructing Worktree entries
struct WorktreeBuilder {
    path: PathBuf,
    branch: Option<String>,
    head: Option<String>,
    is_bare: bool,
    is_detached: bool,
}

impl WorktreeBuilder {
    fn new(path: &str) -> Self {
        Self {
            path: PathBuf::from(path),
            branch: None,
            head: None,
            is_bare: false,
            is_detached: false,
        }
    }

    fn branch(&mut self, branch: &str) {
        self.branch = Some(branch.to_string());
    }

    fn head(&mut self, head: &str) {
        self.head = Some(head.to_string());
    }

    fn is_bare(&mut self, bare: bool) {
        self.is_bare = bare;
    }

    fn is_detached(&mut self, detached: bool) {
        self.is_detached = detached;
    }

    fn build(self) -> Result<Worktree> {
        let name = self
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let branch = if self.is_detached {
            self.head
                .map(|h| format!("detached@{})", &h[..7.min(h.len())]))
                .unwrap_or_else(|| "detached".to_string())
        } else {
            self.branch.unwrap_or_else(|| "unknown".to_string())
        };

        Ok(Worktree::new(name, self.path, branch))
    }
}

/// Tmux session manager
pub struct TmuxManager;

impl TmuxManager {
    /// List all tmux sessions
    pub async fn list_sessions() -> Result<Vec<ShellSession>> {
        let output = Command::new("tmux")
            .args(["list-sessions", "-F", "#{session_name}:#{session_id}"])
            .output()
            .await
            .context("Failed to list tmux sessions")?;

        // tmux returns error if no sessions exist
        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut sessions = Vec::new();

        for (idx, line) in stdout.lines().enumerate() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 2 {
                let name = parts[0].to_string();
                let _id = parts[1].to_string();
                sessions.push(ShellSession::new(
                    format!("{}", idx),
                    name.clone(),
                    name,
                ));
            }
        }

        Ok(sessions)
    }

    /// Create a new tmux session for a worktree
    pub async fn create_session(name: &str, path: &Path) -> Result<String> {
        // Check if session already exists
        if Self::session_exists(name).await? {
            return Ok(name.to_string());
        }

        let output = Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                name,
                "-c",
                &path.to_string_lossy(),
            ])
            .output()
            .await
            .context("Failed to create tmux session")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to create tmux session: {}", stderr));
        }

        Ok(name.to_string())
    }

    /// Kill a tmux session
    pub async fn kill_session(name: &str) -> Result<()> {
        let output = Command::new("tmux")
            .args(["kill-session", "-t", name])
            .output()
            .await
            .context("Failed to kill tmux session")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to kill tmux session: {}", stderr));
        }

        Ok(())
    }

    /// Attach to a tmux session (in a new terminal)
    pub async fn attach_session(name: &str) -> Result<()> {
        // Try different terminal emulators
        let terminals = ["alacritty", "kitty", "wezterm", "gnome-terminal", "xterm"];

        for term in &terminals {
            if Self::command_exists(term).await {
                let _ = Command::new(term)
                    .args(["-e", "tmux", "attach", "-t", name])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
                return Ok(());
            }
        }

        // Fallback: just switch client if already in tmux
        let output = Command::new("tmux")
            .args(["switch-client", "-t", name])
            .output()
            .await;

        if output.is_ok() {
            return Ok(());
        }

        Err(anyhow::anyhow!("Could not attach to tmux session"))
    }

    /// Check if a session exists
    async fn session_exists(name: &str) -> Result<bool> {
        let output = Command::new("tmux")
            .args(["has-session", "-t", name])
            .output()
            .await;

        match output {
            Ok(out) => Ok(out.status.success()),
            Err(_) => Ok(false),
        }
    }

    /// Check if a command exists
    async fn command_exists(cmd: &str) -> bool {
        Command::new("which")
            .arg(cmd)
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

/// AI Agent launcher
pub struct AgentLauncher;

impl AgentLauncher {
    /// Launch an AI agent in the worktree
    pub async fn launch_agent(worktree_path: &Path, task: Option<&str>) -> Result<String> {
        let mut cmd = Command::new("tmux");
        cmd.args([
            "new-window",
            "-c",
            &worktree_path.to_string_lossy(),
            "-n",
            "agent",
        ]);

        // Build agent command
        let agent_cmd = if let Some(t) = task {
            format!("kimi task '{}'", t)
        } else {
            "kimi".to_string()
        };

        cmd.arg(agent_cmd);

        let output = cmd
            .output()
            .await
            .context("Failed to launch AI agent")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to launch agent: {}", stderr));
        }

        Ok("Agent launched".to_string())
    }

    /// Check if kimi CLI is available
    pub async fn is_kimi_available() -> bool {
        Command::new("which")
            .arg("kimi")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worktree_status_default() {
        let status = WorktreeStatus::default();
        assert_eq!(status.staged, 0);
        assert_eq!(status.unstaged, 0);
        assert_eq!(status.untracked, 0);
        assert!(!status.is_dirty);
    }

    #[test]
    fn test_worktree_builder() {
        let builder = WorktreeBuilder::new("/repo/feature");
        let worktree = builder.build().unwrap();
        assert_eq!(worktree.name, "feature");
    }

    #[test]
    fn test_worktree_manager_new() {
        let manager = WorktreeManager::new(PathBuf::from("/repo"));
        assert_eq!(manager.repo_path, PathBuf::from("/repo"));
    }
}
