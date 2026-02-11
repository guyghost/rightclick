//! TTY session management for tmux integration.
//!
//! This module provides the [`Session`] struct for managing an interactive
//! TTY session within a tmux pane. It handles:
//!
//! - Entering/exiting interactive mode
//! - Sending keystrokes to the pane
//! - Capturing and buffering output
//! - Pane resizing
//! - Double-escape detection for exit
//!
//! # Architecture
//!
//! The session is part of the Imperative Shell - it performs I/O operations
//! by executing tmux commands. The pure logic (buffer management) is delegated
//! to [`OutputBuffer`] in the `buffer` module.

use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::buffer::{OutputBuffer, DEFAULT_SCROLLBACK};
use super::polling::Poller;

/// Time window for detecting double escape press.
pub const DOUBLE_ESCAPE_WINDOW: Duration = Duration::from_millis(300);

/// Manages an interactive TTY session with a tmux pane.
///
/// A session represents a connection to a specific tmux pane, allowing
/// the application to send input and receive output. The session maintains
/// an output buffer for captured content and tracks session state.
///
/// # Example
///
/// ```rust,ignore
/// use rightclick::tty::Session;
///
/// let mut session = Session::new("my_session", "%0");
/// session.resize(80, 24).expect("Failed to resize");
/// session.enter().expect("Failed to enter interactive mode");
///
/// // In a real application, you'd poll for output
/// let output = session.poll_output().expect("Failed to poll");
/// println!("Captured {} lines", output.lines().count());
///
/// session.exit().expect("Failed to exit");
/// ```
#[derive(Debug)]
pub struct Session {
    /// Whether the session is currently active.
    pub active: bool,

    /// The tmux pane ID (e.g., "%12").
    pub target_pane: String,

    /// The tmux session name.
    pub target_session: String,

    /// The terminal width in columns.
    pub width: u16,

    /// The terminal height in rows.
    pub height: u16,

    /// Buffer for captured output.
    pub output_buffer: OutputBuffer,

    /// Timestamp of when escape was last pressed (for double-escape detection).
    pub escape_pressed_at: Option<Instant>,

    /// Poller for managing output capture timing.
    poller: Poller,

    /// Last captured output hash for detecting changes.
    last_output_hash: u64,
}

impl Session {
    /// Creates a new TTY session for the specified tmux pane.
    ///
    /// # Arguments
    ///
    /// * `tmux_session` - The tmux session name (e.g., "my_session")
    /// * `pane` - The tmux pane ID (e.g., "%0", "%12")
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::Session;
    ///
    /// let session = Session::new("my_session", "%0");
    /// assert!(!session.active);
    /// assert_eq!(session.target_session, "my_session");
    /// assert_eq!(session.target_pane, "%0");
    /// ```
    pub fn new(tmux_session: &str, pane: &str) -> Self {
        Self {
            active: false,
            target_pane: pane.to_string(),
            target_session: tmux_session.to_string(),
            width: 80,
            height: 24,
            output_buffer: OutputBuffer::new(DEFAULT_SCROLLBACK),
            escape_pressed_at: None,
            poller: Poller::new(),
            last_output_hash: 0,
        }
    }

    /// Enters interactive mode for this session.
    ///
    /// This marks the session as active and prepares it for interaction.
    /// The actual tmux pane is not modified; this is a logical state change.
    ///
    /// # Errors
    ///
    /// Returns an error if the session is already active.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::Session;
    ///
    /// let mut session = Session::new("my_session", "%0");
    /// session.enter().expect("Failed to enter");
    /// assert!(session.active);
    ///
    /// // Cannot enter twice
    /// assert!(session.enter().is_err());
    /// ```
    pub fn enter(&mut self) -> Result<()> {
        if self.active {
            return Err(anyhow!("Session is already active"));
        }

        self.active = true;
        self.poller.reset();
        self.output_buffer.clear();
        Ok(())
    }

    /// Exits interactive mode for this session.
    ///
    /// This marks the session as inactive. The tmux pane continues running
    /// independently; only the interactive session ends.
    ///
    /// # Errors
    ///
    /// Returns an error if the session is not active.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::Session;
    ///
    /// let mut session = Session::new("my_session", "%0");
    /// session.enter().unwrap();
    ///
    /// session.exit().expect("Failed to exit");
    /// assert!(!session.active);
    /// ```
    pub fn exit(&mut self) -> Result<()> {
        if !self.active {
            return Err(anyhow!("Session is not active"));
        }

        self.active = false;
        self.escape_pressed_at = None;
        Ok(())
    }

    /// Resizes the tmux pane to the specified dimensions.
    ///
    /// This sends a resize command to tmux and updates the session's
    /// stored dimensions.
    ///
    /// # Arguments
    ///
    /// * `width` - The new width in columns
    /// * `height` - The new height in rows
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The tmux command fails
    /// - The pane does not exist
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use rightclick::tty::Session;
    ///
    /// let mut session = Session::new("my_session", "%0");
    /// session.resize(100, 30).expect("Failed to resize");
    /// assert_eq!(session.width, 100);
    /// assert_eq!(session.height, 30);
    /// ```
    pub fn resize(&mut self, width: u16, height: u16) -> Result<()> {
        let target = format!("{}:{}", self.target_session, self.target_pane);

        let output = Command::new("tmux")
            .args([
                "resize-pane",
                "-t",
                &target,
                "-x",
                &width.to_string(),
                "-y",
                &height.to_string(),
            ])
            .output()
            .context("Failed to execute tmux resize-pane command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("tmux resize-pane failed: {}", stderr));
        }

        self.width = width;
        self.height = height;
        Ok(())
    }

    /// Sends a key event to the tmux pane.
    ///
    /// Converts the key event to tmux's key syntax and sends it via
    /// the `tmux send-keys` command.
    ///
    /// # Arguments
    ///
    /// * `key` - The key event to send
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The session is not active
    /// - The tmux command fails
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use rightclick::tty::Session;
    /// use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    ///
    /// let mut session = Session::new("my_session", "%0");
    /// session.enter().unwrap();
    ///
    /// // Send 'a' key
    /// session.send_key(KeyEvent::from(KeyCode::Char('a'))).unwrap();
    ///
    /// // Send Enter
    /// session.send_key(KeyEvent::from(KeyCode::Enter)).unwrap();
    ///
    /// // Send Ctrl+C
    /// let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    /// session.send_key(ctrl_c).unwrap();
    /// ```
    pub fn send_key(&mut self, key: KeyEvent) -> Result<()> {
        if !self.active {
            return Err(anyhow!("Session is not active"));
        }

        // Track escape key for double-escape detection
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            let now = Instant::now();
            if let Some(last) = self.escape_pressed_at {
                if now.duration_since(last) <= DOUBLE_ESCAPE_WINDOW {
                    // Double escape detected - will be handled by caller
                    return Ok(());
                }
            }
            self.escape_pressed_at = Some(now);
        } else {
            self.escape_pressed_at = None;
        }

        let key_str = key_event_to_tmux_key(key);
        let target = format!("{}:{}", self.target_session, self.target_pane);

        let output = Command::new("tmux")
            .args(["send-keys", "-t", &target, &key_str])
            .output()
            .context("Failed to execute tmux send-keys command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("tmux send-keys failed: {}", stderr));
        }

        Ok(())
    }

    /// Sends raw text input to the tmux pane.
    ///
    /// Unlike `send_key`, this sends literal text without key event conversion.
    /// Useful for sending strings of text rather than individual keystrokes.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to send
    ///
    /// # Errors
    ///
    /// Returns an error if the tmux command fails.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use rightclick::tty::Session;
    ///
    /// let mut session = Session::new("my_session", "%0");
    /// session.enter().unwrap();
    ///
    /// // Send a command
    /// session.send_text("ls -la").unwrap();
    /// session.send_key(KeyEvent::from(KeyCode::Enter)).unwrap();
    /// ```
    pub fn send_text(&mut self, text: &str) -> Result<()> {
        if !self.active {
            return Err(anyhow!("Session is not active"));
        }

        let target = format!("{}:{}", self.target_session, self.target_pane);

        let output = Command::new("tmux")
            .args(["send-keys", "-t", &target, "-l", text])
            .output()
            .context("Failed to execute tmux send-keys -l command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("tmux send-keys failed: {}", stderr));
        }

        self.escape_pressed_at = None;
        Ok(())
    }

    /// Polls for new output from the tmux pane.
    ///
    /// Captures the current pane content using `tmux capture-pane` and
    /// returns any new output since the last poll. This method respects
    /// the polling interval to prevent excessive CPU usage.
    ///
    /// # Returns
    ///
    /// The new output since the last poll, or an empty string if no
    /// new output or if polling too frequently.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The session is not active
    /// - The tmux command fails
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use rightclick::tty::Session;
    ///
    /// let mut session = Session::new("my_session", "%0");
    /// session.enter().unwrap();
    ///
    /// // Poll for output
    /// match session.poll_output() {
    ///     Ok(output) => println!("New output: {}", output),
    ///     Err(e) => eprintln!("Poll failed: {}", e),
    /// }
    /// ```
    pub fn poll_output(&mut self) -> Result<String> {
        if !self.active {
            return Err(anyhow!("Session is not active"));
        }

        // Check if we should poll based on interval
        if !self.poller.should_poll() {
            return Ok(String::new());
        }

        self.poller.mark_polled();

        let target = format!("{}:{}", self.target_session, self.target_pane);

        let output = Command::new("tmux")
            .args(["capture-pane", "-t", &target, "-p"])
            .output()
            .context("Failed to execute tmux capture-pane command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("tmux capture-pane failed: {}", stderr));
        }

        let content = String::from_utf8_lossy(&output.stdout);

        // Calculate hash to detect changes
        let hash = xxhash_rust::xxh3::xxh3_64(content.as_bytes());

        if hash == self.last_output_hash {
            // No change
            return Ok(String::new());
        }

        self.last_output_hash = hash;

        // Update output buffer
        self.output_buffer.append(&content);

        Ok(content.to_string())
    }

    /// Forces a poll regardless of the polling interval.
    ///
    /// This is useful when you need immediate output, such as after
    /// sending a command and waiting for its response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The session is not active
    /// - The tmux command fails
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use rightclick::tty::Session;
    ///
    /// let mut session = Session::new("my_session", "%0");
    /// session.enter().unwrap();
    ///
    /// // Force immediate poll
    /// let output = session.force_poll().expect("Failed to poll");
    /// ```
    pub fn force_poll(&mut self) -> Result<String> {
        if !self.active {
            return Err(anyhow!("Session is not active"));
        }

        self.poller.reset();
        self.poll_output()
    }

    /// Handles double-escape detection.
    ///
    /// Checks if two escape key presses occurred within the configured
    /// time window. Returns true if a double escape was detected, which
    /// typically signals that the user wants to exit interactive mode.
    ///
    /// # Returns
    ///
    /// `true` if double escape was detected, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use rightclick::tty::Session;
    /// use crossterm::event::{KeyCode, KeyEvent};
    /// use std::thread::sleep;
    /// use std::time::Duration;
    ///
    /// let mut session = Session::new("my_session", "%0");
    /// session.enter().unwrap();
    ///
    /// // First escape
    /// session.send_key(KeyEvent::from(KeyCode::Esc)).unwrap();
    ///
    /// // Quick second escape (within window)
    /// sleep(Duration::from_millis(50));
    /// session.send_key(KeyEvent::from(KeyCode::Esc)).unwrap();
    ///
    /// if session.handle_double_escape() {
    ///     println!("Double escape detected - exiting!");
    ///     session.exit().unwrap();
    /// }
    /// ```
    pub fn handle_double_escape(&mut self) -> bool {
        if let Some(last) = self.escape_pressed_at {
            let elapsed = last.elapsed();
            if elapsed <= DOUBLE_ESCAPE_WINDOW {
                // Double escape detected
                self.escape_pressed_at = None;
                return true;
            }
        }
        false
    }

    /// Returns whether the session has timed out waiting for a second escape.
    ///
    /// This can be used to reset escape-related UI indicators when the
    /// double-escape window has passed.
    ///
    /// # Returns
    ///
    /// `true` if the escape window has timed out, `false` otherwise
    pub fn escape_window_timed_out(&self) -> bool {
        if let Some(last) = self.escape_pressed_at {
            last.elapsed() > DOUBLE_ESCAPE_WINDOW
        } else {
            true
        }
    }

    /// Gets the visible lines from the output buffer.
    ///
    /// Returns the lines that should be displayed in the terminal,
    /// based on the current session height.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::Session;
    ///
    /// let mut session = Session::new("my_session", "%0");
    /// session.output_buffer.append("Line 1\nLine 2\nLine 3");
    ///
    /// let visible = session.visible_lines();
    /// assert_eq!(visible.len(), 3);
    /// ```
    pub fn visible_lines(&self) -> &[String] {
        self.output_buffer.visible_range(self.height as usize)
    }

    /// Gets the current cursor position in the output buffer.
    ///
    /// Returns the cursor position as (row, col) relative to the
    /// visible area, or None if the cursor is outside the visible range.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::Session;
    ///
    /// let mut session = Session::new("my_session", "%0");
    /// session.height = 2;
    /// session.output_buffer.append("Line 1\nLine 2\nLine 3");
    ///
    /// let cursor = session.visible_cursor();
    /// assert_eq!(cursor, Some((1, 6))); // On last visible line
    /// ```
    pub fn visible_cursor(&self) -> Option<(usize, usize)> {
        self.output_buffer
            .visible_cursor_position(self.height as usize)
    }

    /// Returns the full target string for tmux commands.
    ///
    /// This combines the session name and pane ID in the format
    /// expected by tmux commands (e.g., "session:%12").
    pub fn tmux_target(&self) -> String {
        format!("{}:{}", self.target_session, self.target_pane)
    }
}

/// Converts a crossterm KeyEvent to tmux key syntax.
///
/// Tmux uses a specific syntax for special keys:
/// - `C-x` for Ctrl+x
/// - `M-x` for Alt+x
/// - `C-M-x` for Ctrl+Alt+x
/// - Special names like `Enter`, `Escape`, `Space`, etc.
///
/// # Arguments
///
/// * `key` - The crossterm KeyEvent to convert
///
/// # Returns
///
/// A string in tmux key syntax
fn key_event_to_tmux_key(key: KeyEvent) -> String {
    let mut parts = Vec::new();

    // Add modifiers
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("C".to_string());
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        parts.push("M".to_string());
    }
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        // Shift is usually implicit for uppercase characters
        // but we handle it explicitly for special keys
    }

    // Add the key code
    let key_part = match key.code {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Escape".to_string(),
        KeyCode::Backspace => "BSpace".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::Delete => "DC".to_string(),
        KeyCode::Insert => "IC".to_string(),
        KeyCode::F(n) => format!("F{}", n),
        _ => return String::new(), // Unknown key, return empty
    };

    if parts.is_empty() {
        key_part
    } else {
        parts.push(key_part);
        parts.join("-")
    }
}

/// Executes a tmux command and returns the output.
///
/// # Arguments
///
/// * `args` - The arguments to pass to tmux
///
/// # Errors
///
/// Returns an error if the command fails or tmux returns an error.
pub fn execute_tmux(args: &[&str]) -> Result<String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .context("Failed to execute tmux command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("tmux command failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Lists available tmux panes in a session.
///
/// # Arguments
///
/// * `session` - The tmux session name
///
/// # Returns
///
/// A vector of pane IDs (e.g., ["%0", "%1", "%2"])
///
/// # Errors
///
/// Returns an error if the tmux command fails.
pub fn list_panes(session: &str) -> Result<Vec<String>> {
    let output = execute_tmux(&["list-panes", "-t", session, "-F", "#D"])?;

    let panes: Vec<String> = output
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    Ok(panes)
}

/// Creates a new tmux pane in the specified session.
///
/// # Arguments
///
/// * `session` - The tmux session name
/// * `command` - Optional command to run in the new pane
///
/// # Returns
///
/// The ID of the newly created pane
///
/// # Errors
///
/// Returns an error if the tmux command fails.
pub fn create_pane(session: &str, command: Option<&str>) -> Result<String> {
    let mut args = vec!["split-window", "-t", session, "-P", "-F", "#D"];

    if let Some(cmd) = command {
        args.push(cmd);
    }

    let pane_id = execute_tmux(&args)?;
    Ok(pane_id.trim().to_string())
}

/// Kills a tmux pane.
///
/// # Arguments
///
/// * `session` - The tmux session name
/// * `pane` - The pane ID
///
/// # Errors
///
/// Returns an error if the tmux command fails.
pub fn kill_pane(session: &str, pane: &str) -> Result<()> {
    let target = format!("{}:{}", session, pane);
    execute_tmux(&["kill-pane", "-t", &target])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    #[test]
    fn new_session() {
        let session = Session::new("my_session", "%0");
        assert!(!session.active);
        assert_eq!(session.target_session, "my_session");
        assert_eq!(session.target_pane, "%0");
        assert_eq!(session.width, 80);
        assert_eq!(session.height, 24);
    }

    #[test]
    fn enter_session() {
        let mut session = Session::new("my_session", "%0");
        assert!(!session.active);

        session.enter().expect("Failed to enter");
        assert!(session.active);

        // Cannot enter twice
        assert!(session.enter().is_err());
    }

    #[test]
    fn exit_session() {
        let mut session = Session::new("my_session", "%0");
        session.enter().unwrap();
        assert!(session.active);

        session.exit().expect("Failed to exit");
        assert!(!session.active);

        // Cannot exit twice
        assert!(session.exit().is_err());
    }

    #[test]
    fn send_key_requires_active_session() {
        let mut session = Session::new("my_session", "%0");
        let key = KeyEvent::from(KeyCode::Char('a'));

        // Should fail - session not active
        assert!(session.send_key(key).is_err());
    }

    #[test]
    fn send_text_requires_active_session() {
        let mut session = Session::new("my_session", "%0");

        // Should fail - session not active
        assert!(session.send_text("hello").is_err());
    }

    #[test]
    fn poll_output_requires_active_session() {
        let mut session = Session::new("my_session", "%0");

        // Should fail - session not active
        assert!(session.poll_output().is_err());
    }

    #[test]
    fn double_escape_detection() {
        let mut session = Session::new("my_session", "%0");
        session.enter().unwrap();

        // Initially no double escape
        assert!(!session.handle_double_escape());

        // Simulate first escape
        session.escape_pressed_at = Some(Instant::now());

        // Immediately check - should be double escape
        assert!(session.handle_double_escape());
        assert!(session.escape_pressed_at.is_none());
    }

    #[test]
    fn double_escape_window_expired() {
        let mut session = Session::new("my_session", "%0");
        session.enter().unwrap();

        // Set escape time far in the past
        session.escape_pressed_at = Some(Instant::now() - Duration::from_secs(10));

        // Window expired - should not be double escape
        assert!(!session.handle_double_escape());
    }

    #[test]
    fn escape_window_timed_out() {
        let mut session = Session::new("my_session", "%0");

        // No escape pressed - considered timed out
        assert!(session.escape_window_timed_out());

        // Just pressed - not timed out
        session.escape_pressed_at = Some(Instant::now());
        assert!(!session.escape_window_timed_out());
    }

    #[test]
    fn visible_lines() {
        let mut session = Session::new("my_session", "%0");
        session.height = 3;
        session
            .output_buffer
            .append("Line 1\nLine 2\nLine 3\nLine 4\nLine 5");

        let visible = session.visible_lines();
        assert_eq!(visible.len(), 3);
        assert_eq!(visible[0], "Line 3");
        assert_eq!(visible[2], "Line 5");
    }

    #[test]
    fn visible_cursor() {
        let mut session = Session::new("my_session", "%0");
        session.height = 2;
        session.output_buffer.append("Line 1\nLine 2\nLine 3");

        let cursor = session.visible_cursor();
        assert_eq!(cursor, Some((1, 6))); // Row 1 (relative), 6 chars
    }

    #[test]
    fn tmux_target_format() {
        let session = Session::new("my_session", "%12");
        assert_eq!(session.tmux_target(), "my_session:%12");
    }

    #[test]
    fn key_event_to_tmux_key_simple() {
        let key = KeyEvent::from(KeyCode::Char('a'));
        assert_eq!(key_event_to_tmux_key(key), "a");
    }

    #[test]
    fn key_event_to_tmux_key_control() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(key_event_to_tmux_key(key), "C-c");
    }

    #[test]
    fn key_event_to_tmux_key_alt() {
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT);
        assert_eq!(key_event_to_tmux_key(key), "M-x");
    }

    #[test]
    fn key_event_to_tmux_key_control_alt() {
        let key = KeyEvent::new(
            KeyCode::Char('d'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        );
        assert_eq!(key_event_to_tmux_key(key), "C-M-d");
    }

    #[test]
    fn key_event_to_tmux_key_special() {
        assert_eq!(
            key_event_to_tmux_key(KeyEvent::from(KeyCode::Enter)),
            "Enter"
        );
        assert_eq!(
            key_event_to_tmux_key(KeyEvent::from(KeyCode::Esc)),
            "Escape"
        );
        assert_eq!(
            key_event_to_tmux_key(KeyEvent::from(KeyCode::Char(' '))),
            "Space"
        );
        assert_eq!(key_event_to_tmux_key(KeyEvent::from(KeyCode::Tab)), "Tab");
        assert_eq!(
            key_event_to_tmux_key(KeyEvent::from(KeyCode::Backspace)),
            "BSpace"
        );
        assert_eq!(key_event_to_tmux_key(KeyEvent::from(KeyCode::Delete)), "DC");
        assert_eq!(key_event_to_tmux_key(KeyEvent::from(KeyCode::Insert)), "IC");
    }

    #[test]
    fn key_event_to_tmux_key_arrows() {
        assert_eq!(key_event_to_tmux_key(KeyEvent::from(KeyCode::Up)), "Up");
        assert_eq!(key_event_to_tmux_key(KeyEvent::from(KeyCode::Down)), "Down");
        assert_eq!(key_event_to_tmux_key(KeyEvent::from(KeyCode::Left)), "Left");
        assert_eq!(
            key_event_to_tmux_key(KeyEvent::from(KeyCode::Right)),
            "Right"
        );
    }

    #[test]
    fn key_event_to_tmux_key_function() {
        assert_eq!(key_event_to_tmux_key(KeyEvent::from(KeyCode::F(1))), "F1");
        assert_eq!(key_event_to_tmux_key(KeyEvent::from(KeyCode::F(12))), "F12");
    }

    #[test]
    fn key_event_to_tmux_key_navigation() {
        assert_eq!(key_event_to_tmux_key(KeyEvent::from(KeyCode::Home)), "Home");
        assert_eq!(key_event_to_tmux_key(KeyEvent::from(KeyCode::End)), "End");
        assert_eq!(
            key_event_to_tmux_key(KeyEvent::from(KeyCode::PageUp)),
            "PageUp"
        );
        assert_eq!(
            key_event_to_tmux_key(KeyEvent::from(KeyCode::PageDown)),
            "PageDown"
        );
    }
}
