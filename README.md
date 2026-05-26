# RightClick

You might never open your editor again.

**Status: In Development**

RightClick puts your entire development workflow in one shell: plan tasks, chat with AI agents, review diffs, stage commits, review past conversations, and manage workspaces—all without leaving RightClick.

## Overview

RightClick is a Rust port of [Sidecar](https://github.com/guyghost/sidecar), a TUI dashboard for AI coding agents. It provides:

- **Git Status**: View staged, modified, and untracked files with syntax-highlighted diffs
- **Conversations**: Browse AI session history from multiple agents (Claude, Cursor, Codex, etc.)
- **Task Monitor**: Integration with task management systems
- **File Browser**: Navigate project files with tree view and preview
- **Workspaces**: Manage git worktrees for parallel development

## Quick Start

```bash
# Build from source
cargo build --release

# Run from any project directory
./target/release/rightclick

# Or install locally
cargo install --path .
rightclick
```

## Developer Commands

RightClick keeps the common local checks in `scripts/dev.sh`, with optional
`justfile` shortcuts if you use `just`:

```bash
bash scripts/dev.sh ci            # same checks used by GitHub Actions
bash scripts/dev.sh check         # fmt check, clippy with warnings denied, and tests
bash scripts/dev.sh fmt-check
bash scripts/dev.sh clippy
bash scripts/dev.sh test
bash scripts/dev.sh run
bash scripts/dev.sh install-local
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q`, `ctrl+c` | Quit |
| `tab` / `shift+tab` | Navigate plugins |
| `1-9` | Focus plugin by number |
| `j/k`, `↓/↑` | Navigate items |
| `enter` | Select |
| `esc` | Back/close |
| `r` | Refresh |
| `?` | Toggle help |

## Architecture

RightClick follows the **Functional Core & Imperative Shell** pattern:

- **Core**: Pure business logic, no I/O, fully testable
- **Shell**: I/O orchestration, side effects, external integrations

See [AGENTS.md](./AGENTS.md) for development guidelines.

## License

MIT
