//! External service integrations - STUB

use crate::core::models::{Commit, Diff, FileDiff, RepoStatus};
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

#[async_trait]
pub trait GitService: Send + Sync {
    async fn status(&self, repo_path: &Path) -> Result<RepoStatus>;
    async fn diff(&self, repo_path: &Path, file: &Path) -> Result<Diff>;
    async fn commits(&self, repo_path: &Path, limit: usize) -> Result<Vec<Commit>>;
    async fn stage(&self, repo_path: &Path, file: &Path) -> Result<()>;
    async fn unstage(&self, repo_path: &Path, file: &Path) -> Result<()>;
    async fn commit(&self, repo_path: &Path, message: &str) -> Result<()>;
    async fn commit_details(&self, repo_path: &Path, commit_hash: &str) -> Result<Vec<FileDiff>>;
    /// Get the full diff for a specific commit
    async fn commit_diff(&self, repo_path: &Path, commit_hash: &str) -> Result<Diff>;
}

#[derive(Debug, Clone, Default)]
pub struct CliGitService;

impl CliGitService {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl GitService for CliGitService {
    async fn status(&self, _repo_path: &Path) -> Result<RepoStatus> {
        Ok(RepoStatus::default())
    }
    async fn diff(&self, _repo_path: &Path, _file: &Path) -> Result<Diff> {
        Ok(Diff::default())
    }
    async fn commits(&self, _repo_path: &Path, _limit: usize) -> Result<Vec<Commit>> {
        Ok(Vec::new())
    }
    async fn stage(&self, _repo_path: &Path, _file: &Path) -> Result<()> {
        Ok(())
    }
    async fn unstage(&self, _repo_path: &Path, _file: &Path) -> Result<()> {
        Ok(())
    }
    async fn commit(&self, _repo_path: &Path, _message: &str) -> Result<()> {
        Ok(())
    }
    async fn commit_details(&self, _repo_path: &Path, _commit_hash: &str) -> Result<Vec<FileDiff>> {
        Ok(Vec::new())
    }
    async fn commit_diff(&self, _repo_path: &Path, _commit_hash: &str) -> Result<Diff> {
        Ok(Diff::default())
    }
}
