//! Conversation types for RightClick
//!
//! This module defines data structures for chat sessions, messages,
//! content blocks, tool uses, and related AI conversation entities.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A conversation session
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier (UUID)
    pub id: String,
    /// Display name for the session
    pub name: String,
    /// ID of the adapter used for this session
    pub adapter_id: String,
    /// Project associated with this session (optional)
    pub project_name: Option<String>,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session was last updated
    pub updated_at: DateTime<Utc>,
    /// Number of messages in the session
    pub message_count: usize,
    /// Total token usage (if tracked by adapter)
    pub total_tokens: Option<usize>,
    /// Whether this session is currently active/selected
    pub is_active: bool,
    /// Session tags for organization
    pub tags: Vec<String>,
    /// Custom metadata
    pub metadata: Option<serde_json::Value>,
}

impl Session {
    /// Create a new session with the given name and adapter
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        adapter_id: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            adapter_id: adapter_id.into(),
            project_name: None,
            created_at: now,
            updated_at: now,
            message_count: 0,
            total_tokens: None,
            is_active: false,
            tags: Vec::new(),
            metadata: None,
        }
    }

    /// Update the updated_at timestamp to now
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Add a tag to the session
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let tag = tag.into();
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    /// Remove a tag from the session
    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.retain(|t| t != tag);
    }
}

/// A message in a conversation
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier
    pub id: String,
    /// Role of the message sender
    pub role: Role,
    /// Content of the message
    pub content: String,
    /// Timestamp when the message was created
    pub timestamp: DateTime<Utc>,
    /// Model used to generate this message (for assistant messages)
    pub model: Option<String>,
    /// Tool uses in this message
    pub tool_uses: Vec<ToolUse>,
    /// Content blocks for rich content
    pub content_blocks: Vec<ContentBlock>,
    /// Token usage for this specific message
    pub tokens: Option<TokenUsage>,
    /// Whether the message is still being generated
    pub is_streaming: bool,
    /// Additional metadata
    pub metadata: Option<serde_json::Value>,
}

impl Message {
    /// Create a new user message
    pub fn user(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role: Role::User,
            content: content.into(),
            timestamp: Utc::now(),
            model: None,
            tool_uses: Vec::new(),
            content_blocks: Vec::new(),
            tokens: None,
            is_streaming: false,
            metadata: None,
        }
    }

    /// Create a new assistant message
    pub fn assistant(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role: Role::Assistant,
            content: content.into(),
            timestamp: Utc::now(),
            model: None,
            tool_uses: Vec::new(),
            content_blocks: Vec::new(),
            tokens: None,
            is_streaming: false,
            metadata: None,
        }
    }

    /// Create a new system message
    pub fn system(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role: Role::System,
            content: content.into(),
            timestamp: Utc::now(),
            model: None,
            tool_uses: Vec::new(),
            content_blocks: Vec::new(),
            tokens: None,
            is_streaming: false,
            metadata: None,
        }
    }

    /// Check if this message has tool uses
    pub fn has_tool_uses(&self) -> bool {
        !self.tool_uses.is_empty()
    }

    /// Check if this message has content blocks
    pub fn has_content_blocks(&self) -> bool {
        !self.content_blocks.is_empty()
    }
}

/// Role of a message sender
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Human user
    User,
    /// AI assistant
    Assistant,
    /// System message (instructions, context)
    System,
    /// Tool result
    Tool,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::System => write!(f, "system"),
            Role::Tool => write!(f, "tool"),
        }
    }
}

/// A tool use (function calling)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolUse {
    /// Unique identifier for this tool use
    pub id: String,
    /// Name of the tool being used
    pub name: String,
    /// Input/parameters as JSON string
    pub input: String,
    /// Output/result from the tool (None if pending)
    pub output: Option<String>,
    /// Whether the tool use was successful
    pub is_success: Option<bool>,
    /// Error message if the tool failed
    pub error: Option<String>,
    /// Timestamp when the tool was invoked
    pub invoked_at: DateTime<Utc>,
    /// Timestamp when the tool completed
    pub completed_at: Option<DateTime<Utc>>,
}

impl ToolUse {
    /// Create a new pending tool use
    pub fn new(id: impl Into<String>, name: impl Into<String>, input: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            input: input.into(),
            output: None,
            is_success: None,
            error: None,
            invoked_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Mark the tool use as completed successfully
    pub fn complete(&mut self, output: impl Into<String>) {
        self.output = Some(output.into());
        self.is_success = Some(true);
        self.completed_at = Some(Utc::now());
    }

    /// Mark the tool use as failed
    pub fn fail(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
        self.is_success = Some(false);
        self.completed_at = Some(Utc::now());
    }

    /// Check if the tool use is still pending
    pub fn is_pending(&self) -> bool {
        self.completed_at.is_none()
    }

    /// Check if the tool use completed successfully
    pub fn is_success(&self) -> bool {
        self.is_success == Some(true)
    }
}

/// A rich content block within a message
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ContentBlock {
    /// Plain text content
    Text {
        /// The text content
        content: String,
    },
    /// Code block with optional language
    Code {
        /// Programming language identifier
        language: Option<String>,
        /// The code content
        code: String,
        /// File path if this represents a file
        file_path: Option<String>,
    },
    /// Markdown formatted content
    Markdown {
        /// The markdown content
        content: String,
    },
    /// Image reference
    Image {
        /// Image URL or data URI
        source: String,
        /// Alt text description
        alt: Option<String>,
    },
    /// File reference
    File {
        /// File path
        path: String,
        /// File content (if embedded)
        content: Option<String>,
        /// Number of lines (if known)
        line_count: Option<usize>,
    },
    /// Diff/change content
    Diff {
        /// File path
        path: String,
        /// Diff content
        diff: String,
        /// Old content (if available)
        old_content: Option<String>,
        /// New content (if available)
        new_content: Option<String>,
    },
    /// Tool use request
    ToolUse {
        /// Tool use ID
        id: String,
        /// Tool name
        name: String,
        /// Input as string (usually JSON)
        input: String,
    },
    /// Tool result
    ToolResult {
        /// Tool use ID (matches the request)
        tool_use_id: String,
        /// Result content
        content: String,
        /// Whether successful
        is_error: bool,
    },
}

impl ContentBlock {
    /// Create a new text block
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text {
            content: content.into(),
        }
    }

    /// Create a new code block
    pub fn code(code: impl Into<String>) -> Self {
        Self::Code {
            language: None,
            code: code.into(),
            file_path: None,
        }
    }

    /// Create a new code block with language
    pub fn code_with_language(language: impl Into<String>, code: impl Into<String>) -> Self {
        Self::Code {
            language: Some(language.into()),
            code: code.into(),
            file_path: None,
        }
    }

    /// Create a new markdown block
    pub fn markdown(content: impl Into<String>) -> Self {
        Self::Markdown {
            content: content.into(),
        }
    }

    /// Get the text content if this is a text block
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { content } => Some(content),
            _ => None,
        }
    }

    /// Get the code content if this is a code block
    pub fn as_code(&self) -> Option<(&Option<String>, &str)> {
        match self {
            Self::Code { language, code, .. } => Some((language, code)),
            _ => None,
        }
    }

    /// Check if this block contains code
    pub fn is_code(&self) -> bool {
        matches!(self, Self::Code { .. })
    }

    /// Check if this block is text
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text { .. })
    }
}

/// Token usage information
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Number of tokens in the prompt
    pub prompt_tokens: usize,
    /// Number of tokens in the completion
    pub completion_tokens: usize,
    /// Total tokens used
    pub total_tokens: usize,
}

impl TokenUsage {
    /// Create new token usage
    pub fn new(prompt_tokens: usize, completion_tokens: usize) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }
}

/// A streaming response chunk
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StreamChunk {
    /// Chunk index in the stream
    pub index: usize,
    /// Content delta (new content since last chunk)
    pub delta: String,
    /// Whether this is the final chunk
    pub is_done: bool,
    /// Finish reason (if done)
    pub finish_reason: Option<FinishReason>,
    /// Token usage (usually only present in final chunk)
    pub usage: Option<TokenUsage>,
}

/// Reason why generation finished
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// Completed normally
    Stop,
    /// Hit token limit
    Length,
    /// Content filtered
    ContentFilter,
    /// Tool use requested
    ToolUse,
    /// Unknown/other reason
    Other,
}

/// Conversation context for building prompts
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConversationContext {
    /// System prompt/instructions
    pub system_prompt: Option<String>,
    /// Recent file context
    pub file_context: Vec<FileContext>,
    /// Git context
    pub git_context: Option<GitContext>,
    /// Project context
    pub project_context: Option<ProjectContext>,
}

/// File context for conversations
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileContext {
    /// File path
    pub path: String,
    /// File content
    pub content: String,
    /// Programming language
    pub language: Option<String>,
    /// Line range (start, end) if partial content
    pub line_range: Option<(usize, usize)>,
}

/// Git context for conversations
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GitContext {
    /// Current branch
    pub branch: String,
    /// Recent commits
    pub recent_commits: Vec<String>,
    /// Changed files
    pub changed_files: Vec<String>,
    /// Diff summary
    pub diff_summary: Option<String>,
}

/// Project context for conversations
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectContext {
    /// Project name
    pub name: String,
    /// Project path
    pub path: String,
    /// Project structure summary
    pub structure: Option<String>,
    /// Dependency information
    pub dependencies: Option<serde_json::Value>,
}

/// Errors that can occur in conversation operations
#[derive(Debug, thiserror::Error)]
pub enum ConversationError {
    /// Session not found
    #[error("session not found: {0}")]
    SessionNotFound(String),
    /// Message not found
    #[error("message not found: {0}")]
    MessageNotFound(String),
    /// Invalid role
    #[error("invalid role: {0}")]
    InvalidRole(String),
    /// Invalid content
    #[error("invalid content: {0}")]
    InvalidContent(String),
    /// Adapter error
    #[error("adapter error: {0}")]
    AdapterError(String),
    /// Token limit exceeded
    #[error("token limit exceeded: {0}")]
    TokenLimitExceeded(String),
    /// Streaming error
    #[error("streaming error: {0}")]
    StreamingError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new() {
        let session = Session::new("uuid-123", "Test Session", "claude");
        assert_eq!(session.id, "uuid-123");
        assert_eq!(session.name, "Test Session");
        assert_eq!(session.adapter_id, "claude");
        assert_eq!(session.message_count, 0);
        assert!(!session.is_active);
    }

    #[test]
    fn test_session_tags() {
        let mut session = Session::new("id", "name", "adapter");
        session.add_tag("work");
        session.add_tag("urgent");
        assert_eq!(session.tags.len(), 2);

        // Duplicate should be ignored
        session.add_tag("work");
        assert_eq!(session.tags.len(), 2);

        session.remove_tag("work");
        assert_eq!(session.tags.len(), 1);
        assert!(!session.tags.contains(&"work".to_string()));
    }

    #[test]
    fn test_message_user() {
        let msg = Message::user("msg-1", "Hello, world!");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello, world!");
        assert!(!msg.has_tool_uses());
    }

    #[test]
    fn test_message_assistant() {
        let msg = Message::assistant("msg-2", "Hello! How can I help?");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content, "Hello! How can I help?");
    }

    #[test]
    fn test_role_display() {
        assert_eq!(Role::User.to_string(), "user");
        assert_eq!(Role::Assistant.to_string(), "assistant");
        assert_eq!(Role::System.to_string(), "system");
    }

    #[test]
    fn test_tool_use_lifecycle() {
        let mut tool = ToolUse::new("tool-1", "read_file", r#"{"path": "main.rs"}"#);
        assert!(tool.is_pending());
        assert!(!tool.is_success());

        tool.complete("file content here");
        assert!(!tool.is_pending());
        assert!(tool.is_success());
        assert!(tool.output.is_some());
        assert!(tool.completed_at.is_some());

        let mut failed_tool = ToolUse::new("tool-2", "write_file", r#"{}"#);
        failed_tool.fail("permission denied");
        assert!(!failed_tool.is_success());
        assert!(failed_tool.error.is_some());
    }

    #[test]
    fn test_content_block_constructors() {
        let text = ContentBlock::text("Hello");
        assert!(text.is_text());
        assert_eq!(text.as_text(), Some("Hello"));

        let code = ContentBlock::code_with_language("rust", "fn main() {}");
        assert!(code.is_code());
        let (lang, content) = code.as_code().unwrap();
        assert_eq!(lang.as_ref().unwrap(), "rust");
        assert_eq!(content, "fn main() {}");

        let markdown = ContentBlock::markdown("# Title");
        assert!(!markdown.is_code());
    }

    #[test]
    fn test_token_usage() {
        let usage = TokenUsage::new(100, 50);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }
}
