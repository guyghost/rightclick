# RightClick - Project Status

## Overview
RightClick is a Rust port of the Sidecar project - a TUI dashboard for AI coding agents. The project follows the **Functional Core & Imperative Shell** architecture pattern.

## Current Status
✅ **Compiling & Working** - The project builds successfully with `cargo build --release`

```bash
$ ./target/release/rightclick --version
rightclick 0.1.0
```

## Project Structure

### Core Architecture (FC&IS)
```
src/
├── core/           # Functional Core - pure business logic
│   ├── models/     # Domain types (Config, Theme, Git, Conversations)
│   ├── logic/      # Pure functions
│   └── rules/      # Business rules
├── shell/          # Imperative Shell - I/O and side effects
│   ├── usecases/   # Orchestration
│   ├── repositories/ # Data access
│   └── services/   # External integrations (Git CLI)
├── adapters/       # AI agent adapters (Claude, Cursor, Codex)
├── plugins/        # TUI plugins (Git, Files, Conversations, Tasks, Workspaces)
├── ui/             # Reusable UI components
├── config/         # Configuration management
├── state/          # Persistent state
├── event/          # Event bus
├── theme/          # Theme system
├── keymap/         # Keyboard shortcuts
├── modal/          # Modal system
├── palette/        # Command palette
├── tty/            # Terminal/tmux integration
└── version/        # Version checking
```

## Implemented Features

### ✅ Working Components

1. **Configuration System** (`src/config/`)
   - Load/save config from JSON
   - Default configuration
   - Path resolution

2. **State Management** (`src/state/`)
   - Persistent state with file storage
   - Thread-safe access with RwLock
   - Per-workdir state tracking

3. **Event Bus** (`src/event/`)
   - Pub/sub pattern with topics
   - Thread-safe dispatcher
   - Subscription management

4. **Theme System** (`src/theme/`)
   - 4 built-in themes (default, dracula, nord, tokyo-night)
   - Theme resolution from config/project
   - Style generation for ratatui
   - Color palette management

5. **UI Components** (`src/ui/`)
   - Header, Footer components
   - Modal overlay system
   - Scroll state management
   - Text selection
   - Loading spinners

6. **Modal System** (`src/modal/`)
   - Modal widget with focus management
   - Section-based content
   - Button, checkbox components

7. **Command Palette** (`src/palette/`)
   - Fuzzy matching
   - Entry filtering
   - Keyboard navigation

8. **Keymap System** (`src/keymap/`)
   - Key binding registry
   - Focus contexts
   - Command definitions

9. **Plugin System** (`src/plugin/`)
   - Plugin trait definition
   - Plugin registry
   - Context for initialization

10. **AI Adapters** (`src/adapters/`)
    - **Claude Code adapter** - Reads ~/.claude/projects/
    - **Cursor adapter** - Reads ~/.cursor/chats/ (SQLite)
    - **Codex adapter** - Reads ~/.codex/sessions/
    - Adapter registry with auto-detection

11. **Git Service** (`src/shell/services/`)
    - Git operations via CLI (no native git dependency)
    - Status, diff, commits
    - Stage/unstage/commit

12. **Plugins** (`src/plugins/`)
    - **Git Status** - Full implementation with diff view, state machine navigation, action guards
    - **File Browser** - Tree view with syntax highlighting
    - **Conversations** - AI conversation browser
    - **Workers** - Intent/task management and worker tracking
    - **Workspaces** - Git worktree management

13. **TTY Integration** (`src/tty/`)
    - Tmux session management
    - Interactive mode
    - Output polling
    - Key forwarding

14. **Main Application** (`src/main.rs`)
    - Terminal setup with crossterm
    - Event loop
    - Tab-based UI
    - Keyboard handling

## Usage

```bash
# Build release version
cargo build --release

# Run
./target/release/rightclick

# With options
./target/release/rightclick --project /path/to/project --debug

# Help
./target/release/rightclick --help
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Tab` | Next tab |
| `Shift+Tab` | Previous tab |
| `1-5` | Jump to tab |

## Architecture Decisions

- **Functional Core**: Business logic is pure and testable
- **Imperative Shell**: I/O is isolated in the shell layer
- **Event-Driven**: Plugins communicate via events
- **Trait-Based**: Extensible through traits (Plugin, Adapter, GitService)
- **Async**: Uses tokio for async runtime
- **No Native Git Dep**: Uses CLI git for portability

## Dependencies

Key crates used:
- `tokio` - Async runtime
- `ratatui` + `crossterm` - TUI framework
- `serde` + `serde_json` - Serialization
- `anyhow` - Error handling
- `clap` - CLI parsing
- `chrono` - Date/time handling
- `rusqlite` - SQLite for TD/Cursor integration
- `parking_lot` - Synchronization primitives
- `nucleo` - Fuzzy matching

## Stats

- **111 Rust source files**
- **~61,500 lines of code**
- **5 active plugins**
- **1,500+ tests passing** (state manager unit tests plus plugin, adapter, search, and integration coverage)
- **8 AI adapters implemented** (Claude Code, Cursor, Codex, Gemini, Warp, Amp, Kiro, OpenCode)
- **4 built-in themes**

## Next Steps (Future Enhancements)

1. **Enhanced UI**: More visual polish, animations
2. **Plugin Configuration**: Per-plugin settings in config file
3. **More AI Adapters**: Additional sources beyond the eight already supported

## Credits

Ported from the original [Sidecar](https://github.com/guyghost/sidecar) project by Marcus.

## License

MIT
