//! Fuzzy matching for command search.
//!
//! Uses nucleo for fast, high-quality fuzzy matching with scoring
//! and match range extraction.

use super::PaletteEntry;
use nucleo::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo::{Matcher, Utf32Str};

/// Fuzzy matcher for palette entries.
#[derive(Debug, Clone)]
pub struct FuzzyMatcher;

impl FuzzyMatcher {
    /// Creates a new fuzzy matcher with default configuration.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new fuzzy matcher with custom configuration.
    pub fn with_config(_config: ()) -> Self {
        Self::new()
    }

    /// Match entries against a query with context filtering.
    pub fn match_entries_with_context(
        &self,
        entries: &[PaletteEntry],
        query: &str,
        context: crate::keymap::FocusContext,
        show_all: bool,
    ) -> Vec<MatchResult> {
        let visible_entries: Vec<PaletteEntry> = entries
            .iter()
            .filter(|entry| entry.is_available_in(context, show_all))
            .cloned()
            .collect();

        self.match_entries(&visible_entries, query)
    }

    /// Match entries against a query using nucleo fuzzy matching.
    pub fn match_entries(&self, entries: &[PaletteEntry], query: &str) -> Vec<MatchResult> {
        if query.is_empty() {
            return entries
                .iter()
                .enumerate()
                .map(|(i, e)| MatchResult {
                    entry: e.clone(),
                    score: 0,
                    match_ranges: vec![],
                    index: i,
                })
                .collect();
        }

        let pattern = Pattern::new(
            query,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let mut matcher = Matcher::default();

        let mut results: Vec<MatchResult> = entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let text = e.search_text();
                let mut buf = Vec::new();
                let haystack = Utf32Str::new(&text, &mut buf);

                let score = pattern.score(haystack, &mut matcher)?;

                // Get match indices
                let mut indices = Vec::new();
                pattern.indices(haystack, &mut matcher, &mut indices);
                indices.sort_unstable();
                let match_ranges = indices_to_ranges(&indices);

                Some(MatchResult {
                    entry: e.clone(),
                    score,
                    match_ranges,
                    index: i,
                })
            })
            .collect();

        // Sort by score (highest first)
        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }
}

impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a fuzzy match.
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// The matched entry
    pub entry: PaletteEntry,
    /// Match score (higher is better)
    pub score: u32,
    /// Ranges of matching characters (start, end) pairs
    pub match_ranges: Vec<(usize, usize)>,
    /// Original index in the entries list
    pub index: usize,
}

/// Simple fuzzy match function using nucleo.
pub fn fuzzy_match_simple(text: &str, query: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }

    let pattern = Pattern::new(
        query,
        CaseMatching::Smart,
        Normalization::Smart,
        AtomKind::Fuzzy,
    );

    let mut matcher = Matcher::default();
    let mut buf = Vec::new();
    let haystack = Utf32Str::new(text, &mut buf);
    pattern.score(haystack, &mut matcher)
}

/// Simple scoring function using nucleo.
pub fn simple_score(text: &str, query: &str) -> u32 {
    fuzzy_match_simple(text, query).unwrap_or(0)
}

/// Convert sorted character indices into contiguous ranges
fn indices_to_ranges(indices: &[u32]) -> Vec<(usize, usize)> {
    if indices.is_empty() {
        return vec![];
    }

    let mut ranges = Vec::new();
    let mut start = indices[0] as usize;
    let mut end = start + 1;

    for &idx in &indices[1..] {
        let idx = idx as usize;
        if idx == end {
            end += 1;
        } else {
            ranges.push((start, end));
            start = idx;
            end = idx + 1;
        }
    }
    ranges.push((start, end));
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::FocusContext;
    use crate::palette::{Category, PaletteEntry};

    fn create_test_entry(name: &str) -> PaletteEntry {
        PaletteEntry {
            key: String::new(),
            command_id: name.to_string(),
            name: name.to_string(),
            description: String::new(),
            category: Category::Navigation,
            context: FocusContext::Global,
        }
    }

    #[test]
    fn test_fuzzy_matcher_new() {
        let matcher = FuzzyMatcher::new();
        let entries = vec![create_test_entry("quit"), create_test_entry("refresh")];

        let results = matcher.match_entries(&entries, "quit");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_fuzzy_match_simple() {
        assert!(fuzzy_match_simple("hello world", "hello").is_some());
        assert!(fuzzy_match_simple("hello world", "xyz").is_none());
    }

    #[test]
    fn test_fuzzy_match_scoring() {
        // Exact match should score higher than fuzzy match
        let exact = fuzzy_match_simple("quit", "quit").unwrap();
        let fuzzy = fuzzy_match_simple("quick test", "quit").unwrap_or(0);
        assert!(exact > fuzzy || fuzzy == 0);
    }

    #[test]
    fn test_fuzzy_match_case_smart() {
        // Smart case: lowercase query matches uppercase
        assert!(fuzzy_match_simple("Hello World", "hello").is_some());
        // Smart case: uppercase query is case-sensitive (nucleo behavior)
        // "Hello" won't match "hello world" because 'H' requires uppercase
        assert!(fuzzy_match_simple("hello world", "Hello").is_none());
        // But uppercase query matches when case matches
        assert!(fuzzy_match_simple("Hello World", "Hello").is_some());
    }

    #[test]
    fn test_fuzzy_match_empty_query() {
        assert!(fuzzy_match_simple("anything", "").is_some());
        assert_eq!(fuzzy_match_simple("anything", "").unwrap(), 0);
    }

    #[test]
    fn test_match_entries_empty_query_returns_all() {
        let matcher = FuzzyMatcher::new();
        let entries = vec![
            create_test_entry("one"),
            create_test_entry("two"),
            create_test_entry("three"),
        ];

        let results = matcher.match_entries(&entries, "");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_match_entries_filters_non_matching() {
        let matcher = FuzzyMatcher::new();
        let entries = vec![
            create_test_entry("open file"),
            create_test_entry("close tab"),
            create_test_entry("save file"),
        ];

        let results = matcher.match_entries(&entries, "file");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_match_entries_with_context_hides_unavailable_commands() {
        let matcher = FuzzyMatcher::new();
        let entries = vec![
            PaletteEntry::minimal("app.refresh", "Refresh", Category::Actions)
                .with_context(FocusContext::Global),
            PaletteEntry::minimal("git.commit", "Commit", Category::Git)
                .with_context(FocusContext::GitStatus),
        ];

        let scoped =
            matcher.match_entries_with_context(&entries, "", FocusContext::Workspace, false);
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].entry.name, "Refresh");

        let all_contexts =
            matcher.match_entries_with_context(&entries, "", FocusContext::Workspace, true);
        assert_eq!(all_contexts.len(), 2);
    }

    #[test]
    fn test_match_entries_sorted_by_score() {
        let matcher = FuzzyMatcher::new();
        let entries = vec![
            create_test_entry("my file browser"),
            create_test_entry("file"),
            create_test_entry("profile settings"),
        ];

        let results = matcher.match_entries(&entries, "file");
        assert!(!results.is_empty());
        // Results should be sorted by score descending
        for i in 1..results.len() {
            assert!(results[i - 1].score >= results[i].score);
        }
    }

    #[test]
    fn test_indices_to_ranges() {
        assert_eq!(indices_to_ranges(&[0, 1, 2]), vec![(0, 3)]);
        assert_eq!(indices_to_ranges(&[0, 1, 4, 5]), vec![(0, 2), (4, 6)]);
        assert_eq!(indices_to_ranges(&[3]), vec![(3, 4)]);
        assert_eq!(indices_to_ranges(&[]), Vec::<(usize, usize)>::new());
    }

    #[test]
    fn test_simple_score() {
        assert!(simple_score("hello", "hel") > 0);
        assert_eq!(simple_score("hello", "xyz"), 0);
    }
}
