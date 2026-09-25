#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileChange {
    Unchanged,
    Reload,
    Conflict,
    Missing,
    ReadError,
}

pub(crate) struct FileSyncState {
    saved_text: String,
    disk_fingerprint: Option<u64>,
    conflicted: bool,
}

impl FileSyncState {
    pub(crate) fn new(saved_text: String, disk_fingerprint: u64) -> Self {
        Self {
            saved_text,
            disk_fingerprint: Some(disk_fingerprint),
            conflicted: false,
        }
    }

    pub(crate) fn is_clean(&self, current_text: &str) -> bool {
        current_text == self.saved_text
    }

    pub(crate) fn is_conflicted(&self) -> bool {
        self.conflicted
    }

    pub(crate) fn disk_fingerprint(&self) -> Option<u64> {
        self.disk_fingerprint
    }

    pub(crate) fn observe(
        &mut self,
        current_text: &str,
        result: Result<Option<u64>, String>,
    ) -> FileChange {
        let fingerprint = match result {
            Ok(fingerprint) => fingerprint,
            Err(_) => return FileChange::ReadError,
        };

        if let Some(fingerprint) = fingerprint {
            if self.disk_fingerprint == Some(fingerprint) {
                return FileChange::Unchanged;
            }

            self.disk_fingerprint = Some(fingerprint);
            if self.is_clean(current_text) && !self.conflicted {
                FileChange::Reload
            } else {
                self.conflicted = true;
                FileChange::Conflict
            }
        } else {
            self.disk_fingerprint = None;
            FileChange::Missing
        }
    }

    pub(crate) fn mark_reloaded(&mut self, text: String, fingerprint: u64) {
        self.saved_text = text;
        self.disk_fingerprint = Some(fingerprint);
        self.conflicted = false;
    }

    pub(crate) fn mark_saved(&mut self, written_text: String, fingerprint: u64) {
        self.saved_text = written_text;
        self.disk_fingerprint = Some(fingerprint);
        self.conflicted = false;
    }
}

pub(crate) fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

pub(crate) fn generation_is_current(event_generation: u64, active_generation: u64) -> bool {
    event_generation == active_generation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_bytes_reload_clean_text_and_flag_dirty_text_as_conflict() {
        let mut clean = FileSyncState::new("saved".into(), 10);
        assert_eq!(clean.observe("saved", Ok(Some(20))), FileChange::Reload);

        let mut dirty = FileSyncState::new("saved".into(), 10);
        assert_eq!(
            dirty.observe("local edit", Ok(Some(20))),
            FileChange::Conflict
        );
        assert!(dirty.is_conflicted());
        assert_eq!(dirty.disk_fingerprint(), Some(20));
    }

    #[test]
    fn conflict_persists_after_undo_and_tracks_the_latest_disk_fingerprint() {
        let mut state = FileSyncState::new("saved".into(), 10);
        assert_eq!(
            state.observe("local edit", Ok(Some(20))),
            FileChange::Conflict
        );
        assert_eq!(state.observe("saved", Ok(Some(30))), FileChange::Conflict);
        assert!(state.is_conflicted());
        assert_eq!(state.disk_fingerprint(), Some(30));
    }

    #[test]
    fn missing_file_and_read_error_do_not_discard_the_saved_baseline() {
        let mut state = FileSyncState::new("saved".into(), 10);
        assert_eq!(state.observe("local edit", Ok(None)), FileChange::Missing);
        assert_eq!(
            state.observe("local edit", Err("permission denied".into())),
            FileChange::ReadError
        );
        assert_eq!(state.disk_fingerprint(), None);
        assert!(!state.is_clean("local edit"));
    }

    #[test]
    fn deletion_keeps_local_text_and_does_not_create_a_new_conflict() {
        let mut state = FileSyncState::new("saved".into(), 10);
        assert_eq!(state.observe("local edit", Ok(None)), FileChange::Missing);
        assert_eq!(state.disk_fingerprint(), None);
        assert!(!state.is_conflicted());
        assert!(!state.is_clean("local edit"));
    }

    #[test]
    fn deletion_does_not_clear_an_existing_conflict() {
        let mut state = FileSyncState::new("saved".into(), 10);
        assert_eq!(
            state.observe("local edit", Ok(Some(20))),
            FileChange::Conflict
        );
        assert_eq!(state.observe("local edit", Ok(None)), FileChange::Missing);
        assert!(state.is_conflicted());
    }

    #[test]
    fn saving_a_snapshot_keeps_later_edits_dirty() {
        let mut state = FileSyncState::new("before".into(), 10);
        state.mark_saved("written during save".into(), 20);
        assert!(state.is_clean("written during save"));
        assert!(!state.is_clean("typed while save ran"));
    }

    #[test]
    fn identical_decoded_text_with_new_disk_bytes_requests_reload() {
        let mut state = FileSyncState::new("same text".into(), 10);
        assert_eq!(state.observe("same text", Ok(Some(11))), FileChange::Reload);
    }

    #[test]
    fn stale_generation_does_not_match_the_active_document() {
        assert!(!generation_is_current(4, 5));
        assert!(generation_is_current(5, 5));
    }

    #[test]
    fn watcher_event_for_own_save_is_ignored() {
        let mut state = FileSyncState::new("before save".into(), 10);
        state.mark_saved("written text".into(), 20);
        assert_eq!(
            state.observe("written text", Ok(Some(20))),
            FileChange::Unchanged
        );
    }

    #[test]
    fn reload_replaces_the_saved_baseline_and_clears_conflict() {
        let mut state = FileSyncState::new("saved".into(), 10);
        assert_eq!(
            state.observe("local edit", Ok(Some(20))),
            FileChange::Conflict
        );
        state.mark_reloaded("disk version".into(), 20);
        assert!(!state.is_conflicted());
        assert!(state.is_clean("disk version"));
    }
}
