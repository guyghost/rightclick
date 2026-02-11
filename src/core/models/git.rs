//! Git types for RightClick
//!
//! This module defines data structures for representing Git repository state,
//! file changes, commits, diffs, and related operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Status of a file in the working directory
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    /// File is not tracked by git
    Untracked,
    /// File has been modified but not staged
    Modified,
    /// File has been staged for commit
    Staged,
    /// File has been renamed
    Renamed,
    /// File has been deleted
    Deleted,
    /// File is ignored by git
    Ignored,
    /// File is unchanged/clean
    Clean,
    /// File has conflicts that need resolution
    Conflicted,
    /// File type has changed (e.g., file to symlink)
    TypeChanged,
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileStatus::Untracked => write!(f, "Untracked"),
            FileStatus::Modified => write!(f, "Modified"),
            FileStatus::Staged => write!(f, "Staged"),
            FileStatus::Renamed => write!(f, "Renamed"),
            FileStatus::Deleted => write!(f, "Deleted"),
            FileStatus::Ignored => write!(f, "Ignored"),
            FileStatus::Clean => write!(f, "Clean"),
            FileStatus::Conflicted => write!(f, "Conflicted"),
            FileStatus::TypeChanged => write!(f, "TypeChanged"),
        }
    }
}

/// A single file change with its status
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileChange {
    /// Path to the file (relative to repo root)
    pub path: String,
    /// Current status of the file
    pub status: FileStatus,
    /// Previous path (for renamed files)
    pub old_path: Option<String>,
    /// Number of additions (if known)
    pub additions: Option<u32>,
    /// Number of deletions (if known)
    pub deletions: Option<u32>,
}

impl FileChange {
    /// Create a new file change with the given path and status
    pub fn new(path: impl Into<String>, status: FileStatus) -> Self {
        Self {
            path: path.into(),
            status,
            old_path: None,
            additions: None,
            deletions: None,
        }
    }

    /// Create a new renamed file change
    pub fn renamed(old_path: impl Into<String>, new_path: impl Into<String>) -> Self {
        Self {
            path: new_path.into(),
            status: FileStatus::Renamed,
            old_path: Some(old_path.into()),
            additions: None,
            deletions: None,
        }
    }

    /// Check if this file has staged changes
    pub fn is_staged(&self) -> bool {
        matches!(self.status, FileStatus::Staged | FileStatus::Renamed)
    }

    /// Check if this file has unstaged changes
    pub fn is_unstaged(&self) -> bool {
        matches!(
            self.status,
            FileStatus::Modified | FileStatus::Deleted | FileStatus::Untracked
        )
    }
}

/// A git commit
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Commit {
    /// Full commit hash (40 characters for SHA-1)
    pub hash: String,
    /// Short commit hash (usually 7-12 characters)
    pub short_hash: String,
    /// Commit subject (first line of message)
    pub subject: String,
    /// Full commit message
    pub message: Option<String>,
    /// Author name
    pub author: String,
    /// Author email
    pub author_email: Option<String>,
    /// Commit timestamp
    pub date: DateTime<Utc>,
    /// Parent commit hashes
    pub parents: Vec<String>,
    /// Number of files changed (if known)
    pub files_changed: Option<usize>,
    /// Number of insertions (if known)
    pub insertions: Option<usize>,
    /// Number of deletions (if known)
    pub deletions: Option<usize>,
}

impl Commit {
    /// Create a new commit with required fields
    pub fn new(
        hash: impl Into<String>,
        subject: impl Into<String>,
        author: impl Into<String>,
        date: DateTime<Utc>,
    ) -> Self {
        let hash = hash.into();
        let short_hash = hash.chars().take(7).collect();
        Self {
            hash,
            short_hash,
            subject: subject.into(),
            message: None,
            author: author.into(),
            author_email: None,
            date,
            parents: Vec::new(),
            files_changed: None,
            insertions: None,
            deletions: None,
        }
    }
}

/// A hunk (contiguous section) of a diff
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiffHunk {
    /// Old starting line number
    pub old_start: u32,
    /// Number of lines in old file
    pub old_lines: u32,
    /// New starting line number
    pub new_start: u32,
    /// Number of lines in new file
    pub new_lines: u32,
    /// Header line for the hunk (e.g., "@@ -1,5 +1,5 @@")
    pub header: String,
    /// Individual lines in the hunk
    pub lines: Vec<DiffLine>,
}

/// A single line in a diff
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiffLine {
    /// Type of change for this line
    pub change_type: ChangeType,
    /// The content of the line (without the +/- prefix)
    pub content: String,
    /// Old line number (if applicable)
    pub old_line_no: Option<u32>,
    /// New line number (if applicable)
    pub new_line_no: Option<u32>,
}

/// Type of change for a diff line
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    /// Line was added
    Added,
    /// Line was deleted
    Deleted,
    /// Line is unchanged (context)
    Context,
    /// Line contains a message (not actual content)
    Message,
}

/// A diff for a single file
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileDiff {
    /// Path to the file
    pub path: String,
    /// Old path (for renamed files)
    pub old_path: Option<String>,
    /// Whether the file is binary
    pub is_binary: bool,
    /// Old file mode (e.g., "100644")
    pub old_mode: Option<String>,
    /// New file mode
    pub new_mode: Option<String>,
    /// Old file blob hash
    pub old_hash: Option<String>,
    /// New file blob hash
    pub new_hash: Option<String>,
    /// Hunks in this diff
    pub hunks: Vec<DiffHunk>,
    /// Total additions in this file
    pub additions: usize,
    /// Total deletions in this file
    pub deletions: usize,
    /// Whether the file was created
    pub is_created: bool,
    /// Whether the file was deleted
    pub is_deleted: bool,
    /// Whether the file was renamed
    pub is_renamed: bool,
}

impl FileDiff {
    /// Create a new empty file diff
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            old_path: None,
            is_binary: false,
            old_mode: None,
            new_mode: None,
            old_hash: None,
            new_hash: None,
            hunks: Vec::new(),
            additions: 0,
            deletions: 0,
            is_created: false,
            is_deleted: false,
            is_renamed: false,
        }
    }

    /// Calculate total line changes
    pub fn total_changes(&self) -> usize {
        self.additions + self.deletions
    }
}

/// A complete diff between two commits or trees
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Diff {
    /// Individual file diffs
    pub files: Vec<FileDiff>,
    /// Total number of files changed
    pub files_changed: usize,
    /// Total insertions across all files
    pub total_additions: usize,
    /// Total deletions across all files
    pub total_deletions: usize,
}

impl Default for Diff {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            files_changed: 0,
            total_additions: 0,
            total_deletions: 0,
        }
    }
}

/// Branch information
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    /// Branch name
    pub name: String,
    /// Full reference name (e.g., "refs/heads/main")
    pub full_name: String,
    /// Whether this is the current branch
    pub is_current: bool,
    /// Whether this is a remote tracking branch
    pub is_remote: bool,
    /// Remote name (for remote branches)
    pub remote: Option<String>,
    /// Commit hash this branch points to
    pub commit_hash: String,
    /// Upstream branch name (if tracking)
    pub upstream: Option<String>,
    /// Number of commits ahead of upstream
    pub ahead: Option<usize>,
    /// Number of commits behind upstream
    pub behind: Option<usize>,
}

/// Remote repository information
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Remote {
    /// Remote name (e.g., "origin")
    pub name: String,
    /// Fetch URL
    pub fetch_url: String,
    /// Push URL (may differ from fetch URL)
    pub push_url: Option<String>,
    /// Default branch on this remote
    pub default_branch: Option<String>,
}

/// Repository status summary
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RepoStatus {
    /// Current branch name
    pub branch: String,
    /// Short commit hash of HEAD
    pub head: String,
    /// Whether there are uncommitted changes
    pub is_dirty: bool,
    /// Staged file changes
    pub staged: Vec<FileChange>,
    /// Unstaged file changes
    pub unstaged: Vec<FileChange>,
    /// Untracked files
    pub untracked: Vec<FileChange>,
    /// Conflicted files
    pub conflicted: Vec<FileChange>,
    /// Number of commits ahead of upstream
    pub ahead: usize,
    /// Number of commits behind upstream
    pub behind: usize,
    /// Current operation in progress (e.g., rebase, merge)
    pub state: Option<RepoState>,
}

impl Default for RepoStatus {
    fn default() -> Self {
        Self {
            branch: "main".to_string(),
            head: String::new(),
            is_dirty: false,
            staged: Vec::new(),
            unstaged: Vec::new(),
            untracked: Vec::new(),
            conflicted: Vec::new(),
            ahead: 0,
            behind: 0,
            state: None,
        }
    }
}

/// Repository state during ongoing operations
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoState {
    /// Normal state, no operation in progress
    Clean,
    /// Merge in progress
    Merging,
    /// Rebase in progress
    Rebasing,
    /// Revert in progress
    Reverting,
    /// Cherry-pick in progress
    CherryPicking,
    /// Bisect in progress
    Bisecting,
    /// Interactive rebase
    InteractiveRebase,
}

/// Stash entry
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stash {
    /// Stash index (0 = most recent)
    pub index: usize,
    /// Stash message
    pub message: String,
    /// Commit hash of the stash
    pub commit_hash: String,
    /// Timestamp when stash was created
    pub date: Option<DateTime<Utc>>,
    /// Branch that was current when stash was created
    pub branch: Option<String>,
}

/// Blame information for a single line
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlameLine {
    /// Original commit hash
    pub commit_hash: String,
    /// Author name
    pub author: String,
    /// Author email
    pub author_email: Option<String>,
    /// Commit date
    pub date: DateTime<Utc>,
    /// Line number in original file
    pub original_line_no: u32,
    /// Line number in current file
    pub line_no: u32,
    /// Line content
    pub content: String,
    /// Whether this line was changed in this commit
    pub is_boundary: bool,
}

/// Complete blame information for a file
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blame {
    /// File path
    pub path: String,
    /// Commit hash being blamed
    pub commit_hash: String,
    /// Individual line blames
    pub lines: Vec<BlameLine>,
}

/// Errors that can occur when working with git operations
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    /// Repository not found
    #[error("repository not found: {0}")]
    RepoNotFound(String),
    /// Invalid reference
    #[error("invalid reference: {0}")]
    InvalidRef(String),
    /// Not a git repository
    #[error("not a git repository: {0}")]
    NotARepo(String),
    /// File not found in repository
    #[error("file not found: {0}")]
    FileNotFound(String),
    /// Failed to parse git output
    #[error("parse error: {0}")]
    ParseError(String),
    /// Merge conflict
    #[error("merge conflict: {0}")]
    MergeConflict(String),
    /// Operation not allowed in current state
    #[error("invalid operation: {0}")]
    InvalidOperation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_status_display() {
        assert_eq!(FileStatus::Modified.to_string(), "Modified");
        assert_eq!(FileStatus::Staged.to_string(), "Staged");
        assert_eq!(FileStatus::Untracked.to_string(), "Untracked");
    }

    #[test]
    fn test_file_change_new() {
        let change = FileChange::new("src/main.rs", FileStatus::Modified);
        assert_eq!(change.path, "src/main.rs");
        assert_eq!(change.status, FileStatus::Modified);
        assert!(change.old_path.is_none());
    }

    #[test]
    fn test_file_change_renamed() {
        let change = FileChange::renamed("old.rs", "new.rs");
        assert_eq!(change.path, "new.rs");
        assert_eq!(change.status, FileStatus::Renamed);
        assert_eq!(change.old_path, Some("old.rs".to_string()));
    }

    #[test]
    fn test_file_change_staged_check() {
        let staged = FileChange::new("file.rs", FileStatus::Staged);
        let unstaged = FileChange::new("file.rs", FileStatus::Modified);
        assert!(staged.is_staged());
        assert!(!staged.is_unstaged());
        assert!(!unstaged.is_staged());
        assert!(unstaged.is_unstaged());
    }

    #[test]
    fn test_commit_new() {
        let date = Utc::now();
        let commit = Commit::new("abc123", "Initial commit", "John Doe", date);
        assert_eq!(commit.hash, "abc123");
        assert_eq!(commit.short_hash, "abc123");
        assert_eq!(commit.subject, "Initial commit");
        assert_eq!(commit.author, "John Doe");
        assert_eq!(commit.date, date);
    }

    #[test]
    fn test_file_diff_new() {
        let diff = FileDiff::new("src/main.rs");
        assert_eq!(diff.path, "src/main.rs");
        assert!(diff.hunks.is_empty());
        assert_eq!(diff.total_changes(), 0);
    }

    #[test]
    fn test_repo_status_default() {
        let status = RepoStatus::default();
        assert_eq!(status.branch, "main");
        assert!(!status.is_dirty);
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
    }
}
