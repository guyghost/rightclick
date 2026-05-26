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
        let mut input = TextInputState::new(InputMode::SingleLine).with_placeholder("Search...");
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
        if self.selected > 0 {
            self.selected -= 1;
            if self.selected < self.scroll_offset {
                self.scroll_offset = self.selected;
            }
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if !self.results.is_empty() && self.selected < self.results.len() - 1 {
            self.selected += 1;
            if self.selected >= self.scroll_offset + 10 {
                self.scroll_offset = self.selected.saturating_sub(9);
            }
        }
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
        if viewport_height == 0 {
            return;
        }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + viewport_height {
            self.scroll_offset = self.selected - viewport_height + 1;
        }
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
    if !state.visible || area.width < 20 || area.height < 8 {
        return;
    }

    // Calculate overlay dimensions (80% width, 70% height, centered)
    let overlay_width = (area.width * 4 / 5).min(100);
    let overlay_height = (area.height * 7 / 10).min(40);
    let x = area.x + (area.width.saturating_sub(overlay_width)) / 2;
    let y = area.y + (area.height.saturating_sub(overlay_height)) / 2;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

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

    let title = if state.results.is_empty() {
        " Search ".to_string()
    } else {
        format!(" Search - {} results ", state.results.len())
    };

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

    // Layout: input (1) + scope tabs (1) + separator (1) + results (rest)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Search input
            Constraint::Length(1), // Scope tabs
            Constraint::Length(1), // Separator
            Constraint::Min(1),    // Results
        ])
        .split(inner);

    // Render search input line
    render_input_line(&state.input, chunks[0], buf, fg, primary);

    // Render scope tabs
    render_scope_tabs(state, chunks[1], buf, primary, muted);

    // Separator
    let sep = "─".repeat(chunks[2].width as usize);
    Paragraph::new(sep)
        .style(Style::default().fg(border_color))
        .render(chunks[2], buf);

    // Render results
    render_results(state, chunks[3], buf, fg, muted, primary, bg);
}

fn render_input_line(
    input: &TextInputState,
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
        "/ ",
        Style::default().fg(primary).add_modifier(Modifier::BOLD),
    );

    if text.is_empty() {
        let placeholder = input.placeholder();
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
    let scopes = [
        SearchScope::All,
        SearchScope::Files,
        SearchScope::Conversations,
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
        format!(
            "  {} result{}  Tab scope | Enter open | Esc close",
            state.results.len(),
            if state.results.len() == 1 { "" } else { "s" }
        ),
        Style::default().fg(muted),
    ));

    Paragraph::new(Line::from(spans)).render(area, buf);
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
            "Type to search files, commands, and conversations"
        } else {
            "No results. Try another scope or query"
        };
        Paragraph::new(msg)
            .style(Style::default().fg(muted))
            .alignment(Alignment::Center)
            .render(area, buf);
        return;
    }

    let viewport_height = area.height as usize;
    let scroll = state.scroll_offset;

    let items: Vec<ListItem> = state
        .results
        .iter()
        .enumerate()
        .skip(scroll)
        .take(viewport_height)
        .map(|(i, result)| {
            let is_selected = i == state.selected;

            let icon = match &result.kind {
                super::types::SearchResultKind::FileContent { .. } => "file",
                super::types::SearchResultKind::Conversation { .. } => "chat",
                super::types::SearchResultKind::Command { .. } => ">",
            };

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
                Span::styled(result.title.as_str(), title_style),
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

fn truncate_str(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", s.chars().take(max_len - 3).collect::<String>())
    } else {
        s.chars().take(max_len).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::types::SearchResultKind;

    #[test]
    fn test_overlay_new() {
        let state = SearchOverlayState::new();
        assert!(!state.visible);
        assert_eq!(state.scope, SearchScope::All);
        assert!(state.results.is_empty());
        assert_eq!(state.selected, 0);
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
    fn test_overlay_scope_cycle() {
        let mut state = SearchOverlayState::new();
        assert_eq!(state.scope, SearchScope::All);
        state.next_scope();
        assert_eq!(state.scope, SearchScope::Files);
        state.next_scope();
        assert_eq!(state.scope, SearchScope::Conversations);
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
        state.set_results(
            (0..12)
                .map(|idx| SearchResult {
                    kind: SearchResultKind::Command {
                        id: format!("cmd-{}", idx),
                    },
                    title: format!("Command {}", idx),
                    preview: String::new(),
                    score: 100 - idx,
                })
                .collect(),
        );

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
        state.selected = 15;
        state.scroll_offset = 0;
        state.ensure_visible(10);
        assert_eq!(state.scroll_offset, 6); // 15 - 10 + 1

        state.selected = 3;
        state.ensure_visible(10);
        assert_eq!(state.scroll_offset, 3); // scrolled up to show selected
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 8), "hello...");
        assert_eq!(truncate_str("ab", 2), "ab");
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
