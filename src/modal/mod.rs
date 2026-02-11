//! Modal dialog system for RightClick
//!
//! This module provides a flexible modal dialog system for building
//! interactive dialogs and prompts in the TUI interface.
//!
//! # Architecture
//!
//! The modal system consists of:
//!
//! - [`Modal`] - The main modal widget that contains sections and manages focus
//! - [`ModalVariant`] - Visual styling variants (Default, Danger, Warning, Info, Success)
//! - [`Section`] - Trait for modal content sections
//! - [`Button`], [`ButtonVariant`] - Button definitions for button sections
//! - [`Checkbox`] - Checkbox state for checkbox sections
//!
//! # Quick Start
//!
//! ```rust
//! use rightclick::modal::{Modal, ModalVariant, section, Button};
//!
//! // Create a confirmation dialog
//! let mut modal = Modal::new("Confirm Delete")
//!     .with_variant(ModalVariant::Danger)
//!     .with_width(50);
//!
//! modal.add_section(section::text("Are you sure you want to delete this file?"));
//! modal.add_section(section::spacer());
//! modal.add_section(section::buttons(vec![
//!     Button::danger("delete", "Delete"),
//!     Button::secondary("cancel", "Cancel"),
//! ]));
//!
//! // In your render loop:
//! // modal.render(area, buf, &theme);
//!
//! // In your input handler:
//! // if let Some(action) = modal.handle_key(key) {
//! //     match action { ... }
//! // }
//! ```
//!
//! # Sections
//!
//! Sections are composable building blocks for modal content:
//!
//! - **TextSection** - Static text content
//! - **SpacerSection** - Vertical padding
//! - **ButtonSection** - Row of buttons
//! - **CheckboxSection** - Single checkbox
//!
//! Custom sections can be created by implementing the [`Section`] trait.

pub mod modal;
pub mod options;
pub mod section;

// Re-export main types
pub use modal::{Modal, ModalVariant};
pub use options::{Button, ButtonVariant, Checkbox};
pub use section::{ButtonAction, Section};

/// Convenience re-export of section constructors
pub mod section_helpers {
    pub use super::section::{buttons, checkbox, spacer, spacer_lines, text};
}

// Deprecated: Use `section::text` etc. directly
/// Deprecated: Use `section::text` directly
#[deprecated(since = "0.1.0", note = "Use `section::text` directly")]
pub fn text<T: Into<String>>(text: T) -> Box<dyn Section> {
    section::text(text)
}

/// Deprecated: Use `section::spacer` directly
#[deprecated(since = "0.1.0", note = "Use `section::spacer` directly")]
pub fn spacer() -> Box<dyn Section> {
    section::spacer()
}

/// Deprecated: Use `section::buttons` directly
#[deprecated(since = "0.1.0", note = "Use `section::buttons` directly")]
pub fn buttons(btns: Vec<Button>) -> Box<dyn Section> {
    section::buttons(btns)
}

/// Deprecated: Use `section::checkbox` directly
#[deprecated(since = "0.1.0", note = "Use `section::checkbox` directly")]
pub fn checkbox(id: &str, label: &str, checked: bool) -> Box<dyn Section> {
    section::checkbox(id, label, checked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modal_creation() {
        let modal = Modal::new("Test")
            .with_variant(ModalVariant::Info)
            .with_width(40);

        assert_eq!(modal.title, "Test");
        assert_eq!(modal.variant, ModalVariant::Info);
        assert_eq!(modal.width, 40);
    }

    #[test]
    fn button_creation() {
        let primary = Button::primary("ok", "OK");
        assert_eq!(primary.variant, ButtonVariant::Primary);

        let secondary = Button::secondary("cancel", "Cancel");
        assert_eq!(secondary.variant, ButtonVariant::Secondary);

        let danger = Button::danger("delete", "Delete");
        assert_eq!(danger.variant, ButtonVariant::Danger);
    }

    #[test]
    fn checkbox_creation() {
        let cb = Checkbox::new("remember", "Remember me", true);
        assert_eq!(cb.id, "remember");
        assert_eq!(cb.label, "Remember me");
        assert!(cb.checked);
    }

    #[test]
    fn section_convenience_functions() {
        let _text = section::text("Hello");
        let _spacer = section::spacer();
        let _buttons = section::buttons(vec![Button::primary("ok", "OK")]);
        let _checkbox = section::checkbox("test", "Test", false);
    }

    #[test]
    fn modal_with_sections() {
        let mut modal = Modal::new("Test Modal");
        modal.add_section(section::text("Line 1"));
        modal.add_section(section::spacer());
        modal.add_section(section::text("Line 2"));

        assert_eq!(modal.sections.len(), 3);
    }

    #[test]
    fn modal_variants() {
        let variants = vec![
            ModalVariant::Default,
            ModalVariant::Danger,
            ModalVariant::Warning,
            ModalVariant::Info,
            ModalVariant::Success,
        ];

        for variant in variants {
            let modal = Modal::new("Test").with_variant(variant);
            assert_eq!(modal.variant, variant);
        }
    }

    #[test]
    fn button_action_extract() {
        let action = section::ButtonAction {
            button_id: "confirm".to_string(),
        };
        assert_eq!(action.button_id, "confirm");
    }
}
