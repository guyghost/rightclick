use std::process::Command;

#[test]
fn dev_script_doctor_explains_uninitialized_td_workspace() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "rightclick-doctor-td-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&temp_dir).expect("temp dir should be created");

    let fake_td = temp_dir.join("td");
    fs::write(
        &fake_td,
        "#!/usr/bin/env bash\nprintf 'database not found\\n' >&2\nexit 1\n",
    )
    .expect("fake td should be written");
    fs::set_permissions(&fake_td, fs::Permissions::from_mode(0o755))
        .expect("fake td should be executable");

    let path = format!(
        "{}:{}",
        temp_dir.display(),
        std::env::var("PATH").expect("PATH should be set")
    );
    let output = Command::new("bash")
        .args(["scripts/dev.sh", "doctor"])
        .env("PATH", path)
        .output()
        .expect("dev script doctor should run");

    fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");

    assert!(
        output.status.success(),
        "doctor should not fail for optional td workspace setup: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ok   td"),
        "doctor should detect the td executable"
    );
    assert!(
        stdout.contains("setup td workspace (optional; run td init in "),
        "doctor should explain that the td workspace still needs setup"
    );
    assert!(
        stdout.contains("Optional setup:"),
        "doctor should include a follow-up setup section"
    );
    assert!(
        stdout.contains("&& td init"),
        "doctor should print the exact td init follow-up command"
    );
}

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
    assert!(
        stdout.contains("test-list only accepts filters; pass cargo test args to test-one or test-many\nafter --."),
        "dev script help should explain where cargo test args are supported"
    );
    assert!(
        stdout.contains("test-one and test-many print validate test filter before running Cargo"),
        "dev script help should explain the filter validation progress line"
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
fn dev_script_unknown_command_suggests_prefix_match_case_insensitively() {
    let output = Command::new("bash")
        .args(["scripts/dev.sh", "QU"])
        .output()
        .expect("dev script unknown command path should run");

    assert!(
        !output.status.success(),
        "unknown command should fail so callers notice the typo"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown command: QU"),
        "dev script should echo the unknown command"
    );
    assert!(
        stderr.contains("bash scripts/dev.sh quick"),
        "dev script should suggest prefix matches case-insensitively"
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
fn dev_script_test_list_explains_unsupported_cargo_args() {
    let output = Command::new("bash")
        .args(["scripts/dev.sh", "test-list", "--", "--nocapture"])
        .output()
        .expect("dev script test-list usage path should run");

    assert!(
        !output.status.success(),
        "test-list should fail when cargo test args are passed with --"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("test-list does not accept cargo test args; pass filters without --."),
        "dev script should explain that test-list does not accept cargo test args"
    );
    assert!(
        stderr.contains("Usage: bash scripts/dev.sh test-list [<test-filter>...]"),
        "dev script should still print test-list usage"
    );
}

#[test]
fn dev_script_test_list_rejects_cargo_args_before_listing_filters() {
    let output = Command::new("bash")
        .args([
            "scripts/dev.sh",
            "test-list",
            "dev_script_help",
            "--",
            "--nocapture",
        ])
        .output()
        .expect("dev script test-list usage path should run");

    assert!(
        !output.status.success(),
        "test-list should fail before listing filters when -- is present"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("test-list does not accept cargo test args; pass filters without --."),
        "dev script should explain that test-list does not accept cargo test args"
    );
    assert!(
        !stderr.contains("==> cargo test dev_script_help -- --list"),
        "dev script should validate all test-list args before listing any filter"
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

#[test]
fn dev_script_test_many_explains_missing_filters() {
    let output = Command::new("bash")
        .args(["scripts/dev.sh", "test-many"])
        .output()
        .expect("dev script test-many usage path should run");

    assert!(
        !output.status.success(),
        "test-many should fail when no filters are provided"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("At least one test filter is required."),
        "dev script should explain that at least one test filter is required"
    );
    assert!(
        stderr.contains("Usage: bash scripts/dev.sh test-many <test-filter>..."),
        "dev script should still print test-many usage"
    );
}

#[test]
fn dev_script_test_one_reports_filter_validation_before_running() {
    let output = Command::new("bash")
        .args([
            "scripts/dev.sh",
            "test-one",
            "dev_script_help_explains_test_many",
            "--",
            "--nocapture",
        ])
        .output()
        .expect("dev script test-one should run");

    assert!(
        output.status.success(),
        "test-one should pass for a known filter: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("==> validate test filter dev_script_help_explains_test_many"),
        "dev script should report the filter validation step"
    );
    assert!(
        stderr.contains("==> cargo test dev_script_help_explains_test_many -- --nocapture"),
        "dev script should still report the actual cargo test command"
    );
}
