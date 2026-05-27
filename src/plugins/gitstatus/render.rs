//! Git Status Plugin Rendering
//!
//! This module handles all rendering logic for the Git Status plugin,
//! including the sidebar, diff view, and history view.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::core::models::Theme;
use crate::core::models::{ChangeType, Diff, FileChange, FileDiff, FileStatus};
use crate::theme::{UiElement, style_for_git_status, style_for_ui_element};

use super::state::{FocusPane, PluginState, ViewMode};

const GIT_DELETE_BRANCH_MODAL_HINT: &str = "Enter/D: Delete  |  Esc: Cancel";
const GIT_DROP_STASH_MODAL_HINT: &str = "Enter/D: Drop  |  Esc: Cancel";
const GIT_CANCEL_MODAL_HINT: &str = "Esc: Cancel";
const GIT_ERROR_MODAL_HINT: &str = "Esc: Close";
const GIT_MODAL_WIDTH: u16 = 50;
const GIT_MODAL_HEIGHT: u16 = 7;
const MIN_GIT_MODAL_WIDTH: u16 = 20;
const MIN_GIT_MODAL_HEIGHT: u16 = 5;

/// Render the git status plugin
pub fn render_git_status(
    state: &PluginState,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    focused: bool,
) {
    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(state.sidebar_width), Constraint::Min(20)])
        .split(area);

    let sidebar_area = main_layout[0];
    let main_area = main_layout[1];

    // Render sidebar based on view mode
    match state.view_mode {
        ViewMode::Status | ViewMode::Diff => {
            render_sidebar(state, state.focus_pane, focused, sidebar_area, buf, theme);
        }
        ViewMode::History => {
            render_commit_list(state, state.focus_pane, focused, sidebar_area, buf, theme);
        }
        ViewMode::Branches => {
            render_branch_list(state, state.focus_pane, focused, sidebar_area, buf, theme);
        }
        ViewMode::Stash => {
            render_stash_list(state, state.focus_pane, focused, sidebar_area, buf, theme);
        }
    }

    // Render main content based on view mode
    match state.view_mode {
        ViewMode::Status | ViewMode::Diff => {
            render_diff_view(state, state.focus_pane, focused, main_area, buf, theme);
        }
        ViewMode::History => {
            render_commit_details(state, state.focus_pane, focused, main_area, buf, theme);
        }
        ViewMode::Branches => {
            render_branch_details(state, state.focus_pane, main_area, buf, theme);
        }
        ViewMode::Stash => {
            render_stash_details(state, state.focus_pane, main_area, buf, theme);
        }
    }

    // Render modal overlay if active
    if state.modal_active {
        render_modal_overlay(state, area, buf, theme);
    }
}

/// Render the sidebar with file list
fn render_sidebar(
    state: &PluginState,
    focus_pane: FocusPane,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let is_sidebar_focused = focus_pane == FocusPane::Sidebar;
    let border_style = if is_sidebar_focused && focused {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Border)
    };

    // Build title with branch info
    let title = if state.branch.is_empty() {
        " Files ".to_string()
    } else {
        format!(" {} ", state.branch)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    // Calculate available height
    let available_height = inner.height as usize;
    if available_height == 0 {
        return;
    }

    // Collect all file sections
    let mut lines: Vec<Line> = Vec::new();
    let mut selected_line: Option<usize> = None;

    // Staged section
    let staged = state.staged_files();
    if !staged.is_empty() {
        lines.push(build_section_header("Staged", theme, &theme.colors.added));
        for file in staged.iter() {
            let is_selected = state
                .selected_file
                .and_then(|idx| state.files.get(idx))
                .map(|f| f.path == file.path)
                .unwrap_or(false);
            if is_selected {
                selected_line = Some(lines.len());
            }
            lines.push(build_file_line(file, is_selected, theme));
        }
        lines.push(Line::raw(""));
    }

    // Unstaged section
    let unstaged = state.unstaged_files();
    if !unstaged.is_empty() {
        lines.push(build_section_header(
            "Unstaged",
            theme,
            &theme.colors.modified,
        ));
        for file in unstaged.iter() {
            let is_selected = state
                .selected_file
                .and_then(|idx| state.files.get(idx))
                .map(|f| f.path == file.path)
                .unwrap_or(false);
            if is_selected {
                selected_line = Some(lines.len());
            }
            lines.push(build_file_line(file, is_selected, theme));
        }
        lines.push(Line::raw(""));
    }

    // Untracked section
    let untracked = state.untracked_files();
    if !untracked.is_empty() {
        lines.push(build_section_header(
            "Untracked",
            theme,
            &theme.colors.untracked,
        ));
        for file in untracked.iter() {
            let is_selected = state
                .selected_file
                .and_then(|idx| state.files.get(idx))
                .map(|f| f.path == file.path)
                .unwrap_or(false);
            if is_selected {
                selected_line = Some(lines.len());
            }
            lines.push(build_file_line(file, is_selected, theme));
        }
    }

    if state.files.is_empty() {
        lines.push(Line::raw(""));
        for line in git_changes_empty_message(state).lines() {
            lines.push(Line::styled(
                format!("  {}", line),
                style_for_ui_element(theme, UiElement::Success),
            ));
        }
        lines.push(Line::raw(""));
    }

    // Add clear separation between working tree changes and commits
    if !state.commits.is_empty() {
        // Add spacing and a visual separator line
        lines.push(Line::raw(""));

        // Create a horizontal separator line that fills the width
        let separator_style = style_for_ui_element(theme, UiElement::Border);
        lines.push(Line::styled(
            "─────────────────────────────────────────",
            separator_style,
        ));
        lines.push(Line::raw(""));

        // Section header for commits
        let header_style =
            style_for_ui_element(theme, UiElement::Secondary).add_modifier(Modifier::BOLD);
        lines.push(Line::styled(
            count_title(
                "Recent Commit",
                "Recent Commits",
                state.commits.len(),
                "commit",
                "commits",
            )
            .trim_end()
            .to_string(),
            header_style,
        ));
        lines.push(Line::raw(""));

        for (idx, commit) in state.commits.iter().enumerate() {
            // Fix: In History mode, use selected_commit directly
            // In Status/Diff mode, only show commit selection if no file is selected
            let is_selected = match state.view_mode {
                crate::core::models::state_machine::ViewMode::History => {
                    state.selected_commit == Some(idx)
                }
                _ => {
                    // In Status/Diff mode, show commit selection only when no file is selected
                    state.selected_file.is_none() && state.selected_commit == Some(idx)
                }
            };
            if is_selected {
                selected_line = Some(lines.len());
            }
            lines.push(build_commit_line(
                commit.short_hash.as_str(),
                &commit.subject,
                is_selected,
                theme,
            ));
        }
    }

    if lines.is_empty() {
        let empty_text = Paragraph::new(git_sidebar_empty_message(state))
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::MutedText))
            .wrap(ratatui::widgets::Wrap { trim: true });
        empty_text.render(inner, buf);
        return;
    }

    // Calculate scroll offset to keep selection visible
    let scroll_offset = calculate_scroll_offset(lines.len(), available_height, selected_line);

    // Render visible lines
    let visible_lines: Vec<Line> = lines
        .iter()
        .skip(scroll_offset)
        .take(available_height)
        .cloned()
        .collect();

    let text = Paragraph::new(visible_lines);
    text.render(inner, buf);
}

/// Render the diff view
fn render_diff_view(
    state: &PluginState,
    focus_pane: FocusPane,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    if state.diff.is_none() && state.selected_file().is_none() && state.selected_commit().is_some()
    {
        render_commit_details(state, focus_pane, focused, area, buf, theme);
        return;
    }

    let is_main_focused = focus_pane == FocusPane::Main;
    let border_style = if is_main_focused {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Border)
    };

    // Build title
    let title = if let Some(file) = state.selected_file() {
        format!(" {} ", file.path)
    } else {
        " Diff ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    if let Some(diff) = &state.diff {
        render_diff_content(diff, inner, buf, theme);
    } else if let Some(file) = state.selected_file() {
        let no_diff = Paragraph::new(git_file_no_diff_message(file))
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::MutedText));
        no_diff.render(inner, buf);
    } else {
        let empty = Paragraph::new(git_diff_empty_message(state))
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::MutedText));
        empty.render(inner, buf);
    }
}

/// Render diff content
fn render_diff_content(diff: &Diff, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let mut lines: Vec<Line> = Vec::new();

    for file_diff in &diff.files {
        // File header
        let file_style =
            style_for_ui_element(theme, UiElement::Primary).add_modifier(Modifier::BOLD);
        lines.push(Line::from(vec![
            Span::styled("File: ", file_style),
            Span::styled(
                &file_diff.path,
                style_for_ui_element(theme, UiElement::Text),
            ),
        ]));

        if let Some(status_label) = diff_content_status_label(file_diff) {
            lines.push(Line::from(vec![Span::styled(
                status_label,
                style_for_ui_element(theme, UiElement::MutedText),
            )]));
            continue;
        }

        // Hunks
        for hunk in &file_diff.hunks {
            // Hunk header
            let header_style = style_for_ui_element(theme, UiElement::Secondary);
            lines.push(Line::from(vec![Span::styled(&hunk.header, header_style)]));

            // Lines
            for line in &hunk.lines {
                let (prefix, style) = match line.change_type {
                    ChangeType::Added => ("+", style_for_git_status(theme, "added")),
                    ChangeType::Deleted => ("-", style_for_git_status(theme, "deleted")),
                    ChangeType::Context => (" ", style_for_ui_element(theme, UiElement::Text)),
                    ChangeType::Message => ("", style_for_ui_element(theme, UiElement::MutedText)),
                };

                lines.push(Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(&line.content, style),
                ]));
            }
        }

        lines.push(Line::raw(""));
    }

    // Render with scroll handling
    let text = Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false });
    text.render(area, buf);
}

fn diff_content_status_label(file_diff: &FileDiff) -> Option<&'static str> {
    if file_diff.is_binary {
        Some("Binary file")
    } else if file_diff.hunks.is_empty() {
        Some("No diff content available")
    } else {
        None
    }
}

/// Render the commit list sidebar
fn render_commit_list(
    state: &PluginState,
    focus_pane: FocusPane,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let is_sidebar_focused = focus_pane == FocusPane::Sidebar;
    let border_style = if is_sidebar_focused && focused {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Border)
    };

    let title = if state.commits.is_empty() {
        " Commits ".to_string()
    } else {
        count_title(
            "Recent Commit",
            "Recent Commits",
            state.commits.len(),
            "commit",
            "commits",
        )
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    if state.commits.is_empty() {
        let empty = Paragraph::new(git_commits_empty_message())
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::MutedText));
        empty.render(inner, buf);
        return;
    }

    let available_height = inner.height as usize;
    if available_height == 0 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    for (idx, commit) in state.commits.iter().enumerate() {
        let is_selected = state.selected_commit == Some(idx);

        let hash_style = if is_selected {
            style_for_ui_element(theme, UiElement::ActiveItem)
        } else {
            style_for_ui_element(theme, UiElement::Secondary)
        };

        let message_style = if is_selected {
            style_for_ui_element(theme, UiElement::ActiveItem)
        } else {
            style_for_ui_element(theme, UiElement::Text)
        };

        let arrow = if is_selected { "▸ " } else { "  " };

        lines.push(Line::from(vec![
            Span::styled(arrow, hash_style),
            Span::styled(format!("{} ", commit.short_hash), hash_style),
            Span::styled(commit.subject.clone(), message_style),
        ]));
    }

    let scroll_offset =
        calculate_scroll_offset(lines.len(), available_height, state.selected_commit);
    let visible_lines: Vec<Line> = lines
        .iter()
        .skip(scroll_offset)
        .take(available_height)
        .cloned()
        .collect();

    let text = Paragraph::new(visible_lines);
    text.render(inner, buf);
}

/// Format relative time (e.g., "18 mins ago")
fn format_relative_time(date: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(*date);

    if duration.num_minutes() < 1 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        relative_time_label(duration.num_minutes(), "min", "mins")
    } else if duration.num_hours() < 24 {
        relative_time_label(duration.num_hours(), "hour", "hours")
    } else if duration.num_days() < 7 {
        relative_time_label(duration.num_days(), "day", "days")
    } else {
        date.format("%Y-%m-%d").to_string()
    }
}

fn relative_time_label(count: i64, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("1 {} ago", singular)
    } else {
        format!("{} {} ago", count, plural)
    }
}

fn count_title(
    singular: &str,
    plural: &str,
    count: usize,
    unit_singular: &str,
    unit_plural: &str,
) -> String {
    if count == 1 {
        format!(" {} (1 {}) ", singular, unit_singular)
    } else {
        format!(" {} ({} {}) ", plural, count, unit_plural)
    }
}

fn file_count_label(count: usize) -> String {
    if count == 1 {
        "1 file".to_string()
    } else {
        format!("{} files", count)
    }
}

/// Render commit details panel
fn render_commit_details(
    state: &PluginState,
    focus_pane: FocusPane,
    _focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let is_main_focused = focus_pane == FocusPane::Main;
    let border_style = if is_main_focused {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Border)
    };

    let title = state
        .selected_commit()
        .map(|c| format!(" Commit {} ", c.short_hash))
        .unwrap_or_else(|| " Commit ".to_string());

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    let Some(commit) = state.selected_commit() else {
        let empty = Paragraph::new(git_commit_details_empty_message(state))
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::MutedText))
            .wrap(ratatui::widgets::Wrap { trim: true });
        empty.render(inner, buf);
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    let text_style = style_for_ui_element(theme, UiElement::Text);
    let muted_style = style_for_ui_element(theme, UiElement::MutedText);
    let primary_style = style_for_ui_element(theme, UiElement::Primary);
    let header_style = style_for_ui_element(theme, UiElement::Secondary);
    let added_style = style_for_git_status(theme, "added");
    let deleted_style = style_for_git_status(theme, "deleted");

    // Commit hash header
    lines.push(Line::from(vec![
        Span::styled("Commit ", text_style),
        Span::styled(
            commit.short_hash.clone(),
            primary_style.add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    // Author and date
    lines.push(Line::from(vec![
        Span::styled("👤 ", text_style),
        Span::styled(
            commit.author.clone(),
            text_style.add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("🕐 ", text_style),
        Span::styled(format_relative_time(&commit.date), muted_style),
    ]));
    lines.push(Line::raw(""));

    // Commit message
    if let Some(msg) = &commit.message {
        for line in msg.lines() {
            lines.push(Line::styled(line.to_string(), text_style));
        }
    } else {
        lines.push(Line::styled(commit.subject.clone(), text_style));
    }
    lines.push(Line::raw(""));

    // Files and Diff section
    if let Some(ref diff) = state.commit_diff {
        if !diff.files.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Files (", text_style.add_modifier(Modifier::BOLD)),
                Span::styled(
                    file_count_label(diff.files.len()),
                    text_style.add_modifier(Modifier::BOLD),
                ),
                Span::styled(") ", text_style.add_modifier(Modifier::BOLD)),
                Span::styled(format!("+{}", diff.total_additions), added_style),
                Span::styled(format!(" -{}", diff.total_deletions), deleted_style),
            ]));
            lines.push(Line::raw(""));

            // Show diff for each file
            for file_diff in &diff.files {
                // File header with separator
                lines.push(Line::styled(
                    "─────────────────────────────────────────".to_string(),
                    muted_style,
                ));

                let status_label = if file_diff.is_created {
                    "Added"
                } else if file_diff.is_deleted {
                    "Deleted"
                } else if file_diff.is_renamed {
                    "Renamed"
                } else {
                    "Modified"
                };

                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{} ", status_label),
                        primary_style.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        file_diff.path.clone(),
                        text_style.add_modifier(Modifier::BOLD),
                    ),
                ]));

                // Show stats
                if file_diff.additions > 0 || file_diff.deletions > 0 {
                    lines.push(Line::from(vec![
                        Span::styled("  ", text_style),
                        Span::styled(format!("+{}", file_diff.additions), added_style),
                        Span::styled(format!(" -{}", file_diff.deletions), deleted_style),
                    ]));
                }
                lines.push(Line::raw(""));

                // Show diff hunks
                if let Some(status_label) = diff_content_status_label(file_diff) {
                    lines.push(Line::styled(format!("  {}", status_label), muted_style));
                } else {
                    for hunk in &file_diff.hunks {
                        // Hunk header
                        lines.push(Line::styled(format!("  {}", hunk.header), header_style));

                        // Diff lines with color coding
                        for line in &hunk.lines {
                            let (prefix, line_style) = match line.change_type {
                                crate::core::models::ChangeType::Added => ("+", added_style),
                                crate::core::models::ChangeType::Deleted => ("-", deleted_style),
                                crate::core::models::ChangeType::Context => (" ", text_style),
                                crate::core::models::ChangeType::Message => ("", muted_style),
                            };

                            lines.push(Line::from(vec![Span::styled(
                                format!("  {}{}", prefix, line.content),
                                line_style,
                            )]));
                        }
                    }
                }
                lines.push(Line::raw(""));
            }
        }
    } else if !state.commit_files.is_empty() {
        // Fallback: show file list without diff content
        let total_additions: usize = state.commit_files.iter().map(|f| f.additions).sum();
        let total_deletions: usize = state.commit_files.iter().map(|f| f.deletions).sum();

        lines.push(Line::from(vec![
            Span::styled("Files (", text_style.add_modifier(Modifier::BOLD)),
            Span::styled(
                file_count_label(state.commit_files.len()),
                text_style.add_modifier(Modifier::BOLD),
            ),
            Span::styled(") ", text_style.add_modifier(Modifier::BOLD)),
            Span::styled(format!("+{}", total_additions), added_style),
            Span::styled(format!(" -{}", total_deletions), deleted_style),
        ]));

        for file in &state.commit_files {
            let status_char = if file.is_renamed {
                'R'
            } else if file.is_created {
                'A'
            } else if file.is_deleted {
                'D'
            } else {
                'M'
            };

            let mut spans = vec![
                Span::styled(format!("{} ", status_char), muted_style),
                Span::styled(file.path.clone(), text_style),
            ];

            if file.additions > 0 || file.deletions > 0 {
                spans.push(Span::raw(" "));
                if file.additions > 0 {
                    spans.push(Span::styled(format!("+{}", file.additions), added_style));
                }
                if file.deletions > 0 {
                    spans.push(Span::styled(format!("-{}", file.deletions), deleted_style));
                }
            }

            lines.push(Line::from(spans));
        }
    }

    let text = Paragraph::new(lines);
    text.render(inner, buf);
}

/// Build a section header line
fn build_section_header<'a>(name: &'a str, _theme: &'a Theme, color: &'a str) -> Line<'a> {
    use ratatui::style::Color;
    use std::str::FromStr;

    let color = Color::from_str(color).unwrap_or(ratatui::style::Color::Gray);
    let style = Style::default().fg(color).add_modifier(Modifier::BOLD);

    Line::from(vec![Span::styled(
        format!("{} ({})", name, name.to_lowercase()),
        style,
    )])
}

/// Build a line for a file entry
fn build_file_line<'a>(file: &'a FileChange, is_selected: bool, theme: &'a Theme) -> Line<'a> {
    let status_char = match file.status {
        FileStatus::Staged => "S",
        FileStatus::Modified => "M",
        FileStatus::Untracked => "?",
        FileStatus::Deleted => "D",
        FileStatus::Renamed => "R",
        FileStatus::Conflicted => "!",
        _ => " ",
    };

    let status_style = match file.status {
        FileStatus::Staged => style_for_git_status(theme, "staged"),
        FileStatus::Modified => style_for_git_status(theme, "modified"),
        FileStatus::Untracked => style_for_git_status(theme, "untracked"),
        FileStatus::Deleted => style_for_git_status(theme, "deleted"),
        FileStatus::Renamed => style_for_git_status(theme, "modified"),
        FileStatus::Conflicted => style_for_ui_element(theme, UiElement::Error),
        _ => style_for_ui_element(theme, UiElement::Text),
    };

    let path_style = if is_selected {
        style_for_ui_element(theme, UiElement::ActiveItem)
    } else {
        style_for_ui_element(theme, UiElement::Text)
    };

    let mut spans = vec![
        Span::styled(format!(" {} ", status_char), status_style),
        Span::styled(file.path.clone(), path_style),
    ];

    // Add change counts if available
    if let (Some(adds), Some(dels)) = (file.additions, file.deletions) {
        if adds > 0 || dels > 0 {
            spans.push(Span::raw(" "));
            if adds > 0 {
                spans.push(Span::styled(
                    format!("+{}", adds),
                    style_for_git_status(theme, "added"),
                ));
            }
            if dels > 0 {
                spans.push(Span::styled(
                    format!("-{}", dels),
                    style_for_git_status(theme, "deleted"),
                ));
            }
        }
    }

    Line::from(spans)
}

fn build_commit_line<'a>(
    hash: &'a str,
    subject: &'a str,
    is_selected: bool,
    theme: &'a Theme,
) -> Line<'a> {
    let hash_style = if is_selected {
        style_for_ui_element(theme, UiElement::ActiveItem)
    } else {
        style_for_ui_element(theme, UiElement::Secondary)
    };

    let subject_style = if is_selected {
        style_for_ui_element(theme, UiElement::ActiveItem)
    } else {
        style_for_ui_element(theme, UiElement::Text)
    };

    let arrow = if is_selected { "▸ " } else { "  " };

    Line::from(vec![
        Span::styled(arrow, hash_style),
        Span::styled(format!("{} ", hash), hash_style),
        Span::styled(subject.to_string(), subject_style),
    ])
}

/// Calculate scroll offset to keep selection visible
fn calculate_scroll_offset(
    total_lines: usize,
    visible_height: usize,
    selected_line: Option<usize>,
) -> usize {
    if total_lines <= visible_height || visible_height == 0 {
        return 0;
    }

    let Some(selected_line) = selected_line else {
        return 0;
    };

    let max_offset = total_lines.saturating_sub(visible_height);
    selected_line
        .saturating_sub(visible_height / 2)
        .min(max_offset)
}

/// Render the branch list sidebar
fn render_branch_list(
    state: &PluginState,
    focus_pane: FocusPane,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let is_sidebar_focused = focus_pane == FocusPane::Sidebar;
    let border_style = if is_sidebar_focused && focused {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Border)
    };

    let title = count_title(
        "Branch",
        "Branches",
        state.branches.len(),
        "branch",
        "branches",
    );
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    if state.branches.is_empty() {
        let empty = Paragraph::new(git_branches_empty_message())
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::MutedText));
        empty.render(inner, buf);
        return;
    }

    let available_height = inner.height as usize;
    if available_height == 0 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    for (idx, branch) in state.branches.iter().enumerate() {
        let is_selected = state.selected_branch == Some(idx);
        let is_current = branch.is_current;

        let marker = if is_current { "● " } else { "  " };
        let name_style = if is_selected {
            style_for_ui_element(theme, UiElement::ActiveItem)
        } else if is_current {
            style_for_ui_element(theme, UiElement::Primary)
        } else if branch.is_remote {
            style_for_ui_element(theme, UiElement::MutedText)
        } else {
            style_for_ui_element(theme, UiElement::Text)
        };

        let arrow = if is_selected { "▸ " } else { "  " };

        lines.push(Line::from(vec![
            Span::styled(arrow, name_style),
            Span::styled(
                marker,
                if is_current {
                    style_for_ui_element(theme, UiElement::Success)
                } else {
                    Style::default()
                },
            ),
            Span::styled(branch.name.clone(), name_style),
        ]));
    }

    let scroll_offset =
        calculate_scroll_offset(lines.len(), available_height, state.selected_branch);
    let visible_lines: Vec<Line> = lines
        .iter()
        .skip(scroll_offset)
        .take(available_height)
        .cloned()
        .collect();

    Paragraph::new(visible_lines).render(inner, buf);
}

/// Render branch details in the main pane
fn render_branch_details(
    state: &PluginState,
    focus_pane: FocusPane,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let is_main_focused = focus_pane == FocusPane::Main;
    let border_style = if is_main_focused {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Border)
    };

    let block = Block::default()
        .title(" Branch Details ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    let text_style = style_for_ui_element(theme, UiElement::Text);
    let muted_style = style_for_ui_element(theme, UiElement::MutedText);

    let mut lines: Vec<Line> = Vec::new();

    if let Some(branch) = state.selected_branch() {
        lines.push(Line::from(vec![
            Span::styled("Branch: ", muted_style),
            Span::styled(branch.name.clone(), text_style.add_modifier(Modifier::BOLD)),
        ]));
        if branch.is_current {
            lines.push(Line::styled(
                "  ● Current branch",
                style_for_ui_element(theme, UiElement::Success),
            ));
        }
        if branch.is_remote {
            lines.push(Line::styled("  Remote tracking branch", muted_style));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled("Shortcuts:", muted_style));
        lines.push(Line::styled("  Enter: Checkout branch", text_style));
        lines.push(Line::styled("  n: Create new branch", text_style));
        lines.push(Line::styled("  d: Delete branch", text_style));
        lines.push(Line::styled("  y: Copy branch name", text_style));
        lines.push(Line::styled("  S: Back to Status", text_style));
    } else {
        let empty = Paragraph::new(git_branch_details_empty_message(state))
            .alignment(Alignment::Center)
            .style(muted_style)
            .wrap(ratatui::widgets::Wrap { trim: true });
        empty.render(inner, buf);
        return;
    }

    Paragraph::new(lines).render(inner, buf);
}

/// Render the stash list sidebar
fn render_stash_list(
    state: &PluginState,
    focus_pane: FocusPane,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let is_sidebar_focused = focus_pane == FocusPane::Sidebar;
    let border_style = if is_sidebar_focused && focused {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Border)
    };

    let title = count_title("Stash", "Stashes", state.stashes.len(), "stash", "stashes");
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    if state.stashes.is_empty() {
        let empty = Paragraph::new(git_stashes_empty_message())
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::MutedText));
        empty.render(inner, buf);
        return;
    }

    let available_height = inner.height as usize;
    if available_height == 0 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    for (idx, stash) in state.stashes.iter().enumerate() {
        let is_selected = state.selected_stash == Some(idx);

        let style = if is_selected {
            style_for_ui_element(theme, UiElement::ActiveItem)
        } else {
            style_for_ui_element(theme, UiElement::Text)
        };

        let idx_style = if is_selected {
            style_for_ui_element(theme, UiElement::ActiveItem)
        } else {
            style_for_ui_element(theme, UiElement::Secondary)
        };

        let arrow = if is_selected { "▸ " } else { "  " };

        lines.push(Line::from(vec![
            Span::styled(arrow, style),
            Span::styled(format!("@{{{}}} ", stash.index), idx_style),
            Span::styled(stash.message.clone(), style),
        ]));
    }

    let scroll_offset =
        calculate_scroll_offset(lines.len(), available_height, state.selected_stash);
    let visible_lines: Vec<Line> = lines
        .iter()
        .skip(scroll_offset)
        .take(available_height)
        .cloned()
        .collect();

    Paragraph::new(visible_lines).render(inner, buf);
}

/// Render stash details in the main pane
fn render_stash_details(
    state: &PluginState,
    focus_pane: FocusPane,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let is_main_focused = focus_pane == FocusPane::Main;
    let border_style = if is_main_focused {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Border)
    };

    let block = Block::default()
        .title(" Stash Details ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    let text_style = style_for_ui_element(theme, UiElement::Text);
    let muted_style = style_for_ui_element(theme, UiElement::MutedText);

    let mut lines: Vec<Line> = Vec::new();

    if let Some(stash) = state.selected_stash() {
        lines.push(Line::from(vec![
            Span::styled("Stash: ", muted_style),
            Span::styled(
                format!("stash@{{{}}}", stash.index),
                text_style.add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Message: ", muted_style),
            Span::styled(stash.message.clone(), text_style),
        ]));
        if let Some(ref branch) = stash.branch {
            if !branch.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("Branch: ", muted_style),
                    Span::styled(branch.clone(), text_style),
                ]));
            }
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled("Shortcuts:", muted_style));
        lines.push(Line::styled(
            "  Enter: Pop stash (apply & drop)",
            text_style,
        ));
        lines.push(Line::styled("  s: Stash current changes", text_style));
        lines.push(Line::styled("  d: Drop stash entry", text_style));
        lines.push(Line::styled("  y: Copy stash info", text_style));
        lines.push(Line::styled("  S: Back to Status", text_style));
    } else {
        let empty = Paragraph::new(git_stash_details_empty_message(state))
            .alignment(Alignment::Center)
            .style(muted_style)
            .wrap(ratatui::widgets::Wrap { trim: true });
        empty.render(inner, buf);
        return;
    }

    Paragraph::new(lines).render(inner, buf);
}

/// Render a modal overlay
fn render_modal_overlay(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let Some(ref modal) = state.active_modal else {
        return;
    };

    let Some(modal_area) = git_modal_area(area) else {
        return;
    };

    // Clear the area
    for row in modal_area.top()..modal_area.bottom() {
        for col in modal_area.left()..modal_area.right() {
            if let Some(cell) = buf.cell_mut((col, row)) {
                cell.set_char(' ');
                cell.set_style(Style::default());
            }
        }
    }

    let (title, body) = match modal {
        super::state::GitModal::CommitMessage => ("Commit", "Enter commit message..."),
        super::state::GitModal::CreateBranch => ("Create Branch", "Enter branch name..."),
        super::state::GitModal::DeleteBranch { name } => {
            // Can't return a reference to a temporary, so handle inline
            let block = Block::default()
                .title(" Delete Branch ")
                .borders(Borders::ALL)
                .border_style(style_for_ui_element(theme, UiElement::Error));
            let inner = block.inner(modal_area);
            block.render(modal_area, buf);

            let text = Paragraph::new(vec![
                Line::styled(
                    format!("Delete branch '{}'?", name),
                    style_for_ui_element(theme, UiElement::Text),
                ),
                Line::raw(""),
                Line::styled(
                    GIT_DELETE_BRANCH_MODAL_HINT,
                    style_for_ui_element(theme, UiElement::MutedText),
                ),
            ]);
            text.render(inner, buf);
            return;
        }
        super::state::GitModal::DropStash { index } => {
            let block = Block::default()
                .title(" Drop Stash ")
                .borders(Borders::ALL)
                .border_style(style_for_ui_element(theme, UiElement::Error));
            let inner = block.inner(modal_area);
            block.render(modal_area, buf);

            let text = Paragraph::new(vec![
                Line::styled(
                    format!("Drop stash@{{{}}}?", index),
                    style_for_ui_element(theme, UiElement::Text),
                ),
                Line::raw(""),
                Line::styled(
                    GIT_DROP_STASH_MODAL_HINT,
                    style_for_ui_element(theme, UiElement::MutedText),
                ),
            ]);
            text.render(inner, buf);
            return;
        }
        super::state::GitModal::Error { message } => {
            let block = Block::default()
                .title(" Error ")
                .borders(Borders::ALL)
                .border_style(style_for_ui_element(theme, UiElement::Error));
            let inner = block.inner(modal_area);
            block.render(modal_area, buf);

            let text = Paragraph::new(vec![
                Line::styled(
                    message.clone(),
                    style_for_ui_element(theme, UiElement::Error),
                ),
                Line::raw(""),
                Line::styled(
                    GIT_ERROR_MODAL_HINT,
                    style_for_ui_element(theme, UiElement::MutedText),
                ),
            ])
            .wrap(ratatui::widgets::Wrap { trim: false });
            text.render(inner, buf);
            return;
        }
    };

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Primary));
    let inner = block.inner(modal_area);
    block.render(modal_area, buf);

    let text = Paragraph::new(vec![
        Line::styled(body, style_for_ui_element(theme, UiElement::Text)),
        Line::raw(""),
        Line::styled(
            GIT_CANCEL_MODAL_HINT,
            style_for_ui_element(theme, UiElement::MutedText),
        ),
    ]);
    text.render(inner, buf);
}

fn git_modal_area(area: Rect) -> Option<Rect> {
    if area.width < MIN_GIT_MODAL_WIDTH || area.height < MIN_GIT_MODAL_HEIGHT {
        return None;
    }

    let width = GIT_MODAL_WIDTH.min(area.width);
    let height = GIT_MODAL_HEIGHT.min(area.height);
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);

    Some(Rect::new(x, y, width, height))
}

/// Render the status bar info
pub fn render_status_info(state: &PluginState) -> String {
    let mut parts = Vec::new();

    if state.branch.is_empty() && state.files.is_empty() && state.commits.is_empty() {
        return "No repository data loaded | r Refresh git status | /: Global search | :: Command search | ? Toggle help"
            .to_string();
    }

    // Branch status
    if !state.branch.is_empty() {
        parts.push(state.branch.clone());
    }

    // Ahead/behind
    if state.ahead > 0 || state.behind > 0 {
        let mut sync_parts = Vec::new();
        if state.ahead > 0 {
            sync_parts.push(format!("↑{}", state.ahead));
        }
        if state.behind > 0 {
            sync_parts.push(format!("↓{}", state.behind));
        }
        parts.push(format!("[{}]", sync_parts.join(",")));
    }

    // File counts
    let staged = state.staged_files().len();
    let unstaged = state.unstaged_files().len();
    let untracked = state.untracked_files().len();

    if staged > 0 {
        parts.push(format!("{} staged", staged));
    }
    if unstaged > 0 {
        parts.push(format!("{} unstaged", unstaged));
    }
    if untracked > 0 {
        parts.push(format!("{} untracked", untracked));
    }

    if staged == 0 && unstaged == 0 && untracked == 0 {
        parts.push("clean".to_string());
    }

    if !state.commits.is_empty() {
        let suffix = if state.commits.len() == 1 {
            "commit"
        } else {
            "commits"
        };
        parts.push(format!("{} {}", state.commits.len(), suffix));
    }

    parts.join(" | ")
}

fn git_changes_empty_message(state: &PluginState) -> &'static str {
    if state.branch.is_empty() {
        "No repository data loaded\n\nr: Refresh git status\n/: Global search  |  :: Command search\n?: Toggle help"
    } else {
        "Working tree clean\n\nB: Branches\nH: History\nr: Refresh git status\n/: Global search  |  :: Command search\n?: Toggle help"
    }
}

fn git_diff_empty_message(state: &PluginState) -> &'static str {
    if state.files.is_empty() {
        "Working tree clean\n\nH: History\nB: Branches\nr: Refresh git status\n/: Global search  |  :: Command search\n?: Toggle help"
    } else {
        "No file selected\n\nj/k: Navigate files\nS: Status\nH: History\nB: Branches\nr: Refresh git status\n/: Global search  |  :: Command search\n?: Toggle help"
    }
}

fn git_file_no_diff_message(file: &FileChange) -> String {
    format!(
        "No diff available for {}\n\nj/k: Navigate files\ns: Stage\nu: Unstage\nc: Commit\nr: Refresh git status\n/: Global search  |  :: Command search\n?: Toggle help",
        file.path
    )
}

fn git_sidebar_empty_message(state: &PluginState) -> &'static str {
    if state.branch.is_empty() {
        "No repository data loaded\n\nr: Refresh git status\n/: Global search  |  :: Command search\n?: Toggle help"
    } else {
        "No changes or commits\n\nB: Branches\nH: History\nr: Refresh git status\n/: Global search  |  :: Command search\n?: Toggle help"
    }
}

fn git_commits_empty_message() -> &'static str {
    "No commits\n\nS: Status\nB: Branches\nr: Refresh git status\n/: Global search  |  :: Command search\n?: Toggle help"
}

fn git_commit_details_empty_message(state: &PluginState) -> &'static str {
    if state.commits.is_empty() {
        git_commits_empty_message()
    } else {
        "No commit selected\n\nj/k: Navigate commits\nS: Status\nB: Branches\nr: Refresh git status\n/: Global search  |  :: Command search\n?: Toggle help"
    }
}

fn git_branches_empty_message() -> &'static str {
    "No branches\n\nS: Status\nH: History\nr: Refresh git status\n/: Global search  |  :: Command search\n?: Toggle help"
}

fn git_branch_details_empty_message(state: &PluginState) -> &'static str {
    if state.branches.is_empty() {
        git_branches_empty_message()
    } else {
        "No branch selected\n\nj/k: Navigate branches\nn: New branch\nS: Status\nH: History\nr: Refresh git status\n/: Global search  |  :: Command search\n?: Toggle help"
    }
}

fn git_stashes_empty_message() -> &'static str {
    "No stashes\n\ns: Save stash\nS: Status\nB: Branches\nr: Refresh git status\n/: Global search  |  :: Command search\n?: Toggle help"
}

fn git_stash_details_empty_message(state: &PluginState) -> &'static str {
    if state.stashes.is_empty() {
        git_stashes_empty_message()
    } else {
        "No stash selected\n\nj/k: Navigate stashes\ns: Save stash\nS: Status\nB: Branches\nr: Refresh git status\n/: Global search  |  :: Command search\n?: Toggle help"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_file_line() {
        let theme = Theme::default();
        let file = FileChange::new("test.rs", FileStatus::Modified);
        let line = build_file_line(&file, false, &theme);
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn test_build_section_header() {
        let theme = Theme::default();
        let line = build_section_header("Staged", &theme, &theme.colors.added);
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn test_git_modal_hints_use_compact_action_case() {
        let hints = [
            GIT_DELETE_BRANCH_MODAL_HINT,
            GIT_DROP_STASH_MODAL_HINT,
            GIT_CANCEL_MODAL_HINT,
            GIT_ERROR_MODAL_HINT,
        ];

        assert!(GIT_DELETE_BRANCH_MODAL_HINT.contains("Enter/D: Delete"));
        assert!(GIT_DELETE_BRANCH_MODAL_HINT.contains("Esc: Cancel"));
        assert!(GIT_DROP_STASH_MODAL_HINT.contains("Enter/D: Drop"));
        assert!(GIT_DROP_STASH_MODAL_HINT.contains("Esc: Cancel"));
        assert!(GIT_CANCEL_MODAL_HINT.contains("Esc: Cancel"));
        assert!(GIT_ERROR_MODAL_HINT.contains("Esc: Close"));
        assert!(!hints.iter().any(|hint| hint.contains("Escape=")));
        assert!(!hints.iter().any(|hint| hint.contains("Enter=")));
        assert!(!hints.iter().any(|hint| hint.contains("Confirm")));
    }

    #[test]
    fn test_render_branch_details_uses_action_case_shortcuts() {
        let mut state = PluginState::new();
        state.branches.push(crate::core::models::Branch {
            name: "feature/git-ux".to_string(),
            full_name: "refs/heads/feature/git-ux".to_string(),
            is_current: false,
            is_remote: false,
            remote: None,
            commit_hash: "abc1234".to_string(),
            upstream: None,
            ahead: None,
            behind: None,
        });
        state.selected_branch = Some(0);
        let theme = Theme::default();
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);

        render_branch_details(&state, FocusPane::Main, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Shortcuts:"));
        assert!(content.contains("Enter: Checkout branch"));
        assert!(content.contains("n: Create new branch"));
        assert!(content.contains("d: Delete branch"));
        assert!(content.contains("y: Copy branch name"));
        assert!(content.contains("S: Back to Status"));
        assert!(!content.contains("Enter  Checkout branch"));
    }

    #[test]
    fn test_render_stash_details_uses_action_case_shortcuts() {
        let mut state = PluginState::new();
        state.stashes.push(crate::core::models::Stash {
            index: 0,
            message: "WIP git UX".to_string(),
            commit_hash: "def5678".to_string(),
            date: None,
            branch: Some("main".to_string()),
        });
        state.selected_stash = Some(0);
        let theme = Theme::default();
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);

        render_stash_details(&state, FocusPane::Main, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Shortcuts:"));
        assert!(content.contains("Enter: Pop stash (apply & drop)"));
        assert!(content.contains("s: Stash current changes"));
        assert!(content.contains("d: Drop stash entry"));
        assert!(content.contains("y: Copy stash info"));
        assert!(content.contains("S: Back to Status"));
        assert!(!content.contains("Enter  Pop stash"));
    }

    #[test]
    fn test_render_delete_branch_modal_uses_handled_key_hint() {
        let mut state = PluginState::new();
        state.open_modal(crate::plugins::gitstatus::GitModal::DeleteBranch {
            name: "old-branch".to_string(),
        });
        let theme = Theme::default();
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);

        render_modal_overlay(&state, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Delete branch 'old-branch'?"));
        assert!(content.contains(GIT_DELETE_BRANCH_MODAL_HINT));
        assert!(!content.contains("Enter: Confirm"));
    }

    #[test]
    fn test_render_drop_stash_modal_uses_handled_key_hint() {
        let mut state = PluginState::new();
        state.open_modal(crate::plugins::gitstatus::GitModal::DropStash { index: 2 });
        let theme = Theme::default();
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);

        render_modal_overlay(&state, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Drop stash@{2}?"));
        assert!(content.contains(GIT_DROP_STASH_MODAL_HINT));
        assert!(!content.contains("Enter: Confirm"));
    }

    #[test]
    fn test_git_modal_area_uses_preferred_size_when_it_fits() {
        let area = Rect::new(10, 5, 100, 30);
        let modal = git_modal_area(area).unwrap();

        assert_eq!(modal, Rect::new(35, 16, 50, 7));
    }

    #[test]
    fn test_git_modal_area_clamps_to_available_area() {
        let area = Rect::new(4, 3, 30, 6);
        let modal = git_modal_area(area).unwrap();

        assert_eq!(modal, Rect::new(4, 3, 30, 6));
    }

    #[test]
    fn test_git_modal_area_handles_offset_near_u16_max() {
        let area = Rect::new(u16::MAX - 80, u16::MAX - 20, 80, 20);
        let modal = git_modal_area(area).unwrap();

        assert_eq!(modal, Rect::new(u16::MAX - 65, u16::MAX - 14, 50, 7));
    }

    #[test]
    fn test_render_modal_overlay_handles_offset_area_near_u16_max() {
        let mut state = PluginState::new();
        state.open_modal(crate::plugins::gitstatus::GitModal::Error {
            message: "Cannot drop stash".to_string(),
        });
        let theme = Theme::default();
        let area = Rect::new(u16::MAX - 80, u16::MAX - 20, 80, 20);
        let mut buf = Buffer::empty(area);

        render_modal_overlay(&state, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Error"));
        assert!(content.contains("Cannot drop stash"));
    }

    #[test]
    fn test_git_modal_area_skips_tiny_areas() {
        assert!(git_modal_area(Rect::new(0, 0, 19, 10)).is_none());
        assert!(git_modal_area(Rect::new(0, 0, 30, 4)).is_none());
    }

    #[test]
    fn test_render_status_info() {
        let mut state = PluginState::new();
        state.branch = "main".to_string();
        state.ahead = 1;
        state.behind = 0;
        state
            .files
            .push(FileChange::new("test.rs", FileStatus::Modified));

        let info = render_status_info(&state);
        assert!(!info.is_empty());
        assert!(info.contains("main"));
        assert!(info.contains("↑1"));
    }

    #[test]
    fn test_render_status_info_clean() {
        let mut state = PluginState::new();
        state.branch = "main".to_string();
        let info = render_status_info(&state);
        assert!(info.contains("clean"));
    }

    #[test]
    fn test_render_status_info_points_to_refresh_when_unloaded() {
        let state = PluginState::new();
        let info = render_status_info(&state);

        assert_eq!(
            info,
            "No repository data loaded | r Refresh git status | /: Global search | :: Command search | ? Toggle help"
        );
    }

    #[test]
    fn test_git_empty_messages_surface_command_search() {
        let assert_hint = |message: &str| {
            assert!(message.contains("/: Global search"), "{message}");
            assert!(message.contains(":: Command search"), "{message}");
        };

        let unloaded = PluginState::new();
        assert_hint(git_changes_empty_message(&unloaded));
        assert_hint(git_diff_empty_message(&unloaded));
        assert_hint(git_sidebar_empty_message(&unloaded));
        assert!(render_status_info(&unloaded).contains(":: Command search"));

        let mut clean = PluginState::new();
        clean.branch = "main".to_string();
        assert_hint(git_changes_empty_message(&clean));
        assert_hint(git_diff_empty_message(&clean));
        assert_hint(git_sidebar_empty_message(&clean));

        let mut with_file = PluginState::new();
        with_file
            .files
            .push(FileChange::new("src/main.rs", FileStatus::Modified));
        assert_hint(git_diff_empty_message(&with_file));
        assert_hint(&git_file_no_diff_message(&with_file.files[0]));

        assert_hint(git_commits_empty_message());
        assert_hint(git_commit_details_empty_message(&clean));
        assert_hint(git_branches_empty_message());
        assert_hint(git_branch_details_empty_message(&clean));
        assert_hint(git_stashes_empty_message());
        assert_hint(git_stash_details_empty_message(&clean));
    }

    #[test]
    fn test_render_status_info_includes_commit_count() {
        let mut state = PluginState::new();
        state.branch = "main".to_string();
        state.commits.push(crate::core::models::Commit::new(
            "abc123",
            "Initial commit",
            "Test User",
            chrono::Utc::now(),
        ));

        let info = render_status_info(&state);

        assert!(info.contains("main"));
        assert!(info.contains("clean"));
        assert!(info.contains("1 commit"));
    }

    #[test]
    fn test_format_relative_time_uses_singular_labels() {
        let now = chrono::Utc::now();

        assert_eq!(
            format_relative_time(&(now - chrono::Duration::minutes(1))),
            "1 min ago"
        );
        assert_eq!(
            format_relative_time(&(now - chrono::Duration::hours(1))),
            "1 hour ago"
        );
        assert_eq!(
            format_relative_time(&(now - chrono::Duration::days(1))),
            "1 day ago"
        );
    }

    #[test]
    fn test_count_title_uses_singular_labels() {
        assert_eq!(
            count_title("Recent Commit", "Recent Commits", 1, "commit", "commits"),
            " Recent Commit (1 commit) "
        );
        assert_eq!(
            count_title("Recent Commit", "Recent Commits", 2, "commit", "commits"),
            " Recent Commits (2 commits) "
        );
        assert_eq!(
            count_title("Branch", "Branches", 1, "branch", "branches"),
            " Branch (1 branch) "
        );
        assert_eq!(
            count_title("Branch", "Branches", 2, "branch", "branches"),
            " Branches (2 branches) "
        );
        assert_eq!(
            count_title("Stash", "Stashes", 1, "stash", "stashes"),
            " Stash (1 stash) "
        );
        assert_eq!(
            count_title("Stash", "Stashes", 2, "stash", "stashes"),
            " Stashes (2 stashes) "
        );
    }

    #[test]
    fn test_file_count_label_uses_singular_labels() {
        assert_eq!(file_count_label(0), "0 files");
        assert_eq!(file_count_label(1), "1 file");
        assert_eq!(file_count_label(2), "2 files");
    }

    #[test]
    fn test_git_changes_empty_message_points_to_actions() {
        let mut state = PluginState::new();
        state.branch = "main".to_string();

        let message = git_changes_empty_message(&state);

        assert!(message.contains("Working tree clean"));
        assert!(message.contains("B: Branches"));
        assert!(message.contains("H: History"));
        assert!(message.contains("r: Refresh git status"));
        assert!(message.contains("/: Global search"));
        assert!(message.contains(":: Command search"));
        assert!(message.contains("?: Toggle help"));
    }

    #[test]
    fn test_git_changes_empty_message_handles_unloaded_repo() {
        let state = PluginState::new();
        let message = git_changes_empty_message(&state);

        assert!(message.contains("No repository data loaded"));
        assert!(message.contains("r: Refresh git status"));
        assert!(message.contains("/: Global search"));
        assert!(message.contains(":: Command search"));
        assert!(message.contains("?: Toggle help"));
    }

    #[test]
    fn test_git_diff_empty_message_reflects_clean_tree() {
        let state = PluginState::new();
        let message = git_diff_empty_message(&state);

        assert!(message.contains("Working tree clean"));
        assert!(message.contains("H: History"));
        assert!(message.contains("B: Branches"));
        assert!(message.contains("r: Refresh git status"));
        assert!(message.contains("/: Global search"));
        assert!(message.contains(":: Command search"));
        assert!(message.contains("?: Toggle help"));
    }

    #[test]
    fn test_git_diff_empty_message_points_to_file_navigation() {
        let mut state = PluginState::new();
        state
            .files
            .push(FileChange::new("src/main.rs", FileStatus::Modified));

        let message = git_diff_empty_message(&state);

        assert!(message.contains("No file selected"));
        assert!(message.contains("j/k: Navigate files"));
        assert!(message.contains("S: Status"));
        assert!(message.contains("H: History"));
        assert!(message.contains("B: Branches"));
        assert!(message.contains("r: Refresh git status"));
        assert!(message.contains("/: Global search"));
        assert!(message.contains(":: Command search"));
        assert!(message.contains("?: Toggle help"));
    }

    #[test]
    fn test_render_selected_file_without_diff_points_to_file_actions() {
        let mut state = PluginState::new();
        state
            .files
            .push(FileChange::new("src/main.rs", FileStatus::Modified));
        state.selected_file = Some(0);
        state.diff = None;
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_git_status(&state, area, &mut buf, &theme, true);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No diff available for src/main.rs"));
        assert!(content.contains("j/k: Navigate files"));
        assert!(content.contains("s: Stage"));
        assert!(content.contains("u: Unstage"));
        assert!(content.contains("c: Commit"));
        assert!(content.contains("r: Refresh git status"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(":: Command search"));
        assert!(content.contains("?: Toggle help"));
    }

    #[test]
    fn test_render_diff_with_empty_file_content_mentions_missing_content() {
        let mut state = PluginState::new();
        state
            .files
            .push(FileChange::new("src/main.rs", FileStatus::Modified));
        state.selected_file = Some(0);
        state.diff = Some(crate::core::models::Diff {
            files: vec![crate::core::models::FileDiff::new("src/main.rs")],
            files_changed: 1,
            total_additions: 0,
            total_deletions: 0,
        });
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_git_status(&state, area, &mut buf, &theme, true);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("File:"));
        assert!(content.contains("src/main.rs"));
        assert!(content.contains("No diff content available"));
    }

    #[test]
    fn test_diff_content_status_label_handles_binary_and_empty_diffs() {
        let mut binary = FileDiff::new("assets/logo.png");
        binary.is_binary = true;
        assert_eq!(diff_content_status_label(&binary), Some("Binary file"));

        let empty = FileDiff::new("src/main.rs");
        assert_eq!(
            diff_content_status_label(&empty),
            Some("No diff content available")
        );
    }

    #[test]
    fn test_git_sidebar_empty_message_points_to_next_actions() {
        let state = PluginState::new();
        let unloaded = git_sidebar_empty_message(&state);
        assert!(unloaded.contains("No repository data loaded"));
        assert!(unloaded.contains("r: Refresh git status"));
        assert!(unloaded.contains("/: Global search"));
        assert!(unloaded.contains(":: Command search"));
        assert!(unloaded.contains("?: Toggle help"));

        let mut clean_state = PluginState::new();
        clean_state.branch = "main".to_string();
        let clean = git_sidebar_empty_message(&clean_state);
        assert!(clean.contains("No changes or commits"));
        assert!(clean.contains("B: Branches"));
        assert!(clean.contains("H: History"));
        assert!(clean.contains("r: Refresh git status"));
        assert!(clean.contains("/: Global search"));
        assert!(clean.contains(":: Command search"));
        assert!(clean.contains("?: Toggle help"));
    }

    #[test]
    fn test_render_git_status_clean_diff_includes_next_actions() {
        let state = PluginState::new();
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_git_status(&state, area, &mut buf, &theme, true);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Working tree clean"));
        assert!(content.contains("B: Branches"));
        assert!(content.contains("H: History"));
        assert!(content.contains("r: Refresh git status"));
    }

    #[test]
    fn test_git_subview_empty_messages_point_to_next_actions() {
        let commits = git_commits_empty_message();
        assert!(commits.contains("No commits"));
        assert!(commits.contains("S: Status"));
        assert!(commits.contains("B: Branches"));
        assert!(commits.contains("r: Refresh git status"));

        let branches = git_branches_empty_message();
        assert!(branches.contains("No branches"));
        assert!(branches.contains("S: Status"));
        assert!(branches.contains("H: History"));
        assert!(branches.contains("r: Refresh git status"));

        let stashes = git_stashes_empty_message();
        assert!(stashes.contains("No stashes"));
        assert!(stashes.contains("s: Save stash"));
        assert!(stashes.contains("S: Status"));
        assert!(stashes.contains("B: Branches"));
        assert!(stashes.contains("r: Refresh git status"));
    }

    #[test]
    fn test_git_detail_empty_messages_point_to_navigation_actions() {
        let mut state = PluginState::new();
        state.commits.push(crate::core::models::Commit::new(
            "abc1234",
            "Improve git UX",
            "Test User",
            chrono::Utc::now(),
        ));
        state.branches.push(crate::core::models::Branch {
            name: "feature/git-ux".to_string(),
            full_name: "refs/heads/feature/git-ux".to_string(),
            is_current: false,
            is_remote: false,
            remote: None,
            commit_hash: "abc1234".to_string(),
            upstream: None,
            ahead: None,
            behind: None,
        });
        state.stashes.push(crate::core::models::Stash {
            index: 0,
            message: "WIP git UX".to_string(),
            commit_hash: "def5678".to_string(),
            date: None,
            branch: Some("main".to_string()),
        });

        let commit = git_commit_details_empty_message(&state);
        assert!(commit.contains("No commit selected"));
        assert!(commit.contains("j/k: Navigate commits"));
        assert!(commit.contains("S: Status"));
        assert!(commit.contains("/: Global search"));
        assert!(commit.contains(":: Command search"));
        assert!(commit.contains("?: Toggle help"));

        let branch = git_branch_details_empty_message(&state);
        assert!(branch.contains("No branch selected"));
        assert!(branch.contains("j/k: Navigate branches"));
        assert!(branch.contains("n: New branch"));
        assert!(branch.contains("/: Global search"));
        assert!(branch.contains(":: Command search"));
        assert!(branch.contains("?: Toggle help"));

        let stash = git_stash_details_empty_message(&state);
        assert!(stash.contains("No stash selected"));
        assert!(stash.contains("j/k: Navigate stashes"));
        assert!(stash.contains("s: Save stash"));
        assert!(stash.contains("/: Global search"));
        assert!(stash.contains(":: Command search"));
        assert!(stash.contains("?: Toggle help"));
    }
}
