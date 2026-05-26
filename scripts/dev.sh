#!/usr/bin/env bash
set -euo pipefail

cmd="${1:-help}"

case "$cmd" in
  ci)
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
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

    require_cmd cargo
    require_cmd rustc
    require_cmd cargo-fmt
    require_cmd cargo-clippy
    require_cmd git
    require_cmd rg
    optional_cmd tmux
    optional_cmd just

    if command -v cargo >/dev/null 2>&1; then
      cargo --version
    fi
    if command -v rustc >/dev/null 2>&1; then
      rustc --version
    fi

    if [ "$missing_required" -ne 0 ]; then
      echo "One or more required tools are missing." >&2
      exit 1
    fi
    ;;
  check)
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    ;;
  fmt-check)
    cargo fmt --check
    ;;
  fmt)
    cargo fmt
    ;;
  clippy)
    cargo clippy --all-targets -- -D warnings
    ;;
  build)
    cargo build
    ;;
  build-release)
    cargo build --release
    ;;
  test)
    cargo test
    ;;
  test-one)
    shift
    if [ "$#" -eq 0 ]; then
      echo "Usage: bash scripts/dev.sh test-one <test-filter> [-- <cargo-test-args>]" >&2
      exit 2
    fi
    cargo test "$@"
    ;;
  run)
    cargo run
    ;;
  install-local)
    cargo install --path .
    ;;
  help|--help|-h)
    cat <<'EOF'
Usage: bash scripts/dev.sh <command>

Commands:
  ci             same checks used by GitHub Actions
  doctor         check required and optional local developer tools
  check          fmt check, clippy with warnings denied, and tests
  fmt-check      run cargo fmt --check
  fmt            run cargo fmt
  clippy         run cargo clippy --all-targets -- -D warnings
  build          run cargo build
  build-release  run cargo build --release
  test           run cargo test
  test-one       run cargo test with a filter
  run            run RightClick locally
  install-local  install RightClick from this checkout
EOF
    ;;
  *)
    echo "Unknown command: $cmd" >&2
    echo "Run: bash scripts/dev.sh help" >&2
    exit 2
    ;;
esac
