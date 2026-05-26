//! Use case orchestration for RightClick.
//!
//! This module contains use case structs that orchestrate application workflows.
//! Use cases are the primary entry point for application logic from the UI layer.
//! They coordinate between:
//!
//! - The **functional core** (pure business logic)
//! - **Repositories** (data persistence)
//! - **Services** (external integrations like Git)
//!
//! # Design Principles
//!
//! 1. **Orchestration only** - Use cases don't contain business logic, they call the core
//! 2. **Dependency injection** - All dependencies are injected via constructor
//! 3. **Async by default** - All operations are async since they involve I/O
//! 4. **Error handling** - Use `anyhow::Result` for flexible error propagation

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, instrument};

use crate::core::models::config::ProjectConfig;
use crate::shell::repositories::{ConfigRepository, StateRepository};
use crate::shell::services::GitService;

/// Runtime project information.
///
/// This struct combines the static [`ProjectConfig`] with runtime information
/// like Git status. It represents a fully loaded project ready for display
/// and interaction.
///
/// # Fields
///
/// * `config` - The static project configuration
/// * `git_status` - Optional Git status if the project is a git repository
#[derive(Debug, Clone)]
pub struct Project {
    /// Static project configuration
    pub config: ProjectConfig,
    /// Whether this project is currently active/selected
    pub is_active: bool,
}

impl Project {
    /// Create a new Project from a ProjectConfig.
    pub fn new(config: ProjectConfig) -> Self {
        Self {
            config,
            is_active: false,
        }
    }

    /// Get the project ID.
    pub fn id(&self) -> &str {
        &self.config.id
    }

    /// Get the project name.
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Get the project path.
    pub fn path(&self) -> &str {
        &self.config.path
    }

    /// Check if this project is a favorite.
    pub fn is_favorite(&self) -> bool {
        self.config.favorite
    }
}

/// Main application use case.
///
/// This struct orchestrates the primary workflows of the application:
/// - Loading and switching projects
/// - Managing configuration
/// - Coordinating with Git services
/// - Refreshing data
///
/// # Dependencies
///
/// - `config_repo` - Repository for persisting configuration
/// - `state_repo` - Repository for persisting UI state
/// - `git_service` - Service for Git operations
///
/// # Example
///
/// ```rust,ignore
/// use rightclick::shell::usecases::AppUsecase;
/// use rightclick::shell::repositories::{FileConfigRepository, FileStateRepository};
/// use rightclick::shell::services::GixGitService;
/// use std::sync::Arc;
///
/// let usecase = AppUsecase::new(
///     Arc::new(FileConfigRepository::new("~/.config/rightclick/config.json")),
///     Arc::new(FileStateRepository::new("~/.config/rightclick/state.json")),
///     Arc::new(CliGitService::new()),
/// );
///
/// let project = usecase.load_project(Path::new("/path/to/project")).await?;
/// ```
#[derive(Clone)]
pub struct AppUsecase {
    config_repo: Arc<dyn ConfigRepository>,
    state_repo: Arc<dyn StateRepository>,
    git_service: Arc<dyn GitService>,
}

impl AppUsecase {
    /// Create a new AppUsecase with the given dependencies.
    ///
    /// # Arguments
    ///
    /// * `config_repo` - Repository for configuration persistence
    /// * `state_repo` - Repository for state persistence
    /// * `git_service` - Service for Git operations
    pub fn new(
        config_repo: Arc<dyn ConfigRepository>,
        state_repo: Arc<dyn StateRepository>,
        git_service: Arc<dyn GitService>,
    ) -> Self {
        Self {
            config_repo,
            state_repo,
            git_service,
        }
    }

    /// Load a project from the given path.
    ///
    /// This method:
    /// 1. Validates that the path exists and is a directory
    /// 2. Checks if the path is already a known project
    /// 3. If not, creates a new project configuration
    /// 4. Optionally loads Git status if it's a git repository
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the project directory
    ///
    /// # Returns
    ///
    /// Returns a [`Project`] containing the loaded project information.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path doesn't exist or isn't a directory
    /// - The configuration cannot be loaded
    #[instrument(skip(self), fields(path = %path.display()))]
    pub async fn load_project(&self, path: &Path) -> Result<Project> {
        info!("Loading project");

        // Validate path
        if !path.exists() {
            anyhow::bail!("Path does not exist: {}", path.display());
        }
        if !path.is_dir() {
            anyhow::bail!("Path is not a directory: {}", path.display());
        }

        // Load config to check if project already exists
        let config = self
            .config_repo
            .load()
            .await
            .context("Failed to load configuration")?;

        // Check if project already exists by path
        let existing_project = config.projects.list.iter().find(|p| {
            let project_path = std::path::Path::new(&p.path);
            project_path == path
        });

        let project_config = if let Some(existing) = existing_project {
            debug!("Found existing project: {}", existing.id);
            existing.clone()
        } else {
            // Create new project config
            let project_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unnamed")
                .to_string();

            let project_id = format!("{}", uuid::Uuid::new_v4());

            ProjectConfig {
                id: project_id,
                name: project_name,
                path: path.to_string_lossy().to_string(),
                description: None,
                favorite: false,
                tags: Vec::new(),
            }
        };

        let project = Project::new(project_config);

        info!("Project loaded successfully: {}", project.id());
        Ok(project)
    }

    /// Switch to the project with the given name.
    ///
    /// This method:
    /// 1. Loads the current configuration
    /// 2. Finds the project by name
    /// 3. Updates the state to mark this project as active
    /// 4. Persists the updated state
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the project to switch to
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No project with the given name exists
    /// - The state cannot be loaded or saved
    #[instrument(skip(self), fields(name = %name))]
    pub async fn switch_project(&self, name: &str) -> Result<()> {
        info!("Switching project");

        // Load config to find the project
        let config = self
            .config_repo
            .load()
            .await
            .context("Failed to load configuration")?;

        // Find project by name
        let project = config
            .projects
            .list
            .iter()
            .find(|p| p.name == name)
            .with_context(|| format!("Project not found: {}", name))?;

        // Load and update state
        let mut state = self
            .state_repo
            .load()
            .await
            .context("Failed to load state")?;

        // Mark project as active in state
        // Note: In a real implementation, you might want to add a field to State
        // for tracking the active project. For now, we use active_plugins.
        state
            .active_plugins
            .insert(project.path.clone(), "default".to_string());

        self.state_repo
            .save(&state)
            .await
            .context("Failed to save state")?;

        info!("Project switched successfully: {}", name);
        Ok(())
    }

    /// Refresh all data for the current project.
    ///
    /// This method:
    /// 1. Reloads configuration
    /// 2. Refreshes Git status if in a git repository
    /// 3. Updates any cached data
    ///
    /// # Errors
    ///
    /// Returns an error if any of the refresh operations fail.
    #[instrument(skip(self))]
    pub async fn refresh_all(&self) -> Result<()> {
        info!("Refreshing all data");

        // Reload config
        let _config = self
            .config_repo
            .load()
            .await
            .context("Failed to refresh configuration")?;

        // Reload state
        let state = self
            .state_repo
            .load()
            .await
            .context("Failed to refresh state")?;

        for project_path in state.active_plugins.keys() {
            debug!("Refreshing git status for active project: {}", project_path);
            self.git_service
                .status(Path::new(project_path))
                .await
                .with_context(|| format!("Failed to refresh git status for {}", project_path))?;
        }

        info!("All data refreshed successfully");
        Ok(())
    }

    /// Get a reference to the config repository.
    pub fn config_repo(&self) -> &Arc<dyn ConfigRepository> {
        &self.config_repo
    }

    /// Get a reference to the state repository.
    pub fn state_repo(&self) -> &Arc<dyn StateRepository> {
        &self.state_repo
    }

    /// Get a reference to the git service.
    pub fn git_service(&self) -> &Arc<dyn GitService> {
        &self.git_service
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::{Commit, Diff, FileDiff, RepoStatus};
    use crate::shell::repositories::{FileConfigRepository, FileStateRepository};
    use crate::shell::services::{CliGitService, GitService};
    use crate::state::State;
    use async_trait::async_trait;
    use std::sync::Mutex;
    use tempfile::TempDir;

    #[derive(Default)]
    struct RecordingGitService {
        status_calls: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl GitService for RecordingGitService {
        async fn status(&self, repo_path: &Path) -> Result<RepoStatus> {
            self.status_calls
                .lock()
                .unwrap()
                .push(repo_path.display().to_string());
            Ok(RepoStatus::default())
        }

        async fn diff(&self, _repo_path: &Path, _file: &Path) -> Result<Diff> {
            Ok(Diff::default())
        }

        async fn commits(&self, _repo_path: &Path, _limit: usize) -> Result<Vec<Commit>> {
            Ok(Vec::new())
        }

        async fn stage(&self, _repo_path: &Path, _file: &Path) -> Result<()> {
            Ok(())
        }

        async fn unstage(&self, _repo_path: &Path, _file: &Path) -> Result<()> {
            Ok(())
        }

        async fn commit(&self, _repo_path: &Path, _message: &str) -> Result<()> {
            Ok(())
        }

        async fn commit_details(
            &self,
            _repo_path: &Path,
            _commit_hash: &str,
        ) -> Result<Vec<FileDiff>> {
            Ok(Vec::new())
        }

        async fn commit_diff(&self, _repo_path: &Path, _commit_hash: &str) -> Result<Diff> {
            Ok(Diff::default())
        }
    }

    #[tokio::test]
    async fn test_project_new() {
        let config = ProjectConfig {
            id: "test-123".to_string(),
            name: "Test Project".to_string(),
            path: "/tmp/test".to_string(),
            description: None,
            favorite: false,
            tags: vec![],
        };

        let project = Project::new(config);
        assert_eq!(project.id(), "test-123");
        assert_eq!(project.name(), "Test Project");
        assert_eq!(project.path(), "/tmp/test");
        assert!(!project.is_active);
    }

    #[tokio::test]
    async fn test_app_usecase_load_project() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let state_path = temp_dir.path().join("state.json");
        let project_path = temp_dir.path().join("my_project");

        // Create project directory
        tokio::fs::create_dir(&project_path).await.unwrap();

        let usecase = AppUsecase::new(
            Arc::new(FileConfigRepository::new(&config_path)),
            Arc::new(FileStateRepository::new(&state_path)),
            Arc::new(CliGitService::new()),
        );

        let project = usecase.load_project(&project_path).await.unwrap();
        assert_eq!(project.name(), "my_project");
    }

    #[tokio::test]
    async fn test_app_usecase_load_project_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let state_path = temp_dir.path().join("state.json");

        let usecase = AppUsecase::new(
            Arc::new(FileConfigRepository::new(&config_path)),
            Arc::new(FileStateRepository::new(&state_path)),
            Arc::new(CliGitService::new()),
        );

        let result = usecase.load_project(Path::new("/nonexistent/path")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_app_usecase_load_project_not_a_directory() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let state_path = temp_dir.path().join("state.json");
        let file_path = temp_dir.path().join("not_a_dir.txt");

        // Create a file instead of a directory
        tokio::fs::write(&file_path, "content").await.unwrap();

        let usecase = AppUsecase::new(
            Arc::new(FileConfigRepository::new(&config_path)),
            Arc::new(FileStateRepository::new(&state_path)),
            Arc::new(CliGitService::new()),
        );

        let result = usecase.load_project(&file_path).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_refresh_all_refreshes_active_project_git_status() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let state_path = temp_dir.path().join("state.json");
        let project_path = temp_dir.path().join("active_project");
        tokio::fs::create_dir(&project_path).await.unwrap();

        let state_repo = Arc::new(FileStateRepository::new(&state_path));
        let mut state = State::default();
        state
            .active_plugins
            .insert(project_path.display().to_string(), "gitstatus".to_string());
        state_repo.save(&state).await.unwrap();

        let git_service = Arc::new(RecordingGitService::default());
        let usecase = AppUsecase::new(
            Arc::new(FileConfigRepository::new(&config_path)),
            state_repo,
            git_service.clone(),
        );

        usecase.refresh_all().await.unwrap();

        let calls = git_service.status_calls.lock().unwrap();
        assert_eq!(calls.as_slice(), &[project_path.display().to_string()]);
    }
}
