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

#[test]
fn dev_script_unknown_command_suggests_match_case_insensitively() {
    let output = Command::new("bash")
        .args(["scripts/dev.sh", "TEST_MANY"])
        .output()
        .expect("dev script unknown command path should run");

    assert!(
        !output.status.success(),
        "unknown command should fail so callers notice the typo"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown command: TEST_MANY"),
        "dev script should echo the unknown command"
    );
    assert!(
        stderr.contains("bash scripts/dev.sh test-many"),
        "dev script should suggest the command for uppercase typos"
    );
}

#[test]
fn dev_script_test_many_explains_missing_filters_before_cargo_args() {
    let output = Command::new("bash")
        .args(["scripts/dev.sh", "test-many", "--", "--nocapture"])
        .output()
        .expect("dev script test-many usage path should run");

    assert!(
        !output.status.success(),
        "test-many should fail when no filters are provided"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("At least one test filter is required before --."),
        "dev script should explain that filters must come before cargo test args"
    );
    assert!(
        stderr.contains("Usage: bash scripts/dev.sh test-many <test-filter>..."),
        "dev script should still print test-many usage"
    );
}

#[test]
fn dev_script_test_one_explains_missing_filter_before_cargo_args() {
    let output = Command::new("bash")
        .args(["scripts/dev.sh", "test-one", "--", "--nocapture"])
        .output()
        .expect("dev script test-one usage path should run");

    assert!(
        !output.status.success(),
        "test-one should fail when no filter is provided"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("A test filter is required before --."),
        "dev script should explain that a filter must come before cargo test args"
    );
    assert!(
        stderr.contains("Usage: bash scripts/dev.sh test-one <test-filter>"),
        "dev script should still print test-one usage"
    );
}

#[test]
fn dev_script_test_one_explains_missing_filter() {
    let output = Command::new("bash")
        .args(["scripts/dev.sh", "test-one"])
        .output()
        .expect("dev script test-one usage path should run");

    assert!(
        !output.status.success(),
        "test-one should fail when no filter is provided"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("A test filter is required."),
        "dev script should explain that a test filter is required"
    );
    assert!(
        stderr.contains("Usage: bash scripts/dev.sh test-one <test-filter>"),
        "dev script should still print test-one usage"
    );
}
