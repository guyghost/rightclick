//! Global search module for RightClick
//!
//! Provides file content search (using `rg` subprocess), conversation search,
//! and a unified search overlay UI.

pub mod overlay;
pub mod types;

pub use overlay::{SearchOverlayAction, SearchOverlayState, render_search_overlay};
pub use types::{SearchQuery, SearchResult, SearchResultKind, SearchScope};

use crate::palette::fuzzy::fuzzy_match_simple;
use std::path::Path;
use tokio::process::Command as TokioCommand;

/// Search for a query in files using ripgrep
///
/// Spawns `rg` as a subprocess and parses the results.
/// Returns an empty vec if `rg` is not available.
pub async fn search_files(query: &str, directory: &Path, max_results: usize) -> Vec<SearchResult> {
    if query.is_empty() {
        return vec![];
    }

    let output = TokioCommand::new("rg")
        .args([
            "--line-number",
            "--column",
            "--no-heading",
            "--max-count",
            "5", // max matches per file
            "--max-filesize",
            "1M",
            "--color",
            "never",
            query,
        ])
        .current_dir(directory)
        .output()
        .await;

    let output = match output {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_rg_output(&stdout, max_results)
}

/// Parse ripgrep output into SearchResults
fn parse_rg_output(output: &str, max_results: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();

    for line in output.lines() {
        if results.len() >= max_results {
            break;
        }

        // rg format: file:line:column:content
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() >= 4 {
            let file_path = parts[0].to_string();
            let line_number = parts[1].parse::<usize>().unwrap_or(0);
            let column = parts[2].parse::<usize>().unwrap_or(0);
            let content = parts[3].trim().to_string();

            results.push(SearchResult {
                kind: SearchResultKind::FileContent {
                    path: file_path.clone(),
                    line: line_number,
                    column,
                },
                title: file_path,
                preview: content,
                score: 100,
            });
        }
    }

    results
}

/// Search conversations for a query using fuzzy matching
pub fn search_conversations(
    query: &str,
    conversations: &[(String, String)], // (title, preview)
    max_results: usize,
) -> Vec<SearchResult> {
    if query.is_empty() {
        return vec![];
    }

    let mut results: Vec<SearchResult> = conversations
        .iter()
        .filter_map(|(title, preview)| {
            let title_score = fuzzy_match_simple(title, query).unwrap_or(0);
            let preview_score = fuzzy_match_simple(preview, query).unwrap_or(0);
            let score = title_score.max(preview_score);
            if score > 0 {
                Some(SearchResult {
                    kind: SearchResultKind::Conversation { id: title.clone() },
                    title: title.clone(),
                    preview: preview.clone(),
                    score,
                })
            } else {
                None
            }
        })
        .collect();

    results.sort_by(|a, b| b.score.cmp(&a.score));
    results.truncate(max_results);
    results
}

/// Search commands using fuzzy matching
pub fn search_commands(
    query: &str,
    commands: &[(String, String)], // (name, description)
    max_results: usize,
) -> Vec<SearchResult> {
    if query.is_empty() {
        return vec![];
    }

    let mut results: Vec<SearchResult> = commands
        .iter()
        .filter_map(|(name, desc)| {
            let name_score = fuzzy_match_simple(name, query).unwrap_or(0);
            let desc_score = fuzzy_match_simple(desc, query).unwrap_or(0);
            let score = name_score.max(desc_score);
            if score > 0 {
                Some(SearchResult {
                    kind: SearchResultKind::Command { id: name.clone() },
                    title: name.clone(),
                    preview: desc.clone(),
                    score,
                })
            } else {
                None
            }
        })
        .collect();

    results.sort_by(|a, b| b.score.cmp(&a.score));
    results.truncate(max_results);
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rg_output_basic() {
        let output = "src/main.rs:10:5:fn main() {\nsrc/lib.rs:20:1:pub mod core;\n";
        let results = parse_rg_output(output, 10);
        assert_eq!(results.len(), 2);

        assert_eq!(results[0].title, "src/main.rs");
        assert!(matches!(
            &results[0].kind,
            SearchResultKind::FileContent {
                path,
                line: 10,
                column: 5,
            } if path == "src/main.rs"
        ));
        assert_eq!(results[0].preview, "fn main() {");

        assert_eq!(results[1].title, "src/lib.rs");
    }

    #[test]
    fn test_parse_rg_output_max_results() {
        let output = "a.rs:1:1:line1\nb.rs:2:1:line2\nc.rs:3:1:line3\n";
        let results = parse_rg_output(output, 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_parse_rg_output_empty() {
        let results = parse_rg_output("", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_rg_output_malformed_lines() {
        let output = "not a valid line\nstill not valid\n";
        let results = parse_rg_output(output, 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_conversations() {
        let convos = vec![
            (
                "Chat about Rust".to_string(),
                "How do I use traits?".to_string(),
            ),
            (
                "Python debugging".to_string(),
                "Fix this error in my code".to_string(),
            ),
            (
                "Rust async patterns".to_string(),
                "Using tokio for async".to_string(),
            ),
        ];

        let results = search_conversations("rust", &convos, 10);
        assert_eq!(results.len(), 2);
        // Results should be sorted by score
        assert!(results[0].score >= results[1].score);
    }

    #[test]
    fn test_search_conversations_empty_query() {
        let convos = vec![("Test".to_string(), "Content".to_string())];
        let results = search_conversations("", &convos, 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_commands() {
        let commands = vec![
            ("git commit".to_string(), "Create a commit".to_string()),
            ("git push".to_string(), "Push to remote".to_string()),
            ("file open".to_string(), "Open a file".to_string()),
        ];

        let results = search_commands("git", &commands, 10);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_commands_max_results() {
        let commands = vec![
            ("a".to_string(), "abc".to_string()),
            ("b".to_string(), "abc".to_string()),
            ("c".to_string(), "abc".to_string()),
        ];

        let results = search_commands("a", &commands, 1);
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_search_files_empty_query() {
        let results = search_files("", Path::new("."), 10).await;
        assert!(results.is_empty());
    }
}
