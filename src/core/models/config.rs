//! Core configuration types for RightClick.
//!
//! This module defines the configuration structures used throughout the application.
//! All types are serializable/deserializable via serde for JSON configuration files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Current configuration version.
/// This is incremented when breaking changes are made to the config structure.
pub const CURRENT_CONFIG_VERSION: u32 = 1;

/// Main configuration structure for RightClick.
///
/// This is the root configuration type that is serialized to and deserialized from
/// the configuration file at `~/.config/rightclick/config.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// Version of the configuration format.
    /// Used for migration purposes.
    pub version: u32,

    /// Project-related configuration.
    pub projects: ProjectsConfig,

    /// Plugin-specific configurations.
    pub plugins: PluginsConfig,

    /// UI appearance and behavior settings.
    pub ui: UIConfig,

    /// Keymap customizations.
    pub keymap: KeymapConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CURRENT_CONFIG_VERSION,
            projects: ProjectsConfig::default(),
            plugins: PluginsConfig::default(),
            ui: UIConfig::default(),
            keymap: KeymapConfig::default(),
        }
    }
}

/// Configuration for projects and project management.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ProjectsConfig {
    /// List of configured projects.
    pub list: Vec<ProjectConfig>,
}

/// Configuration for a single project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectConfig {
    /// Unique identifier for the project.
    pub id: String,
    /// Display name of the project.
    pub name: String,
    /// Absolute path to the project root directory.
    pub path: String,
    /// Optional description of the project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this project is marked as a favorite.
    #[serde(default)]
    pub favorite: bool,
    /// Custom tags for the project.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// Plugin-specific configurations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[derive(Default)]
pub struct PluginsConfig {
    /// Git status plugin configuration.
    pub git_status: GitStatusPluginConfig,
    /// Conversations plugin configuration.
    pub conversations: ConversationsPluginConfig,
    /// File browser plugin configuration.
    pub file_browser: FileBrowserPluginConfig,
    /// Workspace plugin configuration.
    pub workspace: WorkspacePluginConfig,
    /// Workers plugin configuration.
    pub workers: WorkersPluginConfig,
}

/// Git status plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitStatusPluginConfig {
    /// Whether the plugin is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Show untracked files in status.
    #[serde(default = "default_true")]
    pub show_untracked: bool,
    /// Auto-refresh interval in seconds (0 = disabled).
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval: u64,
}

impl Default for GitStatusPluginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            show_untracked: true,
            refresh_interval: 5,
        }
    }
}

/// Conversations plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConversationsPluginConfig {
    /// Whether the plugin is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum number of conversations to keep in history.
    #[serde(default = "default_max_conversations")]
    pub max_history: usize,
    /// Whether to save conversation context.
    #[serde(default = "default_true")]
    pub save_context: bool,
}

impl Default for ConversationsPluginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_history: 100,
            save_context: true,
        }
    }
}

/// File browser plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileBrowserPluginConfig {
    /// Whether the plugin is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Show hidden files.
    #[serde(default)]
    pub show_hidden: bool,
    /// Default sort order (name, size, modified).
    #[serde(default = "default_sort_order")]
    pub sort_by: String,
    /// File patterns to ignore.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore_patterns: Vec<String>,
}

impl Default for FileBrowserPluginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            show_hidden: false,
            sort_by: "name".to_string(),
            ignore_patterns: vec![
                "*.tmp".to_string(),
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
            ],
        }
    }
}

/// Workspace plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspacePluginConfig {
    /// Whether the plugin is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Default base branch for new worktrees.
    #[serde(default = "default_branch")]
    pub default_branch: String,
    /// Auto-refresh interval in seconds (0 = disabled).
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval: u64,
    /// Tmux session prefix.
    #[serde(default = "default_tmux_prefix")]
    pub tmux_prefix: String,
}

impl Default for WorkspacePluginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_branch: "main".to_string(),
            refresh_interval: 5,
            tmux_prefix: "rc".to_string(),
        }
    }
}

/// Workers plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkersPluginConfig {
    /// Whether the plugin is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Default AI agent command for worker execution.
    #[serde(default = "default_worker_agent")]
    pub default_agent: String,
    /// Relative directory for intent specifications.
    #[serde(default = "default_intents_dir")]
    pub intents_dir: String,
    /// Relative directory for worker logs.
    #[serde(default = "default_worker_logs_dir")]
    pub logs_dir: String,
}

impl Default for WorkersPluginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_agent: "claude".to_string(),
            intents_dir: ".rightclick/intents".to_string(),
            logs_dir: ".rightclick/logs".to_string(),
        }
    }
}

fn default_worker_agent() -> String {
    "claude".to_string()
}

fn default_intents_dir() -> String {
    ".rightclick/intents".to_string()
}

fn default_worker_logs_dir() -> String {
    ".rightclick/logs".to_string()
}

fn default_branch() -> String {
    "main".to_string()
}

fn default_tmux_prefix() -> String {
    "rc".to_string()
}

/// UI appearance and behavior configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UIConfig {
    /// Show clock in the UI.
    #[serde(default = "default_true")]
    pub show_clock: bool,
    /// Color theme name.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Enable nerd font icons.
    #[serde(default = "default_true")]
    pub nerd_fonts_enabled: bool,
    /// UI accent color (hex or named color).
    #[serde(default = "default_accent_color")]
    pub accent_color: String,
    /// Compact mode (reduces padding).
    #[serde(default)]
    pub compact_mode: bool,
}

impl Default for UIConfig {
    fn default() -> Self {
        Self {
            show_clock: true,
            theme: "dark".to_string(),
            nerd_fonts_enabled: true,
            accent_color: "blue".to_string(),
            compact_mode: false,
        }
    }
}

/// Keymap configuration for custom key bindings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct KeymapConfig {
    /// Map of action names to key combinations.
    /// Keys are action names (e.g., "quit", "save", "open_palette").
    /// Values are key combinations (e.g., "Ctrl+q", "Ctrl+s", "Ctrl+p").
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub overrides: HashMap<String, String>,
}

// Default value helpers for serde

fn default_true() -> bool {
    true
}

fn default_refresh_interval() -> u64 {
    5
}

fn default_max_conversations() -> usize {
    100
}

fn default_sort_order() -> String {
    "name".to_string()
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_accent_color() -> String {
    "blue".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.version, CURRENT_CONFIG_VERSION);
        assert!(config.ui.show_clock);
        assert_eq!(config.ui.theme, "dark");
        assert!(config.plugins.git_status.enabled);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("version"));
        assert!(json.contains("projects"));
        assert!(json.contains("plugins"));
    }

    #[test]
    fn test_config_deserialization() {
        let json = r#"{
            "version": 1,
            "projects": {
                "list": []
            },
            "plugins": {
                "git_status": {
                    "enabled": true,
                    "show_untracked": true,
                    "refresh_interval": 5
                },
                "conversations": {
                    "enabled": true,
                    "max_history": 100,
                    "save_context": true
                },
                "file_browser": {
                    "enabled": true,
                    "show_hidden": false,
                    "sort_by": "name",
                    "ignore_patterns": []
                },
                "workspace": {
                    "enabled": true,
                    "default_branch": "main",
                    "refresh_interval": 5,
                    "tmux_prefix": "rc"
                },
                "workers": {
                    "enabled": true,
                    "default_agent": "claude",
                    "intents_dir": ".rightclick/intents",
                    "logs_dir": ".rightclick/logs"
                }
            },
            "ui": {
                "show_clock": true,
                "theme": "dark",
                "nerd_fonts_enabled": true,
                "accent_color": "blue",
                "compact_mode": false
            },
            "keymap": {
                "overrides": {}
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.version, 1);
        assert!(config.ui.show_clock);
    }
}
