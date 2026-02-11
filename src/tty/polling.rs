//! TTY output polling mechanism.
//!
//! This module provides the [`Poller`] struct for managing the timing of
//! output polling from tmux panes. It uses a fixed interval to prevent
//! excessive polling while maintaining responsiveness.
//!
//! # Architecture
//!
//! The poller is part of the Functional Core - it contains pure logic for
//! determining when polling should occur without performing any I/O.

use std::time::{Duration, Instant};

/// Default polling interval in milliseconds.
pub const DEFAULT_POLL_INTERVAL_MS: u64 = 50;

/// Manages polling timing for TTY output.
///
/// The poller tracks when the last poll occurred and determines whether
/// enough time has elapsed for another poll. This prevents excessive
/// CPU usage from continuous polling.
///
/// # Example
///
/// ```
/// use rightclick::tty::polling::Poller;
/// use std::time::Duration;
/// use std::thread::sleep;
///
/// let mut poller = Poller::new();
///
/// // First check should always be true
/// assert!(poller.should_poll());
/// poller.mark_polled();
///
/// // Immediately after, should be false
/// assert!(!poller.should_poll());
///
/// // After waiting the interval, should be true again
/// sleep(Duration::from_millis(60));
/// assert!(poller.should_poll());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Poller {
    /// The interval between polls.
    interval: Duration,

    /// The time of the last poll.
    last_poll: Instant,
}

impl Poller {
    /// Creates a new poller with the default interval.
    ///
    /// The default interval is 50ms, which provides a good balance between
    /// responsiveness and CPU usage.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::polling::Poller;
    ///
    /// let poller = Poller::new();
    /// // Ready to poll immediately
    /// assert!(poller.should_poll());
    /// ```
    pub fn new() -> Self {
        Self {
            interval: Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
            last_poll: Instant::now() - Duration::from_millis(DEFAULT_POLL_INTERVAL_MS * 2),
        }
    }

    /// Creates a new poller with a custom interval.
    ///
    /// # Arguments
    ///
    /// * `interval` - The duration between polls
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::polling::Poller;
    /// use std::time::Duration;
    ///
    /// // Create a poller that polls every 100ms
    /// let poller = Poller::with_interval(Duration::from_millis(100));
    /// assert!(poller.should_poll());
    /// ```
    pub fn with_interval(interval: Duration) -> Self {
        Self {
            interval,
            last_poll: Instant::now() - interval * 2,
        }
    }

    /// Checks if enough time has elapsed since the last poll.
    ///
    /// Returns `true` if the configured interval has passed since the
    /// last poll, indicating that a new poll should be performed.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::polling::Poller;
    /// use std::time::Duration;
    ///
    /// let mut poller = Poller::with_interval(Duration::from_millis(100));
    ///
    /// // Should be able to poll immediately
    /// assert!(poller.should_poll());
    ///
    /// // Mark as polled
    /// poller.mark_polled();
    ///
    /// // Should not be able to poll immediately after
    /// assert!(!poller.should_poll());
    /// ```
    pub fn should_poll(&self) -> bool {
        self.last_poll.elapsed() >= self.interval
    }

    /// Returns the time remaining until the next poll should occur.
    ///
    /// Returns `None` if polling is already due, otherwise returns the
    /// duration until the next poll.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::polling::Poller;
    /// use std::time::Duration;
    ///
    /// let mut poller = Poller::with_interval(Duration::from_millis(100));
    ///
    /// // Just created, so no time remaining (None means poll now)
    /// assert!(poller.time_until_next().is_none());
    ///
    /// poller.mark_polled();
    ///
    /// // Now there should be time remaining
    /// assert!(poller.time_until_next().is_some());
    /// ```
    pub fn time_until_next(&self) -> Option<Duration> {
        let elapsed = self.last_poll.elapsed();
        if elapsed >= self.interval {
            None
        } else {
            Some(self.interval - elapsed)
        }
    }

    /// Marks that a poll has been performed.
    ///
    /// Updates the last poll time to now. After calling this,
    /// `should_poll()` will return `false` until the interval elapses.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::polling::Poller;
    ///
    /// let mut poller = Poller::new();
    ///
    /// assert!(poller.should_poll());
    /// poller.mark_polled();
    /// assert!(!poller.should_poll());
    /// ```
    pub fn mark_polled(&mut self) {
        self.last_poll = Instant::now();
    }

    /// Resets the poller to allow immediate polling.
    ///
    /// This sets the last poll time to well in the past, so the next
    /// call to `should_poll()` will return `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::polling::Poller;
    /// use std::time::Duration;
    ///
    /// let mut poller = Poller::new();
    /// poller.mark_polled();
    ///
    /// // Can't poll yet
    /// assert!(!poller.should_poll());
    ///
    /// // Reset to allow immediate polling
    /// poller.reset();
    /// assert!(poller.should_poll());
    /// ```
    pub fn reset(&mut self) {
        self.last_poll = Instant::now() - self.interval * 2;
    }

    /// Returns the configured polling interval.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::polling::Poller;
    /// use std::time::Duration;
    ///
    /// let poller = Poller::with_interval(Duration::from_millis(200));
    /// assert_eq!(poller.interval(), Duration::from_millis(200));
    /// ```
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Returns the time of the last poll.
    ///
    /// # Examples
    ///
    /// ```
    /// use rightclick::tty::polling::Poller;
    ///
    /// let mut poller = Poller::new();
    /// let before = poller.last_poll();
    ///
    /// poller.mark_polled();
    ///
    /// let after = poller.last_poll();
    /// assert!(after >= before);
    /// ```
    pub fn last_poll(&self) -> Instant {
        self.last_poll
    }
}

impl Default for Poller {
    /// Creates a default poller with the default interval.
    fn default() -> Self {
        Self::new()
    }
}

/// A rate limiter for polling operations.
///
/// Similar to `Poller` but tracks consecutive polls that found no changes,
/// allowing for adaptive polling intervals (e.g., backing off when idle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdaptivePoller {
    /// Base poller for timing.
    base: Poller,

    /// Number of consecutive empty polls.
    empty_count: u32,

    /// Maximum multiplier for backoff.
    max_backoff: u32,
}

impl AdaptivePoller {
    /// Creates a new adaptive poller with default settings.
    ///
    /// Starts with the base interval and will back off up to 4x when idle.
    pub fn new() -> Self {
        Self {
            base: Poller::new(),
            empty_count: 0,
            max_backoff: 4,
        }
    }

    /// Creates a new adaptive poller with a custom base interval.
    ///
    /// # Arguments
    ///
    /// * `interval` - The base duration between polls
    pub fn with_interval(interval: Duration) -> Self {
        Self {
            base: Poller::with_interval(interval),
            empty_count: 0,
            max_backoff: 4,
        }
    }

    /// Checks if enough time has elapsed for the next poll.
    ///
    /// The effective interval increases with consecutive empty polls,
    /// up to `max_backoff` times the base interval.
    pub fn should_poll(&self) -> bool {
        let multiplier = (self.empty_count + 1).min(self.max_backoff);
        let effective_interval = self.base.interval * multiplier as u32;
        self.base.last_poll.elapsed() >= effective_interval
    }

    /// Marks that a poll has been performed.
    ///
    /// # Arguments
    ///
    /// * `had_output` - Whether the poll found new output
    pub fn mark_polled(&mut self, had_output: bool) {
        self.base.mark_polled();

        if had_output {
            self.empty_count = 0;
        } else {
            self.empty_count = (self.empty_count + 1).min(self.max_backoff * 2);
        }
    }

    /// Returns the current backoff multiplier.
    pub fn current_multiplier(&self) -> u32 {
        (self.empty_count + 1).min(self.max_backoff)
    }

    /// Resets the adaptive poller.
    pub fn reset(&mut self) {
        self.base.reset();
        self.empty_count = 0;
    }
}

impl Default for AdaptivePoller {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn new_poller_ready_to_poll() {
        let poller = Poller::new();
        assert!(poller.should_poll());
    }

    #[test]
    fn poller_with_custom_interval() {
        let interval = Duration::from_millis(200);
        let poller = Poller::with_interval(interval);

        assert_eq!(poller.interval(), interval);
        assert!(poller.should_poll());
    }

    #[test]
    fn mark_polled_prevents_immediate_poll() {
        let mut poller = Poller::new();

        assert!(poller.should_poll());
        poller.mark_polled();
        assert!(!poller.should_poll());
    }

    #[test]
    fn time_until_next_returns_some_after_poll() {
        let mut poller = Poller::with_interval(Duration::from_millis(100));

        // Initially should be None (ready to poll)
        assert!(poller.time_until_next().is_none());

        poller.mark_polled();

        // Now should have time remaining
        let remaining = poller.time_until_next();
        assert!(remaining.is_some());
        assert!(remaining.unwrap() > Duration::ZERO);
    }

    #[test]
    fn reset_allows_immediate_poll() {
        let mut poller = Poller::new();

        poller.mark_polled();
        assert!(!poller.should_poll());

        poller.reset();
        assert!(poller.should_poll());
    }

    #[test]
    fn default_poller() {
        let poller: Poller = Default::default();
        assert_eq!(
            poller.interval(),
            Duration::from_millis(DEFAULT_POLL_INTERVAL_MS)
        );
        assert!(poller.should_poll());
    }

    #[test]
    fn adaptive_poller_backoff() {
        let mut poller = AdaptivePoller::with_interval(Duration::from_millis(10));

        // Initially ready
        assert!(poller.should_poll());
        assert_eq!(poller.current_multiplier(), 1);

        // Poll with no output - multiplier increases
        poller.mark_polled(false);
        assert_eq!(poller.current_multiplier(), 2);

        poller.mark_polled(false);
        assert_eq!(poller.current_multiplier(), 3);

        // Poll with output - reset
        poller.mark_polled(true);
        assert_eq!(poller.current_multiplier(), 1);
    }

    #[test]
    fn adaptive_poller_max_backoff() {
        let mut poller = AdaptivePoller::with_interval(Duration::from_millis(1));
        poller.max_backoff = 2;

        // Exhaust backoff
        poller.mark_polled(false);
        poller.mark_polled(false);
        assert_eq!(poller.current_multiplier(), 2);

        poller.mark_polled(false);
        poller.mark_polled(false);
        assert_eq!(poller.current_multiplier(), 2); // Capped at max
    }

    #[test]
    fn last_poll_updates() {
        let mut poller = Poller::new();
        let before = poller.last_poll();

        sleep(Duration::from_millis(5));
        poller.mark_polled();

        let after = poller.last_poll();
        assert!(after > before);
    }
}
