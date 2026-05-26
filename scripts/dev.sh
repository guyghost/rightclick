#!/usr/bin/env bash
set -euo pipefail

cmd="${1:-help}"

case "$cmd" in
  check)
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    ;;
  fmt-check)
    cargo fmt --check
    ;;
  clippy)
    cargo clippy --all-targets -- -D warnings
    ;;
  test)
    cargo test
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
  check          fmt check, clippy with warnings denied, and tests
  fmt-check      run cargo fmt --check
  clippy         run cargo clippy --all-targets -- -D warnings
  test           run cargo test
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
