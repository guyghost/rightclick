//! RightClick — coquille graphique (phase 1).
//!
//! L'éditeur utilise le widget `text_editor` d'iced comme surface de
//! frappe ; les crates du cœur (`rc-buffer`, `rc-text`) interviennent
//! déjà pour la normalisation des documents (détection des fins de ligne)
//! et l'écriture disque. L'intégration complète (rendu custom piloté par
//! `rc-buffer` + parsing incrémental) est la phase 2 — voir README.md.

use std::path::PathBuf;

use iced::widget::{button, column, row, text, text_editor};
use iced::{Center, Element, Fill, Font, Task, Theme};

use rc_buffer::Buffer;
use rc_document::Document;
use rc_text::Newline;

#[allow(dead_code)] // Used by the editor integration in Task 5.
mod file_watcher;
#[allow(dead_code)] // Used by the editor integration in Task 5.
mod file_state;

fn main() -> iced::Result {
    iced::application("RightClick", RightClick::update, RightClick::view)
        .theme(|_: &RightClick| Theme::Dark)
        .centered()
        .run_with(RightClick::new)
}

struct RightClick {
    content: text_editor::Content,
    path: Option<PathBuf>,
    revision: u64,
    saving: bool,
    saved: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    Open,
    FileOpened(Result<(PathBuf, String), Error>),
    Save,
    FileSaved(Result<PathBuf, Error>),
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
        };

        // `rightclick chemin/vers/fichier` ouvre directement le document
        let task = match std::env::args().nth(1).map(PathBuf::from) {
            Some(path) => Task::perform(load_file(path), Message::FileOpened),
            None => Task::none(),
        };

        (init, task)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(action) => {
                let edited = action.is_edit();
                self.content.perform(action);
                if edited {
                    self.revision += 1;
                    self.saved = false;
                }
                Task::none()
            }
            Message::Open => Task::perform(open_dialog(), Message::FileOpened),
            Message::FileOpened(Ok((path, contents))) => {
                self.content = text_editor::Content::with_text(&contents);
                self.path = Some(path);
                self.revision = 0;
                self.saved = true;
                Task::none()
            }
            Message::FileOpened(Err(error)) => {
                eprintln!("Ouverture impossible : {error}");
                Task::none()
            }
            Message::Save => {
                let Some(path) = self.path.clone() else {
                    return Task::none();
                };
                self.saving = true;
                Task::perform(save_file(path, self.content.text()), Message::FileSaved)
            }
            Message::FileSaved(Ok(path)) => {
                self.saving = false;
                self.saved = true;
                println!("Enregistré : {}", path.display());
                Task::none()
            }
            Message::FileSaved(Err(error)) => {
                self.saving = false;
                eprintln!("Enregistrement impossible : {error}");
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let controls = row![
            button("Ouvrir…").on_press(Message::Open).padding(6),
            button(text(if self.saving { "…" } else { "Enregistrer" }))
                .on_press_maybe((!self.saving).then_some(Message::Save))
                .padding(6),
            text(self.status()),
        ]
        .spacing(12)
        .align_y(Center);

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
        let newline = Newline::detect(&self.content.text());
        let dot = if self.saved { "" } else { " •" };
        format!(
            "{file}{dot} — {} lignes · rév. {} · {:?}",
            buffer.line_count(),
            self.revision,
            newline,
        )
    }
}

async fn open_dialog() -> Result<(PathBuf, String), Error> {
    let handle = rfd::AsyncFileDialog::new()
        .pick_file()
        .await
        .ok_or_else(|| Error("boîte de dialogue annulée".into()))?;
    let path = handle.path().to_path_buf();
    load_file(path).await
}

async fn load_file(path: PathBuf) -> Result<(PathBuf, String), Error> {
    // Le cœur détecte l'encodage (BOM UTF-8/UTF-16, heuristique UTF-16 sans
    // BOM, repli Windows-1252) et décode — les fichiers non-UTF-8 s'ouvrent
    // au lieu d'échouer (phase 2a : rc-document).
    let bytes = std::fs::read(&path).map_err(|e| Error(e.to_string()))?;
    let document = Document::from_bytes(&bytes);
    Ok((path, document.text()))
}

async fn save_file(path: PathBuf, contents: String) -> Result<PathBuf, Error> {
    // Normalisation via le cœur : détection de la fin de ligne du document
    // puis écriture par la rope (UTF-8).
    let newline = Newline::detect(&contents);
    let buffer = Buffer::from_str(&contents);
    if matches!(newline, Newline::Crlf | Newline::Cr) {
        // Phase 2 : réécriture des fins de ligne à l'enregistrement
        // (comportement TextMate : conserver le style du document)
        let _ = newline.as_str();
    }
    buffer.to_file(&path).map_err(|e| Error(e.to_string()))?;
    Ok(path)
}
