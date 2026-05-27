//! File preview with syntax highlighting
//!
//! This module provides file preview functionality including syntax highlighting
//! via syntect and detection of binary/image files.

use std::fs;
use std::path::Path;

use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::core::models::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use crate::ui::{clip_display, count_label};

/// Maximum file size to preview (5 MB)
const MAX_PREVIEW_SIZE: u64 = 5 * 1024 * 1024;

/// Maximum number of lines to preview
const MAX_PREVIEW_LINES: usize = 10000;

/// A file preview with syntax highlighting
#[derive(Clone, Debug, PartialEq)]
pub struct Preview {
    /// File content (truncated if too large)
    pub content: String,
    /// Language identifier for syntax highlighting
    pub language: Option<String>,
    /// Whether the file is binary
    pub is_binary: bool,
    /// Whether the file is an image
    pub is_image: bool,
    /// File size in bytes
    pub file_size: u64,
    /// Total number of lines in the file
    pub total_lines: usize,
    /// Whether the content was truncated
    pub is_truncated: bool,
    /// File path
    pub path: std::path::PathBuf,
}

impl Preview {
    /// Create a new preview from a file path
    ///
    /// This will detect file type, check if binary/image, and load content
    /// with appropriate limits.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file to preview
    ///
    /// # Example
    ///
    /// ```rust
    /// use rightclick::plugins::filebrowser::Preview;
    ///
    /// let preview = Preview::from_file("src/main.rs");
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> Option<Self> {
        let path = path.as_ref();

        // Get file metadata
        let metadata = fs::metadata(path).ok()?;
        let file_size = metadata.len();

        // Check if it's a directory
        if metadata.is_dir() {
            return None;
        }

        // Detect file type
        let is_binary = Self::is_binary_file(path);
        let is_image = Self::is_image_file(path);

        // If it's a large file, don't load content
        if file_size > MAX_PREVIEW_SIZE {
            return Some(Self {
                content: String::new(),
                language: None,
                is_binary,
                is_image,
                file_size,
                total_lines: 0,
                is_truncated: true,
                path: path.to_path_buf(),
            });
        }

        // Load content
        let content = if is_binary || is_image {
            String::new()
        } else {
            fs::read_to_string(path).unwrap_or_default()
        };

        let total_lines = content.lines().count();

        // Truncate if too many lines
        let (content, is_truncated) = if total_lines > MAX_PREVIEW_LINES {
            let truncated: String = content
                .lines()
                .take(MAX_PREVIEW_LINES)
                .collect::<Vec<_>>()
                .join("\n");
            (truncated, true)
        } else {
            (content, false)
        };

        // Detect language
        let language = Self::detect_language(path);

        Some(Self {
            content,
            language,
            is_binary,
            is_image,
            file_size,
            total_lines,
            is_truncated,
            path: path.to_path_buf(),
        })
    }

    /// Create a preview for a directory
    ///
    /// Shows a summary of the directory contents.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the directory
    pub fn from_directory<P: AsRef<Path>>(path: P) -> Option<Self> {
        let path = path.as_ref();

        if !path.is_dir() {
            return None;
        }

        let mut entries: Vec<String> = Vec::new();
        let mut file_count = 0;
        let mut dir_count = 0;
        let mut total_size = 0u64;

        if let Ok(dir_entries) = fs::read_dir(path) {
            for entry in dir_entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if metadata.is_dir() {
                        dir_count += 1;
                        entries.push(format!("📁 {}/", name));
                    } else {
                        file_count += 1;
                        total_size += metadata.len();
                        let size = Self::format_size(metadata.len());
                        entries.push(format!("📄 {} ({})", name, size));
                    }
                }
            }
        }

        // Sort entries
        entries.sort();

        // Build content
        let mut content = format!("Directory: {}\n", path.display());
        content.push_str(&format!(
            "{} | {} | Total size: {}\n",
            count_label(file_count, "file", "files"),
            count_label(dir_count, "directory", "directories"),
            Self::format_size(total_size)
        ));
        content.push_str("─".repeat(50).as_str());
        content.push('\n');
        content.push_str(&entries.join("\n"));

        Some(Self {
            content,
            language: None,
            is_binary: false,
            is_image: false,
            file_size: total_size,
            total_lines: entries.len() + 3,
            is_truncated: false,
            path: path.to_path_buf(),
        })
    }

    /// Check if a file is binary
    fn is_binary_file(path: &Path) -> bool {
        // Check extension first
        if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            let binary_extensions = [
                "exe", "dll", "so", "dylib", "bin", "o", "a", "lib", "zip", "tar", "gz", "bz2",
                "xz", "7z", "rar", "jpg", "jpeg", "png", "gif", "bmp", "webp", "svg", "mp3", "mp4",
                "avi", "mov", "mkv", "wav", "flac", "pdf", "doc", "docx", "xls", "xlsx", "ppt",
                "pptx", "db", "sqlite", "sqlite3", "class", "jar", "pyc",
            ];
            if binary_extensions.contains(&ext.as_str()) {
                return true;
            }
        }

        // Try to detect by content
        if let Ok(bytes) = fs::read(path) {
            // Check for null bytes in first 8KB
            let check_len = bytes.len().min(8192);
            for byte in &bytes[..check_len] {
                if *byte == 0 {
                    return true;
                }
            }
        }

        false
    }

    /// Check if a file is an image
    fn is_image_file(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            matches!(
                ext.as_str(),
                "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tiff" | "tif"
            )
        } else {
            false
        }
    }

    /// Detect the programming language from the file path
    fn detect_language(path: &Path) -> Option<String> {
        // Get syntax set for extension detection
        let syntax_set = SyntaxSet::load_defaults_newlines();

        syntax_set
            .find_syntax_for_file(path)
            .ok()?
            .map(|syntax| syntax.name.clone())
    }

    /// Format a size in bytes to a human-readable string
    fn format_size(size: u64) -> String {
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

    /// Get a description of the file type
    pub fn file_type_description(&self) -> String {
        if self.is_image {
            "Image".to_string()
        } else if self.is_binary {
            "Binary".to_string()
        } else if let Some(ref lang) = self.language {
            lang.to_string()
        } else {
            "Text".to_string()
        }
    }
}

/// Widget for rendering file preview with syntax highlighting
pub struct PreviewWidget<'a> {
    preview: &'a Preview,
    scroll_offset: usize,
    #[allow(dead_code)]
    theme: &'a Theme,
}

impl<'a> PreviewWidget<'a> {
    /// Create a new preview widget
    pub fn new(preview: &'a Preview, scroll_offset: usize, theme: &'a Theme) -> Self {
        Self {
            preview,
            scroll_offset,
            theme,
        }
    }

    /// Get highlighted lines using syntect
    fn highlight_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        // Handle special file types
        if self.preview.is_binary {
            lines.push(Line::from("[Binary file]"));
            lines.push(Line::from(format!(
                "Size: {}",
                Self::format_size_static(self.preview.file_size)
            )));
            return lines;
        }

        if self.preview.is_image {
            lines.push(Line::from("[Image file]"));
            lines.push(Line::from(format!(
                "Size: {}",
                Self::format_size_static(self.preview.file_size)
            )));
            lines.push(Line::from(format!("Path: {}", self.preview.path.display())));
            return lines;
        }

        if self.preview.is_truncated && self.preview.content.is_empty() {
            lines.push(Line::from("[File too large to preview]"));
            lines.push(Line::from(format!(
                "Size: {}",
                Self::format_size_static(self.preview.file_size)
            )));
            return lines;
        }

        // Set up syntax highlighting
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();

        // Find the syntax
        let syntax = self
            .preview
            .language
            .as_ref()
            .and_then(|lang| syntax_set.find_syntax_by_name(lang))
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

        // Use a default theme
        let syntect_theme = &theme_set.themes["base16-ocean.dark"];

        let mut highlighter = HighlightLines::new(syntax, syntect_theme);

        // Process each line
        for (line_num, line) in self
            .preview
            .content
            .lines()
            .enumerate()
            .skip(self.scroll_offset)
        {
            let highlighted = highlighter
                .highlight_line(line, &syntax_set)
                .unwrap_or_default();

            let mut spans: Vec<Span> = Vec::new();

            // Add line number
            let line_num_str = format!("{:4} │ ", line_num + 1);
            spans.push(Span::styled(
                line_num_str,
                Style::default().fg(ratatui::style::Color::DarkGray),
            ));

            // Add highlighted content
            for (style, text) in highlighted {
                let color = style.foreground;
                let ratatui_color = ratatui::style::Color::Rgb(color.r, color.g, color.b);
                spans.push(Span::styled(
                    text.to_string(),
                    Style::default().fg(ratatui_color),
                ));
            }

            lines.push(Line::from(spans));
        }

        // Add truncation notice
        if self.preview.is_truncated {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    "[Content truncated - file exceeds ",
                    Style::default().fg(ratatui::style::Color::Yellow),
                ),
                Span::styled(
                    format!("{} lines", MAX_PREVIEW_LINES),
                    Style::default()
                        .fg(ratatui::style::Color::Yellow)
                        .add_modifier(ratatui::style::Modifier::BOLD),
                ),
                Span::styled("]", Style::default().fg(ratatui::style::Color::Yellow)),
            ]));
        }

        lines
    }

    fn format_size_static(size: u64) -> String {
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

impl Widget for PreviewWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create a block with title
        let title = format!(
            " {} ({}) ",
            self.preview
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Preview"),
            self.preview.file_type_description()
        );

        let block = Block::default().title(title).borders(Borders::ALL);

        let inner = block.inner(area);
        block.render(area, buf);

        // Get highlighted lines
        let lines = self.highlight_lines();

        // Render lines within the available area
        for (i, line) in lines.iter().enumerate().take(inner.height as usize) {
            buf.set_line(inner.x, inner.y.saturating_add(i as u16), line, inner.width);
        }
    }
}

#[allow(dead_code)]
/// A simple preview widget without syntax highlighting (faster)
pub struct SimplePreviewWidget<'a> {
    preview: &'a Preview,
    scroll_offset: usize,
}

#[allow(dead_code)]
impl<'a> SimplePreviewWidget<'a> {
    /// Create a new simple preview widget
    pub fn new(preview: &'a Preview, scroll_offset: usize) -> Self {
        Self {
            preview,
            scroll_offset,
        }
    }
}

impl Widget for SimplePreviewWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(format!(
                " {} ",
                self.preview
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Preview")
            ))
            .borders(Borders::ALL);

        let inner = block.inner(area);
        block.render(area, buf);

        // Render content lines
        let lines: Vec<&str> = self.preview.content.lines().collect();

        for i in 0..inner.height as usize {
            let line_idx = self.scroll_offset.saturating_add(i);
            if line_idx < lines.len() {
                let line = format!("{:4} │ {}", line_idx + 1, lines[line_idx]);
                let truncated = clip_display(&line, inner.width as usize);
                buf.set_string(
                    inner.x,
                    inner.y.saturating_add(i as u16),
                    truncated,
                    Style::default(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_preview_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "Hello, World!").unwrap();

        let preview = Preview::from_file(&file_path).unwrap();
        assert!(!preview.is_binary);
        assert!(!preview.is_image);
        assert_eq!(preview.content, "Hello, World!\n");
        assert!(!preview.is_truncated);
    }

    #[test]
    fn test_preview_from_directory() {
        let temp_dir = TempDir::new().unwrap();

        // Create some files
        fs::File::create(temp_dir.path().join("file1.txt")).unwrap();
        fs::File::create(temp_dir.path().join("file2.txt")).unwrap();
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();

        let preview = Preview::from_directory(temp_dir.path()).unwrap();
        assert!(preview.content.contains("Directory:"));
        assert!(preview.content.contains("2 files"));
        assert!(preview.content.contains("1 directory"));
        assert!(!preview.content.contains("1 directories"));
    }

    #[test]
    fn test_preview_binary_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create a file with null bytes
        let binary_path = temp_dir.path().join("binary.bin");
        let mut file = fs::File::create(&binary_path).unwrap();
        file.write_all(&[0x00, 0x01, 0x02, 0x03]).unwrap();

        let preview = Preview::from_file(&binary_path).unwrap();
        assert!(preview.is_binary);
        assert!(preview.content.is_empty());
    }

    #[test]
    fn test_preview_image_detection() {
        let temp_dir = TempDir::new().unwrap();

        let image_path = temp_dir.path().join("image.png");
        fs::File::create(&image_path).unwrap();

        let preview = Preview::from_file(&image_path).unwrap();
        assert!(preview.is_image);
    }

    #[test]
    fn test_preview_format_size() {
        assert_eq!(PreviewWidget::format_size_static(0), "0 B");
        assert_eq!(PreviewWidget::format_size_static(1024), "1.0 KB");
        assert_eq!(PreviewWidget::format_size_static(1536), "1.5 KB");
        assert_eq!(PreviewWidget::format_size_static(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn test_preview_file_type_description() {
        let image_preview = Preview {
            content: String::new(),
            language: None,
            is_binary: false,
            is_image: true,
            file_size: 0,
            total_lines: 0,
            is_truncated: false,
            path: PathBuf::from("test.png"),
        };
        assert_eq!(image_preview.file_type_description(), "Image");

        let binary_preview = Preview {
            content: String::new(),
            language: None,
            is_binary: true,
            is_image: false,
            file_size: 0,
            total_lines: 0,
            is_truncated: false,
            path: PathBuf::from("test.bin"),
        };
        assert_eq!(binary_preview.file_type_description(), "Binary");

        let rust_preview = Preview {
            content: String::new(),
            language: Some("Rust".to_string()),
            is_binary: false,
            is_image: false,
            file_size: 0,
            total_lines: 0,
            is_truncated: false,
            path: PathBuf::from("test.rs"),
        };
        assert_eq!(rust_preview.file_type_description(), "Rust");
    }

    #[test]
    fn test_preview_large_file_truncation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.txt");
        let mut file = fs::File::create(&file_path).unwrap();

        // Write many lines
        for i in 0..MAX_PREVIEW_LINES + 100 {
            writeln!(file, "Line {}", i).unwrap();
        }

        let preview = Preview::from_file(&file_path).unwrap();
        assert!(preview.is_truncated);
        assert!(
            preview
                .content
                .contains(&format!("Line {}", MAX_PREVIEW_LINES - 1))
        );
        assert!(
            !preview
                .content
                .contains(&format!("Line {}", MAX_PREVIEW_LINES))
        );
    }

    #[test]
    fn test_simple_preview_truncates_unicode_lines_safely() {
        let preview = Preview {
            content: "éclair session\n".to_string(),
            language: None,
            is_binary: false,
            is_image: false,
            file_size: 0,
            total_lines: 1,
            is_truncated: false,
            path: PathBuf::from("unicode.txt"),
        };
        let area = Rect::new(0, 0, 12, 3);
        let mut buf = Buffer::empty(area);

        SimplePreviewWidget::new(&preview, 0).render(area, &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("é"));
    }

    #[test]
    fn test_simple_preview_tolerates_extreme_scroll_offset() {
        let preview = Preview {
            content: "first\nsecond\n".to_string(),
            language: None,
            is_binary: false,
            is_image: false,
            file_size: 0,
            total_lines: 2,
            is_truncated: false,
            path: PathBuf::from("scroll.txt"),
        };
        let area = Rect::new(0, 0, 20, 4);
        let mut buf = Buffer::empty(area);

        SimplePreviewWidget::new(&preview, usize::MAX).render(area, &mut buf);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("scroll.txt"));
        assert!(!content.contains("first"));
    }

    #[test]
    fn test_preview_widgets_render_inside_offset_area_near_u16_max() {
        let preview = Preview {
            content: "first\nsecond\nthird\n".to_string(),
            language: None,
            is_binary: false,
            is_image: false,
            file_size: 19,
            total_lines: 3,
            is_truncated: false,
            path: PathBuf::from("offset.txt"),
        };
        let theme = Theme::default();
        let area = Rect::new(u16::MAX - 40, u16::MAX - 3, 40, 4);

        let mut highlighted_buf = Buffer::empty(area);
        PreviewWidget::new(&preview, 0, &theme).render(area, &mut highlighted_buf);
        let highlighted_content: String = highlighted_buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(highlighted_content.contains("offset.txt"));
        assert!(highlighted_content.contains("first"));

        let mut simple_buf = Buffer::empty(area);
        SimplePreviewWidget::new(&preview, 0).render(area, &mut simple_buf);
        let simple_content: String = simple_buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(simple_content.contains("offset.txt"));
        assert!(simple_content.contains("first"));
    }
}
