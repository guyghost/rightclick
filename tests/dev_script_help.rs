use std::process::Command;

#[test]
fn dev_script_help_explains_test_many() {
    let output = Command::new("bash")
        .args(["scripts/dev.sh", "help"])
        .output()
        .expect("dev script help should run");

    assert!(
        output.status.success(),
        "dev script help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Use test-many when you want to check several filters in one command"),
        "dev script help should explain when to use test-many"
    );
    assert!(
        stdout.contains("cargo test\nitself accepts only one substring filter per invocation"),
        "dev script help should explain why test-many runs filters separately"
    );
}

#[test]
fn dev_script_unknown_command_suggests_hyphenated_match_for_underscore() {
    let output = Command::new("bash")
        .args(["scripts/dev.sh", "test_many"])
        .output()
        .expect("dev script unknown command path should run");

    assert!(
        !output.status.success(),
        "unknown command should fail so callers notice the typo"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown command: test_many"),
        "dev script should echo the unknown command"
    );
    assert!(
        stderr.contains("bash scripts/dev.sh test-many"),
        "dev script should suggest the hyphenated command for underscore typos"
    );
}
