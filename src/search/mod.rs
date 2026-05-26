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
            "--field-match-separator",
            "\t",
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

        if let Some((file_path, line_number, column, content)) = parse_rg_line(line) {
            let file_path = file_path.to_string();

            results.push(SearchResult {
                kind: SearchResultKind::FileContent {
                    path: file_path.clone(),
                    line: line_number,
                    column,
                },
                title: format_file_result_title(&file_path, line_number),
                preview: content.to_string(),
                score: 100,
            });
        }
    }

    results
}

fn parse_rg_line(line: &str) -> Option<(&str, usize, usize, &str)> {
    parse_rg_line_with_separator(line, '\t').or_else(|| parse_rg_line_with_separator(line, ':'))
}

fn parse_rg_line_with_separator(line: &str, separator: char) -> Option<(&str, usize, usize, &str)> {
    let parts: Vec<&str> = line.splitn(4, separator).collect();
    if parts.len() < 4 {
        return None;
    }

    Some((
        parts[0],
        parts[1].parse::<usize>().unwrap_or(0),
        parts[2].parse::<usize>().unwrap_or(0),
        parts[3].trim(),
    ))
}

fn format_file_result_title(path: &str, line: usize) -> String {
    format!("{}:{}", path, line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rg_output_basic() {
        let output = "src/main.rs:10:5:fn main() {\nsrc/lib.rs:20:1:pub mod core;\n";
        let results = parse_rg_output(output, 10);
        assert_eq!(results.len(), 2);

        assert_eq!(results[0].title, "src/main.rs:10");
        assert!(matches!(
            &results[0].kind,
            SearchResultKind::FileContent {
                path,
                line: 10,
                column: 5,
            } if path == "src/main.rs"
        ));
        assert_eq!(results[0].preview, "fn main() {");

        assert_eq!(results[1].title, "src/lib.rs:20");
    }

    #[test]
    fn test_format_file_result_title_includes_line_number() {
        assert_eq!(
            format_file_result_title("src/main.rs", 42),
            "src/main.rs:42"
        );
    }

    #[test]
    fn test_parse_rg_output_preserves_colon_in_path_with_tab_separator() {
        let output = "src/path:with-colon.rs\t10\t5\tfn main() {}\n";
        let results = parse_rg_output(output, 10);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "src/path:with-colon.rs:10");
        assert!(matches!(
            &results[0].kind,
            SearchResultKind::FileContent {
                path,
                line: 10,
                column: 5,
            } if path == "src/path:with-colon.rs"
        ));
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
