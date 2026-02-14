//! Plugins for RightClick
//!
//! This module provides plugin implementations that extend RightClick's
//! functionality with additional views and features.

pub mod conversations;
pub mod filebrowser;
pub mod gitstatus;
pub mod tdmonitor;
pub mod workers;
pub mod workspace;

// Re-export conversations plugin types
pub use conversations::{
    ConversationsAction, ConversationsPlugin, ConversationsPluginBuilder,
    ConversationsRenderer, ConversationView, ListNavigation, MessageScroll,
    PluginState as ConversationsPluginState, SessionInfo, builder, default_key_bindings, init,
    role_display_name, role_icon, role_style,
};

// Re-export filebrowser plugin types
pub use filebrowser::{
    FileBrowserPlugin, FileEntry, FileTree, PluginState as FileBrowserPluginState, Preview,
};

// Re-export gitstatus plugin types
pub use gitstatus::{
    Command as GitStatusCommand, FocusPane as GitStatusFocusPane, GitStatusPlugin,
    PluginCommand as GitStatusPluginCommand, GitPluginContext as GitStatusPluginContext,
    PluginState as GitStatusPluginState, ViewMode as GitStatusViewMode,
};

// Re-export tdmonitor plugin types
pub use tdmonitor::{
    ActivityLogEntry, Command as TDMonitorCommand, PluginCommand as TDMonitorPluginCommand,
    TDMonitorPluginContext, PluginState as TDMonitorPluginState, Priority, Task,
    TaskStatus, TDMonitorPlugin, ViewMode as TDMonitorViewMode,
};

// Re-export workers plugin types
pub use workers::{
    Command as WorkersCommand, FocusPane as WorkersFocusPane, IntentEntry, ModalState as WorkersModalState,
    PluginCommand as WorkersPluginCommand, WorkersPluginContext,
    PluginState as WorkersPluginState, PreviewTab as WorkersPreviewTab, ViewMode as WorkersViewMode,
    WorkerEntry, WorkerRunner, WorkerRunnerError, WorkersPlugin,
};

// Re-export workspace plugin types (for backward compatibility)
pub use workspace::{
    AgentLauncher, Command as WorkspaceCommand, FocusPane as WorkspaceFocusPane, ModalState,
    PluginCommand as WorkspacePluginCommand, WorkspacePluginContext,
    PluginState as WorkspacePluginState, PreviewTab, ShellSession, TmuxManager, ViewMode,
    Worktree, WorktreeManager, WorkspacePlugin,
};
