//! UI components for RightClick
//!
//! This module provides reusable UI components for building the TUI interface.
//! All components follow the ratatui widget pattern and integrate with the
//! theme system for consistent styling.
//!
//! # Module Structure
//!
//! - `components` - Basic UI components (Header, Footer, tabs)
//! - `hints` - Shared keyboard hint formatting
//! - `overlay` - Modal overlay for dialogs and popups
//! - `selection` - Text selection handling
//! - `scroll` - Scrollable view state management
//! - `spinner` - Loading spinner animation
//!
//! # Usage
//!
//! ```rust
//! use rightclick::ui::{Header, Footer, KeyHint, ScrollState, Spinner};
//! use ratatui::layout::Rect;
//! use ratatui::buffer::Buffer;
//!
//! // Create and render a header
//! let header = Header::new("My App")
//!     .with_subtitle("v1.0")
//!     .with_tabs(vec!["Tab 1", "Tab 2"], 0);
//!
//! // Create a footer with key hints
//! let footer = Footer::new("Ready")
//!     .with_hint("q", "Quit")
//!     .with_hint("h", "Help");
//! ```

mod components;
mod hints;
pub mod notifications;
mod overlay;
pub mod progress;
mod scroll;
mod selection;
mod spinner;
mod text;
pub mod text_input;

pub use components::{Footer, Header, KeyHint};
pub use hints::{
    GLOBAL_SEARCH_HINT, HELP_HINT, STACKED_GLOBAL_SEARCH_HINT, compact_global_hint_lines,
    compact_global_search_hint, compact_global_search_hint_with_stacked, compact_help_hint,
    compact_prefixed_stacked_global_hint_lines,
};
pub use notifications::{NotificationLevel, NotificationManager};
pub use overlay::Overlay;
pub use progress::ProgressBar;
pub use scroll::ScrollState;
pub use selection::Selection;
pub use spinner::Spinner;
pub use text::{clip_display, truncate_display, truncate_display_with_suffix};
pub use text_input::TextInputWidget;
