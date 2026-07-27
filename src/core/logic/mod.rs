//! Core Logic - Pure functions for state management
//!
//! This module contains pure business logic functions that operate on
//! the core models. These functions have no side effects and are
//! fully deterministic.

pub mod guards;
pub mod intent;
pub mod navigation;
pub mod project;

// Re-export commonly used types for convenience
pub use guards::check_guard;
pub use navigation::{apply_navigation, calculate_navigation};

// Re-export project logic
pub use project::{
    ProjectPathError, build_project_config, extract_project_name, find_existing_project,
    validate_project_path,
};

// Re-export intent functions and types
pub use intent::{
    IntentParseError, build_intent_from_spec, extract_acceptance_criteria,
    extract_context_metadata, extract_description, extract_title, generate_default_spec,
    generate_spec_document, parse_spec_document, update_criteria_in_content, validate_spec,
};

/// Prelude module for convenient imports
pub mod prelude {
    pub use super::{apply_navigation, calculate_navigation, check_guard};
}
