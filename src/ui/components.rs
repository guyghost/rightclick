//! Basic UI components for the TUI interface
//!
//! This module provides Header, Footer, and tab components for consistent
//! UI layout across the application.

use crate::core::models::Theme;
use crate::theme::{UiElement, style_for_ui_element};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Tabs, Widget};
use unicode_width::UnicodeWidthStr;

const FOOTER_HINT_SEPARATOR: &str = "  |  ";
const FOOTER_HINT_OVERFLOW: &str = "...";

/// A key hint displayed in the footer
#[derive(Clone, Debug, PartialEq)]
pub struct KeyHint {
    /// The key to display (e.g., "q", "Ctrl+C")
    pub key: String,
    /// Description of what the key does
    pub description: String,
}

impl KeyHint {
    /// Create a new key hint
    ///
    /// # Arguments
    ///
    /// * `key` - The key combination
    /// * `description` - What the key does
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::KeyHint;
    ///
    /// let hint = KeyHint::new("q", "Quit");
    /// ```
    pub fn new(key: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            description: description.into(),
        }
    }
}

/// Header component with title, subtitle, and tabs
#[derive(Clone, Debug)]
pub struct Header {
    /// Main title displayed in the header
    pub title: String,
    /// Optional subtitle displayed below or beside the title
    pub subtitle: Option<String>,
    /// Tab labels
    pub tabs: Vec<String>,
    /// Index of the currently active tab
    pub active_tab: usize,
}

impl Header {
    /// Create a new header with just a title
    ///
    /// # Arguments
    ///
    /// * `title` - The main title to display
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Header;
    ///
    /// let header = Header::new("My Application");
    /// ```
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            tabs: Vec::new(),
            active_tab: 0,
        }
    }

    /// Add a subtitle to the header
    ///
    /// # Arguments
    ///
    /// * `subtitle` - The subtitle to display
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Header;
    ///
    /// let header = Header::new("My App").with_subtitle("v1.0.0");
    /// ```
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Add tabs to the header
    ///
    /// # Arguments
    ///
    /// * `tabs` - Vector of tab labels
    /// * `active` - Index of the active tab
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Header;
    ///
    /// let header = Header::new("My App")
    ///     .with_tabs(vec!["Files", "Git", "Settings"], 0);
    /// ```
    pub fn with_tabs(mut self, tabs: Vec<impl Into<String>>, active: usize) -> Self {
        self.tabs = tabs.into_iter().map(Into::into).collect();
        self.active_tab = active.min(self.tabs.len().saturating_sub(1));
        self
    }

    /// Set the active tab index
    ///
    /// # Arguments
    ///
    /// * `index` - The new active tab index
    pub fn set_active_tab(&mut self, index: usize) {
        self.active_tab = index.min(self.tabs.len().saturating_sub(1));
    }

    /// Render the header to the buffer
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
    /// use rightclick::ui::Header;
    /// use rightclick::core::models::Theme;
    /// use ratatui::layout::Rect;
    /// use ratatui::buffer::Buffer;
    ///
    /// let header = Header::new("My App");
    /// let area = Rect::new(0, 0, 80, 3);
    /// let mut buf = Buffer::empty(area);
    /// let theme = Theme::default();
    ///
    /// header.render(area, &mut buf, &theme);
    /// ```
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let header_style = style_for_ui_element(theme, UiElement::StatusBar);
        let primary_style = style_for_ui_element(theme, UiElement::Primary);
        let text_style = style_for_ui_element(theme, UiElement::Text);

        // Clear the area with header background
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(header_style);
                }
            }
        }

        let mut lines_used = 0;

        // Render title line
        if !self.title.is_empty() {
            let title_width = area.width.saturating_sub(2) as usize;
            let (display_title, display_subtitle) =
                header_title_parts(&self.title, self.subtitle.as_deref(), title_width);

            let title_spans: Vec<Span> = if let Some(subtitle) = display_subtitle {
                vec![
                    Span::styled(display_title, primary_style.add_modifier(Modifier::BOLD)),
                    Span::raw(" "),
                    Span::styled(format!("({})", subtitle), text_style),
                ]
            } else {
                vec![Span::styled(
                    display_title,
                    primary_style.add_modifier(Modifier::BOLD),
                )]
            };

            let title_line = Line::from(title_spans);
            let title_para = Paragraph::new(title_line)
                .alignment(Alignment::Left)
                .style(header_style);

            let title_area = Rect {
                x: area.x.saturating_add(1),
                y: area.y,
                width: area.width.saturating_sub(2),
                height: 1,
            };
            title_para.render(title_area, buf);
            lines_used = 1;
        }

        // Render tabs if present
        if !self.tabs.is_empty() {
            let active_style = style_for_ui_element(theme, UiElement::ActiveItem);
            let inactive_style = style_for_ui_element(theme, UiElement::InactiveItem);

            let tab_titles: Vec<Line> = self
                .tabs
                .iter()
                .enumerate()
                .map(|(i, title)| {
                    let style = if i == self.active_tab {
                        active_style
                    } else {
                        inactive_style
                    };
                    Line::from(Span::styled(title.clone(), style))
                })
                .collect();

            let tabs_widget = Tabs::new(tab_titles)
                .select(self.active_tab)
                .style(inactive_style)
                .highlight_style(active_style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED))
                .divider(Span::styled(" | ", text_style));

            let tabs_area = Rect {
                x: area.x.saturating_add(1),
                y: area.y.saturating_add(lines_used),
                width: area.width.saturating_sub(2),
                height: 1,
            };
            tabs_widget.render(tabs_area, buf);
            lines_used += 1;
        }

        // Add bottom border if there's room
        if area.height > lines_used {
            let border_style = style_for_ui_element(theme, UiElement::Border);
            let border_char = "─";
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell_mut((x, area.y.saturating_add(lines_used))) {
                    cell.set_symbol(border_char);
                    cell.set_style(border_style);
                }
            }
        }
    }

    /// Calculate the required height for this header
    ///
    /// Returns the number of lines needed to render the header
    pub fn height(&self) -> u16 {
        let mut height = 1; // Title line
        if !self.tabs.is_empty() {
            height += 1; // Tabs line
        }
        height += 1; // Border
        height
    }
}

impl Default for Header {
    fn default() -> Self {
        Self::new("RightClick")
    }
}

/// Footer component with status and key hints
#[derive(Clone, Debug)]
pub struct Footer {
    /// Status message displayed on the left
    pub status: String,
    /// Key hints displayed on the right
    pub hints: Vec<KeyHint>,
}

impl Footer {
    /// Create a new footer with a status message
    ///
    /// # Arguments
    ///
    /// * `status` - The status message to display
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Footer;
    ///
    /// let footer = Footer::new("Ready");
    /// ```
    pub fn new(status: impl Into<String>) -> Self {
        Self {
            status: status.into(),
            hints: Vec::new(),
        }
    }

    /// Add a key hint to the footer
    ///
    /// # Arguments
    ///
    /// * `key` - The key combination
    /// * `description` - What the key does
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Footer;
    ///
    /// let footer = Footer::new("Ready")
    ///     .with_hint("q", "Quit")
    ///     .with_hint("h", "Help");
    /// ```
    pub fn with_hint(mut self, key: impl Into<String>, description: impl Into<String>) -> Self {
        self.hints.push(KeyHint::new(key, description));
        self
    }

    /// Add multiple key hints to the footer
    ///
    /// # Arguments
    ///
    /// * `hints` - Vector of (key, description) tuples
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Footer;
    ///
    /// let footer = Footer::new("Ready").with_hints(vec![
    ///     ("q", "Quit"),
    ///     ("h", "Help"),
    ///     ("n", "New"),
    /// ]);
    /// ```
    pub fn with_hints(mut self, hints: Vec<(impl Into<String>, impl Into<String>)>) -> Self {
        for (key, desc) in hints {
            self.hints.push(KeyHint::new(key, desc));
        }
        self
    }

    /// Set the status message
    ///
    /// # Arguments
    ///
    /// * `status` - The new status message
    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    /// Render the footer to the buffer
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
    /// use rightclick::ui::Footer;
    /// use rightclick::core::models::Theme;
    /// use ratatui::layout::Rect;
    /// use ratatui::buffer::Buffer;
    ///
    /// let footer = Footer::new("Ready").with_hint("q", "Quit");
    /// let area = Rect::new(0, 20, 80, 1);
    /// let mut buf = Buffer::empty(area);
    /// let theme = Theme::default();
    ///
    /// footer.render(area, &mut buf, &theme);
    /// ```
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let status_bar_style = style_for_ui_element(theme, UiElement::StatusBar);
        let primary_style = style_for_ui_element(theme, UiElement::Primary);
        let text_style = style_for_ui_element(theme, UiElement::Text);

        // Clear the area
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(status_bar_style);
                }
            }
        }

        let status_width = area.width / 2;
        let hints_width = area.width.saturating_sub(status_width).saturating_sub(1);
        let display_status =
            truncate_display(&self.status, status_width.saturating_sub(1) as usize);

        let status_span = Span::styled(display_status, text_style);
        let status_line = Line::from(vec![status_span]);
        let status_para = Paragraph::new(status_line).alignment(Alignment::Left);

        let status_area = Rect {
            x: area.x.saturating_add(1),
            y: area.y,
            width: status_width.saturating_sub(1),
            height: 1,
        };
        status_para.render(status_area, buf);

        // Render hints (right-aligned)
        if !self.hints.is_empty() && hints_width > 0 {
            // Parse hints to style the keys differently
            let mut hint_spans: Vec<Span> = Vec::new();
            let visible_hints = visible_hints(&self.hints, hints_width as usize);
            for (i, hint) in visible_hints.iter().enumerate() {
                if i > 0 {
                    hint_spans.push(Span::styled(FOOTER_HINT_SEPARATOR, text_style));
                }
                hint_spans.push(Span::styled(
                    &hint.key,
                    primary_style.add_modifier(Modifier::BOLD),
                ));
                hint_spans.push(Span::styled(format!(": {}", hint.description), text_style));
            }
            if visible_hints.len() < self.hints.len() && hints_width >= 3 {
                if !hint_spans.is_empty() {
                    hint_spans.push(Span::styled(FOOTER_HINT_SEPARATOR, text_style));
                }
                hint_spans.push(Span::styled(FOOTER_HINT_OVERFLOW, text_style));
            }

            let hints_line = Line::from(hint_spans);
            let hints_para = Paragraph::new(hints_line).alignment(Alignment::Right);

            let hints_area = Rect {
                x: area.x.saturating_add(status_width),
                y: area.y,
                width: hints_width,
                height: 1,
            };
            hints_para.render(hints_area, buf);
        }
    }

    /// Calculate the required height for this footer
    ///
    /// Returns the number of lines needed to render the footer
    pub fn height(&self) -> u16 {
        1
    }
}

impl Default for Footer {
    fn default() -> Self {
        Self::new("Ready")
    }
}

fn header_title_parts(
    title: &str,
    subtitle: Option<&str>,
    max_width: usize,
) -> (String, Option<String>) {
    if max_width == 0 {
        return (String::new(), None);
    }

    let title_width = UnicodeWidthStr::width(title);
    if title_width >= max_width || subtitle.is_none() {
        return (truncate_display(title, max_width), None);
    }

    let subtitle_width = max_width.saturating_sub(title_width + 3);
    if subtitle_width == 0 {
        return (title.to_string(), None);
    }

    (
        title.to_string(),
        subtitle.map(|value| truncate_display(value, subtitle_width)),
    )
}

fn truncate_display(value: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    if UnicodeWidthStr::width(value) <= max_width {
        return value.to_string();
    }

    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let mut output = String::new();
    let mut width = 0;
    for ch in value.chars() {
        let ch_width = UnicodeWidthStr::width(ch.to_string().as_str());
        if width + ch_width + 3 > max_width {
            break;
        }
        output.push(ch);
        width += ch_width;
    }
    output.push_str("...");
    output
}

fn visible_hints(hints: &[KeyHint], max_width: usize) -> Vec<&KeyHint> {
    let mut visible = Vec::new();
    let mut used = 0;

    for (idx, hint) in hints.iter().enumerate() {
        let item_width = UnicodeWidthStr::width(hint.key.as_str())
            + 2
            + UnicodeWidthStr::width(hint.description.as_str());
        let separator_width = if visible.is_empty() {
            0
        } else {
            FOOTER_HINT_SEPARATOR.width()
        };
        let hidden_after_this = idx + 1 < hints.len();
        let overflow_marker_width = if hidden_after_this {
            FOOTER_HINT_SEPARATOR.width() + FOOTER_HINT_OVERFLOW.width()
        } else {
            0
        };
        if used + separator_width + item_width + overflow_marker_width > max_width {
            break;
        }
        used += separator_width + item_width;
        visible.push(hint);
    }

    visible
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_new() {
        let header = Header::new("Test");
        assert_eq!(header.title, "Test");
        assert!(header.subtitle.is_none());
        assert!(header.tabs.is_empty());
    }

    #[test]
    fn test_header_with_subtitle() {
        let header = Header::new("Test").with_subtitle("v1.0");
        assert_eq!(header.title, "Test");
        assert_eq!(header.subtitle, Some("v1.0".to_string()));
    }

    #[test]
    fn test_header_with_tabs() {
        let header = Header::new("Test").with_tabs(vec!["A", "B", "C"], 1);
        assert_eq!(header.tabs, vec!["A", "B", "C"]);
        assert_eq!(header.active_tab, 1);
    }

    #[test]
    fn test_header_active_tab_clamping() {
        let header = Header::new("Test").with_tabs(vec!["A", "B"], 10);
        assert_eq!(header.active_tab, 1); // Clamped to last valid index
    }

    #[test]
    fn test_header_height() {
        let header = Header::new("Test");
        assert_eq!(header.height(), 2); // Title + border

        let header_with_tabs = Header::new("Test").with_tabs(vec!["A", "B"], 0);
        assert_eq!(header_with_tabs.height(), 3); // Title + tabs + border
    }

    #[test]
    fn test_header_title_parts_truncates_long_subtitle() {
        let (title, subtitle) =
            header_title_parts("RightClick", Some("/very/long/workspace/path"), 20);

        assert_eq!(title, "RightClick");
        assert_eq!(subtitle, Some("/ver...".to_string()));
    }

    #[test]
    fn test_header_title_parts_hides_subtitle_when_title_fills_width() {
        let (title, subtitle) = header_title_parts("RightClick", Some("/workspace"), 6);

        assert_eq!(title, "Rig...");
        assert_eq!(subtitle, None);
    }

    #[test]
    fn test_footer_new() {
        let footer = Footer::new("Ready");
        assert_eq!(footer.status, "Ready");
        assert!(footer.hints.is_empty());
    }

    #[test]
    fn test_footer_with_hint() {
        let footer = Footer::new("Ready").with_hint("q", "Quit");
        assert_eq!(footer.hints.len(), 1);
        assert_eq!(footer.hints[0].key, "q");
        assert_eq!(footer.hints[0].description, "Quit");
    }

    #[test]
    fn test_footer_with_hints() {
        let footer = Footer::new("Ready").with_hints(vec![("q", "Quit"), ("h", "Help")]);
        assert_eq!(footer.hints.len(), 2);
    }

    #[test]
    fn test_footer_render_small_width_no_panic() {
        let footer = Footer::new("A very long status line").with_hints(vec![
            ("Tab", "Switch"),
            ("1-9", "Go"),
            ("/", "Global search"),
            ("q", "Quit"),
        ]);
        let area = ratatui::layout::Rect::new(0, 0, 12, 1);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        footer.render(area, &mut buf, &Theme::default());
    }

    #[test]
    fn test_footer_render_uses_spacious_hint_separator() {
        let footer = Footer::new("Ready").with_hints(vec![("q", "Quit"), ("?", "Help")]);
        let area = ratatui::layout::Rect::new(0, 0, 80, 1);
        let mut buf = ratatui::buffer::Buffer::empty(area);

        footer.render(area, &mut buf, &Theme::default());

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("q: Quit  |  ?: Help"));
    }

    #[test]
    fn test_header_render_offset_area_near_u16_max() {
        let header = Header::new("RightClick").with_tabs(vec!["A", "B"], 0);
        let area = ratatui::layout::Rect::new(u16::MAX - 4, 10, 4, 3);
        let mut buf = ratatui::buffer::Buffer::empty(area);

        header.render(area, &mut buf, &Theme::default());

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.chars().any(|ch| ch != ' '));
    }

    #[test]
    fn test_footer_render_offset_area_near_u16_max() {
        let footer = Footer::new("Ready").with_hint("q", "Quit");
        let area = ratatui::layout::Rect::new(u16::MAX - 6, 20, 6, 1);
        let mut buf = ratatui::buffer::Buffer::empty(area);

        footer.render(area, &mut buf, &Theme::default());

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.chars().any(|ch| ch != ' '));
    }

    #[test]
    fn test_truncate_display_handles_tiny_widths() {
        assert_eq!(truncate_display("abcdef", 0), "");
        assert_eq!(truncate_display("abcdef", 2), "..");
        assert_eq!(truncate_display("abcdef", 5), "ab...");
    }

    #[test]
    fn test_visible_hints_reserves_overflow_marker_width() {
        let hints = vec![
            KeyHint::new("q", "Quit"),
            KeyHint::new("?", "Help"),
            KeyHint::new("/", "Global search"),
        ];

        let visible = visible_hints(&hints, 11);

        assert!(visible.is_empty());
    }

    #[test]
    fn test_visible_hints_accounts_for_spacious_separator_before_overflow() {
        let hints = vec![KeyHint::new("q", "Quit"), KeyHint::new("?", "Help")];

        assert!(visible_hints(&hints, 14).is_empty());
        assert_eq!(visible_hints(&hints, 15), vec![&hints[0]]);
    }

    #[test]
    fn test_visible_hints_keeps_final_hint_without_overflow_marker() {
        let hints = vec![KeyHint::new("q", "Quit")];

        let visible = visible_hints(&hints, 7);

        assert_eq!(visible, vec![&hints[0]]);
    }

    #[test]
    fn test_key_hint_new() {
        let hint = KeyHint::new("q", "Quit");
        assert_eq!(hint.key, "q");
        assert_eq!(hint.description, "Quit");
    }
}
