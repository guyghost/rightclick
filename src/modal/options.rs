//! Modal UI options (buttons, checkboxes, etc.)
//!
//! This module provides reusable UI components that can be used within
//! modal sections. These are primarily used for user interaction elements
//! like buttons and checkboxes.

/// Button variant for styling
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ButtonVariant {
    /// Primary action button (accent color, most prominent)
    Primary,
    /// Secondary action button (neutral, less prominent)
    Secondary,
    /// Danger button for destructive actions (red/error color)
    Danger,
}

impl ButtonVariant {
    /// Returns the color key for this button variant
    pub fn color_key(&self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "muted",
            Self::Danger => "error",
        }
    }

    /// Returns a label for this variant (useful for styling)
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Danger => "danger",
        }
    }
}

impl Default for ButtonVariant {
    fn default() -> Self {
        Self::Secondary
    }
}

/// A button definition for modal button sections
#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    /// Unique identifier for this button (used for focus and actions)
    pub id: String,
    /// Display label for the button
    pub label: String,
    /// Visual variant for styling
    pub variant: ButtonVariant,
}

impl Button {
    /// Creates a new button
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the button
    /// * `label` - Display label
    /// * `variant` - Visual style variant
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::{Button, ButtonVariant};
    ///
    /// let btn = Button::new("confirm", "Confirm", ButtonVariant::Primary);
    /// ```
    pub fn new<I: Into<String>, L: Into<String>>(id: I, label: L, variant: ButtonVariant) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant,
        }
    }

    /// Creates a primary button
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::Button;
    ///
    /// let btn = Button::primary("save", "Save");
    /// ```
    pub fn primary<I: Into<String>, L: Into<String>>(id: I, label: L) -> Self {
        Self::new(id, label, ButtonVariant::Primary)
    }

    /// Creates a secondary button
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::Button;
    ///
    /// let btn = Button::secondary("cancel", "Cancel");
    /// ```
    pub fn secondary<I: Into<String>, L: Into<String>>(id: I, label: L) -> Self {
        Self::new(id, label, ButtonVariant::Secondary)
    }

    /// Creates a danger button
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::Button;
    ///
    /// let btn = Button::danger("delete", "Delete");
    /// ```
    pub fn danger<I: Into<String>, L: Into<String>>(id: I, label: L) -> Self {
        Self::new(id, label, ButtonVariant::Danger)
    }
}

impl Default for Button {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            variant: ButtonVariant::default(),
        }
    }
}

/// A checkbox state for modal checkbox sections
#[derive(Clone, Debug, PartialEq)]
pub struct Checkbox {
    /// Unique identifier for this checkbox
    pub id: String,
    /// Display label
    pub label: String,
    /// Current checked state
    pub checked: bool,
}

impl Checkbox {
    /// Creates a new checkbox
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier
    /// * `label` - Display label
    /// * `checked` - Initial checked state
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::modal::Checkbox;
    ///
    /// let cb = Checkbox::new("remember", "Remember me", false);
    /// ```
    pub fn new<I: Into<String>, L: Into<String>>(id: I, label: L, checked: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            checked,
        }
    }

    /// Toggles the checked state
    pub fn toggle(&mut self) {
        self.checked = !self.checked;
    }

    /// Returns the checked symbol for rendering
    pub fn symbol(&self) -> &'static str {
        if self.checked { "[x]" } else { "[ ]" }
    }
}

impl Default for Checkbox {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            checked: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_variant_color_key() {
        assert_eq!(ButtonVariant::Primary.color_key(), "primary");
        assert_eq!(ButtonVariant::Secondary.color_key(), "muted");
        assert_eq!(ButtonVariant::Danger.color_key(), "error");
    }

    #[test]
    fn button_new() {
        let btn = Button::new("test", "Test Label", ButtonVariant::Primary);
        assert_eq!(btn.id, "test");
        assert_eq!(btn.label, "Test Label");
        assert_eq!(btn.variant, ButtonVariant::Primary);
    }

    #[test]
    fn button_convenience_constructors() {
        let primary = Button::primary("save", "Save");
        assert_eq!(primary.variant, ButtonVariant::Primary);

        let secondary = Button::secondary("cancel", "Cancel");
        assert_eq!(secondary.variant, ButtonVariant::Secondary);

        let danger = Button::danger("delete", "Delete");
        assert_eq!(danger.variant, ButtonVariant::Danger);
    }

    #[test]
    fn checkbox_new() {
        let cb = Checkbox::new("test", "Test Label", true);
        assert_eq!(cb.id, "test");
        assert_eq!(cb.label, "Test Label");
        assert!(cb.checked);
    }

    #[test]
    fn checkbox_toggle() {
        let mut cb = Checkbox::new("test", "Test", false);
        assert!(!cb.checked);
        cb.toggle();
        assert!(cb.checked);
        cb.toggle();
        assert!(!cb.checked);
    }

    #[test]
    fn checkbox_symbol() {
        let checked = Checkbox::new("test", "Test", true);
        assert_eq!(checked.symbol(), "[x]");

        let unchecked = Checkbox::new("test", "Test", false);
        assert_eq!(unchecked.symbol(), "[ ]");
    }

    #[test]
    fn button_default() {
        let btn: Button = Default::default();
        assert!(btn.id.is_empty());
        assert!(btn.label.is_empty());
        assert_eq!(btn.variant, ButtonVariant::Secondary);
    }

    #[test]
    fn checkbox_default() {
        let cb: Checkbox = Default::default();
        assert!(cb.id.is_empty());
        assert!(cb.label.is_empty());
        assert!(!cb.checked);
    }
}
