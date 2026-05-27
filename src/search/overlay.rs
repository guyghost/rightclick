//! Search overlay UI component
//!
//! Provides a full-screen search overlay with text input, scope tabs,
//! and a scrollable results list.

use crate::core::models::Theme;
use crate::core::models::text_input::{InputMode, TextInputState};
use crate::theme::{UiElement, style_for_ui_element};

use super::types::{SearchResult, SearchScope};

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Widget},
};
use std::str::FromStr;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const SEARCH_EMPTY_ACTION_HINT: &str = "Ctrl+U: Clear | Tab: Scope | Esc: Close";
const MIN_SEARCH_OVERLAY_WIDTH: u16 = 20;
const MIN_SEARCH_OVERLAY_HEIGHT: u16 = 8;
const SEARCH_RESULT_SCROLL_WINDOW: usize = 10;

/// State for the search overlay
#[derive(Debug, Clone)]
pub struct SearchOverlayState {
    /// Whether the overlay is visible
    pub visible: bool,
    /// The text input for the search query
    pub input: TextInputState,
    /// Current search scope
    pub scope: SearchScope,
    /// Search results
    pub results: Vec<SearchResult>,
    /// Currently selected result index
    pub selected: usize,
    /// Scroll offset for the results list
    pub scroll_offset: usize,
}

impl SearchOverlayState {
    /// Create a new search overlay state
    pub fn new() -> Self {
        let mut input = TextInputState::new(InputMode::SingleLine)
            .with_placeholder("Search files, commands, project items...");
        input.set_active(true);
        Self {
            visible: false,
            input,
            scope: SearchScope::All,
            results: Vec::new(),
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Open the search overlay
    pub fn open(&mut self) {
        self.visible = true;
        self.input.set_active(true);
        self.input.clear();
        self.results.clear();
        self.selected = 0;
        self.scroll_offset = 0;
        self.scope = SearchScope::All;
    }

    /// Open the search overlay with a specific scope selected
    pub fn open_with_scope(&mut self, scope: SearchScope) {
        self.open();
        self.scope = scope;
    }

    /// Close the search overlay
    pub fn close(&mut self) {
        self.visible = false;
        self.input.set_active(false);
    }

    /// Cycle to the next search scope
    pub fn next_scope(&mut self) {
        self.scope = self.scope.next();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        self.normalize_selection(SEARCH_RESULT_SCROLL_WINDOW);
        if self.selected > 0 {
            self.selected -= 1;
        }
        self.normalize_selection(SEARCH_RESULT_SCROLL_WINDOW);
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        self.normalize_selection(SEARCH_RESULT_SCROLL_WINDOW);
        if self.selected.saturating_add(1) < self.results.len() {
            self.selected += 1;
        }
        self.normalize_selection(SEARCH_RESULT_SCROLL_WINDOW);
    }

    /// Get the currently selected result
    pub fn selected_result(&self) -> Option<&SearchResult> {
        self.results.get(self.selected)
    }

    /// Update results and reset selection
    pub fn set_results(&mut self, results: Vec<SearchResult>) {
        self.results = results;
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Returns the current query text
    pub fn query(&self) -> &str {
        self.input.text()
    }

    /// Handle a key event, returns true if the overlay consumed it
    pub fn handle_key(&mut self, code: &str, ctrl: bool) -> SearchOverlayAction {
        if !self.visible {
            return SearchOverlayAction::Ignored;
        }

        // Ctrl+U clears the input
        if ctrl && code == "u" {
            self.input.clear();
            return SearchOverlayAction::QueryChanged;
        }

        match code {
            "Esc" => {
                self.close();
                SearchOverlayAction::Closed
            }
            "Tab" => {
                self.next_scope();
                SearchOverlayAction::ScopeChanged
            }
            "Up" => {
                self.select_prev();
                SearchOverlayAction::SelectionChanged
            }
            "Down" => {
                self.select_next();
                SearchOverlayAction::SelectionChanged
            }
            "Enter" => {
                if self.selected_result().is_some() {
                    SearchOverlayAction::ResultSelected
                } else {
                    SearchOverlayAction::Consumed
                }
            }
            "Backspace" => {
                self.input.delete_char_before();
                SearchOverlayAction::QueryChanged
            }
            "Delete" => {
                self.input.delete_char_after();
                SearchOverlayAction::QueryChanged
            }
            "Left" => {
                self.input.move_cursor_left();
                SearchOverlayAction::Consumed
            }
            "Right" => {
                self.input.move_cursor_right();
                SearchOverlayAction::Consumed
            }
            "Home" => {
                self.input.move_cursor_home();
                SearchOverlayAction::Consumed
            }
            "End" => {
                self.input.move_cursor_end();
                SearchOverlayAction::Consumed
            }
            key if key.len() == 1 => {
                if let Some(c) = key.chars().next() {
                    self.input.insert_char(c);
                    SearchOverlayAction::QueryChanged
                } else {
                    SearchOverlayAction::Consumed
                }
            }
            _ => SearchOverlayAction::Consumed,
        }
    }

    /// Ensure the selected item is visible given a viewport height
    pub fn ensure_visible(&mut self, viewport_height: usize) {
        self.normalize_selection(viewport_height);
    }

    fn normalize_selection(&mut self, viewport_height: usize) {
        let (selected, scroll_offset) = normalized_selection_window(
            self.results.len(),
            self.selected,
            self.scroll_offset,
            viewport_height,
        );
        self.selected = selected;
        self.scroll_offset = scroll_offset;
    }
}

impl Default for SearchOverlayState {
    fn default() -> Self {
        Self::new()
    }
}

/// Actions returned by the search overlay's key handler
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchOverlayAction {
    /// Key was not consumed (overlay not visible)
    Ignored,
    /// Key was consumed but no state change needed
    Consumed,
    /// The search query changed, re-run search
    QueryChanged,
    /// The search scope changed, re-run search
    ScopeChanged,
    /// The selection moved
    SelectionChanged,
    /// A result was selected (Enter pressed)
    ResultSelected,
    /// The overlay was closed
    Closed,
}

/// Render the search overlay
pub fn render_search_overlay(
    state: &SearchOverlayState,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    if !state.visible {
        return;
    }

    let Some(overlay_area) = search_overlay_area(area) else {
        return;
    };

    // Dim background
    let dim_style = style_for_ui_element(theme, UiElement::Background);
    for by in area.top()..area.bottom() {
        for bx in area.left()..area.right() {
            let outside = bx < overlay_area.left()
                || bx >= overlay_area.right()
                || by < overlay_area.top()
                || by >= overlay_area.bottom();
            if outside {
                if let Some(cell) = buf.cell_mut((bx, by)) {
                    cell.set_style(dim_style);
                }
            }
        }
    }

    // Clear overlay area
    Clear.render(overlay_area, buf);

    // Theme colors
    let primary = Color::from_str(&theme.colors.primary).unwrap_or(Color::Blue);
    let fg = Color::from_str(&theme.colors.foreground).unwrap_or(Color::White);
    let muted = Color::from_str(&theme.colors.muted).unwrap_or(Color::DarkGray);
    let border_color = Color::from_str(&theme.colors.border).unwrap_or(Color::Gray);
    let bg = Color::from_str(&theme.colors.background).unwrap_or(Color::Black);

    let title = search_overlay_title(state.scope, state.results.len());

    // Outer block
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(primary))
        .style(Style::default().bg(bg).fg(fg))
        .title(Span::styled(
            title,
            Style::default().fg(primary).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(overlay_area);
    block.render(overlay_area, buf);

    if inner.height < 4 || inner.width < 10 {
        return;
    }

    // Layout: input (1) + scope tabs (1) + separator (1) + results (rest) + footer (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Search input
            Constraint::Length(1), // Scope tabs
            Constraint::Length(1), // Separator
            Constraint::Min(1),    // Results
            Constraint::Length(1), // Help footer
        ])
        .split(inner);

    // Render search input line
    render_input_line(&state.input, state.scope, chunks[0], buf, fg, primary);

    // Render scope tabs
    render_scope_tabs(state, chunks[1], buf, primary, muted);

    // Separator
    let sep = "─".repeat(chunks[2].width as usize);
    Paragraph::new(sep)
        .style(Style::default().fg(border_color))
        .render(chunks[2], buf);

    render_results(state, chunks[3], buf, fg, muted, primary, bg);

    render_footer(chunks[4], buf, muted);
}

fn search_overlay_area(area: Rect) -> Option<Rect> {
    if area.width < MIN_SEARCH_OVERLAY_WIDTH || area.height < MIN_SEARCH_OVERLAY_HEIGHT {
        return None;
    }

    let overlay_width = area
        .width
        .saturating_mul(4)
        .saturating_div(5)
        .clamp(MIN_SEARCH_OVERLAY_WIDTH, 100)
        .min(area.width);
    let overlay_height = area
        .height
        .saturating_mul(7)
        .saturating_div(10)
        .clamp(MIN_SEARCH_OVERLAY_HEIGHT, 40)
        .min(area.height);
    let x = area
        .x
        .saturating_add((area.width.saturating_sub(overlay_width)) / 2);
    let y = area
        .y
        .saturating_add((area.height.saturating_sub(overlay_height)) / 2);

    Some(Rect::new(x, y, overlay_width, overlay_height))
}

fn render_input_line(
    input: &TextInputState,
    scope: SearchScope,
    area: Rect,
    buf: &mut Buffer,
    fg: Color,
    primary: Color,
) {
    if area.width < 5 {
        return;
    }

    let text = input.text();
    let cursor_pos = input.cursor_pos();

    // Prompt
    let prompt = Span::styled(
        search_input_prompt(scope),
        Style::default().fg(primary).add_modifier(Modifier::BOLD),
    );

    if text.is_empty() {
        let placeholder = search_input_placeholder(scope);
        let line = Line::from(vec![
            prompt,
            Span::styled(" ", Style::default().bg(Color::White).fg(Color::Black)),
            Span::styled(placeholder, Style::default().fg(Color::DarkGray)),
        ]);
        Paragraph::new(line).render(area, buf);
    } else {
        let before = &text[..cursor_pos];
        let cursor_char = if cursor_pos < text.len() {
            &text[cursor_pos
                ..cursor_pos
                    + text[cursor_pos..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1)]
        } else {
            " "
        };
        let after = if cursor_pos < text.len() {
            &text[cursor_pos + cursor_char.len()..]
        } else {
            ""
        };

        let line = Line::from(vec![
            prompt,
            Span::styled(before, Style::default().fg(fg)),
            Span::styled(
                cursor_char,
                Style::default().bg(Color::White).fg(Color::Black),
            ),
            Span::styled(after, Style::default().fg(fg)),
        ]);
        Paragraph::new(line).render(area, buf);
    }
}

fn render_scope_tabs(
    state: &SearchOverlayState,
    area: Rect,
    buf: &mut Buffer,
    primary: Color,
    muted: Color,
) {
    if area.width == 0 {
        return;
    }

    if area.width < full_scope_tabs_width(state.results.len()) as u16 {
        Paragraph::new(compact_scope_tabs_text(
            state.scope,
            state.results.len(),
            area.width,
        ))
        .style(Style::default().fg(muted))
        .render(area, buf);
        return;
    }

    let scopes = [
        SearchScope::All,
        SearchScope::Files,
        SearchScope::Items,
        SearchScope::Commands,
    ];

    let mut spans = Vec::new();
    for (i, s) in scopes.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" | ", Style::default().fg(muted)));
        }
        if *s == state.scope {
            spans.push(Span::styled(
                s.label(),
                Style::default()
                    .fg(primary)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
        } else {
            spans.push(Span::styled(s.label(), Style::default().fg(muted)));
        }
    }
    spans.push(Span::styled(
        format!("  {}", result_count_label(state.results.len())),
        Style::default().fg(muted),
    ));

    Paragraph::new(Line::from(spans)).render(area, buf);
}

fn full_scope_tabs_width(result_count: usize) -> usize {
    [
        SearchScope::All,
        SearchScope::Files,
        SearchScope::Items,
        SearchScope::Commands,
    ]
    .iter()
    .enumerate()
    .map(|(index, scope)| scope.label().width() + if index == 0 { 0 } else { " | ".width() })
    .sum::<usize>()
        + "  ".width()
        + result_count_label(result_count).width()
}

fn compact_scope_tabs_text(scope: SearchScope, result_count: usize, width: u16) -> String {
    let width = width as usize;
    let count = result_count_label(result_count);
    let label = scope.label();
    let short_label = compact_scope_label(scope);

    [
        format!("Scope: {label} | {count}"),
        format!("{label} | {count}"),
        format!("Scope: {label}"),
        label.to_string(),
        short_label.to_string(),
    ]
    .into_iter()
    .find(|text| text.width() <= width)
    .unwrap_or_default()
}

fn compact_scope_label(scope: SearchScope) -> &'static str {
    match scope {
        SearchScope::All => "All",
        SearchScope::Files => "File",
        SearchScope::Items => "Proj",
        SearchScope::Commands => "Cmd",
    }
}

fn render_results(
    state: &SearchOverlayState,
    area: Rect,
    buf: &mut Buffer,
    fg: Color,
    muted: Color,
    primary: Color,
    bg: Color,
) {
    if area.height == 0 {
        return;
    }

    if state.results.is_empty() {
        let msg = if state.query().is_empty() {
            empty_query_hint(state.scope).to_string()
        } else {
            no_results_message(state.query(), state.scope, area.width)
        };
        Paragraph::new(msg)
            .style(Style::default().fg(muted))
            .alignment(Alignment::Center)
            .render(area, buf);
        return;
    }

    let viewport_height = area.height as usize;
    let (selected, scroll) = normalized_selection_window(
        state.results.len(),
        state.selected,
        state.scroll_offset,
        viewport_height,
    );

    let items: Vec<ListItem> = state
        .results
        .iter()
        .enumerate()
        .skip(scroll)
        .take(viewport_height)
        .map(|(i, result)| {
            let is_selected = i == selected;

            let icon = search_result_icon(&result.kind);

            let title_style = if is_selected {
                Style::default()
                    .fg(bg)
                    .bg(primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(fg)
            };
            let preview_style = if is_selected {
                Style::default().fg(bg).bg(primary)
            } else {
                Style::default().fg(muted)
            };

            let line = Line::from(vec![
                Span::styled(format!("{:<4} ", icon), preview_style),
                Span::styled(
                    display_result_title(&result.title, area.width as usize),
                    title_style,
                ),
                Span::styled("  ", preview_style),
                Span::styled(
                    truncate_str(&result.preview, area.width as usize / 2),
                    preview_style,
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items);
    list.render(area, buf);
}

fn normalized_selection_window(
    item_count: usize,
    selected: usize,
    scroll_offset: usize,
    viewport_height: usize,
) -> (usize, usize) {
    if item_count == 0 {
        return (0, 0);
    }

    let selected = selected.min(item_count - 1);
    let mut scroll_offset = scroll_offset.min(item_count - 1);

    if viewport_height == 0 {
        return (selected, selected);
    }

    if selected < scroll_offset {
        scroll_offset = selected;
    } else if selected >= scroll_offset.saturating_add(viewport_height) {
        scroll_offset = selected.saturating_sub(viewport_height.saturating_sub(1));
    }

    scroll_offset = scroll_offset.min(item_count.saturating_sub(viewport_height));

    (selected, scroll_offset)
}

fn display_result_title(title: &str, area_width: usize) -> String {
    truncate_str(title, result_title_max_len(area_width))
}

fn result_title_max_len(area_width: usize) -> usize {
    if area_width <= 12 {
        return area_width.saturating_sub(5);
    }

    area_width
        .saturating_sub(8)
        .saturating_mul(2)
        .saturating_div(5)
        .clamp(8, 48)
}

fn render_footer(area: Rect, buf: &mut Buffer, muted: Color) {
    if area.height == 0 {
        return;
    }

    Paragraph::new(search_footer_hint(area.width))
        .style(Style::default().fg(muted))
        .alignment(Alignment::Center)
        .render(area, buf);
}

fn search_overlay_title(scope: SearchScope, result_count: usize) -> String {
    let title = match scope {
        SearchScope::All => "Global Search",
        SearchScope::Files => "File Search",
        SearchScope::Items => "Project Item Search",
        SearchScope::Commands => "Command Search",
    };

    if result_count == 0 {
        format!(" {title} ")
    } else {
        format!(" {title} - {} ", result_count_label(result_count))
    }
}

fn search_input_prompt(scope: SearchScope) -> &'static str {
    match scope {
        SearchScope::Commands => ": ",
        _ => "/ ",
    }
}

fn search_input_placeholder(scope: SearchScope) -> &'static str {
    match scope {
        SearchScope::All => "Search files, commands, project items...",
        SearchScope::Files => "Search file contents...",
        SearchScope::Items => "Search sessions, worktrees, and intents...",
        SearchScope::Commands => "Search commands...",
    }
}

fn search_footer_hint(width: u16) -> &'static str {
    let width = width as usize;
    [
        "Tab: Scope  |  Enter: Open  |  Up/Down: Select  |  Ctrl+U: Clear  |  Esc: Close",
        "Tab: Scope  |  Enter: Open  |  Up/Down: Select  |  Esc: Close",
        "Tab: Scope  |  Enter: Open  |  Esc: Close",
        "Enter: Open  |  Esc: Close",
        "Enter: Open | Esc",
        "Enter/Esc",
        "Esc",
    ]
    .into_iter()
    .find(|hint| hint.width() <= width)
    .unwrap_or("")
}

fn result_count_label(count: usize) -> String {
    match count {
        0 => "No results".to_string(),
        1 => "1 result".to_string(),
        n => format!("{} results", n),
    }
}

fn empty_query_hint(scope: SearchScope) -> &'static str {
    match scope {
        SearchScope::All => "Search everything: files, commands, sessions, worktrees, intents",
        SearchScope::Files => "Search file contents with ripgrep",
        SearchScope::Items => "Search project sessions, worktrees, and intents",
        SearchScope::Commands => "Search available commands",
    }
}

fn no_results_message(query: &str, scope: SearchScope, width: u16) -> String {
    let query = truncate_str(query, 40);
    let message = match scope {
        SearchScope::All => format!("No results match \"{}\"", query),
        SearchScope::Files => format!("No file content matches \"{}\"", query),
        SearchScope::Items => format!("No project items match \"{}\"", query),
        SearchScope::Commands => format!("No command matches \"{}\"", query),
    };

    format!("{}\n\n{}", message, search_empty_action_hint(width))
}

fn search_empty_action_hint(width: u16) -> &'static str {
    let width = width as usize;
    [
        SEARCH_EMPTY_ACTION_HINT,
        "Ctrl+U: Clear | Tab: Scope | Esc",
        "Clear | Scope | Esc",
        "Ctrl+U/Tab/Esc",
        "Tab/Esc",
        "Esc",
    ]
    .into_iter()
    .find(|hint| hint.width() <= width)
    .unwrap_or("")
}

fn search_result_icon(kind: &super::types::SearchResultKind) -> &'static str {
    match kind {
        super::types::SearchResultKind::FileContent { .. } => "file",
        super::types::SearchResultKind::Conversation { .. } => "chat",
        super::types::SearchResultKind::PluginEntry { plugin_id, .. } => match plugin_id.as_str() {
            "conversations" => "chat",
            "workspace" => "tree",
            "workers" => "task",
            _ => "item",
        },
        super::types::SearchResultKind::Command { .. } => ">",
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.width() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        let mut output = String::new();
        let mut width = 0;
        for ch in s.chars() {
            let ch_width = ch.width().unwrap_or(0);
            if width + ch_width + 3 > max_len {
                break;
            }
            output.push(ch);
            width += ch_width;
        }
        format!("{}...", output)
    } else {
        let mut output = String::new();
        let mut width = 0;
        for ch in s.chars() {
            let ch_width = ch.width().unwrap_or(0);
            if width + ch_width > max_len {
                break;
            }
            output.push(ch);
            width += ch_width;
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::types::SearchResultKind;

    fn command_result(idx: usize) -> SearchResult {
        SearchResult {
            kind: SearchResultKind::Command {
                id: format!("cmd-{}", idx),
            },
            title: format!("Command {}", idx),
            preview: String::new(),
            score: 100u32.saturating_sub(idx as u32),
        }
    }

    #[test]
    fn test_overlay_new() {
        let state = SearchOverlayState::new();
        assert!(!state.visible);
        assert_eq!(state.scope, SearchScope::All);
        assert!(state.results.is_empty());
        assert_eq!(state.selected, 0);
        assert_eq!(
            state.input.placeholder(),
            "Search files, commands, project items..."
        );
    }

    #[test]
    fn test_overlay_open_close() {
        let mut state = SearchOverlayState::new();
        state.open();
        assert!(state.visible);
        assert!(state.input.is_active());
        assert_eq!(state.query(), "");

        state.close();
        assert!(!state.visible);
        assert!(!state.input.is_active());
    }

    #[test]
    fn test_overlay_open_with_scope() {
        let mut state = SearchOverlayState::new();
        state.open_with_scope(SearchScope::Commands);

        assert!(state.visible);
        assert!(state.input.is_active());
        assert_eq!(state.scope, SearchScope::Commands);
        assert_eq!(state.query(), "");
        assert!(state.results.is_empty());
    }

    #[test]
    fn test_overlay_scope_cycle() {
        let mut state = SearchOverlayState::new();
        assert_eq!(state.scope, SearchScope::All);
        state.next_scope();
        assert_eq!(state.scope, SearchScope::Files);
        state.next_scope();
        assert_eq!(state.scope, SearchScope::Items);
        state.next_scope();
        assert_eq!(state.scope, SearchScope::Commands);
        state.next_scope();
        assert_eq!(state.scope, SearchScope::All);
    }

    #[test]
    fn test_overlay_selection() {
        let mut state = SearchOverlayState::new();
        state.set_results(vec![
            SearchResult {
                kind: SearchResultKind::FileContent {
                    path: "a.rs".into(),
                    line: 1,
                    column: 1,
                },
                title: "a.rs".into(),
                preview: "line 1".into(),
                score: 100,
            },
            SearchResult {
                kind: SearchResultKind::FileContent {
                    path: "b.rs".into(),
                    line: 2,
                    column: 1,
                },
                title: "b.rs".into(),
                preview: "line 2".into(),
                score: 80,
            },
        ]);

        assert_eq!(state.selected, 0);
        state.select_next();
        assert_eq!(state.selected, 1);
        state.select_next();
        assert_eq!(state.selected, 1); // Can't go past end
        state.select_prev();
        assert_eq!(state.selected, 0);
        state.select_prev();
        assert_eq!(state.selected, 0); // Can't go before 0
    }

    #[test]
    fn test_overlay_selection_advances_scroll() {
        let mut state = SearchOverlayState::new();
        state.set_results((0..12).map(command_result).collect());

        for _ in 0..10 {
            state.select_next();
        }

        assert_eq!(state.selected, 10);
        assert_eq!(state.scroll_offset, 1);
    }

    #[test]
    fn test_truncate_str_handles_unicode_boundaries() {
        assert_eq!(truncate_str("éclair", 4), "é...");
        assert_eq!(truncate_str("abc", 2), "ab");
        assert_eq!(truncate_str("検索query", 8), "検索q...");
        assert_eq!(truncate_str("検索", 3), "検");
        assert_eq!(truncate_str("abc", 0), "");
    }

    #[test]
    fn test_display_result_title_truncates_long_titles() {
        assert_eq!(
            display_result_title("src/search/overlay.rs:123", 24),
            "src/s..."
        );
        assert_eq!(display_result_title("src/main.rs:42", 80), "src/main.rs:42");
    }

    #[test]
    fn test_result_title_max_len_preserves_preview_space() {
        assert_eq!(result_title_max_len(10), 5);
        assert_eq!(result_title_max_len(80), 28);
        assert_eq!(result_title_max_len(200), 48);
    }

    #[test]
    fn test_overlay_handle_key_typing() {
        let mut state = SearchOverlayState::new();
        state.open();

        let action = state.handle_key("h", false);
        assert_eq!(action, SearchOverlayAction::QueryChanged);
        assert_eq!(state.query(), "h");

        let action = state.handle_key("i", false);
        assert_eq!(action, SearchOverlayAction::QueryChanged);
        assert_eq!(state.query(), "hi");
    }

    #[test]
    fn test_overlay_handle_key_esc() {
        let mut state = SearchOverlayState::new();
        state.open();

        let action = state.handle_key("Esc", false);
        assert_eq!(action, SearchOverlayAction::Closed);
        assert!(!state.visible);
    }

    #[test]
    fn test_overlay_handle_key_tab() {
        let mut state = SearchOverlayState::new();
        state.open();

        let action = state.handle_key("Tab", false);
        assert_eq!(action, SearchOverlayAction::ScopeChanged);
        assert_eq!(state.scope, SearchScope::Files);
    }

    #[test]
    fn test_overlay_handle_key_backspace() {
        let mut state = SearchOverlayState::new();
        state.open();
        state.handle_key("a", false);
        state.handle_key("b", false);
        assert_eq!(state.query(), "ab");

        let action = state.handle_key("Backspace", false);
        assert_eq!(action, SearchOverlayAction::QueryChanged);
        assert_eq!(state.query(), "a");
    }

    #[test]
    fn test_overlay_handle_key_navigation() {
        let mut state = SearchOverlayState::new();
        state.open();
        state.set_results(vec![
            SearchResult {
                kind: SearchResultKind::Command { id: "a".into() },
                title: "a".into(),
                preview: "".into(),
                score: 100,
            },
            SearchResult {
                kind: SearchResultKind::Command { id: "b".into() },
                title: "b".into(),
                preview: "".into(),
                score: 80,
            },
        ]);

        let action = state.handle_key("Down", false);
        assert_eq!(action, SearchOverlayAction::SelectionChanged);
        assert_eq!(state.selected, 1);

        let action = state.handle_key("Up", false);
        assert_eq!(action, SearchOverlayAction::SelectionChanged);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn test_overlay_handle_key_enter() {
        let mut state = SearchOverlayState::new();
        state.open();
        state.set_results(vec![SearchResult {
            kind: SearchResultKind::Command { id: "a".into() },
            title: "a".into(),
            preview: "".into(),
            score: 100,
        }]);

        let action = state.handle_key("Enter", false);
        assert_eq!(action, SearchOverlayAction::ResultSelected);
    }

    #[test]
    fn test_overlay_handle_key_not_visible() {
        let mut state = SearchOverlayState::new();
        let action = state.handle_key("a", false);
        assert_eq!(action, SearchOverlayAction::Ignored);
    }

    #[test]
    fn test_overlay_ctrl_u_clears() {
        let mut state = SearchOverlayState::new();
        state.open();
        state.handle_key("h", false);
        state.handle_key("e", false);
        assert_eq!(state.query(), "he");

        let action = state.handle_key("u", true);
        assert_eq!(action, SearchOverlayAction::QueryChanged);
        assert_eq!(state.query(), "");
    }

    #[test]
    fn test_overlay_ensure_visible() {
        let mut state = SearchOverlayState::new();
        state.set_results((0..20).map(command_result).collect());
        state.selected = 15;
        state.scroll_offset = 0;
        state.ensure_visible(10);
        assert_eq!(state.scroll_offset, 6); // 15 - 10 + 1

        state.selected = 3;
        state.ensure_visible(10);
        assert_eq!(state.scroll_offset, 3); // scrolled up to show selected
    }

    #[test]
    fn test_overlay_ensure_visible_uses_saturating_offsets() {
        let mut state = SearchOverlayState::new();
        state.set_results((0..20).map(command_result).collect());
        state.selected = usize::MAX;
        state.scroll_offset = usize::MAX;

        state.ensure_visible(10);

        assert_eq!(state.selected, 19);
        assert_eq!(state.scroll_offset, 10);
    }

    #[test]
    fn test_overlay_ensure_visible_resets_empty_stale_state() {
        let mut state = SearchOverlayState::new();
        state.selected = usize::MAX;
        state.scroll_offset = usize::MAX;

        state.ensure_visible(10);

        assert_eq!(state.selected, 0);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_overlay_navigation_clamps_stale_public_selection() {
        let mut state = SearchOverlayState::new();
        state.set_results((0..3).map(command_result).collect());
        state.selected = usize::MAX;
        state.scroll_offset = usize::MAX;

        state.select_next();

        assert_eq!(state.selected, 2);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_render_results_highlights_clamped_stale_selection() {
        let mut state = SearchOverlayState::new();
        state.set_results((0..2).map(command_result).collect());
        state.selected = usize::MAX;
        state.scroll_offset = usize::MAX;

        let area = Rect::new(0, 0, 80, 4);
        let mut buf = Buffer::empty(area);

        render_results(
            &state,
            area,
            &mut buf,
            Color::White,
            Color::Gray,
            Color::Blue,
            Color::Black,
        );

        let second_row: String = (0..area.width)
            .map(|x| buf.cell((x, 1)).unwrap().symbol().to_string())
            .collect();

        assert!(second_row.contains("Command 1"));
        assert_eq!(buf.cell((5, 1)).unwrap().style().bg, Some(Color::Blue));
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 8), "hello...");
        assert_eq!(truncate_str("ab", 2), "ab");
    }

    #[test]
    fn test_result_count_label() {
        assert_eq!(result_count_label(0), "No results");
        assert_eq!(result_count_label(1), "1 result");
        assert_eq!(result_count_label(2), "2 results");
    }

    #[test]
    fn test_scope_tabs_width_accounts_for_result_count() {
        assert_eq!(full_scope_tabs_width(0), 44);
        assert_eq!(full_scope_tabs_width(1), 42);
        assert_eq!(full_scope_tabs_width(2), 43);
    }

    #[test]
    fn test_compact_scope_tabs_keep_active_scope_visible() {
        assert_eq!(
            compact_scope_tabs_text(SearchScope::Commands, 0, 28),
            "Scope: Commands | No results"
        );
        assert_eq!(
            compact_scope_tabs_text(SearchScope::Commands, 0, 20),
            "Scope: Commands"
        );
        assert_eq!(
            compact_scope_tabs_text(SearchScope::Commands, 0, 8),
            "Commands"
        );
        assert_eq!(compact_scope_tabs_text(SearchScope::Commands, 0, 3), "Cmd");
        assert_eq!(compact_scope_tabs_text(SearchScope::Commands, 0, 2), "");
    }

    #[test]
    fn test_render_scope_tabs_compacts_when_narrow() {
        let mut state = SearchOverlayState::new();
        state.open_with_scope(SearchScope::Commands);
        let area = Rect::new(0, 0, 8, 1);
        let mut buf = Buffer::empty(area);

        render_scope_tabs(&state, area, &mut buf, Color::Blue, Color::DarkGray);

        let content: String = (0..area.width)
            .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
            .collect();
        assert!(content.contains("Commands"));
        assert!(!content.contains("All"));
    }

    #[test]
    fn test_search_overlay_title_is_scope_specific() {
        assert_eq!(search_overlay_title(SearchScope::All, 0), " Global Search ");
        assert_eq!(
            search_overlay_title(SearchScope::Files, 2),
            " File Search - 2 results "
        );
        assert_eq!(
            search_overlay_title(SearchScope::Items, 1),
            " Project Item Search - 1 result "
        );
        assert_eq!(
            search_overlay_title(SearchScope::Commands, 0),
            " Command Search "
        );
    }

    #[test]
    fn test_search_input_copy_is_scope_specific() {
        assert_eq!(search_input_prompt(SearchScope::All), "/ ");
        assert_eq!(search_input_prompt(SearchScope::Files), "/ ");
        assert_eq!(search_input_prompt(SearchScope::Items), "/ ");
        assert_eq!(search_input_prompt(SearchScope::Commands), ": ");

        assert_eq!(
            search_input_placeholder(SearchScope::All),
            "Search files, commands, project items..."
        );
        assert_eq!(
            search_input_placeholder(SearchScope::Files),
            "Search file contents..."
        );
        assert_eq!(
            search_input_placeholder(SearchScope::Items),
            "Search sessions, worktrees, and intents..."
        );
        assert_eq!(
            search_input_placeholder(SearchScope::Commands),
            "Search commands..."
        );
    }

    #[test]
    fn test_search_footer_hint_names_scope_action() {
        let hint = search_footer_hint(80);

        assert!(hint.contains("Tab: Scope"));
        assert!(hint.contains("Enter: Open"));
        assert!(hint.contains("Up/Down: Select"));
        assert!(hint.contains("Ctrl+U: Clear"));
        assert!(hint.contains("Esc: Close"));
        assert!(!hint.contains("Tab change scope"));
    }

    #[test]
    fn test_search_footer_hint_compacts_for_narrow_widths() {
        assert_eq!(search_footer_hint(2), "");
        assert_eq!(search_footer_hint(3), "Esc");
        assert_eq!(search_footer_hint(9), "Enter/Esc");
        assert_eq!(search_footer_hint(20), "Enter: Open | Esc");
        assert_eq!(search_footer_hint(26), "Enter: Open  |  Esc: Close");
        assert_eq!(
            search_footer_hint(48),
            "Tab: Scope  |  Enter: Open  |  Esc: Close"
        );
        assert_eq!(
            search_footer_hint(61),
            "Tab: Scope  |  Enter: Open  |  Up/Down: Select  |  Esc: Close"
        );
        assert!(!search_footer_hint(61).contains("Ctrl+U: Clear"));
    }

    #[test]
    fn test_search_footer_hint_never_exceeds_width() {
        for width in 0..=100 {
            assert!(
                search_footer_hint(width).width() <= width as usize,
                "hint for width {width:?} was {:?}",
                search_footer_hint(width)
            );
        }
    }

    #[test]
    fn test_search_empty_action_hint_compacts_for_narrow_widths() {
        assert_eq!(search_empty_action_hint(2), "");
        assert_eq!(search_empty_action_hint(3), "Esc");
        assert_eq!(search_empty_action_hint(7), "Tab/Esc");
        assert_eq!(search_empty_action_hint(14), "Ctrl+U/Tab/Esc");
        assert_eq!(search_empty_action_hint(19), "Clear | Scope | Esc");
        assert_eq!(
            search_empty_action_hint(32),
            "Ctrl+U: Clear | Tab: Scope | Esc"
        );
        assert_eq!(search_empty_action_hint(80), SEARCH_EMPTY_ACTION_HINT);
    }

    #[test]
    fn test_search_empty_action_hint_never_exceeds_width() {
        for width in 0..=100 {
            assert!(
                search_empty_action_hint(width).width() <= width as usize,
                "hint for width {width:?} was {:?}",
                search_empty_action_hint(width)
            );
        }
    }

    #[test]
    fn test_empty_query_hint_is_scope_specific() {
        assert_eq!(
            empty_query_hint(SearchScope::All),
            "Search everything: files, commands, sessions, worktrees, intents"
        );
        assert_eq!(
            empty_query_hint(SearchScope::Files),
            "Search file contents with ripgrep"
        );
        assert_eq!(
            empty_query_hint(SearchScope::Items),
            "Search project sessions, worktrees, and intents"
        );
        assert_eq!(
            empty_query_hint(SearchScope::Commands),
            "Search available commands"
        );
    }

    #[test]
    fn test_no_results_message_includes_scope_and_query() {
        let items = no_results_message("worker", SearchScope::Items, 80);
        assert!(items.contains("No project items match \"worker\""));
        assert!(items.contains(SEARCH_EMPTY_ACTION_HINT));
        assert!(!items.contains("Ctrl+U  Clear search"));
        assert!(!items.contains("Tab  Change scope"));
        assert!(items.contains("Esc: Close"));

        let truncated = no_results_message(
            "abcdefghijklmnopqrstuvwxyz0123456789abcdef",
            SearchScope::All,
            80,
        );
        assert!(
            truncated.contains("No results match \"abcdefghijklmnopqrstuvwxyz0123456789a...\"")
        );
        assert!(truncated.contains(SEARCH_EMPTY_ACTION_HINT));
    }

    #[test]
    fn test_no_results_message_uses_compact_hint_when_narrow() {
        let message = no_results_message("deploy", SearchScope::Commands, 14);

        assert!(message.contains("No command matches \"deploy\""));
        assert!(message.contains("Ctrl+U/Tab/Esc"));
        assert!(!message.contains(SEARCH_EMPTY_ACTION_HINT));
    }

    #[test]
    fn test_search_result_icon_for_plugin_entries() {
        assert_eq!(
            search_result_icon(&SearchResultKind::PluginEntry {
                plugin_id: "conversations".to_string(),
                entry_id: "session-1".to_string(),
            }),
            "chat"
        );
        assert_eq!(
            search_result_icon(&SearchResultKind::PluginEntry {
                plugin_id: "workspace".to_string(),
                entry_id: "main".to_string(),
            }),
            "tree"
        );
        assert_eq!(
            search_result_icon(&SearchResultKind::PluginEntry {
                plugin_id: "workers".to_string(),
                entry_id: "intent-1".to_string(),
            }),
            "task"
        );
    }

    #[test]
    fn test_search_overlay_area_uses_mainstream_dimensions() {
        let area = Rect::new(10, 5, 100, 30);
        let overlay = search_overlay_area(area).unwrap();

        assert_eq!(overlay, Rect::new(20, 9, 80, 21));
    }

    #[test]
    fn test_search_overlay_area_preserves_minimum_renderable_size() {
        let area = Rect::new(0, 0, 20, 8);
        let overlay = search_overlay_area(area).unwrap();

        assert_eq!(overlay, area);
    }

    #[test]
    fn test_search_overlay_area_caps_large_terminals() {
        let area = Rect::new(0, 0, 200, 100);
        let overlay = search_overlay_area(area).unwrap();

        assert_eq!(overlay, Rect::new(50, 30, 100, 40));
    }

    #[test]
    fn test_search_overlay_area_handles_offset_near_u16_max() {
        let area = Rect::new(u16::MAX - 100, u16::MAX - 40, 100, 40);
        let overlay = search_overlay_area(area).unwrap();

        assert_eq!(overlay, Rect::new(u16::MAX - 90, u16::MAX - 34, 80, 28));
    }

    #[test]
    fn test_search_overlay_area_skips_tiny_terminals() {
        assert!(search_overlay_area(Rect::new(0, 0, 19, 8)).is_none());
        assert!(search_overlay_area(Rect::new(0, 0, 20, 7)).is_none());
    }

    #[test]
    fn test_render_search_overlay_not_visible() {
        let state = SearchOverlayState::new();
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let theme = Theme::default();
        render_search_overlay(&state, area, &mut buf, &theme);
        // Should not panic, and no content rendered
    }

    #[test]
    fn test_render_search_overlay_visible() {
        let mut state = SearchOverlayState::new();
        state.open();
        state.set_results(vec![SearchResult {
            kind: SearchResultKind::FileContent {
                path: "test.rs".into(),
                line: 10,
                column: 1,
            },
            title: "test.rs".into(),
            preview: "fn main()".into(),
            score: 100,
        }]);

        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        let theme = Theme::default();
        render_search_overlay(&state, area, &mut buf, &theme);
        // Should not panic
    }

    #[test]
    fn test_render_results_clamps_stale_scroll_offset() {
        let mut state = SearchOverlayState::new();
        state.set_results(vec![
            SearchResult {
                kind: SearchResultKind::Command { id: "first".into() },
                title: "First command".into(),
                preview: "first preview".into(),
                score: 100,
            },
            SearchResult {
                kind: SearchResultKind::Command { id: "last".into() },
                title: "Last command".into(),
                preview: "last preview".into(),
                score: 90,
            },
        ]);
        state.scroll_offset = 99;

        let area = Rect::new(0, 0, 80, 4);
        let mut buf = Buffer::empty(area);

        render_results(
            &state,
            area,
            &mut buf,
            Color::White,
            Color::Gray,
            Color::Blue,
            Color::Black,
        );

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Last command"));
    }

    #[test]
    fn test_render_search_overlay_empty_title_names_global_search() {
        let mut state = SearchOverlayState::new();
        state.open();

        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        let theme = Theme::default();
        render_search_overlay(&state, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Global Search"));
        assert!(content.contains("Search files, commands, project items"));
        assert!(content.contains("Project"));
    }

    #[test]
    fn test_render_search_overlay_command_scope_uses_command_copy() {
        let mut state = SearchOverlayState::new();
        state.open_with_scope(SearchScope::Commands);

        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        let theme = Theme::default();
        render_search_overlay(&state, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Command Search"));
        assert!(content.contains(":"));
        assert!(content.contains("Search commands"));
        assert!(!content.contains("Search files, commands, project items"));
    }

    #[test]
    fn test_render_search_overlay_title_uses_singular_result_count() {
        let mut state = SearchOverlayState::new();
        state.open();
        state.set_results(vec![SearchResult {
            kind: SearchResultKind::FileContent {
                path: "test.rs".into(),
                line: 10,
                column: 1,
            },
            title: "test.rs".into(),
            preview: "fn main()".into(),
            score: 100,
        }]);

        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        let theme = Theme::default();
        render_search_overlay(&state, area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Global Search - 1 result"));
        assert!(!content.contains("Global Search - 1 results"));
    }

    #[test]
    fn test_render_search_overlay_small_area() {
        let mut state = SearchOverlayState::new();
        state.open();
        let area = Rect::new(0, 0, 10, 5);
        let mut buf = Buffer::empty(area);
        let theme = Theme::default();
        render_search_overlay(&state, area, &mut buf, &theme);
        // Should not panic even with small area
    }
}
