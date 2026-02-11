//! Core domain models for RightClick
//!
//! This module contains all pure data structures used throughout the application.
//! These types have no I/O, no async, and no side effects - they are simple
//! value objects and entities that form the functional core of the application.
//!
//! # Module Structure
//!
//! - `config` - Configuration types (Config, ProjectConfig, UIConfig, etc.)
//! - `theme` - Theme types (ColorPalette, Theme, TokenColors, etc.)
//! - `git` - Git types (FileStatus, Diff, Commit, Branch, etc.)
//! - `conversation` - Conversation types (Session, Message, ContentBlock, etc.)

pub mod config;
pub mod conversation;
pub mod git;
pub mod theme;

// Re-export commonly used types for convenience
pub use config::{
    Config, ConversationsPluginConfig, FileBrowserPluginConfig, GitStatusPluginConfig,
    KeymapConfig, PluginsConfig, ProjectConfig, ProjectsConfig, UIConfig, WorkspacePluginConfig,
    CURRENT_CONFIG_VERSION,
};
pub use conversation::{
    ContentBlock, ConversationContext, ConversationError, FileContext, FinishReason, GitContext,
    Message, ProjectContext, Role, Session, StreamChunk, TokenUsage, ToolUse,
};
pub use git::{
    Blame, BlameLine, Branch, ChangeType, Commit, Diff, DiffHunk, DiffLine, FileChange, FileDiff,
    FileStatus, GitError, Remote, RepoState, RepoStatus, Stash,
};
pub use theme::{ColorPalette, Theme, ThemeError, ThemeRef, TokenColors, UiColors};

/// Common error types for core operations
pub mod errors {
    pub use super::conversation::ConversationError;
    pub use super::git::GitError;
    pub use super::theme::ThemeError;
}

/// Prelude module for convenient imports
pub mod prelude {
    pub use super::{
        Blame, BlameLine, Branch, ChangeType, ColorPalette, Commit, Config, ContentBlock,
        ConversationContext, ConversationError, ConversationsPluginConfig, Diff, DiffHunk,
        DiffLine, FileBrowserPluginConfig, FileChange, FileContext, FileDiff, FileStatus,
        FinishReason, GitContext, GitError, GitStatusPluginConfig, KeymapConfig, Message,
        PluginsConfig, ProjectConfig, ProjectContext, ProjectsConfig, Remote, RepoState,
        RepoStatus, Role, Session, Stash, StreamChunk, Theme, ThemeError, ThemeRef, TokenColors,
        TokenUsage, ToolUse, UIConfig, UiColors, WorkspacePluginConfig, CURRENT_CONFIG_VERSION,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_re_exports_compile() {
        // This test ensures all re-exported types are valid
        let _: Config = Config::default();
        let _: Theme = Theme::default();
        let _: ColorPalette = ColorPalette::default();
        let _: UIConfig = UIConfig::default();
        let _: ProjectConfig = ProjectConfig {
            id: "test".to_string(),
            name: "Test".to_string(),
            path: "/tmp".to_string(),
            description: None,
            favorite: false,
            tags: vec![],
        };
        let _: Session = Session::new("id", "name", "adapter");
        let _: Message = Message::user("id", "content");
        let _: FileStatus = FileStatus::Modified;
        let _: Role = Role::User;
    }
}
