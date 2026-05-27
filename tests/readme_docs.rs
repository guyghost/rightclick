const README: &str = include_str!("../README.md");

#[test]
fn keyboard_shortcuts_describe_contextual_refresh() {
    assert!(
        README.contains("| `r`, `Ctrl+R` | Refresh current view |"),
        "README keyboard shortcuts should describe both refresh bindings as view-scoped"
    );
    assert!(
        !README.contains("| `r` | Refresh |"),
        "README keyboard shortcuts should not use the generic refresh label"
    );
    assert!(
        !README.contains("| `r` | Refresh current view |"),
        "README keyboard shortcuts should not omit the Ctrl+R refresh binding"
    );
}

#[test]
fn keyboard_shortcuts_include_command_search() {
    assert!(
        README.contains("| `/` | Global search |"),
        "README keyboard shortcuts should use the UI global search label"
    );
    assert!(
        README.contains("| `:` | Command search |"),
        "README keyboard shortcuts should use the UI command search label"
    );
    assert!(
        README.contains("Press `/` to open global search, or `:` to open command search directly."),
        "README search docs should explain the direct command search shortcut"
    );
}

#[test]
fn search_docs_explain_command_search_fields() {
    assert!(
        README.contains(
            "- **Commands**: search commands by name, description, shortcut, category, or command ID",
        ),
        "README search docs should match command palette search fields"
    );
    assert!(
        !README.contains("search available commands with their current descriptions"),
        "README search docs should not imply command search only uses descriptions"
    );
}

#[test]
fn keyboard_shortcuts_use_display_key_casing() {
    for row in [
        "| `q`, `Ctrl+C` | Quit |",
        "| `Tab` / `Shift+Tab` | Navigate plugins or panes |",
        "| `Enter` | Select |",
        "| `Esc` | Back/close |",
    ] {
        assert!(
            README.contains(row),
            "README keyboard shortcuts should match UI key casing: {row}"
        );
    }

    assert!(
        README.contains("Use\n`Tab` or `Shift+Tab` inside the overlay to switch scope:"),
        "README search docs should match UI key casing for Tab and Shift+Tab"
    );
    assert!(
        README.contains("Use `Ctrl+Tab` or `Ctrl+Shift+Tab` there when"),
        "README should document plugin navigation from pane-based views"
    );
}

#[test]
fn developer_commands_include_diff_check() {
    assert!(
        README.contains("bash scripts/dev.sh diff-check"),
        "README developer commands should document the whitespace diff check"
    );
    assert!(
        README.contains("staged and unstaged changes"),
        "README should clarify that diff-check covers both staged and unstaged changes"
    );
}

#[test]
fn developer_commands_explain_td_workspace_setup() {
    assert!(
        README.contains("`doctor` prints `setup td workspace`"),
        "README should document the doctor output for an uninitialized td workspace"
    );
    assert!(
        README.contains("repo path where\n`td init` should be run"),
        "README should explain how to finish optional td setup"
    );
}

#[test]
fn developer_commands_explain_test_many() {
    assert!(
        README.contains("Use `test-many` when you want to check several filters in one command"),
        "README should explain when to use test-many"
    );
    assert!(
        README
            .contains("Cargo itself accepts only one substring filter per `cargo test` invocation"),
        "README should clarify why test-many runs filters separately"
    );
    assert!(
        README.contains(
            "`test-list` only accepts filters; pass Cargo test args to `test-one` or\n`test-many` after `--`"
        ),
        "README should clarify that cargo test args belong on test-one/test-many"
    );
    assert!(
        README.contains("`test-one` and `test-many` print a `validate test filter` step"),
        "README should explain the filter validation progress line"
    );
    assert!(
        README.contains("`Listed N tests for filter: ...` for filtered lists"),
        "README should explain filtered test-list count feedback"
    );
    assert!(
        README.contains("`test-list` reports `Listed N tests.` for the full list"),
        "README should explain unfiltered test-list count feedback"
    );
    assert!(
        README.contains("Filtered `test-list`,\n`test-one`, and `test-many` reuse one unfiltered Cargo test list"),
        "README should explain that filtered test commands use one unfiltered list"
    );
    assert!(
        README.contains("so broad filters avoid Cargo's slower filtered `--list` path"),
        "README should explain why filtered commands avoid Cargo's filtered list path"
    );
    assert!(
        README.contains("collecting the buffered Cargo test list"),
        "README should explain the visible progress note for buffered test listing"
    );
    assert!(
        README.contains("then report `Matched N tests for filter: ...`"),
        "README should explain the matched test count feedback"
    );
    assert!(
        README.contains("`test-many`\nvalidates all filters from one test list"),
        "README should explain that test-many batches filter validation"
    );
    assert!(
        README.contains("`CARGO_TARGET_DIR=/tmp/rightclick-target-verify`"),
        "README should document the isolated Cargo target directory example"
    );
    assert!(
        README.contains("with an isolated build cache"),
        "README should explain why to set CARGO_TARGET_DIR"
    );
}
