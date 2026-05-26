//! Pure text input state for RightClick
//!
//! This module provides `TextInputState`, a pure data structure for managing
//! text input state. It supports single-line and multi-line modes with cursor
//! navigation, text manipulation, and character limits.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// The input mode for a text input field
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Single line input (e.g., branch name, search query)
    SingleLine,
    /// Multi-line input (e.g., commit message)
    MultiLine,
}

/// Pure state for a text input field
///
/// This struct holds all the state needed to manage text input without
/// any rendering or I/O concerns. It follows the Functional Core pattern.
///
/// # Examples
///
/// ```
/// use rightclick::core::models::text_input::{TextInputState, InputMode};
///
/// let mut state = TextInputState::new(InputMode::SingleLine)
///     .with_placeholder("Enter branch name...");
///
/// state.insert_char('m');
/// state.insert_char('y');
/// state.insert_char('-');
/// state.insert_char('b');
/// assert_eq!(state.text(), "my-b");
/// assert_eq!(state.cursor_pos(), 4);
/// ```
#[derive(Debug, Clone)]
pub struct TextInputState {
    /// The current text content
    text: String,
    /// Current cursor position (byte offset)
    cursor_pos: usize,
    /// Input mode (single or multi line)
    mode: InputMode,
    /// Placeholder text shown when empty
    placeholder: String,
    /// Maximum character limit (0 = unlimited)
    char_limit: usize,
    /// Whether this input is currently active/focused
    active: bool,
    /// Optional validation error message
    error: Option<String>,
}

impl TextInputState {
    /// Creates a new text input state with the given mode
    pub fn new(mode: InputMode) -> Self {
        Self {
            text: String::new(),
            cursor_pos: 0,
            mode,
            placeholder: String::new(),
            char_limit: 0,
            active: false,
            error: None,
        }
    }

    /// Sets the placeholder text
    pub fn with_placeholder<S: Into<String>>(mut self, placeholder: S) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Sets the character limit (0 = unlimited)
    pub fn with_char_limit(mut self, limit: usize) -> Self {
        self.char_limit = limit;
        self
    }

    /// Sets initial text content
    pub fn with_text<S: Into<String>>(mut self, text: S) -> Self {
        let text = text.into();
        self.cursor_pos = text.len();
        self.text = text;
        self
    }

    /// Returns the current text content
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the placeholder text
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Returns the current cursor position (byte offset)
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Returns the input mode
    pub fn mode(&self) -> InputMode {
        self.mode
    }

    /// Returns whether the input is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Sets the active state
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    /// Returns the current error message, if any
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Sets the error message
    pub fn set_error<S: Into<String>>(&mut self, error: Option<S>) {
        self.error = error.map(|s| s.into());
    }

    /// Returns the character count
    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    /// Returns the character limit (0 = unlimited)
    pub fn char_limit(&self) -> usize {
        self.char_limit
    }

    /// Returns whether the input is empty
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Clears the text and resets cursor
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_pos = 0;
        self.error = None;
    }

    /// Sets text content directly and moves cursor to end
    pub fn set_text<S: Into<String>>(&mut self, text: S) {
        let text = text.into();
        self.cursor_pos = text.len();
        self.text = text;
    }

    /// Insert a character at the cursor position
    pub fn insert_char(&mut self, c: char) {
        // Check char limit
        if self.char_limit > 0 && self.char_count() >= self.char_limit {
            return;
        }

        // In single-line mode, don't allow newlines
        if self.mode == InputMode::SingleLine && c == '\n' {
            return;
        }

        self.text.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    /// Insert a newline at the cursor position (multi-line only)
    pub fn insert_newline(&mut self) {
        if self.mode == InputMode::MultiLine {
            self.insert_char('\n');
        }
    }

    /// Delete the character before the cursor (backspace)
    pub fn delete_char_before(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }

        // Find the previous character boundary
        let prev_boundary = self.prev_char_boundary(self.cursor_pos);
        self.text.drain(prev_boundary..self.cursor_pos);
        self.cursor_pos = prev_boundary;
    }

    /// Delete the character after the cursor (delete key)
    pub fn delete_char_after(&mut self) {
        if self.cursor_pos >= self.text.len() {
            return;
        }

        let next_boundary = self.next_char_boundary(self.cursor_pos);
        self.text.drain(self.cursor_pos..next_boundary);
    }

    /// Move cursor one character to the left
    pub fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.prev_char_boundary(self.cursor_pos);
        }
    }

    /// Move cursor one character to the right
    pub fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.text.len() {
            self.cursor_pos = self.next_char_boundary(self.cursor_pos);
        }
    }

    /// Move cursor to the beginning of the line
    pub fn move_cursor_home(&mut self) {
        if self.mode == InputMode::MultiLine {
            // Move to start of current line
            let line_start = self.text[..self.cursor_pos]
                .rfind('\n')
                .map(|pos| pos + 1)
                .unwrap_or(0);
            self.cursor_pos = line_start;
        } else {
            self.cursor_pos = 0;
        }
    }

    /// Move cursor to the end of the line
    pub fn move_cursor_end(&mut self) {
        if self.mode == InputMode::MultiLine {
            // Move to end of current line
            let line_end = self.text[self.cursor_pos..]
                .find('\n')
                .map(|pos| self.cursor_pos + pos)
                .unwrap_or(self.text.len());
            self.cursor_pos = line_end;
        } else {
            self.cursor_pos = self.text.len();
        }
    }

    /// Move cursor up one line (multi-line only)
    pub fn move_cursor_up(&mut self) {
        if self.mode != InputMode::MultiLine {
            return;
        }

        let (row, _) = self.cursor_row_col();
        if row == 0 {
            return;
        }

        let lines: Vec<&str> = self.text.split('\n').collect();
        let target_col = byte_offset_for_display_col(lines[row - 1], self.cursor_display_col());

        // Calculate byte offset for the target position
        let mut offset = 0;
        for line in &lines[..row - 1] {
            offset += line.len() + 1; // +1 for newline
        }
        offset += target_col;
        self.cursor_pos = offset;
    }

    /// Move cursor down one line (multi-line only)
    pub fn move_cursor_down(&mut self) {
        if self.mode != InputMode::MultiLine {
            return;
        }

        let (row, _) = self.cursor_row_col();
        let lines: Vec<&str> = self.text.split('\n').collect();

        if row >= lines.len() - 1 {
            return;
        }

        let target_col = byte_offset_for_display_col(lines[row + 1], self.cursor_display_col());

        // Calculate byte offset for the target position
        let mut offset = 0;
        for line in &lines[..row + 1] {
            offset += line.len() + 1; // +1 for newline
        }
        offset += target_col;
        self.cursor_pos = offset;
    }

    /// Returns the current cursor row (0-indexed)
    pub fn cursor_row(&self) -> usize {
        self.text[..self.cursor_pos].matches('\n').count()
    }

    /// Returns the current cursor column (0-indexed, byte offset from line start)
    pub fn cursor_col(&self) -> usize {
        let line_start = self.text[..self.cursor_pos]
            .rfind('\n')
            .map(|pos| pos + 1)
            .unwrap_or(0);
        self.cursor_pos - line_start
    }

    /// Returns (row, col) tuple for the cursor position
    fn cursor_row_col(&self) -> (usize, usize) {
        (self.cursor_row(), self.cursor_col())
    }

    /// Returns the current cursor column as terminal display width.
    fn cursor_display_col(&self) -> usize {
        let line_start = self.text[..self.cursor_pos]
            .rfind('\n')
            .map(|pos| pos + 1)
            .unwrap_or(0);
        self.text[line_start..self.cursor_pos].width()
    }

    /// Returns the lines of text (for multi-line rendering)
    pub fn lines(&self) -> Vec<&str> {
        self.text.split('\n').collect()
    }

    /// Returns the number of lines
    pub fn line_count(&self) -> usize {
        self.text.split('\n').count()
    }

    /// Delete from cursor to end of line
    pub fn delete_to_end(&mut self) {
        let end = if self.mode == InputMode::MultiLine {
            self.text[self.cursor_pos..]
                .find('\n')
                .map(|pos| self.cursor_pos + pos)
                .unwrap_or(self.text.len())
        } else {
            self.text.len()
        };
        self.text.drain(self.cursor_pos..end);
    }

    /// Delete from cursor to start of line
    pub fn delete_to_start(&mut self) {
        let start = if self.mode == InputMode::MultiLine {
            self.text[..self.cursor_pos]
                .rfind('\n')
                .map(|pos| pos + 1)
                .unwrap_or(0)
        } else {
            0
        };
        self.text.drain(start..self.cursor_pos);
        self.cursor_pos = start;
    }

    /// Find the previous character boundary before the given position
    fn prev_char_boundary(&self, pos: usize) -> usize {
        let mut new_pos = pos.saturating_sub(1);
        while new_pos > 0 && !self.text.is_char_boundary(new_pos) {
            new_pos -= 1;
        }
        new_pos
    }

    /// Find the next character boundary after the given position
    fn next_char_boundary(&self, pos: usize) -> usize {
        let mut new_pos = pos + 1;
        while new_pos < self.text.len() && !self.text.is_char_boundary(new_pos) {
            new_pos += 1;
        }
        new_pos.min(self.text.len())
    }
}

fn byte_offset_for_display_col(line: &str, display_col: usize) -> usize {
    let mut width = 0;
    for (idx, ch) in line.char_indices() {
        let ch_width = ch.width().unwrap_or(0);
        if width + ch_width > display_col {
            return idx;
        }
        width += ch_width;
    }
    line.len()
}

impl Default for TextInputState {
    fn default() -> Self {
        Self::new(InputMode::SingleLine)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_input_is_empty() {
        let state = TextInputState::new(InputMode::SingleLine);
        assert!(state.is_empty());
        assert_eq!(state.cursor_pos(), 0);
        assert_eq!(state.text(), "");
        assert_eq!(state.mode(), InputMode::SingleLine);
    }

    #[test]
    fn insert_char_at_beginning() {
        let mut state = TextInputState::new(InputMode::SingleLine);
        state.insert_char('a');
        assert_eq!(state.text(), "a");
        assert_eq!(state.cursor_pos(), 1);
    }

    #[test]
    fn insert_multiple_chars() {
        let mut state = TextInputState::new(InputMode::SingleLine);
        state.insert_char('h');
        state.insert_char('e');
        state.insert_char('l');
        state.insert_char('l');
        state.insert_char('o');
        assert_eq!(state.text(), "hello");
        assert_eq!(state.cursor_pos(), 5);
    }

    #[test]
    fn insert_char_in_middle() {
        let mut state = TextInputState::new(InputMode::SingleLine).with_text("hllo");
        state.cursor_pos = 1;
        state.insert_char('e');
        assert_eq!(state.text(), "hello");
        assert_eq!(state.cursor_pos(), 2);
    }

    #[test]
    fn delete_char_before() {
        let mut state = TextInputState::new(InputMode::SingleLine).with_text("hello");
        state.delete_char_before();
        assert_eq!(state.text(), "hell");
        assert_eq!(state.cursor_pos(), 4);
    }

    #[test]
    fn delete_char_before_at_beginning() {
        let mut state = TextInputState::new(InputMode::SingleLine).with_text("hello");
        state.cursor_pos = 0;
        state.delete_char_before();
        assert_eq!(state.text(), "hello"); // No change
        assert_eq!(state.cursor_pos(), 0);
    }

    #[test]
    fn delete_char_after() {
        let mut state = TextInputState::new(InputMode::SingleLine).with_text("hello");
        state.cursor_pos = 0;
        state.delete_char_after();
        assert_eq!(state.text(), "ello");
        assert_eq!(state.cursor_pos(), 0);
    }

    #[test]
    fn delete_char_after_at_end() {
        let mut state = TextInputState::new(InputMode::SingleLine).with_text("hello");
        state.delete_char_after();
        assert_eq!(state.text(), "hello"); // No change
    }

    #[test]
    fn move_cursor_left_right() {
        let mut state = TextInputState::new(InputMode::SingleLine).with_text("hello");
        assert_eq!(state.cursor_pos(), 5);

        state.move_cursor_left();
        assert_eq!(state.cursor_pos(), 4);

        state.move_cursor_left();
        assert_eq!(state.cursor_pos(), 3);

        state.move_cursor_right();
        assert_eq!(state.cursor_pos(), 4);
    }

    #[test]
    fn move_cursor_left_at_beginning() {
        let mut state = TextInputState::new(InputMode::SingleLine).with_text("hi");
        state.cursor_pos = 0;
        state.move_cursor_left();
        assert_eq!(state.cursor_pos(), 0);
    }

    #[test]
    fn move_cursor_right_at_end() {
        let mut state = TextInputState::new(InputMode::SingleLine).with_text("hi");
        state.move_cursor_right();
        assert_eq!(state.cursor_pos(), 2);
    }

    #[test]
    fn move_cursor_home_end() {
        let mut state = TextInputState::new(InputMode::SingleLine).with_text("hello");
        state.cursor_pos = 3;

        state.move_cursor_home();
        assert_eq!(state.cursor_pos(), 0);

        state.move_cursor_end();
        assert_eq!(state.cursor_pos(), 5);
    }

    #[test]
    fn char_limit() {
        let mut state = TextInputState::new(InputMode::SingleLine).with_char_limit(5);
        for c in "hello world".chars() {
            state.insert_char(c);
        }
        assert_eq!(state.text(), "hello");
        assert_eq!(state.char_count(), 5);
    }

    #[test]
    fn single_line_no_newline() {
        let mut state = TextInputState::new(InputMode::SingleLine);
        state.insert_char('a');
        state.insert_char('\n');
        state.insert_char('b');
        assert_eq!(state.text(), "ab");
    }

    #[test]
    fn multi_line_newline() {
        let mut state = TextInputState::new(InputMode::MultiLine);
        state.insert_char('a');
        state.insert_newline();
        state.insert_char('b');
        assert_eq!(state.text(), "a\nb");
        assert_eq!(state.line_count(), 2);
    }

    #[test]
    fn multi_line_cursor_row_col() {
        let mut state = TextInputState::new(InputMode::MultiLine).with_text("line1\nline2\nline3");
        // Cursor at end: row 2, col 5
        assert_eq!(state.cursor_row(), 2);
        assert_eq!(state.cursor_col(), 5);

        // Move to start of line 2
        state.cursor_pos = 6; // "line1\n" = 6 bytes
        assert_eq!(state.cursor_row(), 1);
        assert_eq!(state.cursor_col(), 0);
    }

    #[test]
    fn multi_line_cursor_up_down() {
        let mut state = TextInputState::new(InputMode::MultiLine).with_text("abc\ndef\nghi");
        // Cursor at end of "ghi" (row 2, col 3)
        assert_eq!(state.cursor_row(), 2);

        state.move_cursor_up();
        assert_eq!(state.cursor_row(), 1);
        assert_eq!(state.cursor_col(), 3);

        state.move_cursor_up();
        assert_eq!(state.cursor_row(), 0);
        assert_eq!(state.cursor_col(), 3);

        // Can't go up further
        state.move_cursor_up();
        assert_eq!(state.cursor_row(), 0);

        state.move_cursor_down();
        assert_eq!(state.cursor_row(), 1);

        state.move_cursor_down();
        assert_eq!(state.cursor_row(), 2);

        // Can't go down further
        state.move_cursor_down();
        assert_eq!(state.cursor_row(), 2);
    }

    #[test]
    fn multi_line_cursor_up_clamps_col() {
        let mut state =
            TextInputState::new(InputMode::MultiLine).with_text("long line here\nhi\nmedium");
        // At end: row 2, col 6
        state.move_cursor_up();
        // Row 1 "hi" only has 2 chars, so col should clamp to 2
        assert_eq!(state.cursor_row(), 1);
        assert_eq!(state.cursor_col(), 2);
    }

    #[test]
    fn multi_line_cursor_up_preserves_unicode_display_column() {
        let mut state = TextInputState::new(InputMode::MultiLine).with_text("検索a\néabc");
        state.cursor_pos = "検索a\néa".len();

        state.move_cursor_up();

        assert_eq!(state.cursor_row(), 0);
        assert_eq!(state.cursor_pos(), "検".len());
        assert!(state.text().is_char_boundary(state.cursor_pos()));
    }

    #[test]
    fn multi_line_cursor_down_preserves_unicode_display_column() {
        let mut state = TextInputState::new(InputMode::MultiLine).with_text("éa\n検索abc");
        state.cursor_pos = "éa".len();

        state.move_cursor_down();

        assert_eq!(state.cursor_row(), 1);
        assert_eq!(state.cursor_pos(), "éa\n検".len());
        assert!(state.text().is_char_boundary(state.cursor_pos()));
    }

    #[test]
    fn multi_line_home_end() {
        let mut state = TextInputState::new(InputMode::MultiLine).with_text("abc\ndef\nghi");
        state.cursor_pos = 7; // "abc\ndef" = 7, cursor at start of second 'e'

        state.move_cursor_home();
        assert_eq!(state.cursor_pos(), 4); // Start of "def"

        state.move_cursor_end();
        assert_eq!(state.cursor_pos(), 7); // End of "def"
    }

    #[test]
    fn clear_input() {
        let mut state = TextInputState::new(InputMode::SingleLine).with_text("hello");
        state.set_error(Some("some error"));
        state.clear();
        assert!(state.is_empty());
        assert_eq!(state.cursor_pos(), 0);
        assert!(state.error().is_none());
    }

    #[test]
    fn placeholder() {
        let state = TextInputState::new(InputMode::SingleLine).with_placeholder("Type here...");
        assert_eq!(state.placeholder(), "Type here...");
    }

    #[test]
    fn with_text_sets_cursor_to_end() {
        let state = TextInputState::new(InputMode::SingleLine).with_text("hello");
        assert_eq!(state.cursor_pos(), 5);
    }

    #[test]
    fn set_text() {
        let mut state = TextInputState::new(InputMode::SingleLine);
        state.insert_char('a');
        state.set_text("new text");
        assert_eq!(state.text(), "new text");
        assert_eq!(state.cursor_pos(), 8);
    }

    #[test]
    fn lines() {
        let state = TextInputState::new(InputMode::MultiLine).with_text("a\nb\nc");
        assert_eq!(state.lines(), vec!["a", "b", "c"]);
    }

    #[test]
    fn delete_to_end() {
        let mut state = TextInputState::new(InputMode::SingleLine).with_text("hello world");
        state.cursor_pos = 5;
        state.delete_to_end();
        assert_eq!(state.text(), "hello");
    }

    #[test]
    fn delete_to_start() {
        let mut state = TextInputState::new(InputMode::SingleLine).with_text("hello world");
        state.cursor_pos = 5;
        state.delete_to_start();
        assert_eq!(state.text(), " world");
        assert_eq!(state.cursor_pos(), 0);
    }

    #[test]
    fn delete_to_end_multiline() {
        let mut state = TextInputState::new(InputMode::MultiLine).with_text("abc\ndef\nghi");
        state.cursor_pos = 5; // In "def", after 'd'
        state.delete_to_end();
        assert_eq!(state.text(), "abc\nd\nghi");
    }

    #[test]
    fn delete_to_start_multiline() {
        let mut state = TextInputState::new(InputMode::MultiLine).with_text("abc\ndef\nghi");
        state.cursor_pos = 6; // In "def", after "de"
        state.delete_to_start();
        assert_eq!(state.text(), "abc\nf\nghi");
        assert_eq!(state.cursor_pos(), 4); // Start of that line
    }

    #[test]
    fn unicode_handling() {
        let mut state = TextInputState::new(InputMode::SingleLine);
        state.insert_char('é');
        state.insert_char('à');
        assert_eq!(state.text(), "éà");
        assert_eq!(state.char_count(), 2);

        state.move_cursor_left();
        state.delete_char_before();
        assert_eq!(state.text(), "à");
    }

    #[test]
    fn error_state() {
        let mut state = TextInputState::new(InputMode::SingleLine);
        assert!(state.error().is_none());

        state.set_error(Some("Required field"));
        assert_eq!(state.error(), Some("Required field"));

        state.set_error(None::<String>);
        assert!(state.error().is_none());
    }

    #[test]
    fn active_state() {
        let mut state = TextInputState::new(InputMode::SingleLine);
        assert!(!state.is_active());

        state.set_active(true);
        assert!(state.is_active());

        state.set_active(false);
        assert!(!state.is_active());
    }

    #[test]
    fn default_is_single_line() {
        let state = TextInputState::default();
        assert_eq!(state.mode(), InputMode::SingleLine);
        assert!(state.is_empty());
    }

    #[test]
    fn single_line_up_down_noop() {
        let mut state = TextInputState::new(InputMode::SingleLine).with_text("hello");
        state.cursor_pos = 3;
        state.move_cursor_up();
        assert_eq!(state.cursor_pos(), 3); // No change
        state.move_cursor_down();
        assert_eq!(state.cursor_pos(), 3); // No change
    }

    #[test]
    fn insert_newline_single_line_noop() {
        let mut state = TextInputState::new(InputMode::SingleLine);
        state.insert_char('a');
        state.insert_newline();
        assert_eq!(state.text(), "a");
    }
}
