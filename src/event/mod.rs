//! Event bus system for decoupled pub/sub communication.
//!
//! This module provides a thread-safe, async-compatible event bus that enables
//! publish/subscribe communication patterns between components. It supports:
//!
//! - Topic-based subscriptions
//! - Buffered channels per subscriber
//! - Configurable overflow strategies
//! - Thread-safe concurrent access
//!
//! # Examples
//!
//! ```
//! use rightclick::event::{Dispatcher, Topic, Event};
//! use std::path::PathBuf;
//!
//! # tokio_test::block_on(async {
//! // Create a dispatcher
//! let dispatcher = Dispatcher::new();
//!
//! // Subscribe to a topic
//! let mut subscription = dispatcher.subscribe(Topic::FileChanges);
//!
//! // Publish an event
//! dispatcher.publish(Topic::FileChanges, Event::FileChanged {
//!     path: PathBuf::from("/test/file.txt"),
//! });
//!
//! // Receive the event
//! let event = subscription.receiver.recv().await.unwrap();
//! # });
//! ```

mod dispatcher;
mod types;

// Public exports
pub use dispatcher::{Dispatcher, Subscription};
pub use types::{Event, KeyModifiers, NotificationEventLevel, OverflowStrategy, Topic};

/// Type alias for Dispatcher (used by some plugins)
pub type EventDispatcher = Dispatcher;

/// Default buffer size for subscriber channels.
///
/// This value provides a reasonable balance between memory usage and
/// allowing subscribers to keep up with bursts of events.
pub const DEFAULT_BUFFER_SIZE: usize = 128;

/// Minimum buffer size for subscriber channels.
///
/// Very small buffers can cause excessive blocking or event drops.
/// This minimum ensures subscribers have some breathing room.
pub const MIN_BUFFER_SIZE: usize = 4;

/// Maximum buffer size for subscriber channels.
///
/// Very large buffers can consume excessive memory. Subscribers that
/// need larger buffers should consider using `OverflowStrategy::DropOldest`
/// to handle bursts more efficiently.
pub const MAX_BUFFER_SIZE: usize = 10_000;
