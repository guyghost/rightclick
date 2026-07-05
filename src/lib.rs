//! RightClick - A TUI dashboard for AI coding agents
//!
//! This crate provides the core functionality for RightClick, organized
//! following the Functional Core & Imperative Shell architecture.

// Core modules (always available)
pub mod config;
pub mod core;
pub mod event;
pub mod state;
pub mod theme;

// Shell modules (always available)
pub mod shell;

// UI modules (always available)
pub mod keymap;
pub mod modal;
pub mod palette;
pub mod settings;
pub mod ui;

// Search system
pub mod search;

// Plugin system (always available)
pub mod plugin;

// Re-export commonly used types
pub use config::Config;
pub use core::models::*;
pub use event::{Dispatcher, Event, Topic};

// Adapter system for AI coding agents
pub mod adapters;

// Plugin implementations
pub mod plugins;

// TTY/Terminal integration
pub mod tty;

// Version checking
pub mod version;
