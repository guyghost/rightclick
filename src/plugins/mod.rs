//! Plugins for RightClick
//!
//! This module provides plugin implementations that extend RightClick's
//! functionality with additional views and features.

pub mod conversations;
pub mod filebrowser;
pub mod gitstatus;
pub mod workers;
pub mod workspace;

// Re-export conversations plugin types
pub use conversations::{
    ConversationView, ConversationsAction, ConversationsPlugin, ConversationsPluginBuilder,
    ConversationsRenderer, ListNavigation, MessageScroll, PluginState as ConversationsPluginState,
    SessionInfo, builder, default_key_bindings, init, role_display_name, role_icon, role_style,
};

// Re-export filebrowser plugin types
pub use filebrowser::{
    FileBrowserPlugin, FileEntry, FileTree, PluginState as FileBrowserPluginState, Preview,
};

// Re-export gitstatus plugin types
pub use gitstatus::{
    Command as GitStatusCommand, FocusPane as GitStatusFocusPane,
    GitPluginContext as GitStatusPluginContext, GitStatusPlugin,
    PluginCommand as GitStatusPluginCommand, PluginState as GitStatusPluginState,
    ViewMode as GitStatusViewMode,
};

// Re-export workers plugin types
pub use workers::{
    Command as WorkersCommand, FocusPane as WorkersFocusPane, IntentEntry,
    ModalState as WorkersModalState, PluginCommand as WorkersPluginCommand,
    PluginState as WorkersPluginState, PreviewTab as WorkersPreviewTab,
    ViewMode as WorkersViewMode, WorkerEntry, WorkerRunner, WorkerRunnerError, WorkersPlugin,
    WorkersPluginContext,
};

// Re-export workspace plugin types (for backward compatibility)
pub use workspace::{
    AgentLauncher, Command as WorkspaceCommand, FocusPane as WorkspaceFocusPane, ModalState,
    PluginCommand as WorkspacePluginCommand, PluginState as WorkspacePluginState, PreviewTab,
    ShellSession, TmuxManager, ViewMode, WorkspacePlugin, WorkspacePluginContext, Worktree,
    WorktreeManager,
};
