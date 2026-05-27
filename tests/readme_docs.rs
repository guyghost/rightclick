const README: &str = include_str!("../README.md");

#[test]
fn keyboard_shortcuts_describe_contextual_refresh() {
    assert!(
        README.contains("| `r` | Refresh current view |"),
        "README keyboard shortcuts should describe refresh as view-scoped"
    );
    assert!(
        !README.contains("| `r` | Refresh |"),
        "README keyboard shortcuts should not use the generic refresh label"
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
        "| `Tab` / `Shift+Tab` | Navigate plugins |",
        "| `Enter` | Select |",
        "| `Esc` | Back/close |",
    ] {
        assert!(
            README.contains(row),
            "README keyboard shortcuts should match UI key casing: {row}"
        );
    }

    assert!(
        README.contains("Use\n`Tab` inside the overlay to switch scope:"),
        "README search docs should match UI key casing for Tab"
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
}
