//! Plugin state for the file browser
//!
//! This module defines the `PluginState` structure that holds all the state
//! needed by the file browser plugin.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::ui::ScrollState;

use super::preview::Preview;
use super::tree::FileTree;

/// Types of file operation modals
#[derive(Clone, Debug, PartialEq)]
pub enum FileOperationModal {
    /// Create a new file
    CreateFile,
    /// Create a new directory
    CreateDir,
    /// Delete a file or directory
    Delete { path: PathBuf, is_dir: bool },
    /// Rename a file or directory
    Rename {
        path: PathBuf,
        original_name: String,
    },
    /// Filter visible file entries by name
    Filter,
    /// Error display
    Error { message: String },
}

/// The complete state of the file browser plugin
#[derive(Clone, Debug)]
pub struct PluginState {
    /// The file tree structure
    pub tree: FileTree,
    /// Currently selected path
    pub selected_path: Option<PathBuf>,
    /// Set of expanded directory paths
    pub expanded_dirs: HashSet<PathBuf>,
    /// Current file preview
    pub preview: Option<Preview>,
    /// Scroll offset for the tree view
    pub tree_scroll: ScrollState,
    /// Scroll offset for the preview
    pub preview_scroll: ScrollState,
    /// Whether to show git-ignored files
    pub show_ignored: bool,
    /// Whether to show hidden files (starting with .)
    pub show_hidden: bool,
    /// Whether to show file icons
    pub show_icons: bool,
    /// Current working directory
    pub work_dir: PathBuf,
    /// Show detailed file info panel
    pub show_file_info: bool,
    /// Current file filter query
    pub filter_query: Option<String>,
    /// Filtered indices for the current file filter query
    pub filtered_indices: Vec<usize>,
    /// Whether a modal is currently active
    pub modal_active: bool,
    /// The currently active modal, if any
    pub active_modal: Option<FileOperationModal>,
    /// Text input buffer for modal text fields (create file/dir, rename)
    pub input_buffer: String,
}

impl PluginState {
    /// Create a new plugin state with the given working directory
    ///
    /// # Arguments
    ///
    /// * `work_dir` - The working directory to browse
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::plugins::filebrowser::PluginState;
    /// use std::path::PathBuf;
    ///
    /// let state = PluginState::new(PathBuf::from("."));
    /// ```
    pub fn new(work_dir: PathBuf) -> Self {
        let tree = FileTree::new(work_dir.clone());

        Self {
            tree,
            selected_path: Some(work_dir.clone()),
            expanded_dirs: HashSet::new(),
            preview: None,
            tree_scroll: ScrollState::new(24),
            preview_scroll: ScrollState::new(24),
            show_ignored: false,
            show_hidden: true,
            show_icons: true,
            work_dir,
            show_file_info: false,
            filter_query: None,
            filtered_indices: Vec::new(),
            modal_active: false,
            active_modal: None,
            input_buffer: String::new(),
        }
    }

    /// Refresh the file tree and update the preview
    pub fn refresh(&mut self) {
        self.tree.refresh();
        self.update_preview();
    }

    /// Open a modal dialog for a file operation
    ///
    /// # Arguments
    ///
    /// * `modal` - The type of modal to open
    pub fn open_modal(&mut self, modal: FileOperationModal) {
        self.input_buffer = match &modal {
            FileOperationModal::Rename { original_name, .. } => original_name.clone(),
            FileOperationModal::Filter => self.filter_query.clone().unwrap_or_default(),
            _ => String::new(),
        };
        self.active_modal = Some(modal);
        self.modal_active = true;
    }

    /// Close the currently active modal
    pub fn close_modal(&mut self) {
        self.active_modal = None;
        self.modal_active = false;
        self.input_buffer.clear();
    }

    /// Get indices of visible entries (respecting hidden/ignored filters)
    pub fn visible_indices(&self) -> Vec<usize> {
        let lower_query = self.filter_query.as_ref().map(|query| query.to_lowercase());

        self.tree
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                let show_hidden = self.show_hidden || !e.is_hidden;
                let show_ignored = self.show_ignored || !e.is_ignored;
                let matches_filter = lower_query.as_ref().is_none_or(|query| {
                    e.name.to_lowercase().contains(query)
                        || e.path.to_string_lossy().to_lowercase().contains(query)
                });
                show_hidden && show_ignored && matches_filter
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Navigate to the next visible entry in the tree
    pub fn next_entry(&mut self) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            return;
        }

        // Find current position in visible list
        let current_pos = visible.iter().position(|&i| i == self.tree.selected_index);

        let next_idx = match current_pos {
            Some(pos) if pos + 1 < visible.len() => visible[pos + 1],
            _ => visible[0], // Wrap to start
        };

        self.tree.selected_index = next_idx;
        self.update_selected_path_public();
        self.update_preview();
    }

    /// Navigate to the previous visible entry in the tree
    pub fn prev_entry(&mut self) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            return;
        }

        // Find current position in visible list
        let current_pos = visible.iter().position(|&i| i == self.tree.selected_index);

        let prev_idx = match current_pos {
            Some(0) => *visible.last().unwrap(), // Wrap to end
            Some(pos) => visible[pos - 1],
            None => visible[0],
        };

        self.tree.selected_index = prev_idx;
        self.update_selected_path_public();
        self.update_preview();
    }

    /// Expand the currently selected directory
    pub fn expand_selected(&mut self) {
        if let Some(entry) = self.tree.selected() {
            if entry.is_dir {
                let path = entry.path.clone();
                self.tree.expand(&path);
                self.expanded_dirs.insert(path);
            }
        }
    }

    /// Collapse the currently selected directory
    pub fn collapse_selected(&mut self) {
        if let Some(entry) = self.tree.selected() {
            if entry.is_dir {
                let path = entry.path.clone();
                self.tree.collapse(&path);
                self.expanded_dirs.remove(&path);
            }
        }
    }

    /// Toggle the expanded/collapsed state of the currently selected directory
    pub fn toggle_selected(&mut self) {
        if let Some(entry) = self.tree.selected() {
            if entry.is_dir {
                let path = entry.path.clone();
                if entry.is_expanded {
                    self.tree.collapse(&path);
                    self.expanded_dirs.remove(&path);
                } else {
                    self.tree.expand(&path);
                    self.expanded_dirs.insert(path);
                }
            }
        }
    }

    /// Update the selected path from the tree
    pub fn update_selected_path_public(&mut self) {
        self.selected_path = self.tree.selected().map(|e| e.path.clone());
        self.update_tree_scroll_total();
    }

    /// Update tree scroll total based on visible entries
    fn update_tree_scroll_total(&mut self) {
        let total = self.visible_indices().len();
        self.tree_scroll.set_total_lines(total);
    }

    /// Update the preview based on the currently selected path
    pub fn update_preview(&mut self) {
        if let Some(ref path) = self.selected_path {
            if path.is_dir() {
                self.preview = Preview::from_directory(path);
            } else {
                self.preview = Preview::from_file(path);
            }

            // Reset preview scroll when changing files
            self.preview_scroll.scroll_to_top();

            // Update scroll state for preview
            if let Some(ref preview) = self.preview {
                self.preview_scroll.set_total_lines(preview.total_lines);
            }
        } else {
            self.preview = None;
        }
    }

    /// Toggle visibility of git-ignored files
    pub fn toggle_ignored(&mut self) {
        self.show_ignored = !self.show_ignored;
        self.update_tree_scroll_total();
        self.refresh();
    }

    /// Toggle visibility of hidden files
    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.update_tree_scroll_total();
    }

    /// Toggle the file info panel
    pub fn toggle_file_info(&mut self) {
        self.show_file_info = !self.show_file_info;
    }

    /// Set a filter query for visible files
    ///
    /// # Arguments
    ///
    /// * `query` - The filter query (None to clear)
    pub fn set_filter(&mut self, query: Option<String>) {
        self.filter_query = query.and_then(|q| {
            let trimmed = q.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        });

        if self.filter_query.is_some() {
            self.filtered_indices = self.visible_indices();
        } else {
            self.filtered_indices.clear();
        }

        self.update_tree_scroll_total();
        let visible = self.visible_indices();
        if visible.is_empty() {
            self.selected_path = None;
            self.preview = None;
        } else if !visible.contains(&self.tree.selected_index) || self.selected_path.is_none() {
            self.tree.selected_index = visible[0];
            self.update_selected_path_public();
            self.update_preview();
        }
    }

    /// Navigate to the next filtered result
    pub fn next_filtered(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }

        let current = self.tree.selected_index;
        let next = self
            .filtered_indices
            .iter()
            .find(|&&idx| idx > current)
            .copied()
            .or_else(|| self.filtered_indices.first().copied());

        if let Some(idx) = next {
            self.tree.selected_index = idx;
            self.update_selected_path_public();
            self.update_preview();
        }
    }

    /// Navigate to the previous filtered result
    pub fn prev_filtered(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }

        let current = self.tree.selected_index;
        let prev = self
            .filtered_indices
            .iter()
            .rev()
            .find(|&&idx| idx < current)
            .copied()
            .or_else(|| self.filtered_indices.last().copied());

        if let Some(idx) = prev {
            self.tree.selected_index = idx;
            self.update_selected_path_public();
            self.update_preview();
        }
    }

    /// Scroll the preview down
    pub fn scroll_preview_down(&mut self, lines: usize) {
        self.preview_scroll.scroll_down(lines);
    }

    /// Scroll the preview up
    pub fn scroll_preview_up(&mut self, lines: usize) {
        self.preview_scroll.scroll_up(lines);
    }

    /// Scroll the preview to the top
    pub fn scroll_preview_to_top(&mut self) {
        self.preview_scroll.scroll_to_top();
    }

    /// Scroll the preview to the bottom
    pub fn scroll_preview_to_bottom(&mut self) {
        self.preview_scroll.scroll_to_bottom();
    }

    /// Get the currently selected entry
    pub fn selected_entry(&self) -> Option<&super::tree::FileEntry> {
        self.tree.selected()
    }

    /// Check if the currently selected entry is a directory
    pub fn is_selected_dir(&self) -> bool {
        self.selected_entry().map(|e| e.is_dir).unwrap_or(false)
    }

    /// Get the number of entries in the tree
    pub fn entry_count(&self) -> usize {
        self.tree.len()
    }

    /// Get the current filter count
    pub fn filter_count(&self) -> usize {
        self.filtered_indices.len()
    }
}

impl Default for PluginState {
    fn default() -> Self {
        Self::new(PathBuf::from("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_plugin_state_new() {
        let temp_dir = TempDir::new().unwrap();
        let state = PluginState::new(temp_dir.path().to_path_buf());

        assert_eq!(state.work_dir, temp_dir.path());
        assert!(state.selected_path.is_some());
        assert!(state.show_hidden);
        assert!(!state.show_ignored);
    }

    #[test]
    fn test_plugin_state_navigation() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());

        // Create some files to navigate
        fs::File::create(temp_dir.path().join("file1.txt")).unwrap();
        fs::File::create(temp_dir.path().join("file2.txt")).unwrap();

        state.refresh();

        let initial_path = state.selected_path.clone();
        state.next_entry();
        // Path may or may not change depending on tree state
        let _ = state.selected_path.clone();

        state.prev_entry();
        assert_eq!(state.selected_path, initial_path);
    }

    #[test]
    fn test_plugin_state_filter() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());

        // Create test files
        fs::File::create(temp_dir.path().join("test.txt")).unwrap();
        fs::File::create(temp_dir.path().join("other.rs")).unwrap();

        state.refresh();

        // Set filter
        state.set_filter(Some("test.txt".to_string()));
        assert_eq!(state.filter_query.as_deref(), Some("test.txt"));
        assert_eq!(state.filter_count(), 1);
        assert_eq!(
            state
                .selected_path
                .as_ref()
                .and_then(|path| path.file_name()),
            Some(std::ffi::OsStr::new("test.txt"))
        );

        // Clear filter
        state.set_filter(None);
        assert_eq!(state.filter_count(), 0);
    }

    #[test]
    fn test_plugin_state_toggles() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());

        // Test show_ignored toggle
        let initial = state.show_ignored;
        state.toggle_ignored();
        assert_eq!(state.show_ignored, !initial);

        // Test show_hidden toggle
        let initial = state.show_hidden;
        state.toggle_hidden();
        assert_eq!(state.show_hidden, !initial);

        // Test show_file_info toggle
        let initial = state.show_file_info;
        state.toggle_file_info();
        assert_eq!(state.show_file_info, !initial);
    }

    #[test]
    fn test_filter_limits_visible_entries() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("alpha.txt"), "alpha").unwrap();
        fs::write(temp_dir.path().join("beta.txt"), "beta").unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());

        state.set_filter(Some("alpha".to_string()));

        assert_eq!(state.filter_query.as_deref(), Some("alpha"));
        assert_eq!(state.filter_count(), 1);
        let visible_names: Vec<_> = state
            .visible_indices()
            .into_iter()
            .map(|idx| state.tree.entries[idx].name.as_str())
            .collect();
        assert!(visible_names.contains(&"alpha.txt"));
        assert!(!visible_names.contains(&"beta.txt"));
    }

    #[test]
    fn test_filter_count_includes_path_only_matches() {
        let temp_dir = TempDir::new().unwrap();
        let parent = temp_dir.path().join("parent");
        fs::create_dir(&parent).unwrap();
        fs::write(parent.join("report.txt"), "report").unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());
        state.tree.expand(&parent);

        state.set_filter(Some(format!("parent{}report", std::path::MAIN_SEPARATOR)));

        assert_eq!(state.filter_count(), 1);
        assert_eq!(state.filtered_indices, state.visible_indices());
        assert_eq!(
            state
                .selected_path
                .as_ref()
                .and_then(|path| path.file_name()),
            Some(std::ffi::OsStr::new("report.txt"))
        );
    }

    #[test]
    fn test_filter_without_visible_entries_clears_selection_and_preview() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("alpha.txt"), "alpha").unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());

        state.set_filter(Some("alpha".to_string()));
        assert!(state.selected_path.is_some());
        assert!(state.preview.is_some());

        state.set_filter(Some("missing".to_string()));

        assert_eq!(state.visible_indices(), Vec::<usize>::new());
        assert!(state.selected_path.is_none());
        assert!(state.preview.is_none());
        assert_eq!(state.tree_scroll.total_lines, 0);
    }

    #[test]
    fn test_clearing_filter_restores_selection_after_empty_filter() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("alpha.txt"), "alpha").unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());

        state.set_filter(Some("missing".to_string()));
        assert!(state.selected_path.is_none());

        state.set_filter(None);

        assert!(state.selected_path.is_some());
        assert!(state.preview.is_some());
        assert!(state.tree_scroll.total_lines > 0);
    }

    #[test]
    fn test_empty_filter_clears_filter() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());

        state.set_filter(Some("   ".to_string()));

        assert!(state.filter_query.is_none());
        assert!(state.filtered_indices.is_empty());
    }

    #[test]
    fn test_plugin_state_scroll_preview() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());

        // Set preview scroll
        state.preview_scroll.set_total_lines(100);
        state.preview_scroll.set_visible_lines(10);

        assert_eq!(state.preview_scroll.offset, 0);

        state.scroll_preview_down(5);
        assert_eq!(state.preview_scroll.offset, 5);

        state.scroll_preview_up(2);
        assert_eq!(state.preview_scroll.offset, 3);

        state.scroll_preview_to_bottom();
        assert!(state.preview_scroll.is_at_bottom());

        state.scroll_preview_to_top();
        assert!(state.preview_scroll.is_at_top());
    }

    #[test]
    fn test_modal_open_close() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());

        // Initially no modal
        assert!(!state.modal_active);
        assert!(state.active_modal.is_none());

        // Open a CreateFile modal
        state.open_modal(FileOperationModal::CreateFile);
        assert!(state.modal_active);
        assert_eq!(state.active_modal, Some(FileOperationModal::CreateFile));
        assert!(state.input_buffer.is_empty());

        // Close modal
        state.close_modal();
        assert!(!state.modal_active);
        assert!(state.active_modal.is_none());
        assert!(state.input_buffer.is_empty());
    }

    #[test]
    fn test_modal_open_create_dir() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());

        state.open_modal(FileOperationModal::CreateDir);
        assert!(state.modal_active);
        assert_eq!(state.active_modal, Some(FileOperationModal::CreateDir));
    }

    #[test]
    fn test_modal_open_delete() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());

        let path = temp_dir.path().join("test.txt");
        state.open_modal(FileOperationModal::Delete {
            path: path.clone(),
            is_dir: false,
        });
        assert!(state.modal_active);
        assert_eq!(
            state.active_modal,
            Some(FileOperationModal::Delete {
                path,
                is_dir: false
            })
        );
        // Delete modal should not pre-fill input buffer
        assert!(state.input_buffer.is_empty());
    }

    #[test]
    fn test_modal_open_rename_prefills_name() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());

        let path = temp_dir.path().join("original.txt");
        state.open_modal(FileOperationModal::Rename {
            path: path.clone(),
            original_name: "original.txt".to_string(),
        });
        assert!(state.modal_active);
        // Rename should pre-fill the input buffer with the original name
        assert_eq!(state.input_buffer, "original.txt");
    }

    #[test]
    fn test_modal_open_error() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());

        state.open_modal(FileOperationModal::Error {
            message: "Something went wrong".to_string(),
        });
        assert!(state.modal_active);
        assert_eq!(
            state.active_modal,
            Some(FileOperationModal::Error {
                message: "Something went wrong".to_string()
            })
        );
    }

    #[test]
    fn test_modal_close_clears_input_buffer() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = PluginState::new(temp_dir.path().to_path_buf());

        state.open_modal(FileOperationModal::CreateFile);
        state.input_buffer.push_str("test.txt");
        assert_eq!(state.input_buffer, "test.txt");

        state.close_modal();
        assert!(state.input_buffer.is_empty());
    }

    #[test]
    fn test_modal_state_default_inactive() {
        let state = PluginState::default();
        assert!(!state.modal_active);
        assert!(state.active_modal.is_none());
        assert!(state.input_buffer.is_empty());
    }
}
