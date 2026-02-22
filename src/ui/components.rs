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
            let title_spans: Vec<Span> = if let Some(ref subtitle) = self.subtitle {
                vec![
                    Span::styled(&self.title, primary_style.add_modifier(Modifier::BOLD)),
                    Span::raw(" "),
                    Span::styled(format!("({})", subtitle), text_style),
                ]
            } else {
                vec![Span::styled(
                    &self.title,
                    primary_style.add_modifier(Modifier::BOLD),
                )]
            };

            let title_line = Line::from(title_spans);
            let title_para = Paragraph::new(title_line)
                .alignment(Alignment::Left)
                .style(header_style);

            let title_area = Rect {
                x: area.x + 1,
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
                x: area.x + 1,
                y: area.y + lines_used,
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
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, area.y + lines_used)) {
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

        // Build the hints string
        let hints_text: Vec<String> = self
            .hints
            .iter()
            .map(|h| format!("{}: {}", h.key, h.description))
            .collect();
        let hints_str = hints_text.join(" | ");

        // Calculate widths
        let status_width = self.status.len() as u16;
        let hints_width = hints_str.len() as u16;
        let available_width = area.width.saturating_sub(4); // Padding

        // Render status (left-aligned, truncated if needed)
        let display_status = if status_width > available_width / 2 {
            format!(
                "{}...",
                &self.status[..(available_width as usize / 2 - 3).min(self.status.len())]
            )
        } else {
            self.status.clone()
        };

        let status_span = Span::styled(display_status, text_style);
        let status_line = Line::from(vec![status_span]);
        let status_para = Paragraph::new(status_line).alignment(Alignment::Left);

        let status_area = Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width / 2,
            height: 1,
        };
        status_para.render(status_area, buf);

        // Render hints (right-aligned)
        if !hints_str.is_empty() {
            if hints_width > available_width / 2 {
                let mut truncated = hints_str
                    [..(available_width as usize / 2 - 3).min(hints_str.len())]
                    .to_string();
                truncated.push_str("...");
                truncated
            } else {
                hints_str
            };

            // Parse hints to style the keys differently
            let mut hint_spans: Vec<Span> = Vec::new();
            for (i, hint) in self.hints.iter().enumerate() {
                if i > 0 {
                    hint_spans.push(Span::styled(" | ", text_style));
                }
                hint_spans.push(Span::styled(
                    &hint.key,
                    primary_style.add_modifier(Modifier::BOLD),
                ));
                hint_spans.push(Span::styled(format!(": {}", hint.description), text_style));
            }

            let hints_line = Line::from(hint_spans);
            let hints_para = Paragraph::new(hints_line).alignment(Alignment::Right);

            let hints_area = Rect {
                x: area.x + area.width / 2,
                y: area.y,
                width: area.width / 2 - 1,
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
    fn test_key_hint_new() {
        let hint = KeyHint::new("q", "Quit");
        assert_eq!(hint.key, "q");
        assert_eq!(hint.description, "Quit");
    }
}
