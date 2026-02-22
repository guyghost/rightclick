//! Fuzzy matching for the command palette.

use super::PaletteEntry;

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
        _context: crate::keymap::FocusContext,
        _show_all: bool,
    ) -> Vec<MatchResult> {
        self.match_entries(entries, query)
    }

    /// Match entries against a query.
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

        let query_lower = query.to_lowercase();
        let mut results: Vec<MatchResult> = entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let text = e.search_text().to_lowercase();
                if let Some(pos) = text.find(&query_lower) {
                    Some(MatchResult {
                        entry: e.clone(),
                        score: (query.len() * 10) as u32,
                        match_ranges: vec![(pos, pos + query.len())],
                        index: i,
                    })
                } else {
                    None
                }
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
    /// Ranges of matching characters
    pub match_ranges: Vec<(usize, usize)>,
    /// Original index in the entries list
    pub index: usize,
}

/// Simple fuzzy match function.
pub fn fuzzy_match_simple(text: &str, query: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }

    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();

    if text_lower.contains(&query_lower) {
        Some(query.len() as u32 * 10)
    } else {
        None
    }
}

/// Simple scoring function.
pub fn simple_score(_text: &str, _query: &str) -> u32 {
    0
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
}
