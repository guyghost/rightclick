//! UI components for RightClick
//!
//! This module provides reusable UI components for building the TUI interface.
//! All components follow the ratatui widget pattern and integrate with the
//! theme system for consistent styling.
//!
//! # Module Structure
//!
//! - `components` - Basic UI components (Header, Footer, tabs)
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
pub mod notifications;
mod overlay;
pub mod progress;
mod scroll;
mod selection;
mod spinner;
pub mod text_input;

pub use components::{Footer, Header, KeyHint};
pub use notifications::{NotificationLevel, NotificationManager};
pub use overlay::Overlay;
pub use progress::ProgressBar;
pub use scroll::ScrollState;
pub use selection::Selection;
pub use spinner::Spinner;
pub use text_input::TextInputWidget;
