//! TD Monitor Plugin Rendering
//!
//! This module handles all rendering logic for the TD Monitor plugin,
//! including the task list, board view, and activity log.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Table, Widget},
};

use crate::core::models::Theme;
use crate::theme::{style_for_ui_element, UiElement};

use super::state::{PluginState, Priority, Task, TaskStatus, ViewMode};

/// Render the TD Monitor plugin
pub fn render_td_monitor(
    state: &PluginState,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    // Main layout: sidebar (task list) + main content
    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(state.sidebar_width), Constraint::Min(20)])
        .split(area);

    let sidebar_area = main_layout[0];
    let main_area = main_layout[1];

    // Render sidebar with task list
    render_task_list(state, focused, sidebar_area, buf, theme);

    // Render main content based on view mode
    match state.view_mode {
        ViewMode::List => {
            render_task_detail(state, focused, main_area, buf, theme);
        }
        ViewMode::Board => {
            render_board_view(state, focused, main_area, buf, theme);
        }
    }
}

/// Render the task list sidebar
fn render_task_list(
    state: &PluginState,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let border_style = if focused {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Border)
    };

    // Build title
    let title = if state.filter_input_active {
        format!(" Tasks /{} ", state.filter_input)
    } else if let Some(filter) = &state.filter {
        format!(" Tasks [{}] ", filter)
    } else {
        " Tasks ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    // Handle loading state
    if state.is_loading {
        let loading = Paragraph::new("Loading...")
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::MutedText));
        loading.render(inner, buf);
        return;
    }

    // Handle error state
    if let Some(ref error) = state.error {
        let error_text = Paragraph::new(format!("Error: {}", error))
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::Error));
        error_text.render(inner, buf);
        return;
    }

    let tasks = state.filtered_tasks();
    if tasks.is_empty() {
        let empty_text = if state.filter.is_some() {
            "No matching tasks"
        } else {
            "No tasks"
        };
        let empty = Paragraph::new(empty_text)
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::MutedText));
        empty.render(inner, buf);
        return;
    }

    // Build task lines
    let mut lines: Vec<Line> = Vec::new();
    for (idx, task) in tasks.iter().enumerate() {
        let is_selected = state.selected_task == Some(idx);
        lines.push(build_task_line(task, is_selected, theme));
    }

    // Calculate scroll offset
    let available_height = inner.height as usize;
    let scroll_offset = calculate_scroll_offset(&lines, available_height, state.selected_task);

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

/// Build a line for a task entry
fn build_task_line<'a>(task: &'a Task, is_selected: bool, theme: &'a Theme) -> Line<'a> {
    let status_icon = task.status.icon();
    let status_style = style_for_task_status(theme, task.status);

    let priority_indicator = match task.priority {
        Priority::Critical => "! ",
        Priority::High => "^ ",
        _ => "  ",
    };
    let priority_style = style_for_priority(theme, task.priority);

    let path_style = if is_selected {
        style_for_ui_element(theme, UiElement::ActiveItem)
    } else {
        style_for_ui_element(theme, UiElement::Text)
    };

    // Truncate title if needed
    let title = if task.title.len() > 40 {
        format!("{}...", &task.title[..37])
    } else {
        task.title.clone()
    };

    Line::from(vec![
        Span::styled(format!(" {} ", status_icon), status_style),
        Span::styled(priority_indicator, priority_style),
        Span::styled(title, path_style),
    ])
}

/// Render the task detail view
fn render_task_detail(
    state: &PluginState,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let border_style = if focused {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Border)
    };

    let block = Block::default()
        .title(" Task Details ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    if let Some(task) = state.selected_task() {
        let mut lines: Vec<Line> = Vec::new();

        // Title
        lines.push(Line::from(vec![
            Span::styled("Title: ", style_for_ui_element(theme, UiElement::Primary)),
            Span::styled(
                &task.title,
                style_for_ui_element(theme, UiElement::Text).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::raw(""));

        // Status and Priority
        lines.push(Line::from(vec![
            Span::styled("Status: ", style_for_ui_element(theme, UiElement::Primary)),
            Span::styled(
                format!("{} {}", task.status.icon(), task.status.as_str()),
                style_for_task_status(theme, task.status),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Priority: ", style_for_ui_element(theme, UiElement::Primary)),
            Span::styled(
                format!("{} {}", task.priority.icon(), task.priority.as_str()),
                style_for_priority(theme, task.priority),
            ),
        ]));
        lines.push(Line::raw(""));

        // Description
        if let Some(ref desc) = task.description {
            lines.push(Line::from(vec![Span::styled(
                "Description:",
                style_for_ui_element(theme, UiElement::Primary),
            )]));
            for line in desc.lines() {
                lines.push(Line::from(vec![Span::styled(
                    line,
                    style_for_ui_element(theme, UiElement::Text),
                )]));
            }
            lines.push(Line::raw(""));
        }

        // Tags
        if !task.tags.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "Tags:",
                style_for_ui_element(theme, UiElement::Primary),
            )]));
            let tag_spans: Vec<Span> = task
                .tags
                .iter()
                .map(|t| {
                    Span::styled(
                        format!(" #{} ", t),
                        style_for_ui_element(theme, UiElement::Secondary),
                    )
                })
                .collect();
            lines.push(Line::from(tag_spans));
            lines.push(Line::raw(""));
        }

        // Timestamps
        lines.push(Line::from(vec![
            Span::styled("Created: ", style_for_ui_element(theme, UiElement::MutedText)),
            Span::styled(
                task.created_at.format("%Y-%m-%d %H:%M").to_string(),
                style_for_ui_element(theme, UiElement::MutedText),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Updated: ", style_for_ui_element(theme, UiElement::MutedText)),
            Span::styled(
                task.updated_at.format("%Y-%m-%d %H:%M").to_string(),
                style_for_ui_element(theme, UiElement::MutedText),
            ),
        ]));

        let text = Paragraph::new(lines);
        text.render(inner, buf);
    } else {
        let empty = Paragraph::new("Select a task to view details")
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::MutedText));
        empty.render(inner, buf);
    }
}

/// Render the board/Kanban view
fn render_board_view(
    state: &PluginState,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let border_style = if focused {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Border)
    };

    let block = Block::default()
        .title(" Board View ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    // Split into columns for each status
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(inner);

    let statuses = [TaskStatus::Todo, TaskStatus::InProgress, TaskStatus::Review, TaskStatus::Done];
    let titles = [" Todo ", " In Progress ", " Review ", " Done "];

    for (idx, (status, title)) in statuses.iter().zip(titles.iter()).enumerate() {
        let col_area = columns[idx];

        // Column block
        let col_block = Block::default()
            .title(*title)
            .borders(Borders::ALL)
            .border_style(style_for_ui_element(theme, UiElement::Border));

        let col_inner = col_block.inner(col_area);
        col_block.render(col_area, buf);

        // Get tasks for this status
        let tasks: Vec<&Task> = state
            .filtered_tasks()
            .into_iter()
            .filter(|t| t.status == *status)
            .collect();

        if tasks.is_empty() {
            let empty = Paragraph::new("-");
            empty.render(col_inner, buf);
        } else {
            let mut lines: Vec<Line> = Vec::new();
            for task in tasks {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{} ", task.priority.icon()),
                        style_for_priority(theme, task.priority),
                    ),
                    Span::styled(
                        if task.title.len() > 20 {
                            format!("{}...", &task.title[..17])
                        } else {
                            task.title.clone()
                        },
                        style_for_ui_element(theme, UiElement::Text),
                    ),
                ]));
            }

            let text = Paragraph::new(lines);
            text.render(col_inner, buf);
        }
    }
}

/// Render the activity log
pub fn render_activity_log(
    state: &PluginState,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let block = Block::default()
        .title(" Activity Log ")
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Border));

    let inner = block.inner(area);
    block.render(area, buf);

    if state.activity_log.is_empty() {
        let empty = Paragraph::new("No recent activity")
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::MutedText));
        empty.render(inner, buf);
        return;
    }

    // Build table rows
    let header = Row::new(vec!["Time", "Type", "Description"])
        .style(style_for_ui_element(theme, UiElement::Primary).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = state
        .activity_log
        .iter()
        .take(20) // Show last 20 entries
        .map(|entry| {
            let time_str = entry.timestamp.format("%H:%M").to_string();
            Row::new(vec![
                Cell::from(time_str),
                Cell::from(entry.activity_type.clone()),
                Cell::from(entry.description.clone()),
            ])
            .style(style_for_ui_element(theme, UiElement::Text))
        })
        .collect();

    let widths = [
        Constraint::Length(8),
        Constraint::Length(12),
        Constraint::Min(20),
    ];

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::NONE)
            .padding(Padding::horizontal(1)),
    );

    table.render(inner, buf);
}

/// Render the header showing current focused task
pub fn render_focused_task_header(
    focused_task: Option<&Task>,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let text = if let Some(task) = focused_task {
        Line::from(vec![
            Span::styled(
                "Focused: ",
                style_for_ui_element(theme, UiElement::Primary),
            ),
            Span::styled(
                format!("{} {} [{}]", task.status.icon(), task.title, task.priority.as_str()),
                style_for_ui_element(theme, UiElement::Text),
            ),
        ])
    } else {
        Line::from(vec![Span::styled(
            "No focused task",
            style_for_ui_element(theme, UiElement::MutedText),
        )])
    };

    let paragraph = Paragraph::new(text);
    paragraph.render(area, buf);
}

/// Render the not available message when TD is not installed
pub fn render_not_available(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let block = Block::default()
        .title(" TD Monitor ")
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Border));

    let inner = block.inner(area);
    block.render(area, buf);

    let lines = vec![
        Line::raw(""),
        Line::from(vec![Span::styled(
            "TD (Task Driver) is not available",
            style_for_ui_element(theme, UiElement::Warning).add_modifier(Modifier::BOLD),
        )]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "TD is a task management system for the terminal.",
            style_for_ui_element(theme, UiElement::Text),
        )]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "Install TD to use this plugin:",
            style_for_ui_element(theme, UiElement::MutedText),
        )]),
        Line::from(vec![Span::styled(
            "  cargo install td-cli",
            style_for_ui_element(theme, UiElement::Secondary),
        )]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "Or visit: https://github.com/yourusername/td",
            style_for_ui_element(theme, UiElement::MutedText),
        )]),
    ];

    let text = Paragraph::new(lines).alignment(Alignment::Center);
    text.render(inner, buf);
}

/// Get style for a task status
fn style_for_task_status(theme: &Theme, status: TaskStatus) -> Style {
    match status {
        TaskStatus::Todo => style_for_ui_element(theme, UiElement::MutedText),
        TaskStatus::InProgress => style_for_ui_element(theme, UiElement::Info),
        TaskStatus::Review => style_for_ui_element(theme, UiElement::Warning),
        TaskStatus::Done => style_for_ui_element(theme, UiElement::Success),
    }
}

/// Get style for a priority level
fn style_for_priority(theme: &Theme, priority: Priority) -> Style {
    match priority {
        Priority::Low => style_for_ui_element(theme, UiElement::MutedText),
        Priority::Medium => style_for_ui_element(theme, UiElement::Text),
        Priority::High => style_for_ui_element(theme, UiElement::Warning),
        Priority::Critical => style_for_ui_element(theme, UiElement::Error),
    }
}

/// Calculate scroll offset to keep selection visible
fn calculate_scroll_offset(
    _lines: &[Line],
    visible_height: usize,
    selected: Option<usize>,
) -> usize {
    if let Some(selected_idx) = selected {
        if selected_idx >= visible_height {
            selected_idx.saturating_sub(visible_height / 2)
        } else {
            0
        }
    } else {
        0
    }
}

/// Render status info for the footer
pub fn render_status_info(state: &PluginState, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    // Task counts by status
    let todo_count = state.tasks_with_status(TaskStatus::Todo).len();
    let in_progress_count = state.tasks_with_status(TaskStatus::InProgress).len();
    let review_count = state.tasks_with_status(TaskStatus::Review).len();
    let done_count = state.tasks_with_status(TaskStatus::Done).len();
    let total = state.tasks.len();

    spans.push(Span::styled(
        format!("Tasks: {} ", total),
        style_for_ui_element(theme, UiElement::Primary),
    ));

    if todo_count > 0 {
        spans.push(Span::styled(
            format!("○{} ", todo_count),
            style_for_ui_element(theme, UiElement::MutedText),
        ));
    }
    if in_progress_count > 0 {
        spans.push(Span::styled(
            format!("◐{} ", in_progress_count),
            style_for_ui_element(theme, UiElement::Info),
        ));
    }
    if review_count > 0 {
        spans.push(Span::styled(
            format!("◑{} ", review_count),
            style_for_ui_element(theme, UiElement::Warning),
        ));
    }
    if done_count > 0 {
        spans.push(Span::styled(
            format!("●{} ", done_count),
            style_for_ui_element(theme, UiElement::Success),
        ));
    }

    // Filter indicator
    if let Some(ref filter) = state.filter {
        spans.push(Span::styled(
            format!("[filter: {}]", filter),
            style_for_ui_element(theme, UiElement::Secondary),
        ));
    }

    // Focused task
    if let Some(ref task) = state.current_focus {
        spans.push(Span::raw(" | "));
        spans.push(Span::styled(
            format!("Focus: {}", task.title),
            style_for_ui_element(theme, UiElement::Highlight),
        ));
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn test_build_task_line() {
        let theme = test_theme();
        let task = Task::new("1", "Test Task");
        let line = build_task_line(&task, false, &theme);
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn test_style_for_task_status() {
        let theme = test_theme();
        let todo_style = style_for_task_status(&theme, TaskStatus::Todo);
        let done_style = style_for_task_status(&theme, TaskStatus::Done);
        // Just verify they don't panic
        let _ = format!("{:?} {:?}", todo_style, done_style);
    }

    #[test]
    fn test_render_status_info() {
        let theme = test_theme();
        let mut state = PluginState::new();
        state.tasks = vec![
            Task::new("1", "Task 1"),
            Task::new("2", "Task 2"),
        ];
        let spans = render_status_info(&state, &theme);
        assert!(!spans.is_empty());
    }
}
