//! Scrollable view state management
//!
//! This module provides utilities for managing scroll state in scrollable
//! views, including line-based scrolling, page scrolling, and cursor following.

/// Scroll state for managing view position in scrollable content
///
/// Tracks the current offset, total content size, and visible area to
/// provide convenient scrolling operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollState {
    /// Current scroll offset (0-indexed, number of lines scrolled)
    pub offset: usize,
    /// Total number of lines in the content
    pub total_lines: usize,
    /// Number of lines that can be displayed at once
    pub visible_lines: usize,
    /// Current cursor line position (if following cursor)
    pub cursor_line: Option<usize>,
}

impl ScrollState {
    /// Create a new scroll state
    ///
    /// # Arguments
    ///
    /// * `visible_lines` - Number of lines visible at once
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let scroll = ScrollState::new(24);
    /// ```
    pub fn new(visible_lines: usize) -> Self {
        Self {
            offset: 0,
            total_lines: 0,
            visible_lines: visible_lines.max(1),
            cursor_line: None,
        }
    }

    /// Set the total number of lines
    ///
    /// # Arguments
    ///
    /// * `total` - Total number of lines
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(24);
    /// scroll.set_total_lines(100);
    /// ```
    pub fn set_total_lines(&mut self, total: usize) {
        self.total_lines = total;
        self.clamp_offset();
    }

    /// Set the number of visible lines
    ///
    /// # Arguments
    ///
    /// * `visible` - Number of visible lines
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(24);
    /// scroll.set_visible_lines(40);
    /// ```
    pub fn set_visible_lines(&mut self, visible: usize) {
        self.visible_lines = visible.max(1);
        self.clamp_offset();
    }

    /// Clamp the offset to valid range
    fn clamp_offset(&mut self) {
        let max_offset = self.max_offset();
        self.offset = self.offset.min(max_offset);
    }

    /// Get the maximum valid offset
    fn max_offset(&self) -> usize {
        self.total_lines.saturating_sub(self.visible_lines)
    }

    /// Scroll up by a number of lines
    ///
    /// # Arguments
    ///
    /// * `lines` - Number of lines to scroll
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.scroll_down(20);
    /// scroll.scroll_up(5);
    /// assert_eq!(scroll.offset, 15);
    /// ```
    pub fn scroll_up(&mut self, lines: usize) {
        self.offset = self.offset.saturating_sub(lines);
    }

    /// Scroll down by a number of lines
    ///
    /// # Arguments
    ///
    /// * `lines` - Number of lines to scroll
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.scroll_down(20);
    /// assert_eq!(scroll.offset, 20);
    /// ```
    pub fn scroll_down(&mut self, lines: usize) {
        self.offset = self.offset.saturating_add(lines).min(self.max_offset());
    }

    /// Scroll to a specific line
    ///
    /// The offset is adjusted so the specified line is at the top of the view.
    ///
    /// # Arguments
    ///
    /// * `line` - Line number to scroll to (0-indexed)
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.scroll_to(50);
    /// assert_eq!(scroll.offset, 50);
    /// ```
    pub fn scroll_to(&mut self, line: usize) {
        self.offset = line.min(self.max_offset());
    }

    /// Scroll to the top
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.scroll_to(50);
    /// scroll.scroll_to_top();
    /// assert_eq!(scroll.offset, 0);
    /// ```
    pub fn scroll_to_top(&mut self) {
        self.offset = 0;
    }

    /// Scroll to the bottom
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.scroll_to_bottom();
    /// assert_eq!(scroll.offset, 90);
    /// ```
    pub fn scroll_to_bottom(&mut self) {
        self.offset = self.max_offset();
    }

    /// Scroll up one page
    ///
    /// Scrolls up by the number of visible lines.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.scroll_to(50);
    /// scroll.page_up();
    /// assert_eq!(scroll.offset, 40);
    /// ```
    pub fn page_up(&mut self) {
        self.scroll_up(self.visible_lines);
    }

    /// Scroll down one page
    ///
    /// Scrolls down by the number of visible lines.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.page_down();
    /// assert_eq!(scroll.offset, 10);
    /// ```
    pub fn page_down(&mut self) {
        self.scroll_down(self.visible_lines);
    }

    /// Scroll to make a specific line visible
    ///
    /// If the line is already visible, does nothing. Otherwise scrolls
    /// minimally to bring the line into view.
    ///
    /// # Arguments
    ///
    /// * `line` - Line number to make visible
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.scroll_to(50);
    /// scroll.ensure_visible(5);  // Above current view
    /// assert_eq!(scroll.offset, 5);
    /// ```
    pub fn ensure_visible(&mut self, line: usize) {
        if line < self.offset {
            // Line is above current view
            self.offset = line;
        } else if line >= self.offset.saturating_add(self.visible_lines) {
            // Line is below current view
            self.offset = line.saturating_sub(self.visible_lines.saturating_sub(1));
            self.clamp_offset();
        }
    }

    /// Follow cursor position
    ///
    /// Keeps the cursor line in view, scrolling if necessary.
    ///
    /// # Arguments
    ///
    /// * `cursor_line` - Current cursor line
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.follow_cursor(50);
    /// assert_eq!(scroll.offset, 41); // Cursor at bottom of view
    /// ```
    pub fn follow_cursor(&mut self, cursor_line: usize) {
        self.cursor_line = Some(cursor_line);

        if cursor_line < self.offset {
            // Cursor above view
            self.offset = cursor_line;
        } else if cursor_line >= self.offset.saturating_add(self.visible_lines) {
            // Cursor below view
            self.offset = cursor_line.saturating_sub(self.visible_lines.saturating_sub(1));
        }

        self.clamp_offset();
    }

    /// Center the view on a specific line
    ///
    /// Scrolls so the specified line is in the middle of the view.
    ///
    /// # Arguments
    ///
    /// * `line` - Line to center on
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.center_on(50);
    /// assert_eq!(scroll.offset, 45); // 50 - 10/2
    /// ```
    pub fn center_on(&mut self, line: usize) {
        let half_visible = self.visible_lines / 2;
        self.offset = line.saturating_sub(half_visible);
        self.clamp_offset();
    }

    /// Check if we're at the top
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// assert!(scroll.is_at_top());
    /// scroll.scroll_down(5);
    /// assert!(!scroll.is_at_top());
    /// ```
    pub fn is_at_top(&self) -> bool {
        self.offset == 0
    }

    /// Check if we're at the bottom
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// assert!(!scroll.is_at_bottom());
    /// scroll.scroll_to_bottom();
    /// assert!(scroll.is_at_bottom());
    /// ```
    pub fn is_at_bottom(&self) -> bool {
        self.offset >= self.max_offset()
    }

    /// Check if a line is currently visible
    ///
    /// # Arguments
    ///
    /// * `line` - Line number to check
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.scroll_to(20);
    /// assert!(scroll.is_visible(20));
    /// assert!(scroll.is_visible(29));
    /// assert!(!scroll.is_visible(30));
    /// assert!(!scroll.is_visible(19));
    /// ```
    pub fn is_visible(&self, line: usize) -> bool {
        line >= self.offset && line < self.offset.saturating_add(self.visible_lines)
    }

    /// Get the visible line range
    ///
    /// Returns (start, end) where end is exclusive.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.scroll_to(20);
    /// assert_eq!(scroll.visible_range(), (20, 30));
    /// ```
    pub fn visible_range(&self) -> (usize, usize) {
        let end = self
            .offset
            .saturating_add(self.visible_lines)
            .min(self.total_lines);
        (self.offset, end)
    }

    /// Get the scroll percentage (0.0 to 1.0)
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.scroll_to(45);
    /// assert_eq!(scroll.scroll_percentage(), 0.5);
    /// ```
    pub fn scroll_percentage(&self) -> f32 {
        if self.total_lines <= self.visible_lines {
            return 0.0;
        }
        self.offset as f32 / self.max_offset() as f32
    }

    /// Get scrollbar thumb position and size
    ///
    /// Returns (thumb_position, thumb_size) in lines.
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(20);
    /// scroll.set_total_lines(100);
    /// let (pos, size) = scroll.scrollbar_info();
    /// assert!(size > 0);
    /// assert!(size <= scroll.visible_lines);
    /// ```
    pub fn scrollbar_info(&self) -> (usize, usize) {
        if self.visible_lines == 0 {
            return (0, 0);
        }

        if self.total_lines <= self.visible_lines {
            return (0, self.visible_lines);
        }

        let thumb_size = self
            .visible_lines
            .saturating_mul(self.visible_lines)
            .saturating_div(self.total_lines)
            .max(1);
        let max_thumb_pos = self.visible_lines.saturating_sub(thumb_size);
        let thumb_pos = ((self.offset * max_thumb_pos) / self.max_offset()).min(max_thumb_pos);

        (thumb_pos, thumb_size)
    }

    /// Get the line number at the top of the view
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.scroll_to(25);
    /// assert_eq!(scroll.top_line(), 25);
    /// ```
    pub fn top_line(&self) -> usize {
        self.offset
    }

    /// Get the line number at the bottom of the view (inclusive)
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::ScrollState;
    ///
    /// let mut scroll = ScrollState::new(10);
    /// scroll.set_total_lines(100);
    /// scroll.scroll_to(25);
    /// assert_eq!(scroll.bottom_line(), 34);
    /// ```
    pub fn bottom_line(&self) -> usize {
        self.offset
            .saturating_add(self.visible_lines.saturating_sub(1))
            .min(self.total_lines.saturating_sub(1))
    }
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new(24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_state_new() {
        let scroll = ScrollState::new(24);
        assert_eq!(scroll.offset, 0);
        assert_eq!(scroll.total_lines, 0);
        assert_eq!(scroll.visible_lines, 24);
    }

    #[test]
    fn test_scroll_set_total_lines() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        assert_eq!(scroll.total_lines, 100);
    }

    #[test]
    fn test_scroll_clamp_offset() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(20);
        scroll.offset = 15; // Beyond max
        scroll.clamp_offset();
        assert_eq!(scroll.offset, 10); // Clamped to max
    }

    #[test]
    fn test_scroll_up() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_to(50);
        scroll.scroll_up(5);
        assert_eq!(scroll.offset, 45);
    }

    #[test]
    fn test_scroll_up_saturating() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_up(5);
        assert_eq!(scroll.offset, 0);
    }

    #[test]
    fn test_scroll_down() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_down(20);
        assert_eq!(scroll.offset, 20);
    }

    #[test]
    fn test_scroll_down_clamping() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_down(200);
        assert_eq!(scroll.offset, 90); // Max offset
    }

    #[test]
    fn test_scroll_to() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_to(50);
        assert_eq!(scroll.offset, 50);
    }

    #[test]
    fn test_scroll_to_top() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_to(50);
        scroll.scroll_to_top();
        assert_eq!(scroll.offset, 0);
    }

    #[test]
    fn test_scroll_to_bottom() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_to_bottom();
        assert_eq!(scroll.offset, 90);
    }

    #[test]
    fn test_page_up() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_to(50);
        scroll.page_up();
        assert_eq!(scroll.offset, 40);
    }

    #[test]
    fn test_page_down() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.page_down();
        assert_eq!(scroll.offset, 10);
    }

    #[test]
    fn test_ensure_visible_above() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_to(50);
        scroll.ensure_visible(5);
        assert_eq!(scroll.offset, 5);
    }

    #[test]
    fn test_ensure_visible_below() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_to(50);
        scroll.ensure_visible(70);
        assert_eq!(scroll.offset, 61);
    }

    #[test]
    fn test_ensure_visible_already_visible() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_to(50);
        scroll.ensure_visible(55);
        assert_eq!(scroll.offset, 50);
    }

    #[test]
    fn test_follow_cursor() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.follow_cursor(50);
        assert_eq!(scroll.offset, 41); // Cursor at bottom
    }

    #[test]
    fn test_center_on() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.center_on(50);
        assert_eq!(scroll.offset, 45);
    }

    #[test]
    fn test_is_at_top() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        assert!(scroll.is_at_top());
        scroll.scroll_down(5);
        assert!(!scroll.is_at_top());
    }

    #[test]
    fn test_is_at_bottom() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        assert!(!scroll.is_at_bottom());
        scroll.scroll_to_bottom();
        assert!(scroll.is_at_bottom());
    }

    #[test]
    fn test_is_visible() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_to(20);
        assert!(scroll.is_visible(20));
        assert!(scroll.is_visible(29));
        assert!(!scroll.is_visible(30));
        assert!(!scroll.is_visible(19));
    }

    #[test]
    fn test_visible_range() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_to(20);
        assert_eq!(scroll.visible_range(), (20, 30));
    }

    #[test]
    fn test_scroll_percentage() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_to(45);
        assert_eq!(scroll.scroll_percentage(), 0.5);
    }

    #[test]
    fn test_scrollbar_info() {
        let mut scroll = ScrollState::new(20);
        scroll.set_total_lines(100);
        let (pos, size) = scroll.scrollbar_info();
        assert!(size > 0);
        assert!(size <= scroll.visible_lines);
        assert!(pos <= scroll.visible_lines - size);
    }

    #[test]
    fn test_zero_visible_lines_is_safe_for_visibility_helpers() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_to(25);
        scroll.visible_lines = 0;

        assert!(!scroll.is_visible(25));
        assert_eq!(scroll.visible_range(), (25, 25));
        assert_eq!(scroll.bottom_line(), 25);

        scroll.ensure_visible(40);
        assert_eq!(scroll.offset, 40);

        scroll.follow_cursor(60);
        assert_eq!(scroll.offset, 60);
    }

    #[test]
    fn test_zero_visible_lines_has_empty_scrollbar_info() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.visible_lines = 0;

        assert_eq!(scroll.scrollbar_info(), (0, 0));
    }

    #[test]
    fn test_top_line() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_to(25);
        assert_eq!(scroll.top_line(), 25);
    }

    #[test]
    fn test_bottom_line() {
        let mut scroll = ScrollState::new(10);
        scroll.set_total_lines(100);
        scroll.scroll_to(25);
        assert_eq!(scroll.bottom_line(), 34);
    }
}
