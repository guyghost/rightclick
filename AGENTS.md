# RightClick - Agent Context

RightClick is a TUI dashboard for AI coding agents. This document provides context for AI agents working on the codebase.

## Architecture: Functional Core & Imperative Shell

This project follows the **Functional Core & Imperative Shell** (FC&IS) architecture pattern:

```
src/
├── core/                    # Functional Core (pure, no I/O)
│   ├── models/              # Types, value objects, domain models
│   ├── logic/               # Pure functions for transformations
│   └── rules/               # Business rules and validation
│
├── shell/                   # Imperative Shell (I/O, side effects)
│   ├── usecases/            # Orchestrate Core + Shell
│   ├── repositories/        # Data access (files, DB, network)
│   ├── handlers/            # Entry points (CLI, TUI events)
│   ├── services/            # External integrations (git, tmux)
│   └── machines/            # State machines for complex flows
│
├── adapters/                # AI agent adapters (Claude, Cursor, etc.)
├── plugins/                 # TUI plugins
├── ui/                      # Reusable UI components
├── config/                  # Configuration management
├── state/                   # Persistent state
├── event/                   # Event bus
├── theme/                   # Theme resolution
├── keymap/                  # Keyboard shortcuts
├── modal/                   # Modal system
├── palette/                 # Command palette
├── tty/                     # Terminal/tmux integration
└── version/                 # Version checking
```

### Core Principles

**Rule: Shell calls Core. Core NEVER calls Shell. Core IGNORES Shell exists.**

- **Core**: Pure functions, no async, no I/O, no side effects
- **Shell**: Manages I/O, orchestrates workflows, handles side effects

## Build Commands

```bash
# Build
cargo build --release

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run

# Install locally
cargo install --path .
```

## Key Patterns

### Result Types

Use `anyhow::Result` in Shell, custom error types in Core:

```rust
// Core: specific error types
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("invalid field: {0}")]
    InvalidField(String),
}

// Shell: anyhow for flexibility
use anyhow::Result;
```

### Plugin Architecture

Plugins implement the `Plugin` trait:

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn icon(&self) -> char;
    
    async fn init(&mut self, ctx: &PluginContext) -> Result<()>;
    fn handle_event(&mut self, event: Event) -> Vec<Command>;
    fn render(&self, area: Rect, buf: &mut Buffer);
}
```

### Event System

Events flow through the event bus:

```rust
// Publish
event_bus.publish(Event::FileChanged { path });

// Subscribe
let mut rx = event_bus.subscribe(Topic::FileChanges);
while let Ok(event) = rx.recv().await {
    // Handle event
}
```

## Naming Conventions

- **Files**: `snake_case.rs` for modules
- **Types**: `PascalCase` for structs/enums, `SCREAMING_SNAKE_CASE` for constants
- **Functions**: `snake_case` for methods/functions
- **Modules**: `snake_case` for module names

## Testing

- Core: Unit tests in same file, no mocks needed (pure functions)
- Shell: Integration tests in `tests/` directory, use mocks for external deps

## Documentation

- All public APIs must have doc comments
- Use `///` for item documentation
- Use `//!` for module-level documentation
