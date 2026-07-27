//! Project logic - pure functions for project validation and construction.
//!
//! These functions have no side effects (filesystem existence checks are
//! referentially transparent queries with no observable mutation) and are
//! fully deterministic, in accordance with the Functional Core pattern.

use std::path::{Path, PathBuf};

use crate::core::models::config::ProjectConfig;

/// Errors that can occur when validating a project path.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProjectPathError {
    /// The path does not exist on the filesystem.
    #[error("path does not exist: {0}")]
    NotFound(PathBuf),
    /// The path exists but is not a directory.
    #[error("path is not a directory: {0}")]
    NotADirectory(PathBuf),
}

/// Validate that the given path is suitable as a project root.
///
/// Returns `Ok(())` only if the path exists and is a directory.
///
/// # Examples
///
/// ```
/// use rightclick::core::logic::project::validate_project_path;
/// use std::path::Path;
///
/// // A non-existent path is rejected.
/// let err = validate_project_path(Path::new("/definitely/not/here/xyz")).unwrap_err();
/// assert!(matches!(
///     err,
///     rightclick::core::logic::project::ProjectPathError::NotFound(_)
/// ));
/// ```
pub fn validate_project_path(path: &Path) -> Result<(), ProjectPathError> {
    if !path.exists() {
        return Err(ProjectPathError::NotFound(path.to_path_buf()));
    }
    if !path.is_dir() {
        return Err(ProjectPathError::NotADirectory(path.to_path_buf()));
    }
    Ok(())
}

/// Extract a human-readable project name from the final path component.
///
/// Returns `None` if the path has no final component (e.g. `/`).
///
/// # Examples
///
/// ```
/// use rightclick::core::logic::project::extract_project_name;
/// use std::path::Path;
///
/// assert_eq!(extract_project_name(Path::new("/home/user/my-app")), Some("my-app"));
/// ```
pub fn extract_project_name(path: &Path) -> Option<&str> {
    path.file_name().and_then(|n| n.to_str())
}

/// Find an existing project whose path matches the given one.
///
/// Pure lookup over a slice of [`ProjectConfig`]. Canonical comparison is
/// delegated to `Path` equality semantics (no normalization is performed).
///
/// # Examples
///
/// ```
/// use rightclick::core::logic::project::find_existing_project;
/// use rightclick::core::models::config::ProjectConfig;
/// use std::path::Path;
///
/// let projects = vec![
///     ProjectConfig {
///         id: "p1".to_string(),
///         name: "alpha".to_string(),
///         path: "/tmp/alpha".to_string(),
///         description: None,
///         favorite: false,
///         tags: vec![],
///     },
/// ];
/// let found = find_existing_project(&projects, Path::new("/tmp/alpha"));
/// assert!(found.is_some());
/// assert_eq!(found.unwrap().id, "p1");
/// ```
pub fn find_existing_project<'a>(
    projects: &'a [ProjectConfig],
    path: &Path,
) -> Option<&'a ProjectConfig> {
    projects.iter().find(|p| Path::new(&p.path) == path)
}

/// Build a fresh [`ProjectConfig`] from a generated id and a filesystem path.
///
/// The project name is derived from the final path component, falling back to
/// `"unnamed"` when no usable component is available.
///
/// # Examples
///
/// ```
/// use rightclick::core::logic::project::build_project_config;
/// use std::path::Path;
///
/// let cfg = build_project_config("id-123".to_string(), Path::new("/tmp/widget"));
/// assert_eq!(cfg.id, "id-123");
/// assert_eq!(cfg.name, "widget");
/// assert_eq!(cfg.path, "/tmp/widget");
/// assert!(!cfg.favorite);
/// ```
pub fn build_project_config(id: String, path: &Path) -> ProjectConfig {
    let name = extract_project_name(path).unwrap_or("unnamed").to_string();
    ProjectConfig {
        id,
        name,
        path: path.to_string_lossy().to_string(),
        description: None,
        favorite: false,
        tags: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_config(id: &str, path: &str) -> ProjectConfig {
        ProjectConfig {
            id: id.to_string(),
            name: format!("name-{}", id),
            path: path.to_string(),
            description: None,
            favorite: false,
            tags: Vec::new(),
        }
    }

    #[test]
    fn validate_rejects_missing_path() {
        let err = validate_project_path(Path::new("/this/should/not/exist/abczyx")).unwrap_err();
        assert!(matches!(err, ProjectPathError::NotFound(_)));
    }

    #[test]
    fn validate_rejects_file_for_directory() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("file.txt");
        std::fs::write(&file_path, "x").unwrap();
        let err = validate_project_path(&file_path).unwrap_err();
        assert!(matches!(err, ProjectPathError::NotADirectory(_)));
    }

    #[test]
    fn validate_accepts_existing_directory() {
        let tmp = TempDir::new().unwrap();
        validate_project_path(tmp.path()).expect("temp dir should be valid");
    }

    #[test]
    fn extract_name_from_path() {
        assert_eq!(
            extract_project_name(Path::new("/home/user/widget")),
            Some("widget")
        );
        assert_eq!(
            extract_project_name(Path::new("relative")),
            Some("relative")
        );
    }

    #[test]
    fn extract_name_handles_root() {
        // On POSIX root has no file_name
        assert_eq!(extract_project_name(Path::new("/")), None);
    }

    #[test]
    fn find_existing_returns_match() {
        let projects = vec![
            sample_config("p1", "/tmp/alpha"),
            sample_config("p2", "/tmp/beta"),
        ];
        let found = find_existing_project(&projects, Path::new("/tmp/beta"));
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "p2");
    }

    #[test]
    fn find_existing_returns_none_when_absent() {
        let projects = vec![sample_config("p1", "/tmp/alpha")];
        assert!(find_existing_project(&projects, Path::new("/tmp/missing")).is_none());
    }

    #[test]
    fn build_config_extracts_name() {
        let cfg = build_project_config("id-1".to_string(), Path::new("/var/projects/widget"));
        assert_eq!(cfg.id, "id-1");
        assert_eq!(cfg.name, "widget");
        assert_eq!(cfg.path, "/var/projects/widget");
        assert!(cfg.description.is_none());
        assert!(!cfg.favorite);
        assert!(cfg.tags.is_empty());
    }

    #[test]
    fn build_config_falls_back_to_unnamed_for_root() {
        let cfg = build_project_config("id-2".to_string(), Path::new("/"));
        assert_eq!(cfg.name, "unnamed");
    }

    #[test]
    fn build_config_is_deterministic() {
        let a = build_project_config("id".to_string(), Path::new("/tmp/x"));
        let b = build_project_config("id".to_string(), Path::new("/tmp/x"));
        assert_eq!(a, b);
    }
}
