//! Plugin and application settings modal.
//!
//! Builds a [`Modal`](crate::modal::Modal) for editing the boolean fields of
//! [`Config`](crate::core::models::Config) at runtime. The modal mirrors the
//! checkbox state in [`SettingsState`] because the generic modal framework
//! reports a toggle without identifying which checkbox flipped; we track the
//! focused id before each key press to map a toggle back to its field.
//!
//! The shell (`App`) owns a `SettingsModal` while open. On `Save` it writes the
//! resulting `Config` back to disk and applies it to the live plugins.

use crossterm::event::KeyEvent;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::core::models::{Config, Theme};
use crate::keymap::Action;
use crate::modal::{Button, Modal, ModalVariant, section, section::ButtonAction};

/// Stable checkbox ids. Kept in sync with the ids passed to [`section::checkbox`].
mod ids {
    pub const UI_SHOW_CLOCK: &str = "ui.show_clock";
    pub const UI_COMPACT_MODE: &str = "ui.compact_mode";
    pub const UI_NERD_FONTS: &str = "ui.nerd_fonts";
    pub const GIT_ENABLED: &str = "git.enabled";
    pub const GIT_SHOW_UNTRACKED: &str = "git.show_untracked";
    pub const CONVERSATIONS_ENABLED: &str = "conversations.enabled";
    pub const CONVERSATIONS_SAVE_CONTEXT: &str = "conversations.save_context";
    pub const FILE_BROWSER_ENABLED: &str = "file_browser.enabled";
    pub const FILE_BROWSER_SHOW_HIDDEN: &str = "file_browser.show_hidden";
    pub const WORKSPACE_ENABLED: &str = "workspace.enabled";
    pub const WORKERS_ENABLED: &str = "workers.enabled";
    pub const SAVE: &str = "settings.save";
    pub const CANCEL: &str = "settings.cancel";
}

/// Outcome of a key press while the settings modal is open.
#[derive(Debug, PartialEq)]
pub enum SettingsAction {
    /// User confirmed; the shell should call [`SettingsModal::into_config`],
    /// persist the result, and apply it to live plugins.
    Save,
    /// User cancelled or dismissed the modal.
    Cancel,
    /// Key was consumed by the modal (toggle, focus move); no shell action.
    Handled,
    /// Key was not consumed; the shell may forward it.
    Ignored,
}

/// Mirror of every editable boolean in the settings modal.
///
/// Field-for-field with the checkbox ids in [`ids`]. Updating this from a
/// toggle event keeps the modal state and the produced [`Config`] consistent
/// without reaching into the generic `CheckboxSection` internals.
#[derive(Debug, Clone, PartialEq)]
struct SettingsState {
    ui_show_clock: bool,
    ui_compact_mode: bool,
    ui_nerd_fonts: bool,
    git_enabled: bool,
    git_show_untracked: bool,
    conversations_enabled: bool,
    conversations_save_context: bool,
    file_browser_enabled: bool,
    file_browser_show_hidden: bool,
    workspace_enabled: bool,
    workers_enabled: bool,
}

impl SettingsState {
    /// Snapshot the editable fields from a [`Config`].
    fn from_config(config: &Config) -> Self {
        Self {
            ui_show_clock: config.ui.show_clock,
            ui_compact_mode: config.ui.compact_mode,
            ui_nerd_fonts: config.ui.nerd_fonts_enabled,
            git_enabled: config.plugins.git_status.enabled,
            git_show_untracked: config.plugins.git_status.show_untracked,
            conversations_enabled: config.plugins.conversations.enabled,
            conversations_save_context: config.plugins.conversations.save_context,
            file_browser_enabled: config.plugins.file_browser.enabled,
            file_browser_show_hidden: config.plugins.file_browser.show_hidden,
            workspace_enabled: config.plugins.workspace.enabled,
            workers_enabled: config.plugins.workers.enabled,
        }
    }

    /// Apply the mirrored state back onto a [`Config`] (cloned first by caller).
    fn apply_to(&self, config: &mut Config) {
        config.ui.show_clock = self.ui_show_clock;
        config.ui.compact_mode = self.ui_compact_mode;
        config.ui.nerd_fonts_enabled = self.ui_nerd_fonts;
        config.plugins.git_status.enabled = self.git_enabled;
        config.plugins.git_status.show_untracked = self.git_show_untracked;
        config.plugins.conversations.enabled = self.conversations_enabled;
        config.plugins.conversations.save_context = self.conversations_save_context;
        config.plugins.file_browser.enabled = self.file_browser_enabled;
        config.plugins.file_browser.show_hidden = self.file_browser_show_hidden;
        config.plugins.workspace.enabled = self.workspace_enabled;
        config.plugins.workers.enabled = self.workers_enabled;
    }

    /// Flip the boolean identified by `checkbox_id`. Returns `true` if the id
    /// matched an editable field.
    fn toggle(&mut self, checkbox_id: &str) -> bool {
        match checkbox_id {
            ids::UI_SHOW_CLOCK => self.ui_show_clock = !self.ui_show_clock,
            ids::UI_COMPACT_MODE => self.ui_compact_mode = !self.ui_compact_mode,
            ids::UI_NERD_FONTS => self.ui_nerd_fonts = !self.ui_nerd_fonts,
            ids::GIT_ENABLED => self.git_enabled = !self.git_enabled,
            ids::GIT_SHOW_UNTRACKED => self.git_show_untracked = !self.git_show_untracked,
            ids::CONVERSATIONS_ENABLED => self.conversations_enabled = !self.conversations_enabled,
            ids::CONVERSATIONS_SAVE_CONTEXT => {
                self.conversations_save_context = !self.conversations_save_context
            }
            ids::FILE_BROWSER_ENABLED => self.file_browser_enabled = !self.file_browser_enabled,
            ids::FILE_BROWSER_SHOW_HIDDEN => {
                self.file_browser_show_hidden = !self.file_browser_show_hidden
            }
            ids::WORKSPACE_ENABLED => self.workspace_enabled = !self.workspace_enabled,
            ids::WORKERS_ENABLED => self.workers_enabled = !self.workers_enabled,
            _ => return false,
        }
        true
    }
}

/// The settings modal: a thin wrapper over [`Modal`] plus mirrored state.
pub struct SettingsModal {
    modal: Modal,
    state: SettingsState,
}

impl SettingsModal {
    /// Build the modal seeded from `config`.
    pub fn from_config(config: &Config) -> Self {
        let state = SettingsState::from_config(config);
        let mut modal = Modal::new("Settings")
            .with_variant(ModalVariant::Info)
            .with_width(64)
            .with_primary_action("Save")
            .with_close_on_backdrop(true);

        // Appearance section
        modal.add_section(section::text("Appearance"));
        modal.add_section(section::checkbox(
            ids::UI_SHOW_CLOCK,
            "Show clock in header",
            state.ui_show_clock,
        ));
        modal.add_section(section::checkbox(
            ids::UI_COMPACT_MODE,
            "Compact mode (reduces padding)",
            state.ui_compact_mode,
        ));
        modal.add_section(section::checkbox(
            ids::UI_NERD_FONTS,
            "Nerd font icons",
            state.ui_nerd_fonts,
        ));
        modal.add_section(section::spacer());

        // Git Status section
        modal.add_section(section::text("Git Status"));
        modal.add_section(section::checkbox(
            ids::GIT_ENABLED,
            "Enabled",
            state.git_enabled,
        ));
        modal.add_section(section::checkbox(
            ids::GIT_SHOW_UNTRACKED,
            "Show untracked files",
            state.git_show_untracked,
        ));
        modal.add_section(section::spacer());

        // Conversations section
        modal.add_section(section::text("Conversations"));
        modal.add_section(section::checkbox(
            ids::CONVERSATIONS_ENABLED,
            "Enabled",
            state.conversations_enabled,
        ));
        modal.add_section(section::checkbox(
            ids::CONVERSATIONS_SAVE_CONTEXT,
            "Save conversation context",
            state.conversations_save_context,
        ));
        modal.add_section(section::spacer());

        // File Browser section
        modal.add_section(section::text("File Browser"));
        modal.add_section(section::checkbox(
            ids::FILE_BROWSER_ENABLED,
            "Enabled",
            state.file_browser_enabled,
        ));
        modal.add_section(section::checkbox(
            ids::FILE_BROWSER_SHOW_HIDDEN,
            "Show hidden files",
            state.file_browser_show_hidden,
        ));
        modal.add_section(section::spacer());

        // Workspace + Workers (single-line each)
        modal.add_section(section::text("Workspace"));
        modal.add_section(section::checkbox(
            ids::WORKSPACE_ENABLED,
            "Enabled",
            state.workspace_enabled,
        ));
        modal.add_section(section::spacer());

        modal.add_section(section::text("Workers"));
        modal.add_section(section::checkbox(
            ids::WORKERS_ENABLED,
            "Enabled",
            state.workers_enabled,
        ));
        modal.add_section(section::spacer());

        modal.add_section(section::buttons(vec![
            Button::primary(ids::SAVE, "Save"),
            Button::secondary(ids::CANCEL, "Cancel"),
        ]));

        Self { modal, state }
    }

    /// Handle a key event and translate it into a [`SettingsAction`].
    pub fn handle_key(&mut self, key: KeyEvent) -> SettingsAction {
        // Capture focus before the key is handled so a Toggle can be attributed
        // to the right checkbox (focus does not move during a toggle).
        let focused_before = self.modal.focused_id().map(str::to_owned);

        match self.modal.handle_key(key) {
            Some(Action::Back) => SettingsAction::Cancel,
            Some(Action::Toggle) => {
                if let Some(id) = focused_before.as_deref() {
                    self.state.toggle(id);
                }
                SettingsAction::Handled
            }
            Some(Action::Custom(action_any)) => {
                if let Some(button) = action_any.downcast_ref::<ButtonAction>() {
                    match button.button_id.as_str() {
                        ids::SAVE => SettingsAction::Save,
                        ids::CANCEL => SettingsAction::Cancel,
                        _ => SettingsAction::Handled,
                    }
                } else {
                    SettingsAction::Handled
                }
            }
            Some(_) => SettingsAction::Handled,
            None => SettingsAction::Ignored,
        }
    }

    /// Produce the final [`Config`] by applying the edited state onto `base`
    /// (the live config), preserving every non-editable field.
    pub fn into_config(self, base: &Config) -> Config {
        let mut config = base.clone();
        self.state.apply_to(&mut config);
        config
    }

    /// Render the modal.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        self.modal.render(area, buf, theme);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_state_round_trips_through_config() {
        let mut config = Config::default();
        config.ui.show_clock = false;
        config.plugins.git_status.show_untracked = false;
        config.plugins.file_browser.show_hidden = true;

        let state = SettingsState::from_config(&config);
        let mut applied = Config::default();
        state.apply_to(&mut applied);

        assert!(!applied.ui.show_clock);
        assert!(!applied.plugins.git_status.show_untracked);
        assert!(applied.plugins.file_browser.show_hidden);
        // Untouched fields keep the default.
        assert!(applied.plugins.git_status.enabled);
    }

    #[test]
    fn toggle_flips_named_field_only() {
        let mut state = SettingsState::from_config(&Config::default());
        assert!(state.git_show_untracked);
        assert!(state.toggle(ids::GIT_SHOW_UNTRACKED));
        assert!(!state.git_show_untracked);

        // Unknown id is a no-op.
        assert!(!state.toggle("nope"));
        assert!(!state.git_show_untracked);
    }

    #[test]
    fn into_config_preserves_non_editable_fields() {
        let mut base = Config::default();
        base.plugins.workspace.default_branch = "develop".to_string();
        base.ui.theme = "nord".to_string();

        let modal = SettingsModal::from_config(&base);
        let result = modal.into_config(&base);

        // Edited field reflects the snapshot (default true here).
        assert!(result.plugins.workspace.enabled);
        // Non-editable fields are preserved from the live config.
        assert_eq!(result.plugins.workspace.default_branch, "develop");
        assert_eq!(result.ui.theme, "nord");
    }

    #[test]
    fn handle_key_toggles_focused_checkbox() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        // The first focusable id is the first checkbox: ui.show_clock.
        let config = Config::default();
        assert!(config.ui.show_clock);

        let mut modal = SettingsModal::from_config(&config);
        // Sanity: focus starts on the first checkbox.
        assert_eq!(modal.modal.focused_id(), Some(super::ids::UI_SHOW_CLOCK));

        let action = modal.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(action, SettingsAction::Handled);

        // Cancel-style close is not invoked; verify the toggle landed by
        // producing the final config.
        let result = modal.into_config(&config);
        assert!(!result.ui.show_clock);
    }

    #[test]
    fn handle_key_esc_cancels() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut modal = SettingsModal::from_config(&Config::default());
        let action = modal.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(action, SettingsAction::Cancel);
    }
}
