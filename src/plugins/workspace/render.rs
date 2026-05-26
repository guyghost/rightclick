//! Workspace Plugin Rendering
//!
//! This module handles all rendering logic for the Workspace plugin,
//! including the worktree list, kanban view, and preview pane.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Tabs, Widget},
};

use crate::core::models::Theme;
use crate::theme::{UiElement, style_for_git_status, style_for_ui_element};

use super::state::{FocusPane, ModalState, PluginState, PreviewTab, ViewMode, Worktree};

/// Render the workspace plugin
pub fn render_workspace(
    state: &PluginState,
    focus_pane: FocusPane,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    match state.view_mode {
        ViewMode::List => {
            render_list_mode(state, focus_pane, focused, area, buf, theme);
        }
        ViewMode::Kanban => {
            render_kanban_mode(state, focus_pane, focused, area, buf, theme);
        }
        ViewMode::Interactive => {
            render_interactive_mode(state, focus_pane, focused, area, buf, theme);
        }
    }

    // Render modal if open
    match state.modal_state {
        ModalState::CreateWorktree => {
            render_create_worktree_modal(state, area, buf, theme);
        }
        ModalState::DeleteConfirm => {
            render_delete_confirm_modal(state, area, buf, theme);
        }
        ModalState::LinkTask => {
            render_link_task_modal(state, area, buf, theme);
        }
        ModalState::MergeDialog => {
            render_merge_dialog_modal(state, area, buf, theme);
        }
        ModalState::None => {}
    }
}

/// Render list mode with sidebar and preview
fn render_list_mode(
    state: &PluginState,
    focus_pane: FocusPane,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(state.sidebar_width), Constraint::Min(20)])
        .split(area);

    let sidebar_area = main_layout[0];
    let preview_area = main_layout[1];

    // Render sidebar with worktree list
    render_worktree_list(state, focus_pane, focused, sidebar_area, buf, theme);

    // Render preview pane
    render_preview_pane(state, focus_pane, focused, preview_area, buf, theme);
}

/// Render the worktree list sidebar
fn render_worktree_list(
    state: &PluginState,
    focus_pane: FocusPane,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let is_focused = focus_pane == FocusPane::Sidebar;
    let border_style = if is_focused && focused {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Border)
    };

    let block = Block::default()
        .title(" Worktrees ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    if state.worktrees.is_empty() {
        let empty_text = Paragraph::new(empty_worktrees_message())
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::MutedText));
        empty_text.render(inner, buf);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    for (idx, worktree) in state.worktrees.iter().enumerate() {
        let is_selected = state.selected == Some(idx);
        lines.push(build_worktree_line(worktree, is_selected, theme));
    }

    // Calculate scroll offset
    let available_height = inner.height as usize;
    let scroll_offset = calculate_scroll_offset(&lines, available_height, state.selected);

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

/// Render the preview pane
fn render_preview_pane(
    state: &PluginState,
    focus_pane: FocusPane,
    focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let is_focused = focus_pane == FocusPane::Preview;
    let border_style = if is_focused && focused {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Border)
    };

    // Tab titles
    let tabs = vec!["Output", "Diff", "Task"];
    let selected_tab = match state.preview_tab {
        PreviewTab::Output => 0,
        PreviewTab::Diff => 1,
        PreviewTab::Task => 2,
    };

    // Split area for tabs and content
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Render tabs
    let tabs_widget = Tabs::new(tabs)
        .select(selected_tab)
        .style(style_for_ui_element(theme, UiElement::Text))
        .highlight_style(
            style_for_ui_element(theme, UiElement::Primary).add_modifier(Modifier::BOLD),
        )
        .divider(Span::raw(" | "));

    let tabs_block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    tabs_widget.render(tabs_block.inner(layout[0]), buf);
    tabs_block.render(layout[0], buf);

    // Render content based on selected tab
    let content_area = layout[1];
    match state.preview_tab {
        PreviewTab::Output => render_output_content(state, content_area, buf, theme),
        PreviewTab::Diff => render_diff_content(state, content_area, buf, theme),
        PreviewTab::Task => render_task_content(state, content_area, buf, theme),
    }
}

/// Render output tab content
fn render_output_content(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let block = Block::default()
        .title(" Output ")
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Border));

    let inner = block.inner(area);
    block.render(area, buf);

    if state.output_text.is_empty() {
        let empty = Paragraph::new(output_empty_message(state))
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::MutedText));
        empty.render(inner, buf);
        return;
    }

    let text = Paragraph::new(state.output_text.clone())
        .style(style_for_ui_element(theme, UiElement::Text))
        .wrap(ratatui::widgets::Wrap { trim: false });

    text.render(inner, buf);
}

/// Render diff tab content
fn render_diff_content(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let block = Block::default()
        .title(" Diff ")
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Border));

    let inner = block.inner(area);
    block.render(area, buf);

    let diff_text = match &state.diff_content {
        Some(diff) => diff.clone(),
        None => diff_empty_message(state),
    };

    // Parse and colorize diff
    let lines: Vec<Line> = diff_text
        .lines()
        .map(|line| build_diff_line(line, theme))
        .collect();

    let text = Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false });
    text.render(inner, buf);
}

/// Render task tab content
fn render_task_content(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let block = Block::default()
        .title(" Task ")
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Border));

    let inner = block.inner(area);
    block.render(area, buf);

    if let Some(content) = &state.task_content {
        let text = Paragraph::new(content.clone())
            .style(style_for_ui_element(theme, UiElement::Text))
            .wrap(ratatui::widgets::Wrap { trim: false });
        text.render(inner, buf);
    } else if let Some(worktree) = state.selected_worktree() {
        if let Some(task_id) = &worktree.linked_task {
            let text = Paragraph::new(task_details_missing_message(task_id))
                .alignment(Alignment::Center)
                .style(style_for_ui_element(theme, UiElement::MutedText));
            text.render(inner, buf);
        } else {
            let empty = Paragraph::new(no_linked_task_message())
                .alignment(Alignment::Center)
                .style(style_for_ui_element(theme, UiElement::MutedText));
            empty.render(inner, buf);
        }
    } else {
        let message = if state.worktrees.is_empty() {
            create_worktree_for_task_message()
        } else {
            select_worktree_message()
        };
        let empty = Paragraph::new(message)
            .alignment(Alignment::Center)
            .style(style_for_ui_element(theme, UiElement::MutedText));
        empty.render(inner, buf);
    }
}

fn empty_worktrees_message() -> &'static str {
    "No worktrees found\n\nn  Create worktree\nr  Refresh worktrees\n/  Search commands and worktrees\n?  Help\n\nUse worktrees to run agents in parallel without blocking the main checkout."
}

fn no_linked_task_message() -> &'static str {
    "No linked task\n\nT  Link task\n/  Search commands"
}

fn task_details_missing_message(task_id: &str) -> String {
    format!(
        "Task: {}\n\nNo details loaded\n\nT  Relink task\nr  Refresh worktrees",
        task_id
    )
}

fn create_worktree_for_task_message() -> &'static str {
    "No worktree available\n\nn  Create worktree\nr  Refresh worktrees"
}

fn select_worktree_message() -> &'static str {
    "No worktree selected\n\nj/k  Navigate worktrees\nEnter/o  Open worktree\nTab  Focus sidebar"
}

fn output_empty_message(state: &PluginState) -> &'static str {
    if state.worktrees.is_empty() {
        "No output yet\n\nn  Create worktree\nr  Refresh worktrees"
    } else if state.selected_worktree().is_none() {
        "No output selected\n\nj/k  Navigate worktrees\nEnter/o  Open worktree"
    } else {
        "No output yet\n\na  Launch agent\nEnter/o  Open interactive shell\nT  Link task"
    }
}

fn diff_empty_message(state: &PluginState) -> String {
    if let Some(worktree) = state.selected_worktree() {
        if worktree.is_dirty {
            format!(
                "Diff not loaded yet for {}\n\nr  Refresh worktrees\nj/k  Navigate worktrees",
                worktree.name
            )
        } else {
            format!(
                "Working tree clean: {}\n\nj/k  Navigate worktrees\nT  Link task",
                worktree.name
            )
        }
    } else if state.worktrees.is_empty() {
        "No diff available\n\nn  Create worktree\nr  Refresh worktrees".to_string()
    } else {
        select_worktree_message().to_string()
    }
}

/// Render kanban mode
fn render_kanban_mode(
    state: &PluginState,
    _focus_pane: FocusPane,
    _focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let groups = state.worktrees_by_status();

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

    // Active column
    render_kanban_column(
        "Active",
        &groups.active,
        state,
        layout[0],
        buf,
        theme,
        &theme.colors.modified,
    );

    // Waiting column
    render_kanban_column(
        "Waiting",
        &groups.waiting,
        state,
        layout[1],
        buf,
        theme,
        &theme.colors.info,
    );

    // Done column
    render_kanban_column(
        "Done",
        &groups.done,
        state,
        layout[2],
        buf,
        theme,
        &theme.colors.added,
    );
}

/// Render a single kanban column
fn render_kanban_column(
    title: &str,
    indices: &[usize],
    state: &PluginState,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    header_color: &str,
) {
    use ratatui::style::Color;
    use std::str::FromStr;

    let color = Color::from_str(header_color).unwrap_or(ratatui::style::Color::Gray);
    let header_style = Style::default().fg(color).add_modifier(Modifier::BOLD);

    let block = Block::default()
        .title(format!(" {} ({}) ", title, indices.len()))
        .title_style(header_style)
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Border));

    let inner = block.inner(area);
    block.render(area, buf);

    let mut lines: Vec<Line> = Vec::new();

    for &idx in indices {
        if let Some(worktree) = state.worktrees.get(idx) {
            let is_selected = state.selected == Some(idx);
            lines.push(build_worktree_compact_line(worktree, is_selected, theme));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "Empty",
            style_for_ui_element(theme, UiElement::MutedText),
        )]));
    }

    let text = Paragraph::new(lines);
    text.render(inner, buf);
}

/// Render interactive mode
fn render_interactive_mode(
    state: &PluginState,
    _focus_pane: FocusPane,
    _focused: bool,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let block = Block::default()
        .title(" Interactive Mode ")
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Primary));

    let inner = block.inner(area);
    block.render(area, buf);

    let text = if let Some(worktree) = state.selected_worktree() {
        format!(
            "Entering interactive mode for worktree: {}\nPath: {}\nBranch: {}\n\nPress 'q' to return",
            worktree.name,
            worktree.path.display(),
            worktree.branch
        )
    } else {
        select_worktree_message().to_string()
    };

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(style_for_ui_element(theme, UiElement::Text));

    paragraph.render(inner, buf);
}

/// Render create worktree modal
fn render_create_worktree_modal(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let modal_area = centered_rect(60, 40, area);

    // Clear background
    Clear.render(modal_area, buf);

    let block = Block::default()
        .title(" Create Worktree ")
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Primary));

    let inner = block.inner(modal_area);
    block.render(modal_area, buf);

    let text = format!(
        "Name: {}\n\nBranch: {}\n\nPress Enter to create, Esc to cancel",
        state.new_worktree_name, state.new_worktree_branch
    );

    let paragraph = Paragraph::new(text).style(style_for_ui_element(theme, UiElement::Text));

    paragraph.render(inner, buf);
}

/// Render delete confirmation modal
fn render_delete_confirm_modal(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let modal_area = centered_rect(50, 30, area);

    Clear.render(modal_area, buf);

    let block = Block::default()
        .title(" Confirm Delete ")
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Error));

    let inner = block.inner(modal_area);
    block.render(modal_area, buf);

    let worktree_name = state
        .selected_worktree()
        .map(|w| w.name.clone())
        .unwrap_or_default();

    let text = format!(
        "Delete worktree '{}'?\n\nThis cannot be undone.\n\nPress 'y' to confirm, any other key to cancel",
        worktree_name
    );

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(style_for_ui_element(theme, UiElement::Error));

    paragraph.render(inner, buf);
}

/// Render link task modal
fn render_link_task_modal(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let modal_area = centered_rect(50, 30, area);

    Clear.render(modal_area, buf);

    let block = Block::default()
        .title(" Link Task ")
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Primary));

    let inner = block.inner(modal_area);
    block.render(modal_area, buf);

    let text = format!(
        "Task ID: {}\n\nPress Enter to link, Esc to cancel",
        state.task_id_buffer
    );

    let paragraph = Paragraph::new(text).style(style_for_ui_element(theme, UiElement::Text));

    paragraph.render(inner, buf);
}

/// Render merge dialog modal
fn render_merge_dialog_modal(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let modal_area = centered_rect(60, 50, area);

    Clear.render(modal_area, buf);

    let block = Block::default()
        .title(" Merge Workflow ")
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Primary));

    let inner = block.inner(modal_area);
    block.render(modal_area, buf);

    let worktree_name = state
        .selected_worktree()
        .map(|w| format!("{} ({})", w.name, w.branch))
        .unwrap_or_default();

    let text = format!(
        "Worktree: {}\n\nMerge options:\n\n1. git merge --no-ff <branch>\n2. git merge --squash <branch>\n3. gh pr create --fill --head <branch>\n\nCommands run from the main repository checkout.\n\nPress number to select, Esc to cancel",
        worktree_name
    );

    let paragraph = Paragraph::new(text).style(style_for_ui_element(theme, UiElement::Text));

    paragraph.render(inner, buf);
}

/// Build a line for a worktree entry
fn build_worktree_line<'a>(
    worktree: &'a Worktree,
    is_selected: bool,
    theme: &'a Theme,
) -> Line<'a> {
    let icons = worktree.status_icons();

    let name_style = if is_selected {
        style_for_ui_element(theme, UiElement::ActiveItem)
    } else if worktree.is_main {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Text)
    };

    let branch_style = if is_selected {
        style_for_ui_element(theme, UiElement::ActiveItem)
    } else {
        style_for_ui_element(theme, UiElement::MutedText)
    };

    let mut spans = vec![
        Span::styled(format!(" {} ", worktree.name), name_style),
        Span::raw(" "),
        Span::styled(format!("({})", worktree.branch), branch_style),
    ];

    if !icons.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            icons,
            style_for_ui_element(theme, UiElement::Info),
        ));
    }

    if worktree.is_main {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "[main]",
            style_for_ui_element(theme, UiElement::Primary),
        ));
    }

    Line::from(spans)
}

/// Build a compact line for kanban view
fn build_worktree_compact_line<'a>(
    worktree: &'a Worktree,
    is_selected: bool,
    theme: &'a Theme,
) -> Line<'a> {
    let name_style = if is_selected {
        style_for_ui_element(theme, UiElement::ActiveItem)
    } else {
        style_for_ui_element(theme, UiElement::Text)
    };

    let icons = worktree.status_icons();

    let mut spans = vec![Span::styled(worktree.name.clone(), name_style)];

    if !icons.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            icons,
            style_for_ui_element(theme, UiElement::Info),
        ));
    }

    Line::from(spans)
}

/// Build a diff line with appropriate coloring
fn build_diff_line<'a>(line: &'a str, theme: &'a Theme) -> Line<'a> {
    let style = if line.starts_with('+') && !line.starts_with("+++") {
        style_for_git_status(theme, "added")
    } else if line.starts_with('-') && !line.starts_with("---") {
        style_for_git_status(theme, "deleted")
    } else if line.starts_with("@@") {
        style_for_ui_element(theme, UiElement::Secondary)
    } else if line.starts_with("diff ")
        || line.starts_with("index ")
        || line.starts_with("---")
        || line.starts_with("+++")
    {
        style_for_ui_element(theme, UiElement::Primary)
    } else {
        style_for_ui_element(theme, UiElement::Text)
    };

    Line::from(vec![Span::styled(line.to_string(), style)])
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

/// Create a centered rectangle
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

/// Render status info for the footer
pub fn render_workspace_status(state: &PluginState, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    // Worktree count
    let total = state.worktrees.len();
    let dirty = state.worktrees.iter().filter(|w| w.is_dirty).count();
    let with_agents = state.worktrees.iter().filter(|w| w.agent_running).count();

    spans.push(Span::styled(
        format!("{} worktrees ", total),
        style_for_ui_element(theme, UiElement::Text),
    ));

    if dirty > 0 {
        spans.push(Span::styled(
            format!("{} dirty ", dirty),
            style_for_git_status(theme, "modified"),
        ));
    }

    if with_agents > 0 {
        spans.push(Span::styled(
            format!("{} agents ", with_agents),
            style_for_ui_element(theme, UiElement::Info),
        ));
    }

    // Current view mode
    let mode_text = match state.view_mode {
        ViewMode::List => "list",
        ViewMode::Kanban => "kanban",
        ViewMode::Interactive => "interactive",
    };

    spans.push(Span::styled(
        format!("[{}] ", mode_text),
        style_for_ui_element(theme, UiElement::MutedText),
    ));

    // Selected worktree info
    if let Some(worktree) = state.selected_worktree() {
        spans.push(Span::styled(
            format!("{}:{} ", worktree.name, worktree.branch),
            style_for_ui_element(theme, UiElement::Primary),
        ));
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_worktree_line() {
        let theme = Theme::default();
        let worktree = Worktree::new("feature", PathBuf::from("/repo/feature"), "feature-branch");
        let line = build_worktree_line(&worktree, false, &theme);
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn test_build_diff_line() {
        let theme = Theme::default();

        let added = build_diff_line("+added line", &theme);
        assert!(!added.spans.is_empty());

        let deleted = build_diff_line("-deleted line", &theme);
        assert!(!deleted.spans.is_empty());

        let context = build_diff_line(" context", &theme);
        assert!(!context.spans.is_empty());
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 100);
        let centered = centered_rect(50, 50, area);
        assert!(centered.width <= 100);
        assert!(centered.height <= 100);
    }

    #[test]
    fn test_render_workspace_status() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.worktrees.push(Worktree::new(
            "feature",
            PathBuf::from("/repo/feature"),
            "feature-branch",
        ));

        let spans = render_workspace_status(&state, &theme);
        assert!(!spans.is_empty());
    }

    #[test]
    fn test_empty_worktrees_message_points_to_next_actions() {
        let message = empty_worktrees_message();

        assert!(message.contains("No worktrees found"));
        assert!(message.contains("n  Create worktree"));
        assert!(message.contains("r  Refresh worktrees"));
        assert!(message.contains("/  Search commands and worktrees"));
        assert!(message.contains("?  Help"));
    }

    #[test]
    fn test_render_workspace_empty_state_includes_next_actions() {
        let theme = Theme::default();
        let state = PluginState::new();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Sidebar, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No worktrees found"));
        assert!(content.contains("Create worktree"));
        assert!(content.contains("Refresh worktrees"));
        assert!(content.contains("Search commands"));
    }

    #[test]
    fn test_render_output_without_worktrees_points_to_creation() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.preview_tab = PreviewTab::Output;
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No output yet"));
        assert!(content.contains("n  Create worktree"));
        assert!(content.contains("r  Refresh worktrees"));
    }

    #[test]
    fn test_render_output_without_selection_points_to_navigation() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.preview_tab = PreviewTab::Output;
        state.worktrees.push(Worktree::new(
            "feature",
            PathBuf::from("/repo/feature"),
            "feature-branch",
        ));
        state.selected = None;
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No output selected"));
        assert!(content.contains("j/k  Navigate worktrees"));
        assert!(content.contains("Enter/o  Open worktree"));
    }

    #[test]
    fn test_render_output_for_selected_worktree_points_to_actions() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.preview_tab = PreviewTab::Output;
        state.worktrees.push(Worktree::new(
            "feature",
            PathBuf::from("/repo/feature"),
            "feature-branch",
        ));
        state.selected = Some(0);
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No output yet"));
        assert!(content.contains("a  Launch agent"));
        assert!(content.contains("Enter/o  Open interactive shell"));
        assert!(content.contains("T  Link task"));
    }

    #[test]
    fn test_render_diff_without_worktrees_points_to_creation() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.preview_tab = PreviewTab::Diff;
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No diff available"));
        assert!(content.contains("n  Create worktree"));
        assert!(content.contains("r  Refresh worktrees"));
    }

    #[test]
    fn test_render_diff_for_dirty_worktree_points_to_refresh() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.preview_tab = PreviewTab::Diff;
        let mut worktree =
            Worktree::new("feature", PathBuf::from("/repo/feature"), "feature-branch");
        worktree.is_dirty = true;
        state.worktrees.push(worktree);
        state.selected = Some(0);
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Diff not loaded yet for feature"));
        assert!(content.contains("r  Refresh worktrees"));
        assert!(content.contains("j/k  Navigate worktrees"));
    }

    #[test]
    fn test_render_clean_diff_points_to_navigation_actions() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.preview_tab = PreviewTab::Diff;
        state.worktrees.push(Worktree::new(
            "feature",
            PathBuf::from("/repo/feature"),
            "feature-branch",
        ));
        state.selected = Some(0);
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Working tree clean: feature"));
        assert!(content.contains("j/k  Navigate worktrees"));
        assert!(content.contains("T  Link task"));
    }

    #[test]
    fn test_render_task_tab_uses_actual_link_shortcut() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.preview_tab = PreviewTab::Task;
        state.worktrees.push(Worktree::new(
            "feature",
            PathBuf::from("/repo/feature"),
            "feature-branch",
        ));
        state.selected = Some(0);
        let area = Rect::new(0, 0, 80, 12);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No linked task"));
        assert!(content.contains("T  Link task"));
        assert!(content.contains("/  Search commands"));
        assert!(!content.contains("t  Link task"));
    }

    #[test]
    fn test_render_task_tab_without_worktrees_points_to_creation() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.preview_tab = PreviewTab::Task;
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No worktree available"));
        assert!(content.contains("n  Create worktree"));
        assert!(content.contains("r  Refresh worktrees"));
    }

    #[test]
    fn test_render_task_tab_with_linked_task_without_details_points_to_actions() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.preview_tab = PreviewTab::Task;
        state.worktrees.push(
            Worktree::new("feature", PathBuf::from("/repo/feature"), "feature-branch")
                .with_task("TASK-123"),
        );
        state.selected = Some(0);
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Task: TASK-123"));
        assert!(content.contains("No details loaded"));
        assert!(content.contains("T  Relink task"));
        assert!(content.contains("r  Refresh worktrees"));
        assert!(!content.contains("No details available"));
    }

    #[test]
    fn test_render_task_tab_without_selection_points_to_navigation() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.preview_tab = PreviewTab::Task;
        state.worktrees.push(Worktree::new(
            "feature",
            PathBuf::from("/repo/feature"),
            "feature-branch",
        ));
        state.selected = None;
        let area = Rect::new(0, 0, 80, 12);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Preview, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No worktree selected"));
        assert!(content.contains("j/k  Navigate worktrees"));
        assert!(content.contains("Enter/o  Open worktree"));
        assert!(content.contains("Tab  Focus sidebar"));
        assert!(!content.contains("Select a worktree"));
    }

    #[test]
    fn test_render_interactive_without_selection_points_to_navigation() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.view_mode = ViewMode::Interactive;
        state.worktrees.push(Worktree::new(
            "feature",
            PathBuf::from("/repo/feature"),
            "feature-branch",
        ));
        state.selected = None;
        let area = Rect::new(0, 0, 80, 12);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Sidebar, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No worktree selected"));
        assert!(content.contains("j/k  Navigate worktrees"));
        assert!(content.contains("Enter/o  Open worktree"));
        assert!(content.contains("Tab  Focus sidebar"));
        assert!(!content.contains("Select a worktree first"));
    }
}
