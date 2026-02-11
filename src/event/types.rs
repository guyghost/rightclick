//! Event types for the pub/sub event bus system.
//!
//! This module defines the core event and topic types used throughout
//! the rightclick application for decoupled communication between components.

use std::path::PathBuf;

/// Events that can be published and subscribed to in the system.
///
/// Events represent state changes or significant occurrences that components
/// may need to react to. All events are Clone to allow distribution to multiple subscribers.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A file in the workspace has changed.
    FileChanged { path: PathBuf },

    /// Git repository state has changed (new commits, branch changes, etc.).
    GitChanged,

    /// A session file has been modified.
    SessionFileChanged,

    /// Time-driven update tick.
    TDUpdate,

    /// Session state has been updated.
    SessionUpdate,

    /// Focus has moved to a different plugin.
    FocusChanged { plugin_id: String },

    /// A refresh of the UI or data is needed.
    RefreshNeeded,

    /// Configuration has been modified.
    ConfigChanged,

    /// An error occurred that should be propagated.
    Error { message: String },

    /// A key was pressed.
    Key { code: String, modifiers: KeyModifiers },
}

/// Keyboard modifiers for key events.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct KeyModifiers {
    /// Control key pressed
    pub ctrl: bool,
    /// Alt key pressed
    pub alt: bool,
    /// Shift key pressed
    pub shift: bool,
}

/// Topics for event subscription.
///
/// Topics categorize events and allow subscribers to receive only the events
/// they are interested in. The special `All` topic receives every event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Topic {
    /// File system change events.
    FileChanges,

    /// Git repository change events.
    GitChanges,

    /// Adapter watch events (for monitoring external processes).
    AdapterWatch,

    /// Configuration change events.
    ConfigChange,

    /// Special topic that receives all events regardless of their primary topic.
    All,
}

impl Topic {
    /// Returns true if this topic should receive events from the given source topic.
    ///
    /// The `All` topic receives events from all topics. Other topics only
    /// receive events from their matching topic.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::event::Topic;
    ///
    /// assert!(Topic::All.matches(Topic::FileChanges));
    /// assert!(Topic::FileChanges.matches(Topic::FileChanges));
    /// assert!(!Topic::FileChanges.matches(Topic::GitChanges));
    /// ```
    pub fn matches(&self, source: Topic) -> bool {
        matches!(self, Topic::All) || self == &source
    }
}

/// Strategy for handling buffer overflow in subscriber channels.
///
/// When a subscriber's channel buffer is full, the dispatcher must decide
/// how to handle new events. This enum defines the available strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowStrategy {
    /// Drop new events when the buffer is full.
    ///
    /// Use this strategy when you only care about the most recent events
    /// and don't want to block the publisher.
    Drop,

    /// Drop the oldest events when the buffer is full.
    ///
    /// Use this strategy when you need to process all events eventually
    /// and prefer to lose older events rather than newer ones.
    DropOldest,
}

impl Default for OverflowStrategy {
    fn default() -> Self {
        OverflowStrategy::Drop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_all_matches_everything() {
        assert!(Topic::All.matches(Topic::FileChanges));
        assert!(Topic::All.matches(Topic::GitChanges));
        assert!(Topic::All.matches(Topic::AdapterWatch));
        assert!(Topic::All.matches(Topic::ConfigChange));
    }

    #[test]
    fn topic_matches_only_same() {
        assert!(Topic::FileChanges.matches(Topic::FileChanges));
        assert!(!Topic::FileChanges.matches(Topic::GitChanges));
        assert!(!Topic::FileChanges.matches(Topic::AdapterWatch));
    }

    #[test]
    fn overflow_strategy_default_is_drop() {
        assert_eq!(OverflowStrategy::default(), OverflowStrategy::Drop);
    }

    #[test]
    fn event_clone_preserves_data() {
        let event = Event::FileChanged {
            path: PathBuf::from("/test/path"),
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn event_error_with_message() {
        let event = Event::Error {
            message: "test error".to_string(),
        };
        match event {
            Event::Error { message } => assert_eq!(message, "test error"),
            _ => panic!("Expected Error variant"),
        }
    }

    #[test]
    fn event_focus_changed_with_plugin_id() {
        let event = Event::FocusChanged {
            plugin_id: "plugin-1".to_string(),
        };
        match event {
            Event::FocusChanged { plugin_id } => assert_eq!(plugin_id, "plugin-1"),
            _ => panic!("Expected FocusChanged variant"),
        }
    }
}
