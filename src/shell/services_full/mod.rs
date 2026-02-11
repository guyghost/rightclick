//! External service integrations for RightClick.
//!
//! This module defines traits and implementations for external services like Git
//! operations. Services are the outermost layer that interacts with external systems.

mod git;

pub use git::{CliGitService, GitService};
