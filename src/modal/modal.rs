//! Modal widget for displaying dialogs and prompts
//!
//! This module provides the main `Modal` widget for creating overlay dialogs
//! in the TUI interface. Modals can contain multiple sections and support
//! keyboard navigation and focus management.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Clear, Widget},
};

use crate::core::models::Theme;
use crate::keymap::Action;

use super::section::Section;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Visual variant of a modal
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum ModalVariant {
    /// Default neutral styling
    #[default]
    Default,
    /// Danger/error styling (red accents)
    Danger,
    /// Warning styling (yellow/orange accents)
    Warning,
    /// Info styling (blue accents)
    Info,
    /// Success styling (green accents)
    Success,
}

impl ModalVariant {
    /// Returns the color key for this variant
    pub fn color_key(&self) -> &'static str {
        match self {
            Self::Default => "border",
            Self::Danger => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Success => "success",
        }
    }

    /// Returns the border symbol for the top-left corner
    pub fn top_left(&self) -> &'static str {
        "╭"
    }

    /// Returns the border symbol for the top-right corner
    pub fn top_right(&self) -> &'static str {
        "╮"
    }

    /// Returns the border symbol for the bottom-left corner
    pub fn bottom_left(&self) -> &'static str {
        "╰"
    }

    /// Returns the border symbol for the bottom-right corner
    pub fn bottom_right(&self) -> &'static str {
        "╯"
    }

    /// Returns the horizontal border character
    pub fn horizontal(&self) -> &'static str {
        "─"
    }

    /// Returns the vertical border character
    pub fn vertical(&self) -> &'static str {
        "│"
    }
}

/// A modal dialog widget
///
/// Modals are overlay dialogs that appear on top of the main UI. They
/// support multiple content sections, keyboard navigation, and various
/// visual styles.
///
/// # Example
///
/// ```rust
/// use rightclick::modal::{Modal, ModalVariant, section, Button};
///
/// let mut modal = Modal::new("Confirm Delete")
///     .with_variant(ModalVariant::Danger)
///     .with_width(50);
///
/// modal.add_section(section::text("Are you sure you want to delete this file?"));
/// modal.add_section(section::spacer());
/// modal.add_section(section::buttons(vec![
///     Button::danger("delete", "Delete"),
///     Button::secondary("cancel", "Cancel"),
/// ]));
/// ```
#[derive(Default)]
pub struct Modal {
    /// Modal title displayed in the header
    pub title: String,
    /// Visual variant for styling
    pub variant: ModalVariant,
    /// Width of the modal in characters
    pub width: u16,
    /// Content sections
    pub sections: Vec<Box<dyn Section>>,
    /// Whether to show keyboard hints
    pub show_hints: bool,
    /// Label for the primary action (for hints)
    pub primary_action: Option<String>,
    /// Whether to close when clicking outside (backdrop)
    pub close_on_backdrop: bool,
    /// Current focus index
    pub focus_index: usize,
    /// Currently hovered element ID
    hover_id: Option<String>,
    /// All focusable element IDs across all sections
    focusable_ids: Vec<String>,
}

impl Modal {
    /// Creates a new modal with the given title
    ///
    /// # Arguments
    ///
    /// * `title` - The modal title
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::Modal;
    ///
    /// let modal = Modal::new("My Modal");
    /// ```
    pub fn new<T: Into<String>>(title: T) -> Self {
        Self {
            title: title.into(),
            variant: ModalVariant::Default,
            width: 60,
            sections: Vec::new(),
            show_hints: true,
            primary_action: None,
            close_on_backdrop: true,
            focus_index: 0,
            hover_id: None,
            focusable_ids: Vec::new(),
        }
    }

    /// Sets the visual variant
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::{Modal, ModalVariant};
    ///
    /// let modal = Modal::new("Warning").with_variant(ModalVariant::Warning);
    /// ```
    pub fn with_variant(mut self, variant: ModalVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets the modal width
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::Modal;
    ///
    /// let modal = Modal::new("Wide Modal").with_width(80);
    /// ```
    pub fn with_width(mut self, width: u16) -> Self {
        self.width = width;
        self
    }

    /// Sets whether to show keyboard hints
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::Modal;
    ///
    /// let modal = Modal::new("Modal").with_show_hints(false);
    /// ```
    pub fn with_show_hints(mut self, show: bool) -> Self {
        self.show_hints = show;
        self
    }

    /// Sets the primary action label for hints
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::Modal;
    ///
    /// let modal = Modal::new("Modal").with_primary_action("Confirm");
    /// ```
    pub fn with_primary_action<T: Into<String>>(mut self, action: T) -> Self {
        self.primary_action = Some(action.into());
        self
    }

    /// Sets whether the modal closes when clicking outside
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::Modal;
    ///
    /// let modal = Modal::new("Modal").with_close_on_backdrop(false);
    /// ```
    pub fn with_close_on_backdrop(mut self, close: bool) -> Self {
        self.close_on_backdrop = close;
        self
    }

    /// Adds a section to the modal
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::{Modal, section};
    ///
    /// let mut modal = Modal::new("Modal");
    /// modal.add_section(section::text("Hello"));
    /// ```
    pub fn add_section(&mut self, section: Box<dyn Section>) {
        // Update focusable IDs
        self.focusable_ids.extend(section.focusable_ids());
        self.sections.push(section);
    }

    /// Returns the ID of the currently focused element
    pub fn focused_id(&self) -> Option<&str> {
        self.focusable_ids.get(self.focus_index).map(|s| s.as_str())
    }

    /// Calculates the total height needed for this modal
    fn calculate_height(&self, width: u16) -> u16 {
        // Header: 3 lines (top border + title + separator)
        // Footer: 2 lines (separator + bottom border) if hints enabled, 1 otherwise
        // Content: sum of section heights
        let header_height = 3;
        let footer_height = if self.show_hints { 2 } else { 1 };
        let content_height: u16 = self.sections.iter().map(|s| s.height(width - 4)).sum(); // -4 for padding

        header_height + content_height + footer_height
    }

    /// Renders the modal
    ///
    /// # Arguments
    ///
    /// * `area` - The available area
    /// * `buf` - The buffer to render to
    /// * `theme` - The current theme
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let height = self.calculate_height(self.width);
        let width = self.width.min(area.width.saturating_sub(4));

        // Center the modal
        let modal_area = Self::centered_rect(width, height, area);

        // Clear the background
        Clear.render(modal_area, buf);

        // Get border color based on variant
        let border_color_str = match self.variant {
            ModalVariant::Default => &theme.colors.border,
            ModalVariant::Danger => &theme.colors.error,
            ModalVariant::Warning => &theme.colors.warning,
            ModalVariant::Info => &theme.colors.info,
            ModalVariant::Success => &theme.colors.success,
        };

        let border_color = ratatui::style::Color::from_str(border_color_str)
            .unwrap_or(ratatui::style::Color::Gray);

        let bg_color = ratatui::style::Color::from_str(&theme.ui_colors.popup_bg)
            .unwrap_or(ratatui::style::Color::Black);

        let fg_color = ratatui::style::Color::from_str(&theme.ui_colors.popup_fg)
            .unwrap_or(ratatui::style::Color::White);

        // Render border and background
        self.render_border(modal_area, buf, border_color, bg_color);

        // Render title
        self.render_title(modal_area, buf, fg_color, border_color);

        // Render content sections
        self.render_content(modal_area, buf, theme);

        // Render hints if enabled
        if self.show_hints {
            self.render_hints(modal_area, buf, theme);
        }
    }

    /// Renders the modal border
    fn render_border(
        &self,
        area: Rect,
        buf: &mut Buffer,
        border_color: ratatui::style::Color,
        bg_color: ratatui::style::Color,
    ) {
        let style = Style::default().fg(border_color).bg(bg_color);

        // Top border with title
        buf.set_string(
            area.x,
            area.y,
            format!("{}╭", self.variant.top_left()),
            style,
        );
        for x in 1..area.width - 1 {
            buf.set_string(area.x + x, area.y, self.variant.horizontal(), style);
        }
        buf.set_string(
            area.x + area.width - 1,
            area.y,
            self.variant.top_right(),
            style,
        );

        // Side borders
        for y in 1..area.height - 1 {
            buf.set_string(area.x, area.y + y, self.variant.vertical(), style);
            buf.set_string(
                area.x + area.width - 1,
                area.y + y,
                self.variant.vertical(),
                style,
            );
        }

        // Bottom border
        buf.set_string(
            area.x,
            area.y + area.height - 1,
            self.variant.bottom_left(),
            style,
        );
        for x in 1..area.width - 1 {
            buf.set_string(
                area.x + x,
                area.y + area.height - 1,
                self.variant.horizontal(),
                style,
            );
        }
        buf.set_string(
            area.x + area.width - 1,
            area.y + area.height - 1,
            self.variant.bottom_right(),
            style,
        );
    }

    /// Renders the modal title
    fn render_title(
        &self,
        area: Rect,
        buf: &mut Buffer,
        fg_color: ratatui::style::Color,
        border_color: ratatui::style::Color,
    ) {
        let title_style = Style::default().fg(fg_color).add_modifier(Modifier::BOLD);

        // Title on second line
        let title_x = area.x + 2;
        let title_text = truncate_display_width(
            &format!(" {} ", self.title),
            area.width.saturating_sub(4) as usize,
        );
        buf.set_string(title_x, area.y + 1, &title_text, title_style);

        // Separator line
        let sep_style = Style::default().fg(border_color);
        buf.set_string(area.x + 1, area.y + 2, "─", sep_style);
        for x in 2..area.width - 1 {
            buf.set_string(area.x + x, area.y + 2, "─", sep_style);
        }
        buf.set_string(area.x + area.width - 1, area.y + 2, "─", sep_style);
    }

    /// Renders the content sections
    fn render_content(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let content_start_y = area.y + 3;
        let content_width = area.width.saturating_sub(4); // 2 padding each side
        let mut current_y = content_start_y;

        let focus_id = self.focused_id();

        for section in &self.sections {
            let section_height = section.height(content_width);
            let section_area = Rect {
                x: area.x + 2,
                y: current_y,
                width: content_width,
                height: section_height,
            };

            section.render(section_area, buf, theme, focus_id, self.hover_id.as_deref());
            current_y += section_height;
        }
    }

    /// Renders keyboard hints at the bottom
    fn render_hints(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let hint_y = area.y + area.height - 2;
        let muted_color = ratatui::style::Color::from_str(&theme.colors.muted)
            .unwrap_or(ratatui::style::Color::Gray);
        let hint_style = Style::default().fg(muted_color);

        let hint_text = modal_hint_text(
            self.primary_action.as_deref(),
            self.has_focusable_elements(),
        );

        // Center the hints
        let hint_width = unicode_width::UnicodeWidthStr::width(hint_text.as_str()) as u16;
        let hint_x = area.x + (area.width.saturating_sub(hint_width)) / 2;

        buf.set_string(hint_x, hint_y, hint_text, hint_style);

        // Bottom separator
        let border_color = match self.variant {
            ModalVariant::Default => &theme.colors.border,
            ModalVariant::Danger => &theme.colors.error,
            ModalVariant::Warning => &theme.colors.warning,
            ModalVariant::Info => &theme.colors.info,
            ModalVariant::Success => &theme.colors.success,
        };
        let sep_style = Style::default()
            .fg(ratatui::style::Color::from_str(border_color)
                .unwrap_or(ratatui::style::Color::Gray));
        buf.set_string(area.x + 1, hint_y - 1, "─", sep_style);
        for x in 2..area.width - 1 {
            buf.set_string(area.x + x, hint_y - 1, "─", sep_style);
        }
        buf.set_string(area.x + area.width - 1, hint_y - 1, "─", sep_style);
    }

    /// Helper to center a rectangle within another
    fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(height),
                Constraint::Min(0),
            ])
            .split(area);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(width),
                Constraint::Min(0),
            ])
            .split(popup_layout[1])[1]
    }

    /// Handles a key event
    ///
    /// Returns `Some(Action)` if the key was handled by the modal or its sections,
    /// `None` otherwise.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc => {
                if self.close_on_backdrop {
                    return Some(Action::Back);
                }
                None
            }
            KeyCode::Tab => {
                self.cycle_focus();
                Some(Action::Refresh) // Trigger a refresh to update focus display
            }
            KeyCode::BackTab => {
                self.cycle_focus_reverse();
                Some(Action::Refresh)
            }
            _ => {
                // Pass to focused section
                let focus_id = self.focused_id().map(|s| s.to_string());
                for section in &mut self.sections {
                    if let Some(action) = section.handle_input(key, focus_id.as_deref()) {
                        return Some(action);
                    }
                }
                None
            }
        }
    }

    /// Cycles focus to the next focusable element
    fn cycle_focus(&mut self) {
        if self.focusable_ids.is_empty() {
            return;
        }
        self.focus_index = (self.focus_index + 1) % self.focusable_ids.len();
    }

    /// Cycles focus to the previous focusable element
    fn cycle_focus_reverse(&mut self) {
        if self.focusable_ids.is_empty() {
            return;
        }
        if self.focus_index == 0 {
            self.focus_index = self.focusable_ids.len() - 1;
        } else {
            self.focus_index -= 1;
        }
    }

    /// Returns whether there are any focusable elements
    fn has_focusable_elements(&self) -> bool {
        !self.focusable_ids.is_empty()
    }

    /// Updates the hover state
    pub fn set_hover(&mut self, id: Option<String>) {
        self.hover_id = id;
    }

    /// Returns the current hover ID
    pub fn hover_id(&self) -> Option<&str> {
        self.hover_id.as_deref()
    }
}

use std::str::FromStr;

fn truncate_display_width(value: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    if value.width() <= max_width {
        return value.to_string();
    }

    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let mut output = String::new();
    let mut width = 0;
    for ch in value.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if width + ch_width + 3 > max_width {
            break;
        }
        output.push(ch);
        width += ch_width;
    }
    output.push_str("...");
    output
}

fn modal_hint_text(primary_action: Option<&str>, has_focusable_elements: bool) -> String {
    let mut hints = vec!["Esc: Close".to_string()];

    if let Some(action) = primary_action {
        hints.push(format!("Enter: {}", action));
    }

    if has_focusable_elements {
        hints.push("Tab: Navigate".to_string());
    }

    hints.join("  |  ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modal_new() {
        let modal = Modal::new("Test Modal");
        assert_eq!(modal.title, "Test Modal");
        assert_eq!(modal.variant, ModalVariant::Default);
        assert_eq!(modal.width, 60);
        assert!(modal.sections.is_empty());
    }

    #[test]
    fn modal_with_variant() {
        let modal = Modal::new("Test").with_variant(ModalVariant::Danger);
        assert_eq!(modal.variant, ModalVariant::Danger);
    }

    #[test]
    fn modal_with_width() {
        let modal = Modal::new("Test").with_width(80);
        assert_eq!(modal.width, 80);
    }

    #[test]
    fn modal_with_show_hints() {
        let modal = Modal::new("Test").with_show_hints(false);
        assert!(!modal.show_hints);
    }

    #[test]
    fn modal_with_primary_action() {
        let modal = Modal::new("Test").with_primary_action("Confirm");
        assert_eq!(modal.primary_action, Some("Confirm".to_string()));
    }

    #[test]
    fn modal_hint_text_uses_compact_action_case() {
        assert_eq!(modal_hint_text(None, false), "Esc: Close");
        assert_eq!(
            modal_hint_text(Some("Confirm"), false),
            "Esc: Close  |  Enter: Confirm"
        );
        assert_eq!(
            modal_hint_text(Some("Save changes"), true),
            "Esc: Close  |  Enter: Save changes  |  Tab: Navigate"
        );
    }

    #[test]
    fn modal_with_close_on_backdrop() {
        let modal = Modal::new("Test").with_close_on_backdrop(false);
        assert!(!modal.close_on_backdrop);
    }

    #[test]
    fn modal_variant_color_keys() {
        assert_eq!(ModalVariant::Default.color_key(), "border");
        assert_eq!(ModalVariant::Danger.color_key(), "error");
        assert_eq!(ModalVariant::Warning.color_key(), "warning");
        assert_eq!(ModalVariant::Info.color_key(), "info");
        assert_eq!(ModalVariant::Success.color_key(), "success");
    }

    #[test]
    fn modal_variant_borders() {
        assert_eq!(ModalVariant::Default.top_left(), "╭");
        assert_eq!(ModalVariant::Default.top_right(), "╮");
        assert_eq!(ModalVariant::Default.bottom_left(), "╰");
        assert_eq!(ModalVariant::Default.bottom_right(), "╯");
        assert_eq!(ModalVariant::Default.horizontal(), "─");
        assert_eq!(ModalVariant::Default.vertical(), "│");
    }

    #[test]
    fn modal_default() {
        let modal: Modal = Default::default();
        assert!(modal.title.is_empty());
        assert_eq!(modal.variant, ModalVariant::Default);
    }

    #[test]
    fn modal_centered_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = Modal::centered_rect(40, 20, area);

        assert_eq!(centered.width, 40);
        assert_eq!(centered.height, 20);
        assert_eq!(centered.x, 30); // (100 - 40) / 2
        assert_eq!(centered.y, 15); // (50 - 20) / 2
    }

    #[test]
    fn modal_focus_management() {
        use crate::modal::section::CheckboxSection;

        let mut modal = Modal::new("Test");
        modal.add_section(Box::new(CheckboxSection::new("cb1", "Checkbox 1", false)));
        modal.add_section(Box::new(CheckboxSection::new("cb2", "Checkbox 2", false)));

        assert_eq!(modal.focusable_ids.len(), 2);
        assert_eq!(modal.focus_index, 0);
        assert_eq!(modal.focused_id(), Some("cb1"));

        modal.cycle_focus();
        assert_eq!(modal.focus_index, 1);
        assert_eq!(modal.focused_id(), Some("cb2"));

        modal.cycle_focus();
        assert_eq!(modal.focus_index, 0); // Wraps around

        modal.cycle_focus_reverse();
        assert_eq!(modal.focus_index, 1); // Wraps to end
    }

    #[test]
    fn modal_handle_esc() {
        let mut modal = Modal::new("Test");
        let key = KeyEvent::from(KeyCode::Esc);

        let action = modal.handle_key(key);
        assert!(matches!(action, Some(Action::Back)));

        // Test with close_on_backdrop disabled
        modal.close_on_backdrop = false;
        let action = modal.handle_key(key);
        assert!(action.is_none());
    }

    #[test]
    fn modal_handle_tab() {
        use crate::modal::section::CheckboxSection;

        let mut modal = Modal::new("Test");
        modal.add_section(Box::new(CheckboxSection::new("cb1", "Checkbox 1", false)));
        modal.add_section(Box::new(CheckboxSection::new("cb2", "Checkbox 2", false)));

        let key = KeyEvent::from(KeyCode::Tab);
        let action = modal.handle_key(key);

        assert!(matches!(action, Some(Action::Refresh)));
        assert_eq!(modal.focus_index, 1);
    }

    #[test]
    fn modal_has_focusable_elements() {
        use crate::modal::section::CheckboxSection;

        let mut modal = Modal::new("Test");
        assert!(!modal.has_focusable_elements());

        modal.add_section(Box::new(CheckboxSection::new("cb1", "Checkbox 1", false)));
        assert!(modal.has_focusable_elements());
    }

    #[test]
    fn modal_hover_state() {
        let mut modal = Modal::new("Test");
        assert!(modal.hover_id().is_none());

        modal.set_hover(Some("button1".to_string()));
        assert_eq!(modal.hover_id(), Some("button1"));

        modal.set_hover(None);
        assert!(modal.hover_id().is_none());
    }

    #[test]
    fn modal_calculate_height() {
        use crate::modal::section::{SpacerSection, TextSection};

        let mut modal = Modal::new("Test");
        modal.show_hints = false;

        // Header: 3 lines
        assert_eq!(modal.calculate_height(60), 3 + 1); // header + footer

        modal.add_section(Box::new(TextSection::new("Hello")));
        // Text section is 1 line at width 60
        assert_eq!(modal.calculate_height(60), 3 + 1 + 1); // header + content + footer

        modal.add_section(Box::new(SpacerSection::new(2)));
        assert_eq!(modal.calculate_height(60), 3 + 1 + 2 + 1);

        modal.show_hints = true;
        assert_eq!(modal.calculate_height(60), 3 + 1 + 2 + 2); // + hints line
    }

    #[test]
    fn truncate_display_width_handles_unicode_boundaries() {
        assert_eq!(truncate_display_width("Hello", 10), "Hello");
        assert_eq!(truncate_display_width("éclair session", 7), "écla...");
        assert_eq!(truncate_display_width("検索 session", 8), "検索 ...");
        assert_eq!(truncate_display_width("abc", 2), "..");
        assert_eq!(truncate_display_width("abc", 0), "");
    }

    #[test]
    fn modal_title_truncates_before_right_border() {
        let modal = Modal::new("検索 session title that is far too long");
        let theme = Theme::default();
        let area = Rect::new(0, 0, 30, 8);
        let mut buf = Buffer::empty(area);

        modal.render(area, &mut buf, &theme);

        let modal_area =
            Modal::centered_rect(modal.width.min(area.width.saturating_sub(4)), 5, area);
        let right_border_x = modal_area.x + modal_area.width - 1;

        assert_eq!(
            buf.cell((right_border_x, modal_area.y + 1))
                .unwrap()
                .symbol(),
            "│"
        );
    }
}
