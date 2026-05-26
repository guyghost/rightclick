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
  doctor         check required and optional local developer tools
  rust-version   print the required Rust version from Cargo.toml
  check          fmt check, clippy with warnings denied, and tests
  quick          fmt check and clippy with warnings denied
  fmt-check      run cargo fmt --check
  fmt            run cargo fmt
  clippy         run cargo clippy --all-targets -- -D warnings
  lint           alias for clippy
  build          run cargo build
  build-release  run cargo build --release
  test           run cargo test
  doc-test       run cargo test --doc
  test-list      list available tests, optionally filtered by one or more filters
  test-one       run cargo test with a filter
  test-many      run cargo test once per filter
  run            run RightClick locally
  install-local  install RightClick from this checkout

Examples:
  bash scripts/dev.sh doctor
  bash scripts/dev.sh ci
  bash scripts/dev.sh test-list gitstatus search::overlay
  bash scripts/dev.sh test-one plugins::gitstatus
  bash scripts/dev.sh test-many test_plugin_commands test_footer_hints

If you use just:
  just help
  just ci
  just quick
  just test-list gitstatus search::overlay
  just test-one plugins::gitstatus
  just test-many test_plugin_commands test_footer_hints
EOF
}

run_step() {
  printf '\n==> %s\n' "$*" >&2
  "$@"
}

run_checks() {
  run_step cargo fmt --check
  run_step cargo clippy --all-targets -- -D warnings
  run_step cargo test
}

run_quick_checks() {
  run_step cargo fmt --check
  run_step cargo clippy --all-targets -- -D warnings
}

ensure_test_filter_matches() {
  local filter="$1"
  local output
  local matches

  if ! output="$(cargo test "$filter" -- --list 2>&1)"; then
    printf '%s\n' "$output" >&2
    exit 1
  fi

  matches="$(printf '%s\n' "$output" | grep -c ': test$' || true)"
  if [ "$matches" -eq 0 ]; then
    echo "No tests matched filter: $filter" >&2
    echo "Use 'bash scripts/dev.sh test-list [filter...]' to inspect available test names." >&2
    exit 2
  fi
}

list_tests_for_filter() {
  local filter="$1"
  local output
  local matches

  printf '\n==> cargo test %s -- --list\n' "$filter" >&2
  if ! output="$(cargo test "$filter" -- --list 2>&1)"; then
    printf '%s\n' "$output" >&2
    exit 1
  fi

  printf '%s\n' "$output"

  matches="$(printf '%s\n' "$output" | grep -c ': test$' || true)"
  if [ "$matches" -eq 0 ]; then
    echo "No tests matched filter: $filter" >&2
    echo "Try a broader filter or run 'bash scripts/dev.sh test-list' to list every test." >&2
    exit 2
  fi
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
  rust-version)
    rust_version
    ;;
  doctor)
    missing_required=0

    require_cmd() {
      if command -v "$1" >/dev/null 2>&1; then
        printf 'ok   %s\n' "$1"
      else
        printf 'miss %s (required)\n' "$1"
        missing_required=1
      fi
    }

    optional_cmd() {
      if command -v "$1" >/dev/null 2>&1; then
        printf 'ok   %s\n' "$1"
      else
        printf 'skip %s (optional)\n' "$1"
      fi
    }

    optional_td_workspace() {
      if ! command -v td >/dev/null 2>&1; then
        return
      fi

      if td usage -q -w "$repo_root" >/dev/null 2>&1; then
        printf 'ok   td workspace\n'
      else
        printf 'skip td workspace (optional; run td init to enable task tracking)\n'
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
    optional_cmd tmux
    optional_cmd td
    optional_td_workspace
    optional_cmd just

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
    ;;
  check)
    run_checks
    ;;
  quick)
    run_quick_checks
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
      run_step cargo test -- --list
    else
      for filter in "$@"; do
        list_tests_for_filter "$filter"
      done
    fi
    ;;
  test-one)
    shift
    if [ "$#" -eq 0 ]; then
      echo "Usage: bash scripts/dev.sh test-one <test-filter> [-- <cargo-test-args>]" >&2
      exit 2
    fi
    ensure_test_filter_matches "$1"
    run_step cargo test "$@"
    ;;
  test-many)
    shift
    if [ "$#" -eq 0 ]; then
      echo "Usage: bash scripts/dev.sh test-many <test-filter>..." >&2
      exit 2
    fi
    for filter in "$@"; do
      ensure_test_filter_matches "$filter"
    done
    for filter in "$@"; do
      run_step cargo test "$filter"
    done
    ;;
  run)
    run_step cargo run
    ;;
  install-local)
    run_step cargo install --path .
    ;;
  help|--help|-h)
    print_help
    ;;
  *)
    echo "Unknown command: $cmd" >&2
    echo >&2
    print_help >&2
    exit 2
    ;;
esac
