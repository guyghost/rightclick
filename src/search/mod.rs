//! Global search module for RightClick
//!
//! Provides file content search using `rg`, shared search result types, and
//! the unified search overlay UI. Plugin-owned item search is composed by the
//! application from plugin metadata.

pub mod overlay;
pub mod types;

pub use overlay::{SearchOverlayAction, SearchOverlayState, render_search_overlay};
pub use types::{SearchQuery, SearchResult, SearchResultKind, SearchScope};

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

    #[tokio::test]
    async fn test_search_files_empty_query() {
        let results = search_files("", Path::new("."), 10).await;
        assert!(results.is_empty());
    }
}
