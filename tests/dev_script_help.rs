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
        stdout.contains(
            "test-one and test-many print a \"validate test filter\" step before running Cargo"
        ),
        "dev script help should explain the filter validation progress line"
    );
    assert!(
        stdout.contains("and then report \"Matched N tests for filter: ...\""),
        "dev script help should explain matched test count feedback"
    );
    assert!(
        stdout.contains("test-many\nvalidates all filters from one test list"),
        "dev script help should explain that test-many batches validation"
    );
    assert!(
        stdout.contains("test-list reports \"Listed N tests.\" for the full list"),
        "dev script help should explain unfiltered test-list count feedback"
    );
    assert!(
        stdout.contains("When passed multiple filters, test-list reuses one test list"),
        "dev script help should explain batched test-list filtering"
    );
}

#[test]
fn dev_script_test_list_reports_filter_match_count() {
    let output = Command::new("bash")
        .args([
            "scripts/dev.sh",
            "test-list",
            "dev_script_help_explains_test_many",
        ])
        .output()
        .expect("dev script test-list should run");

    assert!(
        output.status.success(),
        "test-list should pass for a known filter: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Listed 1 test for filter: dev_script_help_explains_test_many"),
        "test-list should report how many tests matched the filter"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("dev_script_help_explains_test_many: test"),
        "test-list should still print the matching Cargo test list"
    );
}

#[test]
fn dev_script_test_list_lists_multiple_filters_from_single_test_list() {
    let output = Command::new("bash")
        .args([
            "scripts/dev.sh",
            "test-list",
            "dev_script_help_explains_test_many",
            "dev_script_run_step_shell_quotes_spaced_args",
        ])
        .output()
        .expect("dev script test-list should run");

    assert!(
        output.status.success(),
        "test-list should pass for known filters: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.matches("==> cargo test -- --list").count(),
        1,
        "test-list should list tests once for multiple filters: {stderr}"
    );
    assert!(
        stderr.contains("Listed 1 test for filter: dev_script_help_explains_test_many"),
        "test-list should report the first filter count"
    );
    assert!(
        stderr.contains("Listed 1 test for filter: dev_script_run_step_shell_quotes_spaced_args"),
        "test-list should report the second filter count"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("dev_script_help_explains_test_many: test"),
        "test-list should print the first matching test"
    );
    assert!(
        stdout.contains("dev_script_run_step_shell_quotes_spaced_args: test"),
        "test-list should print the second matching test"
    );
}

#[test]
fn dev_script_test_list_validates_all_filters_before_printing_matches() {
    let output = Command::new("bash")
        .args([
            "scripts/dev.sh",
            "test-list",
            "dev_script_help_explains_test_many",
            "missing_filter_for_partial_output",
        ])
        .output()
        .expect("dev script test-list should run");

    assert!(
        !output.status.success(),
        "test-list should fail when any filter does not match"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.matches("==> cargo test -- --list").count(),
        1,
        "test-list should still list tests once for multiple filters: {stderr}"
    );
    assert!(
        stderr.contains("No tests matched filter: missing_filter_for_partial_output"),
        "test-list should report the missing filter"
    );
    assert!(
        !stderr.contains("Listed 1 test for filter: dev_script_help_explains_test_many"),
        "test-list should not print successful filter counts before all filters validate"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("dev_script_help_explains_test_many: test"),
        "test-list should not print partial matches when a later filter is invalid"
    );
}

#[test]
fn dev_script_test_list_reports_unfiltered_match_count() {
    let output = Command::new("bash")
        .args(["scripts/dev.sh", "test-list"])
        .output()
        .expect("dev script test-list should run");

    assert!(
        output.status.success(),
        "test-list should pass without filters: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Listed "),
        "test-list should report how many tests were listed"
    );
    assert!(
        stderr.contains(" tests."),
        "test-list should use the unfiltered count wording"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("dev_script_help_explains_test_many: test"),
        "test-list should still print the full Cargo test list"
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
        stderr.contains("Did you mean this command?"),
        "dev script should use singular wording for one suggestion"
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
        stderr.contains("Did you mean this command?"),
        "dev script should use singular wording for one case-insensitive suggestion"
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
        stderr.contains("Did you mean this command?"),
        "dev script should use singular wording for one prefix suggestion"
    );
    assert!(
        stderr.contains("bash scripts/dev.sh quick"),
        "dev script should suggest prefix matches case-insensitively"
    );
}

#[test]
fn dev_script_unknown_command_uses_plural_wording_for_multiple_suggestions() {
    let output = Command::new("bash")
        .args(["scripts/dev.sh", "te"])
        .output()
        .expect("dev script unknown command path should run");

    assert!(
        !output.status.success(),
        "unknown command should fail so callers notice the typo"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown command: te"),
        "dev script should echo the unknown command"
    );
    assert!(
        stderr.contains("Did you mean one of these commands?"),
        "dev script should use plural wording for multiple suggestions"
    );
    assert!(
        stderr.contains("bash scripts/dev.sh test"),
        "dev script should include the direct test command suggestion"
    );
    assert!(
        stderr.contains("bash scripts/dev.sh test-many"),
        "dev script should include related test command suggestions"
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
        stderr.contains("Matched 1 test for filter: dev_script_help_explains_test_many"),
        "dev script should report how many tests matched the filter"
    );
    assert!(
        stderr.contains("==> cargo test dev_script_help_explains_test_many -- --nocapture"),
        "dev script should still report the actual cargo test command"
    );
}

#[test]
fn dev_script_test_many_validates_filters_from_single_test_list() {
    let output = Command::new("bash")
        .args([
            "scripts/dev.sh",
            "test-many",
            "dev_script_help_explains_test_many",
            "dev_script_run_step_shell_quotes_spaced_args",
            "--",
            "--nocapture",
        ])
        .output()
        .expect("dev script test-many should run");

    assert!(
        output.status.success(),
        "test-many should pass for known filters: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.matches("==> cargo test -- --list").count(),
        1,
        "test-many should list tests once for all filter validation: {stderr}"
    );
    assert!(
        stderr.contains("Matched 1 test for filter: dev_script_help_explains_test_many"),
        "test-many should still report the first filter match count"
    );
    assert!(
        stderr.contains("Matched 1 test for filter: dev_script_run_step_shell_quotes_spaced_args"),
        "test-many should still report the second filter match count"
    );
    assert!(
        stderr.contains("==> cargo test dev_script_help_explains_test_many -- --nocapture"),
        "test-many should still run the first filter separately"
    );
    assert!(
        stderr
            .contains("==> cargo test dev_script_run_step_shell_quotes_spaced_args -- --nocapture"),
        "test-many should still run the second filter separately"
    );
}

#[test]
fn dev_script_run_step_shell_quotes_spaced_args() {
    let output = Command::new("bash")
        .args([
            "scripts/dev.sh",
            "test-one",
            "dev_script_help_explains_test_many",
            "--",
            "--skip",
            "no matching skipped test",
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
        stderr.contains("==> cargo test dev_script_help_explains_test_many -- --skip no\\ matching\\ skipped\\ test"),
        "dev script should quote spaced args in progress output: {stderr}"
    );
}
