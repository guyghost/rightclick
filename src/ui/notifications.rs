//! Notification system for RightClick
//!
//! Provides `NotificationManager` for managing toast-style notifications
//! and a rendering function for displaying them as overlays.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};
use std::collections::VecDeque;
use std::str::FromStr;
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

use crate::core::models::Theme;

/// Notification severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    /// Informational message
    Info,
    /// Success message
    Success,
    /// Warning message
    Warning,
    /// Error message
    Error,
}

impl NotificationLevel {
    /// Get the display icon for this level
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Info => "ℹ",
            Self::Success => "✓",
            Self::Warning => "⚠",
            Self::Error => "✗",
        }
    }

    /// Get the theme color key for this level
    pub fn color_key(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// A single notification
#[derive(Debug, Clone)]
pub struct Notification {
    /// The message text
    pub message: String,
    /// Severity level
    pub level: NotificationLevel,
    /// When this notification was created
    pub created_at: Instant,
    /// How long to display (None = default 3s)
    pub duration: Duration,
}

impl Notification {
    /// Create a new notification
    pub fn new(message: impl Into<String>, level: NotificationLevel) -> Self {
        Self {
            message: message.into(),
            level,
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        }
    }

    /// Set custom duration
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Check if this notification has expired
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= self.duration
    }

    /// Get the remaining time ratio (1.0 = full, 0.0 = expired)
    pub fn remaining_ratio(&self) -> f64 {
        let elapsed = self.created_at.elapsed().as_secs_f64();
        let total = self.duration.as_secs_f64();
        (1.0 - elapsed / total).max(0.0)
    }
}

/// Manages a queue of notifications with auto-expiration
#[derive(Debug)]
pub struct NotificationManager {
    /// Active notifications
    notifications: VecDeque<Notification>,
    /// Maximum visible notifications at once
    max_visible: usize,
}

impl NotificationManager {
    /// Create a new notification manager
    pub fn new() -> Self {
        Self {
            notifications: VecDeque::new(),
            max_visible: 5,
        }
    }

    /// Set maximum visible notifications
    pub fn with_max_visible(mut self, max: usize) -> Self {
        self.max_visible = max;
        self
    }

    /// Add an info notification
    pub fn info(&mut self, message: impl Into<String>) {
        self.push(Notification::new(message, NotificationLevel::Info));
    }

    /// Add a success notification
    pub fn success(&mut self, message: impl Into<String>) {
        self.push(Notification::new(message, NotificationLevel::Success));
    }

    /// Add a warning notification
    pub fn warning(&mut self, message: impl Into<String>) {
        self.push(Notification::new(message, NotificationLevel::Warning));
    }

    /// Add an error notification
    pub fn error(&mut self, message: impl Into<String>) {
        self.push(Notification::new(message, NotificationLevel::Error));
    }

    /// Push a notification onto the queue
    pub fn push(&mut self, notification: Notification) {
        self.notifications.push_back(notification);
        // Trim to max
        while self.notifications.len() > self.max_visible {
            self.notifications.pop_front();
        }
    }

    /// Remove expired notifications and return the active ones
    pub fn active(&mut self) -> Vec<&Notification> {
        // Remove expired
        self.notifications.retain(|n| !n.is_expired());
        // Return active (up to max_visible)
        self.notifications.iter().take(self.max_visible).collect()
    }

    /// Get the count of active notifications
    pub fn active_count(&mut self) -> usize {
        self.notifications.retain(|n| !n.is_expired());
        self.notifications.len().min(self.max_visible)
    }

    /// Check if there are any active notifications
    pub fn has_active(&mut self) -> bool {
        self.notifications.retain(|n| !n.is_expired());
        !self.notifications.is_empty()
    }

    /// Clear all notifications
    pub fn clear(&mut self) {
        self.notifications.clear();
    }

    /// Render notifications as toast overlays in the bottom-right corner
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let notifications: Vec<Notification> = {
            self.notifications.retain(|n| !n.is_expired());
            self.notifications
                .iter()
                .take(self.max_visible)
                .cloned()
                .collect()
        };

        if notifications.is_empty() {
            return;
        }

        let toast_width: u16 = 40;
        let toast_height: u16 = 3;

        for (i, notification) in notifications.iter().enumerate() {
            let x = area.width.saturating_sub(toast_width + 1);
            let y = area
                .height
                .saturating_sub((i as u16 + 1) * (toast_height + 1) + 1);

            if y < area.y || x < area.x {
                break;
            }

            let toast_area = Rect {
                x: area.x + x,
                y: area.y + y,
                width: toast_width.min(area.width - x),
                height: toast_height,
            };

            render_toast(toast_area, buf, notification, theme);
        }
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Render a single toast notification
fn render_toast(area: Rect, buf: &mut Buffer, notification: &Notification, theme: &Theme) {
    let color = match notification.level {
        NotificationLevel::Info => Color::from_str(&theme.colors.info).unwrap_or(Color::Cyan),
        NotificationLevel::Success => {
            Color::from_str(&theme.colors.success).unwrap_or(Color::Green)
        }
        NotificationLevel::Warning => {
            Color::from_str(&theme.colors.warning).unwrap_or(Color::Yellow)
        }
        NotificationLevel::Error => Color::from_str(&theme.colors.error).unwrap_or(Color::Red),
    };

    let bg = Color::from_str(&theme.colors.background).unwrap_or(Color::Black);

    // Clear the area first
    Clear.render(area, buf);

    // Render bordered block
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color))
        .style(Style::default().bg(bg));

    let inner = block.inner(area);
    block.render(area, buf);

    // Render icon + message
    if inner.width > 0 && inner.height > 0 {
        let icon = notification.level.icon();
        let max_msg_width = (inner.width as usize).saturating_sub(UnicodeWidthStr::width(icon) + 1);
        let msg = truncate_notification_message(&notification.message, max_msg_width);

        let line = Line::from(vec![
            Span::styled(
                format!("{} ", icon),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                msg,
                Style::default()
                    .fg(Color::from_str(&theme.colors.foreground).unwrap_or(Color::White)),
            ),
        ]);

        Paragraph::new(line).render(inner, buf);
    }
}

fn truncate_notification_message(message: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    if UnicodeWidthStr::width(message) <= max_width {
        return message.to_string();
    }

    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let mut output = String::new();
    let mut width = 0;
    for ch in message.chars() {
        let ch_width = UnicodeWidthStr::width(ch.to_string().as_str());
        if width + ch_width + 3 > max_width {
            break;
        }
        output.push(ch);
        width += ch_width;
    }
    output.push_str("...");
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_new() {
        let n = Notification::new("test message", NotificationLevel::Info);
        assert_eq!(n.message, "test message");
        assert_eq!(n.level, NotificationLevel::Info);
        assert!(!n.is_expired());
    }

    #[test]
    fn notification_with_duration() {
        let n = Notification::new("test", NotificationLevel::Success)
            .with_duration(Duration::from_secs(10));
        assert_eq!(n.duration, Duration::from_secs(10));
    }

    #[test]
    fn notification_expiration() {
        let n = Notification::new("test", NotificationLevel::Warning)
            .with_duration(Duration::from_millis(0));
        std::thread::sleep(Duration::from_millis(1));
        assert!(n.is_expired());
    }

    #[test]
    fn notification_remaining_ratio() {
        let n = Notification::new("test", NotificationLevel::Info)
            .with_duration(Duration::from_secs(10));
        let ratio = n.remaining_ratio();
        assert!(ratio > 0.9); // Should be close to 1.0
    }

    #[test]
    fn notification_level_icons() {
        assert_eq!(NotificationLevel::Info.icon(), "ℹ");
        assert_eq!(NotificationLevel::Success.icon(), "✓");
        assert_eq!(NotificationLevel::Warning.icon(), "⚠");
        assert_eq!(NotificationLevel::Error.icon(), "✗");
    }

    #[test]
    fn notification_level_color_keys() {
        assert_eq!(NotificationLevel::Info.color_key(), "info");
        assert_eq!(NotificationLevel::Success.color_key(), "success");
        assert_eq!(NotificationLevel::Warning.color_key(), "warning");
        assert_eq!(NotificationLevel::Error.color_key(), "error");
    }

    #[test]
    fn manager_new() {
        let manager = NotificationManager::new();
        assert_eq!(manager.max_visible, 5);
        assert!(manager.notifications.is_empty());
    }

    #[test]
    fn manager_push_and_active() {
        let mut manager = NotificationManager::new();
        manager.info("Info message");
        manager.success("Success message");
        manager.warning("Warning message");
        manager.error("Error message");

        let active = manager.active();
        assert_eq!(active.len(), 4);
    }

    #[test]
    fn manager_max_visible() {
        let mut manager = NotificationManager::new().with_max_visible(2);
        manager.info("First");
        manager.info("Second");
        manager.info("Third");

        let active = manager.active();
        assert_eq!(active.len(), 2);
        // First should have been dropped
        assert_eq!(active[0].message, "Second");
        assert_eq!(active[1].message, "Third");
    }

    #[test]
    fn manager_auto_expiration() {
        let mut manager = NotificationManager::new();
        let notification = Notification::new("expires", NotificationLevel::Info)
            .with_duration(Duration::from_millis(0));
        manager.push(notification);
        std::thread::sleep(Duration::from_millis(1));

        let active = manager.active();
        assert!(active.is_empty());
    }

    #[test]
    fn manager_clear() {
        let mut manager = NotificationManager::new();
        manager.info("test");
        manager.clear();
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn manager_has_active() {
        let mut manager = NotificationManager::new();
        assert!(!manager.has_active());
        manager.info("test");
        assert!(manager.has_active());
    }

    #[test]
    fn render_toasts_no_panic() {
        let mut manager = NotificationManager::new();
        manager.info("Info toast");
        manager.error("Error toast");

        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        manager.render(Rect::new(0, 0, 80, 24), &mut buf, &theme);
        // Just verify no panic
    }

    #[test]
    fn render_toasts_small_area() {
        let mut manager = NotificationManager::new();
        manager.info("test");

        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
        manager.render(Rect::new(0, 0, 10, 5), &mut buf, &theme);
        // Verify no panic with small area
    }

    #[test]
    fn truncate_notification_message_handles_unicode_boundaries() {
        assert_eq!(truncate_notification_message("éclair session", 5), "éc...");
        assert_eq!(truncate_notification_message("ok", 5), "ok");
        assert_eq!(truncate_notification_message("abcdef", 2), "..");
    }

    #[test]
    fn render_toast_handles_unicode_message() {
        let notification = Notification::new(
            "éclair session opened from workspace",
            NotificationLevel::Info,
        );
        let theme = Theme::default();
        let area = Rect::new(0, 0, 20, 3);
        let mut buf = Buffer::empty(area);

        render_toast(area, &mut buf, &notification, &theme);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("éc"));
    }
}
