//! Main file browser plugin implementation
//!
//! This module provides the `FileBrowserPlugin` struct which implements
//! the complete file browser functionality including tree view, file preview,
//! and keyboard navigation.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::core::models::Theme;
use crate::event::Event;
use crate::keymap::{Action, FocusContext};
use crate::plugin::{Command, Plugin, PluginCommand, PluginContext};
use crate::theme::{UiElement, style_for_ui_element};
use crate::ui::{
    Footer, HELP_HINT, Header, KeyHint, count_label, global_hint_message, truncate_display,
};

use super::preview::PreviewWidget;
use super::state::{FileOperationModal, PluginState};
use super::tree::FileTreeWidget;

const CREATE_ENTRY_MODAL_HINT: &str = "Enter: Create  |  Esc: Cancel";
const DELETE_ENTRY_MODAL_HINT: &str = "Enter/D: Delete  |  Esc: Cancel";
const RENAME_ENTRY_MODAL_HINT: &str = "Enter: Rename  |  Esc: Cancel";
const FILTER_FILES_MODAL_HINT: &str = "Enter: Apply  |  Empty input: Clear  |  Esc: Cancel";
const ERROR_MODAL_HINT: &str = "Enter/Esc: Close";
const HELP_OVERLAY_HINT: &str = HELP_HINT;
const FILE_INFO_OVERLAY_HINT: &str = "I: Close";
const MIN_OVERLAY_WIDTH: u16 = 24;
const MIN_OVERLAY_HEIGHT: u16 = 5;

fn centered_overlay_area(area: Rect, preferred_width: u16, preferred_height: u16) -> Option<Rect> {
    if area.width < MIN_OVERLAY_WIDTH || area.height < MIN_OVERLAY_HEIGHT {
        return None;
    }

    let width = preferred_width.min(area.width);
    let height = preferred_height.min(area.height);
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);

    Some(Rect::new(x, y, width, height))
}

/// Commands for file operations that are executed asynchronously
#[derive(Debug, Clone, PartialEq)]
pub enum FileCommand {
    /// Create a new file at the given path
    CreateFile(PathBuf),
    /// Create a new directory at the given path
    CreateDir(PathBuf),
    /// Delete a file or directory at the given path
    DeletePath(PathBuf),
    /// Rename a file or directory from one path to another
    RenamePath { from: PathBuf, to: PathBuf },
}

/// The main file browser plugin
///
/// This plugin provides a two-pane file browser with a collapsible directory
/// tree on the left and a file preview panel on the right.
///
/// # Features
///
/// - Collapsible directory tree with expand/collapse
/// - File preview with syntax highlighting
/// - Quick navigation (j/k for up/down)
/// - Toggle hidden files and git-ignored files
/// - File info panel
///
/// # Example
///
/// ```rust
/// use rightclick::plugins::filebrowser::FileBrowserPlugin;
/// use std::path::PathBuf;
///
/// let plugin = FileBrowserPlugin::new(PathBuf::from("."));
/// ```
#[derive(Clone, Debug)]
pub struct FileBrowserPlugin {
    /// Plugin state
    pub state: PluginState,
    /// Working directory
    pub work_dir: PathBuf,
    /// Whether the plugin has focus
    pub focused: bool,
    /// Current theme
    theme: Theme,
    /// Show help overlay
    show_help: bool,
    /// Pending file operation commands to execute asynchronously
    pending_commands: VecDeque<FileCommand>,
}

#[async_trait]
impl Plugin for FileBrowserPlugin {
    fn id(&self) -> &str {
        "file_browser"
    }

    fn name(&self) -> &str {
        "File Browser"
    }

    fn icon(&self) -> char {
        '📁'
    }

    async fn init(&mut self, _ctx: &PluginContext) -> anyhow::Result<()> {
        self.refresh();
        Ok(())
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Vec<Command> {
        let commands = Vec::new();

        match event {
            Event::FocusChanged { .. } => {
                // Handle focus changes (ignore unused _ prefix)
            }
            Event::RefreshNeeded => {
                self.refresh();
            }
            Event::Key { code, modifiers } => {
                // When a modal is active, route keys to modal handler
                if self.state.modal_active {
                    self.handle_modal_key(&code, &modifiers);
                } else {
                    // Handle Ctrl+key combinations first
                    if modifiers.ctrl {
                        match code.as_str() {
                            "d" => {
                                self.state.scroll_preview_down(
                                    self.state.preview_scroll.visible_lines / 2,
                                );
                            }
                            "u" => {
                                self.state
                                    .scroll_preview_up(self.state.preview_scroll.visible_lines / 2);
                            }
                            _ => {}
                        }
                    } else if !modifiers.alt {
                        // Handle simple key presses (including shift which affects the code)
                        self.handle_key(&code);
                    }
                }
            }
            _ => {}
        }

        commands
    }

    fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        // Create a temporary plugin with the theme for rendering
        let mut plugin = self.clone();
        plugin.theme = theme.clone();
        plugin.render_internal(area, buf);
    }

    fn is_focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::with_context_description(
                "create_file",
                "New File",
                "Create a file in the current directory",
                'a',
                crate::keymap::FocusContext::FileBrowserTree,
            )
            .with_footer_priority(2),
            PluginCommand::with_context_description(
                "create_dir",
                "New Directory",
                "Create a directory in the current directory",
                'A',
                crate::keymap::FocusContext::FileBrowserTree,
            )
            .with_footer_priority(1),
            PluginCommand::with_context_description(
                "delete",
                "Delete",
                "Delete the selected file or directory",
                'd',
                crate::keymap::FocusContext::FileBrowserTree,
            ),
            PluginCommand::with_context_description(
                "rename",
                "Rename",
                "Rename the selected file or directory",
                'R',
                crate::keymap::FocusContext::FileBrowserTree,
            ),
            PluginCommand::with_context_description(
                "filter",
                "Filter Files",
                "Filter visible files",
                'f',
                crate::keymap::FocusContext::FileBrowserTree,
            )
            .with_footer_priority(5),
            PluginCommand::with_context_description(
                "refresh",
                "Refresh Files",
                "Reload the file tree",
                'r',
                crate::keymap::FocusContext::FileBrowserTree,
            )
            .with_footer_priority(4),
            PluginCommand::with_context_description(
                "toggle_hidden",
                "Toggle Hidden",
                "Show or hide dotfiles",
                'H',
                crate::keymap::FocusContext::FileBrowserTree,
            ),
            PluginCommand::with_context_description(
                "toggle_ignored",
                "Toggle Ignored",
                "Show or hide ignored files",
                'i',
                crate::keymap::FocusContext::FileBrowserTree,
            ),
            PluginCommand::with_context_description(
                "file_info",
                "File Info",
                "Show metadata for the selected file",
                'I',
                crate::keymap::FocusContext::FileBrowserTree,
            )
            .with_footer_priority(3),
            PluginCommand::with_context_description(
                "preview_top",
                "Preview Top",
                "Scroll the preview to the top",
                'g',
                crate::keymap::FocusContext::FileBrowserTree,
            ),
            PluginCommand::with_context_description(
                "preview_bottom",
                "Preview Bottom",
                "Scroll the preview to the bottom",
                'G',
                crate::keymap::FocusContext::FileBrowserTree,
            ),
        ]
    }

    fn status_line(&self) -> Option<String> {
        Some(file_browser_status_line(&self.state))
    }

    fn reveal_path(&mut self, path: &Path) -> bool {
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.work_dir.join(path)
        };

        if !resolved.exists() {
            return false;
        }

        self.navigate_to(resolved);
        self.ensure_selected_visible();
        true
    }

    fn focus_context(&self) -> FocusContext {
        FocusContext::FileBrowserTree
    }

    fn consumes_text_input(&self) -> bool {
        self.state.modal_active
    }

    async fn update(&mut self) -> anyhow::Result<()> {
        while let Some(cmd) = self.pending_commands.pop_front() {
            if let Err(e) = self.execute_file_command(cmd).await {
                tracing::warn!("Failed to execute file command: {}", e);
                self.state.open_modal(FileOperationModal::Error {
                    message: e.to_string(),
                });
            }
        }
        Ok(())
    }
}

impl FileBrowserPlugin {
    /// Create a new file browser plugin
    ///
    /// # Arguments
    ///
    /// * `work_dir` - The working directory to browse
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::plugins::filebrowser::FileBrowserPlugin;
    /// use std::path::PathBuf;
    ///
    /// let plugin = FileBrowserPlugin::new(PathBuf::from("."));
    /// ```
    pub fn new(work_dir: PathBuf) -> Self {
        let state = PluginState::new(work_dir.clone());

        Self {
            state,
            work_dir: work_dir.clone(),
            focused: true,
            theme: Theme::default(),
            show_help: false,
            pending_commands: VecDeque::new(),
        }
    }

    /// Create a new file browser plugin with a specific theme
    ///
    /// # Arguments
    ///
    /// * `work_dir` - The working directory to browse
    /// * `theme` - The theme to use for rendering
    pub fn with_theme(work_dir: PathBuf, theme: Theme) -> Self {
        let mut plugin = Self::new(work_dir);
        plugin.theme = theme;
        plugin
    }

    /// Set the theme for the plugin
    ///
    /// # Arguments
    ///
    /// * `theme` - The theme to use
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    /// Handle an action from the keymap system
    ///
    /// # Arguments
    ///
    /// * `action` - The action to handle
    ///
    /// # Returns
    ///
    /// `true` if the action was handled, `false` otherwise
    pub fn handle_action(&mut self, action: &Action) -> bool {
        match action {
            Action::NavigateUp => {
                self.state.prev_entry();
                self.ensure_selected_visible();
                true
            }
            Action::NavigateDown => {
                self.state.next_entry();
                self.ensure_selected_visible();
                true
            }
            Action::NavigateRight | Action::Expand => {
                self.state.expand_selected();
                true
            }
            Action::NavigateLeft | Action::Collapse => {
                self.state.collapse_selected();
                true
            }
            Action::Select | Action::Open | Action::Toggle => {
                self.state.toggle_selected();
                true
            }
            Action::Refresh => {
                self.state.refresh();
                true
            }
            Action::Search => {
                self.state.open_modal(FileOperationModal::Filter);
                true
            }
            Action::Filter => {
                self.state.open_modal(FileOperationModal::Filter);
                true
            }
            _ => false,
        }
    }

    /// Handle a key press event
    ///
    /// This handles keys that aren't part of the standard action system,
    /// such as custom file browser shortcuts.
    ///
    /// # Arguments
    ///
    /// * `key` - The key that was pressed
    ///
    /// # Returns
    ///
    /// `true` if the key was handled, `false` otherwise
    pub fn handle_key(&mut self, key: &str) -> bool {
        match key {
            "i" => {
                self.state.toggle_ignored();
                true
            }
            "I" => {
                self.state.toggle_file_info();
                true
            }
            "H" => {
                self.state.toggle_hidden();
                true
            }
            "f" => {
                self.state.open_modal(FileOperationModal::Filter);
                true
            }
            "?" => {
                self.show_help = !self.show_help;
                true
            }
            "g" => {
                self.state.scroll_preview_to_top();
                true
            }
            "G" => {
                self.state.scroll_preview_to_bottom();
                true
            }
            // Navigation
            "j" | "Down" => {
                self.state.next_entry();
                self.ensure_selected_visible();
                true
            }
            "k" | "Up" => {
                self.state.prev_entry();
                self.ensure_selected_visible();
                true
            }
            "l" | "Right" => {
                self.state.expand_selected();
                true
            }
            "h" | "Left" => {
                self.state.collapse_selected();
                true
            }
            "Enter" | " " => {
                self.state.toggle_selected();
                true
            }
            "r" => {
                self.state.refresh();
                true
            }
            // File operations
            "a" => {
                self.state.open_modal(FileOperationModal::CreateFile);
                true
            }
            "A" => {
                self.state.open_modal(FileOperationModal::CreateDir);
                true
            }
            "d" => {
                if let Some(entry) = self.state.selected_entry() {
                    let path = entry.path.clone();
                    let is_dir = entry.is_dir;
                    self.state
                        .open_modal(FileOperationModal::Delete { path, is_dir });
                }
                true
            }
            "R" => {
                if let Some(entry) = self.state.selected_entry() {
                    let path = entry.path.clone();
                    let original_name = entry.name.clone();
                    self.state.open_modal(FileOperationModal::Rename {
                        path,
                        original_name,
                    });
                }
                true
            }
            _ => false,
        }
    }

    /// Handle key events when a modal is active
    ///
    /// Routes key presses to the appropriate modal handler based on
    /// the type of modal currently displayed.
    fn handle_modal_key(&mut self, key: &str, modifiers: &crate::event::KeyModifiers) {
        let _ = modifiers;

        match key {
            "Escape" => {
                self.state.close_modal();
            }
            "Enter" => {
                self.confirm_modal_action();
            }
            "Backspace" => {
                self.state.input_buffer.pop();
            }
            _ => {
                // For text-input modals, append single characters to the input buffer
                if key.len() == 1 {
                    if let Some(modal) = &self.state.active_modal {
                        match modal {
                            FileOperationModal::CreateFile
                            | FileOperationModal::CreateDir
                            | FileOperationModal::Rename { .. }
                            | FileOperationModal::Filter => {
                                self.state.input_buffer.push_str(key);
                            }
                            FileOperationModal::Delete { .. } if key == "d" || key == "D" => {
                                self.confirm_modal_action();
                            }
                            FileOperationModal::Delete { .. }
                            | FileOperationModal::Error { .. } => {
                                // No text input for delete confirmation or error display
                            }
                        }
                    }
                }
            }
        }
    }

    /// Confirm the current modal action and queue the corresponding file command
    fn confirm_modal_action(&mut self) {
        let modal = match self.state.active_modal.clone() {
            Some(m) => m,
            None => return,
        };

        match modal {
            FileOperationModal::CreateFile => {
                let name = self.state.input_buffer.trim().to_string();
                if !name.is_empty() {
                    let base_dir = self.get_base_dir_for_create();
                    let path = base_dir.join(&name);
                    self.pending_commands
                        .push_back(FileCommand::CreateFile(path));
                }
                self.state.close_modal();
            }
            FileOperationModal::CreateDir => {
                let name = self.state.input_buffer.trim().to_string();
                if !name.is_empty() {
                    let base_dir = self.get_base_dir_for_create();
                    let path = base_dir.join(&name);
                    self.pending_commands
                        .push_back(FileCommand::CreateDir(path));
                }
                self.state.close_modal();
            }
            FileOperationModal::Delete { path, .. } => {
                self.pending_commands
                    .push_back(FileCommand::DeletePath(path));
                self.state.close_modal();
            }
            FileOperationModal::Rename { path, .. } => {
                let new_name = self.state.input_buffer.trim().to_string();
                if !new_name.is_empty() {
                    let to = path.parent().unwrap_or(&self.work_dir).join(&new_name);
                    self.pending_commands
                        .push_back(FileCommand::RenamePath { from: path, to });
                }
                self.state.close_modal();
            }
            FileOperationModal::Filter => {
                self.state
                    .set_filter(Some(self.state.input_buffer.trim().to_string()));
                self.ensure_selected_visible();
                self.state.close_modal();
            }
            FileOperationModal::Error { .. } => {
                self.state.close_modal();
            }
        }
    }

    /// Get the base directory for creating new files/directories.
    ///
    /// If the currently selected entry is a directory, use it as the base.
    /// Otherwise use the parent of the selected file, or fall back to the work dir.
    fn get_base_dir_for_create(&self) -> PathBuf {
        if let Some(entry) = self.state.selected_entry() {
            if entry.is_dir {
                entry.path.clone()
            } else {
                entry.path.parent().unwrap_or(&self.work_dir).to_path_buf()
            }
        } else {
            self.work_dir.clone()
        }
    }

    /// Execute an async file operation command
    async fn execute_file_command(&mut self, cmd: FileCommand) -> anyhow::Result<()> {
        match cmd {
            FileCommand::CreateFile(path) => {
                tokio::fs::File::create(&path).await?;
                tracing::info!("Created file: {:?}", path);
                self.state.refresh();
            }
            FileCommand::CreateDir(path) => {
                tokio::fs::create_dir_all(&path).await?;
                tracing::info!("Created directory: {:?}", path);
                self.state.refresh();
            }
            FileCommand::DeletePath(path) => {
                if path.is_dir() {
                    tokio::fs::remove_dir_all(&path).await?;
                    tracing::info!("Deleted directory: {:?}", path);
                } else {
                    tokio::fs::remove_file(&path).await?;
                    tracing::info!("Deleted file: {:?}", path);
                }
                self.state.refresh();
            }
            FileCommand::RenamePath { from, to } => {
                tokio::fs::rename(&from, &to).await?;
                tracing::info!("Renamed {:?} to {:?}", from, to);
                self.state.refresh();
            }
        }
        Ok(())
    }

    /// Refresh the file tree
    pub fn refresh(&mut self) {
        self.state.refresh();
    }

    /// Navigate to a specific path
    ///
    /// # Arguments
    ///
    /// * `path` - The path to navigate to
    pub fn navigate_to(&mut self, path: PathBuf) {
        self.state.tree.navigate_to(&path);
        self.state.update_selected_path_public();
        self.state.update_preview();
    }

    /// Get the currently selected path
    pub fn selected_path(&self) -> Option<&PathBuf> {
        self.state.selected_path.as_ref()
    }

    /// Ensure the currently selected item is visible in the tree view
    fn ensure_selected_visible(&mut self) {
        // Calculate position in visible (filtered) list
        let visible_pos = self
            .state
            .visible_indices()
            .into_iter()
            .take_while(|idx| *idx != self.state.tree.selected_index)
            .count();

        self.state.tree_scroll.ensure_visible(visible_pos);
    }

    /// Render the plugin to the given buffer (public API)
    ///
    /// # Arguments
    ///
    /// * `area` - The area to render in
    /// * `buf` - The buffer to render to
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_internal(area, buf);
    }

    /// Render the header
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        let header =
            Header::new("File Browser").with_subtitle(format!("{}", self.work_dir.display()));
        header.render(area, buf, &self.theme);
    }

    /// Render the main content area (tree + preview)
    fn render_content(&self, area: Rect, buf: &mut Buffer) {
        let text_style = style_for_ui_element(&self.theme, UiElement::Text);
        let border_style = style_for_ui_element(&self.theme, UiElement::Border);

        // Split into tree (30%) and preview (70%)
        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        // Render tree panel
        let tree_title = if let Some(query) = &self.state.filter_query {
            format!(" Files (filter: {}) ", query)
        } else if self.state.show_ignored {
            " Files (showing ignored) ".to_string()
        } else {
            " Files ".to_string()
        };

        let tree_block = Block::default()
            .title(tree_title)
            .borders(Borders::ALL)
            .border_style(if self.focused {
                style_for_ui_element(&self.theme, UiElement::Primary)
            } else {
                border_style
            });

        let tree_inner = tree_block.inner(content_layout[0]);
        tree_block.render(content_layout[0], buf);

        // Render tree content
        let _tree_widget = FileTreeWidget::new(&self.state.tree)
            .show_icons(self.state.show_icons)
            .show_hidden(self.state.show_hidden);

        // Manual rendering of tree entries with selection highlighting
        self.render_tree_entries(tree_inner, buf);

        // Render preview panel
        if let Some(ref preview) = self.state.preview {
            let preview_widget =
                PreviewWidget::new(preview, self.state.preview_scroll.offset, &self.theme);
            preview_widget.render(content_layout[1], buf);
        } else {
            let preview_block = Block::default().title(" Preview ").borders(Borders::ALL);
            let inner = preview_block.inner(content_layout[1]);
            preview_block.render(content_layout[1], buf);

            let no_preview = Paragraph::new(file_preview_empty_message(&self.state, inner.width))
                .alignment(Alignment::Center)
                .style(text_style)
                .wrap(ratatui::widgets::Wrap { trim: true });
            no_preview.render(inner, buf);
        }
    }

    /// Render the tree entries
    fn render_tree_entries(&self, area: Rect, buf: &mut Buffer) {
        let text_style = style_for_ui_element(&self.theme, UiElement::Text);
        let selected_style =
            style_for_ui_element(&self.theme, UiElement::Highlight).add_modifier(Modifier::BOLD);
        let muted_style = style_for_ui_element(&self.theme, UiElement::MutedText);
        let primary_style = style_for_ui_element(&self.theme, UiElement::Primary);

        let visible_indices = self.state.visible_indices();

        if visible_indices.is_empty() {
            let empty = Paragraph::new(file_tree_empty_message(&self.state, area.width))
                .alignment(Alignment::Center)
                .style(muted_style)
                .wrap(ratatui::widgets::Wrap { trim: true });
            empty.render(area, buf);
            return;
        }

        // Calculate visible range based on scroll offset
        let scroll_offset = self.state.tree_scroll.offset;
        let visible_count = area.height as usize;

        // Update scroll state total
        let total_visible = visible_indices.len();

        for (row, (_vis_pos, &entry_idx)) in visible_indices
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_count)
            .enumerate()
        {
            let y = area.y.saturating_add(row as u16);
            if y >= area.bottom() {
                break;
            }

            let entry = &self.state.tree.entries[entry_idx];
            let is_selected = self.state.tree.selected_index == entry_idx;
            let style = if is_selected {
                selected_style
            } else {
                text_style
            };

            // Build the line
            let mut spans: Vec<Span> = Vec::new();

            // Indentation
            let indent = "  ".repeat(entry.depth);
            spans.push(Span::styled(indent, style));

            // Expand indicator for directories
            if entry.is_dir {
                spans.push(Span::styled(
                    if entry.is_expanded { "▼ " } else { "▶ " },
                    primary_style,
                ));
            } else {
                spans.push(Span::styled("  ", style));
            }

            // Icon
            if self.state.show_icons {
                let icon = if entry.is_dir {
                    if entry.is_expanded { "📂 " } else { "📁 " }
                } else {
                    "📄 "
                };
                spans.push(Span::styled(icon, style));
            }

            // Name
            let name_style = if entry.is_hidden {
                muted_style
            } else if entry.is_dir {
                primary_style.add_modifier(Modifier::BOLD)
            } else {
                style
            };
            spans.push(Span::styled(entry.name.clone(), name_style));

            // Render the line
            let line = Line::from(spans);
            buf.set_line(area.x, y, &line, area.width);
        }

        // Update total lines for scrolling
        let _ = (total_visible, visible_count);
    }

    /// Render the footer
    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let hints = [
            KeyHint::new("j/k", "Navigate"),
            KeyHint::new("r", "Refresh files"),
            KeyHint::new("/", "Global search"),
            KeyHint::new(":", "Command search"),
            KeyHint::new("f", "Filter files"),
            KeyHint::new("a/A", "New file/directory"),
            KeyHint::new("↵/space", "Expand/Collapse"),
            KeyHint::new("d", "Delete"),
            KeyHint::new("R", "Rename"),
            KeyHint::new("i", "Toggle ignored"),
            KeyHint::new("H", "Toggle hidden"),
            KeyHint::new("?", "Toggle help"),
        ];

        let status = if let Some(ref path) = self.state.selected_path {
            format!("{}", path.display())
        } else {
            file_browser_status_line(&self.state)
        };

        let footer = Footer::new(status).with_hints(
            hints
                .iter()
                .map(|h| (h.key.clone(), h.description.clone()))
                .collect(),
        );
        footer.render(area, buf, &self.theme);
    }

    /// Render the help overlay
    fn render_help(&self, area: Rect, buf: &mut Buffer) {
        let _popup_style = style_for_ui_element(&self.theme, UiElement::Popup);
        let primary_style = style_for_ui_element(&self.theme, UiElement::Primary);
        let text_style = style_for_ui_element(&self.theme, UiElement::Text);

        let Some(popup_area) = centered_overlay_area(area, 52, 34) else {
            return;
        };

        // Clear background
        Clear.render(popup_area, buf);

        // Render border
        let block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(primary_style);
        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Help content
        let help_text = vec![
            Line::from(vec![Span::styled(
                "Navigation",
                primary_style.add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("j/↓    ", primary_style),
                Span::styled("Next entry", text_style),
            ]),
            Line::from(vec![
                Span::styled("k/↑    ", primary_style),
                Span::styled("Previous entry", text_style),
            ]),
            Line::from(vec![
                Span::styled("h/←    ", primary_style),
                Span::styled("Collapse directory", text_style),
            ]),
            Line::from(vec![
                Span::styled("l/→    ", primary_style),
                Span::styled("Expand directory", text_style),
            ]),
            Line::from(vec![
                Span::styled("↵/space", primary_style),
                Span::styled("Toggle directory", text_style),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "View",
                primary_style.add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("i      ", primary_style),
                Span::styled("Toggle git-ignored files", text_style),
            ]),
            Line::from(vec![
                Span::styled("H      ", primary_style),
                Span::styled("Toggle hidden files", text_style),
            ]),
            Line::from(vec![
                Span::styled("I      ", primary_style),
                Span::styled("Show file info", text_style),
            ]),
            Line::from(vec![
                Span::styled("g/G    ", primary_style),
                Span::styled("Go to top/bottom of preview", text_style),
            ]),
            Line::from(vec![
                Span::styled("?      ", primary_style),
                Span::styled("Toggle help", text_style),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Global",
                primary_style.add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("/      ", primary_style),
                Span::styled("Global search", text_style),
            ]),
            Line::from(vec![
                Span::styled(":      ", primary_style),
                Span::styled("Command search", text_style),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "File Operations",
                primary_style.add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("a      ", primary_style),
                Span::styled("Create new file", text_style),
            ]),
            Line::from(vec![
                Span::styled("A      ", primary_style),
                Span::styled("Create new directory", text_style),
            ]),
            Line::from(vec![
                Span::styled("d      ", primary_style),
                Span::styled("Delete selected", text_style),
            ]),
            Line::from(vec![
                Span::styled("R      ", primary_style),
                Span::styled("Rename selected", text_style),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(HELP_OVERLAY_HINT, muted_style())]),
        ];

        let help_para = Paragraph::new(help_text);
        help_para.render(inner, buf);
    }

    /// Render the file info panel
    fn render_file_info(&self, area: Rect, buf: &mut Buffer) {
        let _popup_style = style_for_ui_element(&self.theme, UiElement::Popup);
        let primary_style = style_for_ui_element(&self.theme, UiElement::Primary);
        let text_style = style_for_ui_element(&self.theme, UiElement::Text);
        let muted_style = style_for_ui_element(&self.theme, UiElement::MutedText);

        let Some(popup_area) = centered_overlay_area(area, 40, 12) else {
            return;
        };

        // Clear background
        Clear.render(popup_area, buf);

        // Render border
        let block = Block::default()
            .title(" File Info ")
            .borders(Borders::ALL)
            .border_style(primary_style);
        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // File info content
        let info_text = if let Some(entry) = self.state.selected_entry() {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Name: ", primary_style),
                    Span::styled(&entry.name, text_style),
                ]),
                Line::from(vec![
                    Span::styled("Path: ", primary_style),
                    Span::styled(format!("{}", entry.path.display()), text_style),
                ]),
                Line::from(vec![
                    Span::styled("Type: ", primary_style),
                    Span::styled(if entry.is_dir { "Directory" } else { "File" }, text_style),
                ]),
            ];

            if let Some(_size) = entry.size {
                lines.push(Line::from(vec![
                    Span::styled("Size: ", primary_style),
                    Span::styled(entry.format_size(), text_style),
                ]));
            }

            if let Some(ref preview) = self.state.preview {
                lines.push(Line::from(vec![
                    Span::styled("Lines: ", primary_style),
                    Span::styled(format!("{}", preview.total_lines), text_style),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("Type: ", primary_style),
                    Span::styled(preview.file_type_description(), text_style),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                FILE_INFO_OVERLAY_HINT,
                muted_style,
            )]));

            lines
        } else {
            file_info_empty_message(inner.width)
                .lines()
                .map(|line| Line::from(vec![Span::styled(line.to_string(), muted_style)]))
                .collect()
        };

        let info_para = Paragraph::new(info_text);
        info_para.render(inner, buf);
    }

    /// Render the file operation modal overlay
    fn render_modal(&self, area: Rect, buf: &mut Buffer) {
        let modal = match &self.state.active_modal {
            Some(m) => m,
            None => return,
        };

        let primary_style = style_for_ui_element(&self.theme, UiElement::Primary);
        let text_style = style_for_ui_element(&self.theme, UiElement::Text);
        let error_style = Style::default().fg(ratatui::style::Color::Red);

        let (title, variant_style, lines) = match modal {
            FileOperationModal::CreateFile => {
                let lines = vec![
                    Line::from(vec![Span::styled("Enter file name:", text_style)]),
                    Line::from(""),
                    Line::from(vec![Span::styled(
                        format!("> {}_", &self.state.input_buffer),
                        primary_style,
                    )]),
                    Line::from(""),
                    Line::from(vec![Span::styled(CREATE_ENTRY_MODAL_HINT, muted_style())]),
                ];
                (" Create File ", primary_style, lines)
            }
            FileOperationModal::CreateDir => {
                let lines = vec![
                    Line::from(vec![Span::styled("Enter directory name:", text_style)]),
                    Line::from(""),
                    Line::from(vec![Span::styled(
                        format!("> {}_", &self.state.input_buffer),
                        primary_style,
                    )]),
                    Line::from(""),
                    Line::from(vec![Span::styled(CREATE_ENTRY_MODAL_HINT, muted_style())]),
                ];
                (" Create Directory ", primary_style, lines)
            }
            FileOperationModal::Delete { path, is_dir } => {
                let type_str = if *is_dir { "directory" } else { "file" };
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                let lines = vec![
                    Line::from(vec![Span::styled(
                        format!("Delete {} \"{}\"?", type_str, name),
                        text_style,
                    )]),
                    Line::from(""),
                    Line::from(vec![Span::styled(
                        format!("{}", path.display()),
                        muted_style(),
                    )]),
                    Line::from(""),
                    Line::from(vec![Span::styled(DELETE_ENTRY_MODAL_HINT, muted_style())]),
                ];
                (" Confirm Delete ", error_style, lines)
            }
            FileOperationModal::Rename { path, .. } => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                let lines = vec![
                    Line::from(vec![Span::styled(
                        format!("Rename \"{}\" to:", name),
                        text_style,
                    )]),
                    Line::from(""),
                    Line::from(vec![Span::styled(
                        format!("> {}_", &self.state.input_buffer),
                        primary_style,
                    )]),
                    Line::from(""),
                    Line::from(vec![Span::styled(RENAME_ENTRY_MODAL_HINT, muted_style())]),
                ];
                (" Rename ", primary_style, lines)
            }
            FileOperationModal::Filter => {
                let lines = vec![
                    Line::from(vec![Span::styled(
                        "Filter files by name or path:",
                        text_style,
                    )]),
                    Line::from(""),
                    Line::from(vec![Span::styled(
                        format!("> {}_", &self.state.input_buffer),
                        primary_style,
                    )]),
                    Line::from(""),
                    Line::from(vec![Span::styled(FILTER_FILES_MODAL_HINT, muted_style())]),
                ];
                (" Filter Files ", primary_style, lines)
            }
            FileOperationModal::Error { message } => {
                let lines = vec![
                    Line::from(vec![Span::styled(message.as_str(), error_style)]),
                    Line::from(""),
                    Line::from(vec![Span::styled(ERROR_MODAL_HINT, muted_style())]),
                ];
                (" Error ", error_style, lines)
            }
        };

        let popup_height = (lines.len() as u16) + 2; // +2 for border
        let Some(popup_area) = centered_overlay_area(area, 50, popup_height) else {
            return;
        };

        // Clear background
        Clear.render(popup_area, buf);

        // Render border
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(variant_style);
        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Render content
        let para = Paragraph::new(lines);
        para.render(inner, buf);
    }

    /// Get the plugin ID (public API)
    pub fn plugin_id(&self) -> &'static str {
        "file_browser"
    }

    /// Get the plugin display name (public API)
    pub fn display_name(&self) -> &'static str {
        "File Browser"
    }

    /// Get the current focus context (public API)
    pub fn get_focus_context(&self) -> FocusContext {
        FocusContext::FileBrowserTree
    }

    #[allow(dead_code)]
    /// Map a key to an action
    fn map_key_to_action(&self, key: &str) -> Action {
        match key {
            "j" | "down" => Action::NavigateDown,
            "k" | "up" => Action::NavigateUp,
            "h" | "left" => Action::NavigateLeft,
            "l" | "right" => Action::NavigateRight,
            "enter" | "space" => Action::Select,
            "r" => Action::Refresh,
            _ => Action::Back,
        }
    }

    /// Internal render method (used by both Plugin trait and direct calls)
    fn render_internal(&self, area: Rect, buf: &mut Buffer) {
        // Split area into header, content, and footer
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(10),   // Content
                Constraint::Length(1), // Footer
            ])
            .split(area);

        // Render header
        self.render_header(main_layout[0], buf);

        // Render content (tree + preview)
        self.render_content(main_layout[1], buf);

        // Render footer
        self.render_footer(main_layout[2], buf);

        // Render help overlay if active
        if self.show_help {
            self.render_help(area, buf);
        }

        // Render file info panel if active
        if self.state.show_file_info {
            self.render_file_info(area, buf);
        }

        // Render file operation modal if active
        if self.state.modal_active {
            self.render_modal(area, buf);
        }
    }

    /// Update visible line counts for scroll states
    ///
    /// Call this when the terminal size changes
    ///
    /// # Arguments
    ///
    /// * `tree_visible_lines` - Number of visible lines in tree panel
    /// * `preview_visible_lines` - Number of visible lines in preview panel
    pub fn update_visible_lines(
        &mut self,
        tree_visible_lines: usize,
        preview_visible_lines: usize,
    ) {
        self.state.tree_scroll.set_visible_lines(tree_visible_lines);
        self.state
            .preview_scroll
            .set_visible_lines(preview_visible_lines);

        // Calculate visible (filtered) count for tree
        let visible_count = self
            .state
            .tree
            .entries
            .iter()
            .filter(|e| {
                let show_hidden = self.state.show_hidden || !e.is_hidden;
                let show_ignored = self.state.show_ignored || !e.is_ignored;
                show_hidden && show_ignored
            })
            .count();
        self.state.tree_scroll.set_total_lines(visible_count);

        if let Some(ref preview) = self.state.preview {
            self.state
                .preview_scroll
                .set_total_lines(preview.total_lines);
        }
    }
}

impl Default for FileBrowserPlugin {
    fn default() -> Self {
        Self::new(PathBuf::from("."))
    }
}

fn file_browser_status_line(state: &PluginState) -> String {
    let visible = state.visible_indices().len();
    if state.tree.entries.is_empty() {
        return "No files yet | a: New file | A: New directory | r: Refresh files".to_string();
    }

    let filter = state
        .filter_query
        .as_ref()
        .map(|query| format!(" | filter: {}", truncate_display(query, 40)))
        .unwrap_or_default();

    if visible == 0 && state.filter_query.is_some() {
        return format!(
            "No matching files{} | f: Change filter | r: Refresh files",
            filter
        );
    }

    let selected = state
        .selected_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| {
            if visible == 0 {
                "No matching files".to_string()
            } else {
                "No file selected".to_string()
            }
        });

    if state.selected_path.is_none() && visible > 0 {
        return format!(
            "{} | {}{} | j/k: Navigate | Enter/Space: Toggle directory",
            selected,
            count_label(visible, "visible file", "visible files"),
            filter
        );
    }

    format!(
        "{} | {}{}",
        selected,
        count_label(visible, "visible file", "visible files"),
        filter
    )
}

fn file_tree_empty_message(state: &PluginState, width: u16) -> String {
    if let Some(query) = &state.filter_query {
        file_browser_empty_message(
            vec![
                format!("No files match \"{}\"", truncate_display(query, 40)),
                String::new(),
                "f: Change or clear filter".to_string(),
                "r: Refresh files".to_string(),
            ],
            width,
        )
    } else if state.tree.entries.is_empty() {
        file_browser_empty_message(
            vec![
                "No files yet".to_string(),
                String::new(),
                "a: New file".to_string(),
                "A: New directory".to_string(),
                "r: Refresh files".to_string(),
            ],
            width,
        )
    } else {
        file_browser_empty_message(
            vec![
                "No visible files".to_string(),
                String::new(),
                "H: Toggle hidden".to_string(),
                "i: Toggle ignored".to_string(),
                "f: Filter files".to_string(),
                "r: Refresh files".to_string(),
            ],
            width,
        )
    }
}

fn file_preview_empty_message(state: &PluginState, width: u16) -> String {
    if let Some(query) = &state.filter_query {
        file_browser_empty_message(
            vec![
                "No matching file to preview".to_string(),
                String::new(),
                format!("No files match \"{}\"", truncate_display(query, 40)),
                "f: Change or clear filter".to_string(),
                "r: Refresh files".to_string(),
            ],
            width,
        )
    } else if state.tree.entries.is_empty() {
        file_browser_empty_message(
            vec![
                "No file to preview yet".to_string(),
                String::new(),
                "a: New file".to_string(),
                "A: New directory".to_string(),
                "r: Refresh files".to_string(),
            ],
            width,
        )
    } else if state.visible_indices().is_empty() {
        file_browser_empty_message(
            vec![
                "No visible file to preview".to_string(),
                String::new(),
                "H: Toggle hidden".to_string(),
                "i: Toggle ignored".to_string(),
                "f: Filter files".to_string(),
                "r: Refresh files".to_string(),
            ],
            width,
        )
    } else {
        file_browser_empty_message(
            vec![
                "No file selected".to_string(),
                String::new(),
                "j/k: Navigate files".to_string(),
                "Enter/Space: Toggle directory".to_string(),
                "f: Filter files".to_string(),
                "r: Refresh files".to_string(),
            ],
            width,
        )
    }
}

fn file_info_empty_message(width: u16) -> String {
    file_browser_empty_message(
        vec![
            "No file selected".to_string(),
            String::new(),
            "j/k: Navigate files".to_string(),
            "Enter/Space: Toggle directory".to_string(),
            "I: Close info".to_string(),
            "r: Refresh files".to_string(),
        ],
        width,
    )
}

fn file_browser_empty_message(lines: Vec<String>, width: u16) -> String {
    global_hint_message(lines, width)
}

// Helper function for muted style
fn muted_style() -> Style {
    Style::default().fg(ratatui::style::Color::DarkGray)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_file_browser_plugin_new() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        assert_eq!(plugin.work_dir, temp_dir.path());
        assert!(plugin.focused);
        assert_eq!(plugin.name(), "File Browser");
        assert_eq!(plugin.display_name(), "File Browser");
    }

    #[test]
    fn test_file_operation_modal_hints_use_action_case() {
        let hints = [
            CREATE_ENTRY_MODAL_HINT,
            DELETE_ENTRY_MODAL_HINT,
            RENAME_ENTRY_MODAL_HINT,
            FILTER_FILES_MODAL_HINT,
            ERROR_MODAL_HINT,
        ];

        assert!(hints.iter().all(|hint| hint.contains("Enter")));
        assert!(hints.iter().all(|hint| !hint.contains(": create")));
        assert!(hints.iter().all(|hint| !hint.contains(": cancel")));
        assert!(DELETE_ENTRY_MODAL_HINT.contains("Enter/D: Delete"));
        assert!(FILTER_FILES_MODAL_HINT.contains("Empty input: Clear"));
        assert!(!FILTER_FILES_MODAL_HINT.contains("Empty: Clear"));
    }

    #[test]
    fn test_file_overlay_hints_use_compact_action_case() {
        let hints = [HELP_OVERLAY_HINT, FILE_INFO_OVERLAY_HINT];

        assert!(HELP_OVERLAY_HINT.contains(": Toggle help"));
        assert!(FILE_INFO_OVERLAY_HINT.contains(": Close"));
        assert!(HELP_OVERLAY_HINT.starts_with('?'));
        assert!(FILE_INFO_OVERLAY_HINT.starts_with('I'));
        assert!(!hints.iter().any(|hint| hint.starts_with("Press ")));
    }

    #[test]
    fn test_render_help_uses_short_help_label() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);

        plugin.render_help(area, &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("?      Toggle help"));
        assert!(content.contains("/      Global search"));
        assert!(content.contains(":      Command search"));
        assert!(!content.contains("Toggle this help"));
    }

    #[test]
    fn test_centered_overlay_area_uses_preferred_size_when_it_fits() {
        let area = Rect::new(10, 5, 100, 40);
        let popup = centered_overlay_area(area, 50, 20).unwrap();

        assert_eq!(popup, Rect::new(35, 15, 50, 20));
    }

    #[test]
    fn test_centered_overlay_area_clamps_to_available_area() {
        let area = Rect::new(4, 3, 40, 12);
        let popup = centered_overlay_area(area, 50, 30).unwrap();

        assert_eq!(popup, area);
    }

    #[test]
    fn test_centered_overlay_area_handles_offset_near_u16_max() {
        let area = Rect::new(u16::MAX - 80, u16::MAX - 40, 80, 40);
        let popup = centered_overlay_area(area, 50, 20).unwrap();

        assert_eq!(popup, Rect::new(u16::MAX - 65, u16::MAX - 30, 50, 20));
    }

    #[test]
    fn test_centered_overlay_area_skips_tiny_areas() {
        assert!(centered_overlay_area(Rect::new(0, 0, 23, 12), 50, 30).is_none());
        assert!(centered_overlay_area(Rect::new(0, 0, 40, 4), 50, 30).is_none());
    }

    #[test]
    fn test_file_browser_plugin_navigation() {
        let temp_dir = TempDir::new().unwrap();

        // Create test files
        fs::File::create(temp_dir.path().join("file1.txt")).unwrap();
        fs::File::create(temp_dir.path().join("file2.txt")).unwrap();

        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.refresh();

        // Test navigation actions
        assert!(plugin.handle_action(&Action::NavigateDown));
        assert!(plugin.handle_action(&Action::NavigateUp));
        assert!(plugin.handle_action(&Action::Refresh));
    }

    #[test]
    fn test_render_tree_entries_handles_offset_area_near_u16_max() {
        let temp_dir = TempDir::new().unwrap();
        fs::File::create(temp_dir.path().join("file.txt")).unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.refresh();
        plugin.state.show_icons = false;
        let area = Rect::new(u16::MAX - 30, u16::MAX - 2, 30, 3);
        let mut buf = Buffer::empty(area);

        plugin.render_tree_entries(area, &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("."));
    }

    #[test]
    fn test_file_browser_plugin_keys() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        // Test custom key handlers
        assert!(plugin.handle_key("i")); // Toggle ignored
        assert!(plugin.handle_key("I")); // Toggle file info
        assert!(plugin.handle_key("H")); // Toggle hidden
        assert!(plugin.handle_key("?")); // Toggle help
        assert!(plugin.handle_key("g")); // Go to top
        assert!(plugin.handle_key("G")); // Go to bottom

        // Unknown key
        assert!(!plugin.handle_key("unknown_key"));
    }

    #[test]
    fn test_file_browser_commands_include_visible_actions() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        let commands = plugin.commands();

        for (id, key, name) in [
            ("create_file", 'a', "New File"),
            ("create_dir", 'A', "New Directory"),
            ("delete", 'd', "Delete"),
            ("rename", 'R', "Rename"),
            ("filter", 'f', "Filter Files"),
            ("refresh", 'r', "Refresh Files"),
            ("toggle_ignored", 'i', "Toggle Ignored"),
            ("toggle_hidden", 'H', "Toggle Hidden"),
            ("file_info", 'I', "File Info"),
            ("preview_top", 'g', "Preview Top"),
            ("preview_bottom", 'G', "Preview Bottom"),
        ] {
            assert!(
                commands
                    .iter()
                    .any(|command| command.id == id && command.key == key && command.name == name),
                "missing command {id}"
            );
        }

        let prioritized: Vec<(&str, u8)> = commands
            .iter()
            .filter(|command| command.priority > 0)
            .map(|command| (command.id.as_str(), command.priority))
            .collect();
        assert_eq!(
            prioritized,
            vec![
                ("create_file", 2),
                ("create_dir", 1),
                ("filter", 5),
                ("refresh", 4),
                ("file_info", 3)
            ]
        );
    }

    #[test]
    fn test_file_browser_execute_command_opens_create_file_modal() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        let execution = plugin
            .execute_command("create_file")
            .expect("create file command should execute");

        assert_eq!(execution.command_name, "New File");
        assert!(plugin.state.modal_active);
        assert_eq!(
            plugin.state.active_modal,
            Some(FileOperationModal::CreateFile)
        );
    }

    #[test]
    fn test_file_browser_execute_command_opens_filter_modal() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        let execution = plugin
            .execute_command("filter")
            .expect("filter command should execute");

        assert_eq!(execution.command_name, "Filter Files");
        assert!(plugin.state.modal_active);
        assert_eq!(plugin.state.active_modal, Some(FileOperationModal::Filter));
    }

    #[test]
    fn test_file_browser_execute_refresh_command_uses_declared_shortcut() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        let execution = plugin
            .execute_command("refresh")
            .expect("refresh command should execute");

        assert_eq!(execution.command_name, "Refresh Files");
        assert!(execution.emitted_commands.is_empty());
        assert!(!plugin.state.modal_active);
    }

    #[test]
    fn test_file_browser_plugin_focus() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        assert!(plugin.is_focused());

        plugin.set_focused(false);
        assert!(!plugin.is_focused());

        plugin.set_focused(true);
        assert!(plugin.is_focused());
    }

    #[test]
    fn test_file_browser_plugin_focus_context() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        assert_eq!(plugin.focus_context(), FocusContext::FileBrowserTree);
    }

    #[test]
    fn test_file_browser_status_line_empty_tree() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.state.tree.entries.clear();
        plugin.state.selected_path = None;

        assert_eq!(
            plugin.status_line(),
            Some("No files yet | a: New file | A: New directory | r: Refresh files".to_string())
        );
    }

    #[test]
    fn test_file_browser_status_line_mentions_filter_miss() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("alpha.txt"), "alpha").unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.refresh();
        plugin.state.selected_path = None;
        plugin.state.set_filter(Some("missing".to_string()));

        assert_eq!(
            plugin.status_line(),
            Some(
                "No matching files | filter: missing | f: Change filter | r: Refresh files"
                    .to_string()
            )
        );
    }

    #[test]
    fn test_file_browser_status_line_truncates_long_filter_query() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("alpha.txt"), "alpha").unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.refresh();
        plugin.state.selected_path = None;
        plugin.state.set_filter(Some(
            "abcdefghijklmnopqrstuvwxyz0123456789abcdef".to_string(),
        ));

        let status = plugin.status_line().expect("status line");

        assert!(status.contains("filter: abcdefghijklmnopqrstuvwxyz0123456789a..."));
        assert!(!status.contains("abcdef |"));
        assert!(status.contains("f: Change filter"));
        assert!(status.contains("r: Refresh files"));
    }

    #[test]
    fn test_file_browser_status_line_uses_visible_file_counts() {
        use super::super::tree::FileEntry;

        let temp_dir = TempDir::new().unwrap();
        let alpha = temp_dir.path().join("alpha.txt");
        fs::write(&alpha, "alpha").unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());
        state.tree.entries.clear();
        state
            .tree
            .entries
            .push(FileEntry::new(alpha.clone(), 0, None, None));
        state.selected_path = None;

        assert_eq!(
            file_browser_status_line(&state),
            "No file selected | 1 visible file | j/k: Navigate | Enter/Space: Toggle directory"
        );

        let beta = temp_dir.path().join("beta.txt");
        fs::write(&beta, "beta").unwrap();
        state
            .tree
            .entries
            .push(FileEntry::new(beta.clone(), 0, None, None));

        assert_eq!(
            file_browser_status_line(&state),
            "No file selected | 2 visible files | j/k: Navigate | Enter/Space: Toggle directory"
        );
    }

    #[test]
    fn test_file_tree_empty_message_points_to_next_actions() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());
        state.tree.entries.clear();

        let message = file_tree_empty_message(&state, 80);

        assert!(message.contains("No files yet"));
        assert!(message.contains("a: New file"));
        assert!(message.contains("A: New directory"));
        assert!(message.contains("r: Refresh files"));
        assert!(message.contains("/: Global search"));
        assert!(message.contains(": Command search"));
        assert!(message.contains("?: Toggle help"));
        assert!(!message.contains("No files found"));
    }

    #[test]
    fn test_file_browser_empty_messages_surface_command_search() {
        let temp_dir = TempDir::new().unwrap();
        let mut empty_state = PluginState::new(temp_dir.path().to_path_buf());
        empty_state.tree.entries.clear();

        let assert_hint = |message: &str| {
            assert!(message.contains("/: Global search"), "{message}");
            assert!(message.contains(": Command search"), "{message}");
        };

        assert_hint(&file_tree_empty_message(&empty_state, 80));
        assert_hint(&file_preview_empty_message(&empty_state, 80));
        assert_hint(&file_info_empty_message(80));

        let mut filtered_state = PluginState::new(temp_dir.path().to_path_buf());
        filtered_state.set_filter(Some("missing".to_string()));
        assert_hint(&file_tree_empty_message(&filtered_state, 80));
        assert_hint(&file_preview_empty_message(&filtered_state, 80));

        let hidden_path = temp_dir.path().join(".hidden");
        fs::write(&hidden_path, "hidden").unwrap();
        let mut hidden_state = PluginState::new(temp_dir.path().to_path_buf());
        hidden_state.refresh();
        hidden_state.show_hidden = false;
        assert_hint(&file_tree_empty_message(&hidden_state, 80));
        assert_hint(&file_preview_empty_message(&hidden_state, 80));

        fs::write(temp_dir.path().join("alpha.txt"), "alpha").unwrap();
        let mut unselected_state = PluginState::new(temp_dir.path().to_path_buf());
        unselected_state.refresh();
        unselected_state.selected_path = None;
        assert_hint(&file_preview_empty_message(&unselected_state, 80));
    }

    #[test]
    fn test_file_browser_empty_messages_truncate_long_filter_query() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());
        state.set_filter(Some(
            "abcdefghijklmnopqrstuvwxyz0123456789abcdef".to_string(),
        ));

        let tree = file_tree_empty_message(&state, 80);
        let preview = file_preview_empty_message(&state, 80);

        assert!(tree.contains("No files match \"abcdefghijklmnopqrstuvwxyz0123456789a...\""));
        assert!(preview.contains("No files match \"abcdefghijklmnopqrstuvwxyz0123456789a...\""));
        assert!(!tree.contains("abcdef\""));
        assert!(!preview.contains("abcdef\""));
    }

    #[test]
    fn test_file_info_empty_message_omits_search_hint_when_too_narrow() {
        let message = file_info_empty_message(1);

        assert!(message.contains("No file selected"));
        assert!(!message.contains("Global search"));
        assert!(!message.contains("/:"));
        assert!(message.contains("?"));
        assert!(!message.contains("?: Toggle help"));
    }

    #[test]
    fn test_file_tree_empty_message_mentions_filter() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());
        state.set_filter(Some("missing".to_string()));

        let message = file_tree_empty_message(&state, 80);

        assert!(message.contains("No files match \"missing\""));
        assert!(message.contains("f: Change or clear filter"));
        assert!(message.contains("r: Refresh files"));
        assert!(message.contains("/: Global search"));
        assert!(message.contains(": Command search"));
        assert!(!message.contains("Esc  Close dialogs"));
        assert!(message.contains("?: Toggle help"));
    }

    #[test]
    fn test_file_preview_empty_message_mentions_filter_clear_action() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());
        state.set_filter(Some("missing".to_string()));

        let message = file_preview_empty_message(&state, 80);

        assert!(message.contains("No matching file to preview"));
        assert!(message.contains("No files match \"missing\""));
        assert!(message.contains("f: Change or clear filter"));
        assert!(message.contains("r: Refresh files"));
        assert!(message.contains("/: Global search"));
        assert!(message.contains(": Command search"));
        assert!(message.contains("?: Toggle help"));
        assert!(!message.contains("No preview available"));
    }

    #[test]
    fn test_file_tree_empty_message_handles_hidden_visible_miss() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".hidden"), "hidden").unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());
        state.refresh();
        state.show_hidden = false;

        let message = file_tree_empty_message(&state, 80);

        assert!(message.contains("No visible files"));
        assert!(message.contains("H: Toggle hidden"));
        assert!(message.contains("i: Toggle ignored"));
        assert!(message.contains("f: Filter files"));
        assert!(message.contains("r: Refresh files"));
        assert!(message.contains("/: Global search"));
        assert!(message.contains(": Command search"));
        assert!(message.contains("?: Toggle help"));
    }

    #[test]
    fn test_file_preview_empty_message_handles_hidden_visible_miss() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".hidden"), "hidden").unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());
        state.refresh();
        state.show_hidden = false;

        let message = file_preview_empty_message(&state, 80);

        assert!(message.contains("No visible file to preview"));
        assert!(message.contains("H: Toggle hidden"));
        assert!(message.contains("i: Toggle ignored"));
        assert!(message.contains("f: Filter files"));
        assert!(message.contains("r: Refresh files"));
        assert!(message.contains("/: Global search"));
        assert!(message.contains(": Command search"));
        assert!(message.contains("?: Toggle help"));
        assert!(!message.contains("No preview available"));
    }

    #[test]
    fn test_file_preview_empty_message_points_to_creation_when_tree_empty() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());
        state.tree.entries.clear();

        let message = file_preview_empty_message(&state, 80);

        assert!(message.contains("No file to preview yet"));
        assert!(message.contains("a: New file"));
        assert!(message.contains("A: New directory"));
        assert!(message.contains("r: Refresh files"));
        assert!(message.contains("/: Global search"));
        assert!(message.contains(": Command search"));
        assert!(message.contains("?: Toggle help"));
        assert!(!message.contains("No preview available"));
    }

    #[test]
    fn test_render_empty_preview_points_to_navigation_actions() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("alpha.txt"), "alpha").unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.refresh();
        plugin.state.selected_path = None;
        plugin.state.preview = None;

        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);
        plugin.render_internal(area, &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No file selected"));
        assert!(!content.contains("No preview selected"));
        assert!(content.contains("j/k: Navigate files"));
        assert!(content.contains("Enter/Space: Toggle directory"));
        assert!(content.contains("f: Filter files"));
        assert!(content.contains("r: Refresh files"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
        assert!(content.contains("?: Toggle help"));
    }

    #[test]
    fn test_render_footer_includes_refresh_and_search_hints() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        let area = Rect::new(0, 0, 320, 1);
        let mut buf = Buffer::empty(area);

        plugin.render_footer(area, &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("r: Refresh files"));
        assert!(content.contains("f: Filter files"));
        assert!(content.contains("a/A: New file/directory"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
    }

    #[test]
    fn test_render_file_info_without_selection_points_to_navigation() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("alpha.txt"), "alpha").unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.refresh();
        plugin.state.tree.entries.clear();
        plugin.state.selected_path = None;
        plugin.state.preview = None;
        plugin.state.show_file_info = true;

        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);
        plugin.render_internal(area, &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("No file selected"));
        assert!(content.contains("j/k: Navigate files"));
        assert!(content.contains("Enter/Space: Toggle directory"));
        assert!(content.contains("Close info"));
        assert!(content.contains("r: Refresh files"));
        assert!(content.contains("/: Global search"));
        assert!(content.contains(": Command search"));
        assert!(content.contains("?: Toggle help"));
    }

    #[test]
    fn test_file_browser_plugin_navigate_to() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.navigate_to(sub_dir.clone());

        assert_eq!(plugin.selected_path(), Some(&sub_dir));
    }

    #[test]
    fn test_reveal_path_selects_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target.txt");
        fs::write(&target, "hello").unwrap();

        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        assert!(plugin.reveal_path(&target));
        assert_eq!(plugin.selected_path(), Some(&target));
    }

    #[test]
    fn test_key_a_opens_create_file_modal() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        assert!(!plugin.state.modal_active);
        assert!(plugin.handle_key("a"));
        assert!(plugin.state.modal_active);
        assert_eq!(
            plugin.state.active_modal,
            Some(FileOperationModal::CreateFile)
        );
    }

    #[test]
    fn test_key_shift_a_opens_create_dir_modal() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        assert!(plugin.handle_key("A"));
        assert!(plugin.state.modal_active);
        assert_eq!(
            plugin.state.active_modal,
            Some(FileOperationModal::CreateDir)
        );
    }

    #[test]
    fn test_key_d_opens_delete_modal() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("target.txt");
        fs::File::create(&test_file).unwrap();

        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.refresh();

        // Navigate to the file (entry 0 is root ".", entry 1 should be the file)
        plugin.state.tree.selected_index = 1;
        plugin.state.update_selected_path_public();

        assert!(plugin.handle_key("d"));
        assert!(plugin.state.modal_active);
        match &plugin.state.active_modal {
            Some(FileOperationModal::Delete { path, is_dir }) => {
                assert_eq!(path, &test_file);
                assert!(!is_dir);
            }
            other => panic!("Expected Delete modal, got {:?}", other),
        }
    }

    #[test]
    fn test_key_shift_r_opens_rename_modal() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("original.txt");
        fs::File::create(&test_file).unwrap();

        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.refresh();

        // Navigate to the file
        plugin.state.tree.selected_index = 1;
        plugin.state.update_selected_path_public();

        assert!(plugin.handle_key("R"));
        assert!(plugin.state.modal_active);
        match &plugin.state.active_modal {
            Some(FileOperationModal::Rename {
                path,
                original_name,
            }) => {
                assert_eq!(path, &test_file);
                assert_eq!(original_name, "original.txt");
                // Input buffer should be pre-filled with original name
                assert_eq!(plugin.state.input_buffer, "original.txt");
            }
            other => panic!("Expected Rename modal, got {:?}", other),
        }
    }

    #[test]
    fn test_consumes_text_input_when_modal_active() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        // No modal active -> does not consume text input
        assert!(!plugin.consumes_text_input());

        // Open a modal -> consumes text input
        plugin.state.open_modal(FileOperationModal::CreateFile);
        assert!(plugin.consumes_text_input());

        // Close modal -> no longer consumes
        plugin.state.close_modal();
        assert!(!plugin.consumes_text_input());
    }

    #[test]
    fn test_modal_escape_closes_modal() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        plugin.state.open_modal(FileOperationModal::CreateFile);
        assert!(plugin.state.modal_active);

        let modifiers = crate::event::KeyModifiers::default();
        plugin.handle_modal_key("Escape", &modifiers);

        assert!(!plugin.state.modal_active);
        assert!(plugin.state.active_modal.is_none());
    }

    #[test]
    fn test_modal_text_input() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        plugin.state.open_modal(FileOperationModal::CreateFile);
        let modifiers = crate::event::KeyModifiers::default();

        // Type characters
        plugin.handle_modal_key("t", &modifiers);
        plugin.handle_modal_key("e", &modifiers);
        plugin.handle_modal_key("s", &modifiers);
        plugin.handle_modal_key("t", &modifiers);
        assert_eq!(plugin.state.input_buffer, "test");

        // Backspace
        plugin.handle_modal_key("Backspace", &modifiers);
        assert_eq!(plugin.state.input_buffer, "tes");
    }

    #[test]
    fn test_modal_confirm_create_file_queues_command() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        plugin.state.open_modal(FileOperationModal::CreateFile);
        plugin.state.input_buffer = "new_file.txt".to_string();

        plugin.confirm_modal_action();

        assert!(!plugin.state.modal_active);
        assert_eq!(plugin.pending_commands.len(), 1);
        match &plugin.pending_commands[0] {
            FileCommand::CreateFile(path) => {
                assert!(path.to_string_lossy().contains("new_file.txt"));
            }
            other => panic!("Expected CreateFile command, got {:?}", other),
        }
    }

    #[test]
    fn test_modal_confirm_create_dir_queues_command() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        plugin.state.open_modal(FileOperationModal::CreateDir);
        plugin.state.input_buffer = "new_dir".to_string();

        plugin.confirm_modal_action();

        assert!(!plugin.state.modal_active);
        assert_eq!(plugin.pending_commands.len(), 1);
        match &plugin.pending_commands[0] {
            FileCommand::CreateDir(path) => {
                assert!(path.to_string_lossy().contains("new_dir"));
            }
            other => panic!("Expected CreateDir command, got {:?}", other),
        }
    }

    #[test]
    fn test_modal_confirm_delete_queues_command() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("to_delete.txt");
        fs::File::create(&test_file).unwrap();

        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        plugin.state.open_modal(FileOperationModal::Delete {
            path: test_file.clone(),
            is_dir: false,
        });

        plugin.confirm_modal_action();

        assert!(!plugin.state.modal_active);
        assert_eq!(plugin.pending_commands.len(), 1);
        assert_eq!(
            plugin.pending_commands[0],
            FileCommand::DeletePath(test_file)
        );
    }

    #[test]
    fn test_modal_d_confirms_delete() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("to_delete.txt");
        fs::File::create(&test_file).unwrap();

        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.state.open_modal(FileOperationModal::Delete {
            path: test_file.clone(),
            is_dir: false,
        });

        let modifiers = crate::event::KeyModifiers::default();
        plugin.handle_modal_key("D", &modifiers);

        assert!(!plugin.state.modal_active);
        assert_eq!(
            plugin.pending_commands[0],
            FileCommand::DeletePath(test_file)
        );
    }

    #[test]
    fn test_modal_d_still_types_in_text_modal() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.state.open_modal(FileOperationModal::CreateFile);

        let modifiers = crate::event::KeyModifiers::default();
        plugin.handle_modal_key("d", &modifiers);

        assert!(plugin.state.modal_active);
        assert_eq!(plugin.state.input_buffer, "d");
        assert!(plugin.pending_commands.is_empty());
    }

    #[test]
    fn test_render_delete_modal_uses_handled_key_hint() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("to_delete.txt");
        fs::File::create(&test_file).unwrap();

        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.state.open_modal(FileOperationModal::Delete {
            path: test_file,
            is_dir: false,
        });
        let area = Rect::new(0, 0, 100, 24);
        let mut buf = Buffer::empty(area);

        plugin.render_modal(area, &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Delete file \"to_delete.txt\"?"));
        assert!(content.contains(DELETE_ENTRY_MODAL_HINT));
        assert!(!content.contains("Enter: Delete"));
    }

    #[test]
    fn test_modal_confirm_rename_queues_command() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("old_name.txt");
        fs::File::create(&test_file).unwrap();

        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        plugin.state.open_modal(FileOperationModal::Rename {
            path: test_file.clone(),
            original_name: "old_name.txt".to_string(),
        });
        plugin.state.input_buffer = "new_name.txt".to_string();

        plugin.confirm_modal_action();

        assert!(!plugin.state.modal_active);
        assert_eq!(plugin.pending_commands.len(), 1);
        match &plugin.pending_commands[0] {
            FileCommand::RenamePath { from, to } => {
                assert_eq!(from, &test_file);
                assert_eq!(to, &temp_dir.path().join("new_name.txt"));
            }
            other => panic!("Expected RenamePath command, got {:?}", other),
        }
    }

    #[test]
    fn test_filter_modal_applies_filter_without_file_command() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        plugin.state.open_modal(FileOperationModal::Filter);
        plugin.state.input_buffer = "src".to_string();

        plugin.confirm_modal_action();

        assert!(!plugin.state.modal_active);
        assert_eq!(plugin.state.filter_query.as_deref(), Some("src"));
        assert!(plugin.pending_commands.is_empty());
    }

    #[test]
    fn test_modal_confirm_empty_input_does_not_queue() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        plugin.state.open_modal(FileOperationModal::CreateFile);
        // Leave input buffer empty

        plugin.confirm_modal_action();

        assert!(!plugin.state.modal_active);
        assert!(plugin.pending_commands.is_empty());
    }

    #[test]
    fn test_modal_error_closes_on_confirm() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        plugin.state.open_modal(FileOperationModal::Error {
            message: "Test error".to_string(),
        });
        assert!(plugin.state.modal_active);

        plugin.confirm_modal_action();
        assert!(!plugin.state.modal_active);
        assert!(plugin.pending_commands.is_empty());
    }

    #[test]
    fn test_delete_modal_does_not_accept_text_input() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        plugin.state.open_modal(FileOperationModal::Delete {
            path: temp_dir.path().join("file.txt"),
            is_dir: false,
        });

        let modifiers = crate::event::KeyModifiers::default();
        plugin.handle_modal_key("x", &modifiers);
        plugin.handle_modal_key("y", &modifiers);
        plugin.handle_modal_key("z", &modifiers);

        // Input buffer should remain empty for delete modals
        assert!(plugin.state.input_buffer.is_empty());
    }

    #[tokio::test]
    async fn test_execute_file_command_create_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        let new_file = temp_dir.path().join("created.txt");

        let result = plugin
            .execute_file_command(FileCommand::CreateFile(new_file.clone()))
            .await;
        assert!(result.is_ok());
        assert!(new_file.exists());
    }

    #[tokio::test]
    async fn test_execute_file_command_create_dir() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        let new_dir = temp_dir.path().join("created_dir");

        let result = plugin
            .execute_file_command(FileCommand::CreateDir(new_dir.clone()))
            .await;
        assert!(result.is_ok());
        assert!(new_dir.exists());
        assert!(new_dir.is_dir());
    }

    #[tokio::test]
    async fn test_execute_file_command_delete_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        let file_to_delete = temp_dir.path().join("to_delete.txt");
        fs::File::create(&file_to_delete).unwrap();
        assert!(file_to_delete.exists());

        let result = plugin
            .execute_file_command(FileCommand::DeletePath(file_to_delete.clone()))
            .await;
        assert!(result.is_ok());
        assert!(!file_to_delete.exists());
    }

    #[tokio::test]
    async fn test_execute_file_command_delete_dir() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        let dir_to_delete = temp_dir.path().join("to_delete_dir");
        fs::create_dir(&dir_to_delete).unwrap();
        fs::File::create(dir_to_delete.join("inner.txt")).unwrap();
        assert!(dir_to_delete.exists());

        let result = plugin
            .execute_file_command(FileCommand::DeletePath(dir_to_delete.clone()))
            .await;
        assert!(result.is_ok());
        assert!(!dir_to_delete.exists());
    }

    #[tokio::test]
    async fn test_execute_file_command_rename() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        let from = temp_dir.path().join("old_name.txt");
        let to = temp_dir.path().join("new_name.txt");
        fs::File::create(&from).unwrap();

        let result = plugin
            .execute_file_command(FileCommand::RenamePath {
                from: from.clone(),
                to: to.clone(),
            })
            .await;
        assert!(result.is_ok());
        assert!(!from.exists());
        assert!(to.exists());
    }

    #[test]
    fn test_get_base_dir_for_create_with_dir_selected() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.refresh();

        // Select the subdirectory (entry after root)
        for (i, entry) in plugin.state.tree.entries.iter().enumerate() {
            if entry.path == sub_dir {
                plugin.state.tree.selected_index = i;
                break;
            }
        }
        plugin.state.update_selected_path_public();

        let base = plugin.get_base_dir_for_create();
        assert_eq!(base, sub_dir);
    }

    #[test]
    fn test_get_base_dir_for_create_with_file_selected() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::File::create(&test_file).unwrap();

        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.refresh();

        // Select the file
        for (i, entry) in plugin.state.tree.entries.iter().enumerate() {
            if entry.path == test_file {
                plugin.state.tree.selected_index = i;
                break;
            }
        }
        plugin.state.update_selected_path_public();

        let base = plugin.get_base_dir_for_create();
        // Should return parent of file, which is the temp_dir
        assert_eq!(base, temp_dir.path().to_path_buf());
    }

    #[test]
    fn test_event_handling_routes_to_modal_when_active() {
        let temp_dir = TempDir::new().unwrap();
        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());

        // Open a modal
        plugin.state.open_modal(FileOperationModal::CreateFile);

        // Send a key event - should route to modal handler, not normal handler
        let event = Event::Key {
            code: "t".to_string(),
            modifiers: crate::event::KeyModifiers::default(),
        };
        plugin.handle_event(event);

        // The "t" should have been added to input buffer (modal text input)
        assert_eq!(plugin.state.input_buffer, "t");
    }
}
