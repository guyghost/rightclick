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
        README.contains("| `:` | Search commands |"),
        "README keyboard shortcuts should document the direct command search shortcut"
    );
    assert!(
        README.contains("Press `/` to open global search, or `:` to open it directly on commands."),
        "README search docs should explain the direct commands scope shortcut"
    );
}
