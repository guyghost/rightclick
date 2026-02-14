//! Text selection handling for the TUI interface
//!
//! This module provides utilities for managing text selections, including
//! cursor positioning, selection ranges, and coordinate conversions.

use ratatui::layout::Position;

/// Text selection state
///
/// Represents a selection in the text area with start and end positions.
/// The selection is inclusive of both start and end points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    /// Start position (line, column)
    pub start: (u16, u16),
    /// End position (line, column)
    pub end: (u16, u16),
}

impl Selection {
    /// Create a new selection at a specific position
    ///
    /// # Arguments
    ///
    /// * `line` - The line number (0-indexed)
    /// * `column` - The column number (0-indexed)
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Selection;
    ///
    /// let selection = Selection::new(0, 0);
    /// ```
    pub fn new(line: u16, column: u16) -> Self {
        Self {
            start: (line, column),
            end: (line, column),
        }
    }

    /// Create a selection from two positions
    ///
    /// The selection will be normalized so that start <= end.
    ///
    /// # Arguments
    ///
    /// * `start` - Start position (line, column)
    /// * `end` - End position (line, column)
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Selection;
    ///
    /// let selection = Selection::from_positions((1, 5), (3, 10));
    /// ```
    pub fn from_positions(start: (u16, u16), end: (u16, u16)) -> Self {
        let mut selection = Self { start, end };
        selection.normalize();
        selection
    }

    /// Normalize the selection so start <= end
    fn normalize(&mut self) {
        if self.start.0 > self.end.0 || (self.start.0 == self.end.0 && self.start.1 > self.end.1) {
            std::mem::swap(&mut self.start, &mut self.end);
        }
    }

    /// Check if the selection is empty (start == end)
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Selection;
    ///
    /// let empty = Selection::new(0, 0);
    /// assert!(empty.is_empty());
    ///
    /// let not_empty = Selection::from_positions((0, 0), (0, 5));
    /// assert!(!not_empty.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Extend the selection to a new position
    ///
    /// The start position remains fixed, and the end position is updated.
    ///
    /// # Arguments
    ///
    /// * `line` - The new line number
    /// * `column` - The new column number
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Selection;
    ///
    /// let mut selection = Selection::new(0, 0);
    /// selection.extend_to(2, 10);
    /// assert_eq!(selection.end, (2, 10));
    /// ```
    pub fn extend_to(&mut self, line: u16, column: u16) {
        self.end = (line, column);
    }

    /// Move the selection to a new position (collapse to single point)
    ///
    /// # Arguments
    ///
    /// * `line` - The line number
    /// * `column` - The column number
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Selection;
    ///
    /// let mut selection = Selection::from_positions((0, 0), (2, 10));
    /// selection.move_to(5, 5);
    /// assert!(selection.is_empty());
    /// assert_eq!(selection.start, (5, 5));
    /// ```
    pub fn move_to(&mut self, line: u16, column: u16) {
        self.start = (line, column);
        self.end = (line, column);
    }

    /// Check if a position is within the selection
    ///
    /// # Arguments
    ///
    /// * `line` - The line to check
    /// * `column` - The column to check
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Selection;
    ///
    /// let selection = Selection::from_positions((1, 5), (3, 10));
    /// assert!(selection.contains(2, 7));
    /// assert!(!selection.contains(0, 0));
    /// ```
    pub fn contains(&self, line: u16, column: u16) -> bool {
        let (start_line, start_col) = self.start;
        let (end_line, end_col) = self.end;

        if line < start_line || line > end_line {
            return false;
        }

        if line == start_line && column < start_col {
            return false;
        }

        if line == end_line && column > end_col {
            return false;
        }

        true
    }

    /// Check if a line is at least partially within the selection
    ///
    /// # Arguments
    ///
    /// * `line` - The line to check
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Selection;
    ///
    /// let selection = Selection::from_positions((1, 5), (3, 10));
    /// assert!(selection.intersects_line(1));
    /// assert!(selection.intersects_line(2));
    /// assert!(selection.intersects_line(3));
    /// assert!(!selection.intersects_line(0));
    /// assert!(!selection.intersects_line(4));
    /// ```
    pub fn intersects_line(&self, line: u16) -> bool {
        line >= self.start.0 && line <= self.end.0
    }

    /// Get the selected text from a vector of lines
    ///
    /// # Arguments
    ///
    /// * `lines` - The lines of text to extract from
    ///
    /// # Returns
    ///
    /// The selected text as a string
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Selection;
    ///
    /// let lines = vec![
    ///     "Hello, World!",
    ///     "How are you?",
    ///     "Goodbye!",
    /// ];
    /// let selection = Selection::from_positions((0, 7), (1, 3));
    /// let selected = selection.get_text(&lines);
    /// assert_eq!(selected, "World!\nHow ");
    /// ```
    pub fn get_text(&self, lines: &[impl AsRef<str>]) -> String {
        if self.is_empty() {
            return String::new();
        }

        let (start_line, start_col) = self.start;
        let (end_line, end_col) = self.end;

        let mut result = String::new();

        for line_idx in start_line..=end_line {
            if let Some(line) = lines.get(line_idx as usize) {
                let line_str = line.as_ref();

                if line_idx == start_line && line_idx == end_line {
                    // Single line selection
                    let start = (start_col as usize).min(line_str.len());
                    let end = (end_col as usize + 1).min(line_str.len());
                    result.push_str(&line_str[start..end]);
                } else if line_idx == start_line {
                    // First line of multi-line selection
                    let start = (start_col as usize).min(line_str.len());
                    result.push_str(&line_str[start..]);
                    result.push('\n');
                } else if line_idx == end_line {
                    // Last line of multi-line selection
                    let end = (end_col as usize + 1).min(line_str.len());
                    result.push_str(&line_str[..end]);
                } else {
                    // Middle line
                    result.push_str(line_str);
                    result.push('\n');
                }
            }
        }

        result
    }

    /// Get the start and end column for a specific line within the selection
    ///
    /// Returns `Some((start_col, end_col))` if the line intersects the selection,
    /// or `None` if it doesn't.
    ///
    /// # Arguments
    ///
    /// * `line` - The line number
    /// * `line_length` - The length of the line in characters
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Selection;
    ///
    /// let selection = Selection::from_positions((1, 5), (3, 10));
    /// assert_eq!(selection.line_range(1, 20), Some((5, 19)));
    /// assert_eq!(selection.line_range(2, 30), Some((0, 29)));
    /// assert_eq!(selection.line_range(3, 15), Some((0, 10)));
    /// assert_eq!(selection.line_range(0, 10), None);
    /// ```
    pub fn line_range(&self, line: u16, line_length: u16) -> Option<(u16, u16)> {
        if !self.intersects_line(line) {
            return None;
        }

        let (start_line, start_col) = self.start;
        let (end_line, end_col) = self.end;

        let range_start = if line == start_line { start_col } else { 0 };
        let range_end = if line == end_line {
            end_col.min(line_length.saturating_sub(1))
        } else {
            line_length.saturating_sub(1)
        };

        Some((range_start, range_end))
    }

    /// Convert to ratatui Position for the start
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Selection;
    /// use ratatui::layout::Position;
    ///
    /// let selection = Selection::new(5, 10);
    /// let pos = selection.start_position();
    /// assert_eq!(pos.x, 10);
    /// assert_eq!(pos.y, 5);
    /// ```
    pub fn start_position(&self) -> Position {
        Position {
            x: self.start.1,
            y: self.start.0,
        }
    }

    /// Convert to ratatui Position for the end
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Selection;
    /// use ratatui::layout::Position;
    ///
    /// let selection = Selection::new(5, 10);
    /// let pos = selection.end_position();
    /// assert_eq!(pos.x, 10);
    /// assert_eq!(pos.y, 5);
    /// ```
    pub fn end_position(&self) -> Position {
        Position {
            x: self.end.1,
            y: self.end.0,
        }
    }

    /// Clear the selection (set start and end to 0, 0)
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Selection;
    ///
    /// let mut selection = Selection::from_positions((1, 5), (3, 10));
    /// selection.clear();
    /// assert!(selection.is_empty());
    /// assert_eq!(selection.start, (0, 0));
    /// ```
    pub fn clear(&mut self) {
        self.start = (0, 0);
        self.end = (0, 0);
    }

    /// Select the entire line
    ///
    /// # Arguments
    ///
    /// * `line` - The line number
    /// * `line_length` - The length of the line
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Selection;
    ///
    /// let mut selection = Selection::new(0, 0);
    /// selection.select_line(5, 50);
    /// assert_eq!(selection.start, (5, 0));
    /// assert_eq!(selection.end, (5, 49));
    /// ```
    pub fn select_line(&mut self, line: u16, line_length: u16) {
        self.start = (line, 0);
        self.end = (line, line_length.saturating_sub(1));
    }

    /// Select all lines
    ///
    /// # Arguments
    ///
    /// * `total_lines` - The total number of lines
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::ui::Selection;
    ///
    /// let mut selection = Selection::new(0, 0);
    /// selection.select_all(100);
    /// assert_eq!(selection.start, (0, 0));
    /// assert_eq!(selection.end, (99, 0));
    /// ```
    pub fn select_all(&mut self, total_lines: u16) {
        self.start = (0, 0);
        self.end = (total_lines.saturating_sub(1), 0);
    }
}

impl Default for Selection {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_new() {
        let sel = Selection::new(5, 10);
        assert_eq!(sel.start, (5, 10));
        assert_eq!(sel.end, (5, 10));
    }

    #[test]
    fn test_selection_from_positions() {
        let sel = Selection::from_positions((3, 15), (1, 5));
        // Should be normalized
        assert_eq!(sel.start, (1, 5));
        assert_eq!(sel.end, (3, 15));
    }

    #[test]
    fn test_selection_is_empty() {
        let empty = Selection::new(0, 0);
        assert!(empty.is_empty());

        let not_empty = Selection::from_positions((0, 0), (0, 5));
        assert!(!not_empty.is_empty());
    }

    #[test]
    fn test_selection_extend_to() {
        let mut sel = Selection::new(0, 0);
        sel.extend_to(2, 10);
        assert_eq!(sel.start, (0, 0));
        assert_eq!(sel.end, (2, 10));
    }

    #[test]
    fn test_selection_move_to() {
        let mut sel = Selection::from_positions((0, 0), (2, 10));
        sel.move_to(5, 5);
        assert!(sel.is_empty());
        assert_eq!(sel.start, (5, 5));
    }

    #[test]
    fn test_selection_contains() {
        let sel = Selection::from_positions((1, 5), (3, 10));
        assert!(sel.contains(1, 5));
        assert!(sel.contains(2, 0));
        assert!(sel.contains(3, 10));
        assert!(!sel.contains(0, 0));
        assert!(!sel.contains(4, 0));
        assert!(!sel.contains(1, 4));
        assert!(!sel.contains(3, 11));
    }

    #[test]
    fn test_selection_intersects_line() {
        let sel = Selection::from_positions((1, 5), (3, 10));
        assert!(sel.intersects_line(1));
        assert!(sel.intersects_line(2));
        assert!(sel.intersects_line(3));
        assert!(!sel.intersects_line(0));
        assert!(!sel.intersects_line(4));
    }

    #[test]
    fn test_selection_get_text() {
        let lines = vec!["Hello, World!", "How are you?", "Goodbye!"];
        let sel = Selection::from_positions((0, 7), (1, 3));
        assert_eq!(sel.get_text(&lines), "World!\nHow ");
    }

    #[test]
    fn test_selection_get_text_single_line() {
        let lines = vec!["Hello, World!", "How are you?"];
        let sel = Selection::from_positions((0, 0), (0, 4));
        assert_eq!(sel.get_text(&lines), "Hello");
    }

    #[test]
    fn test_selection_get_text_empty() {
        let lines = vec!["Hello", "World"];
        let sel = Selection::new(0, 0);
        assert_eq!(sel.get_text(&lines), "");
    }

    #[test]
    fn test_selection_line_range() {
        let sel = Selection::from_positions((1, 5), (3, 10));
        assert_eq!(sel.line_range(1, 20), Some((5, 19)));
        assert_eq!(sel.line_range(2, 30), Some((0, 29)));
        assert_eq!(sel.line_range(3, 15), Some((0, 10)));
        assert_eq!(sel.line_range(0, 10), None);
    }

    #[test]
    fn test_selection_clear() {
        let mut sel = Selection::from_positions((1, 5), (3, 10));
        sel.clear();
        assert!(sel.is_empty());
        assert_eq!(sel.start, (0, 0));
    }

    #[test]
    fn test_selection_select_line() {
        let mut sel = Selection::new(0, 0);
        sel.select_line(5, 50);
        assert_eq!(sel.start, (5, 0));
        assert_eq!(sel.end, (5, 49));
    }

    #[test]
    fn test_selection_select_all() {
        let mut sel = Selection::new(0, 0);
        sel.select_all(100);
        assert_eq!(sel.start, (0, 0));
        assert_eq!(sel.end, (99, 0));
    }
}
