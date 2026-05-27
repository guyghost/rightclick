//! Shared text formatting helpers.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Truncate text to fit within `max_width` display columns.
pub fn truncate_display(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    if text.width() <= max_width {
        return text.to_string();
    }

    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let mut output = String::new();
    let mut width = 0;
    for ch in text.chars() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_display_handles_ascii_widths() {
        assert_eq!(truncate_display("abcdef", 0), "");
        assert_eq!(truncate_display("abcdef", 2), "..");
        assert_eq!(truncate_display("abcdef", 5), "ab...");
        assert_eq!(truncate_display("abc", 5), "abc");
    }

    #[test]
    fn truncate_display_handles_unicode_widths() {
        assert_eq!(truncate_display("éclair session", 5), "éc...");
        assert_eq!(truncate_display("éclair", 2), "..");
    }
}
