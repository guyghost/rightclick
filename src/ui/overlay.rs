//! Modal overlay component for dialogs and popups
//!
//! This module provides an overlay component for rendering modal dialogs
//! on top of the main interface.

use crate::core::models::Theme;
use crate::theme::{style_for_ui_element, UiElement};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Modifier;
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

/// Modal overlay for displaying dialogs and popups
#[derive(Clone, Debug)]
pub struct Overlay {
    /// Content text to display in the overlay
    pub content: String,
    /// Width of the overlay in characters
    pub width: u16,
    /// Height of the overlay in lines
    pub height: u16,
    /// Optional title for the overlay
    pub title: Option<String>,
    /// Whether to wrap text
    pub wrap_text: bool,
}

impl Overlay {
    /// Create a new overlay with default dimensions
    ///
    /// # Arguments
    ///
    /// * `content` - The content to display
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Overlay;
    ///
    /// let overlay = Overlay::new("Hello, World!");
    /// ```
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            width: 60,
            height: 20,
            title: None,
            wrap_text: true,
        }
    }

    /// Set the dimensions of the overlay
    ///
    /// # Arguments
    ///
    /// * `width` - Width in characters
    /// * `height` - Height in lines
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Overlay;
    ///
    /// let overlay = Overlay::new("Content").with_dimensions(80, 24);
    /// ```
    pub fn with_dimensions(mut self, width: u16, height: u16) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set the title of the overlay
    ///
    /// # Arguments
    ///
    /// * `title` - The title to display
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Overlay;
    ///
    /// let overlay = Overlay::new("Content").with_title("Dialog");
    /// ```
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Enable or disable text wrapping
    ///
    /// # Arguments
    ///
    /// * `wrap` - Whether to wrap text
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Overlay;
    ///
    /// let overlay = Overlay::new("Content").with_wrap(false);
    /// ```
    pub fn with_wrap(mut self, wrap: bool) -> Self {
        self.wrap_text = wrap;
        self
    }

    /// Calculate the centered area for the overlay within the given area
    ///
    /// # Arguments
    ///
    /// * `parent_area` - The parent area to center within
    ///
    /// Returns the centered rectangle for the overlay
    fn centered_area(&self, parent_area: Rect) -> Rect {
        let x = parent_area
            .x
            .saturating_add((parent_area.width.saturating_sub(self.width)) / 2);
        let y = parent_area
            .y
            .saturating_add((parent_area.height.saturating_sub(self.height)) / 2);
        let width = self.width.min(parent_area.width);
        let height = self.height.min(parent_area.height);

        Rect::new(x, y, width, height)
    }

    /// Render the overlay to the buffer
    ///
    /// # Arguments
    ///
    /// * `background` - The background content type (e.g., "dim", "clear")
    /// * `area` - The parent area to render in
    /// * `buf` - The buffer to render to
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Overlay;
    /// use ratatui::layout::Rect;
    /// use ratatui::buffer::Buffer;
    ///
    /// let overlay = Overlay::new("Hello").with_title("Greeting");
    /// let area = Rect::new(0, 0, 80, 24);
    /// let mut buf = Buffer::empty(area);
    ///
    /// overlay.render("dim", area, &mut buf);
    /// ```
    pub fn render(&self, background: &str, area: Rect, buf: &mut Buffer) {
        // Calculate centered area
        let overlay_area = self.centered_area(area);

        // Handle background dimming
        if background == "dim" {
            self.dim_background(area, overlay_area, buf);
        } else if background == "clear" {
            Clear.render(overlay_area, buf);
        }

        // Create the block with optional title
        let popup_style = style_for_ui_element(&Theme::default(), UiElement::Popup);
        let border_style = style_for_ui_element(&Theme::default(), UiElement::Border);
        let text_style = style_for_ui_element(&Theme::default(), UiElement::Text);

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .style(popup_style);

        if let Some(ref title) = self.title {
            let title_style = style_for_ui_element(&Theme::default(), UiElement::Primary);
            block = block.title(Span::styled(
                title.clone(),
                title_style.add_modifier(Modifier::BOLD),
            ));
        }

        // Create paragraph with content
        let mut paragraph = Paragraph::new(self.content.clone())
            .block(block)
            .style(text_style)
            .alignment(Alignment::Left);

        if self.wrap_text {
            paragraph = paragraph.wrap(Wrap { trim: true });
        }

        paragraph.render(overlay_area, buf);
    }

    /// Dim the background area outside the overlay
    fn dim_background(&self, area: Rect, overlay_area: Rect, buf: &mut Buffer) {
        let dim_style = style_for_ui_element(&Theme::default(), UiElement::Background);

        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                // Check if this cell is outside the overlay
                let outside_overlay = x < overlay_area.left()
                    || x >= overlay_area.right()
                    || y < overlay_area.top()
                    || y >= overlay_area.bottom();

                if outside_overlay {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_style(dim_style);
                    }
                }
            }
        }
    }

    /// Render the overlay with a custom theme
    ///
    /// # Arguments
    ///
    /// * `background` - The background content type (e.g., "dim", "clear")
    /// * `area` - The parent area to render in
    /// * `buf` - The buffer to render to
    /// * `theme` - The theme to use for styling
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Overlay;
    /// use rightclick::core::models::Theme;
    /// use ratatui::layout::Rect;
    /// use ratatui::buffer::Buffer;
    ///
    /// let overlay = Overlay::new("Hello").with_title("Greeting");
    /// let area = Rect::new(0, 0, 80, 24);
    /// let mut buf = Buffer::empty(area);
    /// let theme = Theme::default();
    ///
    /// overlay.render_with_theme("dim", area, &mut buf, &theme);
    /// ```
    pub fn render_with_theme(&self, background: &str, area: Rect, buf: &mut Buffer, theme: &Theme) {
        // Calculate centered area
        let overlay_area = self.centered_area(area);

        // Handle background dimming
        if background == "dim" {
            let dim_style = style_for_ui_element(theme, UiElement::Background);
            for y in area.top()..area.bottom() {
                for x in area.left()..area.right() {
                    let outside_overlay = x < overlay_area.left()
                        || x >= overlay_area.right()
                        || y < overlay_area.top()
                        || y >= overlay_area.bottom();

                    if outside_overlay {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_style(dim_style);
                        }
                    }
                }
            }
        } else if background == "clear" {
            Clear.render(overlay_area, buf);
        }

        // Create the block with theme-aware styling
        let popup_style = style_for_ui_element(theme, UiElement::Popup);
        let border_style = style_for_ui_element(theme, UiElement::Border);
        let text_style = style_for_ui_element(theme, UiElement::Text);

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .style(popup_style);

        if let Some(ref title) = self.title {
            let title_style = style_for_ui_element(theme, UiElement::Primary);
            block = block.title(Span::styled(
                title.clone(),
                title_style.add_modifier(Modifier::BOLD),
            ));
        }

        // Create paragraph with content
        let mut paragraph = Paragraph::new(self.content.clone())
            .block(block)
            .style(text_style)
            .alignment(Alignment::Left);

        if self.wrap_text {
            paragraph = paragraph.wrap(Wrap { trim: true });
        }

        paragraph.render(overlay_area, buf);
    }
}

impl Default for Overlay {
    fn default() -> Self {
        Self::new("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_new() {
        let overlay = Overlay::new("Hello");
        assert_eq!(overlay.content, "Hello");
        assert_eq!(overlay.width, 60);
        assert_eq!(overlay.height, 20);
        assert!(overlay.title.is_none());
        assert!(overlay.wrap_text);
    }

    #[test]
    fn test_overlay_with_dimensions() {
        let overlay = Overlay::new("Hello").with_dimensions(80, 24);
        assert_eq!(overlay.width, 80);
        assert_eq!(overlay.height, 24);
    }

    #[test]
    fn test_overlay_with_title() {
        let overlay = Overlay::new("Hello").with_title("Dialog");
        assert_eq!(overlay.title, Some("Dialog".to_string()));
    }

    #[test]
    fn test_overlay_with_wrap() {
        let overlay = Overlay::new("Hello").with_wrap(false);
        assert!(!overlay.wrap_text);
    }

    #[test]
    fn test_overlay_centered_area() {
        let overlay = Overlay::new("Hello").with_dimensions(40, 10);
        let parent = Rect::new(0, 0, 100, 50);
        let centered = overlay.centered_area(parent);

        // Should be centered: (100-40)/2 = 30, (50-10)/2 = 20
        assert_eq!(centered.x, 30);
        assert_eq!(centered.y, 20);
        assert_eq!(centered.width, 40);
        assert_eq!(centered.height, 10);
    }

    #[test]
    fn test_overlay_centered_area_clamping() {
        let overlay = Overlay::new("Hello").with_dimensions(200, 100);
        let parent = Rect::new(0, 0, 80, 24);
        let centered = overlay.centered_area(parent);

        // Should be clamped to parent size
        assert_eq!(centered.width, 80);
        assert_eq!(centered.height, 24);
    }
}
