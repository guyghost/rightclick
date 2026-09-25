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
    watch_generation: u64,
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
    FileOpened(Result<LoadedFile, Error>),
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
        let init = Self {
            content: text_editor::Content::new(),
            path: None,
            revision: 0,
            saving: false,
            saved: false,
            watch_generation: 0,
            snapshot_request: 0,
            format: None,
            file_state: None,
            watch_error: None,
            file_error: None,
        };

        // `rightclick chemin/vers/fichier` ouvre directement le document
        let task = match std::env::args().nth(1).map(PathBuf::from) {
            Some(path) => Task::perform(load_file(path), Message::FileOpened),
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
                let edited = action.is_edit();
                self.content.perform(action);
                if edited {
                    self.revision += 1;
                    self.saved = self
                        .file_state
                        .as_ref()
                        .is_some_and(|state| state.is_clean(&self.content.text()));
                }
                Task::none()
            }
            Message::Open => Task::perform(open_dialog(), Message::FileOpened),
            Message::FileOpened(Ok(loaded)) => {
                self.install_loaded(loaded, true);
                Task::none()
            }
            Message::FileOpened(Err(error)) => {
                self.file_error = Some(error.0.clone());
                eprintln!("Ouverture impossible : {error}");
                Task::none()
            }
            Message::Save => self.start_save(false),
            Message::Overwrite => self.start_save(true),
            Message::ReloadFromDisk => match self.path.clone() {
                Some(path) => Task::perform(load_file(path), Message::FileOpened),
                None => Task::none(),
            },
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
                    || request_id < self.snapshot_request
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
                        if let Some(state) = self.file_state.as_mut() {
                            state.mark_saved(saved.written_text, saved.fingerprint);
                            self.saved = state.is_clean(&self.content.text());
                        }
                        self.file_error = None;
                        println!("Enregistré : {}", saved.path.display());
                    }
                    Ok(SaveOutcome::Conflict(loaded)) => {
                        let current_text = self.content.text();
                        let change = self.file_state.as_mut().map(|state| {
                            state.observe(&current_text, Ok(Some(loaded.fingerprint)))
                        });
                        match change {
                            Some(file_state::FileChange::Reload) => self.replace_from_disk(loaded),
                            Some(file_state::FileChange::Unchanged) if !self.is_conflicted() => {
                                self.file_error = None;
                            }
                            Some(file_state::FileChange::Conflict)
                            | Some(file_state::FileChange::Unchanged)
                            | Some(file_state::FileChange::Missing)
                            | Some(file_state::FileChange::ReadError)
                            | None => {
                                self.file_error = Some("Le fichier a changé sur le disque.".into());
                            }
                        }
                    }
                }
                Task::none()
            }
        }
    }

    fn install_loaded(&mut self, loaded: LoadedFile, advance_generation: bool) {
        if advance_generation {
            self.watch_generation = self.watch_generation.wrapping_add(1);
        }
        self.snapshot_request = 0;
        self.content = text_editor::Content::with_text(&loaded.contents);
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
        self.content = text_editor::Content::with_text(&loaded.contents);
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
        let text = self.content.text();
        let expected_fingerprint = state.disk_fingerprint();
        let generation = self.watch_generation;
        self.saving = true;
        Task::perform(
            save_file(path, text, format, expected_fingerprint, overwrite),
            move |result| Message::FileSaved { generation, result },
        )
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
        let current_text = self.content.text();
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
            (DiskSnapshotResult::Present(_), Some(file_state::FileChange::Conflict)) => {
                self.saved = self
                    .file_state
                    .as_ref()
                    .is_some_and(|state| state.is_clean(&self.content.text()));
                self.file_error = Some("Conflit : le fichier a changé sur le disque.".into());
            }
            (DiskSnapshotResult::Present(_), Some(file_state::FileChange::Unchanged)) => {
                self.file_error = None;
            }
            (DiskSnapshotResult::Missing, Some(file_state::FileChange::Missing)) => {
                self.file_error = Some("Le fichier a été supprimé du disque.".into());
            }
            (DiskSnapshotResult::ReadError(message), Some(file_state::FileChange::ReadError)) => {
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
        let buffer = Buffer::from_str(&self.content.text());
        let newline = self
            .format
            .map(FileFormat::newline)
            .unwrap_or_else(|| Newline::detect(&self.content.text()));
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
}
