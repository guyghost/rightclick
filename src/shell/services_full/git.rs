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
}

/// Git service implementation using command-line git
#[derive(Debug, Clone, Default)]
pub struct CliGitService;

impl CliGitService {
    /// Create a new CLI git service
    pub fn new() -> Self {
        Self
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
            if line.len() < 3 {
                continue;
            }

            let status_code = &line[..2];
            let path = line[3..].to_string();

            let status = match status_code {
                " M" => FileStatus::Modified,
                "M " => FileStatus::Staged,
                "MM" => FileStatus::Modified, // Staged and modified
                "A " => FileStatus::Staged,   // Added
                "AM" => FileStatus::Staged,
                "D " => FileStatus::Deleted,
                " D" => FileStatus::Deleted,
                "R " => FileStatus::Renamed,
                "RM" => FileStatus::Renamed,
                "C " => FileStatus::TypeChanged,
                "??" => FileStatus::Untracked,
                "!!" => FileStatus::Ignored,
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
            if line.starts_with("commit ") {
                // Save previous commit if exists
                if !current_hash.is_empty() {
                    if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&current_date) {
                        commits.push(Commit {
                            hash: current_hash.clone(),
                            short_hash: current_hash[..7].to_string(),
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
                current_hash = line[7..].to_string();
            } else if line.starts_with("Author: ") {
                current_author = line[8..].to_string();
            } else if line.starts_with("Date: ") {
                current_date = line[6..].to_string();
            } else if !line.starts_with(' ') && !line.is_empty() {
                current_subject = line.to_string();
            }
        }

        // Add last commit
        if !current_hash.is_empty() {
            if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&current_date) {
                commits.push(Commit {
                    hash: current_hash.clone(),
                    short_hash: current_hash[..7].to_string(),
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
                    short_hash: hash.chars().take(7).collect(),
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

        // Parse branch info from first line
        let branch = stdout.lines().next().and_then(|line| {
            if line.starts_with("## ") {
                Some(line[3..].to_string())
            } else {
                None
            }
        });

        let (branch_name, ahead, behind) = if let Some(b) = branch {
            // Parse branch name and ahead/behind
            let parts: Vec<&str> = b.split("...").collect();
            let name = parts[0].to_string();

            let ahead = b
                .find("[ahead ")
                .and_then(|i| b[i + 7..].split(']').next())
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);

            let behind = b
                .find("[behind ")
                .and_then(|i| b[i + 8..].split(']').next())
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);

            (name, ahead, behind)
        } else {
            ("main".to_string(), 0, 0)
        };

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

        // Parse diff output into hunks
        let mut hunks = Vec::new();
        let mut current_hunk: Option<DiffHunk> = None;
        let mut lines = Vec::new();

        for line in stdout.lines() {
            if line.starts_with("@@") {
                // Save previous hunk
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

                // Parse hunk header: @@ -old_start,old_lines +new_start,new_lines @@
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
                    header: header.clone(),
                    lines: Vec::new(),
                });
            } else if !line.starts_with("---")
                && !line.starts_with("+++")
                && !line.starts_with("diff")
            {
                let diff_line = if line.starts_with('+') {
                    DiffLine {
                        content: line[1..].to_string(),
                        change_type: crate::core::models::ChangeType::Added,
                        old_line_no: None,
                        new_line_no: None,
                    }
                } else if line.starts_with('-') {
                    DiffLine {
                        content: line[1..].to_string(),
                        change_type: crate::core::models::ChangeType::Deleted,
                        old_line_no: None,
                        new_line_no: None,
                    }
                } else if line.starts_with(' ') {
                    DiffLine {
                        content: line[1..].to_string(),
                        change_type: crate::core::models::ChangeType::Context,
                        old_line_no: None,
                        new_line_no: None,
                    }
                } else {
                    DiffLine {
                        content: line.to_string(),
                        change_type: crate::core::models::ChangeType::Context,
                        old_line_no: None,
                        new_line_no: None,
                    }
                };
                lines.push(diff_line);
            }
        }

        // Add last hunk
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
            additions: 0,
            deletions: 0,
            is_created: false,
            is_deleted: false,
            is_renamed: false,
        };

        Ok(Diff {
            files: vec![file_diff],
            files_changed: 1,
            total_additions: 0,
            total_deletions: 0,
        })
    }

    #[instrument(skip(self))]
    async fn commits(&self, repo_path: &Path, limit: usize) -> Result<Vec<Commit>> {
        debug!("Getting {} commits for {}", limit, repo_path.display());

        // Use a more robust format with clear separators
        let format = "--format=%H|%an|%aI|%s";
        let output = self
            .run_git(repo_path, &["log", &format, &format!("-n{}", limit)])
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
                let (change_type, content) = if line.starts_with('+') {
                    (ChangeType::Added, line[1..].to_string())
                } else if line.starts_with('-') {
                    (ChangeType::Deleted, line[1..].to_string())
                } else if line.starts_with(' ') {
                    (ChangeType::Context, line[1..].to_string())
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
}

/// Parse a range string like "-1,5" or "+5" into (start, lines)
fn parse_range(range: &str) -> (usize, usize) {
    let range = range.trim_start_matches(&['-', '+'][..]);
    let parts: Vec<&str> = range.split(',').collect();

    let start = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let lines = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

    (start, lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_output() {
        let service = CliGitService::new();
        let output = " M src/main.rs\nM  src/lib.rs\n?? new_file.txt";
        let files = service.parse_status_output(output);

        assert_eq!(files.len(), 3);
        assert!(matches!(files[0].status, FileStatus::Modified));
        assert!(matches!(files[1].status, FileStatus::Staged));
        assert!(matches!(files[2].status, FileStatus::Untracked));
    }

    #[test]
    fn test_parse_range() {
        assert_eq!(parse_range("-1,5"), (1, 5));
        assert_eq!(parse_range("+10"), (10, 1));
        assert_eq!(parse_range("0"), (0, 1));
    }
}
