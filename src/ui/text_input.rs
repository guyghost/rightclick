//! TextInput widget for ratatui rendering
//!
//! This module provides `TextInputWidget`, a ratatui widget that renders
//! a `TextInputState` with cursor, placeholder, and styling support.

use crate::core::models::Theme;
use crate::core::models::text_input::{InputMode, TextInputState};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use std::str::FromStr;

/// A widget that renders a `TextInputState` into a ratatui buffer
pub struct TextInputWidget<'a> {
    state: &'a TextInputState,
    theme: &'a Theme,
    /// Optional title for a bordered block
    title: Option<String>,
    /// Whether to show a border
    bordered: bool,
}

impl<'a> TextInputWidget<'a> {
    /// Create a new TextInputWidget
    pub fn new(state: &'a TextInputState, theme: &'a Theme) -> Self {
        Self {
            state,
            theme,
            title: None,
            bordered: false,
        }
    }

    /// Add a title (implies bordered)
    pub fn with_title<S: Into<String>>(mut self, title: S) -> Self {
        self.title = Some(title.into());
        self.bordered = true;
        self
    }

    /// Set bordered mode
    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    /// Render the widget to the buffer
    pub fn render(self, area: Rect, buf: &mut Buffer) {
        match self.state.mode() {
            InputMode::SingleLine => self.render_single_line(area, buf),
            InputMode::MultiLine => self.render_multi_line(area, buf),
        }
    }

    fn render_single_line(&self, area: Rect, buf: &mut Buffer) {
        let fg = Color::from_str(&self.theme.colors.foreground).unwrap_or(Color::White);
        let muted = Color::from_str(&self.theme.colors.muted).unwrap_or(Color::DarkGray);
        let error_color = Color::from_str(&self.theme.colors.error).unwrap_or(Color::Red);
        let border_color = Color::from_str(&self.theme.colors.border).unwrap_or(Color::Gray);
        let cursor_color = Color::from_str(&self.theme.colors.cursor).unwrap_or(Color::White);

        let has_error = self.state.error().is_some();
        let active_border_color = if has_error {
            error_color
        } else if self.state.is_active() {
            Color::from_str(&self.theme.colors.primary).unwrap_or(Color::Blue)
        } else {
            border_color
        };

        // Build block
        let inner_area = if self.bordered || self.title.is_some() {
            let mut block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(active_border_color));
            if let Some(ref title) = self.title {
                block = block.title(title.as_str());
            }
            let inner = block.inner(area);
            block.render(area, buf);
            inner
        } else {
            area
        };

        if inner_area.height == 0 || inner_area.width == 0 {
            return;
        }

        // Render content or placeholder
        if self.state.is_empty() && !self.state.is_active() {
            let placeholder =
                Paragraph::new(self.state.placeholder()).style(Style::default().fg(muted));
            placeholder.render(inner_area, buf);
        } else if self.state.is_empty() && self.state.is_active() {
            // Show placeholder in muted + cursor at start
            let placeholder_text = self.state.placeholder();
            if !placeholder_text.is_empty() {
                let spans = vec![
                    Span::styled(" ", Style::default().bg(cursor_color).fg(Color::Black)),
                    Span::styled(
                        placeholder_after_cursor(placeholder_text),
                        Style::default().fg(muted),
                    ),
                ];
                let line = Line::from(spans);
                Paragraph::new(line).render(inner_area, buf);
            } else {
                // Just show cursor block
                let spans = vec![Span::styled(
                    " ",
                    Style::default().bg(cursor_color).fg(Color::Black),
                )];
                Paragraph::new(Line::from(spans)).render(inner_area, buf);
            }
        } else {
            let text = self.state.text();
            let cursor_pos = self.state.cursor_pos();

            if self.state.is_active() {
                // Render text with cursor highlight
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

                let spans = vec![
                    Span::styled(before, Style::default().fg(fg)),
                    Span::styled(
                        cursor_char,
                        Style::default().bg(cursor_color).fg(Color::Black),
                    ),
                    Span::styled(after, Style::default().fg(fg)),
                ];
                Paragraph::new(Line::from(spans)).render(inner_area, buf);
            } else {
                Paragraph::new(text)
                    .style(Style::default().fg(fg))
                    .render(inner_area, buf);
            }
        }

        // Render error below if there's space
        if let Some(error) = self.state.error() {
            let buffer_bottom = buf.area().y.saturating_add(buf.area().height);
            let error_area = if self.bordered {
                // Below the border
                let error_y = area.y.saturating_add(area.height);
                if error_y < buffer_bottom {
                    Rect {
                        x: area.x,
                        y: error_y,
                        width: area.width,
                        height: 1,
                    }
                } else {
                    return;
                }
            } else {
                let error_y = inner_area.y.saturating_add(inner_area.height);
                if error_y < buffer_bottom {
                    Rect {
                        x: inner_area.x,
                        y: error_y,
                        width: inner_area.width,
                        height: 1,
                    }
                } else {
                    return;
                }
            };
            let error_text = Paragraph::new(error).style(Style::default().fg(error_color));
            error_text.render(error_area, buf);
        }
    }

    fn render_multi_line(&self, area: Rect, buf: &mut Buffer) {
        let fg = Color::from_str(&self.theme.colors.foreground).unwrap_or(Color::White);
        let muted = Color::from_str(&self.theme.colors.muted).unwrap_or(Color::DarkGray);
        let error_color = Color::from_str(&self.theme.colors.error).unwrap_or(Color::Red);
        let border_color = Color::from_str(&self.theme.colors.border).unwrap_or(Color::Gray);
        let cursor_color = Color::from_str(&self.theme.colors.cursor).unwrap_or(Color::White);

        let has_error = self.state.error().is_some();
        let active_border_color = if has_error {
            error_color
        } else if self.state.is_active() {
            Color::from_str(&self.theme.colors.primary).unwrap_or(Color::Blue)
        } else {
            border_color
        };

        // Build block
        let inner_area = if self.bordered || self.title.is_some() {
            let mut block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(active_border_color));
            if let Some(ref title) = self.title {
                block = block.title(title.as_str());
            }
            let inner = block.inner(area);
            block.render(area, buf);
            inner
        } else {
            area
        };

        if inner_area.height == 0 || inner_area.width == 0 {
            return;
        }

        // Render placeholder if empty
        if self.state.is_empty() && !self.state.is_active() {
            let placeholder =
                Paragraph::new(self.state.placeholder()).style(Style::default().fg(muted));
            placeholder.render(inner_area, buf);
            return;
        }

        let lines = self.state.lines();
        let cursor_row = self.state.cursor_row();
        let cursor_col = self.state.cursor_col();
        let line_num_width = format!("{}", lines.len()).len();

        for (i, line_text) in lines.iter().enumerate() {
            if i as u16 >= inner_area.height {
                break;
            }

            let y = inner_area.y.saturating_add(i as u16);

            // Line number
            let line_num = format!("{:>width$} ", i + 1, width = line_num_width);
            let line_num_area = Rect {
                x: inner_area.x,
                y,
                width: (line_num_width as u16 + 1).min(inner_area.width),
                height: 1,
            };
            Paragraph::new(line_num.as_str())
                .style(Style::default().fg(muted))
                .render(line_num_area, buf);

            let text_offset = (line_num_width as u16).saturating_add(1);
            let text_x = inner_area.x.saturating_add(text_offset);
            let text_width = inner_area.width.saturating_sub(text_offset);
            if text_width == 0 {
                continue;
            }

            let text_area = Rect {
                x: text_x,
                y,
                width: text_width,
                height: 1,
            };

            if self.state.is_active() && i == cursor_row {
                // Render line with cursor
                let before = &line_text[..cursor_col.min(line_text.len())];
                let cursor_char = if cursor_col < line_text.len() {
                    &line_text[cursor_col
                        ..cursor_col
                            + line_text[cursor_col..]
                                .chars()
                                .next()
                                .map(|c| c.len_utf8())
                                .unwrap_or(1)]
                } else {
                    " "
                };
                let after = if cursor_col < line_text.len() {
                    &line_text[cursor_col + cursor_char.len()..]
                } else {
                    ""
                };

                let spans = vec![
                    Span::styled(before, Style::default().fg(fg)),
                    Span::styled(
                        cursor_char,
                        Style::default().bg(cursor_color).fg(Color::Black),
                    ),
                    Span::styled(after, Style::default().fg(fg)),
                ];
                Paragraph::new(Line::from(spans)).render(text_area, buf);
            } else {
                Paragraph::new(*line_text)
                    .style(Style::default().fg(fg))
                    .render(text_area, buf);
            }
        }
    }
}

fn placeholder_after_cursor(placeholder: &str) -> &str {
    placeholder
        .chars()
        .next()
        .map(|ch| &placeholder[ch.len_utf8()..])
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::text_input::TextInputState;

    fn test_theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn render_empty_single_line() {
        let state = TextInputState::new(InputMode::SingleLine).with_placeholder("Type here...");
        let theme = test_theme();
        let widget = TextInputWidget::new(&state, &theme);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 3));
        widget.render(Rect::new(0, 0, 40, 1), &mut buf);
        // Just verify it doesn't panic
    }

    #[test]
    fn render_active_unicode_placeholder() {
        let mut state =
            TextInputState::new(InputMode::SingleLine).with_placeholder("Écrire ici...");
        state.set_active(true);
        let theme = test_theme();
        let widget = TextInputWidget::new(&state, &theme);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 3));

        widget.render(Rect::new(0, 0, 40, 1), &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("crire ici"));
    }

    #[test]
    fn render_with_text() {
        let mut state = TextInputState::new(InputMode::SingleLine);
        state.insert_char('h');
        state.insert_char('i');
        state.set_active(true);
        let theme = test_theme();
        let widget = TextInputWidget::new(&state, &theme);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 3));
        widget.render(Rect::new(0, 0, 40, 1), &mut buf);
    }

    #[test]
    fn render_multi_line() {
        let state = TextInputState::new(InputMode::MultiLine).with_text("line1\nline2\nline3");
        let theme = test_theme();
        let widget = TextInputWidget::new(&state, &theme).bordered(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        widget.render(Rect::new(0, 0, 40, 8), &mut buf);
    }

    #[test]
    fn render_multi_line_inside_offset_buffer_near_u16_max() {
        let state =
            TextInputState::new(InputMode::MultiLine).with_text("line1\nline2\nline3\nline4");
        let theme = test_theme();
        let widget = TextInputWidget::new(&state, &theme);
        let area = Rect::new(u16::MAX - 20, u16::MAX - 2, 20, 4);
        let mut buf = Buffer::empty(area);

        widget.render(area, &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("line"));
    }

    #[test]
    fn render_with_title() {
        let state = TextInputState::new(InputMode::SingleLine).with_placeholder("Enter name");
        let theme = test_theme();
        let widget = TextInputWidget::new(&state, &theme).with_title("Name");
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 5));
        widget.render(Rect::new(0, 0, 40, 3), &mut buf);
    }

    #[test]
    fn render_with_error() {
        let mut state = TextInputState::new(InputMode::SingleLine);
        state.set_error(Some("Required field"));
        let theme = test_theme();
        let widget = TextInputWidget::new(&state, &theme).bordered(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 5));
        widget.render(Rect::new(0, 0, 40, 3), &mut buf);
    }

    #[test]
    fn render_error_inside_offset_buffer() {
        let mut state = TextInputState::new(InputMode::SingleLine);
        state.set_error(Some("Required field"));
        let theme = test_theme();
        let widget = TextInputWidget::new(&state, &theme).bordered(true);
        let area = Rect::new(10, 10, 40, 3);
        let mut buf = Buffer::empty(Rect::new(10, 10, 40, 5));

        widget.render(area, &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Required field"));
    }
}
