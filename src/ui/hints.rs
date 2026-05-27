//! Shared hint formatting helpers.

use unicode_width::UnicodeWidthStr;

/// Full help hint label used when enough horizontal space is available.
pub const HELP_HINT: &str = "?: Toggle help";

/// Full global search hint label used when enough horizontal space is available.
pub const GLOBAL_SEARCH_HINT: &str = "/: Global search  |  : Command search";

/// Stacked global search hint for narrow panes that can spare vertical space.
pub const STACKED_GLOBAL_SEARCH_HINT: &str = "/: Global search\n: Command search";

/// Return the most descriptive help hint that fits within `width`.
pub fn compact_help_hint(width: u16) -> Option<&'static str> {
    let width = width as usize;
    [HELP_HINT, "?: Help", "?"]
        .into_iter()
        .find(|hint| hint.width() <= width)
}

/// Return the most descriptive single-line global search hint that fits.
pub fn compact_global_search_hint(width: u16) -> Option<&'static str> {
    compact_hint(
        [
            GLOBAL_SEARCH_HINT,
            "/: Search  |  : Commands",
            "/: Search  |  : Cmds",
            "/: Search",
            "/:",
        ],
        width,
    )
}

/// Return the most descriptive global search hint, allowing a stacked variant.
pub fn compact_global_search_hint_with_stacked(width: u16) -> Option<&'static str> {
    compact_hint(
        [
            GLOBAL_SEARCH_HINT,
            STACKED_GLOBAL_SEARCH_HINT,
            "/: Search  |  : Commands",
            "/: Search  |  : Cmds",
            "/: Search",
            "/:",
        ],
        width,
    )
}

fn compact_hint(
    candidates: impl IntoIterator<Item = &'static str>,
    width: u16,
) -> Option<&'static str> {
    let width = width as usize;
    candidates
        .into_iter()
        .find(|hint| hint.lines().all(|line| line.width() <= width))
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

    #[test]
    fn compact_global_search_hint_fits_narrow_widths() {
        assert_eq!(compact_global_search_hint(1), None);
        assert_eq!(compact_global_search_hint(2), Some("/:"));
        assert_eq!(compact_global_search_hint(9), Some("/: Search"));
        assert_eq!(compact_global_search_hint(20), Some("/: Search  |  : Cmds"));
        assert_eq!(
            compact_global_search_hint(24),
            Some("/: Search  |  : Commands")
        );
        assert_eq!(compact_global_search_hint(80), Some(GLOBAL_SEARCH_HINT));
    }

    #[test]
    fn compact_global_search_hint_with_stacked_prefers_stacked_when_narrow() {
        assert_eq!(compact_global_search_hint_with_stacked(1), None);
        assert_eq!(compact_global_search_hint_with_stacked(2), Some("/:"));
        assert_eq!(
            compact_global_search_hint_with_stacked(20),
            Some(STACKED_GLOBAL_SEARCH_HINT)
        );
        assert_eq!(
            compact_global_search_hint_with_stacked(28),
            Some(STACKED_GLOBAL_SEARCH_HINT)
        );
        assert_eq!(
            compact_global_search_hint_with_stacked(80),
            Some(GLOBAL_SEARCH_HINT)
        );

        for width in 0..=80 {
            if let Some(hint) = compact_global_search_hint_with_stacked(width) {
                assert!(
                    hint.lines().all(|line| line.width() <= width as usize),
                    "hint {hint:?} overflowed width {width}"
                );
            }
        }
    }
}
