//! TTY output buffer management.
//!
//! This module provides the [`OutputBuffer`] struct for managing terminal output
//! from a tmux pane. It handles line storage, cursor tracking, and scrollback
//! management for terminal display.
//!
//! # Architecture
//!
//! The output buffer is part of the Functional Core - it contains pure logic
//! for managing terminal output without any I/O operations. It maintains:
//!
//! - A vector of output lines
//! - Cursor position tracking
//! - Configurable scrollback limit

/// Default maximum scrollback lines.
pub const DEFAULT_SCROLLBACK: usize = 600;

/// Buffer for storing and managing terminal output.
///
/// The output buffer stores lines of text from a tmux pane and tracks the
/// cursor position. It supports scrollback limiting to prevent unbounded
/// memory growth.
///
/// # Example
///
/// ```
/// use rightclick::tty::OutputBuffer;
///
/// let mut buffer = OutputBuffer::new(100);
/// buffer.append("Hello, World!\n");
/// buffer.append("Second line");
///
/// assert_eq!(buffer.lines.len(), 2);
/// assert_eq!(buffer.cursor_position(), (1, 11)); // row 1, col 11
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputBuffer {
    /// Lines of text in the buffer.
    pub lines: Vec<String>,

    /// Current cursor row (0-indexed).
    pub cursor_row: usize,

    /// Current cursor column (0-indexed).
    pub cursor_col: usize,

    /// Maximum number of lines to retain (scrollback limit).
    pub scrollback: usize,
}

impl OutputBuffer {
    /// Creates a new output buffer with the specified scrollback limit.
    ///
    /// # Arguments
    ///
    /// * `scrollback` - Maximum number of lines to retain
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::OutputBuffer;
    ///
    /// let buffer = OutputBuffer::new(1000);
    /// assert_eq!(buffer.scrollback, 1000);
    /// assert!(buffer.lines.is_empty());
    /// assert_eq!(buffer.cursor_row, 0);
    /// assert_eq!(buffer.cursor_col, 0);
    /// ```
    pub fn new(scrollback: usize) -> Self {
        Self {
            lines: Vec::new(),
            cursor_row: 0,
            cursor_col: 0,
            scrollback,
        }
    }

    /// Appends text to the buffer, processing newlines and updating cursor.
    ///
    /// This method handles:
    /// - Splitting text on newlines
    /// - Appending to existing lines or creating new ones
    /// - Updating cursor position
    /// - Enforcing scrollback limits
    ///
    /// # Arguments
    ///
    /// * `text` - The text to append
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::OutputBuffer;
    ///
    /// let mut buffer = OutputBuffer::new(100);
    /// buffer.append("Hello");
    /// buffer.append(", World!");
    ///
    /// assert_eq!(buffer.lines, vec!["Hello, World!"]);
    /// assert_eq!(buffer.cursor_position(), (0, 13));
    /// ```
    pub fn append(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        let segments: Vec<&str> = text.split('\n').collect();

        for (i, segment) in segments.iter().enumerate() {
            if i > 0 {
                // Newline encountered - move to next row
                self.cursor_row += 1;
                self.cursor_col = 0;

                // Add new empty line if needed
                if self.cursor_row >= self.lines.len() {
                    self.lines.push(String::new());
                }
            }

            // Append text to current line
            if !segment.is_empty() {
                if self.cursor_row >= self.lines.len() {
                    self.lines.push(segment.to_string());
                } else {
                    self.lines[self.cursor_row].push_str(segment);
                }
                self.cursor_col += segment.chars().count();
            }
        }

        self.enforce_scrollback();
    }

    /// Returns the visible range of lines based on the given height.
    ///
    /// This returns a slice of lines that would be visible in a terminal
    /// of the specified height, starting from the bottom (most recent).
    ///
    /// # Arguments
    ///
    /// * `height` - The number of lines that can be displayed
    ///
    /// # Returns
    ///
    /// A slice of the lines that should be visible
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::OutputBuffer;
    ///
    /// let mut buffer = OutputBuffer::new(100);
    /// buffer.append("Line 1\nLine 2\nLine 3\nLine 4\nLine 5");
    ///
    /// let visible = buffer.visible_range(3);
    /// assert_eq!(visible.len(), 3);
    /// assert_eq!(visible[0], "Line 3");
    /// ```
    pub fn visible_range(&self, height: usize) -> &[String] {
        let total_lines = self.lines.len();
        if total_lines == 0 {
            return &[];
        }

        let start = total_lines.saturating_sub(height);
        &self.lines[start..]
    }

    /// Returns the current cursor position as (row, col).
    ///
    /// The row is relative to the buffer's internal storage, not the
    /// visible area.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::OutputBuffer;
    ///
    /// let mut buffer = OutputBuffer::new(100);
    /// buffer.append("Hello\nWorld");
    ///
    /// assert_eq!(buffer.cursor_position(), (1, 5));
    /// ```
    pub fn cursor_position(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    /// Returns the cursor position relative to a visible window.
    ///
    /// If the cursor is outside the visible range, returns None.
    ///
    /// # Arguments
    ///
    /// * `height` - The height of the visible area
    ///
    /// # Returns
    ///
    /// Some((visible_row, col)) if cursor is visible, None otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::OutputBuffer;
    ///
    /// let mut buffer = OutputBuffer::new(100);
    /// buffer.append("Line 1\nLine 2\nLine 3\nLine 4\nLine 5");
    ///
    /// // Cursor is at line 5 (index 4), visible range is lines 2-4
    /// let visible_cursor = buffer.visible_cursor_position(3);
    /// assert_eq!(visible_cursor, Some((2, 6))); // Last line, 6 chars "Line 5"
    /// ```
    pub fn visible_cursor_position(&self, height: usize) -> Option<(usize, usize)> {
        let total_lines = self.lines.len();
        if total_lines == 0 {
            return None;
        }

        let start = total_lines.saturating_sub(height);
        if self.cursor_row < start {
            return None;
        }

        let visible_row = self.cursor_row - start;
        Some((visible_row, self.cursor_col))
    }

    /// Clears all lines from the buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::OutputBuffer;
    ///
    /// let mut buffer = OutputBuffer::new(100);
    /// buffer.append("Some text\nMore text");
    /// buffer.clear();
    ///
    /// assert!(buffer.lines.is_empty());
    /// assert_eq!(buffer.cursor_row, 0);
    /// assert_eq!(buffer.cursor_col, 0);
    /// ```
    pub fn clear(&mut self) {
        self.lines.clear();
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    /// Returns the total number of lines in the buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::OutputBuffer;
    ///
    /// let mut buffer = OutputBuffer::new(100);
    /// buffer.append("Line 1\nLine 2");
    ///
    /// assert_eq!(buffer.line_count(), 2);
    /// ```
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Returns the content as a single string with newlines.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::OutputBuffer;
    ///
    /// let mut buffer = OutputBuffer::new(100);
    /// buffer.append("Line 1\nLine 2");
    ///
    /// assert_eq!(buffer.to_string(), "Line 1\nLine 2");
    /// ```
    pub fn to_string(&self) -> String {
        self.lines.join("\n")
    }

    /// Enforces the scrollback limit by removing oldest lines if needed.
    fn enforce_scrollback(&mut self) {
        if self.lines.len() > self.scrollback {
            let to_remove = self.lines.len() - self.scrollback;
            self.lines.drain(0..to_remove);
            self.cursor_row = self.cursor_row.saturating_sub(to_remove);
        }
    }
}

impl Default for OutputBuffer {
    /// Creates a default output buffer with DEFAULT_SCROLLBACK lines.
    fn default() -> Self {
        Self::new(DEFAULT_SCROLLBACK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_buffer() {
        let buffer = OutputBuffer::new(100);
        assert_eq!(buffer.scrollback, 100);
        assert!(buffer.lines.is_empty());
        assert_eq!(buffer.cursor_row, 0);
        assert_eq!(buffer.cursor_col, 0);
    }

    #[test]
    fn default_buffer() {
        let buffer: OutputBuffer = Default::default();
        assert_eq!(buffer.scrollback, DEFAULT_SCROLLBACK);
    }

    #[test]
    fn append_single_line() {
        let mut buffer = OutputBuffer::new(100);
        buffer.append("Hello, World!");

        assert_eq!(buffer.lines, vec!["Hello, World!"]);
        assert_eq!(buffer.cursor_row, 0);
        assert_eq!(buffer.cursor_col, 13);
    }

    #[test]
    fn append_multiple_lines() {
        let mut buffer = OutputBuffer::new(100);
        buffer.append("Line 1\nLine 2\nLine 3");

        assert_eq!(buffer.lines, vec!["Line 1", "Line 2", "Line 3"]);
        assert_eq!(buffer.cursor_row, 2);
        assert_eq!(buffer.cursor_col, 6);
    }

    #[test]
    fn append_incrementally() {
        let mut buffer = OutputBuffer::new(100);
        buffer.append("Hello");
        buffer.append(", ");
        buffer.append("World!");

        assert_eq!(buffer.lines, vec!["Hello, World!"]);
        assert_eq!(buffer.cursor_col, 13);
    }

    #[test]
    fn append_with_newlines() {
        let mut buffer = OutputBuffer::new(100);
        buffer.append("First");
        buffer.append("\n");
        buffer.append("Second");

        assert_eq!(buffer.lines, vec!["First", "Second"]);
        assert_eq!(buffer.cursor_row, 1);
        assert_eq!(buffer.cursor_col, 6);
    }

    #[test]
    fn empty_append() {
        let mut buffer = OutputBuffer::new(100);
        buffer.append("");

        assert!(buffer.lines.is_empty());
        assert_eq!(buffer.cursor_row, 0);
        assert_eq!(buffer.cursor_col, 0);
    }

    #[test]
    fn visible_range_basic() {
        let mut buffer = OutputBuffer::new(100);
        buffer.append("Line 1\nLine 2\nLine 3\nLine 4\nLine 5");

        let visible = buffer.visible_range(3);
        assert_eq!(visible, vec!["Line 3", "Line 4", "Line 5"]);
    }

    #[test]
    fn visible_range_empty_buffer() {
        let buffer = OutputBuffer::new(100);
        let visible = buffer.visible_range(10);
        assert!(visible.is_empty());
    }

    #[test]
    fn visible_range_taller_than_content() {
        let mut buffer = OutputBuffer::new(100);
        buffer.append("Line 1\nLine 2");

        let visible = buffer.visible_range(10);
        assert_eq!(visible, vec!["Line 1", "Line 2"]);
    }

    #[test]
    fn visible_cursor_position_in_range() {
        let mut buffer = OutputBuffer::new(100);
        buffer.append("Line 1\nLine 2\nLine 3");

        let pos = buffer.visible_cursor_position(2);
        assert_eq!(pos, Some((1, 6)));
    }

    #[test]
    fn visible_cursor_position_out_of_range() {
        let mut buffer = OutputBuffer::new(100);
        buffer.append("Line 1\nLine 2\nLine 3");

        let pos = buffer.visible_cursor_position(1);
        assert_eq!(pos, Some((0, 6)));
    }

    #[test]
    fn scrollback_enforcement() {
        let mut buffer = OutputBuffer::new(3);
        buffer.append("Line 1\nLine 2\nLine 3\nLine 4\nLine 5");

        assert_eq!(buffer.lines.len(), 3);
        assert_eq!(buffer.lines, vec!["Line 3", "Line 4", "Line 5"]);
        assert_eq!(buffer.cursor_row, 2); // Adjusted after removal
    }

    #[test]
    fn clear_buffer() {
        let mut buffer = OutputBuffer::new(100);
        buffer.append("Some text\nMore text");
        buffer.clear();

        assert!(buffer.lines.is_empty());
        assert_eq!(buffer.cursor_row, 0);
        assert_eq!(buffer.cursor_col, 0);
    }

    #[test]
    fn line_count() {
        let mut buffer = OutputBuffer::new(100);
        assert_eq!(buffer.line_count(), 0);

        buffer.append("Line 1");
        assert_eq!(buffer.line_count(), 1);

        buffer.append("\nLine 2");
        assert_eq!(buffer.line_count(), 2);
    }

    #[test]
    fn to_string_format() {
        let mut buffer = OutputBuffer::new(100);
        buffer.append("Line 1\nLine 2\nLine 3");

        assert_eq!(buffer.to_string(), "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn cursor_position_updates_correctly() {
        let mut buffer = OutputBuffer::new(100);
        assert_eq!(buffer.cursor_position(), (0, 0));

        buffer.append("Test");
        assert_eq!(buffer.cursor_position(), (0, 4));

        buffer.append("\nLine");
        assert_eq!(buffer.cursor_position(), (1, 4));
    }
}
