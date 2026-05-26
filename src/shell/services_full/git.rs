//! Git service implementation using command-line git
//!
//! This module provides git operations by shelling out to the git command
//! rather than using a native git library. This avoids heavy dependencies.

use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::process::Command;
use tracing::{debug, instrument};

use crate::core::models::{
    Branch, ChangeType, Commit, Diff, DiffHunk, DiffLine, FileChange, FileDiff, FileStatus, Remote,
    RepoStatus,
};

/// Trait for git operations
#[async_trait]
pub trait GitService: Send + Sync {
    /// Get repository status
    async fn status(&self, repo_path: &Path) -> Result<RepoStatus>;

    /// Get diff for a file
    async fn diff(&self, repo_path: &Path, file: &Path) -> Result<Diff>;

    /// Get diff for a file based on its status (staged, unstaged, or untracked)
    async fn diff_file(&self, repo_path: &Path, file: &Path, status: FileStatus) -> Result<Diff>;

    /// Get recent commits
    async fn commits(&self, repo_path: &Path, limit: usize) -> Result<Vec<Commit>>;

    /// Stage a file
    async fn stage(&self, repo_path: &Path, file: &Path) -> Result<()>;

    /// Unstage a file
    async fn unstage(&self, repo_path: &Path, file: &Path) -> Result<()>;

    /// Commit with message
    async fn commit(&self, repo_path: &Path, message: &str) -> Result<()>;

    /// Get current branch
    async fn current_branch(&self, repo_path: &Path) -> Result<Option<String>>;

    /// List branches
    async fn branches(&self, repo_path: &Path) -> Result<Vec<Branch>>;

    /// List remotes
    async fn remotes(&self, repo_path: &Path) -> Result<Vec<Remote>>;

    /// Get commit details including changed files
    async fn commit_details(&self, repo_path: &Path, commit_hash: &str) -> Result<Vec<FileDiff>>;

    /// Get the full diff for a specific commit
    async fn commit_diff(&self, repo_path: &Path, commit_hash: &str) -> Result<Diff>;

    /// Checkout a branch
    async fn checkout(&self, repo_path: &Path, branch: &str) -> Result<()>;

    /// Create a new branch
    async fn create_branch(&self, repo_path: &Path, name: &str) -> Result<()>;

    /// Delete a branch
    async fn delete_branch(&self, repo_path: &Path, name: &str) -> Result<()>;

    /// Push to remote
    async fn push(&self, repo_path: &Path, remote: &str, branch: &str) -> Result<()>;

    /// Pull from remote
    async fn pull(&self, repo_path: &Path, remote: &str, branch: &str) -> Result<()>;

    /// List stashes
    async fn stash_list(&self, repo_path: &Path) -> Result<Vec<crate::core::models::Stash>>;

    /// Save a stash
    async fn stash_save(&self, repo_path: &Path, message: Option<&str>) -> Result<()>;

    /// Pop the top stash
    async fn stash_pop(&self, repo_path: &Path, index: usize) -> Result<()>;

    /// Drop a stash
    async fn stash_drop(&self, repo_path: &Path, index: usize) -> Result<()>;
}

/// Git service implementation using command-line git
#[derive(Debug, Clone, Default)]
pub struct CliGitService;

impl CliGitService {
    /// Create a new CLI git service
    pub fn new() -> Self {
        Self
    }

    /// Parse unified diff output into a Diff struct
    fn parse_diff_output(&self, stdout: &str, file: &Path) -> Result<Diff> {
        let mut hunks = Vec::new();
        let mut current_hunk: Option<DiffHunk> = None;
        let mut lines = Vec::new();
        let mut total_additions: usize = 0;
        let mut total_deletions: usize = 0;

        for line in stdout.lines() {
            if line.starts_with("@@") {
                if let Some(hunk) = current_hunk.take() {
                    hunks.push(DiffHunk {
                        old_start: hunk.old_start,
                        old_lines: hunk.old_lines,
                        new_start: hunk.new_start,
                        new_lines: hunk.new_lines,
                        header: hunk.header.clone(),
                        lines: std::mem::take(&mut lines),
                    });
                }

                let header = line.to_string();
                let parts: Vec<&str> = line.split_whitespace().collect();
                let old_range = parts.get(1).unwrap_or(&"-0,0");
                let new_range = parts.get(2).unwrap_or(&"+0,0");
                let (old_start, old_lines) = parse_range(old_range);
                let (new_start, new_lines) = parse_range(new_range);

                current_hunk = Some(DiffHunk {
                    old_start: old_start as u32,
                    old_lines: old_lines as u32,
                    new_start: new_start as u32,
                    new_lines: new_lines as u32,
                    header,
                    lines: Vec::new(),
                });
            } else if !line.starts_with("---")
                && !line.starts_with("+++")
                && !line.starts_with("diff")
                && !line.starts_with("index ")
                && !line.starts_with("new file")
                && !line.starts_with("old mode")
                && !line.starts_with("new mode")
            {
                let diff_line = if let Some(content) = line.strip_prefix('+') {
                    total_additions += 1;
                    DiffLine {
                        content: content.to_string(),
                        change_type: ChangeType::Added,
                        old_line_no: None,
                        new_line_no: None,
                    }
                } else if let Some(content) = line.strip_prefix('-') {
                    total_deletions += 1;
                    DiffLine {
                        content: content.to_string(),
                        change_type: ChangeType::Deleted,
                        old_line_no: None,
                        new_line_no: None,
                    }
                } else if let Some(content) = line.strip_prefix(' ') {
                    DiffLine {
                        content: content.to_string(),
                        change_type: ChangeType::Context,
                        old_line_no: None,
                        new_line_no: None,
                    }
                } else {
                    DiffLine {
                        content: line.to_string(),
                        change_type: ChangeType::Context,
                        old_line_no: None,
                        new_line_no: None,
                    }
                };
                lines.push(diff_line);
            }
        }

        if let Some(hunk) = current_hunk {
            hunks.push(DiffHunk {
                old_start: hunk.old_start,
                old_lines: hunk.old_lines,
                new_start: hunk.new_start,
                new_lines: hunk.new_lines,
                header: hunk.header,
                lines,
            });
        }

        let file_diff = FileDiff {
            path: file.to_string_lossy().to_string(),
            old_path: None,
            is_binary: false,
            old_mode: None,
            new_mode: None,
            old_hash: None,
            new_hash: None,
            hunks,
            additions: total_additions,
            deletions: total_deletions,
            is_created: false,
            is_deleted: false,
            is_renamed: false,
        };

        Ok(Diff {
            files: vec![file_diff],
            files_changed: 1,
            total_additions,
            total_deletions,
        })
    }

    /// Run a git command
    async fn run_git(&self, repo_path: &Path, args: &[&str]) -> Result<std::process::Output> {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute git command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git command failed: {}", stderr);
        }

        Ok(output)
    }

    /// Parse git status porcelain output
    fn parse_status_output(&self, output: &str) -> Vec<FileChange> {
        let mut files = Vec::new();

        for line in output.lines() {
            if line.starts_with("## ") {
                continue;
            }

            let mut chars = line.char_indices();
            let Some((_, index_status)) = chars.next() else {
                continue;
            };
            let Some((_, worktree_status)) = chars.next() else {
                continue;
            };
            let Some((separator_index, ' ')) = chars.next() else {
                continue;
            };

            let path = line[separator_index + 1..].to_string();

            let status = match (index_status, worktree_status) {
                (' ', 'M') => FileStatus::Modified,
                ('M', ' ') => FileStatus::Staged,
                ('M', 'M') => FileStatus::Modified, // Staged and modified
                ('A', ' ') => FileStatus::Staged,   // Added
                ('A', 'M') => FileStatus::Staged,
                ('D', ' ') => FileStatus::Deleted,
                (' ', 'D') => FileStatus::Deleted,
                ('R', ' ') => FileStatus::Renamed,
                ('R', 'M') => FileStatus::Renamed,
                ('C', ' ') => FileStatus::TypeChanged,
                ('?', '?') => FileStatus::Untracked,
                ('!', '!') => FileStatus::Ignored,
                _ => FileStatus::Modified,
            };

            files.push(FileChange {
                path,
                status,
                old_path: None,
                additions: None,
                deletions: None,
            });
        }

        files
    }

    /// Parse git log output
    #[allow(dead_code)]
    fn parse_log_output(&self, output: &str) -> Vec<Commit> {
        let mut commits = Vec::new();
        let mut current_hash = String::new();
        let mut current_subject = String::new();
        let mut current_author = String::new();
        let mut current_date = String::new();

        for line in output.lines() {
            if let Some(hash) = line.strip_prefix("commit ") {
                // Save previous commit if exists
                if !current_hash.is_empty() {
                    if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&current_date) {
                        commits.push(Commit {
                            hash: current_hash.clone(),
                            short_hash: short_hash(&current_hash),
                            subject: current_subject.clone(),
                            author: current_author.clone(),
                            author_email: None,
                            date: date.with_timezone(&chrono::Utc),
                            message: Some(current_subject.clone()),
                            parents: Vec::new(),
                            files_changed: None,
                            insertions: None,
                            deletions: None,
                        });
                    }
                }
                current_hash = hash.to_string();
            } else if let Some(author) = line.strip_prefix("Author: ") {
                current_author = author.to_string();
            } else if let Some(date) = line.strip_prefix("Date: ") {
                current_date = date.to_string();
            } else if !line.starts_with(' ') && !line.is_empty() {
                current_subject = line.to_string();
            }
        }

        // Add last commit
        if !current_hash.is_empty() {
            if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&current_date) {
                commits.push(Commit {
                    hash: current_hash.clone(),
                    short_hash: short_hash(&current_hash),
                    subject: current_subject.clone(),
                    author: current_author.clone(),
                    author_email: None,
                    date: date.with_timezone(&chrono::Utc),
                    message: Some(current_subject.clone()),
                    parents: Vec::new(),
                    files_changed: None,
                    insertions: None,
                    deletions: None,
                });
            }
        }

        commits
    }

    /// Parse commits from pipe-delimited format: hash|author|date|subject
    fn parse_commits_delimited(&self, output: &str) -> Vec<Commit> {
        let mut commits = Vec::new();

        for line in output.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 4 {
                let hash = parts[0].to_string();
                let author = parts[1].to_string();
                let date_str = parts[2];
                let subject = parts[3].to_string();

                // Try to parse date in various formats
                let date = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date_str) {
                    dt.with_timezone(&chrono::Utc)
                } else if let Ok(dt) =
                    chrono::DateTime::parse_from_str(date_str, "%a %b %e %H:%M:%S %Y %z")
                {
                    dt.with_timezone(&chrono::Utc)
                } else {
                    // Fallback to current time if parsing fails
                    chrono::Utc::now()
                };

                commits.push(Commit {
                    hash: hash.clone(),
                    short_hash: short_hash(&hash),
                    subject,
                    author,
                    author_email: None,
                    date,
                    message: None,
                    parents: Vec::new(),
                    files_changed: None,
                    insertions: None,
                    deletions: None,
                });
            }
        }

        commits
    }
}

#[async_trait]
impl GitService for CliGitService {
    #[instrument(skip(self))]
    async fn status(&self, repo_path: &Path) -> Result<RepoStatus> {
        debug!("Getting git status for {}", repo_path.display());

        let output = self
            .run_git(repo_path, &["status", "--porcelain", "-b"])
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let files = self.parse_status_output(&stdout);

        let (branch_name, ahead, behind) = stdout
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("## "))
            .map(parse_branch_status)
            .unwrap_or_else(|| ("main".to_string(), 0, 0));

        // Categorize files
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();
        let mut conflicted = Vec::new();

        for file in files {
            match file.status {
                FileStatus::Staged => staged.push(file),
                FileStatus::Modified | FileStatus::Deleted => unstaged.push(file),
                FileStatus::Untracked => untracked.push(file),
                FileStatus::Conflicted => conflicted.push(file),
                _ => {}
            }
        }

        let is_dirty = !staged.is_empty() || !unstaged.is_empty() || !untracked.is_empty();

        Ok(RepoStatus {
            branch: branch_name,
            head: String::new(),
            is_dirty,
            staged,
            unstaged,
            untracked,
            conflicted,
            ahead,
            behind,
            state: Some(crate::core::models::RepoState::Clean),
        })
    }

    #[instrument(skip(self))]
    async fn diff(&self, repo_path: &Path, file: &Path) -> Result<Diff> {
        debug!(
            "Getting diff for {} in {}",
            file.display(),
            repo_path.display()
        );

        let output = self
            .run_git(
                repo_path,
                &["diff", "--cached", "--", file.to_str().unwrap_or(".")],
            )
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        self.parse_diff_output(&stdout, file)
    }

    async fn diff_file(&self, repo_path: &Path, file: &Path, status: FileStatus) -> Result<Diff> {
        debug!(
            "Getting diff for {} (status={:?}) in {}",
            file.display(),
            status,
            repo_path.display()
        );

        let file_str = file.to_str().unwrap_or(".");

        let output_result = match status {
            FileStatus::Staged => {
                self.run_git(repo_path, &["diff", "--cached", "--", file_str])
                    .await
            }
            FileStatus::Untracked => {
                // For untracked files, use --no-index which returns exit code 1 for differences
                let output = Command::new("git")
                    .arg("-C")
                    .arg(repo_path)
                    .args(["diff", "--no-index", "--", "/dev/null", file_str])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
                    .context("Failed to execute git diff --no-index")?;
                // --no-index returns exit code 1 when there are differences, which is normal
                Ok(output)
            }
            _ => {
                // Modified, Deleted, Renamed, etc.
                self.run_git(repo_path, &["diff", "--", file_str]).await
            }
        };

        let output = output_result?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        self.parse_diff_output(&stdout, file)
    }

    #[instrument(skip(self))]
    async fn commits(&self, repo_path: &Path, limit: usize) -> Result<Vec<Commit>> {
        debug!("Getting {} commits for {}", limit, repo_path.display());

        // Use a more robust format with clear separators
        let format = "--format=%H|%an|%aI|%s";
        let output = self
            .run_git(repo_path, &["log", format, &format!("-n{}", limit)])
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        Ok(self.parse_commits_delimited(&stdout))
    }

    #[instrument(skip(self))]
    async fn stage(&self, repo_path: &Path, file: &Path) -> Result<()> {
        debug!("Staging {} in {}", file.display(), repo_path.display());
        self.run_git(repo_path, &["add", file.to_str().unwrap_or(".")])
            .await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn unstage(&self, repo_path: &Path, file: &Path) -> Result<()> {
        debug!("Unstaging {} in {}", file.display(), repo_path.display());
        self.run_git(repo_path, &["reset", "HEAD", file.to_str().unwrap_or(".")])
            .await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn commit(&self, repo_path: &Path, message: &str) -> Result<()> {
        debug!(
            "Committing in {} with message: {}",
            repo_path.display(),
            message
        );
        self.run_git(repo_path, &["commit", "-m", message]).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn current_branch(&self, repo_path: &Path) -> Result<Option<String>> {
        let output = self
            .run_git(repo_path, &["branch", "--show-current"])
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let branch = stdout.trim().to_string();

        if branch.is_empty() {
            Ok(None)
        } else {
            Ok(Some(branch))
        }
    }

    #[instrument(skip(self))]
    async fn branches(&self, repo_path: &Path) -> Result<Vec<Branch>> {
        let output = self
            .run_git(repo_path, &["branch", "-a", "--format=%(refname:short)"])
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let branches = stdout
            .lines()
            .map(|name| Branch {
                name: name.to_string(),
                full_name: name.to_string(),
                is_remote: name.starts_with("remotes/"),
                is_current: false,
                upstream: None,
                commit_hash: String::new(),
                ahead: None,
                behind: None,
                remote: None,
            })
            .collect();

        Ok(branches)
    }

    #[instrument(skip(self))]
    async fn remotes(&self, repo_path: &Path) -> Result<Vec<Remote>> {
        let output = self.run_git(repo_path, &["remote", "-v"]).await?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut remotes = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[0].to_string();
                let url = parts[1].to_string();

                if seen.insert(name.clone()) {
                    remotes.push(Remote {
                        name,
                        fetch_url: url,
                        push_url: None,
                        default_branch: None,
                    });
                }
            }
        }

        Ok(remotes)
    }

    #[instrument(skip(self))]
    async fn commit_details(&self, repo_path: &Path, commit_hash: &str) -> Result<Vec<FileDiff>> {
        // Use --name-status to get file status (A=Added, M=Modified, D=Deleted, R=Renamed)
        let output = self
            .run_git(
                repo_path,
                &["show", "--name-status", "--format=", commit_hash],
            )
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse name-status output
        let mut file_diffs = Vec::new();

        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Parse lines like "M	src/main.rs" or "A	new_file.txt" or "R100	old	new"
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.is_empty() {
                continue;
            }

            let status = parts[0];
            let is_renamed = status.starts_with('R');
            let is_created = status == "A";
            let is_deleted = status == "D";
            let is_modified = status == "M";

            if !is_renamed && !is_created && !is_deleted && !is_modified {
                continue;
            }

            let (path, old_path) = if is_renamed && parts.len() >= 3 {
                (parts[2].to_string(), Some(parts[1].to_string()))
            } else if parts.len() >= 2 {
                (parts[1].to_string(), None)
            } else {
                continue;
            };

            file_diffs.push(FileDiff {
                path,
                old_path,
                is_binary: false,
                old_mode: None,
                new_mode: None,
                old_hash: None,
                new_hash: None,
                hunks: Vec::new(),
                additions: 0,
                deletions: 0,
                is_created,
                is_deleted,
                is_renamed,
            });
        }

        // Now get the diff stats to populate additions/deletions
        // Use --numstat which gives exact line counts: "<additions>\t<deletions>\t<file>"
        if !file_diffs.is_empty() {
            if let Ok(stats_output) = self
                .run_git(repo_path, &["show", "--numstat", "--format=", commit_hash])
                .await
            {
                let stats_stdout = String::from_utf8_lossy(&stats_output.stdout);
                for line in stats_stdout.lines() {
                    // Format: "10\t5\tfile.txt" (10 additions, 5 deletions)
                    // Binary files: "-\t-\tfile.bin"
                    let parts: Vec<&str> = line.split('\t').collect();
                    if parts.len() >= 3 {
                        let additions_str = parts[0].trim();
                        let deletions_str = parts[1].trim();
                        let file_path = parts[2].trim();

                        // Check for binary files (-\t-\tfile)
                        if additions_str == "-" && deletions_str == "-" {
                            if let Some(fd) = file_diffs.iter_mut().find(|fd| fd.path == file_path)
                            {
                                fd.is_binary = true;
                                fd.additions = 0;
                                fd.deletions = 0;
                            }
                            continue;
                        }

                        // Parse additions and deletions
                        if let (Ok(additions), Ok(deletions)) = (
                            additions_str.parse::<usize>(),
                            deletions_str.parse::<usize>(),
                        ) {
                            if let Some(fd) = file_diffs.iter_mut().find(|fd| fd.path == file_path)
                            {
                                fd.additions = additions;
                                fd.deletions = deletions;
                            }
                        }
                    }
                }
            }
        }

        Ok(file_diffs)
    }

    #[instrument(skip(self))]
    async fn commit_diff(&self, repo_path: &Path, commit_hash: &str) -> Result<Diff> {
        // Get the full diff with patch content
        let output = self
            .run_git(repo_path, &["show", "--patch", "--no-color", commit_hash])
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse the diff output
        let mut files = Vec::new();
        let mut current_file: Option<FileDiff> = None;
        let mut current_hunk: Option<DiffHunk> = None;
        let mut total_additions = 0;
        let mut total_deletions = 0;

        for line in stdout.lines() {
            // New file diff starts with "diff --git"
            if line.starts_with("diff --git") {
                // Save previous file if exists
                if let Some(hunk) = current_hunk.take() {
                    if let Some(ref mut file) = current_file {
                        file.hunks.push(hunk);
                    }
                }
                if let Some(file) = current_file.take() {
                    files.push(file);
                }

                // Parse file path from "diff --git a/old_path b/new_path"
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let path = parts[3].trim_start_matches('b').trim_start_matches('/');
                    current_file = Some(FileDiff::new(path));
                }
            }
            // File mode/permission changes
            else if line.starts_with("new file mode") {
                if let Some(ref mut file) = current_file {
                    file.is_created = true;
                }
            } else if line.starts_with("deleted file mode") {
                if let Some(ref mut file) = current_file {
                    file.is_deleted = true;
                }
            } else if line.starts_with("rename from") {
                if let Some(ref mut file) = current_file {
                    file.is_renamed = true;
                    file.old_path = Some(line[12..].to_string());
                }
            } else if line.starts_with("rename to") {
                if let Some(ref mut file) = current_file {
                    file.path = line[10..].to_string();
                }
            }
            // Binary file
            else if line.starts_with("Binary files") || line.starts_with("GIT binary patch") {
                if let Some(ref mut file) = current_file {
                    file.is_binary = true;
                }
            }
            // Hunk header: "@@ -old_start,old_lines +new_start,new_lines @@"
            else if line.starts_with("@@") && line.contains("@@") {
                // Save previous hunk
                if let Some(hunk) = current_hunk.take() {
                    if let Some(ref mut file) = current_file {
                        file.hunks.push(hunk);
                    }
                }

                // Parse hunk header
                let header_end = line[2..].find("@@").map(|i| i + 2).unwrap_or(line.len()) + 2;
                let _header = line[..header_end].to_string();

                // Extract old and new line info
                let range_part = &line[2..header_end - 2].trim();
                let ranges: Vec<&str> = range_part.split_whitespace().collect();

                let (old_start, old_lines) = if let Some(old_range) = ranges.first() {
                    parse_range(old_range)
                } else {
                    (0, 0)
                };

                let (new_start, new_lines) = if ranges.len() > 1 {
                    parse_range(ranges[1])
                } else {
                    (0, 0)
                };

                current_hunk = Some(DiffHunk {
                    old_start: old_start as u32,
                    old_lines: old_lines as u32,
                    new_start: new_start as u32,
                    new_lines: new_lines as u32,
                    header: line.to_string(),
                    lines: Vec::new(),
                });
            }
            // Diff line content
            else if let Some(ref mut hunk) = current_hunk {
                let (change_type, content) = if let Some(content) = line.strip_prefix('+') {
                    (ChangeType::Added, content.to_string())
                } else if let Some(content) = line.strip_prefix('-') {
                    (ChangeType::Deleted, content.to_string())
                } else if let Some(content) = line.strip_prefix(' ') {
                    (ChangeType::Context, content.to_string())
                } else if line.starts_with("\\") {
                    // "\ No newline at end of file" - skip
                    continue;
                } else {
                    (ChangeType::Message, line.to_string())
                };

                if change_type == ChangeType::Added {
                    total_additions += 1;
                    if let Some(ref mut file) = current_file {
                        file.additions += 1;
                    }
                } else if change_type == ChangeType::Deleted {
                    total_deletions += 1;
                    if let Some(ref mut file) = current_file {
                        file.deletions += 1;
                    }
                }

                hunk.lines.push(DiffLine {
                    change_type,
                    content,
                    old_line_no: None,
                    new_line_no: None,
                });
            }
        }

        // Save last hunk and file
        if let Some(hunk) = current_hunk {
            if let Some(ref mut file) = current_file {
                file.hunks.push(hunk);
            }
        }
        if let Some(file) = current_file {
            files.push(file);
        }

        let files_changed = files.len();
        Ok(Diff {
            files,
            files_changed,
            total_additions,
            total_deletions,
        })
    }

    #[instrument(skip(self))]
    async fn checkout(&self, repo_path: &Path, branch: &str) -> Result<()> {
        self.run_git(repo_path, &["checkout", branch]).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn create_branch(&self, repo_path: &Path, name: &str) -> Result<()> {
        self.run_git(repo_path, &["checkout", "-b", name]).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn delete_branch(&self, repo_path: &Path, name: &str) -> Result<()> {
        self.run_git(repo_path, &["branch", "-d", name]).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn push(&self, repo_path: &Path, remote: &str, branch: &str) -> Result<()> {
        self.run_git(repo_path, &["push", remote, branch]).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn pull(&self, repo_path: &Path, remote: &str, branch: &str) -> Result<()> {
        self.run_git(repo_path, &["pull", remote, branch]).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn stash_list(&self, repo_path: &Path) -> Result<Vec<crate::core::models::Stash>> {
        let output = self
            .run_git(repo_path, &["stash", "list", "--format=%H|%gd|%gs"])
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut stashes = Vec::new();

        for (index, line) in stdout.lines().enumerate() {
            let parts: Vec<&str> = line.splitn(3, '|').collect();
            if parts.len() >= 3 {
                stashes.push(crate::core::models::Stash {
                    index,
                    message: parts[2].to_string(),
                    commit_hash: parts[0].to_string(),
                    branch: None,
                    date: None,
                });
            } else if !line.is_empty() {
                stashes.push(crate::core::models::Stash {
                    index,
                    message: line.to_string(),
                    commit_hash: String::new(),
                    branch: None,
                    date: None,
                });
            }
        }

        Ok(stashes)
    }

    #[instrument(skip(self))]
    async fn stash_save(&self, repo_path: &Path, message: Option<&str>) -> Result<()> {
        let mut args = vec!["stash", "push"];
        if let Some(msg) = message {
            args.push("-m");
            args.push(msg);
        }
        self.run_git(repo_path, &args).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn stash_pop(&self, repo_path: &Path, index: usize) -> Result<()> {
        let stash_ref = format!("stash@{{{}}}", index);
        self.run_git(repo_path, &["stash", "pop", &stash_ref])
            .await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn stash_drop(&self, repo_path: &Path, index: usize) -> Result<()> {
        let stash_ref = format!("stash@{{{}}}", index);
        self.run_git(repo_path, &["stash", "drop", &stash_ref])
            .await?;
        Ok(())
    }
}

/// Parse a range string like "-1,5" or "+5" into (start, lines)
fn parse_range(range: &str) -> (usize, usize) {
    let range = range.trim_start_matches(&['-', '+'][..]);
    let parts: Vec<&str> = range.split(',').collect();

    let start = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let lines = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

    (start, lines)
}

fn short_hash(hash: &str) -> String {
    hash.chars().take(7).collect()
}

fn parse_branch_status(line: &str) -> (String, usize, usize) {
    let name = line
        .split_once("...")
        .map(|(name, _)| name)
        .or_else(|| line.split_once(' ').map(|(name, _)| name))
        .unwrap_or(line)
        .to_string();

    (
        name,
        parse_tracking_count(line, "ahead"),
        parse_tracking_count(line, "behind"),
    )
}

fn parse_tracking_count(line: &str, marker: &str) -> usize {
    let bracket_marker = format!("[{} ", marker);
    let comma_marker = format!(", {} ", marker);

    [&bracket_marker, &comma_marker]
        .iter()
        .find_map(|prefix| {
            line.find(prefix.as_str())
                .and_then(|start| parse_digits(&line[start + prefix.len()..]))
        })
        .unwrap_or(0)
}

fn parse_digits(value: &str) -> Option<usize> {
    value
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_output() {
        let service = CliGitService::new();
        let output = "## main...origin/main\n M src/main.rs\nM  src/lib.rs\n?? new_file.txt";
        let files = service.parse_status_output(output);

        assert_eq!(files.len(), 3);
        assert!(matches!(files[0].status, FileStatus::Modified));
        assert!(matches!(files[1].status, FileStatus::Staged));
        assert!(matches!(files[2].status, FileStatus::Untracked));
    }

    #[test]
    fn test_parse_status_output_ignores_malformed_lines() {
        let service = CliGitService::new();
        let output = "漢\nMMsrc/no-separator.rs\n M src/main.rs";
        let files = service.parse_status_output(output);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/main.rs");
        assert!(matches!(files[0].status, FileStatus::Modified));
    }

    #[test]
    fn test_parse_log_output_handles_short_hash() {
        let service = CliGitService::new();
        let output = "\
commit abc
Author: RightClick
Date: 2026-05-27T10:00:00Z
short hash commit
";

        let commits = service.parse_log_output(output);

        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].hash, "abc");
        assert_eq!(commits[0].short_hash, "abc");
        assert_eq!(commits[0].subject, "short hash commit");
    }

    #[test]
    fn test_parse_range() {
        assert_eq!(parse_range("-1,5"), (1, 5));
        assert_eq!(parse_range("+10"), (10, 1));
        assert_eq!(parse_range("0"), (0, 1));
    }

    #[test]
    fn test_parse_branch_status_with_ahead_and_behind() {
        assert_eq!(
            parse_branch_status("main...origin/main [ahead 2, behind 1]"),
            ("main".to_string(), 2, 1)
        );
    }

    #[test]
    fn test_parse_branch_status_with_single_tracking_counts() {
        assert_eq!(
            parse_branch_status("feature...origin/feature [ahead 3]"),
            ("feature".to_string(), 3, 0)
        );
        assert_eq!(
            parse_branch_status("feature...origin/feature [behind 4]"),
            ("feature".to_string(), 0, 4)
        );
    }

    #[test]
    fn test_parse_branch_status_without_upstream() {
        assert_eq!(parse_branch_status("main"), ("main".to_string(), 0, 0));
        assert_eq!(
            parse_branch_status("feature [gone]"),
            ("feature".to_string(), 0, 0)
        );
    }

    #[test]
    fn test_parse_branch_status_ignores_tracking_words_in_branch_name() {
        assert_eq!(
            parse_branch_status("ahead-fix...origin/ahead-fix [ahead 2]"),
            ("ahead-fix".to_string(), 2, 0)
        );
    }
}
