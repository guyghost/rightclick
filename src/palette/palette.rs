//! Command palette widget for RightClick.
//!
//! This module provides a TUI component for displaying and interacting
//! with searchable command results using fuzzy matching.

use std::str::FromStr;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    StatefulWidget, Widget,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::core::models::Theme;
use crate::keymap::FocusContext;
use crate::palette::entries::{Category, PaletteEntry};
use crate::palette::fuzzy::{FuzzyMatcher, MatchResult};
use crate::theme::UiElement;
use crate::theme::style_for_ui_element;

const PALETTE_EMPTY_ACTION_HINT_SCOPED: &str = "Esc: Close | Tab: All contexts | ?: Toggle help";
const PALETTE_EMPTY_ACTION_HINT_ALL: &str = "Esc: Close | Tab: Current context | ?: Toggle help";
const PALETTE_NO_MATCH_ACTION_HINT_SCOPED: &str =
    "Backspace: Edit | Ctrl+U: Clear | Esc: Close | Tab: All | ?: Toggle help";
const PALETTE_NO_MATCH_ACTION_HINT_ALL: &str =
    "Backspace: Edit | Ctrl+U: Clear | Esc: Close | Tab: Current | ?: Toggle help";
const MIN_VISIBLE_RESULTS: usize = 1;
const MAX_RENDERABLE_VISIBLE_RESULTS: usize = u16::MAX as usize - 4;

/// Actions that can be returned from handling key events.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteAction {
    /// Select an entry
    Select(PaletteEntry),
    /// Close the palette
    Close,
    /// Toggle showing all contexts
    ToggleContextMode,
}

/// The command search widget.
#[derive(Debug, Clone)]
pub struct Palette {
    /// Current input text
    pub input: String,
    /// Cursor position in the input (byte index)
    pub cursor_pos: usize,
    /// All available entries
    pub all_entries: Vec<PaletteEntry>,
    /// Currently filtered (matched) entries
    pub filtered: Vec<MatchResult>,
    /// Index of the currently selected entry
    pub selected: usize,
    /// Scroll offset for the results list
    pub scroll_offset: usize,
    /// Maximum number of visible entries
    pub max_visible: usize,
    /// Whether to show entries from all contexts
    pub show_all_contexts: bool,
    /// Current focus context
    pub current_context: FocusContext,
    /// Fuzzy matcher
    matcher: FuzzyMatcher,
    /// Whether the palette is active/focused
    pub active: bool,
}

impl Default for Palette {
    fn default() -> Self {
        Self::new()
    }
}

impl Palette {
    /// Creates a new empty palette.
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor_pos: 0,
            all_entries: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            max_visible: 10,
            show_all_contexts: false,
            current_context: FocusContext::Global,
            matcher: FuzzyMatcher::new(),
            active: true,
        }
    }

    /// Creates a new palette with the given entries.
    pub fn with_entries(entries: Vec<PaletteEntry>) -> Self {
        let mut palette = Self::new();
        palette.set_entries(entries);
        palette
    }

    /// Sets the entries and re-filters.
    pub fn set_entries(&mut self, entries: Vec<PaletteEntry>) {
        self.all_entries = entries;
        self.filter(&self.input.clone());
    }

    /// Sets the current focus context and re-filters.
    pub fn set_context(&mut self, context: FocusContext) {
        self.current_context = context;
        self.filter(&self.input.clone());
    }

    /// Sets whether to show all contexts.
    pub fn set_show_all_contexts(&mut self, show_all: bool) {
        self.show_all_contexts = show_all;
        self.filter(&self.input.clone());
    }

    /// Toggles showing all contexts.
    pub fn toggle_context_mode(&mut self) {
        self.show_all_contexts = !self.show_all_contexts;
        self.filter(&self.input.clone());
    }

    /// Filters entries based on the query.
    pub fn filter(&mut self, query: &str) {
        self.input = query.to_string();
        self.cursor_pos = self.input.len();

        self.filtered = self.matcher.match_entries_with_context(
            &self.all_entries,
            query,
            self.current_context,
            self.show_all_contexts,
        );

        // Reset selection and scroll
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Updates the filter with the current input.
    fn update_filter(&mut self) {
        self.filtered = self.matcher.match_entries_with_context(
            &self.all_entries,
            &self.input,
            self.current_context,
            self.show_all_contexts,
        );
        self.adjust_scroll();
    }

    /// Moves the selection up.
    pub fn move_up(&mut self) {
        self.clamp_selection();
        if self.selected > 0 {
            self.selected -= 1;
        }
        self.adjust_scroll();
    }

    /// Moves the selection down.
    pub fn move_down(&mut self) {
        self.clamp_selection();
        if self.selected.saturating_add(1) < self.filtered.len() {
            self.selected += 1;
        }
        self.adjust_scroll();
    }

    /// Adjusts the scroll offset to keep the selected item visible.
    fn adjust_scroll(&mut self) {
        self.clamp_selection();
        if self.filtered.is_empty() {
            return;
        }

        let max_visible = self.visible_limit();
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset.saturating_add(max_visible) {
            self.scroll_offset = self.selected.saturating_sub(max_visible.saturating_sub(1));
        }

        self.scroll_offset = self
            .scroll_offset
            .min(self.filtered.len().saturating_sub(max_visible));
    }

    fn clamp_selection(&mut self) {
        if self.filtered.is_empty() {
            self.selected = 0;
            self.scroll_offset = 0;
            return;
        }

        let last_index = self.filtered.len() - 1;
        self.selected = self.selected.min(last_index);
        self.scroll_offset = self.scroll_offset.min(last_index);
    }

    fn visible_limit(&self) -> usize {
        self.max_visible
            .clamp(MIN_VISIBLE_RESULTS, MAX_RENDERABLE_VISIBLE_RESULTS)
            .min(self.filtered.len().max(MIN_VISIBLE_RESULTS))
    }

    fn page_step(&self) -> usize {
        self.visible_limit()
    }

    /// Returns the currently selected entry, if any.
    pub fn select(&self) -> Option<&PaletteEntry> {
        self.filtered.get(self.selected).map(|r| &r.entry)
    }

    /// Returns the currently selected match result, if any.
    pub fn selected_match(&self) -> Option<&MatchResult> {
        self.filtered.get(self.selected)
    }

    /// Clears the input and resets the filter.
    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor_pos = 0;
        self.filter("");
    }

    /// Handles a key event and returns an action if applicable.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<PaletteAction> {
        match key.code {
            KeyCode::Enter => {
                return self.select().cloned().map(PaletteAction::Select);
            }
            KeyCode::Esc => {
                return Some(PaletteAction::Close);
            }
            KeyCode::Up => {
                self.move_up();
            }
            KeyCode::Down => {
                self.move_down();
            }
            KeyCode::PageUp => {
                for _ in 0..self.page_step() {
                    self.move_up();
                }
            }
            KeyCode::PageDown => {
                for _ in 0..self.page_step() {
                    self.move_down();
                }
            }
            KeyCode::Home => {
                self.selected = 0;
                self.scroll_offset = 0;
            }
            KeyCode::End => {
                self.selected = self.filtered.len().saturating_sub(1);
                self.adjust_scroll();
            }
            KeyCode::Tab => {
                self.toggle_context_mode();
                return Some(PaletteAction::ToggleContextMode);
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    let char_before = self.input[..self.cursor_pos].chars().last();
                    if let Some(ch) = char_before {
                        self.input.remove(self.cursor_pos - ch.len_utf8());
                        self.cursor_pos -= ch.len_utf8();
                        self.update_filter();
                    }
                }
            }
            KeyCode::Delete => {
                if self.cursor_pos < self.input.len() {
                    let _ch = self.input[self.cursor_pos..].chars().next();
                    self.input.remove(self.cursor_pos);
                    // cursor_pos stays the same since we removed the character at current position
                    self.update_filter();
                }
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    let char_before = self.input[..self.cursor_pos].chars().last();
                    if let Some(ch) = char_before {
                        self.cursor_pos -= ch.len_utf8();
                    }
                }
            }
            KeyCode::Right => {
                if self.cursor_pos < self.input.len() {
                    let ch = self.input[self.cursor_pos..].chars().next();
                    if let Some(ch) = ch {
                        self.cursor_pos += ch.len_utf8();
                    }
                }
            }
            KeyCode::Char(c) => {
                // Check for Ctrl+U to clear input
                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'u' {
                    self.clear();
                } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'n' {
                    self.move_down();
                } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'p' {
                    self.move_up();
                } else {
                    self.input.insert(self.cursor_pos, c);
                    self.cursor_pos += c.len_utf8();
                    self.update_filter();
                }
            }
            _ => {}
        }

        None
    }

    /// Renders the palette to the buffer.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width < 10 || area.height < 5 {
            return;
        }

        // Clear the area first
        Clear.render(area, buf);

        // Create a centered popup
        let popup_width = ((u32::from(area.width) * 4 / 5) as u16)
            .max(40)
            .min(area.width - 4);
        let popup_height =
            (self.max_visible.min(MAX_RENDERABLE_VISIBLE_RESULTS) as u16 + 4).min(area.height - 2);

        let popup_area = Rect {
            x: area.x + (area.width - popup_width) / 2,
            y: area.y + (area.height - popup_height) / 2,
            width: popup_width,
            height: popup_height,
        };

        // Draw the main block
        let block_style = style_for_ui_element(theme, UiElement::Popup);
        let block = Block::default()
            .title(self.build_title(theme, popup_width))
            .borders(Borders::ALL)
            .border_style(block_style);

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Split into input and results areas
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(inner);

        // Render input field
        self.render_input(chunks[0], buf, theme);

        // Render results
        self.render_results(chunks[1], buf, theme);
    }

    /// Builds the title for the palette.
    fn build_title(&self, theme: &Theme, width: u16) -> Line<'_> {
        let (context_text, hint_text) =
            palette_title_context_and_hint(self.show_all_contexts, width);

        Line::from(vec![
            Span::styled(
                "Command Search",
                style_for_ui_element(theme, UiElement::Primary).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                context_text,
                style_for_ui_element(theme, UiElement::MutedText),
            ),
            Span::styled(hint_text, style_for_ui_element(theme, UiElement::MutedText)),
        ])
    }

    /// Renders the input field.
    fn render_input(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let input_style = style_for_ui_element(theme, UiElement::Input);
        let placeholder_style = style_for_ui_element(theme, UiElement::InputPlaceholder);

        // Draw input background
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(input_style);
                    cell.set_symbol(" ");
                }
            }
        }

        // Draw the input text or placeholder
        let display_text = if self.input.is_empty() {
            "Search commands..."
        } else {
            &self.input
        };

        let text_style = if self.input.is_empty() {
            placeholder_style
        } else {
            input_style
        };

        // Render the text
        let text_area = area.inner(Margin::new(1, 1));
        let text = Paragraph::new(display_text).style(text_style);
        text.render(text_area, buf);

        // Draw cursor if active
        if self.active && !self.input.is_empty() {
            let cursor_x = text_area
                .x
                .saturating_add(cursor_display_width(&self.input, self.cursor_pos) as u16);
            if cursor_x < text_area.right() {
                if let Some(cell) = buf.cell_mut((cursor_x, text_area.y)) {
                    let cursor_style = style_for_ui_element(theme, UiElement::Text)
                        .bg(Color::from_str(&theme.colors.cursor).unwrap_or(Color::White));
                    cell.set_style(cursor_style);
                }
            }
        }
    }

    /// Renders the results list.
    fn render_results(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        if self.filtered.is_empty() {
            self.render_empty_state(area, buf, theme);
            return;
        }

        let item_height = 2u16; // Each item takes 2 rows
        let visible_capacity = area.height.div_ceil(item_height) as usize;
        let max_visible = self.visible_limit();
        let visible_count = max_visible.min(self.filtered.len()).min(visible_capacity);

        // Calculate visible range
        let start_idx = self
            .scroll_offset
            .min(self.filtered.len().saturating_sub(1));
        let end_idx = (start_idx + visible_count).min(self.filtered.len());

        // Render visible items
        for (i, match_result) in self.filtered[start_idx..end_idx].iter().enumerate() {
            let item_idx = start_idx + i;
            let is_selected = item_idx == self.selected;
            let y = area.y.saturating_add(i as u16 * item_height);
            let remaining_height = area.y.saturating_add(area.height).saturating_sub(y);

            let item_area = Rect {
                x: area.x,
                y,
                width: area.width,
                height: item_height.min(remaining_height),
            };

            self.render_entry(item_area, buf, theme, match_result, is_selected);
        }

        // Render scrollbar if needed
        if self.filtered.len() > visible_count {
            let scrollbar_area = Rect {
                x: area.right().saturating_sub(1),
                y: area.y,
                width: 1,
                height: area.height,
            };

            let mut scrollbar_state = ScrollbarState::new(self.filtered.len())
                .position(self.selected)
                .viewport_content_length(visible_count.min(max_visible));

            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            scrollbar.render(scrollbar_area, buf, &mut scrollbar_state);
        }
    }

    /// Renders a single entry.
    fn render_entry(
        &self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        match_result: &MatchResult,
        is_selected: bool,
    ) {
        let entry = &match_result.entry;

        // Determine styles
        let bg_style = if is_selected {
            style_for_ui_element(theme, UiElement::ActiveItem)
        } else {
            style_for_ui_element(theme, UiElement::InactiveItem)
        };

        // Fill background
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(bg_style);
                    cell.set_symbol(" ");
                }
            }
        }

        // Category color
        let category_color = self.category_color(&entry.category, theme);
        let category_style = Style::default().fg(category_color);

        // Build the first line: [CATEGORY] Name with highlights
        let mut first_line_spans = Vec::new();

        // Category prefix
        first_line_spans.push(Span::styled(
            format!("[{:>4}] ", entry.category.prefix()),
            category_style,
        ));

        // Name with match highlighting
        let name_spans =
            self.highlight_matches(&entry.name, &match_result.match_ranges, theme, is_selected);
        first_line_spans.extend(name_spans);

        // Key binding (right-aligned)
        if !entry.key.is_empty() {
            let key_style = style_for_ui_element(theme, UiElement::Secondary);
            first_line_spans.push(Span::raw(" "));
            first_line_spans.push(Span::styled(format!("({})", entry.key), key_style));
        }

        // Render first line
        let first_line = Line::from(first_line_spans);
        let first_line_area = Rect {
            x: area.x.saturating_add(1),
            y: area.y,
            width: area.width.saturating_sub(2),
            height: 1,
        };
        Paragraph::new(Text::from(vec![first_line])).render(first_line_area, buf);

        // Render description on second line (if present)
        if area.height > 1 && !entry.description.is_empty() {
            let desc_style = style_for_ui_element(theme, UiElement::MutedText);
            let desc_line = Line::from(vec![
                Span::raw("      "), // Align with name
                Span::styled(entry.description.clone(), desc_style),
            ]);
            let desc_area = Rect {
                x: area.x.saturating_add(1),
                y: area.y.saturating_add(1),
                width: area.width.saturating_sub(2),
                height: 1,
            };
            Paragraph::new(Text::from(vec![desc_line])).render(desc_area, buf);
        }
    }

    /// Renders the empty state when no results are found.
    fn render_empty_state(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let text_style = style_for_ui_element(theme, UiElement::MutedText);
        let message = palette_empty_state_message(
            &self.input,
            area.width,
            !self.all_entries.is_empty(),
            self.show_all_contexts,
        );

        let text = Paragraph::new(message)
            .style(text_style)
            .alignment(Alignment::Center);

        text.render(area, buf);
    }

    /// Highlights matched characters in the text.
    fn highlight_matches(
        &self,
        text: &str,
        match_ranges: &[(usize, usize)],
        theme: &Theme,
        is_selected: bool,
    ) -> Vec<Span<'_>> {
        if match_ranges.is_empty() {
            return vec![Span::raw(text.to_string())];
        }

        let highlight_style = if is_selected {
            Style::default()
                .fg(Color::from_str(&theme.colors.highlight).unwrap_or(Color::Yellow))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::from_str(&theme.colors.primary).unwrap_or(Color::Cyan))
                .add_modifier(Modifier::BOLD)
        };

        let normal_style = if is_selected {
            style_for_ui_element(theme, UiElement::ActiveItem)
        } else {
            style_for_ui_element(theme, UiElement::Text)
        };

        let mut spans = Vec::new();
        let mut last_end = 0;

        for &(start, end) in match_ranges {
            let Some((start, end)) = char_range_to_byte_range(text, start, end) else {
                continue;
            };

            // Add text before the match
            if start > last_end {
                spans.push(Span::styled(
                    text[last_end..start].to_string(),
                    normal_style,
                ));
            }

            // Add the matched text with highlight
            spans.push(Span::styled(text[start..end].to_string(), highlight_style));
            last_end = end;
        }

        // Add remaining text after the last match
        if last_end < text.len() {
            spans.push(Span::styled(text[last_end..].to_string(), normal_style));
        }

        spans
    }

    /// Returns the color for a category.
    fn category_color(&self, category: &Category, theme: &Theme) -> Color {
        match category {
            Category::Navigation => Color::from_str(&theme.colors.info).unwrap_or(Color::Cyan),
            Category::Actions => Color::from_str(&theme.colors.primary).unwrap_or(Color::Blue),
            Category::View => Color::from_str(&theme.colors.secondary).unwrap_or(Color::Magenta),
            Category::Search => Color::from_str(&theme.colors.warning).unwrap_or(Color::Yellow),
            Category::Edit => Color::from_str(&theme.colors.success).unwrap_or(Color::Green),
            Category::Git => Color::from_str(&theme.colors.added).unwrap_or(Color::Green),
            Category::System => Color::from_str(&theme.colors.error).unwrap_or(Color::Red),
        }
    }

    /// Returns the number of filtered results.
    pub fn result_count(&self) -> usize {
        self.filtered.len()
    }

    /// Returns true if the palette has any results.
    pub fn has_results(&self) -> bool {
        !self.filtered.is_empty()
    }

    /// Sets the maximum number of visible entries.
    pub fn set_max_visible(&mut self, max_visible: usize) {
        self.max_visible = max_visible.clamp(MIN_VISIBLE_RESULTS, MAX_RENDERABLE_VISIBLE_RESULTS);
        self.adjust_scroll();
    }

    /// Sets whether the palette is active.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }
}

fn palette_empty_state_message(
    input: &str,
    width: u16,
    has_commands: bool,
    show_all_contexts: bool,
) -> String {
    let hint = palette_empty_action_hint(width, !input.is_empty(), show_all_contexts);

    if input.is_empty() {
        if has_commands {
            let heading = if show_all_contexts {
                "No commands available"
            } else {
                "No commands in this context"
            };
            return format!("{heading}\n\n{hint}");
        }

        format!(
            "No commands yet\n\n{}\n\nCommands appear after plugins finish loading.",
            hint
        )
    } else {
        let input = truncate_query(input, 40);
        format!("No command matches \"{}\"\n\n{}", input, hint)
    }
}

fn palette_empty_action_hint(width: u16, has_input: bool, show_all_contexts: bool) -> &'static str {
    let width = width as usize;
    if has_input {
        let full_hint = if show_all_contexts {
            PALETTE_NO_MATCH_ACTION_HINT_ALL
        } else {
            PALETTE_NO_MATCH_ACTION_HINT_SCOPED
        };
        let context_hint = if show_all_contexts {
            "Tab: Current context"
        } else {
            "Tab: All contexts"
        };
        let short_context_hint = if show_all_contexts {
            "Tab: Current"
        } else {
            "Tab: All"
        };
        let edit_context_hint = if show_all_contexts {
            "Edit/Clear | Tab: Current"
        } else {
            "Edit/Clear | Tab: All"
        };
        let detailed_context_hint = if show_all_contexts {
            "Backspace: Edit | Ctrl+U: Clear | Esc: Close | Tab: Current"
        } else {
            "Backspace: Edit | Ctrl+U: Clear | Esc: Close | Tab: All"
        };

        [
            full_hint,
            detailed_context_hint,
            "Backspace: Edit | Ctrl+U: Clear | Esc: Close | Tab",
            "Backspace: Edit | Ctrl+U: Clear | Esc: Close",
            "Backspace/Ctrl+U  Esc/Tab/?",
            edit_context_hint,
            context_hint,
            short_context_hint,
            "Edit/Clear  Close",
            "Esc",
        ]
        .into_iter()
        .find(|hint| hint.width() <= width)
        .unwrap_or("")
    } else {
        let full_hint = if show_all_contexts {
            PALETTE_EMPTY_ACTION_HINT_ALL
        } else {
            PALETTE_EMPTY_ACTION_HINT_SCOPED
        };
        let context_hint = if show_all_contexts {
            "Tab: Current context"
        } else {
            "Tab: All contexts"
        };
        let close_context_hint = if show_all_contexts {
            "Esc: Close | Tab: Current"
        } else {
            "Esc: Close | Tab: All"
        };
        let short_context_hint = if show_all_contexts {
            "Tab: Current"
        } else {
            "Tab: All"
        };
        let short_close_context_hint = if show_all_contexts {
            "Esc | Tab: Current"
        } else {
            "Esc | Tab: All"
        };

        [
            full_hint,
            close_context_hint,
            short_close_context_hint,
            context_hint,
            short_context_hint,
            "Esc: Close | Tab",
            "Esc/Tab/?",
            "Esc",
            "",
        ]
        .into_iter()
        .find(|hint| hint.width() <= width)
        .unwrap_or("")
    }
}

fn palette_title_context_and_hint(
    show_all_contexts: bool,
    width: u16,
) -> (&'static str, &'static str) {
    let width = width as usize;
    let base = "Command Search";

    if show_all_contexts {
        [
            (" (all contexts)", " (Tab: contexts)"),
            (" (all contexts)", ""),
            (" (all)", ""),
        ]
        .into_iter()
        .find(|(context, hint)| base.width() + context.width() + hint.width() <= width)
        .unwrap_or(("", ""))
    } else if base.width() + " (Tab: contexts)".width() <= width {
        ("", " (Tab: contexts)")
    } else {
        ("", "")
    }
}

fn truncate_query(input: &str, max_len: usize) -> String {
    if input.width() <= max_len {
        input.to_string()
    } else if max_len > 3 {
        let mut output = String::new();
        let mut width = 0;
        for ch in input.chars() {
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
        for ch in input.chars() {
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

fn char_range_to_byte_range(text: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    if start >= end {
        return None;
    }

    let start_byte = char_index_to_byte_index(text, start)?;
    let end_byte = char_index_to_byte_index(text, end).unwrap_or(text.len());

    if start_byte >= end_byte {
        None
    } else {
        Some((start_byte, end_byte))
    }
}

fn char_index_to_byte_index(text: &str, char_index: usize) -> Option<usize> {
    if char_index == text.chars().count() {
        return Some(text.len());
    }

    text.char_indices().nth(char_index).map(|(idx, _)| idx)
}

fn cursor_display_width(input: &str, cursor_pos: usize) -> usize {
    let mut byte_pos = cursor_pos.min(input.len());
    while byte_pos > 0 && !input.is_char_boundary(byte_pos) {
        byte_pos -= 1;
    }
    input[..byte_pos].width()
}

impl Widget for &Palette {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Use default theme if none provided
        let default_theme = Theme::default();
        self.render(area, buf, &default_theme);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_palette() -> Palette {
        let entries = vec![
            PaletteEntry::minimal("file.open", "Open File", Category::Navigation)
                .with_description("Open a file in the editor"),
            PaletteEntry::minimal("file.save", "Save File", Category::Actions)
                .with_description("Save the current file"),
            PaletteEntry::minimal("git.commit", "Git Commit", Category::Git)
                .with_description("Create a git commit"),
        ];

        Palette::with_entries(entries)
    }

    #[test]
    fn test_palette_new() {
        let palette = Palette::new();
        assert!(palette.input.is_empty());
        assert_eq!(palette.cursor_pos, 0);
        assert!(palette.all_entries.is_empty());
        assert!(palette.filtered.is_empty());
        assert_eq!(palette.selected, 0);
        assert!(!palette.show_all_contexts);
    }

    #[test]
    fn test_palette_with_entries() {
        let entries = vec![PaletteEntry::minimal("test", "Test", Category::Actions)];

        let palette = Palette::with_entries(entries);
        assert_eq!(palette.all_entries.len(), 1);
        assert_eq!(palette.filtered.len(), 1);
    }

    #[test]
    fn test_palette_render_handles_offset_area_near_u16_max() {
        let palette = test_palette();
        let theme = Theme::default();
        let area = Rect::new(u16::MAX - 100, u16::MAX - 20, 100, 20);
        let mut buf = Buffer::empty(area);

        palette.render(area, &mut buf, &theme);
    }

    #[test]
    fn test_set_entries() {
        let mut palette = Palette::new();
        let entries = vec![PaletteEntry::minimal("test", "Test", Category::Actions)];

        palette.set_entries(entries);
        assert_eq!(palette.all_entries.len(), 1);
        assert_eq!(palette.filtered.len(), 1);
    }

    #[test]
    fn test_filter() {
        let mut palette = test_palette();
        palette.filter("open");

        assert_eq!(palette.input, "open");
        // Should match "Open File"
        assert!(palette.filtered.iter().any(|r| r.entry.name == "Open File"));
    }

    #[test]
    fn test_move_up_down() {
        let mut palette = test_palette();

        assert_eq!(palette.selected, 0);

        palette.move_down();
        assert_eq!(palette.selected, 1);

        palette.move_down();
        assert_eq!(palette.selected, 2);

        palette.move_down(); // Should not go past end
        assert_eq!(palette.selected, 2);

        palette.move_up();
        assert_eq!(palette.selected, 1);

        palette.move_up();
        assert_eq!(palette.selected, 0);

        palette.move_up(); // Should not go below 0
        assert_eq!(palette.selected, 0);
    }

    #[test]
    fn test_select() {
        let mut palette = test_palette();

        let selected = palette.select();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().name, "Open File");

        palette.move_down();
        let selected = palette.select();
        assert_eq!(selected.unwrap().name, "Save File");
    }

    #[test]
    fn test_clear() {
        let mut palette = test_palette();
        palette.filter("test");
        palette.move_down();

        palette.clear();

        assert!(palette.input.is_empty());
        assert_eq!(palette.cursor_pos, 0);
        assert_eq!(palette.selected, 0);
    }

    #[test]
    fn test_context_filtering() {
        let entries = vec![
            PaletteEntry::minimal("file.open", "Open File", Category::Navigation)
                .with_context(FocusContext::Global),
            PaletteEntry::minimal("git.commit", "Git Commit", Category::Git)
                .with_context(FocusContext::GitStatus),
        ];

        let mut palette = Palette::with_entries(entries);
        palette.set_context(FocusContext::Workspace);

        // Without show_all, context-compatible entries should be visible
        assert!(palette.filtered.iter().any(|m| m.entry.name == "Open File"));
        assert!(
            !palette
                .filtered
                .iter()
                .any(|m| m.entry.name == "Git Commit")
        );

        // With show_all, all entries should be visible
        palette.set_show_all_contexts(true);
        assert_eq!(palette.filtered.len(), 2);
    }

    #[test]
    fn test_handle_key_typing() {
        let mut palette = Palette::new();

        palette.handle_key(KeyEvent::from(KeyCode::Char('h')));
        palette.handle_key(KeyEvent::from(KeyCode::Char('i')));

        assert_eq!(palette.input, "hi");
        assert_eq!(palette.cursor_pos, 2);
    }

    #[test]
    fn test_cursor_display_width_handles_unicode_input() {
        assert_eq!(cursor_display_width("éa", "éa".len()), 2);
        assert_eq!(cursor_display_width("検索a", "検".len()), 2);
        assert_eq!(cursor_display_width("検索a", "検索a".len()), 5);
        assert_eq!(cursor_display_width("éa", 1), 0);
    }

    #[test]
    fn test_render_input_positions_cursor_by_display_width() {
        let mut palette = Palette::new();
        palette.input = "éa".to_string();
        palette.cursor_pos = palette.input.len();
        palette.active = true;

        let theme = Theme::default();
        let area = Rect::new(0, 0, 20, 3);
        let mut buf = Buffer::empty(area);

        palette.render_input(area, &mut buf, &theme);

        let cursor_color = Color::from_str(&theme.colors.cursor).unwrap_or(Color::White);
        assert_eq!(buf.cell((3, 1)).unwrap().style().bg, Some(cursor_color));
        assert_ne!(buf.cell((4, 1)).unwrap().style().bg, Some(cursor_color));
    }

    #[test]
    fn test_render_results_zero_area_no_panic() {
        let mut palette = test_palette();
        palette.set_max_visible(1);

        let theme = Theme::default();
        let area = Rect::new(0, 0, 1, 1);
        let mut buf = Buffer::empty(area);

        palette.render_results(Rect::new(0, 0, 0, 0), &mut buf, &theme);
    }

    #[test]
    fn test_render_results_clips_entry_to_available_height() {
        let palette = test_palette();
        let theme = Theme::default();
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);

        palette.render_results(Rect::new(0, 0, 40, 1), &mut buf, &theme);

        let first_row: String = (0..40)
            .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
            .collect();
        let second_row: String = (0..40)
            .map(|x| buf.cell((x, 1)).unwrap().symbol().to_string())
            .collect();

        assert!(first_row.contains("Open File"));
        assert!(!second_row.contains("Open a file"));
    }

    #[test]
    fn test_handle_key_backspace() {
        let mut palette = Palette::new();
        palette.input = "hi".to_string();
        palette.cursor_pos = 2;

        palette.handle_key(KeyEvent::from(KeyCode::Backspace));

        assert_eq!(palette.input, "h");
        assert_eq!(palette.cursor_pos, 1);
    }

    #[test]
    fn test_handle_key_enter() {
        let mut palette = test_palette();

        let action = palette.handle_key(KeyEvent::from(KeyCode::Enter));

        assert!(matches!(action, Some(PaletteAction::Select(_))));
    }

    #[test]
    fn test_handle_key_esc() {
        let mut palette = test_palette();

        let action = palette.handle_key(KeyEvent::from(KeyCode::Esc));

        assert_eq!(action, Some(PaletteAction::Close));
    }

    #[test]
    fn test_handle_key_tab() {
        let mut palette = test_palette();

        let action = palette.handle_key(KeyEvent::from(KeyCode::Tab));

        assert_eq!(action, Some(PaletteAction::ToggleContextMode));
        assert!(palette.show_all_contexts);
    }

    #[test]
    fn test_toggle_context_mode() {
        let mut palette = test_palette();

        assert!(!palette.show_all_contexts);
        palette.toggle_context_mode();
        assert!(palette.show_all_contexts);
        palette.toggle_context_mode();
        assert!(!palette.show_all_contexts);
    }

    #[test]
    fn test_max_visible() {
        let mut palette = test_palette();

        palette.set_max_visible(2);
        assert_eq!(palette.max_visible, 2);
    }

    #[test]
    fn test_max_visible_clamps_to_renderable_range() {
        let mut palette = test_palette();

        palette.set_max_visible(0);
        assert_eq!(palette.max_visible, MIN_VISIBLE_RESULTS);

        palette.set_max_visible(usize::MAX);
        assert_eq!(palette.max_visible, MAX_RENDERABLE_VISIBLE_RESULTS);
    }

    #[test]
    fn test_adjust_scroll_tolerates_zero_max_visible() {
        let mut palette = test_palette();
        palette.max_visible = 0;
        palette.selected = 2;

        palette.adjust_scroll();

        assert_eq!(palette.scroll_offset, 2);
    }

    #[test]
    fn test_navigation_clamps_stale_public_selection() {
        let mut palette = test_palette();
        palette.selected = usize::MAX;
        palette.scroll_offset = usize::MAX;

        palette.move_down();

        assert_eq!(palette.selected, 2);
        assert_eq!(palette.scroll_offset, 0);
    }

    #[test]
    fn test_page_down_clamps_extreme_public_max_visible() {
        let mut palette = test_palette();
        palette.max_visible = usize::MAX;

        palette.handle_key(KeyEvent::from(KeyCode::PageDown));

        assert_eq!(palette.selected, 2);
        assert_eq!(palette.scroll_offset, 0);
    }

    #[test]
    fn test_update_filter_clamps_stale_scroll_offset() {
        let mut palette = test_palette();
        palette.selected = usize::MAX;
        palette.scroll_offset = usize::MAX;

        palette.handle_key(KeyEvent::from(KeyCode::Char('o')));

        assert!(palette.selected < palette.filtered.len());
        assert_eq!(palette.scroll_offset, 0);
    }

    #[test]
    fn test_render_tolerates_extreme_max_visible() {
        let mut palette = test_palette();
        palette.max_visible = usize::MAX;

        let theme = Theme::default();
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);

        palette.render(area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Command Search"));
    }

    #[test]
    fn test_palette_title_hint_compacts_for_narrow_widths() {
        assert_eq!(
            palette_title_context_and_hint(false, 80),
            ("", " (Tab: contexts)")
        );
        assert_eq!(palette_title_context_and_hint(false, 26), ("", ""));
        assert_eq!(
            palette_title_context_and_hint(true, 80),
            (" (all contexts)", " (Tab: contexts)")
        );
        assert_eq!(
            palette_title_context_and_hint(true, 40),
            (" (all contexts)", "")
        );
        assert_eq!(palette_title_context_and_hint(true, 22), (" (all)", ""));
        assert_eq!(palette_title_context_and_hint(true, 10), ("", ""));
    }

    #[test]
    fn test_palette_title_render_hides_context_hint_when_narrow() {
        let palette = test_palette();
        let theme = Theme::default();
        let area = Rect::new(0, 0, 30, 20);
        let mut buf = Buffer::empty(area);

        palette.render(area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Command Search"));
        assert!(!content.contains("(Tab: contexts)"));
    }

    #[test]
    fn test_result_count() {
        let palette = test_palette();

        assert_eq!(palette.result_count(), 3);
        assert!(palette.has_results());
    }

    #[test]
    fn test_palette_empty_state_message_points_to_next_actions() {
        let empty = palette_empty_state_message("", 80, false, false);
        assert!(empty.contains("No commands yet"));
        assert!(empty.contains(PALETTE_EMPTY_ACTION_HINT_SCOPED));
        assert!(!empty.contains("Esc  Close palette"));
        assert!(!empty.contains("Tab  Toggle contexts"));
        assert!(empty.contains("Commands appear after plugins finish loading"));

        let context_empty = palette_empty_state_message("", 80, true, false);
        assert!(context_empty.contains("No commands in this context"));
        assert!(context_empty.contains("Tab: All contexts"));
        assert!(!context_empty.contains("Commands appear after plugins finish loading"));

        let all_contexts_empty = palette_empty_state_message("", 80, true, true);
        assert!(all_contexts_empty.contains("No commands available"));
        assert!(all_contexts_empty.contains("Tab: Current context"));

        let no_match = palette_empty_state_message("deploy", 80, true, false);
        assert!(no_match.contains("No command matches \"deploy\""));
        assert!(no_match.contains(PALETTE_NO_MATCH_ACTION_HINT_SCOPED));
        assert!(!no_match.contains("Backspace  Edit search"));
        assert!(!no_match.contains("Ctrl+U  Clear search"));
        assert!(!no_match.contains("Esc  Close palette"));
        assert!(!no_match.contains("Tab  Toggle contexts"));

        let no_match_all_contexts = palette_empty_state_message("deploy", 80, true, true);
        assert!(no_match_all_contexts.contains(PALETTE_NO_MATCH_ACTION_HINT_ALL));

        let truncated = palette_empty_state_message(
            "abcdefghijklmnopqrstuvwxyz0123456789abcdef",
            80,
            true,
            false,
        );
        assert!(
            truncated.contains("No command matches \"abcdefghijklmnopqrstuvwxyz0123456789a...\"")
        );
    }

    #[test]
    fn test_palette_empty_action_hint_compacts_for_narrow_widths() {
        assert_eq!(palette_empty_action_hint(2, false, false), "");
        assert_eq!(palette_empty_action_hint(3, false, false), "Esc");
        assert_eq!(palette_empty_action_hint(9, false, false), "Tab: All");
        assert_eq!(
            palette_empty_action_hint(17, false, false),
            "Esc | Tab: All"
        );
        assert_eq!(
            palette_empty_action_hint(21, false, false),
            "Esc: Close | Tab: All"
        );
        assert_eq!(
            palette_empty_action_hint(28, false, false),
            "Esc: Close | Tab: All"
        );
        assert_eq!(palette_empty_action_hint(17, false, true), "Tab: Current");
        assert_eq!(
            palette_empty_action_hint(18, false, true),
            "Esc | Tab: Current"
        );
        assert_eq!(
            palette_empty_action_hint(25, false, true),
            "Esc: Close | Tab: Current"
        );
        assert_eq!(
            palette_empty_action_hint(80, false, false),
            PALETTE_EMPTY_ACTION_HINT_SCOPED
        );
        assert_eq!(
            palette_empty_action_hint(80, false, true),
            PALETTE_EMPTY_ACTION_HINT_ALL
        );

        assert_eq!(palette_empty_action_hint(3, true, false), "Esc");
        assert_eq!(
            palette_empty_action_hint(17, true, false),
            "Tab: All contexts"
        );
        assert_eq!(
            palette_empty_action_hint(18, true, false),
            "Tab: All contexts"
        );
        assert_eq!(
            palette_empty_action_hint(27, true, false),
            "Backspace/Ctrl+U  Esc/Tab/?"
        );
        assert_eq!(
            palette_empty_action_hint(55, true, false),
            "Backspace: Edit | Ctrl+U: Clear | Esc: Close | Tab: All"
        );
        assert_eq!(
            palette_empty_action_hint(59, true, true),
            "Backspace: Edit | Ctrl+U: Clear | Esc: Close | Tab: Current"
        );
        assert_eq!(
            palette_empty_action_hint(80, true, false),
            PALETTE_NO_MATCH_ACTION_HINT_SCOPED
        );
        assert_eq!(
            palette_empty_action_hint(80, true, true),
            PALETTE_NO_MATCH_ACTION_HINT_ALL
        );
    }

    #[test]
    fn test_palette_empty_action_hint_never_exceeds_width() {
        for width in 0..=80 {
            let empty = palette_empty_action_hint(width, false, false);
            let all_empty = palette_empty_action_hint(width, false, true);
            let no_match = palette_empty_action_hint(width, true, false);
            let all_no_match = palette_empty_action_hint(width, true, true);

            assert!(empty.width() <= width as usize);
            assert!(all_empty.width() <= width as usize);
            assert!(no_match.width() <= width as usize);
            assert!(all_no_match.width() <= width as usize);
        }
    }

    #[test]
    fn test_truncate_query_handles_unicode_boundaries() {
        assert_eq!(truncate_query("deploy", 40), "deploy");
        assert_eq!(truncate_query("ééééé", 4), "é...");
        assert_eq!(truncate_query("abc", 2), "ab");
        assert_eq!(truncate_query("検索command", 8), "検索c...");
        assert_eq!(truncate_query("検索", 3), "検");
        assert_eq!(truncate_query("abc", 0), "");
    }

    #[test]
    fn test_highlight_matches_handles_unicode_ranges() {
        let palette = Palette::new();
        let theme = Theme::default();

        let spans = palette.highlight_matches("Éclair", &[(0, 1)], &theme, false);
        let rendered: String = spans.iter().map(|span| span.content.as_ref()).collect();

        assert_eq!(rendered, "Éclair");
        assert_eq!(spans[0].content.as_ref(), "É");
    }

    #[test]
    fn test_palette_empty_state_render_includes_action_hints() {
        let palette = Palette::new();
        let theme = Theme::default();
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);

        palette.render(area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No commands yet"));
        assert!(content.contains(PALETTE_EMPTY_ACTION_HINT_SCOPED));
    }

    #[test]
    fn test_palette_empty_state_render_uses_compact_hints_when_narrow() {
        let palette = Palette::new();
        let theme = Theme::default();
        let area = Rect::new(0, 0, 44, 20);
        let mut buf = Buffer::empty(area);

        palette.render(area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No commands yet"));
        assert!(content.contains("Esc: Close | Tab"));
        assert!(!content.contains(PALETTE_EMPTY_ACTION_HINT_SCOPED));
    }

    #[test]
    fn test_selected_match() {
        let mut palette = test_palette();

        let match_result = palette.selected_match();
        assert!(match_result.is_some());
        assert_eq!(match_result.unwrap().entry.name, "Open File");

        palette.move_down();
        let match_result = palette.selected_match();
        assert_eq!(match_result.unwrap().entry.name, "Save File");
    }
}
