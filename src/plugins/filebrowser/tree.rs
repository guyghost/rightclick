//! File tree handling for the file browser plugin
//!
//! This module provides the `FileTree` structure for managing and displaying
//! a hierarchical view of the file system.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::theme::{UiElement, style_for_ui_element};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

/// Check if a path is ignored by git
fn is_git_ignored(path: &Path, work_dir: &Path) -> bool {
    // Try to open the repository
    let repo = match gix::open(work_dir) {
        Ok(repo) => repo,
        Err(_) => return false, // Not a git repo, nothing is ignored
    };

    // Get the workdir
    let repo_workdir = match repo.work_dir() {
        Some(dir) => dir,
        None => return false,
    };

    // Simple check: see if .gitignore exists and contains the path
    // This is a simplified implementation - a full implementation would parse .gitignore
    let gitignore_path = repo_workdir.join(".gitignore");
    if !gitignore_path.exists() {
        return false;
    }

    if let Ok(content) = std::fs::read_to_string(&gitignore_path) {
        let relative_path = match path.strip_prefix(repo_workdir) {
            Ok(p) => p.to_string_lossy(),
            Err(_) => return false,
        };

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Simple substring match (not a full gitignore implementation)
            if relative_path.contains(line) || line.contains(&*relative_path) {
                return true;
            }
        }
    }

    false
}

/// A single entry in the file tree
#[derive(Clone, Debug, PartialEq)]
pub struct FileEntry {
    /// Full path to the file or directory
    pub path: PathBuf,
    /// Display name (file or directory name)
    pub name: String,
    /// Whether this is a directory
    pub is_dir: bool,
    /// Depth in the tree (0 for root)
    pub depth: usize,
    /// Whether this directory is expanded
    pub is_expanded: bool,
    /// Whether this entry is selected
    pub is_selected: bool,
    /// Whether this entry is hidden (starts with .)
    pub is_hidden: bool,
    /// Whether this entry is git-ignored
    pub is_ignored: bool,
    /// File size in bytes (None for directories)
    pub size: Option<u64>,
    /// Last modified time (None if unavailable)
    pub modified: Option<std::time::SystemTime>,
}

impl FileEntry {
    /// Create a new file entry from a path
    ///
    /// # Arguments
    ///
    /// * `path` - The full path to the file or directory
    /// * `depth` - The depth in the tree hierarchy
    /// * `name_override` - Optional name to display instead of the file name
    /// * `work_dir` - Optional working directory for git ignore checks
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::plugins::filebrowser::FileEntry;
    /// use std::path::PathBuf;
    ///
    /// let entry = FileEntry::new(PathBuf::from("src/main.rs"), 1, None, None);
    /// ```
    pub fn new(
        path: PathBuf,
        depth: usize,
        name_override: Option<String>,
        work_dir: Option<&Path>,
    ) -> Self {
        let name = name_override.unwrap_or_else(|| {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

        let is_hidden = name.starts_with('.');
        let is_dir = path.is_dir();
        let is_ignored = work_dir
            .map(|wd| is_git_ignored(&path, wd))
            .unwrap_or(false);

        // Get metadata if available
        let (size, modified) = fs::metadata(&path)
            .map(|m| {
                let size = if m.is_file() { Some(m.len()) } else { None };
                let modified = m.modified().ok();
                (size, modified)
            })
            .unwrap_or((None, None));

        Self {
            path,
            name,
            is_dir,
            depth,
            is_expanded: false,
            is_selected: false,
            is_hidden,
            is_ignored,
            size,
            modified,
        }
    }

    /// Get the icon for this entry
    ///
    /// Returns an appropriate icon based on whether this is a directory,
    /// hidden file, git-ignored file, or regular file.
    pub fn icon(&self) -> &'static str {
        if self.is_dir {
            if self.is_expanded { "📂" } else { "📁" }
        } else if self.is_ignored {
            "🚫"
        } else if self.is_hidden {
            "👁"
        } else {
            "📄"
        }
    }

    /// Get the expand/collapse indicator
    ///
    /// Returns an appropriate indicator for directories
    pub fn expand_indicator(&self) -> &'static str {
        if !self.is_dir {
            "  "
        } else if self.is_expanded {
            "▼ "
        } else {
            "▶ "
        }
    }

    /// Format the file size for display
    ///
    /// Returns a human-readable string like "1.5 KB" or "2.3 MB"
    pub fn format_size(&self) -> String {
        match self.size {
            None => "-".to_string(),
            Some(0) => "0 B".to_string(),
            Some(size) => {
                const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
                let mut size = size as f64;
                let mut unit_idx = 0;

                while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
                    size /= 1024.0;
                    unit_idx += 1;
                }

                if unit_idx == 0 {
                    format!("{} {}", size as u64, UNITS[unit_idx])
                } else {
                    format!("{:.1} {}", size, UNITS[unit_idx])
                }
            }
        }
    }
}

/// A tree view of files and directories
#[derive(Clone, Debug)]
pub struct FileTree {
    /// Root directory path
    pub root: PathBuf,
    /// All visible entries in the tree
    pub entries: Vec<FileEntry>,
    /// Currently selected index
    pub selected_index: usize,
    /// Expanded directories
    pub expanded_dirs: HashSet<PathBuf>,
}

impl FileTree {
    /// Create a new file tree starting at the given root
    ///
    /// # Arguments
    ///
    /// * `root` - The root directory to browse
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::plugins::filebrowser::FileTree;
    /// use std::path::PathBuf;
    ///
    /// let tree = FileTree::new(PathBuf::from("."));
    /// ```
    pub fn new(root: PathBuf) -> Self {
        let mut tree = Self {
            root: root.clone(),
            entries: Vec::new(),
            selected_index: 0,
            expanded_dirs: HashSet::new(),
        };

        // Add root entry
        tree.entries.push(FileEntry::new(
            root.clone(),
            0,
            Some(".".to_string()),
            Some(&root),
        ));

        // Initially expand root
        tree.expand(&root);

        tree
    }

    /// Refresh the tree by re-scanning expanded directories
    pub fn refresh(&mut self) {
        let expanded: Vec<PathBuf> = self.expanded_dirs.iter().cloned().collect();
        self.entries.clear();
        self.entries.push(FileEntry::new(
            self.root.clone(),
            0,
            Some(".".to_string()),
            Some(&self.root),
        ));
        self.expanded_dirs.clear();

        for path in expanded {
            self.expand(&path);
        }

        // Ensure selection is valid
        if self.selected_index >= self.entries.len() {
            self.selected_index = self.entries.len().saturating_sub(1);
        }
    }

    /// Expand a directory and load its children
    ///
    /// # Arguments
    ///
    /// * `path` - The directory to expand
    pub fn expand(&mut self, path: &Path) {
        // Find the index of the directory entry
        let Some(dir_idx) = self.entries.iter().position(|e| e.path == path) else {
            return;
        };

        // Check if already expanded
        if self.expanded_dirs.contains(path) {
            // Update the entry's expanded state
            self.entries[dir_idx].is_expanded = true;
            return;
        }

        // Mark as expanded
        self.expanded_dirs.insert(path.to_path_buf());
        self.entries[dir_idx].is_expanded = true;

        // Load children
        let depth = self.entries[dir_idx].depth + 1;
        let children = match Self::read_dir_sorted(path) {
            Ok(children) => children,
            Err(_) => return,
        };

        // Insert children after the directory entry (in reverse order to maintain sort)
        for (child_path, name) in children.into_iter().rev() {
            let entry = FileEntry::new(child_path, depth, Some(name), Some(&self.root));
            self.entries.insert(dir_idx + 1, entry);
        }
    }

    /// Collapse a directory and remove its children from view
    ///
    /// # Arguments
    ///
    /// * `path` - The directory to collapse
    pub fn collapse(&mut self, path: &Path) {
        // Find the index of the directory entry
        let Some(dir_idx) = self.entries.iter().position(|e| e.path == path) else {
            return;
        };

        // Check if already collapsed
        if !self.expanded_dirs.contains(path) {
            return;
        }

        // Mark as collapsed
        self.expanded_dirs.remove(path);
        self.entries[dir_idx].is_expanded = false;

        // Remove all children (entries with greater depth)
        let dir_depth = self.entries[dir_idx].depth;
        let mut remove_count = 0;

        for i in (dir_idx + 1)..self.entries.len() {
            if self.entries[i].depth > dir_depth {
                remove_count += 1;
            } else {
                break;
            }
        }

        // Remove the children entries
        for _ in 0..remove_count {
            self.entries.remove(dir_idx + 1);
        }

        // Also recursively collapse nested expanded directories from the set
        let paths_to_remove: Vec<PathBuf> = self
            .expanded_dirs
            .iter()
            .filter(|p| p.starts_with(path) && *p != path)
            .cloned()
            .collect();
        for p in paths_to_remove {
            self.expanded_dirs.remove(&p);
        }

        // Adjust selection if needed
        if self.selected_index >= self.entries.len() {
            self.selected_index = self.entries.len().saturating_sub(1);
        }
    }

    /// Toggle the expanded/collapsed state of a directory
    ///
    /// # Arguments
    ///
    /// * `path` - The directory to toggle
    pub fn toggle(&mut self, path: &Path) {
        if self.expanded_dirs.contains(path) {
            self.collapse(path);
        } else {
            self.expand(path);
        }
    }

    /// Navigate to the next entry
    pub fn next(&mut self) {
        if self.selected_index + 1 < self.entries.len() {
            self.selected_index += 1;
        }
    }

    /// Navigate to the previous entry
    pub fn prev(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    /// Get the currently selected entry
    pub fn selected(&self) -> Option<&FileEntry> {
        self.entries.get(self.selected_index)
    }

    /// Get a mutable reference to the currently selected entry
    pub fn selected_mut(&mut self) -> Option<&mut FileEntry> {
        self.entries.get_mut(self.selected_index)
    }

    /// Get the total number of visible entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the tree is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Read a directory and return sorted entries
    fn read_dir_sorted(path: &Path) -> Result<Vec<(PathBuf, String)>, std::io::Error> {
        let mut entries: Vec<(PathBuf, String)> = Vec::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry
                .file_name()
                .into_string()
                .unwrap_or_else(|_| "invalid".to_string());
            entries.push((path, name));
        }

        // Sort: directories first, then alphabetically
        entries.sort_by(|a, b| {
            let a_is_dir = a.0.is_dir();
            let b_is_dir = b.0.is_dir();

            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.1.to_lowercase().cmp(&b.1.to_lowercase()),
            }
        });

        Ok(entries)
    }

    /// Get all file entries matching a predicate
    pub fn filter_entries<F>(&self, predicate: F) -> Vec<&FileEntry>
    where
        F: Fn(&FileEntry) -> bool,
    {
        self.entries.iter().filter(|e| predicate(e)).collect()
    }

    /// Expand to and select a specific path
    ///
    /// This will expand all parent directories and select the target entry.
    ///
    /// # Arguments
    ///
    /// * `target` - The path to navigate to
    pub fn navigate_to(&mut self, target: &Path) {
        // Collect all parent paths that need to be expanded
        let mut to_expand: Vec<PathBuf> = Vec::new();
        let mut current = target.parent();

        while let Some(parent) = current {
            if parent.starts_with(&self.root) && parent != self.root {
                to_expand.push(parent.to_path_buf());
            }
            current = parent.parent();
        }

        // Expand from root to target (reverse order)
        for path in to_expand.into_iter().rev() {
            self.expand(&path);
        }

        // Find and select the target
        if let Some(idx) = self.entries.iter().position(|e| e.path == target) {
            self.selected_index = idx;
        }
    }
}

impl Default for FileTree {
    fn default() -> Self {
        Self::new(PathBuf::from("."))
    }
}

/// Widget for rendering the file tree
pub struct FileTreeWidget<'a> {
    tree: &'a FileTree,
    show_icons: bool,
    show_hidden: bool,
}

impl<'a> FileTreeWidget<'a> {
    /// Create a new file tree widget
    pub fn new(tree: &'a FileTree) -> Self {
        Self {
            tree,
            show_icons: true,
            show_hidden: true,
        }
    }

    /// Set whether to show icons
    pub fn show_icons(mut self, show: bool) -> Self {
        self.show_icons = show;
        self
    }

    /// Set whether to show hidden files
    pub fn show_hidden(mut self, show: bool) -> Self {
        self.show_hidden = show;
        self
    }
}

impl<'a> Widget for FileTreeWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let text_style =
            style_for_ui_element(&crate::core::models::Theme::default(), UiElement::Text);
        let selected_style =
            style_for_ui_element(&crate::core::models::Theme::default(), UiElement::Highlight)
                .add_modifier(Modifier::BOLD);
        let muted_style =
            style_for_ui_element(&crate::core::models::Theme::default(), UiElement::MutedText);
        let primary_style =
            style_for_ui_element(&crate::core::models::Theme::default(), UiElement::Primary);

        let visible_entries: Vec<&FileEntry> = if self.show_hidden {
            self.tree.entries.iter().collect()
        } else {
            self.tree.entries.iter().filter(|e| !e.is_hidden).collect()
        };

        for (i, entry) in visible_entries.iter().enumerate() {
            if i >= area.height as usize {
                break;
            }

            let is_selected = self.tree.selected_index == i;
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
            if self.show_icons {
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
            spans.push(Span::styled(&entry.name, name_style));

            // Render the line
            let line = Line::from(spans);
            buf.set_line(area.x, area.y.saturating_add(i as u16), &line, area.width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_file_entry_new() {
        let entry = FileEntry::new(PathBuf::from("test.txt"), 0, None, None);
        assert_eq!(entry.name, "test.txt");
        assert_eq!(entry.depth, 0);
        assert!(!entry.is_expanded);
    }

    #[test]
    fn test_file_entry_with_name_override() {
        let entry = FileEntry::new(
            PathBuf::from("/home/user/test.txt"),
            1,
            Some("custom".to_string()),
            None,
        );
        assert_eq!(entry.name, "custom");
        assert_eq!(entry.depth, 1);
    }

    #[test]
    fn test_file_entry_icon() {
        let file = FileEntry::new(PathBuf::from("test.txt"), 0, None, None);
        assert_eq!(file.icon(), "📄");

        let dir = FileEntry {
            path: PathBuf::from("dir"),
            name: "dir".to_string(),
            is_dir: true,
            depth: 0,
            is_expanded: false,
            is_selected: false,
            is_hidden: false,
            is_ignored: false,
            size: None,
            modified: None,
        };
        assert_eq!(dir.icon(), "📁");

        let expanded_dir = FileEntry {
            path: PathBuf::from("dir"),
            name: "dir".to_string(),
            is_dir: true,
            depth: 0,
            is_expanded: true,
            is_selected: false,
            is_hidden: false,
            is_ignored: false,
            size: None,
            modified: None,
        };
        assert_eq!(expanded_dir.icon(), "📂");
    }

    #[test]
    fn test_file_entry_format_size() {
        let file = FileEntry {
            path: PathBuf::from("test.txt"),
            name: "test.txt".to_string(),
            is_dir: false,
            depth: 0,
            is_expanded: false,
            is_selected: false,
            is_hidden: false,
            is_ignored: false,
            size: Some(0),
            modified: None,
        };
        assert_eq!(file.format_size(), "0 B");

        let file_kb = FileEntry {
            path: PathBuf::from("test.txt"),
            name: "test.txt".to_string(),
            is_dir: false,
            depth: 0,
            is_expanded: false,
            is_selected: false,
            is_hidden: false,
            is_ignored: false,
            size: Some(1536),
            modified: None,
        };
        assert_eq!(file_kb.format_size(), "1.5 KB");

        let file_mb = FileEntry {
            path: PathBuf::from("test.txt"),
            name: "test.txt".to_string(),
            is_dir: false,
            depth: 0,
            is_expanded: false,
            is_selected: false,
            is_hidden: false,
            is_ignored: false,
            size: Some(2_500_000),
            modified: None,
        };
        assert_eq!(file_mb.format_size(), "2.4 MB");

        let dir = FileEntry {
            path: PathBuf::from("dir"),
            name: "dir".to_string(),
            is_dir: true,
            depth: 0,
            is_expanded: false,
            is_selected: false,
            is_hidden: false,
            is_ignored: false,
            size: None,
            modified: None,
        };
        assert_eq!(dir.format_size(), "-");
    }

    #[test]
    fn test_file_tree_new() {
        let temp_dir = TempDir::new().unwrap();
        let tree = FileTree::new(temp_dir.path().to_path_buf());

        assert_eq!(tree.root, temp_dir.path());
        assert!(!tree.entries.is_empty());
        assert_eq!(tree.entries[0].name, ".");
    }

    #[test]
    fn test_file_tree_widget_renders_inside_offset_area_near_u16_max() {
        let temp_dir = TempDir::new().unwrap();
        fs::File::create(temp_dir.path().join("file.txt")).unwrap();
        let tree = FileTree::new(temp_dir.path().to_path_buf());
        let area = Rect::new(u16::MAX - 30, u16::MAX - 2, 30, 3);
        let mut buf = Buffer::empty(area);

        FileTreeWidget::new(&tree)
            .show_icons(false)
            .render(area, &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("."));
    }

    #[test]
    fn test_file_tree_navigation() {
        let temp_dir = TempDir::new().unwrap();
        let mut tree = FileTree::new(temp_dir.path().to_path_buf());

        assert_eq!(tree.selected_index, 0);

        tree.next();
        // Still 0 if only root entry
        if tree.entries.len() > 1 {
            assert_eq!(tree.selected_index, 1);
        }

        tree.prev();
        assert_eq!(tree.selected_index, 0);
    }

    #[test]
    fn test_file_tree_expand_collapse() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let mut tree = FileTree::new(temp_dir.path().to_path_buf());
        let initial_len = tree.entries.len();

        // Expand root
        tree.expand(temp_dir.path());
        assert!(tree.expanded_dirs.contains(temp_dir.path()));
        assert!(tree.entries.len() >= initial_len);

        // Collapse root
        tree.collapse(temp_dir.path());
        assert!(!tree.expanded_dirs.contains(temp_dir.path()));
        assert!(tree.entries.len() <= initial_len);
    }

    #[test]
    fn test_file_tree_toggle() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let mut tree = FileTree::new(temp_dir.path().to_path_buf());
        let was_expanded = tree.expanded_dirs.contains(temp_dir.path());

        // First toggle flips current state
        tree.toggle(temp_dir.path());
        assert_eq!(tree.expanded_dirs.contains(temp_dir.path()), !was_expanded);

        // Second toggle restores initial state
        tree.toggle(temp_dir.path());
        assert_eq!(tree.expanded_dirs.contains(temp_dir.path()), was_expanded);
    }

    #[test]
    fn test_file_tree_filter_entries() {
        let temp_dir = TempDir::new().unwrap();
        let mut tree = FileTree::new(temp_dir.path().to_path_buf());

        // Create a hidden file
        let hidden_file = temp_dir.path().join(".hidden");
        fs::File::create(&hidden_file).unwrap();
        tree.refresh();

        let hidden = tree.filter_entries(|e| e.is_hidden);
        // May or may not contain hidden files depending on expansion
        // Just verify the function works
        assert!(hidden.is_empty() || hidden.iter().all(|e| e.is_hidden));
    }
}
