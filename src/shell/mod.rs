//! Shell module (Imperative Shell - I/O and side effects)
//!
//! This module contains all the impure code that handles I/O, external service
//! integrations, and orchestration. Following the Functional Core & Imperative Shell
//! architecture, this layer:
//!
//! - Calls into the functional core for business logic
//! - Handles all async I/O operations
//! - Manages external dependencies (git, filesystem, etc.)
//! - Orchestrates use cases by coordinating repositories and services
//!
//! # Module Structure
//!
//! - `usecases` - Use case orchestration (AppUsecase, etc.)
//! - `repositories` - Data access layer (ConfigRepository, StateRepository, etc.)
//! - `services` - External service integrations (GitService, etc.)
//!
//! # Architecture Rules
//!
//! 1. Shell calls Core. Core NEVER calls Shell.
//! 2. All I/O happens in the Shell layer
//! 3. Use cases orchestrate workflows by calling Core + repositories/services
//! 4. Repositories isolate data access (no business logic)
//! 5. Services handle external integrations

pub mod machines;
pub mod repositories;
pub mod services;
pub mod services_full;
pub mod usecases;

// Re-export commonly used types for convenience
pub use repositories::{ConfigRepository, FileConfigRepository, FileStateRepository, StateRepository};
pub use usecases::{AppUsecase, Project};

// Use the full git service implementation
pub use services_full::{GitService, CliGitService};
