//! Modal sections for building modal content
//!
//! This module defines the `Section` trait and various implementations
//! that can be composed to build modal dialog content. Sections are the
//! building blocks of modal UIs.

use crate::keymap::Action;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};

use super::options::{Button, Checkbox};

/// A section of modal content that can be rendered and handle input
///
/// Sections are composable building blocks for modal dialogs. Each section
/// knows how to render itself and can optionally handle keyboard input for
/// interactive elements.
///
/// # Implementing Custom Sections
///
/// To create a custom section, implement this trait and provide:
/// - `render`: Draw the section content to the buffer
/// - `handle_input`: Process key events (return `Some(Action)` if handled)
/// - `focusable_ids`: Return IDs of focusable elements within this section
/// - `height`: Calculate the rendered height for layout
pub trait Section: Send {
    /// Render this section to the buffer
    ///
    /// # Arguments
    ///
    /// * `area` - The area to render in
    /// * `buf` - The buffer to render to
    /// * `theme` - The current theme for styling
    /// * `focus_id` - The ID of the currently focused element, if any
    /// * `hover_id` - The ID of the hovered element, if any
    fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        theme: &crate::core::models::Theme,
        focus_id: Option<&str>,
        hover_id: Option<&str>,
    );

    /// Handle a key event
    ///
    /// Returns `Some(Action)` if the key was handled, `None` otherwise.
    fn handle_input(&mut self, key: KeyEvent, focus_id: Option<&str>) -> Option<Action>;

    /// Returns the IDs of all focusable elements in this section
    fn focusable_ids(&self) -> Vec<String>;

    /// Calculate the height this section needs for rendering
    ///
    /// # Arguments
    ///
    /// * `width` - The available width
    fn height(&self, width: u16) -> u16;
}

/// A text section displays static text content
pub struct TextSection {
    /// The text content to display
    pub text: String,
    /// Optional alignment (defaults to Left)
    pub alignment: Alignment,
}

impl TextSection {
    /// Creates a new text section
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::section::TextSection;
    ///
    /// let section = TextSection::new("This is some text");
    /// ```
    pub fn new<T: Into<String>>(text: T) -> Self {
        Self {
            text: text.into(),
            alignment: Alignment::Left,
        }
    }

    /// Sets the alignment for this text section
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::section::TextSection;
    /// use ratatui::layout::Alignment;
    ///
    /// let section = TextSection::new("Centered").with_alignment(Alignment::Center);
    /// ```
    pub fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}

impl Section for TextSection {
    fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        theme: &crate::core::models::Theme,
        _focus_id: Option<&str>,
        _hover_id: Option<&str>,
    ) {
        let text_color = theme.colors.foreground.clone();
        let style = Style::default()
            .fg(ratatui::style::Color::from_str(&text_color)
                .unwrap_or(ratatui::style::Color::White));

        let paragraph = Paragraph::new(self.text.as_str())
            .style(style)
            .alignment(self.alignment)
            .wrap(Wrap { trim: true });

        paragraph.render(area, buf);
    }

    fn handle_input(&mut self, _key: KeyEvent, _focus_id: Option<&str>) -> Option<Action> {
        None // Text sections don't handle input
    }

    fn focusable_ids(&self) -> Vec<String> {
        vec![] // Text sections have no focusable elements
    }

    fn height(&self, width: u16) -> u16 {
        // Calculate wrapped line count
        let text_width = unicode_width::UnicodeWidthStr::width(self.text.as_str()) as u16;
        if text_width == 0 {
            return 1;
        }
        let lines = (text_width + width - 1) / width;
        lines.max(1)
    }
}

/// A spacer section adds vertical padding
pub struct SpacerSection {
    /// Number of empty lines
    pub lines: u16,
}

impl SpacerSection {
    /// Creates a new spacer section
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::section::SpacerSection;
    ///
    /// let spacer = SpacerSection::new(1);
    /// ```
    pub fn new(lines: u16) -> Self {
        Self { lines }
    }
}

impl Section for SpacerSection {
    fn render(
        &self,
        _area: Rect,
        _buf: &mut Buffer,
        _theme: &crate::core::models::Theme,
        _focus_id: Option<&str>,
        _hover_id: Option<&str>,
    ) {
        // Spacers render nothing
    }

    fn handle_input(&mut self, _key: KeyEvent, _focus_id: Option<&str>) -> Option<Action> {
        None
    }

    fn focusable_ids(&self) -> Vec<String> {
        vec![]
    }

    fn height(&self, _width: u16) -> u16 {
        self.lines
    }
}

/// A section containing a row of buttons
pub struct ButtonSection {
    /// The buttons to display
    pub buttons: Vec<Button>,
}

impl ButtonSection {
    /// Creates a new button section
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::{Button, ButtonVariant, section::ButtonSection};
    ///
    /// let section = ButtonSection::new(vec![
    ///     Button::primary("ok", "OK"),
    ///     Button::secondary("cancel", "Cancel"),
    /// ]);
    /// ```
    pub fn new(buttons: Vec<Button>) -> Self {
        Self { buttons }
    }

    /// Adds a button to this section
    pub fn add_button(mut self, button: Button) -> Self {
        self.buttons.push(button);
        self
    }
}

impl Section for ButtonSection {
    fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        theme: &crate::core::models::Theme,
        focus_id: Option<&str>,
        hover_id: Option<&str>,
    ) {
        if self.buttons.is_empty() {
            return;
        }

        // Calculate button spacing
        let total_button_width: u16 = self
            .buttons
            .iter()
            .map(|b| unicode_width::UnicodeWidthStr::width(b.label.as_str()) as u16 + 4) // 2 padding each side
            .sum();
        let spacing = if self.buttons.len() > 1 {
            (area.width.saturating_sub(total_button_width)) / (self.buttons.len() as u16 - 1)
        } else {
            0
        };

        let mut x = area.x;
        let y = area.y;

        for button in &self.buttons {
            let is_focused = focus_id == Some(button.id.as_str());
            let is_hovered = hover_id == Some(button.id.as_str());

            let button_width =
                unicode_width::UnicodeWidthStr::width(button.label.as_str()) as u16 + 4;

            // Get color based on variant and state
            let color = match button.variant {
                super::options::ButtonVariant::Primary => &theme.colors.primary,
                super::options::ButtonVariant::Secondary => &theme.colors.muted,
                super::options::ButtonVariant::Danger => &theme.colors.error,
            };

            let bg_color = if is_focused || is_hovered {
                &theme.ui_colors.button_hover_bg
            } else {
                &theme.ui_colors.button_bg
            };

            let fg_color = if is_focused || is_hovered {
                &theme.ui_colors.button_hover_fg
            } else {
                color
            };

            let style = Style::default()
                .fg(ratatui::style::Color::from_str(fg_color)
                    .unwrap_or(ratatui::style::Color::White))
                .bg(ratatui::style::Color::from_str(bg_color)
                    .unwrap_or(ratatui::style::Color::Black));

            let style = if is_focused {
                style.add_modifier(Modifier::BOLD)
            } else {
                style
            };

            // Render button with padding
            let button_area = Rect {
                x,
                y,
                width: button_width.min(area.x + area.width - x),
                height: 1,
            };

            let label = format!("  {}  ", button.label);
            let span = Span::styled(label, style);
            let line = Line::from(vec![span]);
            let paragraph = Paragraph::new(line);
            paragraph.render(button_area, buf);

            x += button_width + spacing;
            if x >= area.x + area.width {
                break;
            }
        }
    }

    fn handle_input(&mut self, key: KeyEvent, focus_id: Option<&str>) -> Option<Action> {
        if key.code == KeyCode::Enter {
            // Find the focused button and return its action
            if let Some(id) = focus_id {
                if let Some(button) = self.buttons.iter().find(|b| b.id == id) {
                    return Some(Action::Custom(Box::new(ButtonAction {
                        button_id: button.id.clone(),
                    })));
                }
            }
        }
        None
    }

    fn focusable_ids(&self) -> Vec<String> {
        self.buttons.iter().map(|b| b.id.clone()).collect()
    }

    fn height(&self, _width: u16) -> u16 {
        if self.buttons.is_empty() {
            0
        } else {
            1
        }
    }
}

/// Action emitted when a button is pressed
#[derive(Clone, Debug, PartialEq)]
pub struct ButtonAction {
    /// The ID of the button that was pressed
    pub button_id: String,
}

/// A section containing a checkbox
pub struct CheckboxSection {
    /// The checkbox state
    pub checkbox: Checkbox,
}

impl CheckboxSection {
    /// Creates a new checkbox section
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::section::CheckboxSection;
    ///
    /// let section = CheckboxSection::new("remember", "Remember me", false);
    /// ```
    pub fn new<I: Into<String>, L: Into<String>>(id: I, label: L, checked: bool) -> Self {
        Self {
            checkbox: Checkbox::new(id, label, checked),
        }
    }
}

impl Section for CheckboxSection {
    fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        theme: &crate::core::models::Theme,
        focus_id: Option<&str>,
        _hover_id: Option<&str>,
    ) {
        let is_focused = focus_id == Some(self.checkbox.id.as_str());

        let fg_color = &theme.colors.foreground;
        let accent_color = &theme.colors.primary;

        let style = Style::default()
            .fg(ratatui::style::Color::from_str(fg_color).unwrap_or(ratatui::style::Color::White));
        let accent_style = Style::default()
            .fg(ratatui::style::Color::from_str(accent_color)
                .unwrap_or(ratatui::style::Color::Cyan))
            .add_modifier(Modifier::BOLD);

        let focused_style = if is_focused {
            style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            style
        };

        // Render checkbox: [x] Label or [ ] Label
        let symbol = self.checkbox.symbol();
        let label = &self.checkbox.label;

        let spans = vec![
            Span::styled(symbol, accent_style),
            Span::raw(" "),
            Span::styled(label.clone(), focused_style),
        ];

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line);
        paragraph.render(area, buf);
    }

    fn handle_input(&mut self, key: KeyEvent, focus_id: Option<&str>) -> Option<Action> {
        // Only handle input if this checkbox is focused
        if focus_id != Some(self.checkbox.id.as_str()) {
            return None;
        }

        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.checkbox.toggle();
                Some(Action::Toggle)
            }
            _ => None,
        }
    }

    fn focusable_ids(&self) -> Vec<String> {
        vec![self.checkbox.id.clone()]
    }

    fn height(&self, _width: u16) -> u16 {
        1
    }
}

/// Convenience function to create a text section
///
/// # Example
///
/// ```rust
/// use rightclick::modal::section;
///
/// let text_section = section::text("Hello, World!");
/// ```
pub fn text<T: Into<String>>(text: T) -> Box<dyn Section> {
    Box::new(TextSection::new(text))
}

/// Convenience function to create a spacer section
///
/// # Example
///
/// ```rust
/// use rightclick::modal::section;
///
/// let spacer = section::spacer();
/// ```
pub fn spacer() -> Box<dyn Section> {
    Box::new(SpacerSection::new(1))
}

/// Convenience function to create a spacer section with custom height
///
/// # Example
///
/// ```rust
/// use rightclick::modal::section;
///
/// let spacer = section::spacer_lines(2);
/// ```
pub fn spacer_lines(lines: u16) -> Box<dyn Section> {
    Box::new(SpacerSection::new(lines))
}

/// Convenience function to create a button section
///
/// # Example
///
/// ```rust
/// use rightclick::modal::{section, Button, ButtonVariant};
///
/// let button_section = section::buttons(vec![
///     Button::primary("ok", "OK"),
///     Button::secondary("cancel", "Cancel"),
/// ]);
/// ```
pub fn buttons(btns: Vec<Button>) -> Box<dyn Section> {
    Box::new(ButtonSection::new(btns))
}

/// Convenience function to create a checkbox section
///
/// # Example
///
/// ```rust
/// use rightclick::modal::section;
///
/// let checkbox_section = section::checkbox("remember", "Remember me", false);
/// ```
pub fn checkbox(id: &str, label: &str, checked: bool) -> Box<dyn Section> {
    Box::new(CheckboxSection::new(id, label, checked))
}

use std::str::FromStr;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_section_new() {
        let section = TextSection::new("Hello");
        assert_eq!(section.text, "Hello");
    }

    #[test]
    fn text_section_with_alignment() {
        let section = TextSection::new("Centered").with_alignment(Alignment::Center);
        assert_eq!(section.alignment, Alignment::Center);
    }

    #[test]
    fn text_section_focusable_ids() {
        let section = TextSection::new("Hello");
        assert!(section.focusable_ids().is_empty());
    }

    #[test]
    fn spacer_section_new() {
        let section = SpacerSection::new(2);
        assert_eq!(section.lines, 2);
        assert_eq!(section.height(100), 2);
    }

    #[test]
    fn button_section_new() {
        let buttons = vec![Button::primary("ok", "OK")];
        let section = ButtonSection::new(buttons);
        assert_eq!(section.buttons.len(), 1);
        assert_eq!(section.height(100), 1);
    }

    #[test]
    fn button_section_focusable_ids() {
        let section = ButtonSection::new(vec![
            Button::primary("ok", "OK"),
            Button::secondary("cancel", "Cancel"),
        ]);
        let ids = section.focusable_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"ok".to_string()));
        assert!(ids.contains(&"cancel".to_string()));
    }

    #[test]
    fn checkbox_section_new() {
        let section = CheckboxSection::new("test", "Test Label", false);
        assert_eq!(section.checkbox.id, "test");
        assert_eq!(section.checkbox.label, "Test Label");
        assert!(!section.checkbox.checked);
    }

    #[test]
    fn checkbox_section_focusable_ids() {
        let section = CheckboxSection::new("remember", "Remember", false);
        let ids = section.focusable_ids();
        assert_eq!(ids, vec!["remember"]);
    }

    #[test]
    fn convenience_functions() {
        let _ = text("Hello");
        let _ = spacer();
        let _ = spacer_lines(2);
        let _ = buttons(vec![Button::primary("ok", "OK")]);
        let _ = checkbox("test", "Test", false);
    }

    #[test]
    fn text_section_height_calculation() {
        let section = TextSection::new("Hello World");
        // "Hello World" is 11 chars, at width 5 that's 3 lines
        assert_eq!(section.height(5), 3);

        let empty = TextSection::new("");
        assert_eq!(empty.height(10), 1);
    }

    #[test]
    fn button_action_struct() {
        let action = ButtonAction {
            button_id: "confirm".to_string(),
        };
        assert_eq!(action.button_id, "confirm");
    }
}
