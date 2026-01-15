//! Configuration parsing for agentsync
//!
//! Handles TOML configuration files that define how AI agent
//! configurations should be synchronized via symbolic links.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
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

    /// MCP global settings
    #[serde(default)]
    pub mcp: McpGlobalConfig,

    /// MCP server definitions
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
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

// =============================================================================
// MCP Configuration
// =============================================================================

/// Global MCP configuration settings
#[derive(Debug, Deserialize, Clone)]
pub struct McpGlobalConfig {
    /// Enable/disable MCP propagation globally
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Global merge strategy: 'merge' or 'overwrite'
    #[serde(default)]
    pub merge_strategy: McpMergeStrategy,
}

impl Default for McpGlobalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            merge_strategy: McpMergeStrategy::default(),
        }
    }
}

/// Merge strategy for MCP configurations
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum McpMergeStrategy {
    /// Merge with existing configuration (default)
    #[default]
    Merge,
    /// Overwrite existing configuration completely
    Overwrite,
}

/// Configuration for a single MCP server
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct McpServerConfig {
    /// Command to execute (for stdio transport)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Arguments for the command
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,

    /// Environment variables
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,

    /// URL for HTTP/SSE transport
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// HTTP headers (for remote servers)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,

    /// Transport type (stdio, http, sse)
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub transport_type: Option<String>,

    /// Whether the server is disabled
    #[serde(default, skip_serializing_if = "is_false")]
    pub disabled: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
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
        if parent
            .file_name()
            .is_some_and(|name| name == DEFAULT_SOURCE_DIR)
        {
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
    use tempfile::TempDir;

    // ==========================================================================
    // PARSING TESTS
    // ==========================================================================

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

    #[test]
    fn test_parse_empty_config() {
        let toml = "";
        let config: Config = toml::from_str(toml).unwrap();

        assert!(config.agents.is_empty());
        assert_eq!(config.source_dir, ".");
        assert!(config.gitignore.enabled);
    }

    #[test]
    fn test_parse_config_with_defaults() {
        let toml = r#"
            [agents.test]
            [agents.test.targets.main]
            source = "src.md"
            destination = "dest.md"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();

        // Agent enabled by default
        assert!(config.agents["test"].enabled);
        // Default description is empty
        assert!(config.agents["test"].description.is_empty());
        // Default source_dir
        assert_eq!(config.source_dir, ".");
    }

    #[test]
    fn test_parse_invalid_sync_type() {
        let toml = r#"
            [agents.test.targets.main]
            source = "src.md"
            destination = "dest.md"
            type = "invalid-type"
        "#;

        let result: Result<Config, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_required_fields() {
        let toml = r#"
            [agents.test.targets.main]
            source = "src.md"
            type = "symlink"
        "#;

        let result: Result<Config, _> = toml::from_str(toml);
        assert!(result.is_err(), "Should fail without destination field");
    }

    // ==========================================================================
    // CONFIG LOAD TESTS
    // ==========================================================================

    #[test]
    fn test_load_config_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("agentsync.toml");

        let config_content = r#"
            source_dir = "custom_dir"
            
            [agents.test]
            enabled = true
            
            [agents.test.targets.main]
            source = "src.md"
            destination = "dest.md"
            type = "symlink"
        "#;

        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        assert_eq!(config.source_dir, "custom_dir");
        assert!(config.agents.contains_key("test"));
    }

    #[test]
    fn test_load_config_file_not_found() {
        let result = Config::load(Path::new("/nonexistent/path/agentsync.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_config_invalid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("agentsync.toml");

        fs::write(&config_path, "this is not { valid toml").unwrap();

        let result = Config::load(&config_path);
        assert!(result.is_err());
    }

    // ==========================================================================
    // FIND CONFIG TESTS
    // ==========================================================================

    #[test]
    fn test_find_config_in_agents_dir() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        fs::write(&config_path, "[agents]").unwrap();

        let found = Config::find_config(temp_dir.path()).unwrap();
        assert_eq!(found, config_path);
    }

    #[test]
    fn test_find_config_in_root() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("agentsync.toml");
        fs::write(&config_path, "[agents]").unwrap();

        let found = Config::find_config(temp_dir.path()).unwrap();
        assert_eq!(found, config_path);
    }

    #[test]
    fn test_find_config_prefers_agents_dir() {
        let temp_dir = TempDir::new().unwrap();

        // Create both configs
        let root_config = temp_dir.path().join("agentsync.toml");
        fs::write(&root_config, "[agents]").unwrap();

        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        let agents_config = agents_dir.join("agentsync.toml");
        fs::write(&agents_config, "[agents]").unwrap();

        // Should prefer .agents/agentsync.toml
        let found = Config::find_config(temp_dir.path()).unwrap();
        assert_eq!(found, agents_config);
    }

    #[test]
    fn test_find_config_searches_parent_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let nested_dir = temp_dir.path().join("sub1").join("sub2").join("sub3");
        fs::create_dir_all(&nested_dir).unwrap();

        // Config at root
        let config_path = temp_dir.path().join("agentsync.toml");
        fs::write(&config_path, "[agents]").unwrap();

        // Search from nested directory
        let found = Config::find_config(&nested_dir).unwrap();
        assert_eq!(found, config_path);
    }

    #[test]
    fn test_find_config_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let result = Config::find_config(temp_dir.path());
        assert!(result.is_err());
    }

    // ==========================================================================
    // PROJECT ROOT TESTS
    // ==========================================================================

    #[test]
    fn test_project_root_from_agents_dir() {
        let config_path = PathBuf::from("/project/.agents/agentsync.toml");
        let root = Config::project_root(&config_path);
        assert_eq!(root, PathBuf::from("/project"));
    }

    #[test]
    fn test_project_root_from_root_config() {
        let config_path = PathBuf::from("/project/agentsync.toml");
        let root = Config::project_root(&config_path);
        assert_eq!(root, PathBuf::from("/project"));
    }

    #[test]
    fn test_project_root_nested_agents_dir() {
        // Edge case: .agents inside another directory
        let config_path = PathBuf::from("/project/subdir/.agents/agentsync.toml");
        let root = Config::project_root(&config_path);
        assert_eq!(root, PathBuf::from("/project/subdir"));
    }

    // ==========================================================================
    // SOURCE DIR TESTS
    // ==========================================================================

    #[test]
    fn test_source_dir_default() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        fs::write(&config_path, "source_dir = \".\"").unwrap();

        let config = Config::load(&config_path).unwrap();
        let source = config.source_dir(&config_path);

        assert_eq!(source, agents_dir.join("."));
    }

    #[test]
    fn test_source_dir_custom() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("agentsync.toml");

        fs::write(&config_path, "source_dir = \"custom/sources\"").unwrap();

        let config = Config::load(&config_path).unwrap();
        let source = config.source_dir(&config_path);

        assert_eq!(source, temp_dir.path().join("custom/sources"));
    }

    // ==========================================================================
    // GITIGNORE ENTRIES TESTS
    // ==========================================================================

    #[test]
    fn test_all_gitignore_entries_collects_destinations() {
        let toml = r#"
            [gitignore]
            entries = ["manual-entry.md"]

            [agents.claude]
            enabled = true
            
            [agents.claude.targets.instructions]
            source = "AGENTS.md"
            destination = "CLAUDE.md"
            type = "symlink"

            [agents.copilot]
            enabled = true
            
            [agents.copilot.targets.instructions]
            source = "AGENTS.md"
            destination = ".github/copilot-instructions.md"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        assert!(entries.contains(&"manual-entry.md".to_string()));
        assert!(entries.contains(&"CLAUDE.md".to_string()));
        assert!(entries.contains(&".github/copilot-instructions.md".to_string()));
    }

    #[test]
    fn test_all_gitignore_entries_skips_disabled_agents() {
        let toml = r#"
            [agents.enabled_agent]
            enabled = true
            
            [agents.enabled_agent.targets.main]
            source = "src.md"
            destination = "enabled.md"
            type = "symlink"

            [agents.disabled_agent]
            enabled = false
            
            [agents.disabled_agent.targets.main]
            source = "src.md"
            destination = "disabled.md"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        assert!(entries.contains(&"enabled.md".to_string()));
        assert!(!entries.contains(&"disabled.md".to_string()));
    }

    #[test]
    fn test_all_gitignore_entries_deduplicates() {
        let toml = r#"
            [gitignore]
            entries = ["AGENTS.md"]

            [agents.agent1]
            enabled = true
            [agents.agent1.targets.main]
            source = "src.md"
            destination = "AGENTS.md"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        // Should only appear once
        assert_eq!(entries.iter().filter(|e| *e == "AGENTS.md").count(), 1);
    }

    #[test]
    fn test_all_gitignore_entries_sorted() {
        let toml = r#"
            [agents.z_agent]
            enabled = true
            [agents.z_agent.targets.main]
            source = "z.md"
            destination = "z-dest.md"
            type = "symlink"

            [agents.a_agent]
            enabled = true
            [agents.a_agent.targets.main]
            source = "a.md"
            destination = "a-dest.md"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        // Should be sorted alphabetically
        let mut sorted = entries.clone();
        sorted.sort();
        assert_eq!(entries, sorted);
    }

    // ==========================================================================
    // SYNC TYPE TESTS
    // ==========================================================================

    #[test]
    fn test_sync_type_symlink() {
        let toml = r#"
            [agents.test.targets.main]
            source = "src"
            destination = "dest"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            config.agents["test"].targets["main"].sync_type,
            SyncType::Symlink
        );
    }

    #[test]
    fn test_sync_type_symlink_contents() {
        let toml = r#"
            [agents.test.targets.main]
            source = "src"
            destination = "dest"
            type = "symlink-contents"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            config.agents["test"].targets["main"].sync_type,
            SyncType::SymlinkContents
        );
    }

    // ==========================================================================
    // GITIGNORE CONFIG DEFAULTS TESTS
    // ==========================================================================

    #[test]
    fn test_gitignore_config_defaults() {
        let config = GitignoreConfig::default();

        assert!(config.enabled);
        assert_eq!(config.marker, "AI Agent Symlinks");
        assert!(config.entries.is_empty());
    }

    #[test]
    fn test_gitignore_config_custom_marker() {
        let toml = r#"
            [gitignore]
            marker = "Custom Marker"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.gitignore.marker, "Custom Marker");
    }

    #[test]
    fn test_gitignore_config_disabled() {
        let toml = r#"
            [gitignore]
            enabled = false
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert!(!config.gitignore.enabled);
    }

    // ==========================================================================
    // MCP CONFIG TESTS
    // ==========================================================================

    #[test]
    fn test_mcp_config_defaults() {
        let toml = "";
        let config: Config = toml::from_str(toml).unwrap();

        assert!(config.mcp.enabled);
        assert_eq!(config.mcp.merge_strategy, McpMergeStrategy::Merge);
        assert!(config.mcp_servers.is_empty());
    }

    #[test]
    fn test_mcp_config_disabled() {
        let toml = r#"
            [mcp]
            enabled = false
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert!(!config.mcp.enabled);
    }

    #[test]
    fn test_mcp_merge_strategy_overwrite() {
        let toml = r#"
            [mcp]
            merge_strategy = "overwrite"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.mcp.merge_strategy, McpMergeStrategy::Overwrite);
    }

    #[test]
    fn test_mcp_server_stdio_config() {
        let toml = r#"
            [mcp_servers.filesystem]
            command = "npx"
            args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/project"]
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let server = &config.mcp_servers["filesystem"];

        assert_eq!(server.command.as_deref(), Some("npx"));
        assert_eq!(server.args.len(), 3);
        assert_eq!(server.args[0], "-y");
        assert!(server.url.is_none());
    }

    #[test]
    fn test_mcp_server_with_env() {
        let toml = r#"
            [mcp_servers.test]
            command = "node"
            args = ["server.js"]
            
            [mcp_servers.test.env]
            DEBUG = "true"
            API_KEY = "${MY_API_KEY}"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let server = &config.mcp_servers["test"];

        assert_eq!(server.env.get("DEBUG"), Some(&"true".to_string()));
        assert_eq!(
            server.env.get("API_KEY"),
            Some(&"${MY_API_KEY}".to_string())
        );
    }

    #[test]
    fn test_mcp_server_remote_url() {
        let toml = r#"
            [mcp_servers.remote_api]
            url = "https://api.example.com"
            
            [mcp_servers.remote_api.headers]
            Authorization = "Bearer token123"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let server = &config.mcp_servers["remote_api"];

        assert_eq!(server.url.as_deref(), Some("https://api.example.com"));
        assert_eq!(
            server.headers.get("Authorization"),
            Some(&"Bearer token123".to_string())
        );
        assert!(server.command.is_none());
    }

    #[test]
    fn test_mcp_server_disabled() {
        let toml = r#"
            [mcp_servers.disabled_server]
            command = "npx"
            args = ["server"]
            disabled = true
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let server = &config.mcp_servers["disabled_server"];

        assert!(server.disabled);
    }

    #[test]
    fn test_mcp_multiple_servers() {
        let toml = r#"
            [mcp]
            enabled = true
            merge_strategy = "merge"
            
            [mcp_servers.filesystem]
            command = "npx"
            args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
            
            [mcp_servers.git]
            command = "npx"
            args = ["-y", "@modelcontextprotocol/server-git", "--repository", "."]
            
            [mcp_servers.postgres]
            command = "npx"
            args = ["-y", "@modelcontextprotocol/server-postgres"]
            
            [mcp_servers.postgres.env]
            DATABASE_URL = "${DATABASE_URL}"
        "#;

        let config: Config = toml::from_str(toml).unwrap();

        assert_eq!(config.mcp_servers.len(), 3);
        assert!(config.mcp_servers.contains_key("filesystem"));
        assert!(config.mcp_servers.contains_key("git"));
        assert!(config.mcp_servers.contains_key("postgres"));
    }

    #[test]
    fn test_mcp_full_config_with_agents() {
        let toml = r#"
            source_dir = "."
            
            [mcp]
            enabled = true
            
            [mcp_servers.filesystem]
            command = "npx"
            args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
            
            [agents.claude]
            enabled = true
            
            [agents.claude.targets.instructions]
            source = "AGENTS.md"
            destination = "CLAUDE.md"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();

        // MCP config should coexist with agent config
        assert!(config.mcp.enabled);
        assert_eq!(config.mcp_servers.len(), 1);
        assert!(config.agents.contains_key("claude"));
    }

    #[test]
    fn test_mcp_server_config_serialization() {
        let server = McpServerConfig {
            command: Some("npx".to_string()),
            args: vec!["-y".to_string(), "server".to_string()],
            env: HashMap::from([("DEBUG".to_string(), "true".to_string())]),
            url: None,
            headers: HashMap::new(),
            transport_type: Some("stdio".to_string()),
            disabled: false,
        };

        let json = serde_json::to_string(&server).unwrap();

        // Should serialize command and args
        assert!(json.contains("\"command\":\"npx\""));
        assert!(json.contains("\"args\":["));
        // Should NOT serialize empty headers
        assert!(!json.contains("\"headers\""));
        // Should NOT serialize disabled=false
        assert!(!json.contains("\"disabled\""));
    }
}
