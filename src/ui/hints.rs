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

/// Return global search and help hint lines that fit within `width`.
pub fn compact_global_hint_lines(width: u16) -> Vec<String> {
    compact_prefixed_global_hint_lines(width, "", false)
}

/// Append global search and help hint lines that fit within `width`.
pub fn append_global_hint_lines(lines: &mut Vec<String>, width: u16) {
    lines.extend(compact_global_hint_lines(width));
}

/// Build a newline-delimited message with global search and help hint lines.
pub fn global_hint_message(mut lines: Vec<String>, width: u16) -> String {
    append_global_hint_lines(&mut lines, width);
    lines.join("\n")
}

/// Return prefixed global search and help hint lines, allowing stacked search.
pub fn compact_prefixed_stacked_global_hint_lines(width: u16, prefix: &str) -> Vec<String> {
    compact_prefixed_global_hint_lines(width, prefix, true)
}

/// Build a newline-delimited message with prefixed stacked global hint lines.
pub fn prefixed_stacked_global_hint_message(
    mut lines: Vec<String>,
    width: u16,
    prefix: &str,
) -> String {
    lines.extend(compact_prefixed_stacked_global_hint_lines(width, prefix));
    lines.join("\n")
}

fn compact_prefixed_global_hint_lines(
    width: u16,
    prefix: &str,
    allow_stacked_search: bool,
) -> Vec<String> {
    let hint_width = width.saturating_sub(prefix.width() as u16);
    let search_hint = if allow_stacked_search {
        compact_global_search_hint_with_stacked(hint_width)
    } else {
        compact_global_search_hint(hint_width)
    };

    let mut lines = Vec::new();
    if let Some(hint) = search_hint {
        lines.extend(hint.lines().map(|line| format!("{prefix}{line}")));
    }
    if let Some(hint) = compact_help_hint(hint_width) {
        lines.push(format!("{prefix}{hint}"));
    }
    lines
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

    #[test]
    fn compact_global_hint_lines_keep_search_before_help() {
        assert_eq!(
            compact_global_hint_lines(80),
            vec![GLOBAL_SEARCH_HINT.to_string(), HELP_HINT.to_string()]
        );
        assert_eq!(
            compact_global_hint_lines(9),
            vec!["/: Search".to_string(), "?: Help".to_string()]
        );
        assert_eq!(compact_global_hint_lines(1), vec!["?".to_string()]);
    }

    #[test]
    fn global_hint_message_appends_search_and_help() {
        assert_eq!(
            global_hint_message(vec!["No items".to_string(), String::new()], 80),
            format!("No items\n\n{GLOBAL_SEARCH_HINT}\n{HELP_HINT}")
        );
    }

    #[test]
    fn compact_prefixed_stacked_global_hint_lines_preserve_prefix_and_width() {
        let lines = compact_prefixed_stacked_global_hint_lines(22, "  ");

        assert_eq!(
            lines,
            vec![
                "  /: Global search".to_string(),
                "  : Command search".to_string(),
                "  ?: Toggle help".to_string(),
            ]
        );
        assert!(
            lines
                .iter()
                .all(|line| line.width() <= 22 && line.starts_with("  "))
        );
    }

    #[test]
    fn prefixed_stacked_global_hint_message_appends_prefixed_hints() {
        assert_eq!(
            prefixed_stacked_global_hint_message(vec!["No items".to_string()], 22, "  "),
            "No items\n  /: Global search\n  : Command search\n  ?: Toggle help"
        );
    }
}
