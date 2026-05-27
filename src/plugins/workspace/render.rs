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

const CREATE_WORKTREE_MODAL_HINT: &str = "Enter: Create  |  Esc: Cancel";
const DELETE_WORKTREE_MODAL_HINT: &str = "Enter/D: Delete  |  Esc: Cancel";
const LINK_TASK_MODAL_HINT: &str = "Enter: Link  |  Esc: Cancel";
const MERGE_WORKFLOW_MODAL_HINT: &str = "1-3: Select  |  Esc: Cancel";
const INTERACTIVE_MODE_HINT: &str = "q: Return";
const MIN_WORKSPACE_MODAL_WIDTH: u16 = 30;
const MIN_WORKSPACE_MODAL_HEIGHT: u16 = 8;

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
    "No worktrees found\n\nn: Create worktree\nr: Refresh worktrees\n/: Global search  |  : Command search\n?: Toggle help\n\nUse worktrees to run agents in parallel without blocking the main checkout."
}

fn no_linked_task_message() -> &'static str {
    "No linked task\n\nT: Link task\nr: Refresh worktrees\n/: Global search  |  : Command search\n?: Toggle help"
}

fn task_details_missing_message(task_id: &str) -> String {
    format!(
        "Task: {}\n\nNo task details loaded\n\nT: Relink task\nr: Refresh worktrees\n/: Global search  |  : Command search\n?: Toggle help",
        task_id
    )
}

fn create_worktree_for_task_message() -> &'static str {
    "No worktrees yet\n\nn: Create worktree\nr: Refresh worktrees\n/: Global search  |  : Command search\n?: Toggle help"
}

fn select_worktree_message() -> &'static str {
    "No worktree selected\n\nj/k: Navigate | Enter/o: Open\nTab/Shift+Tab: Switch pane\nr: Refresh worktrees\n/: Global search  |  : Command search\n?: Toggle help"
}

fn output_empty_message(state: &PluginState) -> &'static str {
    if state.worktrees.is_empty() {
        "No output yet\n\nn: Create worktree\nr: Refresh worktrees\n/: Global search  |  : Command search\n?: Toggle help"
    } else if state.selected_worktree().is_none() {
        "No output selected\n\nj/k: Navigate worktrees\nEnter/o: Open worktree\nTab/Shift+Tab: Switch pane\nr: Refresh worktrees\n/: Global search  |  : Command search\n?: Toggle help"
    } else {
        "No output yet\n\na: Launch agent\nEnter/o: Open interactive shell\nT: Link task\nr: Refresh worktrees\n/: Global search  |  : Command search\n?: Toggle help"
    }
}

fn diff_empty_message(state: &PluginState) -> String {
    if let Some(worktree) = state.selected_worktree() {
        if worktree.is_dirty {
            format!(
                "Diff not loaded yet for {}\n\nr: Refresh worktrees\nj/k: Navigate worktrees\n/: Global search  |  : Command search\n?: Toggle help",
                worktree.name
            )
        } else {
            format!(
                "Working tree clean: {}\n\nj/k: Navigate worktrees\nT: Link task\nr: Refresh worktrees\n/: Global search  |  : Command search\n?: Toggle help",
                worktree.name
            )
        }
    } else if state.worktrees.is_empty() {
        "No diff available\n\nn: Create worktree\nr: Refresh worktrees\n/: Global search  |  : Command search\n?: Toggle help"
            .to_string()
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
        .title(format!(
            " {} ({}) ",
            title,
            workspace_render_count_label(indices.len(), "worktree")
        ))
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
        lines.extend(
            kanban_empty_column_message(state, title)
                .lines()
                .map(|line| {
                    Line::styled(
                        line.to_string(),
                        style_for_ui_element(theme, UiElement::MutedText),
                    )
                }),
        );
    }

    let text = Paragraph::new(lines);
    text.render(inner, buf);
}

fn kanban_empty_column_message(state: &PluginState, title: &str) -> String {
    let status = title.to_lowercase();
    if state.worktrees.is_empty() {
        format!(
            "No {status} worktrees\n\nn: Create worktree\nr: Refresh worktrees\nv: Switch view\n/: Global search  |  : Command search\n?: Toggle help"
        )
    } else {
        format!(
            "No {status} worktrees\n\nj/k: Navigate worktrees\nv: Switch view\nr: Refresh worktrees\n/: Global search  |  : Command search\n?: Toggle help"
        )
    }
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
            "Entering interactive mode for worktree: {}\nPath: {}\nBranch: {}\n\n{}",
            worktree.name,
            worktree.path.display(),
            worktree.branch,
            INTERACTIVE_MODE_HINT
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
    let Some(modal_area) = workspace_modal_area(60, 40, area) else {
        return;
    };

    // Clear background
    Clear.render(modal_area, buf);

    let block = Block::default()
        .title(" Create Worktree ")
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Primary));

    let inner = block.inner(modal_area);
    block.render(modal_area, buf);

    let text = format!(
        "Name: {}\n\nBranch: {}\n\n{}",
        state.new_worktree_name, state.new_worktree_branch, CREATE_WORKTREE_MODAL_HINT
    );

    let paragraph = Paragraph::new(text).style(style_for_ui_element(theme, UiElement::Text));

    paragraph.render(inner, buf);
}

/// Render delete confirmation modal
fn render_delete_confirm_modal(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let Some(modal_area) = workspace_modal_area(50, 30, area) else {
        return;
    };

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
        "Delete worktree '{}'?\n\nThis cannot be undone.\n\n{}",
        worktree_name, DELETE_WORKTREE_MODAL_HINT
    );

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(style_for_ui_element(theme, UiElement::Error));

    paragraph.render(inner, buf);
}

/// Render link task modal
fn render_link_task_modal(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let Some(modal_area) = workspace_modal_area(50, 30, area) else {
        return;
    };

    Clear.render(modal_area, buf);

    let block = Block::default()
        .title(" Link Task ")
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Primary));

    let inner = block.inner(modal_area);
    block.render(modal_area, buf);

    let text = format!(
        "Task ID: {}\n\n{}",
        state.task_id_buffer, LINK_TASK_MODAL_HINT
    );

    let paragraph = Paragraph::new(text).style(style_for_ui_element(theme, UiElement::Text));

    paragraph.render(inner, buf);
}

/// Render merge dialog modal
fn render_merge_dialog_modal(state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let Some(modal_area) = workspace_modal_area(60, 50, area) else {
        return;
    };

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
        "Worktree: {}\n\nMerge options:\n\n1. git merge --no-ff <branch>\n2. git merge --squash <branch>\n3. gh pr create --fill --head <branch>\n\nCommands run from the main repository checkout.\n\n{}",
        worktree_name, MERGE_WORKFLOW_MODAL_HINT
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

fn workspace_modal_area(percent_x: u16, percent_y: u16, area: Rect) -> Option<Rect> {
    if area.width < MIN_WORKSPACE_MODAL_WIDTH || area.height < MIN_WORKSPACE_MODAL_HEIGHT {
        return None;
    }

    let width = area
        .width
        .saturating_mul(percent_x)
        .saturating_div(100)
        .clamp(MIN_WORKSPACE_MODAL_WIDTH, area.width);
    let height = area
        .height
        .saturating_mul(percent_y)
        .saturating_div(100)
        .clamp(MIN_WORKSPACE_MODAL_HEIGHT, area.height);
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);

    Some(Rect::new(x, y, width, height))
}

/// Render status info for the footer
pub fn render_workspace_status(state: &PluginState, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    // Worktree count
    let total = state.worktrees.len();
    let dirty = state.worktrees.iter().filter(|w| w.is_dirty).count();
    let with_agents = state.worktrees.iter().filter(|w| w.agent_running).count();

    spans.push(Span::styled(
        format!("{} ", workspace_render_count_label(total, "worktree")),
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
            format!("{} ", workspace_render_count_label(with_agents, "agent")),
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

fn workspace_render_count_label(count: usize, label: &str) -> String {
    if count == 1 {
        format!("1 {}", label)
    } else {
        format!("{} {}s", count, label)
    }
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
    fn test_workspace_modal_area_uses_percentage_size_when_it_fits() {
        let area = Rect::new(0, 0, 100, 100);
        let centered = workspace_modal_area(50, 50, area).unwrap();

        assert_eq!(centered, Rect::new(25, 25, 50, 50));
    }

    #[test]
    fn test_workspace_modal_area_preserves_minimum_size() {
        let area = Rect::new(6, 4, 40, 12);
        let centered = workspace_modal_area(50, 30, area).unwrap();

        assert_eq!(centered, Rect::new(11, 6, 30, 8));
    }

    #[test]
    fn test_workspace_modal_area_handles_offset_near_u16_max() {
        let area = Rect::new(u16::MAX - 100, u16::MAX - 100, 100, 100);
        let centered = workspace_modal_area(50, 30, area).unwrap();

        assert_eq!(centered, Rect::new(u16::MAX - 75, u16::MAX - 65, 50, 30));
    }

    #[test]
    fn test_workspace_modal_area_skips_tiny_areas() {
        assert!(workspace_modal_area(50, 30, Rect::new(0, 0, 29, 12)).is_none());
        assert!(workspace_modal_area(50, 30, Rect::new(0, 0, 40, 7)).is_none());
    }

    #[test]
    fn test_workspace_modal_hints_use_compact_action_case() {
        let hints = [
            CREATE_WORKTREE_MODAL_HINT,
            DELETE_WORKTREE_MODAL_HINT,
            LINK_TASK_MODAL_HINT,
            MERGE_WORKFLOW_MODAL_HINT,
        ];

        assert!(hints.iter().all(|hint| hint.contains(": ")));
        assert!(hints.iter().all(|hint| hint.contains("Cancel")));
        assert!(CREATE_WORKTREE_MODAL_HINT.contains("Enter: Create"));
        assert!(DELETE_WORKTREE_MODAL_HINT.contains("Enter/D: Delete"));
        assert!(!DELETE_WORKTREE_MODAL_HINT.contains("Other: Cancel"));
        assert!(!DELETE_WORKTREE_MODAL_HINT.contains("y:"));
        assert!(LINK_TASK_MODAL_HINT.contains("Enter: Link"));
        assert!(MERGE_WORKFLOW_MODAL_HINT.contains("1-3: Select"));
        assert!(!hints.iter().any(|hint| hint.starts_with("Press ")));
    }

    #[test]
    fn test_render_delete_worktree_modal_uses_handled_key_hint() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.worktrees.push(Worktree::new(
            "feature-cleanup",
            PathBuf::from("/repo/feature-cleanup"),
            "feature-cleanup",
        ));
        state.selected = Some(0);
        state.modal_state = ModalState::DeleteConfirm;
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Sidebar, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Delete worktree 'feature-cleanup'?"));
        assert!(content.contains(DELETE_WORKTREE_MODAL_HINT));
        assert!(!content.contains("Other: Cancel"));
        assert!(!content.contains("y: Confirm"));
    }

    #[test]
    fn test_render_workspace_status() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.worktrees.push(
            Worktree::new("feature", PathBuf::from("/repo/feature"), "feature-branch")
                .with_agent_running(true),
        );

        let spans = render_workspace_status(&state, &theme);
        assert!(!spans.is_empty());
        let text = spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(text.contains("1 worktree"));
        assert!(text.contains("1 agent"));
        assert!(!text.contains("1 worktrees"));
        assert!(!text.contains("1 agents"));
    }

    #[test]
    fn test_empty_worktrees_message_points_to_next_actions() {
        let message = empty_worktrees_message();

        assert!(message.contains("No worktrees found"));
        assert!(message.contains("n: Create worktree"));
        assert!(message.contains("r: Refresh worktrees"));
        assert!(message.contains("/: Global search"));
        assert!(message.contains(": Command search"));
        assert!(message.contains("?: Toggle help"));
    }

    #[test]
    fn test_workspace_empty_messages_surface_command_search() {
        let assert_hint = |message: &str| {
            assert!(message.contains("/: Global search"), "{message}");
            assert!(message.contains(": Command search"), "{message}");
        };

        assert_hint(empty_worktrees_message());
        assert_hint(no_linked_task_message());
        assert_hint(&task_details_missing_message("TASK-123"));
        assert_hint(create_worktree_for_task_message());
        assert_hint(select_worktree_message());

        let empty_state = PluginState::new();
        assert_hint(output_empty_message(&empty_state));
        assert_hint(&diff_empty_message(&empty_state));

        let mut unselected_state = PluginState::new();
        unselected_state.worktrees.push(Worktree::new(
            "feature",
            PathBuf::from("/repo/feature"),
            "feature-branch",
        ));
        unselected_state.selected = None;
        assert_hint(output_empty_message(&unselected_state));
        assert_hint(&diff_empty_message(&unselected_state));

        let mut clean_state = PluginState::new();
        clean_state.worktrees.push(Worktree::new(
            "clean",
            PathBuf::from("/repo/clean"),
            "clean-branch",
        ));
        clean_state.selected = Some(0);
        assert_hint(output_empty_message(&clean_state));
        assert_hint(&diff_empty_message(&clean_state));

        let mut dirty_state = PluginState::new();
        let mut dirty = Worktree::new("dirty", PathBuf::from("/repo/dirty"), "dirty-branch");
        dirty.is_dirty = true;
        dirty_state.worktrees.push(dirty);
        dirty_state.selected = Some(0);
        assert_hint(&diff_empty_message(&dirty_state));
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
        assert!(content.contains("Global search"));
        assert!(content.contains("Command search"));
    }

    #[test]
    fn test_render_kanban_headers_use_worktree_count_labels() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.view_mode = ViewMode::Kanban;
        let mut active = Worktree::new("active", PathBuf::from("/repo/active"), "feature-active");
        active.is_dirty = true;
        state.worktrees.push(active);
        state.worktrees.push(
            Worktree::new("waiting", PathBuf::from("/repo/waiting"), "feature-waiting")
                .with_task("task-1"),
        );
        state.worktrees.push(Worktree::new(
            "done",
            PathBuf::from("/repo/done"),
            "feature-done",
        ));
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Sidebar, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Active (1 worktree)"));
        assert!(content.contains("Waiting (1 worktree)"));
        assert!(content.contains("Done (1 worktree)"));
        assert!(!content.contains("Active (1)"));
    }

    #[test]
    fn test_render_kanban_empty_columns_point_to_next_actions() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.view_mode = ViewMode::Kanban;
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Sidebar, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No active worktrees"));
        assert!(content.contains("No waiting worktrees"));
        assert!(content.contains("No done worktrees"));
        assert!(content.contains("n: Create worktree"));
        assert!(content.contains("r: Refresh worktrees"));
        assert!(content.contains("v: Switch view"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
        assert!(content.contains("?: Toggle help"));
        assert!(!content.contains("Empty"));
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
        assert!(content.contains("n: Create worktree"));
        assert!(content.contains("r: Refresh worktrees"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
        assert!(content.contains("?: Toggle help"));
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
        assert!(content.contains("j/k: Navigate worktrees"));
        assert!(content.contains("Enter/o: Open worktree"));
        assert!(content.contains("Tab/Shift+Tab: Switch pane"));
        assert!(content.contains("r: Refresh worktrees"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
        assert!(content.contains("?: Toggle help"));
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
        assert!(content.contains("a: Launch agent"));
        assert!(content.contains("Enter/o: Open interactive shell"));
        assert!(content.contains("T: Link task"));
        assert!(content.contains("r: Refresh worktrees"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
        assert!(content.contains("?: Toggle help"));
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
        assert!(content.contains("n: Create worktree"));
        assert!(content.contains("r: Refresh worktrees"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
        assert!(content.contains("?: Toggle help"));
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
        assert!(content.contains("r: Refresh worktrees"));
        assert!(content.contains("j/k: Navigate worktrees"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
        assert!(content.contains("?: Toggle help"));
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
        assert!(content.contains("j/k: Navigate worktrees"));
        assert!(content.contains("T: Link task"));
        assert!(content.contains("r: Refresh worktrees"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
        assert!(content.contains("?: Toggle help"));
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
        assert!(content.contains("T: Link task"));
        assert!(content.contains("r: Refresh worktrees"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
        assert!(content.contains("?: Toggle help"));
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
        assert!(content.contains("No worktrees yet"));
        assert!(content.contains("n: Create worktree"));
        assert!(content.contains("r: Refresh worktrees"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
        assert!(content.contains("?: Toggle help"));
        assert!(!content.contains("No worktree available"));
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
        assert!(content.contains("No task details loaded"));
        assert!(content.contains("T: Relink task"));
        assert!(content.contains("r: Refresh worktrees"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
        assert!(content.contains("?: Toggle help"));
        assert!(!content.contains("No details loaded"));
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
        assert!(content.contains("j/k: Navigate"));
        assert!(content.contains("Enter/o: Open"));
        assert!(content.contains("Tab/Shift+Tab: Switch pane"));
        assert!(content.contains("r: Refresh worktrees"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
        assert!(content.contains("?: Toggle help"));
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
        assert!(content.contains("j/k: Navigate"));
        assert!(content.contains("Enter/o: Open"));
        assert!(content.contains("Tab/Shift+Tab: Switch pane"));
        assert!(content.contains("r: Refresh worktrees"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
        assert!(content.contains("?: Toggle help"));
        assert!(!content.contains("Select a worktree first"));
    }

    #[test]
    fn test_render_interactive_with_selection_uses_compact_return_hint() {
        let theme = Theme::default();
        let mut state = PluginState::new();
        state.view_mode = ViewMode::Interactive;
        state.worktrees.push(Worktree::new(
            "feature",
            PathBuf::from("/repo/feature"),
            "feature-branch",
        ));
        state.selected = Some(0);
        let area = Rect::new(0, 0, 100, 14);
        let mut buf = Buffer::empty(area);

        render_workspace(&state, FocusPane::Sidebar, true, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Entering interactive mode for worktree: feature"));
        assert!(content.contains("Path: /repo/feature"));
        assert!(content.contains("Branch: feature-branch"));
        assert!(content.contains(INTERACTIVE_MODE_HINT));
        assert!(!content.contains("Press 'q' to return"));
    }
}
