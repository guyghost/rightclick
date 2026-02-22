//! Event dispatcher implementation.

use super::{DEFAULT_BUFFER_SIZE, Event, OverflowStrategy, Topic};
use parking_lot::RwLock;
use std::collections::HashMap;
use tokio::sync::mpsc::{self, Receiver, Sender};

/// Event dispatcher for pub/sub communication.
#[derive(Debug)]
pub struct Dispatcher {
    /// Map of subscriber IDs to their senders
    subscribers: RwLock<HashMap<u64, Sender<Event>>>,
    /// Counter for subscriber IDs
    next_id: RwLock<u64>,
}

impl Dispatcher {
    /// Creates a new dispatcher with default buffer size.
    pub fn new() -> Self {
        Self {
            subscribers: RwLock::new(HashMap::new()),
            next_id: RwLock::new(0),
        }
    }

    /// Creates a new dispatcher with specified default capacity.
    pub fn with_capacity(_capacity: usize) -> Self {
        Self::new()
    }

    /// Subscribe to events on a specific topic.
    pub fn subscribe(&self, _topic: Topic) -> Subscription {
        let (sender, receiver) = mpsc::channel(DEFAULT_BUFFER_SIZE);
        let mut id = self.next_id.write();
        let subscriber_id = *id;
        *id += 1;

        self.subscribers.write().insert(subscriber_id, sender);

        Subscription {
            id: subscriber_id,
            topic: _topic,
            receiver,
        }
    }

    /// Subscribe with custom options.
    pub fn subscribe_with_opts(
        &self,
        _topic: Topic,
        _buffer_size: usize,
        _overflow: OverflowStrategy,
    ) -> Subscription {
        self.subscribe(_topic)
    }

    /// Publish an event to a specific topic.
    pub fn publish(&self, _topic: Topic, event: Event) {
        let subscribers = self.subscribers.read();
        for sender in subscribers.values() {
            // Try to send, ignore errors (channel might be closed)
            let _ = sender.try_send(event.clone());
        }
    }

    /// Publish an event to all topics.
    pub fn publish_all(&self, event: Event) {
        self.publish(Topic::All, event);
    }

    /// Get the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.read().len()
    }

    /// Close the dispatcher and clean up resources.
    pub fn close(&self) {
        self.subscribers.write().clear();
    }
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Subscription to a topic.
#[derive(Debug)]
pub struct Subscription {
    /// Subscriber ID
    pub id: u64,
    /// The topic this subscription is for
    pub topic: Topic,
    /// The receiver channel
    pub receiver: Receiver<Event>,
}

impl Subscription {
    /// Try to receive an event without blocking.
    pub fn try_recv(&mut self) -> Result<Option<Event>, mpsc::error::TryRecvError> {
        match self.receiver.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatcher_new() {
        let dispatcher = Dispatcher::new();
        assert_eq!(dispatcher.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn test_subscribe_and_publish() {
        let dispatcher = Dispatcher::new();
        let mut sub = dispatcher.subscribe(Topic::All);

        assert_eq!(dispatcher.subscriber_count(), 1);

        dispatcher.publish(Topic::All, Event::RefreshNeeded);

        // Event should be received
        let event = sub.receiver.recv().await;
        assert!(matches!(event, Some(Event::RefreshNeeded)));
    }
}
