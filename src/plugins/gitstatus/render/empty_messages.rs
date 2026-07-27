//! Empty-state message builders for the git status plugin.
use crate::core::models::FileChange;
use crate::ui::{global_hint_message, truncate_display};

use super::super::state::PluginState;

pub(super) fn git_changes_empty_message(state: &PluginState, width: u16) -> String {
    if state.branch.is_empty() {
        git_empty_message(
            vec![
                "Git status not loaded yet".to_string(),
                String::new(),
                "r: Refresh git status".to_string(),
            ],
            width,
        )
    } else {
        git_empty_message(
            vec![
                "Working tree clean".to_string(),
                String::new(),
                "B: Branches".to_string(),
                "H: History".to_string(),
                "r: Refresh git status".to_string(),
            ],
            width,
        )
    }
}

pub(super) fn git_diff_empty_message(state: &PluginState, width: u16) -> String {
    if state.branch.is_empty() {
        git_empty_message(
            vec![
                "Git status not loaded yet".to_string(),
                String::new(),
                "r: Refresh git status".to_string(),
            ],
            width,
        )
    } else if state.files.is_empty() {
        git_empty_message(
            vec![
                "Working tree clean".to_string(),
                String::new(),
                "H: History".to_string(),
                "B: Branches".to_string(),
                "r: Refresh git status".to_string(),
            ],
            width,
        )
    } else {
        git_empty_message(
            vec![
                "No file selected".to_string(),
                String::new(),
                "j/k: Navigate files".to_string(),
                "Tab/Shift+Tab: Switch pane".to_string(),
                "S: Status".to_string(),
                "H: History".to_string(),
                "B: Branches".to_string(),
                "r: Refresh git status".to_string(),
            ],
            width,
        )
    }
}

pub(super) fn git_file_no_diff_message(file: &FileChange, width: u16) -> String {
    git_empty_message(
        vec![
            format!(
                "Diff not loaded yet for {}",
                truncate_display(&file.path, 48)
            ),
            String::new(),
            "j/k: Navigate files".to_string(),
            "s: Stage".to_string(),
            "u: Unstage".to_string(),
            "c: Commit".to_string(),
            "r: Refresh git status".to_string(),
        ],
        width,
    )
}

pub(super) fn git_sidebar_empty_message(state: &PluginState, width: u16) -> String {
    if state.branch.is_empty() {
        git_empty_message(
            vec![
                "Git status not loaded yet".to_string(),
                String::new(),
                "r: Refresh git status".to_string(),
            ],
            width,
        )
    } else {
        git_empty_message(
            vec![
                "Working tree clean".to_string(),
                String::new(),
                "B: Branches".to_string(),
                "H: History".to_string(),
                "r: Refresh git status".to_string(),
            ],
            width,
        )
    }
}

pub(super) fn git_commits_empty_message(width: u16) -> String {
    git_empty_message(
        vec![
            "No commits".to_string(),
            String::new(),
            "S: Status".to_string(),
            "B: Branches".to_string(),
            "r: Refresh git status".to_string(),
        ],
        width,
    )
}

pub(super) fn git_commit_details_empty_message(state: &PluginState, width: u16) -> String {
    if state.commits.is_empty() {
        git_commits_empty_message(width)
    } else {
        git_empty_message(
            vec![
                "No commit selected".to_string(),
                String::new(),
                "j/k: Navigate commits".to_string(),
                "Tab/Shift+Tab: Switch pane".to_string(),
                "S: Status".to_string(),
                "B: Branches".to_string(),
                "r: Refresh git status".to_string(),
            ],
            width,
        )
    }
}

pub(super) fn git_branches_empty_message(width: u16) -> String {
    git_empty_message(
        vec![
            "No branches".to_string(),
            String::new(),
            "S: Status".to_string(),
            "H: History".to_string(),
            "r: Refresh git status".to_string(),
        ],
        width,
    )
}

pub(super) fn git_branch_details_empty_message(state: &PluginState, width: u16) -> String {
    if state.branches.is_empty() {
        git_branches_empty_message(width)
    } else {
        git_empty_message(
            vec![
                "No branch selected".to_string(),
                String::new(),
                "j/k: Navigate branches".to_string(),
                "Tab/Shift+Tab: Switch pane".to_string(),
                "n: New branch".to_string(),
                "S: Status".to_string(),
                "H: History".to_string(),
                "r: Refresh git status".to_string(),
            ],
            width,
        )
    }
}

pub(super) fn git_stashes_empty_message(width: u16) -> String {
    git_empty_message(
        vec![
            "No stashes".to_string(),
            String::new(),
            "s: Save stash".to_string(),
            "S: Status".to_string(),
            "B: Branches".to_string(),
            "r: Refresh git status".to_string(),
        ],
        width,
    )
}

pub(super) fn git_stash_details_empty_message(state: &PluginState, width: u16) -> String {
    if state.stashes.is_empty() {
        git_stashes_empty_message(width)
    } else {
        git_empty_message(
            vec![
                "No stash selected".to_string(),
                String::new(),
                "j/k: Navigate stashes".to_string(),
                "Tab/Shift+Tab: Switch pane".to_string(),
                "s: Save stash".to_string(),
                "S: Status".to_string(),
                "B: Branches".to_string(),
                "r: Refresh git status".to_string(),
            ],
            width,
        )
    }
}

fn git_empty_message(lines: Vec<String>, width: u16) -> String {
    global_hint_message(lines, width)
}
