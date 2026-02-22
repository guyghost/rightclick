//! TTY/Terminal integration module for RightClick.
//!
//! This module provides functionality for interacting with terminal sessions,
//! specifically through tmux integration. It allows RightClick to:
//!
//! - Create and manage interactive terminal sessions
//! - Send keystrokes to tmux panes
//! - Capture and buffer output from terminal sessions
//! - Handle resize operations
//! - Detect exit sequences (double escape)
//!
//! # Architecture
//!
//! The module follows the Functional Core & Imperative Shell pattern:
//!
//! - **Functional Core**: [`OutputBuffer`] in `buffer.rs` - pure logic for
//!   managing terminal output without I/O
//! - **Functional Core**: [`Poller`] in `polling.rs` - pure timing logic
//! - **Imperative Shell**: [`Session`] in `session.rs` - I/O operations via tmux
//!
//! # Module Structure
//!
//! ```text
//! tty/
//! ├── mod.rs      # Module exports and documentation
//! ├── buffer.rs   # OutputBuffer for terminal output management
//! ├── polling.rs  # Poller for output polling timing
//! └── session.rs  # Session for tmux integration
//! ```
//!
//! # Usage Example
//!
//! ```rust,ignore
//! use rightclick::tty::{Session, OutputBuffer, Poller};
//! use crossterm::event::{KeyCode, KeyEvent};
//!
//! // Create a new session
//! let mut session = Session::new("my_session", "%0");
//!
//! // Resize the pane
//! session.resize(100, 30)?;
//!
//! // Enter interactive mode
//! session.enter()?;
//!
//! // Send some input
//! session.send_text("ls -la")?;
//! session.send_key(KeyEvent::from(KeyCode::Enter))?;
//!
//! // Poll for output
//! let output = session.poll_output()?;
//! println!("Captured: {}", output);
//!
//! // Exit when done
//! session.exit()?;
//! ```
//!
//! # Tmux Integration
//!
//! This module requires tmux to be installed and available in PATH. The
//! following tmux commands are used:
//!
//! - `tmux capture-pane -t {target} -p` - Capture pane output
//! - `tmux resize-pane -t {target} -x {width} -y {height}` - Resize pane
//! - `tmux send-keys -t {target} {keys}` - Send keystrokes
//! - `tmux list-panes -t {session}` - List available panes
//!
//! # Key Concepts
//!
//! ## Session
//!
//! A [`Session`] represents a connection to a specific tmux pane. It manages:
//! - Active/inactive state
//! - Pane and session identifiers
//! - Terminal dimensions
//! - Output buffering
//! - Double-escape detection for exit
//!
//! ## OutputBuffer
//!
//! An [`OutputBuffer`] stores captured terminal output with:
//! - Line-based storage
//! - Cursor position tracking
//! - Configurable scrollback limit
//! - Efficient memory management
//!
//! ## Poller
//!
//! A [`Poller`] manages the timing of output capture:
//! - Configurable polling interval (default 50ms)
//! - Prevents excessive CPU usage
//! - Adaptive backoff for idle sessions

pub mod buffer;
pub mod polling;
pub mod session;

// Re-export commonly used types
pub use buffer::{DEFAULT_SCROLLBACK, OutputBuffer};
pub use polling::{AdaptivePoller, DEFAULT_POLL_INTERVAL_MS, Poller};
pub use session::{
    DOUBLE_ESCAPE_WINDOW, Session, create_pane, execute_tmux, kill_pane, list_panes,
};

use std::process::Command;

/// Checks if tmux is installed and available.
///
/// This function attempts to run `tmux -V` to verify that tmux
/// is installed and accessible in the system PATH.
///
/// # Returns
///
/// `true` if tmux is available, `false` otherwise
///
/// # Examples
///
/// ```
/// use rightclick::tty;
///
/// let available = tty::is_tmux_available();
/// // Result depends on whether tmux is installed
/// ```
pub fn is_tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Gets the tmux version string.
///
/// Attempts to parse the version from `tmux -V` output.
///
/// # Returns
///
/// Some(version_string) if tmux is available, None otherwise
///
/// # Examples
///
/// ```
/// use rightclick::tty;
///
/// if let Some(version) = tty::get_tmux_version() {
///     println!("Tmux version: {}", version);
/// }
/// ```
pub fn get_tmux_version() -> Option<String> {
    let output = Command::new("tmux").arg("-V").output().ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}

/// Checks if running inside a tmux session.
///
/// This checks for the presence of the `TMUX` environment variable,
/// which is set by tmux when running inside a session.
///
/// # Returns
///
/// `true` if running inside tmux, `false` otherwise
///
/// # Examples
///
/// ```
/// use rightclick::tty;
///
/// let inside = tty::is_inside_tmux();
/// println!("Inside tmux: {}", inside);
/// ```
pub fn is_inside_tmux() -> bool {
    std::env::var("TMUX").is_ok()
}

/// Gets the current tmux session name.
///
/// Returns the name of the current tmux session if running inside one.
///
/// # Returns
///
/// Some(session_name) if inside tmux, None otherwise
///
/// # Examples
///
/// ```
/// use rightclick::tty;
///
/// if let Some(session) = tty::current_session() {
///     println!("Current session: {}", session);
/// }
/// ```
pub fn current_session() -> Option<String> {
    if !is_inside_tmux() {
        return None;
    }

    let output = Command::new("tmux")
        .args(["display-message", "-p", "#S"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}

/// Gets the current tmux pane ID.
///
/// Returns the ID of the current tmux pane if running inside one.
///
/// # Returns
///
/// Some(pane_id) if inside tmux, None otherwise (e.g., "%0")
///
/// # Examples
///
/// ```
/// use rightclick::tty;
///
/// if let Some(pane) = tty::current_pane() {
///     println!("Current pane: {}", pane);
/// }
/// ```
pub fn current_pane() -> Option<String> {
    if !is_inside_tmux() {
        return None;
    }

    let output = Command::new("tmux")
        .args(["display-message", "-p", "#D"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}

/// Error types for TTY operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TtyError {
    /// The session is not active.
    SessionInactive,
    /// The session is already active.
    SessionAlreadyActive,
    /// Tmux command failed.
    TmuxCommandFailed(String),
    /// Tmux is not available.
    TmuxNotAvailable,
    /// Invalid pane or session.
    InvalidTarget(String),
}

impl std::fmt::Display for TtyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SessionInactive => write!(f, "TTY session is not active"),
            Self::SessionAlreadyActive => write!(f, "TTY session is already active"),
            Self::TmuxCommandFailed(msg) => write!(f, "tmux command failed: {}", msg),
            Self::TmuxNotAvailable => write!(f, "tmux is not installed or not in PATH"),
            Self::InvalidTarget(target) => write!(f, "invalid tmux target: {}", target),
        }
    }
}

impl std::error::Error for TtyError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tty_error_display() {
        assert_eq!(
            TtyError::SessionInactive.to_string(),
            "TTY session is not active"
        );
        assert_eq!(
            TtyError::SessionAlreadyActive.to_string(),
            "TTY session is already active"
        );
        assert_eq!(
            TtyError::TmuxCommandFailed("test error".to_string()).to_string(),
            "tmux command failed: test error"
        );
        assert_eq!(
            TtyError::TmuxNotAvailable.to_string(),
            "tmux is not installed or not in PATH"
        );
        assert_eq!(
            TtyError::InvalidTarget("bad:target".to_string()).to_string(),
            "invalid tmux target: bad:target"
        );
    }

    #[test]
    fn tty_error_implements_error() {
        let err: Box<dyn std::error::Error> = Box::new(TtyError::SessionInactive);
        assert_eq!(err.to_string(), "TTY session is not active");
    }

    #[test]
    fn is_inside_tmux_checks_env() {
        // Test depends on environment, just verify it doesn't panic
        let _ = is_inside_tmux();
    }
}
