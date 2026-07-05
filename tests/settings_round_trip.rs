//! Integration test: editing settings via the modal persists through config.
//!
//! Exercises the full shell-side flow without the terminal: build a modal from
//! a live config, simulate toggling a checkbox, produce the new config, save it
//! to disk, and reload it to confirm the change survived a round-trip.

use rightclick::config::{load_from, save_to};
use rightclick::core::models::Config;
use rightclick::settings::{SettingsAction, SettingsModal};

#[test]
fn settings_modal_edit_persists_through_config_round_trip() {
    // Simulate the live config held by the App.
    let mut live = Config::default();
    live.plugins.file_browser.show_hidden = false;
    let base = live.clone();

    // Open the settings modal seeded from the live config.
    let mut modal = SettingsModal::from_config(&base);
    // Focus starts on the first checkbox (ui.show_clock); tab down to the
    // file browser "show hidden" checkbox and flip it. The checkbox ids are
    // stable, so we navigate by counting Tab presses to land on it.
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    for _ in 0..8 {
        modal.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    }
    // Toggle the now-focused file browser show_hidden checkbox.
    assert_eq!(
        modal.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        SettingsAction::Handled
    );

    // The shell would call into_config on Save; do it directly here.
    let edited = modal.into_config(&base);
    assert!(edited.plugins.file_browser.show_hidden);

    // Persist and reload: the edited value must survive.
    let dir = tempfile::TempDir::new().expect("temp dir");
    let path = dir.path().join("config.json");
    save_to(&edited, &path).expect("save edited config");
    let reloaded = load_from(&path).expect("reload config");

    assert!(reloaded.plugins.file_browser.show_hidden);
    // Non-edited fields are unchanged.
    assert_eq!(
        reloaded.plugins.git_status.enabled,
        base.plugins.git_status.enabled
    );
}
