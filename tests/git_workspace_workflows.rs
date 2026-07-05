use std::path::{Path, PathBuf};
use std::process::Command;

use rightclick::core::models::FileStatus;
use rightclick::plugins::workspace::WorktreeManager;
use rightclick::shell::services_full::{CliGitService, GitService};

fn run_git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("git command should run");

    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn create_repo() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let repo = temp.path().join("repo");
    std::fs::create_dir(&repo).expect("repo dir");

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "rightclick@example.com"]);
    run_git(&repo, &["config", "user.name", "RightClick Test"]);
    // Neutralize host-level signing settings so commits succeed without gpg.
    run_git(&repo, &["config", "commit.gpgsign", "false"]);
    run_git(&repo, &["config", "tag.gpgsign", "false"]);

    std::fs::write(repo.join("README.md"), "initial\n").expect("seed file");
    run_git(&repo, &["add", "README.md"]);
    run_git(&repo, &["commit", "-m", "initial"]);

    (temp, repo)
}

#[tokio::test]
async fn git_service_reports_modified_and_untracked_diffs() {
    let (_temp, repo) = create_repo();
    let service = CliGitService::new();

    std::fs::write(repo.join("README.md"), "initial\nchanged\n").expect("modify file");
    std::fs::write(repo.join("new.txt"), "new file\n").expect("new file");

    let status = service.status(&repo).await.expect("repo status");
    assert!(
        status
            .unstaged
            .iter()
            .any(|file| file.path == "README.md" && file.status == FileStatus::Modified)
    );
    assert!(
        status
            .untracked
            .iter()
            .any(|file| file.path == "new.txt" && file.status == FileStatus::Untracked)
    );

    let modified = service
        .diff_file(&repo, Path::new("README.md"), FileStatus::Modified)
        .await
        .expect("modified diff");
    assert_eq!(modified.files_changed, 1);
    assert!(modified.total_additions >= 1);

    let untracked = service
        .diff_file(&repo, Path::new("new.txt"), FileStatus::Untracked)
        .await
        .expect("untracked diff");
    assert_eq!(untracked.files_changed, 1);
    assert!(untracked.total_additions >= 1);
}

#[tokio::test]
async fn worktree_manager_creates_lists_and_deletes_worktrees() {
    let (temp, repo) = create_repo();
    let manager = WorktreeManager::new(repo.clone());

    let worktree_path = manager
        .create_worktree("feature-a", Some("feature/a"), None)
        .await
        .expect("create worktree");
    assert_eq!(worktree_path, temp.path().join("feature-a"));
    assert!(worktree_path.exists());
    let canonical_worktree_path = worktree_path
        .canonicalize()
        .expect("canonical worktree path");

    let worktrees = manager.list_worktrees().await.expect("list worktrees");
    let created = worktrees
        .iter()
        .find(|worktree| {
            worktree.path.canonicalize().ok().as_deref() == Some(canonical_worktree_path.as_path())
        })
        .expect("created worktree should be listed");
    assert_eq!(created.name, "feature-a");
    assert_eq!(created.branch, "feature/a");

    manager
        .delete_worktree(&worktree_path, false)
        .await
        .expect("delete worktree");

    let worktrees = manager.list_worktrees().await.expect("list after delete");
    assert!(!worktrees.iter().any(|worktree| {
        worktree.path.canonicalize().ok().as_deref() == Some(canonical_worktree_path.as_path())
    }));
}
