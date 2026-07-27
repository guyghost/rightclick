//! Modal overlay rendering for the git status plugin.
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::core::models::Theme;
use crate::theme::{UiElement, style_for_ui_element};

use super::super::state::{GitModal, PluginState};

pub(super) const GIT_DELETE_BRANCH_MODAL_HINT: &str = "Enter/D: Delete  |  Esc: Cancel";
pub(super) const GIT_DROP_STASH_MODAL_HINT: &str = "Enter/D: Drop  |  Esc: Cancel";
pub(super) const GIT_CANCEL_MODAL_HINT: &str = "Esc: Cancel";
pub(super) const GIT_ERROR_MODAL_HINT: &str = "Esc: Close";
pub(super) const GIT_MODAL_WIDTH: u16 = 50;
pub(super) const GIT_MODAL_HEIGHT: u16 = 7;
pub(super) const MIN_GIT_MODAL_WIDTH: u16 = 20;
pub(super) const MIN_GIT_MODAL_HEIGHT: u16 = 5;

pub(super) fn render_modal_overlay(
    state: &PluginState,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let Some(ref modal) = state.active_modal else {
        return;
    };

    let Some(modal_area) = git_modal_area(area) else {
        return;
    };

    // Clear the area
    for row in modal_area.top()..modal_area.bottom() {
        for col in modal_area.left()..modal_area.right() {
            if let Some(cell) = buf.cell_mut((col, row)) {
                cell.set_char(' ');
                cell.set_style(Style::default());
            }
        }
    }

    let (title, body) = match modal {
        GitModal::CommitMessage => ("Commit", "Enter commit message..."),
        GitModal::CreateBranch => ("Create Branch", "Enter branch name..."),
        GitModal::DeleteBranch { name } => {
            // Can't return a reference to a temporary, so handle inline
            let block = Block::default()
                .title(" Delete Branch ")
                .borders(Borders::ALL)
                .border_style(style_for_ui_element(theme, UiElement::Error));
            let inner = block.inner(modal_area);
            block.render(modal_area, buf);

            let text = Paragraph::new(vec![
                Line::styled(
                    format!("Delete branch '{}'?", name),
                    style_for_ui_element(theme, UiElement::Text),
                ),
                Line::raw(""),
                Line::styled(
                    GIT_DELETE_BRANCH_MODAL_HINT,
                    style_for_ui_element(theme, UiElement::MutedText),
                ),
            ]);
            text.render(inner, buf);
            return;
        }
        GitModal::DropStash { index } => {
            let block = Block::default()
                .title(" Drop Stash ")
                .borders(Borders::ALL)
                .border_style(style_for_ui_element(theme, UiElement::Error));
            let inner = block.inner(modal_area);
            block.render(modal_area, buf);

            let text = Paragraph::new(vec![
                Line::styled(
                    format!("Drop stash@{{{}}}?", index),
                    style_for_ui_element(theme, UiElement::Text),
                ),
                Line::raw(""),
                Line::styled(
                    GIT_DROP_STASH_MODAL_HINT,
                    style_for_ui_element(theme, UiElement::MutedText),
                ),
            ]);
            text.render(inner, buf);
            return;
        }
        GitModal::Error { message } => {
            let block = Block::default()
                .title(" Error ")
                .borders(Borders::ALL)
                .border_style(style_for_ui_element(theme, UiElement::Error));
            let inner = block.inner(modal_area);
            block.render(modal_area, buf);

            let text = Paragraph::new(vec![
                Line::styled(
                    message.clone(),
                    style_for_ui_element(theme, UiElement::Error),
                ),
                Line::raw(""),
                Line::styled(
                    GIT_ERROR_MODAL_HINT,
                    style_for_ui_element(theme, UiElement::MutedText),
                ),
            ])
            .wrap(ratatui::widgets::Wrap { trim: false });
            text.render(inner, buf);
            return;
        }
    };

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(style_for_ui_element(theme, UiElement::Primary));
    let inner = block.inner(modal_area);
    block.render(modal_area, buf);

    let text = Paragraph::new(vec![
        Line::styled(body, style_for_ui_element(theme, UiElement::Text)),
        Line::raw(""),
        Line::styled(
            GIT_CANCEL_MODAL_HINT,
            style_for_ui_element(theme, UiElement::MutedText),
        ),
    ]);
    text.render(inner, buf);
}

pub(super) fn git_modal_area(area: Rect) -> Option<Rect> {
    if area.width < MIN_GIT_MODAL_WIDTH || area.height < MIN_GIT_MODAL_HEIGHT {
        return None;
    }

    let width = GIT_MODAL_WIDTH.min(area.width);
    let height = GIT_MODAL_HEIGHT.min(area.height);
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);

    Some(Rect::new(x, y, width, height))
}
