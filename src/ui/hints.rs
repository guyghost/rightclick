//! Shared hint formatting helpers.

use unicode_width::UnicodeWidthStr;

/// Full help hint label used when enough horizontal space is available.
pub const HELP_HINT: &str = "?: Toggle help";

/// Return the most descriptive help hint that fits within `width`.
pub fn compact_help_hint(width: u16) -> Option<&'static str> {
    let width = width as usize;
    [HELP_HINT, "?: Help", "?"]
        .into_iter()
        .find(|hint| hint.width() <= width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_help_hint_fits_narrow_widths() {
        assert_eq!(compact_help_hint(0), None);
        assert_eq!(compact_help_hint(1), Some("?"));
        assert_eq!(compact_help_hint(7), Some("?: Help"));
        assert_eq!(compact_help_hint(14), Some(HELP_HINT));

        for width in 0..=30 {
            if let Some(hint) = compact_help_hint(width) {
                assert!(
                    hint.width() <= width as usize,
                    "hint {hint:?} overflowed width {width}"
                );
            }
        }
    }
}
