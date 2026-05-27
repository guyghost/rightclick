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
