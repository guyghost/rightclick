//! Search types and data structures

/// Scope of the search
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchScope {
    /// Search everything
    #[default]
    All,
    /// Search file contents only
    Files,
    /// Search sessions, worktrees, and intents exposed by plugins.
    Items,
    /// Search commands only
    Commands,
}

impl SearchScope {
    /// Get display label
    pub fn label(&self) -> &str {
        match self {
            SearchScope::All => "All",
            SearchScope::Files => "Files",
            SearchScope::Items => "Project",
            SearchScope::Commands => "Commands",
        }
    }

    /// Cycle to next scope
    pub fn next(&self) -> Self {
        match self {
            SearchScope::All => SearchScope::Files,
            SearchScope::Files => SearchScope::Items,
            SearchScope::Items => SearchScope::Commands,
            SearchScope::Commands => SearchScope::All,
        }
    }
}

/// A search query with scope and text
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// The search text
    pub text: String,
    /// The search scope
    pub scope: SearchScope,
}

impl SearchQuery {
    pub fn new(text: impl Into<String>, scope: SearchScope) -> Self {
        Self {
            text: text.into(),
            scope,
        }
    }
}

/// A single search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// What kind of result this is
    pub kind: SearchResultKind,
    /// Display title
    pub title: String,
    /// Preview text
    pub preview: String,
    /// Match score for sorting
    pub score: u32,
}

/// Kind of search result
#[derive(Debug, Clone)]
pub enum SearchResultKind {
    /// File content match
    FileContent {
        path: String,
        line: usize,
        column: usize,
    },
    /// Conversation match
    Conversation { id: String },
    /// Plugin-provided search entry
    PluginEntry {
        /// Plugin identifier that owns the entry.
        plugin_id: String,
        /// Entry identifier within the plugin.
        entry_id: String,
    },
    /// Command match
    Command { id: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_scope_cycle() {
        assert_eq!(SearchScope::All.next(), SearchScope::Files);
        assert_eq!(SearchScope::Files.next(), SearchScope::Items);
        assert_eq!(SearchScope::Items.next(), SearchScope::Commands);
        assert_eq!(SearchScope::Commands.next(), SearchScope::All);
    }

    #[test]
    fn test_search_scope_label() {
        assert_eq!(SearchScope::All.label(), "All");
        assert_eq!(SearchScope::Files.label(), "Files");
        assert_eq!(SearchScope::Items.label(), "Project");
        assert_eq!(SearchScope::Commands.label(), "Commands");
    }

    #[test]
    fn test_search_scope_default() {
        assert_eq!(SearchScope::default(), SearchScope::All);
    }

    #[test]
    fn test_search_query_new() {
        let query = SearchQuery::new("hello", SearchScope::Files);
        assert_eq!(query.text, "hello");
        assert_eq!(query.scope, SearchScope::Files);
    }

    #[test]
    fn test_search_result_kinds() {
        let file_result = SearchResult {
            kind: SearchResultKind::FileContent {
                path: "test.rs".to_string(),
                line: 10,
                column: 5,
            },
            title: "test.rs".to_string(),
            preview: "fn main()".to_string(),
            score: 100,
        };
        assert!(matches!(
            file_result.kind,
            SearchResultKind::FileContent { .. }
        ));

        let cmd_result = SearchResult {
            kind: SearchResultKind::Command {
                id: "git.commit".to_string(),
            },
            title: "Git Commit".to_string(),
            preview: "Create a commit".to_string(),
            score: 50,
        };
        assert!(matches!(cmd_result.kind, SearchResultKind::Command { .. }));

        let plugin_result = SearchResult {
            kind: SearchResultKind::PluginEntry {
                plugin_id: "workspace".to_string(),
                entry_id: "main".to_string(),
            },
            title: "main".to_string(),
            preview: "main branch".to_string(),
            score: 75,
        };
        assert!(matches!(
            plugin_result.kind,
            SearchResultKind::PluginEntry { .. }
        ));
    }
}
