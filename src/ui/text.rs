//! Shared text formatting helpers.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Return the display width of text, saturated to `u16::MAX`.
pub fn display_width_u16(text: &str) -> u16 {
    text.width().min(u16::MAX as usize) as u16
}

/// Truncate text to fit within `max_width` display columns.
pub fn truncate_display(text: &str, max_width: usize) -> String {
    truncate_display_with_suffix(text, max_width, "...")
}

/// Truncate text to fit within `max_width` display columns using `suffix`.
pub fn truncate_display_with_suffix(text: &str, max_width: usize, suffix: &str) -> String {
    if max_width == 0 {
        return String::new();
    }

    if text.width() <= max_width {
        return text.to_string();
    }

    let suffix_width = suffix.width();
    if max_width <= suffix_width {
        return ".".repeat(max_width);
    }

    let mut output = String::new();
    let mut width = 0;
    for ch in text.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if width + ch_width + suffix_width > max_width {
            break;
        }
        output.push(ch);
        width += ch_width;
    }
    output.push_str(suffix);
    output
}

/// Clip text to fit within `max_width` display columns without adding a suffix.
pub fn clip_display(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    if text.width() <= max_width {
        return text.to_string();
    }

    let mut output = String::new();
    let mut width = 0;
    for ch in text.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if width + ch_width > max_width {
            break;
        }
        output.push(ch);
        width += ch_width;
    }
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

    #[test]
    fn truncate_display_with_suffix_handles_custom_suffixes() {
        assert_eq!(
            truncate_display_with_suffix("éclair-session", 8, ".."),
            "éclair.."
        );
        assert_eq!(truncate_display_with_suffix("abcdef", 2, ".."), "..");
        assert_eq!(truncate_display_with_suffix("abc", 0, ".."), "");
        assert_eq!(truncate_display_with_suffix("abc", 5, ".."), "abc");
    }

    #[test]
    fn clip_display_handles_unicode_boundaries() {
        assert_eq!(clip_display("éclair", 4), "écla");
        assert_eq!(clip_display("éclair", 0), "");
        assert_eq!(clip_display("abc", 5), "abc");
    }

    #[test]
    fn display_width_u16_handles_unicode_and_saturates() {
        assert_eq!(display_width_u16("abc"), 3);
        assert_eq!(display_width_u16("検索a"), 5);
        assert_eq!(
            display_width_u16(&"x".repeat(u16::MAX as usize + 1)),
            u16::MAX
        );
    }
}
