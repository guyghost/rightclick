#!/usr/bin/env bash
set -euo pipefail

cmd="${1:-help}"
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
cd "$repo_root"

print_help() {
  cat <<'EOF'
Usage: bash scripts/dev.sh <command>

Commands:
  ci             same checks used by GitHub Actions
  pre-commit     quick checks to run before a local commit
  pre-push       full local verification before pushing
  doctor         check required and optional local developer tools
  rust-version   print the required Rust version from Cargo.toml
  check          diff check, fmt check, clippy with warnings denied, and tests
  quick          diff check, fmt check, and clippy with warnings denied
  script-check   validate shell helper script and justfile syntax when available
  diff-check     run git whitespace checks for staged and unstaged changes
  fmt-check      run cargo fmt --check
  fmt            run cargo fmt
  clippy         run cargo clippy --all-targets -- -D warnings
  lint           alias for clippy
  build          run cargo build
  build-release  run cargo build --release
  test           run cargo test
  doc-test       run cargo test --doc
  test-list      list available tests, optionally filtered by one or more filters
  test-one       run cargo test with a filter and optional cargo test args
  test-many      run cargo test once per filter, with optional shared cargo test args
  run            run RightClick locally, forwarding extra args to rightclick
  install-local  install RightClick from this checkout, forwarding extra args to cargo install

Examples:
  bash scripts/dev.sh doctor
  bash scripts/dev.sh ci
  bash scripts/dev.sh pre-commit
  bash scripts/dev.sh pre-push
  bash scripts/dev.sh test-list gitstatus search::overlay
  bash scripts/dev.sh test-one plugins::gitstatus
  bash scripts/dev.sh test-one test_plugin_commands -- --nocapture
  bash scripts/dev.sh test-many test_plugin_commands test_key_hints
  bash scripts/dev.sh test-many test_plugin_commands test_key_hints -- --nocapture
  bash scripts/dev.sh run -- --project ~/Developer/OSS/rightclick --debug
  bash scripts/dev.sh install-local --locked

Test filters are passed to Cargo as substring filters. Use test-list first when
you are unsure which module path or test name to target.
test-list only accepts filters; pass cargo test args to test-one or test-many
after --.
Use test-many when you want to check several filters in one command; cargo test
itself accepts only one substring filter per invocation.
test-list reports "Listed N tests." for the full list and
"Listed N tests for filter: ..." for filtered lists.
Filtered test-list, test-one, and test-many reuse one unfiltered Cargo test list
for validation so broad filters do not pay Cargo's slower filtered --list path.
The script prints when it is collecting the buffered Cargo test list so long
test discovery phases do not look stalled.
test-one and test-many print a "validate test filter" step before running Cargo
and then report "Matched N tests for filter: ..." so long filter checks are
visible and confirm the filter scope before the test run starts. test-many
validates all filters from one test list before running each filter separately.
When another Cargo job is using the default target directory, set
CARGO_TARGET_DIR=/tmp/rightclick-target-verify before a command to run checks
with an isolated build cache.

If you use just:
  just help
  just ci
  just pre-commit
  just pre-push
  just quick
  just script-check
  just diff-check
  just test-list gitstatus search::overlay
  just test-one plugins::gitstatus
  just test-one test_plugin_commands -- --nocapture
  just test-many test_plugin_commands test_key_hints
  just test-many test_plugin_commands test_key_hints -- --nocapture
EOF
}

known_commands() {
  cat <<'EOF'
ci
pre-commit
pre-push
doctor
rust-version
check
quick
script-check
diff-check
fmt-check
fmt
clippy
lint
build
build-release
test
doc-test
test-list
test-one
test-many
run
install-local
help
EOF
}

print_unknown_command() {
  local unknown="$1"
  local normalized_unknown
  normalized_unknown="$(printf '%s' "$unknown" | tr '[:upper:]' '[:lower:]')"
  normalized_unknown="${normalized_unknown//-/}"
  normalized_unknown="${normalized_unknown//_/}"
  local suggestions=()
  local command normalized_command

  while IFS= read -r command; do
    normalized_command="$(printf '%s' "$command" | tr '[:upper:]' '[:lower:]')"
    normalized_command="${normalized_command//-/}"
    normalized_command="${normalized_command//_/}"
    if [[ "$command" == *"$unknown"* || "$normalized_command" == *"$normalized_unknown"* ]]; then
      suggestions+=("$command")
    fi
  done < <(known_commands)

  if [ "${#suggestions[@]}" -eq 0 ] && [ "${#unknown}" -ge 2 ]; then
    local prefix="${normalized_unknown:0:2}"
    while IFS= read -r command; do
      normalized_command="$(printf '%s' "$command" | tr '[:upper:]' '[:lower:]')"
      normalized_command="${normalized_command//-/}"
      normalized_command="${normalized_command//_/}"
      if [[ "$normalized_command" == "$prefix"* ]]; then
        suggestions+=("$command")
      fi
    done < <(known_commands)
  fi

  echo "Unknown command: $unknown" >&2
  if [ "${#suggestions[@]}" -ne 0 ]; then
    if [ "${#suggestions[@]}" -eq 1 ]; then
      echo "Did you mean this command?" >&2
    else
      echo "Did you mean one of these commands?" >&2
    fi
    for command in "${suggestions[@]:0:5}"; do
      echo "  bash scripts/dev.sh $command" >&2
    done
  else
    echo "Run bash scripts/dev.sh help to list available commands." >&2
  fi
  echo >&2
  print_help >&2
}

print_command() {
  local arg quoted
  local separator=""

  for arg in "$@"; do
    printf -v quoted '%q' "$arg"
    printf '%s%s' "$separator" "$quoted"
    separator=" "
  done
}

run_step() {
  local target_dir_prefix=""
  if [ "${1:-}" = "cargo" ] && [ -n "${CARGO_TARGET_DIR:-}" ]; then
    local quoted_target_dir
    printf -v quoted_target_dir '%q' "$CARGO_TARGET_DIR"
    target_dir_prefix="CARGO_TARGET_DIR=$quoted_target_dir "
  fi

  printf '\n==> ' >&2
  printf '%s' "$target_dir_prefix" >&2
  print_command "$@" >&2
  printf '\n' >&2
  "$@"
}

print_cargo_list_step() {
  if [ -n "${CARGO_TARGET_DIR:-}" ]; then
    local quoted_target_dir
    printf -v quoted_target_dir '%q' "$CARGO_TARGET_DIR"
    printf '\n==> CARGO_TARGET_DIR=%s cargo test -- --list\n' "$quoted_target_dir" >&2
  else
    printf '\n==> cargo test -- --list\n' >&2
  fi
}

run_script_checks() {
  run_step bash -n scripts/dev.sh
  if command -v just >/dev/null 2>&1; then
    run_step just --summary >/dev/null
  else
    printf '\n==> skip just --summary (just not installed)\n' >&2
  fi
}

run_diff_checks() {
  run_step git diff --check
  run_step git diff --cached --check
}

run_checks() {
  run_script_checks
  run_diff_checks
  run_step cargo fmt --check
  run_step cargo clippy --all-targets -- -D warnings
  run_step cargo test
}

run_quick_checks() {
  run_script_checks
  run_diff_checks
  run_step cargo fmt --check
  run_step cargo clippy --all-targets -- -D warnings
}

print_test_filter_hint() {
  local filter="$1"
  local quoted_filter
  local broader_filters=()
  local token quoted_token quoted_broader_filters

  printf -v quoted_filter '%q' "$filter"
  echo "Test filters are passed to Cargo as substring filters; module paths work too." >&2
  echo "Fix the filter before rerunning; no matching tests were selected." >&2
  echo "Inspect matches with: bash scripts/dev.sh test-list $quoted_filter" >&2
  while IFS= read -r token; do
    case "$token" in
      test|tests|should|with|without|when|then|from|into|uses|use|and|the|for)
        continue
        ;;
    esac

    if [ "${#token}" -ge 4 ]; then
      broader_filters+=("$token")
    fi
    if [ "${#broader_filters[@]}" -ge 4 ]; then
      break
    fi
  done < <(printf '%s\n' "$filter" | tr '[:upper:]' '[:lower:]' | tr -cs '[:alnum:]' '\n')

  if [ "${#broader_filters[@]}" -ne 0 ]; then
    quoted_broader_filters=()
    for token in "${broader_filters[@]}"; do
      printf -v quoted_token '%q' "$token"
      quoted_broader_filters+=("$quoted_token")
    done
    echo "Try broader matches with: bash scripts/dev.sh test-list ${quoted_broader_filters[*]}" >&2
  fi
  echo "List every test with: bash scripts/dev.sh test-list" >&2
}

ensure_test_filter_arg() {
  local command_name="$1"
  local usage="$2"
  local filter="$3"

  if [ "$filter" = "--" ]; then
    if [ "$command_name" = "test-list" ]; then
      echo "test-list does not accept cargo test args; pass filters without --." >&2
      echo "Usage: bash scripts/dev.sh $command_name $usage" >&2
      exit 2
    fi
    echo "A test filter is required before --." >&2
    echo "Usage: bash scripts/dev.sh $command_name $usage" >&2
    exit 2
  fi
}

test_filter_match_count() {
  local output="$1"
  local filter="$2"

  printf '%s\n' "$output" | awk -v filter="$filter" '
    index($0, filter) && /: test$/ { matches++ }
    END { print matches + 0 }
  '
}

print_test_filter_matches() {
  local output="$1"
  local filter="$2"

  printf '%s\n' "$output" | awk -v filter="$filter" '
    index($0, filter) && /: test$/ { print }
  '
}

print_test_list_collection_note() {
  echo "Collecting test list; Cargo output is buffered until listing completes." >&2
}

ensure_test_filters_match() {
  local output
  local filter
  local matches

  print_cargo_list_step
  print_test_list_collection_note
  if ! output="$(cargo test -- --list 2>&1)"; then
    printf '%s\n' "$output" >&2
    exit 1
  fi

  for filter in "$@"; do
    printf '\n==> validate test filter %s\n' "$filter" >&2
    matches="$(test_filter_match_count "$output" "$filter")"
    if [ "$matches" -eq 0 ]; then
      echo "No tests matched filter: $filter" >&2
      print_test_filter_hint "$filter"
      exit 2
    fi

    if [ "$matches" -eq 1 ]; then
      echo "Matched 1 test for filter: $filter" >&2
    else
      echo "Matched $matches tests for filter: $filter" >&2
    fi
  done
}

list_tests_for_filters() {
  local output
  local filter
  local matches

  print_cargo_list_step
  print_test_list_collection_note
  if ! output="$(cargo test -- --list 2>&1)"; then
    printf '%s\n' "$output" >&2
    exit 1
  fi

  for filter in "$@"; do
    matches="$(test_filter_match_count "$output" "$filter")"
    if [ "$matches" -eq 0 ]; then
      echo "No tests matched filter: $filter" >&2
      print_test_filter_hint "$filter"
      exit 2
    fi
  done

  for filter in "$@"; do
    matches="$(test_filter_match_count "$output" "$filter")"
    if [ "$matches" -eq 1 ]; then
      echo "Listed 1 test for filter: $filter" >&2
    else
      echo "Listed $matches tests for filter: $filter" >&2
    fi
    print_test_filter_matches "$output" "$filter"
  done
}

list_all_tests() {
  local output
  local matches

  print_cargo_list_step
  print_test_list_collection_note
  if ! output="$(cargo test -- --list 2>&1)"; then
    printf '%s\n' "$output" >&2
    exit 1
  fi

  matches="$(printf '%s\n' "$output" | grep -c ': test$' || true)"
  if [ "$matches" -eq 1 ]; then
    echo "Listed 1 test." >&2
  else
    echo "Listed $matches tests." >&2
  fi
  printf '%s\n' "$output"
}

rust_version() {
  local version
  version="$(sed -n 's/^rust-version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
  if [ -z "$version" ]; then
    echo "rust-version not found in Cargo.toml" >&2
    return 1
  fi
  printf '%s\n' "$version"
}

case "$cmd" in
  ci)
    run_checks
    ;;
  pre-commit)
    run_quick_checks
    ;;
  pre-push)
    run_checks
    ;;
  rust-version)
    rust_version
    ;;
  doctor)
    missing_required=0
    optional_setup=()
    printf -v quoted_repo_root '%q' "$repo_root"
    printf 'RightClick doctor\n'
    printf 'repo %s\n' "$repo_root"

    require_cmd() {
      if command -v "$1" >/dev/null 2>&1; then
        printf 'ok   %s\n' "$1"
      else
        printf 'miss %s (required)\n' "$1"
        missing_required=1
      fi
    }

    optional_cmd_hint() {
      if command -v "$1" >/dev/null 2>&1; then
        printf 'ok   %s\n' "$1"
      else
        printf 'skip %s (optional; %s)\n' "$1" "$2"
        if [ -n "${3:-}" ]; then
          optional_setup+=("$3")
        fi
      fi
    }

    optional_td_workspace() {
      if ! command -v td >/dev/null 2>&1; then
        return
      fi

      if td usage -q -w "$repo_root" >/dev/null 2>&1; then
        printf 'ok   td workspace\n'
      else
        printf 'setup td workspace (optional; run td init in %s to enable task tracking)\n' "$repo_root"
        optional_setup+=("cd $quoted_repo_root && td init")
      fi
    }

    version_ge() {
      local current="$1"
      local required="$2"
      local current_major current_minor current_patch
      local required_major required_minor required_patch

      IFS=. read -r current_major current_minor current_patch <<<"${current%%-*}"
      IFS=. read -r required_major required_minor required_patch <<<"${required%%-*}"
      current_patch="${current_patch:-0}"
      required_patch="${required_patch:-0}"

      if ((current_major != required_major)); then
        ((current_major > required_major))
        return
      fi
      if ((current_minor != required_minor)); then
        ((current_minor > required_minor))
        return
      fi
      ((current_patch >= required_patch))
    }

    require_cmd cargo
    require_cmd rustc
    require_cmd cargo-fmt
    require_cmd cargo-clippy
    require_cmd git
    require_cmd rg
    optional_cmd_hint tmux "needed for embedded terminal sessions; install with brew install tmux" "brew install tmux"
    optional_cmd_hint td "enables task tracking workflows; run td init in this checkout after installing" "install td, then cd $quoted_repo_root && td init"
    optional_td_workspace
    optional_cmd_hint just "enables shorter justfile commands; install with brew install just or cargo install just" "brew install just"

    if command -v cargo >/dev/null 2>&1; then
      cargo --version
    fi
    if command -v rustc >/dev/null 2>&1; then
      rustc --version
      required_rust="$(rust_version)"
      current_rust="$(rustc --version | awk '{print $2}')"
      if [ -n "$required_rust" ]; then
        if version_ge "$current_rust" "$required_rust"; then
          printf 'ok   rustc >= %s (%s)\n' "$required_rust" "$current_rust"
        else
          printf 'miss rustc >= %s (%s installed)\n' "$required_rust" "$current_rust"
          missing_required=1
        fi
      fi
    fi

    if [ "$missing_required" -ne 0 ]; then
      echo "One or more required tools are missing." >&2
      exit 1
    fi

    echo "All required tools are available."
    if [ "${#optional_setup[@]}" -ne 0 ]; then
      echo "Optional setup:"
      for setup_step in "${optional_setup[@]}"; do
        echo "  $setup_step"
      done
    fi
    echo "Next checks:"
    echo "  bash scripts/dev.sh quick"
    echo "  bash scripts/dev.sh pre-push"
    ;;
  check)
    run_checks
    ;;
  quick)
    run_quick_checks
    ;;
  script-check)
    run_script_checks
    ;;
  diff-check)
    run_diff_checks
    ;;
  fmt-check)
    run_step cargo fmt --check
    ;;
  fmt)
    run_step cargo fmt
    ;;
  clippy|lint)
    run_step cargo clippy --all-targets -- -D warnings
    ;;
  build)
    run_step cargo build
    ;;
  build-release)
    run_step cargo build --release
    ;;
  test)
    run_step cargo test
    ;;
  doc-test)
    run_step cargo test --doc
    ;;
  test-list)
    shift
    if [ "$#" -eq 0 ]; then
      list_all_tests
    else
      for filter in "$@"; do
        ensure_test_filter_arg "test-list" "[<test-filter>...]" "$filter"
      done
      list_tests_for_filters "$@"
    fi
    ;;
  test-one)
    shift
    if [ "$#" -eq 0 ]; then
      echo "A test filter is required." >&2
      echo "Usage: bash scripts/dev.sh test-one <test-filter> [-- <cargo-test-args>]" >&2
      exit 2
    fi
    ensure_test_filter_arg "test-one" "<test-filter> [-- <cargo-test-args>]" "$1"
    ensure_test_filters_match "$1"
    run_step cargo test "$@"
    ;;
  test-many)
    shift
    if [ "$#" -eq 0 ]; then
      echo "At least one test filter is required." >&2
      echo "Usage: bash scripts/dev.sh test-many <test-filter>... [-- <cargo-test-args>]" >&2
      exit 2
    fi

    filters=()
    cargo_test_args=()
    collect_cargo_args=0
    for arg in "$@"; do
      if [ "$collect_cargo_args" -eq 0 ] && [ "$arg" = "--" ]; then
        collect_cargo_args=1
        continue
      fi
      if [ "$collect_cargo_args" -eq 0 ]; then
        filters+=("$arg")
      else
        cargo_test_args+=("$arg")
      fi
    done

    if [ "${#filters[@]}" -eq 0 ]; then
      echo "At least one test filter is required before --." >&2
      echo "Usage: bash scripts/dev.sh test-many <test-filter>... [-- <cargo-test-args>]" >&2
      exit 2
    fi

    ensure_test_filters_match "${filters[@]}"
    for filter in "${filters[@]}"; do
      if [ "${#cargo_test_args[@]}" -eq 0 ]; then
        run_step cargo test "$filter"
      else
        run_step cargo test "$filter" -- "${cargo_test_args[@]}"
      fi
    done
    ;;
  run)
    shift
    if [ "${1:-}" = "--" ]; then
      shift
    fi
    run_step cargo run -- "$@"
    ;;
  install-local)
    shift
    run_step cargo install --path . "$@"
    ;;
  help|--help|-h)
    print_help
    ;;
  *)
    print_unknown_command "$cmd"
    exit 2
    ;;
esac
