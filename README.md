# RightClick

You might never open your editor again.

**Status: In Development**

RightClick puts your entire development workflow in one shell: plan tasks, chat with AI agents, review diffs, stage commits, review past conversations, and manage workspaces—all without leaving RightClick.

## Overview

RightClick is a Rust port of [Sidecar](https://github.com/guyghost/sidecar), a TUI dashboard for AI coding agents. It provides:

- **Git Status**: View staged, modified, and untracked files with syntax-highlighted diffs
- **Conversations**: Browse AI session history from multiple agents (Claude, Cursor, Codex, etc.)
- **Workers**: Manage intent-based AI worker workflows and inspect their output
- **File Browser**: Navigate project files with tree view and preview
- **Workspaces**: Manage git worktrees for parallel development
- **Global Search**: Search files, commands, sessions, worktrees, and intents from one overlay

## Quick Start

```bash
# Build from source
bash scripts/dev.sh build-release

# Run from any project directory
./target/release/rightclick

# Or install locally
bash scripts/dev.sh install-local
rightclick
```

## Developer Commands

RightClick keeps the common local checks in `scripts/dev.sh`, with optional
`justfile` shortcuts if you use `just`. Run `doctor` first on a new machine: it
checks Rust, Cargo, the required `rust-version`, `rustfmt`, `clippy`, Git, and
`rg`, plus optional tools like `tmux`, `td`, and `just`. The script can be launched
from any directory inside the checkout. When `td` is installed, `doctor` also
reports whether this checkout has task tracking initialized. Missing optional
tools include a short hint explaining what workflow they unlock and, for common
tools, the install command to use. If `td` is present but this checkout is not
initialized yet, `doctor` prints the repo path where `td init` should be run.

```bash
bash scripts/dev.sh ci            # same checks used by GitHub Actions
bash scripts/dev.sh pre-commit    # quick checks before committing
bash scripts/dev.sh pre-push      # full local verification before pushing
bash scripts/dev.sh doctor        # check required and optional local tools
bash scripts/dev.sh rust-version  # print the required Rust version
bash scripts/dev.sh check         # diff check, fmt check, clippy with warnings denied, and tests
bash scripts/dev.sh quick         # diff check, fmt check, and clippy without tests
bash scripts/dev.sh script-check  # validate shell helper and justfile syntax when available
bash scripts/dev.sh diff-check    # git whitespace checks for staged and unstaged changes
bash scripts/dev.sh fmt-check
bash scripts/dev.sh fmt
bash scripts/dev.sh clippy
bash scripts/dev.sh lint
bash scripts/dev.sh build
bash scripts/dev.sh build-release
bash scripts/dev.sh test
bash scripts/dev.sh doc-test
bash scripts/dev.sh test-list gitstatus search::overlay
bash scripts/dev.sh test-one plugins::gitstatus
bash scripts/dev.sh test-one test_plugin_commands -- --nocapture
bash scripts/dev.sh test-many test_plugin_commands test_key_hints
bash scripts/dev.sh test-many test_plugin_commands test_key_hints -- --nocapture
bash scripts/dev.sh run
bash scripts/dev.sh run -- --project ~/Developer/OSS/rightclick --debug
bash scripts/dev.sh install-local
bash scripts/dev.sh install-local --locked
```

If you prefer `just`, run `just help` for the same command overview. The default
`just` recipe also opens that help instead of a terse recipe list.

`test-one`, `test-many`, and `test-list` pass filters through to Cargo as
substring filters. If a filter does not match any test, the script prints the
matching `test-list` command plus broader token searches to help refine the
filter. `test-list` only accepts filters; pass Cargo test args to `test-one` or
`test-many` after `--`. Use `test-many` when you want to check several filters in one command;
Cargo itself accepts only one substring filter per `cargo test` invocation.
`test-one` and `test-many` print `validate test filter` before running Cargo so
long filter checks are visible.

`run` forwards arguments after `--` to the RightClick binary. `install-local`
forwards extra arguments to `cargo install`, so flags like `--locked` or
`--force` work through the helper script.

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q`, `Ctrl+C` | Quit |
| `Tab` / `Shift+Tab` | Navigate plugins or panes |
| `1-9` | Focus plugin by number |
| `/` | Search files, commands, sessions, worktrees, and intents |
| `:` | Command search |
| `j/k`, `↓/↑` | Navigate items |
| `Enter` | Select |
| `Esc` | Back/close |
| `r`, `Ctrl+R` | Refresh current view |
| `?` | Toggle help |

In pane-based views such as Git Status, Workspace, and Workers, `Tab` and
`Shift+Tab` move between panes. Use `Ctrl+Tab` or `Ctrl+Shift+Tab` there when
you want to move between plugins instead.

## Search

Press `/` to open global search, or `:` to open command search directly. Use
`Tab` or `Shift+Tab` inside the overlay to switch scope:

- **All**: search files, commands, sessions, worktrees, and intents together
- **Files**: search file contents with `rg`
- **Project**: search sessions, worktrees, and intents exposed by plugins
- **Commands**: search commands by name, description, shortcut, category, or command ID

Selecting an item opens the owning plugin and focuses the matching session,
worktree, intent, or file result when the plugin supports it.

## Architecture

RightClick follows the **Functional Core & Imperative Shell** pattern:

- **Core**: Pure business logic, no I/O, fully testable
- **Shell**: I/O orchestration, side effects, external integrations

See [AGENTS.md](./AGENTS.md) for development guidelines.

## License

MIT
