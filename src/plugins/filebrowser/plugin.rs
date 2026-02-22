//! Main file browser plugin implementation
//!
//! This module provides the `FileBrowserPlugin` struct which implements
//! the complete file browser functionality including tree view, file preview,
//! and keyboard navigation.

use std::path::PathBuf;

use async_trait::async_trait;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::core::models::Theme;
use crate::event::Event;
use crate::keymap::{Action, FocusContext};
use crate::plugin::{Command, Plugin, PluginCommand, PluginContext};
use crate::theme::{UiElement, style_for_ui_element};
use crate::ui::{Footer, Header, KeyHint};

use super::preview::PreviewWidget;
use super::state::PluginState;
use super::tree::FileTreeWidget;

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
                // Handle Ctrl+key combinations first
                if modifiers.ctrl {
                    match code.as_str() {
                        "d" => {
                            self.state
                                .scroll_preview_down(self.state.preview_scroll.visible_lines / 2);
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
            PluginCommand::with_context(
                "refresh",
                "Refresh",
                'r',
                crate::keymap::FocusContext::FileBrowserTree,
            ),
            PluginCommand::with_context(
                "toggle_hidden",
                "Toggle Hidden",
                'H',
                crate::keymap::FocusContext::FileBrowserTree,
            ),
            PluginCommand::with_context(
                "toggle_ignored",
                "Toggle Ignored",
                'i',
                crate::keymap::FocusContext::FileBrowserTree,
            ),
        ]
    }

    fn focus_context(&self) -> FocusContext {
        FocusContext::FileBrowserTree
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
                // TODO: Open search input
                true
            }
            Action::Filter => {
                // TODO: Open filter input
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
            _ => false,
        }
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
            .tree
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                let show_hidden = self.state.show_hidden || !e.is_hidden;
                let show_ignored = self.state.show_ignored || !e.is_ignored;
                show_hidden && show_ignored
            })
            .take_while(|(i, _)| *i != self.state.tree.selected_index)
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
        let tree_block = Block::default()
            .title(if self.state.show_ignored {
                " Files (showing ignored) "
            } else {
                " Files "
            })
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

            let no_preview = Paragraph::new("No preview available").style(text_style);
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

        // Build list of visible entry indices (not filtered)
        let visible_indices: Vec<usize> = self
            .state
            .tree
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                // Filter hidden files
                let show_hidden = self.state.show_hidden || !e.is_hidden;
                // Filter git-ignored files
                let show_ignored = self.state.show_ignored || !e.is_ignored;
                show_hidden && show_ignored
            })
            .map(|(i, _)| i)
            .collect();

        // Calculate visible range based on scroll offset
        let scroll_offset = self.state.tree_scroll.offset;
        let visible_count = area.height as usize;

        // Update scroll state total
        let total_visible = visible_indices.len();

        let mut row = 0u16;
        for (_vis_pos, &entry_idx) in visible_indices
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_count)
        {
            let y = area.y + row;
            if y >= area.y + area.height {
                break;
            }
            row += 1;

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
        let hints = vec![
            KeyHint::new("j/k", "Navigate"),
            KeyHint::new("↵/space", "Expand/Collapse"),
            KeyHint::new("i", "Toggle ignored"),
            KeyHint::new("H", "Toggle hidden"),
            KeyHint::new("I", "File info"),
            KeyHint::new("Ctrl+d/u", "Page scroll"),
            KeyHint::new("?", "Help"),
            KeyHint::new("q", "Quit"),
        ];

        let status = if let Some(ref path) = self.state.selected_path {
            format!("{}", path.display())
        } else {
            "No selection".to_string()
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

        // Calculate popup size
        let popup_width = 50u16;
        let popup_height = 20u16;
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

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
                Span::styled("Toggle this help", text_style),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled("Press ? to close", muted_style())]),
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

        // Calculate popup size
        let popup_width = 40u16;
        let popup_height = 12u16;
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

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
        let info_text = if let Some(ref entry) = self.state.selected_entry() {
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
                "Press I to close",
                muted_style,
            )]));

            lines
        } else {
            vec![Line::from(vec![Span::styled(
                "No file selected",
                muted_style,
            )])]
        };

        let info_para = Paragraph::new(info_text);
        info_para.render(inner, buf);
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
    fn test_file_browser_plugin_navigate_to() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let mut plugin = FileBrowserPlugin::new(temp_dir.path().to_path_buf());
        plugin.navigate_to(sub_dir.clone());

        assert_eq!(plugin.selected_path(), Some(&sub_dir));
    }
}
