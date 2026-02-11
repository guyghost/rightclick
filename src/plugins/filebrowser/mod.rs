//! File Browser Plugin for RightClick
//!
//! This plugin provides a file browser interface with a collapsible directory tree,
//! file preview with syntax highlighting, and quick navigation.
//!
//! # Features
//!
//! - Collapsible directory tree view
//! - File preview with syntax highlighting via syntect
//! - Quick file navigation (j/k)
//! - Expand/collapse directories (enter/space)
//! - Toggle git-ignored files visibility ('i')
//! - Show file info with 'I'
//!
//! # Layout
//!
//! The file browser uses a two-pane layout:
//! - File tree (30%): Shows the directory structure with expand/collapse indicators
//! - Preview (70%): Shows syntax-highlighted file content or binary/image indicators
//!
//! # Example
//!
//! ```rust
//! use rightclick::plugins::filebrowser::FileBrowserPlugin;
//! use std::path::PathBuf;
//!
//! let plugin = FileBrowserPlugin::new(PathBuf::from("."));
//! ```

mod plugin;
mod preview;
mod state;
mod tree;

pub use plugin::FileBrowserPlugin;
pub use preview::Preview;
pub use state::PluginState;
pub use tree::{FileEntry, FileTree};
