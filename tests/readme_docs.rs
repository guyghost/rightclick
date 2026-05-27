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
        README.contains("| `:` | Command search |"),
        "README keyboard shortcuts should use the UI command search label"
    );
    assert!(
        README.contains("Press `/` to open global search, or `:` to open command search directly."),
        "README search docs should explain the direct command search shortcut"
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
}
