//! Core Logic - Pure functions for state management
//!
//! This module contains pure business logic functions that operate on
//! the core models. These functions have no side effects and are
//! fully deterministic.

pub mod guards;
pub mod navigation;

// Re-export commonly used types for convenience
pub use guards::check_guard;
pub use navigation::{apply_navigation, calculate_navigation};

/// Prelude module for convenient imports
pub mod prelude {
    pub use super::{apply_navigation, calculate_navigation, check_guard};
}
