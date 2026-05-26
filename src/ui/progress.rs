//! Progress bar widget for RightClick
//!
//! A simple progress bar widget that can be used to show loading
//! or operation progress in the TUI.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};
use std::str::FromStr;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::core::models::Theme;

/// A progress bar widget
pub struct ProgressBar<'a> {
    /// Progress ratio (0.0 to 1.0)
    ratio: f64,
    /// Theme for styling
    theme: &'a Theme,
    /// Optional label to display
    label: Option<String>,
    /// Character for filled portion
    filled_char: char,
    /// Character for empty portion
    empty_char: char,
}

impl<'a> ProgressBar<'a> {
    /// Create a new progress bar
    pub fn new(ratio: f64, theme: &'a Theme) -> Self {
        Self {
            ratio: ratio.clamp(0.0, 1.0),
            theme,
            label: None,
            filled_char: '█',
            empty_char: '░',
        }
    }

    /// Set a label to display on the progress bar
    pub fn with_label<S: Into<String>>(mut self, label: S) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set custom characters for the progress bar
    pub fn with_chars(mut self, filled: char, empty: char) -> Self {
        self.filled_char = filled;
        self.empty_char = empty;
        self
    }
}

impl Widget for ProgressBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let primary = Color::from_str(&self.theme.colors.primary).unwrap_or(Color::Blue);
        let muted = Color::from_str(&self.theme.colors.muted).unwrap_or(Color::DarkGray);
        let fg = Color::from_str(&self.theme.colors.foreground).unwrap_or(Color::White);

        let bar_width = if let Some(ref label) = self.label {
            let label_width = label.width().saturating_add(1).min(u16::MAX as usize) as u16;
            area.width.saturating_sub(label_width)
        } else {
            area.width
        };

        let filled_width = (bar_width as f64 * self.ratio) as u16;
        let empty_width = bar_width.saturating_sub(filled_width);

        let mut x = area.x;

        // Render label if present
        if let Some(ref label) = self.label {
            let mut label_x = x;
            for ch in label.chars() {
                let ch_width = ch.width().unwrap_or(0) as u16;
                if ch_width == 0 {
                    continue;
                }
                if label_x.saturating_add(ch_width) > area.x.saturating_add(area.width) {
                    break;
                }
                buf[(label_x, area.y)]
                    .set_char(ch)
                    .set_style(Style::default().fg(fg));
                label_x = label_x.saturating_add(ch_width);
            }
            x = x
                .saturating_add(label.width().min(u16::MAX as usize) as u16)
                .saturating_add(1);
        }

        // Render filled portion
        for i in 0..filled_width {
            let cell_x = x.saturating_add(i);
            if cell_x >= area.x.saturating_add(area.width) {
                break;
            }
            buf[(cell_x, area.y)]
                .set_char(self.filled_char)
                .set_style(Style::default().fg(primary));
        }

        // Render empty portion
        for i in 0..empty_width {
            let cell_x = x.saturating_add(filled_width).saturating_add(i);
            if cell_x >= area.x.saturating_add(area.width) {
                break;
            }
            buf[(cell_x, area.y)]
                .set_char(self.empty_char)
                .set_style(Style::default().fg(muted));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn progress_bar_renders() {
        let theme = test_theme();
        let bar = ProgressBar::new(0.5, &theme);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        bar.render(Rect::new(0, 0, 20, 1), &mut buf);
        // Just verify no panic
    }

    #[test]
    fn progress_bar_with_label() {
        let theme = test_theme();
        let bar = ProgressBar::new(0.75, &theme).with_label("Loading:");
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
        bar.render(Rect::new(0, 0, 30, 1), &mut buf);
    }

    #[test]
    fn progress_bar_with_unicode_label_uses_display_width() {
        let theme = test_theme();
        let bar = ProgressBar::new(0.2, &theme).with_label("検a");
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));

        bar.render(Rect::new(0, 0, 10, 1), &mut buf);

        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "検");
        assert_eq!(buf.cell((2, 0)).unwrap().symbol(), "a");
        assert_eq!(buf.cell((4, 0)).unwrap().symbol(), "█");
        assert_eq!(buf.cell((5, 0)).unwrap().symbol(), "░");
    }

    #[test]
    fn progress_bar_clamps_ratio() {
        let theme = test_theme();
        let bar = ProgressBar::new(1.5, &theme);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        bar.render(Rect::new(0, 0, 20, 1), &mut buf);
    }

    #[test]
    fn progress_bar_zero() {
        let theme = test_theme();
        let bar = ProgressBar::new(0.0, &theme);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        bar.render(Rect::new(0, 0, 20, 1), &mut buf);
    }

    #[test]
    fn progress_bar_custom_chars() {
        let theme = test_theme();
        let bar = ProgressBar::new(0.5, &theme).with_chars('=', '-');
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        bar.render(Rect::new(0, 0, 20, 1), &mut buf);
    }

    #[test]
    fn progress_bar_empty_area() {
        let theme = test_theme();
        let bar = ProgressBar::new(0.5, &theme);
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        bar.render(Rect::new(0, 0, 0, 0), &mut buf);
    }

    #[test]
    fn progress_bar_handles_offset_area_near_u16_max() {
        let theme = test_theme();
        let area = Rect::new(u16::MAX - 2, 0, 2, 1);
        let bar = ProgressBar::new(1.0, &theme);
        let mut buf = Buffer::empty(area);

        bar.render(area, &mut buf);

        assert_eq!(buf.cell((u16::MAX - 2, 0)).unwrap().symbol(), "█");
        assert_eq!(buf.cell((u16::MAX - 1, 0)).unwrap().symbol(), "█");
    }
}
