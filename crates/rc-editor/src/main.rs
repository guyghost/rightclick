//! RightClick — coquille graphique (phase 1).
//!
//! L'éditeur utilise le widget `text_editor` d'iced comme surface de
//! frappe ; les crates du cœur (`rc-buffer`, `rc-text`) interviennent
//! déjà pour la normalisation des documents (détection des fins de ligne)
//! et l'écriture disque. L'intégration complète (rendu custom piloté par
//! `rc-buffer` + parsing incrémental) est la phase 2 — voir README.md.

use std::path::PathBuf;

use iced::widget::{button, column, row, text, text_editor};
use iced::{Center, Element, Fill, Font, Subscription, Task, Theme};

use rc_buffer::Buffer;
use rc_document::{Document, FileFormat};
use rc_text::{normalize_newlines, Newline};

mod file_state;
mod file_watcher;

fn main() -> iced::Result {
    iced::application("RightClick", RightClick::update, RightClick::view)
        .theme(|_: &RightClick| Theme::Dark)
        .subscription(RightClick::subscription)
        .centered()
        .run_with(RightClick::new)
}

struct RightClick {
    content: text_editor::Content,
    path: Option<PathBuf>,
    revision: u64,
    saving: bool,
    saved: bool,
    // cosmic-text omits the terminal line ending from `Content::lines()`.
    trailing_newline_suffix: bool,
    watch_generation: u64,
    open_request: u64,
    reload_request: Option<u64>,
    snapshot_request: u64,
    format: Option<FileFormat>,
    file_state: Option<file_state::FileSyncState>,
    watch_error: Option<String>,
    file_error: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    Open,
    FileOpened {
        request_id: u64,
        result: Result<LoadedFile, Error>,
    },
    Save,
    Overwrite,
    ReloadFromDisk,
    Watch(file_watcher::WatchSignal),
    DiskSnapshot {
        generation: u64,
        request_id: u64,
        result: DiskSnapshotResult,
    },
    FileSaved {
        generation: u64,
        result: Result<SaveOutcome, Error>,
    },
}

#[derive(Debug, Clone)]
struct LoadedFile {
    path: PathBuf,
    contents: String,
    format: FileFormat,
    fingerprint: u64,
}

#[derive(Debug, Clone)]
struct SavedFile {
    path: PathBuf,
    written_text: String,
    fingerprint: u64,
}

#[derive(Debug, Clone)]
enum SaveOutcome {
    Saved(SavedFile),
    Conflict(LoadedFile),
}

#[derive(Debug, Clone)]
enum DiskSnapshotResult {
    Present(LoadedFile),
    Missing,
    ReadError(String),
}

#[derive(Debug, Clone)]
struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl RightClick {
    fn new() -> (Self, Task<Message>) {
        let mut init = Self {
            content: text_editor::Content::new(),
            path: None,
            revision: 0,
            saving: false,
            saved: false,
            trailing_newline_suffix: false,
            watch_generation: 0,
            open_request: 0,
            reload_request: None,
            snapshot_request: 0,
            format: None,
            file_state: None,
            watch_error: None,
            file_error: None,
        };

        // `rightclick chemin/vers/fichier` ouvre directement le document
        let task = match std::env::args().nth(1).map(PathBuf::from) {
            Some(path) => {
                init.open_request = 1;
                Task::perform(load_file(path), |result| Message::FileOpened {
                    request_id: 1,
                    result,
                })
            }
            None => Task::none(),
        };

        (init, task)
    }

    fn subscription(&self) -> Subscription<Message> {
        match &self.path {
            Some(path) => {
                file_watcher::subscription(path.clone(), self.watch_generation).map(Message::Watch)
            }
            None => Subscription::none(),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(action) => {
                let action = match action {
                    text_editor::Action::Edit(text_editor::Edit::Paste(pasted)) => {
                        text_editor::Action::Edit(text_editor::Edit::Paste(std::sync::Arc::new(
                            normalize_newlines(&pasted, Newline::Lf),
                        )))
                    }
                    action => action,
                };
                let edited = action.is_edit();
                let cursor_at_end = self.cursor_at_document_end();
                let replaces_all = self.selection_covers_document();
                let pasted_final_newline = match &action {
                    text_editor::Action::Edit(text_editor::Edit::Paste(pasted)) => {
                        Some(pasted.ends_with('\n'))
                    }
                    _ => None,
                };
                let previous_trailing_newline_suffix = self.trailing_newline_suffix;
                let trailing_newline_suffix_after = match &action {
                    text_editor::Action::Edit(text_editor::Edit::Enter)
                    | text_editor::Action::Edit(text_editor::Edit::Insert('\n'))
                        if replaces_all =>
                    {
                        Some(false)
                    }
                    text_editor::Action::Edit(text_editor::Edit::Insert(_)) if replaces_all => {
                        Some(false)
                    }
                    text_editor::Action::Edit(
                        text_editor::Edit::Backspace | text_editor::Edit::Delete,
                    ) if replaces_all => Some(false),
                    text_editor::Action::Edit(text_editor::Edit::Delete)
                        if cursor_at_end && self.trailing_newline_suffix =>
                    {
                        Some(false)
                    }
                    _ => None,
                };
                self.content.perform(action);
                if let Some(trailing_newline_suffix) = trailing_newline_suffix_after {
                    self.trailing_newline_suffix = trailing_newline_suffix;
                }
                if let Some(pasted_final_newline) = pasted_final_newline {
                    let editor_has_final_newline = self.editor_text().ends_with('\n');
                    if replaces_all {
                        self.trailing_newline_suffix =
                            pasted_final_newline && !editor_has_final_newline;
                    } else if cursor_at_end && pasted_final_newline {
                        self.trailing_newline_suffix =
                            previous_trailing_newline_suffix || !editor_has_final_newline;
                    }
                }
                if edited {
                    self.revision += 1;
                    self.saved = self
                        .file_state
                        .as_ref()
                        .is_some_and(|state| state.is_clean(&self.current_text()));
                }
                Task::none()
            }
            Message::Open => self.begin_open_dialog(),
            Message::FileOpened { request_id, result } => {
                if request_id != self.open_request {
                    return Task::none();
                }
                self.reload_request = None;
                match result {
                    Ok(loaded) => {
                        self.install_loaded(loaded);
                        self.request_snapshot(self.watch_generation)
                    }
                    Err(error) => {
                        self.file_error = Some(error.0.clone());
                        eprintln!("Ouverture impossible : {error}");
                        Task::none()
                    }
                }
            }
            Message::Save => self.start_save(false),
            Message::Overwrite => self.start_save(true),
            Message::ReloadFromDisk => self.begin_reload(),
            Message::Watch(signal) => match signal {
                file_watcher::WatchSignal::Changed { generation }
                    if file_state::generation_is_current(generation, self.watch_generation) =>
                {
                    self.request_snapshot(generation)
                }
                file_watcher::WatchSignal::Unavailable {
                    generation,
                    message,
                } if file_state::generation_is_current(generation, self.watch_generation) => {
                    self.watch_error = Some(message);
                    Task::none()
                }
                _ => Task::none(),
            },
            Message::DiskSnapshot {
                generation,
                request_id,
                result,
            } => {
                if !file_state::generation_is_current(generation, self.watch_generation)
                    || request_id != self.snapshot_request
                {
                    return Task::none();
                }
                self.apply_disk_snapshot(result);
                Task::none()
            }
            Message::FileSaved { generation, result } => {
                if !file_state::generation_is_current(generation, self.watch_generation) {
                    return Task::none();
                }
                self.saving = false;
                match result {
                    Err(error) => {
                        self.file_error = Some(error.0.clone());
                        eprintln!("Enregistrement impossible : {error}");
                    }
                    Ok(SaveOutcome::Saved(saved)) => {
                        self.invalidate_snapshots();
                        let current_text = self.current_text();
                        if let Some(state) = self.file_state.as_mut() {
                            state.mark_saved(saved.written_text, saved.fingerprint);
                            self.saved = state.is_clean(&current_text);
                        }
                        self.file_error = None;
                        println!("Enregistré : {}", saved.path.display());
                    }
                    Ok(SaveOutcome::Conflict(loaded)) => {
                        let current_text = self.current_text();
                        let change = self.file_state.as_mut().map(|state| {
                            state.observe(&current_text, Ok(Some(loaded.fingerprint)))
                        });
                        match change {
                            Some(file_state::FileChange::Reload) => self.replace_from_disk(loaded),
                            Some(file_state::FileChange::Unchanged) if !self.is_conflicted() => {
                                self.invalidate_snapshots();
                                self.file_error = None;
                            }
                            Some(file_state::FileChange::Conflict)
                            | Some(file_state::FileChange::Unchanged)
                            | Some(file_state::FileChange::Missing)
                            | Some(file_state::FileChange::ReadError)
                            | None => {
                                self.invalidate_snapshots();
                                self.file_error = Some("Le fichier a changé sur le disque.".into());
                            }
                        }
                    }
                }
                Task::none()
            }
        }
    }

    fn begin_open_dialog(&mut self) -> Task<Message> {
        self.open_request = self.open_request.wrapping_add(1);
        self.reload_request = None;
        let request_id = self.open_request;
        Task::perform(open_dialog(), move |result| Message::FileOpened {
            request_id,
            result,
        })
    }

    fn begin_reload(&mut self) -> Task<Message> {
        let Some(path) = self.path.clone() else {
            return Task::none();
        };
        self.open_request = self.open_request.wrapping_add(1);
        let request_id = self.open_request;
        self.reload_request = Some(request_id);
        Task::perform(load_file(path), move |result| Message::FileOpened {
            request_id,
            result,
        })
    }

    fn install_loaded(&mut self, loaded: LoadedFile) {
        self.watch_generation = self.watch_generation.wrapping_add(1);
        self.snapshot_request = 0;
        self.content = text_editor::Content::with_text(&loaded.contents);
        self.trailing_newline_suffix = loaded.contents.ends_with('\n');
        self.path = Some(loaded.path);
        self.revision = 0;
        self.saving = false;
        self.saved = true;
        self.format = Some(loaded.format);
        self.file_state = Some(file_state::FileSyncState::new(
            loaded.contents,
            loaded.fingerprint,
        ));
        self.watch_error = None;
        self.file_error = None;
    }

    fn replace_from_disk(&mut self, loaded: LoadedFile) {
        self.cancel_pending_reload();
        self.invalidate_snapshots();
        self.content = text_editor::Content::with_text(&loaded.contents);
        self.trailing_newline_suffix = loaded.contents.ends_with('\n');
        self.path = Some(loaded.path);
        self.revision = 0;
        self.saving = false;
        self.saved = true;
        self.format = Some(loaded.format);
        if let Some(state) = self.file_state.as_mut() {
            state.mark_reloaded(loaded.contents, loaded.fingerprint);
        } else {
            self.file_state = Some(file_state::FileSyncState::new(
                loaded.contents,
                loaded.fingerprint,
            ));
        }
        self.watch_error = None;
        self.file_error = None;
    }

    fn start_save(&mut self, overwrite: bool) -> Task<Message> {
        if self.saving || (overwrite != self.is_conflicted()) {
            return Task::none();
        }
        let (Some(path), Some(format), Some(state)) =
            (self.path.clone(), self.format, self.file_state.as_ref())
        else {
            return Task::none();
        };
        let text = self.current_text();
        let expected_fingerprint = state.disk_fingerprint();
        let generation = self.watch_generation;
        self.saving = true;
        Task::perform(
            save_file(path, text, format, expected_fingerprint, overwrite),
            move |result| Message::FileSaved { generation, result },
        )
    }

    fn invalidate_snapshots(&mut self) {
        self.snapshot_request = self.snapshot_request.wrapping_add(1);
    }

    fn cancel_pending_reload(&mut self) {
        if self.reload_request.take() == Some(self.open_request) {
            self.open_request = self.open_request.wrapping_add(1);
        }
    }

    fn request_snapshot(&mut self, generation: u64) -> Task<Message> {
        let Some(path) = self.path.clone() else {
            return Task::none();
        };
        self.snapshot_request = self.snapshot_request.wrapping_add(1);
        let request_id = self.snapshot_request;
        Task::perform(read_snapshot(path), move |result| Message::DiskSnapshot {
            generation,
            request_id,
            result,
        })
    }

    fn apply_disk_snapshot(&mut self, result: DiskSnapshotResult) {
        let current_text = self.current_text();
        let change = self.file_state.as_mut().map(|state| match &result {
            DiskSnapshotResult::Present(loaded) => {
                state.observe(&current_text, Ok(Some(loaded.fingerprint)))
            }
            DiskSnapshotResult::Missing => state.observe(&current_text, Ok(None)),
            DiskSnapshotResult::ReadError(message) => {
                state.observe(&current_text, Err(message.clone()))
            }
        });

        match (result, change) {
            (DiskSnapshotResult::Present(loaded), Some(file_state::FileChange::Reload)) => {
                self.replace_from_disk(loaded);
            }
            (DiskSnapshotResult::Present(loaded), Some(file_state::FileChange::Conflict)) => {
                if self.reload_request == Some(self.open_request) {
                    self.replace_from_disk(loaded);
                } else {
                    self.saved = self
                        .file_state
                        .as_ref()
                        .is_some_and(|state| state.is_clean(&self.current_text()));
                    self.file_error = Some("Conflit : le fichier a changé sur le disque.".into());
                }
            }
            (DiskSnapshotResult::Present(_), Some(file_state::FileChange::Unchanged)) => {
                self.file_error = None;
            }
            (DiskSnapshotResult::Missing, Some(file_state::FileChange::Missing)) => {
                self.cancel_pending_reload();
                self.file_error = Some("Le fichier a été supprimé du disque.".into());
            }
            (DiskSnapshotResult::ReadError(message), Some(file_state::FileChange::ReadError)) => {
                self.cancel_pending_reload();
                self.file_error = Some(message);
            }
            _ => {}
        }
    }

    fn is_conflicted(&self) -> bool {
        self.file_state
            .as_ref()
            .is_some_and(file_state::FileSyncState::is_conflicted)
    }

    fn cursor_at_document_end(&self) -> bool {
        if self.content.selection().is_some() {
            return false;
        }
        let (line_index, column) = self.content.cursor_position();
        line_index + 1 == self.content.line_count()
            && self
                .content
                .line(line_index)
                .is_some_and(|line| column >= line.len())
    }

    fn selection_covers_document(&self) -> bool {
        let Some(selection) = self.content.selection() else {
            return false;
        };
        let text = self.current_text();
        selection == text || selection == text.strip_suffix('\n').unwrap_or(&text)
    }

    fn current_text(&self) -> String {
        let mut text = self.editor_text();
        // Keep the hidden suffix distinct from newlines introduced in the editor buffer.
        if self.trailing_newline_suffix {
            text.push('\n');
        }
        text
    }

    fn editor_text(&self) -> String {
        self.content
            .lines()
            .enumerate()
            .fold(String::new(), |mut text, (index, line)| {
                if index > 0 {
                    text.push('\n');
                }
                text.push_str(&line);
                text
            })
    }

    fn view(&self) -> Element<'_, Message> {
        let conflicted = self.is_conflicted();
        let mut controls = row![
            button("Ouvrir…").on_press(Message::Open).padding(6),
            button(text(if self.saving { "…" } else { "Enregistrer" }))
                .on_press_maybe((!self.saving && !conflicted).then_some(Message::Save))
                .padding(6),
        ]
        .spacing(12)
        .align_y(Center);

        if conflicted {
            controls = controls
                .push(
                    button("Recharger depuis le disque")
                        .on_press_maybe((!self.saving).then_some(Message::ReloadFromDisk))
                        .padding(6),
                )
                .push(
                    button("Écraser sur le disque")
                        .on_press_maybe((!self.saving).then_some(Message::Overwrite))
                        .padding(6),
                );
        }
        controls = controls.push(text(self.status()));

        let editor = text_editor(&self.content)
            .height(Fill)
            .font(Font::MONOSPACE)
            .padding(10)
            .on_action(Message::Edit);

        column![controls, editor].spacing(8).padding(8).into()
    }

    fn status(&self) -> String {
        let file = self
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "sans titre".into());
        let current_text = self.current_text();
        let buffer = Buffer::from_str(&current_text);
        let newline = self
            .format
            .map(FileFormat::newline)
            .unwrap_or_else(|| Newline::detect(&current_text));
        let dot = if self.saved { "" } else { " •" };
        let sync = if self.is_conflicted() {
            " — conflit disque".to_owned()
        } else if let Some(error) = &self.file_error {
            format!(" — {error}")
        } else if let Some(error) = &self.watch_error {
            format!(" — surveillance indisponible : {error}")
        } else {
            String::new()
        };
        format!(
            "{file}{dot} — {} lignes · rév. {} · {:?}{sync}",
            buffer.line_count(),
            self.revision,
            newline,
        )
    }
}

async fn open_dialog() -> Result<LoadedFile, Error> {
    let handle = rfd::AsyncFileDialog::new()
        .pick_file()
        .await
        .ok_or_else(|| Error("boîte de dialogue annulée".into()))?;
    let path = handle.path().to_path_buf();
    load_file(path).await
}

async fn load_file(path: PathBuf) -> Result<LoadedFile, Error> {
    let bytes = std::fs::read(&path).map_err(|e| Error(e.to_string()))?;
    Ok(decode_loaded_file(path, &bytes))
}

fn decode_loaded_file(path: PathBuf, bytes: &[u8]) -> LoadedFile {
    let document = Document::from_bytes(bytes);
    let contents = normalize_newlines(&document.text(), Newline::Lf);
    LoadedFile {
        path,
        contents,
        format: document.format(),
        fingerprint: file_state::fingerprint(bytes),
    }
}

async fn read_snapshot(path: PathBuf) -> DiskSnapshotResult {
    match std::fs::read(&path) {
        Ok(bytes) => DiskSnapshotResult::Present(decode_loaded_file(path, &bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => DiskSnapshotResult::Missing,
        Err(error) => DiskSnapshotResult::ReadError(error.to_string()),
    }
}

async fn save_file(
    path: PathBuf,
    text: String,
    format: FileFormat,
    expected_fingerprint: Option<u64>,
    overwrite: bool,
) -> Result<SaveOutcome, Error> {
    if !overwrite {
        match std::fs::read(&path) {
            Ok(current_bytes)
                if Some(file_state::fingerprint(&current_bytes)) != expected_fingerprint =>
            {
                return Ok(SaveOutcome::Conflict(load_file(path).await?));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(Error(error.to_string())),
        }
    }

    let bytes = format.encode_text(&text);
    std::fs::write(&path, &bytes).map_err(|error| Error(error.to_string()))?;
    Ok(SaveOutcome::Saved(SavedFile {
        path,
        written_text: text,
        fingerprint: file_state::fingerprint(&bytes),
    }))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn editor_test_directory() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("rightclick-editor-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn editor_with_active_document(
        contents: &str,
        fingerprint: u64,
        open_request: u64,
        snapshot_request: u64,
    ) -> RightClick {
        RightClick {
            content: text_editor::Content::with_text(contents),
            path: Some(PathBuf::from("active.txt")),
            revision: 0,
            saving: false,
            saved: true,
            trailing_newline_suffix: contents.ends_with('\n'),
            watch_generation: 5,
            open_request,
            reload_request: None,
            snapshot_request,
            format: Some(FileFormat::for_text(contents)),
            file_state: Some(file_state::FileSyncState::new(contents.into(), fingerprint)),
            watch_error: None,
            file_error: None,
        }
    }

    #[test]
    fn save_preflight_refuses_to_overwrite_external_bytes() {
        let directory = editor_test_directory();
        let path = directory.join("doc.txt");
        std::fs::write(&path, b"external").unwrap();
        let result = iced::futures::executor::block_on(save_file(
            path.clone(),
            "local".into(),
            FileFormat::for_text("local"),
            Some(file_state::fingerprint(b"saved")),
            false,
        ))
        .unwrap();

        assert!(matches!(result, SaveOutcome::Conflict(_)));
        assert_eq!(std::fs::read(&path).unwrap(), b"external");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn save_recreates_a_deleted_file() {
        let directory = editor_test_directory();
        let path = directory.join("new.txt");
        std::fs::write(&path, b"old").unwrap();
        let expected_fingerprint = file_state::fingerprint(b"old");
        std::fs::remove_file(&path).unwrap();
        let result = iced::futures::executor::block_on(save_file(
            path.clone(),
            "draft".into(),
            FileFormat::for_text("draft"),
            Some(expected_fingerprint),
            false,
        ))
        .unwrap();

        assert!(matches!(result, SaveOutcome::Saved(_)));
        assert_eq!(std::fs::read(&path).unwrap(), b"draft");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn load_and_save_preserve_the_file_newline_format() {
        let directory = editor_test_directory();
        let path = directory.join("mixed.txt");
        let original_bytes = b"before\r\nformat";
        std::fs::write(&path, original_bytes).unwrap();

        let loaded = iced::futures::executor::block_on(load_file(path.clone())).unwrap();
        assert_eq!(loaded.contents, "before\nformat");
        assert_eq!(loaded.format.newline(), Newline::Crlf);
        assert_eq!(loaded.fingerprint, file_state::fingerprint(original_bytes));

        let result = iced::futures::executor::block_on(save_file(
            path.clone(),
            "after\nformat".into(),
            loaded.format,
            Some(loaded.fingerprint),
            false,
        ))
        .unwrap();
        assert!(matches!(result, SaveOutcome::Saved(_)));
        assert_eq!(std::fs::read(&path).unwrap(), b"after\r\nformat");

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn stale_open_result_does_not_replace_the_active_document() {
        let mut editor = editor_with_active_document("current", 10, 2, 0);
        let stale = LoadedFile {
            path: PathBuf::from("stale.txt"),
            contents: "stale contents".into(),
            format: FileFormat::for_text("stale contents"),
            fingerprint: 20,
        };

        let _ = editor.update(Message::FileOpened {
            request_id: 1,
            result: Ok(stale),
        });

        assert_eq!(editor.current_text(), "current");
        assert_eq!(editor.path, Some(PathBuf::from("active.txt")));
    }

    #[test]
    fn editor_text_preserves_the_real_final_newline_state() {
        let mut editor = editor_with_active_document("single line", 10, 2, 0);
        assert_eq!(editor.current_text(), "single line");

        let _ = editor.update(Message::Edit(text_editor::Action::Move(
            text_editor::Motion::DocumentEnd,
        )));
        let _ = editor.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Enter,
        )));
        assert_eq!(editor.current_text(), "single line\n");

        let _ = editor.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Insert('x'),
        )));
        assert_eq!(editor.current_text(), "single line\nx");

        let mut pasted = editor_with_active_document("single line", 10, 2, 0);
        let _ = pasted.update(Message::Edit(text_editor::Action::Move(
            text_editor::Motion::DocumentEnd,
        )));
        let _ = pasted.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Enter,
        )));
        let _ = pasted.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Paste(std::sync::Arc::new("x".into())),
        )));
        assert_eq!(pasted.current_text(), "single line\nx");

        let mut pasted_crlf = editor_with_active_document("single line", 10, 2, 0);
        let _ = pasted_crlf.update(Message::Edit(text_editor::Action::Move(
            text_editor::Motion::DocumentEnd,
        )));
        let _ = pasted_crlf.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Paste(std::sync::Arc::new("\r\nx\r\n".into())),
        )));
        assert_eq!(pasted_crlf.current_text(), "single line\nx\n");

        let mut replaced = editor_with_active_document("previous", 10, 2, 0);
        let _ = replaced.update(Message::Edit(text_editor::Action::SelectAll));
        let _ = replaced.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Paste(std::sync::Arc::new("replacement\n".into())),
        )));
        assert_eq!(replaced.current_text(), "replacement\n");

        let mut loaded_with_final_newline = editor_with_active_document("single line\n", 10, 2, 0);
        let _ = loaded_with_final_newline.update(Message::Edit(text_editor::Action::Move(
            text_editor::Motion::DocumentEnd,
        )));
        let _ = loaded_with_final_newline.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Enter,
        )));
        assert_eq!(loaded_with_final_newline.current_text(), "single line\n\n");
        let _ = editor.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Backspace,
        )));
        let _ = editor.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Backspace,
        )));
        assert_eq!(editor.current_text(), "single line");

        let mut final_newline = editor_with_active_document("single line\n", 10, 2, 0);
        assert_eq!(final_newline.current_text(), "single line\n");
        let _ = final_newline.update(Message::Edit(text_editor::Action::Move(
            text_editor::Motion::DocumentEnd,
        )));
        let _ = final_newline.update(Message::Edit(text_editor::Action::Edit(
            text_editor::Edit::Delete,
        )));
        assert_eq!(final_newline.current_text(), "single line");
    }

    #[test]
    fn newer_snapshot_wins_over_a_reload_that_started_earlier() {
        let mut editor = editor_with_active_document("local draft", 10, 2, 0);
        let mut state = file_state::FileSyncState::new("saved version".into(), 10);
        assert_eq!(
            state.observe("local draft", Ok(Some(20))),
            file_state::FileChange::Conflict
        );
        editor.file_state = Some(state);
        editor.saved = false;

        let _ = editor.begin_reload();
        let stale_request = editor.open_request;
        assert_eq!(editor.reload_request, Some(stale_request));

        let latest = LoadedFile {
            path: PathBuf::from("active.txt"),
            contents: "latest disk version".into(),
            format: FileFormat::for_text("latest disk version"),
            fingerprint: 30,
        };
        let _ = editor.update(Message::DiskSnapshot {
            generation: 5,
            request_id: 0,
            result: DiskSnapshotResult::Present(latest),
        });

        let stale = LoadedFile {
            path: PathBuf::from("active.txt"),
            contents: "stale reload result".into(),
            format: FileFormat::for_text("stale reload result"),
            fingerprint: 20,
        };
        let _ = editor.update(Message::FileOpened {
            request_id: stale_request,
            result: Ok(stale),
        });

        assert_eq!(editor.current_text(), "latest disk version");
        assert_eq!(
            editor.file_state.as_ref().unwrap().disk_fingerprint(),
            Some(30)
        );
        assert!(editor.reload_request.is_none());
    }

    #[test]
    fn deleted_snapshot_cancels_a_pending_reload() {
        let mut editor = editor_with_active_document("local draft", 10, 2, 0);
        let mut state = file_state::FileSyncState::new("saved version".into(), 10);
        assert_eq!(
            state.observe("local draft", Ok(Some(20))),
            file_state::FileChange::Conflict
        );
        editor.file_state = Some(state);

        let _ = editor.begin_reload();
        let stale_request = editor.open_request;
        let _ = editor.update(Message::DiskSnapshot {
            generation: 5,
            request_id: 0,
            result: DiskSnapshotResult::Missing,
        });
        let stale = LoadedFile {
            path: PathBuf::from("active.txt"),
            contents: "stale reload result".into(),
            format: FileFormat::for_text("stale reload result"),
            fingerprint: 20,
        };
        let _ = editor.update(Message::FileOpened {
            request_id: stale_request,
            result: Ok(stale),
        });

        assert_eq!(editor.current_text(), "local draft");
        assert!(editor.file_error.as_deref().unwrap().contains("supprimé"));
        assert!(editor.reload_request.is_none());
    }

    #[test]
    fn read_error_snapshot_cancels_a_pending_reload() {
        let mut editor = editor_with_active_document("local draft", 10, 2, 0);
        let mut state = file_state::FileSyncState::new("saved version".into(), 10);
        assert_eq!(
            state.observe("local draft", Ok(Some(20))),
            file_state::FileChange::Conflict
        );
        editor.file_state = Some(state);

        let _ = editor.begin_reload();
        let stale_request = editor.open_request;
        let _ = editor.update(Message::DiskSnapshot {
            generation: 5,
            request_id: 0,
            result: DiskSnapshotResult::ReadError("read failed".into()),
        });
        let stale = LoadedFile {
            path: PathBuf::from("active.txt"),
            contents: "stale reload result".into(),
            format: FileFormat::for_text("stale reload result"),
            fingerprint: 20,
        };
        let _ = editor.update(Message::FileOpened {
            request_id: stale_request,
            result: Ok(stale),
        });

        assert_eq!(editor.current_text(), "local draft");
        assert_eq!(editor.file_error.as_deref(), Some("read failed"));
        assert!(editor.reload_request.is_none());
    }

    #[test]
    fn stale_snapshot_after_save_conflict_does_not_roll_back_reloaded_text() {
        let mut editor = editor_with_active_document("saved", 10, 2, 8);
        let latest = LoadedFile {
            path: PathBuf::from("active.txt"),
            contents: "latest disk text".into(),
            format: FileFormat::for_text("latest disk text"),
            fingerprint: 20,
        };

        let _ = editor.update(Message::FileSaved {
            generation: 5,
            result: Ok(SaveOutcome::Conflict(latest)),
        });
        assert_eq!(editor.current_text(), "latest disk text");
        assert!(editor.snapshot_request > 8);

        let stale = LoadedFile {
            path: PathBuf::from("active.txt"),
            contents: "stale disk text".into(),
            format: FileFormat::for_text("stale disk text"),
            fingerprint: 15,
        };
        let _ = editor.update(Message::DiskSnapshot {
            generation: 5,
            request_id: 8,
            result: DiskSnapshotResult::Present(stale),
        });

        assert_eq!(editor.current_text(), "latest disk text");
        assert_eq!(
            editor.file_state.as_ref().unwrap().disk_fingerprint(),
            Some(20)
        );
    }

    #[test]
    fn stale_snapshot_after_successful_save_does_not_restore_older_disk_text() {
        let mut editor = editor_with_active_document("saved", 10, 2, 8);

        let _ = editor.update(Message::FileSaved {
            generation: 5,
            result: Ok(SaveOutcome::Saved(SavedFile {
                path: PathBuf::from("active.txt"),
                written_text: "saved".into(),
                fingerprint: 20,
            })),
        });
        let stale = LoadedFile {
            path: PathBuf::from("active.txt"),
            contents: "older disk text".into(),
            format: FileFormat::for_text("older disk text"),
            fingerprint: 10,
        };
        let _ = editor.update(Message::DiskSnapshot {
            generation: 5,
            request_id: 8,
            result: DiskSnapshotResult::Present(stale),
        });

        assert_eq!(editor.current_text(), "saved");
        assert_eq!(
            editor.file_state.as_ref().unwrap().disk_fingerprint(),
            Some(20)
        );
        assert!(!editor.is_conflicted());
    }
}
