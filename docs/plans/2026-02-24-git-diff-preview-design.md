# Git Diff Auto-Preview Design

## Problem

The git status plugin does not auto-load diffs when navigating files. Users must press `d` explicitly. Additionally, `diff()` only uses `--cached`, so unstaged and untracked files show no diff.

## Design

### 1. Git Service: `diff_file()` method

Add `diff_file(repo_path, file, status) -> Result<Diff>` to `GitService` trait and `CliGitService`:

- `Staged` -> `git diff --cached -- <path>`
- `Modified`/`Deleted` -> `git diff -- <path>`
- `Untracked` -> `git diff --no-index /dev/null <path>` (handle non-zero exit)

### 2. Plugin: Auto-load on selection change

Add `pending_diff_path: Option<(PathBuf, FileStatus)>` to `GitStatusPlugin`.

- In `SelectIndex` handler (Status/Diff mode): set `pending_diff_path` from selected file
- In `update()`: if `pending_diff_path` is set, call `diff_file()` and update `state.diff`
- On `init()` and `refresh()`: if files exist and one is selected, queue its diff

### 3. Rendering

No major changes. `render_diff_view()` already displays `state.diff` when `Some`. Untracked file diffs appear as all-green added lines naturally from the diff output.

## Files Changed

- `src/shell/services_full/git.rs` - Add `diff_file()` method
- `src/plugins/gitstatus/plugin.rs` - Add `pending_diff_path`, auto-load logic
