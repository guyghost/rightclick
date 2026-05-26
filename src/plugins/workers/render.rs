//! Workers Plugin Rendering
//!
//! This module provides the UI rendering for the Workers plugin,
//! displaying intents, workers, and their outputs.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Tabs, Widget, Wrap},
};

use crate::core::models::Theme;

use super::state::{FocusPane, ModalState, PluginState, PreviewTab, ViewMode};

// Helper functions to get colors from theme
fn theme_fg(theme: &Theme) -> Color {
    theme.colors.foreground.parse().unwrap_or(Color::White)
}

fn theme_primary(theme: &Theme) -> Color {
    theme.colors.primary.parse().unwrap_or(Color::Cyan)
}

fn theme_secondary(theme: &Theme) -> Color {
    theme.colors.secondary.parse().unwrap_or(Color::Magenta)
}

fn theme_comment(theme: &Theme) -> Color {
    theme.colors.muted.parse().unwrap_or(Color::Gray)
}

fn theme_border(theme: &Theme) -> Color {
    theme.colors.border.parse().unwrap_or(Color::DarkGray)
}

fn theme_selection_bg(theme: &Theme) -> Color {
    theme.colors.highlight.parse().unwrap_or(Color::DarkGray)
}

fn theme_selection_fg(theme: &Theme) -> Color {
    theme.colors.foreground.parse().unwrap_or(Color::White)
}

fn theme_error(theme: &Theme) -> Color {
    theme.colors.error.parse().unwrap_or(Color::Red)
}

fn theme_info(theme: &Theme) -> Color {
    theme.colors.info.parse().unwrap_or(Color::LightCyan)
}

fn theme_success(theme: &Theme) -> Color {
    theme.colors.success.parse().unwrap_or(Color::Green)
}

#[allow(dead_code)]
fn theme_warning(theme: &Theme) -> Color {
    theme.colors.warning.parse().unwrap_or(Color::Yellow)
}

/// Render the main workers view
pub fn render_workers(
    state: &PluginState,
    focus_pane: FocusPane,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    if state.view_mode == ViewMode::Kanban {
        render_kanban(state, area, buf, theme);

        // Render modal if open
        if state.is_modal_open() {
            render_modal(state, area, buf, theme);
        }
        return;
    }

    // Main layout: sidebar | preview
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Render sidebar
    render_sidebar(state, focus_pane, focused, main_chunks[0], buf, theme);

    // Render preview
    render_preview(state, focus_pane, focused, main_chunks[1], buf, theme);

    // Render modal if open
    if state.is_modal_open() {
        render_modal(state, area, buf, theme);
    }
}

/// Render the sidebar (intent list)
fn render_sidebar(
    state: &PluginState,
    focus_pane: FocusPane,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let border_style = if focus_pane == FocusPane::Sidebar && focused {
        Style::default()
            .fg(theme_primary(theme))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme_border(theme))
    };

    let block = Block::default()
        .title(" 📋 Intents ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    if state.intents.is_empty() {
        let text = Paragraph::new(empty_intents_message(state))
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme_comment(theme)));
        text.render(inner, buf);
        return;
    }

    // Render intent list
    let mut lines = Vec::new();

    for (idx, entry) in state.intents.iter().enumerate() {
        let is_selected = state.selected_intent == Some(idx);
        let style = if is_selected {
            Style::default()
                .bg(theme_selection_bg(theme))
                .fg(theme_selection_fg(theme))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme_fg(theme))
        };

        // Status icon + title
        let status_icon = entry.status_icon();
        let completion = entry.completion_percentage();
        let progress_bar = render_progress_bar(completion, 10);

        let title_line = Line::from(vec![
            Span::styled(format!("{} ", status_icon), style),
            Span::styled(entry.intent.title.clone(), style),
        ]);
        lines.push(title_line);

        // Progress line
        let progress_style = if is_selected {
            style
        } else {
            Style::default().fg(theme_comment(theme))
        };
        let progress_line = Line::from(vec![
            Span::styled("   ", progress_style),
            Span::styled(progress_bar, progress_style),
            Span::styled(format!(" {}%", completion), progress_style),
        ]);
        lines.push(progress_line);

        // Show workers if expanded or selected
        if entry.expanded || is_selected {
            let workers = state.get_intent_workers(&entry.intent.id);
            for worker_entry in workers {
                let worker_style = if is_selected {
                    style
                } else {
                    Style::default().fg(theme_comment(theme))
                };

                let worker_line = Line::from(vec![
                    Span::styled("   ├─ ", worker_style),
                    Span::styled(worker_entry.type_icon().to_string(), worker_style),
                    Span::styled(" ", worker_style),
                    Span::styled(worker_entry.status_icon().to_string(), worker_style),
                    Span::styled(
                        format!(" {:?}", worker_entry.worker.worker_type),
                        worker_style,
                    ),
                ]);
                lines.push(worker_line);
            }
        }

        // Empty line between intents
        lines.push(Line::from(""));
    }

    let content = Paragraph::new(lines).wrap(Wrap { trim: true });

    content.render(inner, buf);
}

/// Render the preview pane
fn render_preview(
    state: &PluginState,
    focus_pane: FocusPane,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let border_style = if focus_pane == FocusPane::Preview && focused {
        Style::default()
            .fg(theme_primary(theme))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme_border(theme))
    };

    // Tab titles
    let tab_titles = vec![" Spec ", " Output ", " Criteria "];
    let selected_tab = match state.preview_tab {
        PreviewTab::Spec => 0,
        PreviewTab::Output => 1,
        PreviewTab::Criteria => 2,
    };

    // Render tabs
    let tabs = Tabs::new(tab_titles)
        .select(selected_tab)
        .style(Style::default().fg(theme_fg(theme)))
        .highlight_style(
            Style::default()
                .fg(theme_primary(theme))
                .add_modifier(Modifier::BOLD),
        )
        .divider("|");

    let tabs_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    tabs.render(tabs_area, buf);

    // Content area
    let content_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height - 1,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(content_area);
    block.render(content_area, buf);

    // Render content based on selected tab
    match state.preview_tab {
        PreviewTab::Spec => render_spec_preview(state, inner, buf, theme),
        PreviewTab::Output => render_output_preview(state, inner, buf, theme),
        PreviewTab::Criteria => render_criteria_preview(state, inner, buf, theme),
    }
}

/// Render the spec preview
fn render_spec_preview(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    if let Some(entry) = state.selected_intent() {
        let intent = &entry.intent;

        let mut lines = Vec::new();

        // Title
        lines.push(Line::from(vec![Span::styled(
            intent.title.clone(),
            Style::default()
                .fg(theme_primary(theme))
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        )]));
        lines.push(Line::from(""));

        // Status
        lines.push(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(theme_comment(theme))),
            Span::styled(
                format!("{} {:?}", intent.status.icon(), intent.status),
                Style::default().fg(theme_fg(theme)),
            ),
        ]));
        lines.push(Line::from(""));

        // Description
        if !intent.description.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "Description",
                Style::default()
                    .fg(theme_secondary(theme))
                    .add_modifier(Modifier::BOLD),
            )]));
            for line in intent.description.lines() {
                lines.push(Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(theme_fg(theme)),
                )));
            }
            lines.push(Line::from(""));
        }

        // Workers
        lines.push(Line::from(vec![Span::styled(
            "Workers",
            Style::default()
                .fg(theme_secondary(theme))
                .add_modifier(Modifier::BOLD),
        )]));

        let workers = state.get_intent_workers(&intent.id);
        if workers.is_empty() {
            lines.push(Line::from(Span::styled(
                no_workers_for_intent_message(),
                Style::default().fg(theme_comment(theme)),
            )));
        } else {
            for worker_entry in workers {
                let w = &worker_entry.worker;
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {} ", w.type_icon()),
                        Style::default().fg(theme_fg(theme)),
                    ),
                    Span::styled(
                        format!("{} ", w.status_icon()),
                        Style::default().fg(theme_fg(theme)),
                    ),
                    Span::styled(
                        format!("{:?}", w.worker_type),
                        Style::default().fg(theme_fg(theme)),
                    ),
                    Span::styled(
                        format!(" ({})", w.agent),
                        Style::default().fg(theme_comment(theme)),
                    ),
                ]));
            }
        }

        let content = Paragraph::new(lines).wrap(Wrap { trim: true });

        content.render(area, buf);
    } else {
        let text = Paragraph::new(select_intent_details_message())
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme_comment(theme)));
        text.render(area, buf);
    }
}

/// Render the output preview
fn render_output_preview(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let mut lines = Vec::new();

    // Show output from selected worker or all workers
    if let Some(worker_id) = &state.selected_worker {
        if let Some(entry) = state.get_worker(worker_id) {
            lines.push(Line::from(vec![Span::styled(
                format!("Output for {:?} worker", entry.worker.worker_type),
                Style::default()
                    .fg(theme_secondary(theme))
                    .add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(""));

            for line in &entry.output_lines {
                lines.push(Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(theme_fg(theme)),
                )));
            }

            if entry.streaming {
                lines.push(Line::from(Span::styled(
                    "...",
                    Style::default().fg(theme_comment(theme)),
                )));
            }
        }
    } else if let Some(entry) = state.selected_intent() {
        // Show output from all workers for this intent
        let workers = state.get_intent_workers(&entry.intent.id);

        for worker_entry in workers {
            lines.push(Line::from(vec![Span::styled(
                format!("=== {:?} ===", worker_entry.worker.worker_type),
                Style::default()
                    .fg(theme_secondary(theme))
                    .add_modifier(Modifier::BOLD),
            )]));

            for line in worker_entry.last_output(20) {
                lines.push(Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(theme_fg(theme)),
                )));
            }

            lines.push(Line::from(""));
        }
    }

    if lines.is_empty() {
        lines.extend(output_empty_message(state).lines().map(|line| {
            Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(theme_comment(theme)),
            ))
        }));
    }

    let content = Paragraph::new(lines).wrap(Wrap { trim: true });

    content.render(area, buf);
}

/// Render the criteria preview
fn render_criteria_preview(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    if let Some(entry) = state.selected_intent() {
        let intent = &entry.intent;
        let mut lines = Vec::new();

        lines.push(Line::from(vec![Span::styled(
            "Acceptance Criteria",
            Style::default()
                .fg(theme_secondary(theme))
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));

        if intent.acceptance_criteria.is_empty() {
            lines.push(Line::from(Span::styled(
                "No criteria defined.",
                Style::default().fg(theme_comment(theme)),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Edit the intent spec to add acceptance criteria.",
                Style::default().fg(theme_comment(theme)),
            )));
            lines.push(Line::from(Span::styled(
                "/  Search commands",
                Style::default().fg(theme_comment(theme)),
            )));
            lines.push(Line::from(Span::styled(
                "?  Help",
                Style::default().fg(theme_comment(theme)),
            )));
        } else {
            for (idx, criterion) in intent.acceptance_criteria.iter().enumerate() {
                let icon = if criterion.completed { "✅" } else { "⬜" };
                let style = if criterion.completed {
                    Style::default().fg(theme_comment(theme))
                } else {
                    Style::default().fg(theme_fg(theme))
                };

                lines.push(Line::from(vec![
                    Span::styled(format!("{} ", icon), style),
                    Span::styled(format!("{}.", idx + 1), style),
                    Span::styled(criterion.description.clone(), style),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                format!(
                    "Progress: {}/{} ({}%)",
                    intent
                        .acceptance_criteria
                        .iter()
                        .filter(|c| c.completed)
                        .count(),
                    intent.acceptance_criteria.len(),
                    entry.completion_percentage()
                ),
                Style::default().fg(theme_primary(theme)),
            )]));
        }

        let content = Paragraph::new(lines).wrap(Wrap { trim: true });

        content.render(area, buf);
    } else {
        let message = if state.intents.is_empty() {
            empty_criteria_message()
        } else {
            select_intent_criteria_message()
        };
        let text = Paragraph::new(message)
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme_comment(theme)));
        text.render(area, buf);
    }
}

fn empty_intents_message(state: &PluginState) -> String {
    format!(
        "No intents yet\n\nn  New intent\nf  Refresh intents\n/  Search commands and intents\n?  Help\n\nSpecs: {}",
        state.intents_dir.display()
    )
}

fn no_workers_for_intent_message() -> &'static str {
    "  No workers yet\n  r  Run workers\n  f  Refresh intents\n  /  Search commands\n  ?  Help"
}

fn output_empty_message(state: &PluginState) -> &'static str {
    if state.intents.is_empty() {
        "No output yet\n\nn  New intent\nf  Refresh intents\n/  Search commands\n?  Help"
    } else if state.selected_intent().is_none() {
        "No output selected\n\nj/k  Navigate intents\nEnter/o  Open intent\n/  Search intents\n?  Help"
    } else {
        "No output yet\n\nr  Run workers\nf  Refresh intents\n/  Search commands\n?  Help"
    }
}

fn select_intent_details_message() -> &'static str {
    "Select an intent to view details\n\nj/k  Navigate intents\nEnter/o  Open intent\n/  Search intents\n?  Help"
}

fn select_intent_criteria_message() -> &'static str {
    "Select an intent to view criteria\n\nj/k  Navigate intents\nEnter/o  Open intent\n/  Search intents\n?  Help"
}

fn empty_criteria_message() -> &'static str {
    "No criteria yet\n\nn  New intent\nf  Refresh intents\n/  Search commands and intents\n?  Help"
}

/// Render the kanban board view showing workers grouped by status
pub fn render_kanban(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let groups = state.workers_by_status();
    let kanban = &state.kanban_state;

    // Column definitions: (title, color, worker_ids)
    let columns: Vec<(&str, Color, &Vec<String>)> = vec![
        ("Pending", theme_info(theme), &groups.pending),
        ("Running", theme_primary(theme), &groups.running),
        ("Completed", theme_success(theme), &groups.completed),
        ("Failed", theme_error(theme), &groups.failed),
    ];

    // Split area into 4 equal columns
    let col_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    for (col_idx, (title, color, worker_ids)) in columns.iter().enumerate() {
        let is_focused_col = kanban.focused_column == col_idx;

        // Column border style: highlight if this column is focused
        let border_style = if is_focused_col {
            Style::default().fg(*color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme_border(theme))
        };

        let header = format!(" {} ({}) ", title, worker_ids.len());
        let block = Block::default()
            .title(header)
            .title_style(Style::default().fg(*color).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(col_chunks[col_idx]);
        block.render(col_chunks[col_idx], buf);

        if worker_ids.is_empty() {
            let empty_text = Paragraph::new(kanban_empty_column_message(state, title))
                .alignment(Alignment::Center)
                .style(Style::default().fg(theme_comment(theme)))
                .wrap(Wrap { trim: true });
            empty_text.render(inner, buf);
            continue;
        }

        // Render worker cards as lines within the column
        let mut lines: Vec<Line> = Vec::new();

        for (row_idx, worker_id) in worker_ids.iter().enumerate() {
            if let Some(entry) = state.get_worker(worker_id) {
                let is_focused_card = is_focused_col && kanban.focused_row == row_idx;

                let card_style = if is_focused_card {
                    Style::default()
                        .bg(theme_selection_bg(theme))
                        .fg(theme_selection_fg(theme))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme_fg(theme))
                };

                let muted_style = if is_focused_card {
                    card_style
                } else {
                    Style::default().fg(theme_comment(theme))
                };

                // Line 1: type icon + worker name
                let type_icon = entry.type_icon();
                let worker_name = format!("{:?}", entry.worker.worker_type);
                lines.push(Line::from(vec![
                    Span::styled(format!("{} ", type_icon), card_style),
                    Span::styled(worker_name, card_style),
                ]));

                // Line 2: intent ID (truncated)
                let intent_label = if entry.worker.intent_id.len() > 20 {
                    format!("  {}..", &entry.worker.intent_id[..18])
                } else {
                    format!("  {}", entry.worker.intent_id)
                };
                lines.push(Line::from(Span::styled(intent_label, muted_style)));

                // Line 3: elapsed time / status info
                let time_info = if let Some(ref completed_at) = entry.worker.completed_at {
                    format!("  done: {}", &completed_at[..10.min(completed_at.len())])
                } else if entry.streaming {
                    "  streaming...".to_string()
                } else {
                    format!(
                        "  since: {}",
                        &entry.worker.created_at[..10.min(entry.worker.created_at.len())]
                    )
                };
                lines.push(Line::from(Span::styled(time_info, muted_style)));

                // Separator between cards
                lines.push(Line::from(Span::styled(
                    "────────────────",
                    Style::default().fg(theme_border(theme)),
                )));
            }
        }

        let content = Paragraph::new(lines).wrap(Wrap { trim: true });
        content.render(inner, buf);
    }
}

fn kanban_empty_column_message(state: &PluginState, title: &str) -> String {
    let status = title.to_lowercase();

    if state.workers.is_empty() {
        if state.selected_intent().is_some() {
            format!("No {status} workers\n\nr  Run workers\nv  Switch view")
        } else {
            format!("No {status} workers\n\nn  New intent\nv  Switch view")
        }
    } else {
        format!("No {status} workers\n\nh/l  Move columns\nv  Switch view")
    }
}

/// Render modal dialogs
fn render_modal(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    match state.modal_state {
        ModalState::CreateIntent => {
            render_create_intent_modal(state, area, buf, theme);
        }
        ModalState::DeleteConfirm => {
            render_delete_confirm_modal(state, area, buf, theme);
        }
        _ => {}
    }
}

/// Render create intent modal
fn render_create_intent_modal(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let popup_area = centered_rect(60, 20, area);

    // Clear background
    Clear.render(popup_area, buf);

    let block = Block::default()
        .title(" Create New Intent ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme_primary(theme)));

    let inner = block.inner(popup_area);
    block.render(popup_area, buf);

    let text = vec![
        Line::from("Enter intent title:"),
        Line::from(""),
        Line::from(Span::styled(
            &state.new_intent_title,
            Style::default()
                .fg(theme_fg(theme))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Enter: Confirm | Esc: Cancel",
            Style::default().fg(theme_comment(theme)),
        )),
    ];

    let paragraph = Paragraph::new(text);
    paragraph.render(inner, buf);
}

/// Render delete confirmation modal
fn render_delete_confirm_modal(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let popup_area = centered_rect(60, 20, area);

    Clear.render(popup_area, buf);

    let title = if let Some(entry) = state.selected_intent() {
        format!(" Delete '{}' ? ", entry.intent.title)
    } else {
        " Delete Intent ? ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme_error(theme)));

    let inner = block.inner(popup_area);
    block.render(popup_area, buf);

    let text = vec![
        Line::from("This action cannot be undone."),
        Line::from(""),
        Line::from(Span::styled(
            "y: Confirm | n: Cancel",
            Style::default().fg(theme_comment(theme)),
        )),
    ];

    let paragraph = Paragraph::new(text);
    paragraph.render(inner, buf);
}

/// Render a progress bar
fn render_progress_bar(percentage: u8, width: usize) -> String {
    let filled = (percentage as usize * width) / 100;
    let empty = width - filled;

    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

/// Render status line for the footer
pub fn render_workers_status<'a>(state: &'a PluginState, theme: &'a Theme) -> Vec<Span<'a>> {
    let mut spans = Vec::new();

    // Intent count
    spans.push(Span::styled(
        format!("📋 {} intents", state.intents.len()),
        Style::default().fg(theme_fg(theme)),
    ));

    spans.push(Span::raw(" | "));

    // Worker counts
    let running = state.running_workers_count();
    let completed = state.completed_workers_count();

    if running > 0 {
        spans.push(Span::styled(
            format!("🔄 {} running", running),
            Style::default().fg(theme_primary(theme)),
        ));
    } else {
        spans.push(Span::styled(
            format!("✅ {} completed", completed),
            Style::default().fg(theme_comment(theme)),
        ));
    }

    spans
}

/// Create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_progress_bar() {
        assert_eq!(render_progress_bar(0, 10), "[░░░░░░░░░░]");
        assert_eq!(render_progress_bar(50, 10), "[█████░░░░░]");
        assert_eq!(render_progress_bar(100, 10), "[██████████]");
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 100);
        let centered = centered_rect(60, 20, area);

        assert_eq!(centered.width, 60);
        assert_eq!(centered.height, 20);
    }

    #[test]
    fn test_render_kanban_empty_state() {
        use std::path::PathBuf;

        let state = PluginState::new(PathBuf::from("intents"), PathBuf::from("logs"));
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 40);
        let mut buf = Buffer::empty(area);

        render_kanban(&state, area, &mut buf, &theme);

        // Buffer should be non-empty (at least borders and column headers rendered)
        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(!content.is_empty());
        // Should contain column headers
        assert!(content.contains("Pending"));
        assert!(content.contains("Running"));
        assert!(content.contains("Completed"));
        assert!(content.contains("Failed"));
        assert!(content.contains("No pending workers"));
        assert!(content.contains("No running workers"));
        assert!(content.contains("n  New intent"));
        assert!(content.contains("v  Switch view"));
        assert!(!content.contains("No workers"));
    }

    #[test]
    fn test_empty_intents_message_points_to_next_actions() {
        use std::path::PathBuf;

        let state = PluginState::new(PathBuf::from(".rightclick/intents"), PathBuf::from("logs"));
        let message = empty_intents_message(&state);

        assert!(message.contains("No intents yet"));
        assert!(message.contains("n  New intent"));
        assert!(message.contains("f  Refresh intents"));
        assert!(message.contains("/  Search commands and intents"));
        assert!(message.contains("?  Help"));
        assert!(message.contains(".rightclick/intents"));
    }

    #[test]
    fn test_render_workers_empty_state_includes_next_actions() {
        use std::path::PathBuf;

        let state = PluginState::new(PathBuf::from(".rightclick/intents"), PathBuf::from("logs"));
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workers(&state, FocusPane::Sidebar, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No intents yet"));
        assert!(content.contains("New intent"));
        assert!(content.contains("Refresh intents"));
        assert!(content.contains("Search commands"));
    }

    #[test]
    fn test_render_workers_selected_intent_without_workers_includes_next_actions() {
        use crate::core::models::intent::Intent;
        use std::path::PathBuf;

        let mut state =
            PluginState::new(PathBuf::from(".rightclick/intents"), PathBuf::from("logs"));
        state.add_intent(Intent::new(
            "Improve worker UX",
            PathBuf::from(".rightclick/intents/worker-ux.md"),
            "2026-02-14T10:00:00Z",
        ));
        state.selected_intent = Some(0);

        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workers(&state, FocusPane::Sidebar, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No workers yet"));
        assert!(content.contains("r  Run workers"));
        assert!(content.contains("f  Refresh intents"));
        assert!(content.contains("/  Search commands"));
        assert!(content.contains("?  Help"));
    }

    #[test]
    fn test_render_workers_without_selection_includes_preview_next_actions() {
        use crate::core::models::intent::Intent;
        use std::path::PathBuf;

        let mut state =
            PluginState::new(PathBuf::from(".rightclick/intents"), PathBuf::from("logs"));
        state.add_intent(Intent::new(
            "Improve worker navigation",
            PathBuf::from(".rightclick/intents/worker-navigation.md"),
            "2026-02-14T10:00:00Z",
        ));
        state.selected_intent = None;

        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workers(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Select an intent to view details"));
        assert!(content.contains("j/k  Navigate intents"));
        assert!(content.contains("Enter/o  Open intent"));
        assert!(content.contains("/  Search intents"));
        assert!(content.contains("?  Help"));
    }

    #[test]
    fn test_render_output_without_intents_points_to_creation() {
        use std::path::PathBuf;

        let mut state =
            PluginState::new(PathBuf::from(".rightclick/intents"), PathBuf::from("logs"));
        state.preview_tab = PreviewTab::Output;
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workers(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No output yet"));
        assert!(content.contains("n  New intent"));
        assert!(content.contains("f  Refresh intents"));
        assert!(content.contains("/  Search commands"));
        assert!(content.contains("?  Help"));
    }

    #[test]
    fn test_render_criteria_without_intents_points_to_creation() {
        use std::path::PathBuf;

        let mut state =
            PluginState::new(PathBuf::from(".rightclick/intents"), PathBuf::from("logs"));
        state.preview_tab = PreviewTab::Criteria;
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workers(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No criteria yet"));
        assert!(content.contains("n  New intent"));
        assert!(content.contains("f  Refresh intents"));
        assert!(content.contains("/  Search commands and intents"));
        assert!(content.contains("?  Help"));
    }

    #[test]
    fn test_render_selected_criteria_without_items_points_to_spec_editing() {
        use crate::core::models::intent::Intent;
        use std::path::PathBuf;

        let mut state =
            PluginState::new(PathBuf::from(".rightclick/intents"), PathBuf::from("logs"));
        state.preview_tab = PreviewTab::Criteria;
        state.add_intent(Intent::new(
            "Clarify worker acceptance criteria",
            PathBuf::from(".rightclick/intents/criteria.md"),
            "2026-02-14T10:00:00Z",
        ));
        state.selected_intent = Some(0);

        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workers(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Acceptance Criteria"));
        assert!(content.contains("No criteria defined."));
        assert!(content.contains("Edit the intent spec to add acceptance criteria."));
        assert!(content.contains("/  Search commands"));
        assert!(content.contains("?  Help"));
    }

    #[test]
    fn test_render_output_without_selection_points_to_navigation() {
        use crate::core::models::intent::Intent;
        use std::path::PathBuf;

        let mut state =
            PluginState::new(PathBuf::from(".rightclick/intents"), PathBuf::from("logs"));
        state.add_intent(Intent::new(
            "Improve output UX",
            PathBuf::from(".rightclick/intents/output-ux.md"),
            "2026-02-14T10:00:00Z",
        ));
        state.selected_intent = None;
        state.preview_tab = PreviewTab::Output;
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workers(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No output selected"));
        assert!(content.contains("j/k  Navigate intents"));
        assert!(content.contains("Enter/o  Open intent"));
        assert!(content.contains("/  Search intents"));
        assert!(content.contains("?  Help"));
    }

    #[test]
    fn test_render_output_for_selected_intent_points_to_run_workers() {
        use crate::core::models::intent::Intent;
        use std::path::PathBuf;

        let mut state =
            PluginState::new(PathBuf::from(".rightclick/intents"), PathBuf::from("logs"));
        state.add_intent(Intent::new(
            "Improve output run state",
            PathBuf::from(".rightclick/intents/output-run.md"),
            "2026-02-14T10:00:00Z",
        ));
        state.selected_intent = Some(0);
        state.preview_tab = PreviewTab::Output;
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workers(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No output yet"));
        assert!(content.contains("r  Run workers"));
        assert!(content.contains("f  Refresh intents"));
        assert!(content.contains("/  Search commands"));
        assert!(content.contains("?  Help"));
    }

    #[test]
    fn test_render_kanban_with_workers() {
        use super::super::state::WorkerEntry;
        use crate::core::models::intent::{Worker, WorkerType};
        use std::path::PathBuf;

        let mut state = PluginState::new(PathBuf::from("intents"), PathBuf::from("logs"));
        let theme = Theme::default();

        // Create workers in various statuses
        let w1 = Worker::new(
            "investigate",
            WorkerType::Investigator,
            "intent-1",
            PathBuf::from("/repo/w1"),
            "branch",
            "claude",
            PathBuf::from("/repo/log1"),
            "2026-02-14T10:00:00Z",
        );
        // w1 is Pending by default

        let mut w2 = Worker::new(
            "implement",
            WorkerType::Implementer,
            "intent-1",
            PathBuf::from("/repo/w2"),
            "branch",
            "claude",
            PathBuf::from("/repo/log2"),
            "2026-02-14T10:00:00Z",
        );
        w2.mark_running();

        let mut w3 = Worker::new(
            "verify",
            WorkerType::Verifier,
            "intent-1",
            PathBuf::from("/repo/w3"),
            "branch",
            "claude",
            PathBuf::from("/repo/log3"),
            "2026-02-14T10:00:00Z",
        );
        w3.mark_completed("2026-02-14T11:00:00Z");

        state.workers.insert(w1.id.clone(), WorkerEntry::new(w1));
        state.workers.insert(w2.id.clone(), WorkerEntry::new(w2));
        state.workers.insert(w3.id.clone(), WorkerEntry::new(w3));

        let area = Rect::new(0, 0, 120, 40);
        let mut buf = Buffer::empty(area);

        render_kanban(&state, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        // Should render counts in headers
        assert!(content.contains("Pending"));
        assert!(content.contains("Running"));
        assert!(content.contains("Completed"));
        // Should contain worker type names
        assert!(content.contains("Investigator"));
        assert!(content.contains("Implementer"));
        assert!(content.contains("Verifier"));
    }
}
