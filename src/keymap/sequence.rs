//! Multi-key sequence support for Vim-style key bindings.
//!
//! This module provides a key sequence buffer that accumulates key presses
//! and matches them against registered multi-key sequences (e.g., `gg` to
//! go to the first item, `dd` to delete in file browser). It also supports
//! count prefixes (e.g., `5j` to move down 5 times).
//!
//! # Architecture
//!
//! The [`KeySequenceBuffer`] sits between raw key input and the keymap
//! [`Registry`](super::Registry). Keys are fed into the buffer, which
//! determines whether they form part of a multi-key sequence, a count
//! prefix, or should be passed through immediately.
//!
//! # Example
//!
//! ```
//! use rightclick::keymap::sequence::{KeySequenceBuffer, SequenceResult};
//! use rightclick::keymap::FocusContext;
//!
//! let mut buf = KeySequenceBuffer::new();
//!
//! // "j" is not a prefix of any sequence, so it passes through immediately
//! match buf.feed("j", FocusContext::Global) {
//!     SequenceResult::PassThrough { key, count } => {
//!         assert_eq!(key, "j");
//!         assert!(count.is_none());
//!     }
//!     _ => panic!("Expected PassThrough"),
//! }
//!
//! // "g" is the start of "gg", so it pends
//! match buf.feed("g", FocusContext::Global) {
//!     SequenceResult::Pending { .. } => {}
//!     _ => panic!("Expected Pending"),
//! }
//!
//! // Second "g" completes the sequence
//! match buf.feed("g", FocusContext::Global) {
//!     SequenceResult::Resolved { command_id, .. } => {
//!         assert_eq!(command_id, "nav.first");
//!     }
//!     _ => panic!("Expected Resolved"),
//! }
//! ```

use super::context::FocusContext;

/// State of the sequence buffer.
#[derive(Debug, Clone, PartialEq)]
pub enum SequenceState {
    /// No pending input.
    Empty,
    /// A count prefix has been entered (e.g., "5").
    CountPrefix {
        /// The accumulated count value.
        count: usize,
    },
    /// A partial sequence is pending (e.g., "g" waiting for "g").
    PendingSequence {
        /// The keys accumulated so far.
        prefix: String,
        /// An optional count prefix entered before the sequence.
        count: Option<usize>,
        /// When the pending sequence started (for timeout detection).
        started_at: std::time::Instant,
    },
}

/// Result of feeding a key into the buffer.
#[derive(Debug)]
pub enum SequenceResult {
    /// The key is being buffered; show feedback to the user.
    Pending {
        /// Display string showing what has been typed so far.
        display: String,
    },
    /// A sequence resolved to an action.
    Resolved {
        /// The command ID to trigger.
        command_id: String,
        /// An optional count prefix (e.g., `3gg` would have count = Some(3)).
        count: Option<usize>,
    },
    /// The sequence timed out; return the pending key.
    Timeout {
        /// The key string that was pending.
        key: String,
    },
    /// The key does not match any sequence; pass through immediately.
    PassThrough {
        /// The key to pass through.
        key: String,
        /// An optional count prefix that was accumulated before this key.
        count: Option<usize>,
    },
}

/// Definition of a key sequence.
#[derive(Debug, Clone)]
pub struct SequenceDefinition {
    /// The keys that make up the sequence (e.g., `["g", "g"]`).
    pub keys: Vec<String>,
    /// The command ID to trigger when the sequence is completed.
    pub command_id: String,
    /// The context this sequence is valid in (`None` means all contexts).
    pub context: Option<FocusContext>,
}

/// Buffer that accumulates key presses and matches them against registered sequences.
///
/// The buffer supports:
/// - Count prefixes: typing digits accumulates a count (e.g., `5j`)
/// - Multi-key sequences: sequences like `gg`, `dd`, `yy`
/// - Timeout: if a partial sequence is not completed within the timeout,
///   the pending keys are returned
/// - Zero-latency pass-through: keys that are not a prefix of any sequence
///   are returned immediately
pub struct KeySequenceBuffer {
    state: SequenceState,
    sequences: Vec<SequenceDefinition>,
    timeout_ms: u64,
}

impl KeySequenceBuffer {
    /// Creates a new `KeySequenceBuffer` with default timeout and default sequences.
    pub fn new() -> Self {
        let mut buf = Self {
            state: SequenceState::Empty,
            sequences: Vec::new(),
            timeout_ms: 500,
        };
        buf.register_defaults();
        buf
    }

    /// Sets the timeout in milliseconds for pending sequences.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Registers a new sequence definition.
    pub fn register_sequence(&mut self, definition: SequenceDefinition) {
        self.sequences.push(definition);
    }

    /// Feeds a key press into the buffer. Returns what to do.
    ///
    /// The logic is:
    /// 1. If the key is a digit and state is `Empty` or `CountPrefix`, accumulate count.
    /// 2. If there are sequences that start with the current prefix + key, go to `PendingSequence`.
    /// 3. If current prefix + key matches a complete sequence, return `Resolved`.
    /// 4. If no sequences match, return `PassThrough` (with accumulated count if any).
    pub fn feed(&mut self, key: &str, context: FocusContext) -> SequenceResult {
        // Step 1: handle digit accumulation for count prefix
        if key.len() == 1 {
            if let Some(digit) = key.chars().next().and_then(|c| c.to_digit(10)) {
                match &self.state {
                    SequenceState::Empty => {
                        self.state = SequenceState::CountPrefix {
                            count: digit as usize,
                        };
                        return SequenceResult::Pending {
                            display: format!("{}", digit),
                        };
                    }
                    SequenceState::CountPrefix { count } => {
                        let new_count = count * 10 + digit as usize;
                        self.state = SequenceState::CountPrefix { count: new_count };
                        return SequenceResult::Pending {
                            display: format!("{}", new_count),
                        };
                    }
                    SequenceState::PendingSequence { .. } => {
                        // digits after a sequence prefix are not count digits;
                        // fall through to sequence matching
                    }
                }
            }
        }

        // Determine current prefix and count from state
        let (current_prefix, current_count) = match &self.state {
            SequenceState::Empty => (String::new(), None),
            SequenceState::CountPrefix { count } => (String::new(), Some(*count)),
            SequenceState::PendingSequence { prefix, count, .. } => (prefix.clone(), *count),
        };

        // Build the candidate prefix: existing prefix keys + new key
        let candidate_keys: Vec<String> = if current_prefix.is_empty() {
            vec![key.to_string()]
        } else {
            let mut keys: Vec<String> = current_prefix.split(' ').map(|s| s.to_string()).collect();
            keys.push(key.to_string());
            keys
        };

        // Check for exact match first
        if let Some(seq) = self.find_exact_match(&candidate_keys, context) {
            let command_id = seq.command_id.clone();
            self.state = SequenceState::Empty;
            return SequenceResult::Resolved {
                command_id,
                count: current_count,
            };
        }

        // Check for prefix match (there exist sequences that start with candidate_keys)
        if self.has_prefix_match(&candidate_keys, context) {
            let prefix_display = candidate_keys.join(" ");
            self.state = SequenceState::PendingSequence {
                prefix: prefix_display.clone(),
                count: current_count,
                started_at: std::time::Instant::now(),
            };
            let display = if let Some(c) = current_count {
                format!("{}{}", c, prefix_display)
            } else {
                prefix_display
            };
            return SequenceResult::Pending { display };
        }

        // No match at all - pass through
        self.state = SequenceState::Empty;
        SequenceResult::PassThrough {
            key: key.to_string(),
            count: current_count,
        }
    }

    /// Checks for timeout on the pending sequence. Call this from the tick loop.
    ///
    /// Returns `Some(SequenceResult::Timeout)` if the pending sequence has timed out,
    /// or `None` if there is no pending sequence or it has not timed out yet.
    pub fn check_timeout(&mut self) -> Option<SequenceResult> {
        if let SequenceState::PendingSequence {
            prefix, started_at, ..
        } = &self.state
        {
            if started_at.elapsed().as_millis() >= u128::from(self.timeout_ms) {
                let key = prefix.clone();
                self.state = SequenceState::Empty;
                return Some(SequenceResult::Timeout { key });
            }
        }
        None
    }

    /// Resets the buffer state to empty.
    pub fn reset(&mut self) {
        self.state = SequenceState::Empty;
    }

    /// Returns the current display string for the pending sequence (for the footer).
    ///
    /// Returns `None` if the buffer is empty.
    pub fn pending_display(&self) -> Option<String> {
        match &self.state {
            SequenceState::Empty => None,
            SequenceState::CountPrefix { count } => Some(format!("{}", count)),
            SequenceState::PendingSequence { prefix, count, .. } => {
                if let Some(c) = count {
                    Some(format!("{}{}", c, prefix))
                } else {
                    Some(prefix.clone())
                }
            }
        }
    }

    /// Registers default sequences.
    fn register_defaults(&mut self) {
        self.register_sequence(SequenceDefinition {
            keys: vec!["g".to_string(), "g".to_string()],
            command_id: "nav.first".to_string(),
            context: None,
        });
        self.register_sequence(SequenceDefinition {
            keys: vec!["d".to_string(), "d".to_string()],
            command_id: "file.delete".to_string(),
            context: Some(FocusContext::FileBrowser),
        });
        self.register_sequence(SequenceDefinition {
            keys: vec!["y".to_string(), "y".to_string()],
            command_id: "clipboard.copy".to_string(),
            context: None,
        });
    }

    /// Finds an exact match for the given keys in the given context.
    fn find_exact_match(
        &self,
        candidate_keys: &[String],
        context: FocusContext,
    ) -> Option<&SequenceDefinition> {
        self.sequences
            .iter()
            .find(|seq| seq.keys == candidate_keys && self.context_matches(seq, context))
    }

    /// Returns true if any registered sequence starts with the given keys
    /// (but is longer, i.e., a proper prefix).
    fn has_prefix_match(&self, candidate_keys: &[String], context: FocusContext) -> bool {
        self.sequences.iter().any(|seq| {
            seq.keys.len() > candidate_keys.len()
                && seq.keys.starts_with(candidate_keys)
                && self.context_matches(seq, context)
        })
    }

    /// Checks whether a sequence definition matches the given context.
    /// A sequence with `context: None` matches all contexts.
    fn context_matches(&self, seq: &SequenceDefinition, context: FocusContext) -> bool {
        match seq.context {
            None => true,
            Some(seq_context) => seq_context == context,
        }
    }
}

impl Default for KeySequenceBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for KeySequenceBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeySequenceBuffer")
            .field("state", &self.state)
            .field("sequences", &self.sequences)
            .field("timeout_ms", &self.timeout_ms)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_for_non_sequence_key() {
        let mut buf = KeySequenceBuffer::new();
        match buf.feed("j", FocusContext::Global) {
            SequenceResult::PassThrough { key, count } => {
                assert_eq!(key, "j");
                assert!(count.is_none());
            }
            other => panic!("Expected PassThrough, got {:?}", other),
        }
    }

    #[test]
    fn gg_sequence_resolves() {
        let mut buf = KeySequenceBuffer::new();

        // First "g" should pend
        match buf.feed("g", FocusContext::Global) {
            SequenceResult::Pending { display } => {
                assert_eq!(display, "g");
            }
            other => panic!("Expected Pending, got {:?}", other),
        }

        // Second "g" should resolve to "nav.first"
        match buf.feed("g", FocusContext::Global) {
            SequenceResult::Resolved { command_id, count } => {
                assert_eq!(command_id, "nav.first");
                assert!(count.is_none());
            }
            other => panic!("Expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn count_prefix_single_digit() {
        let mut buf = KeySequenceBuffer::new();

        // "5" should start count prefix
        match buf.feed("5", FocusContext::Global) {
            SequenceResult::Pending { display } => {
                assert_eq!(display, "5");
            }
            other => panic!("Expected Pending, got {:?}", other),
        }
        assert_eq!(buf.state, SequenceState::CountPrefix { count: 5 });

        // "j" should pass through with count
        match buf.feed("j", FocusContext::Global) {
            SequenceResult::PassThrough { key, count } => {
                assert_eq!(key, "j");
                assert_eq!(count, Some(5));
            }
            other => panic!("Expected PassThrough, got {:?}", other),
        }
    }

    #[test]
    fn count_prefix_multi_digit() {
        let mut buf = KeySequenceBuffer::new();

        // "1" then "2" should accumulate to 12
        match buf.feed("1", FocusContext::Global) {
            SequenceResult::Pending { display } => assert_eq!(display, "1"),
            other => panic!("Expected Pending, got {:?}", other),
        }
        match buf.feed("2", FocusContext::Global) {
            SequenceResult::Pending { display } => assert_eq!(display, "12"),
            other => panic!("Expected Pending, got {:?}", other),
        }

        // "j" should pass through with count 12
        match buf.feed("j", FocusContext::Global) {
            SequenceResult::PassThrough { key, count } => {
                assert_eq!(key, "j");
                assert_eq!(count, Some(12));
            }
            other => panic!("Expected PassThrough, got {:?}", other),
        }
    }

    #[test]
    fn dd_in_file_browser_context() {
        let mut buf = KeySequenceBuffer::new();

        // "d" in FileBrowser should pend (dd is registered for FileBrowser)
        match buf.feed("d", FocusContext::FileBrowser) {
            SequenceResult::Pending { display } => {
                assert_eq!(display, "d");
            }
            other => panic!("Expected Pending, got {:?}", other),
        }

        // Second "d" should resolve to "file.delete"
        match buf.feed("d", FocusContext::FileBrowser) {
            SequenceResult::Resolved { command_id, count } => {
                assert_eq!(command_id, "file.delete");
                assert!(count.is_none());
            }
            other => panic!("Expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn dd_in_wrong_context_passes_through() {
        let mut buf = KeySequenceBuffer::new();

        // "d" in GitStatus has no "dd" sequence, but "gg" is global so "d" is not
        // a prefix of any sequence in this context. Should pass through.
        match buf.feed("d", FocusContext::GitStatus) {
            SequenceResult::PassThrough { key, count } => {
                assert_eq!(key, "d");
                assert!(count.is_none());
            }
            other => panic!("Expected PassThrough, got {:?}", other),
        }
    }

    #[test]
    fn timeout_returns_pending_key() {
        let mut buf = KeySequenceBuffer::new().with_timeout(0); // immediate timeout

        // "g" should pend
        match buf.feed("g", FocusContext::Global) {
            SequenceResult::Pending { .. } => {}
            other => panic!("Expected Pending, got {:?}", other),
        }

        // Wait a tiny bit to ensure timeout triggers
        std::thread::sleep(std::time::Duration::from_millis(1));

        // check_timeout should return Timeout
        match buf.check_timeout() {
            Some(SequenceResult::Timeout { key }) => {
                assert_eq!(key, "g");
            }
            other => panic!("Expected Timeout, got {:?}", other),
        }

        // State should be empty after timeout
        assert_eq!(buf.state, SequenceState::Empty);
    }

    #[test]
    fn no_timeout_when_empty() {
        let mut buf = KeySequenceBuffer::new();
        assert!(buf.check_timeout().is_none());
    }

    #[test]
    fn reset_clears_state() {
        let mut buf = KeySequenceBuffer::new();

        // Enter a pending state
        buf.feed("g", FocusContext::Global);
        assert_ne!(buf.state, SequenceState::Empty);

        // Reset should clear it
        buf.reset();
        assert_eq!(buf.state, SequenceState::Empty);
    }

    #[test]
    fn pending_display_empty() {
        let buf = KeySequenceBuffer::new();
        assert!(buf.pending_display().is_none());
    }

    #[test]
    fn pending_display_count_prefix() {
        let mut buf = KeySequenceBuffer::new();
        buf.feed("5", FocusContext::Global);
        assert_eq!(buf.pending_display(), Some("5".to_string()));
    }

    #[test]
    fn pending_display_sequence() {
        let mut buf = KeySequenceBuffer::new();
        buf.feed("g", FocusContext::Global);
        assert_eq!(buf.pending_display(), Some("g".to_string()));
    }

    #[test]
    fn pending_display_count_and_sequence() {
        let mut buf = KeySequenceBuffer::new();
        buf.feed("3", FocusContext::Global);
        buf.feed("g", FocusContext::Global);
        assert_eq!(buf.pending_display(), Some("3g".to_string()));
    }

    #[test]
    fn count_prefix_then_sequence() {
        let mut buf = KeySequenceBuffer::new();

        // "3" then "g" then "g" should resolve with count=3
        buf.feed("3", FocusContext::Global);
        buf.feed("g", FocusContext::Global);
        match buf.feed("g", FocusContext::Global) {
            SequenceResult::Resolved { command_id, count } => {
                assert_eq!(command_id, "nav.first");
                assert_eq!(count, Some(3));
            }
            other => panic!("Expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn yy_sequence_resolves_globally() {
        let mut buf = KeySequenceBuffer::new();

        match buf.feed("y", FocusContext::Workspace) {
            SequenceResult::Pending { .. } => {}
            other => panic!("Expected Pending, got {:?}", other),
        }

        match buf.feed("y", FocusContext::Workspace) {
            SequenceResult::Resolved { command_id, count } => {
                assert_eq!(command_id, "clipboard.copy");
                assert!(count.is_none());
            }
            other => panic!("Expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn partial_sequence_then_non_matching_key_passes_through() {
        let mut buf = KeySequenceBuffer::new();

        // "g" should pend
        match buf.feed("g", FocusContext::Global) {
            SequenceResult::Pending { .. } => {}
            other => panic!("Expected Pending, got {:?}", other),
        }

        // "x" does not continue any sequence from "g", so pass through
        match buf.feed("x", FocusContext::Global) {
            SequenceResult::PassThrough { key, count } => {
                assert_eq!(key, "x");
                assert!(count.is_none());
            }
            other => panic!("Expected PassThrough, got {:?}", other),
        }
    }
}
