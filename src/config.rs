//! Configuration parsing for agentsync
//!
//! Handles TOML configuration files that define how AI agent
//! configurations should be synchronized via symbolic links.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Default configuration file name
pub const CONFIG_FILE_NAME: &str = "agentsync.toml";

/// Default source directory name
pub const DEFAULT_SOURCE_DIR: &str = ".agents";

/// Root configuration structure
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Directory containing source files (relative to config file)
    #[serde(default = "default_source_dir")]
    pub source_dir: String,

    /// Agent configurations
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,

    /// Gitignore management settings
    #[serde(default)]
    pub gitignore: GitignoreConfig,
}

fn default_source_dir() -> String {
    ".".to_string()
}

/// Configuration for a single AI agent
#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    /// Whether this agent is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// Targets to sync for this agent
    #[serde(default)]
    pub targets: HashMap<String, TargetConfig>,
}

fn default_true() -> bool {
    true
}

/// Configuration for a single sync target
#[derive(Debug, Deserialize)]
pub struct TargetConfig {
    /// Source path (relative to source_dir)
    pub source: String,

    /// Destination path (relative to project root)
    pub destination: String,

    /// Type of sync operation
    #[serde(rename = "type")]
    pub sync_type: SyncType,

    /// Pattern for filtering files (only for symlink-contents)
    #[serde(default)]
    pub pattern: Option<String>,
}

/// Type of synchronization to perform
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SyncType {
    /// Create a single symlink to a file or directory
    Symlink,
    /// Create symlinks for all contents of a directory
    SymlinkContents,
}

/// Gitignore management configuration
#[derive(Debug, Deserialize)]
pub struct GitignoreConfig {
    /// Whether to manage .gitignore
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Marker text for the managed section
    #[serde(default = "default_marker")]
    pub marker: String,

    /// Additional entries to add to .gitignore
    #[serde(default)]
    pub entries: Vec<String>,
}

fn default_marker() -> String {
    "AI Agent Symlinks".to_string()
}

impl Default for GitignoreConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            marker: default_marker(),
            entries: Vec::new(),
        }
    }
}

impl Config {
    /// Load configuration from a file
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok(config)
    }

    /// Find configuration file by searching up from current directory
    pub fn find_config(start_dir: &Path) -> Result<PathBuf> {
        let mut current = start_dir.to_path_buf();

        loop {
            // Check for .agents/agentsync.toml
            let config_path = current.join(DEFAULT_SOURCE_DIR).join(CONFIG_FILE_NAME);
            if config_path.exists() {
                return Ok(config_path);
            }

            // Check for agentsync.toml in root
            let root_config = current.join(CONFIG_FILE_NAME);
            if root_config.exists() {
                return Ok(root_config);
            }

            // Move up to parent directory
            if !current.pop() {
                anyhow::bail!(
                    "Could not find {} in {} or any parent directory",
                    CONFIG_FILE_NAME,
                    start_dir.display()
                );
            }
        }
    }

    /// Get the project root directory (parent of .agents or location of config)
    pub fn project_root(config_path: &Path) -> PathBuf {
        let parent = config_path.parent().unwrap_or(config_path);

        // If config is inside .agents directory, go up one more level
        if parent.file_name().is_some_and(|name| name == DEFAULT_SOURCE_DIR) {
            parent.parent().unwrap_or(parent).to_path_buf()
        } else {
            parent.to_path_buf()
        }
    }

    /// Get the source directory (where source files are located)
    pub fn source_dir(&self, config_path: &Path) -> PathBuf {
        let config_dir = config_path.parent().unwrap_or(config_path);
        config_dir.join(&self.source_dir)
    }

    /// Get all gitignore entries (from config + auto-generated from targets)
    pub fn all_gitignore_entries(&self) -> Vec<String> {
        let mut entries = self.gitignore.entries.clone();

        // Add destinations from all enabled agents
        for agent in self.agents.values() {
            if agent.enabled {
                for target in agent.targets.values() {
                    let dest = &target.destination;
                    if !entries.contains(dest) {
                        entries.push(dest.clone());
                    }
                }
            }
        }

        entries.sort();
        entries.dedup();
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
            [agents.test]
            enabled = true

            [agents.test.targets.main]
            source = "README.md"
            destination = "OUTPUT.md"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert!(config.agents.contains_key("test"));
        assert!(config.agents["test"].enabled);
    }

    #[test]
    fn test_parse_full_config() {
        let toml = r#"
            source_dir = "."

            [gitignore]
            enabled = true
            marker = "Test Marker"
            entries = ["file1.md", "file2.md"]

            [agents.claude]
            enabled = true
            description = "Claude Code"

            [agents.claude.targets.instructions]
            source = "AGENTS.md"
            destination = "CLAUDE.md"
            type = "symlink"

            [agents.copilot]
            enabled = false

            [agents.copilot.targets.skills]
            source = "skills"
            destination = ".copilot/skills"
            type = "symlink-contents"
            pattern = "*.md"
        "#;

        let config: Config = toml::from_str(toml).unwrap();

        assert!(config.agents["claude"].enabled);
        assert!(!config.agents["copilot"].enabled);
        assert_eq!(config.gitignore.marker, "Test Marker");
        assert_eq!(
            config.agents["copilot"].targets["skills"].sync_type,
            SyncType::SymlinkContents
        );
    }
}
