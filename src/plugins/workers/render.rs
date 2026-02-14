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

use super::state::{FocusPane, ModalState, PluginState, PreviewTab};

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

/// Render the main workers view
pub fn render_workers(
    state: &PluginState,
    focus_pane: FocusPane,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
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
        let text = Paragraph::new("No intents yet.\n\nPress 'n' to create one.")
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
                "  No workers yet. Press 'r' to run.",
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
        let text = Paragraph::new("Select an intent to view details")
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
        lines.push(Line::from(Span::styled(
            "No output yet. Run workers to see output here.",
            Style::default().fg(theme_comment(theme)),
        )));
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
        let text = Paragraph::new("Select an intent to view criteria")
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme_comment(theme)));
        text.render(area, buf);
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
}
