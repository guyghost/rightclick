//! Line-builder helpers (ratatui `Line<'a>`) for the git status plugin.
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::core::models::{FileChange, FileStatus, Theme};
use crate::theme::{UiElement, style_for_git_status, style_for_ui_element};

pub(super) fn build_section_header<'a>(
    name: &'a str,
    _theme: &'a Theme,
    color: &'a str,
) -> Line<'a> {
    use ratatui::style::Color;
    use std::str::FromStr;

    let color = Color::from_str(color).unwrap_or(ratatui::style::Color::Gray);
    let style = Style::default().fg(color).add_modifier(Modifier::BOLD);

    Line::from(vec![Span::styled(
        format!("{} ({})", name, name.to_lowercase()),
        style,
    )])
}

pub(super) fn build_file_line<'a>(
    file: &'a FileChange,
    is_selected: bool,
    theme: &'a Theme,
) -> Line<'a> {
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

pub(super) fn build_commit_line<'a>(
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
