//! Loading spinner animation component
//!
//! This module provides an animated spinner for indicating loading states
//! in the TUI interface.

use crate::core::models::Theme;
use crate::theme::{UiElement, style_for_ui_element};
use crate::ui::display_width_u16;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::time::{Duration, Instant};

/// Loading spinner with animated frames
///
/// Provides various spinner styles and automatic frame advancement
/// based on elapsed time.
#[derive(Clone, Debug)]
pub struct Spinner {
    /// Animation frames
    frames: Vec<&'static str>,
    /// Current frame index
    frame_index: usize,
    /// Last time the frame was updated
    last_update: Instant,
    /// Frame duration (time between frames)
    frame_duration: Duration,
    /// Optional label to display next to the spinner
    label: Option<String>,
}

impl Spinner {
    /// Create a new spinner with the default style
    ///
    /// Uses a rotating line animation (", /, -, \") with 100ms frame duration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let spinner = Spinner::new();
    /// ```
    pub fn new() -> Self {
        Self::dots()
    }

    /// Create a dots-style spinner
    ///
    /// Displays animated dots: ⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let spinner = Spinner::dots();
    /// ```
    pub fn dots() -> Self {
        Self {
            frames: vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            frame_index: 0,
            last_update: Instant::now(),
            frame_duration: Duration::from_millis(80),
            label: None,
        }
    }

    /// Create a line-style spinner
    ///
    /// Displays a rotating line: | / - \
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let spinner = Spinner::line();
    /// ```
    pub fn line() -> Self {
        Self {
            frames: vec!["|", "/", "-", "\\"],
            frame_index: 0,
            last_update: Instant::now(),
            frame_duration: Duration::from_millis(100),
            label: None,
        }
    }

    /// Create a double-line spinner
    ///
    /// Displays rotating double lines: ⣾ ⣽ ⣻ ⢿ ⡿ ⣟ ⣯ ⣷
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let spinner = Spinner::double_line();
    /// ```
    pub fn double_line() -> Self {
        Self {
            frames: vec!["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"],
            frame_index: 0,
            last_update: Instant::now(),
            frame_duration: Duration::from_millis(80),
            label: None,
        }
    }

    /// Create an arrow spinner
    ///
    /// Displays a rotating arrow: ← ↖ ↑ ↗ → ↘ ↓ ↙
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let spinner = Spinner::arrow();
    /// ```
    pub fn arrow() -> Self {
        Self {
            frames: vec!["←", "↖", "↑", "↗", "→", "↘", "↓", "↙"],
            frame_index: 0,
            last_update: Instant::now(),
            frame_duration: Duration::from_millis(100),
            label: None,
        }
    }

    /// Create a bouncing ball spinner
    ///
    /// Displays a bouncing ball: (●    ) ((●   ) ( (●  ) (  (● ) (   (●) (   ●)) (  ● ) ( ●  ) (●   )
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let spinner = Spinner::bouncing_ball();
    /// ```
    pub fn bouncing_ball() -> Self {
        Self {
            frames: vec![
                "( ●    )",
                "(  ●   )",
                "(   ●  )",
                "(    ● )",
                "(     ●)",
                "(    ● )",
                "(   ●  )",
                "(  ●   )",
            ],
            frame_index: 0,
            last_update: Instant::now(),
            frame_duration: Duration::from_millis(100),
            label: None,
        }
    }

    /// Create a moon spinner
    ///
    /// Displays moon phases: 🌑 🌒 🌓 🌔 🌕 🌖 🌗 🌘
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let spinner = Spinner::moon();
    /// ```
    pub fn moon() -> Self {
        Self {
            frames: vec!["🌑", "🌒", "🌓", "🌔", "🌕", "🌖", "🌗", "🌘"],
            frame_index: 0,
            last_update: Instant::now(),
            frame_duration: Duration::from_millis(150),
            label: None,
        }
    }

    /// Create a clock spinner
    ///
    /// Displays clock hands: 🕐 🕑 🕒 🕓 🕔 🕕 🕖 🕗 🕘 🕙 🕚 🕛
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let spinner = Spinner::clock();
    /// ```
    pub fn clock() -> Self {
        Self {
            frames: vec![
                "🕐", "🕑", "🕒", "🕓", "🕔", "🕕", "🕖", "🕗", "🕘", "🕙", "🕚", "🕛",
            ],
            frame_index: 0,
            last_update: Instant::now(),
            frame_duration: Duration::from_millis(100),
            label: None,
        }
    }

    /// Set a label to display next to the spinner
    ///
    /// # Arguments
    ///
    /// * `label` - The label text
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let spinner = Spinner::new().with_label("Loading...");
    /// ```
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set a custom frame duration
    ///
    /// # Arguments
    ///
    /// * `duration` - Duration between frame updates
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    /// use std::time::Duration;
    ///
    /// let spinner = Spinner::new().with_frame_duration(Duration::from_millis(50));
    /// ```
    pub fn with_frame_duration(mut self, duration: Duration) -> Self {
        self.frame_duration = duration;
        self
    }

    /// Advance to the next frame if enough time has elapsed
    ///
    /// Returns the current frame symbol.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let mut spinner = Spinner::new();
    /// let frame = spinner.tick();
    /// ```
    pub fn tick(&mut self) -> &str {
        let now = Instant::now();
        if now.duration_since(self.last_update) >= self.frame_duration {
            self.frame_index = (self.frame_index + 1) % self.frames.len();
            self.last_update = now;
        }
        self.current_frame()
    }

    /// Force advancement to the next frame
    ///
    /// Returns the new frame symbol.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let mut spinner = Spinner::new();
    /// let frame = spinner.next_frame();
    /// ```
    pub fn next_frame(&mut self) -> &str {
        self.frame_index = (self.frame_index + 1) % self.frames.len();
        self.last_update = Instant::now();
        self.current_frame()
    }

    /// Get the current frame without advancing
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let mut spinner = Spinner::new();
    /// let frame = spinner.current_frame();
    /// ```
    pub fn current_frame(&self) -> &str {
        self.frames.get(self.frame_index).copied().unwrap_or(" ")
    }

    /// Check if the spinner is running (has multiple frames)
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let spinner = Spinner::new();
    /// assert!(spinner.is_running());
    /// ```
    pub fn is_running(&self) -> bool {
        self.frames.len() > 1
    }

    /// Reset the spinner to the first frame
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let mut spinner = Spinner::new();
    /// spinner.tick();
    /// spinner.reset();
    /// assert_eq!(spinner.current_index(), 0);
    /// ```
    pub fn reset(&mut self) {
        self.frame_index = 0;
        self.last_update = Instant::now();
    }

    /// Get the current frame index
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let spinner = Spinner::new();
    /// assert_eq!(spinner.current_index(), 0);
    /// ```
    pub fn current_index(&self) -> usize {
        self.frame_index
    }

    /// Get the full display text (spinner + optional label)
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    ///
    /// let spinner = Spinner::new().with_label("Loading");
    /// let text = spinner.display_text();
    /// assert!(text.contains("Loading"));
    /// ```
    pub fn display_text(&self) -> String {
        let frame = self.current_frame();
        if let Some(ref label) = self.label {
            format!("{} {}", frame, label)
        } else {
            frame.to_string()
        }
    }

    /// Render the spinner to the buffer
    ///
    /// # Arguments
    ///
    /// * `area` - The area to render in
    /// * `buf` - The buffer to render to
    /// * `theme` - The theme to use for styling
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    /// use rightclick::core::models::Theme;
    /// use ratatui::layout::Rect;
    /// use ratatui::buffer::Buffer;
    ///
    /// let spinner = Spinner::new().with_label("Loading...");
    /// let area = Rect::new(0, 0, 20, 1);
    /// let mut buf = Buffer::empty(area);
    /// let theme = Theme::default();
    ///
    /// spinner.render(area, &mut buf, &theme);
    /// ```
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let primary_style = style_for_ui_element(theme, UiElement::Primary);
        let text_style = style_for_ui_element(theme, UiElement::Text);

        let frame = self.current_frame();

        let spans: Vec<Span> = if let Some(ref label) = self.label {
            vec![
                Span::styled(
                    frame.to_string(),
                    primary_style.add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(label.clone(), text_style),
            ]
        } else {
            vec![Span::styled(
                frame.to_string(),
                primary_style.add_modifier(Modifier::BOLD),
            )]
        };

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line).alignment(Alignment::Left);

        paragraph.render(area, buf);
    }

    /// Render the spinner centered in the given area
    ///
    /// # Arguments
    ///
    /// * `area` - The area to render in
    /// * `buf` - The buffer to render to
    /// * `theme` - The theme to use for styling
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Spinner;
    /// use rightclick::core::models::Theme;
    /// use ratatui::layout::Rect;
    /// use ratatui::buffer::Buffer;
    ///
    /// let spinner = Spinner::new().with_label("Loading...");
    /// let area = Rect::new(0, 0, 80, 24);
    /// let mut buf = Buffer::empty(area);
    /// let theme = Theme::default();
    ///
    /// spinner.render_centered(area, &mut buf, &theme);
    /// ```
    pub fn render_centered(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let primary_style = style_for_ui_element(theme, UiElement::Primary);
        let text_style = style_for_ui_element(theme, UiElement::Text);

        let frame = self.current_frame();

        let text = if let Some(ref label) = self.label {
            format!("{} {}", frame, label)
        } else {
            frame.to_string()
        };

        let text_width = display_width_u16(&text);
        let x = area
            .x
            .saturating_add((area.width.saturating_sub(text_width)) / 2);
        let y = area.y.saturating_add(area.height / 2);
        let right = area.x.saturating_add(area.width);
        let bottom = area.y.saturating_add(area.height);

        if x < right && y < bottom {
            let spans: Vec<Span> = if let Some(ref label) = self.label {
                vec![
                    Span::styled(
                        frame.to_string(),
                        primary_style.add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    Span::styled(label.clone(), text_style),
                ]
            } else {
                vec![Span::styled(
                    frame.to_string(),
                    primary_style.add_modifier(Modifier::BOLD),
                )]
            };

            let line = Line::from(spans);
            let paragraph = Paragraph::new(line).alignment(Alignment::Center);

            let center_area = Rect {
                x,
                y,
                width: text_width.min(area.width),
                height: 1,
            };

            paragraph.render(center_area, buf);
        }
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_new() {
        let spinner = Spinner::new();
        assert!(!spinner.frames.is_empty());
        assert_eq!(spinner.frame_index, 0);
    }

    #[test]
    fn test_spinner_line() {
        let spinner = Spinner::line();
        assert_eq!(spinner.frames, vec!["|", "/", "-", "\\"]);
    }

    #[test]
    fn test_spinner_dots() {
        let spinner = Spinner::dots();
        assert_eq!(spinner.frames.len(), 10);
    }

    #[test]
    fn test_spinner_with_label() {
        let spinner = Spinner::new().with_label("Loading");
        assert_eq!(spinner.label, Some("Loading".to_string()));
    }

    #[test]
    fn test_spinner_tick() {
        let mut spinner = Spinner::line();
        let frame1 = spinner.tick();
        assert!(!frame1.is_empty());

        // Force next frame by waiting
        spinner.last_update = Instant::now() - Duration::from_millis(200);
        let _frame2 = spinner.tick();

        // Frames should be different after enough time
        assert_ne!(spinner.frame_index, 0);
    }

    #[test]
    fn test_spinner_next_frame() {
        let mut spinner = Spinner::line();
        let frame1 = spinner.current_frame().to_string();
        let frame2 = spinner.next_frame();
        assert_ne!(frame1, frame2);
    }

    #[test]
    fn test_spinner_current_frame() {
        let spinner = Spinner::line();
        assert_eq!(spinner.current_frame(), "|");
    }

    #[test]
    fn test_spinner_is_running() {
        let spinner = Spinner::new();
        assert!(spinner.is_running());
    }

    #[test]
    fn test_spinner_reset() {
        let mut spinner = Spinner::line();
        spinner.next_frame();
        spinner.next_frame();
        spinner.reset();
        assert_eq!(spinner.frame_index, 0);
    }

    #[test]
    fn test_spinner_display_text() {
        let spinner = Spinner::new().with_label("Loading");
        let text = spinner.display_text();
        assert!(text.contains("Loading"));
    }

    #[test]
    fn test_spinner_display_text_no_label() {
        let spinner = Spinner::line();
        let text = spinner.display_text();
        assert_eq!(text, "|");
    }

    #[test]
    fn test_render_centered_positions_unicode_label_by_display_width() {
        let spinner = Spinner::line().with_label("検a");
        let theme = Theme::default();
        let area = Rect::new(0, 0, 13, 3);
        let mut buf = Buffer::empty(area);

        spinner.render_centered(area, &mut buf, &theme);

        assert_eq!(buf.cell((4, 1)).unwrap().symbol(), "|");
        assert_eq!(buf.cell((6, 1)).unwrap().symbol(), "検");
        assert_eq!(buf.cell((8, 1)).unwrap().symbol(), "a");
        assert_ne!(buf.cell((3, 1)).unwrap().symbol(), "|");
    }

    #[test]
    fn test_render_centered_handles_offset_area_near_u16_max() {
        let spinner = Spinner::line().with_label("Load");
        let theme = Theme::default();
        let area = Rect::new(u16::MAX - 80, u16::MAX - 40, 80, 40);
        let mut buf = Buffer::empty(area);

        spinner.render_centered(area, &mut buf, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("|"));
        assert!(content.contains("Load"));
    }

    #[test]
    fn test_spinner_current_index() {
        let mut spinner = Spinner::line();
        assert_eq!(spinner.current_index(), 0);
        spinner.next_frame();
        assert_eq!(spinner.current_index(), 1);
    }
}
