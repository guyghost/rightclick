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
checks Rust, Cargo, `rustfmt`, `clippy`, Git, and `rg`, plus optional tools like
`tmux` and `just`.

```bash
bash scripts/dev.sh ci            # same checks used by GitHub Actions
bash scripts/dev.sh doctor        # check required and optional local tools
bash scripts/dev.sh check         # fmt check, clippy with warnings denied, and tests
bash scripts/dev.sh fmt-check
bash scripts/dev.sh fmt
bash scripts/dev.sh clippy
bash scripts/dev.sh build
bash scripts/dev.sh build-release
bash scripts/dev.sh test
bash scripts/dev.sh test-one plugins::gitstatus
bash scripts/dev.sh run
bash scripts/dev.sh install-local
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q`, `ctrl+c` | Quit |
| `tab` / `shift+tab` | Navigate plugins |
| `1-9` | Focus plugin by number |
| `/` | Search files, commands, sessions, worktrees, and intents |
| `j/k`, `↓/↑` | Navigate items |
| `enter` | Select |
| `esc` | Back/close |
| `r` | Refresh |
| `?` | Toggle help |

## Search

Press `/` to open global search. Use `tab` inside the overlay to switch scope:

- **All**: search files, plugin-owned items, and commands together
- **Files**: search file contents with `rg`
- **Items**: search sessions, worktrees, and intents exposed by plugins
- **Commands**: search plugin commands with their current descriptions

Selecting an item opens the owning plugin and focuses the matching session,
worktree, intent, or file result when the plugin supports it.

## Architecture

RightClick follows the **Functional Core & Imperative Shell** pattern:

- **Core**: Pure business logic, no I/O, fully testable
- **Shell**: I/O orchestration, side effects, external integrations

See [AGENTS.md](./AGENTS.md) for development guidelines.

## License

MIT
