//! Configuration parsing for agentsync
//!
//! Handles TOML configuration files that define how AI agent
//! configurations should be synchronized via symbolic links.

use crate::agent_ids;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Default configuration file name
pub const CONFIG_FILE_NAME: &str = "agentsync.toml";

/// Default source directory name
pub const DEFAULT_SOURCE_DIR: &str = ".agents";

/// Represents the root of the `agentsync.toml` configuration file.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// The directory where source files for agents are stored, relative to the
    /// configuration file. Defaults to ".".
    #[serde(default = "default_source_dir")]
    pub source_dir: String,

    /// If true, generate a compressed AGENTS.md and point AGENTS.md symlinks to it.
    #[serde(default)]
    pub compress_agents_md: bool,

    /// Default agents to run when --agents is not specified.
    /// Uses case-insensitive substring matching.
    /// If empty, all enabled agents will be processed.
    #[serde(default)]
    pub default_agents: Vec<String>,

    /// A map of agent configurations, where the key is the agent's name (e.g., "claude").
    /// Uses BTreeMap to ensure deterministic ordering in config and generated files
    /// while avoiding redundant sorting overhead.
    #[serde(default)]
    pub agents: BTreeMap<String, AgentConfig>,

    /// Settings for managing the project's `.gitignore` file.
    #[serde(default)]
    pub gitignore: GitignoreConfig,

    /// Global settings for Model Context Protocol (MCP) integration.
    #[serde(default)]
    pub mcp: McpGlobalConfig,

    /// A map of MCP server configurations, where the key is a unique server name.
    /// Uses BTreeMap to maintain deterministic order without manual sorting.
    #[serde(default)]
    pub mcp_servers: BTreeMap<String, McpServerConfig>,
}

fn default_source_dir() -> String {
    ".".to_string()
}

/// Defines the configuration for a single AI agent, such as Claude or GitHub Copilot.
/// Corresponds to a `[agents.agent_name]` section in `agentsync.toml`.
#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    /// If `true`, this agent's configuration will be processed. Defaults to `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// A human-readable description of the agent. Not used programmatically.
    #[serde(default)]
    pub description: String,

    /// A map of synchronization targets for this agent, where the key is a logical name
    /// for the target (e.g., "instructions").
    #[serde(default)]
    pub targets: BTreeMap<String, TargetConfig>,
}

fn default_true() -> bool {
    true
}

/// Specifies a single file or directory synchronization rule for an agent.
/// Corresponds to a `[agents.agent_name.targets.target_name]` section.
#[derive(Debug, Deserialize)]
pub struct TargetConfig {
    /// The source file or directory, relative to `Config.source_dir`.
    /// For `nested-glob` targets this is the root directory to search, relative
    /// to the project root (not `source_dir`).
    pub source: String,

    /// The destination path for the symlink, relative to the project root.
    /// For `nested-glob` targets this is a template string where the following
    /// placeholders are replaced for each discovered file:
    ///   - `{relative_path}` – the parent directory of the matched file,
    ///     relative to the search root (e.g. `clients/agent-runtime`)
    ///   - `{file_name}` – the matched file's name (e.g. `AGENTS.md`)
    ///   - `{stem}` – the file name without its extension (e.g. `AGENTS`)
    ///   - `{ext}` – the file extension without the leading dot (e.g. `md`)
    pub destination: String,

    /// The type of synchronization to perform.
    #[serde(rename = "type")]
    pub sync_type: SyncType,

    /// An optional glob pattern.
    ///
    /// * For `symlink-contents`: filters which items inside `source` are linked
    ///   (e.g. `*.md` links only Markdown files).
    /// * For `nested-glob`: the recursive glob pattern used to discover files
    ///   under the search root (e.g. `**/AGENTS.md`).
    #[serde(default)]
    pub pattern: Option<String>,

    /// Glob patterns that exclude paths from a `nested-glob` search.
    /// Each pattern is matched against discovered paths relative to the search
    /// root, including both files and directories. Directory matches are used
    /// to prune whole subtrees during traversal, so `node_modules`,
    /// `node_modules/**`, `.git`, and `**/.git/**` all prevent descending into
    /// those directories. Has no effect on other target types.
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Mappings for `module-map` targets. Each mapping defines a
    /// source file and destination directory pair. Ignored for other
    /// sync types.
    #[serde(default)]
    pub mappings: Vec<ModuleMapping>,
}

/// The type of synchronization operation to perform for a target.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SyncType {
    /// Creates a single symlink from `source` to `destination`.
    /// If `source` is a directory, the entire directory is linked.
    Symlink,
    /// Creates symlinks for each item *inside* the `source` directory at the `destination`.
    /// The `destination` directory will be created if it doesn't exist.
    SymlinkContents,
    /// Recursively discovers files matching a glob `pattern` under `source`
    /// (relative to the project root) and creates a symlink for each one at the
    /// path produced by expanding the `destination` template.
    NestedGlob,
    /// Maps centrally-managed source files to specific module directories,
    /// creating a symlink per mapping with convention-based filenames.
    ModuleMap,
}

/// A single source-to-destination mapping within a `module-map` target.
/// Maps a centrally-managed source file to a specific module directory
/// with an optional filename override.
#[derive(Debug, Deserialize, Clone)]
pub struct ModuleMapping {
    /// Source file path, relative to `source_dir` (e.g., "api-context.md").
    pub source: String,

    /// Destination directory, relative to project root (e.g., "src/api").
    pub destination: String,

    /// Override the output filename. If `None`, uses the agent's convention
    /// filename (e.g., CLAUDE.md for claude) or falls back to the source
    /// file's basename.
    #[serde(default)]
    pub filename_override: Option<String>,
}

/// Resolve the output filename for a module-map mapping.
///
/// Priority: filename_override > agent convention > source basename.
pub fn resolve_module_map_filename(mapping: &ModuleMapping, agent_name: &str) -> String {
    if let Some(ref override_name) = mapping.filename_override {
        return override_name.clone();
    }
    if let Some(convention) = crate::agent_ids::agent_convention_filename(agent_name) {
        return convention.to_string();
    }
    std::path::Path::new(&mapping.source)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| mapping.source.clone())
}

/// Configuration for managing the `.gitignore` file.
/// Corresponds to the `[gitignore]` section in `agentsync.toml`.
#[derive(Debug, Deserialize)]
pub struct GitignoreConfig {
    /// If `true`, agentsync will add symlink destinations to `.gitignore`. Defaults to `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// The marker used to delineate the section in `.gitignore` managed by agentsync.
    /// Defaults to "AI Agent Symlinks".
    #[serde(default = "default_marker")]
    pub marker: String,

    /// A list of additional paths to include in the `.gitignore` managed section.
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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,

    /// URL for HTTP/SSE transport
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// HTTP headers (for remote servers)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,

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

    /// Get all gitignore entries (from config + auto-generated from targets + known patterns)
    /// Uses a BTreeSet for efficient deduplication and automatic sorting.
    pub fn all_gitignore_entries(&self) -> Vec<String> {
        let mut entries: BTreeSet<String> = self.gitignore.entries.iter().cloned().collect();

        entries.insert(".agents/skills/*.bak".to_string()); // Defensive pattern to ignore skill backup files even if skills aren't used yet
        // Add destinations from all enabled agents and their known patterns
        for (agent_name, agent) in &self.agents {
            if agent.enabled {
                // Add target destinations
                for target in agent.targets.values() {
                    // NestedGlob destinations are templates, not literal paths –
                    // skip them to avoid polluting .gitignore with raw template strings.
                    if target.sync_type == SyncType::NestedGlob {
                        continue;
                    }
                    // ModuleMap targets expand each mapping into its own gitignore entry
                    if target.sync_type == SyncType::ModuleMap {
                        for mapping in &target.mappings {
                            let filename = resolve_module_map_filename(mapping, agent_name);
                            let entry = format!("{}/{}", mapping.destination, filename);
                            entries.insert(normalize_managed_gitignore_entry(&entry));
                            entries.insert(normalize_managed_gitignore_entry(&format!(
                                "{}.bak",
                                entry
                            )));
                        }
                        continue;
                    }
                    entries.insert(normalize_managed_gitignore_entry(&target.destination));
                    entries.insert(normalize_managed_gitignore_entry(&format!(
                        "{}.bak",
                        target.destination
                    )));
                }

                // Add known ignore patterns for this agent
                for pattern in Self::known_ignore_patterns(agent_name) {
                    entries.insert(normalize_managed_gitignore_entry(pattern));
                }
            }
        }

        entries.into_iter().collect()
    }

    /// Get known gitignore patterns for a specific agent.
    /// These are files/directories that agents generate but are not direct symlink targets.
    pub fn known_ignore_patterns(agent_name: &str) -> &'static [&'static str] {
        agent_ids::known_ignore_patterns(agent_name)
    }
}

fn normalize_managed_gitignore_entry(entry: &str) -> String {
    if entry.contains('/')
        || entry.ends_with('/')
        || entry.contains('*')
        || entry.contains('?')
        || entry.contains('[')
        || entry.starts_with('!')
    {
        return entry.to_string();
    }

    format!("/{entry}")
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
        assert!(!config.compress_agents_md);
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
    fn test_parse_compress_agents_md_enabled() {
        let toml = r#"
            compress_agents_md = true

            [agents.test]
            enabled = true

            [agents.test.targets.main]
            source = "AGENTS.md"
            destination = "TEST.md"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert!(config.compress_agents_md);
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
    fn test_parse_nested_glob_target() {
        let toml = r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.nested]
            source = "."
            pattern = "**/AGENTS.md"
            exclude = [".agents/**", "node_modules/**"]
            destination = "{relative_path}/CLAUDE.md"
            type = "nested-glob"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let target = &config.agents["claude"].targets["nested"];
        assert_eq!(target.sync_type, SyncType::NestedGlob);
        assert_eq!(target.source, ".");
        assert_eq!(target.pattern.as_deref(), Some("**/AGENTS.md"));
        assert_eq!(target.destination, "{relative_path}/CLAUDE.md");
        assert_eq!(target.exclude, vec![".agents/**", "node_modules/**"]);
    }

    #[test]
    fn test_nested_glob_destination_not_added_to_gitignore() {
        let toml = r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.nested]
            source = "."
            pattern = "**/AGENTS.md"
            destination = "{relative_path}/CLAUDE.md"
            type = "nested-glob"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();
        // Template string should not appear in gitignore entries
        assert!(
            !entries.contains(&"{relative_path}/CLAUDE.md".to_string()),
            "Template destination must not be added to gitignore verbatim"
        );
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
        assert!(entries.contains(&"/CLAUDE.md".to_string()));
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

        assert!(entries.contains(&"/enabled.md".to_string()));
        assert!(entries.contains(&"/enabled.md.bak".to_string()));
        assert!(!entries.contains(&"disabled.md".to_string()));
        assert!(!entries.contains(&"disabled.md.bak".to_string()));
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

        assert!(entries.contains(&"AGENTS.md".to_string()));
        assert!(entries.contains(&"/AGENTS.md".to_string()));
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
    // KNOWN IGNORE PATTERNS TESTS
    // ==========================================================================

    #[test]
    fn test_known_ignore_patterns_claude() {
        let patterns = Config::known_ignore_patterns("claude");
        assert!(patterns.contains(&".mcp.json"));
        assert!(patterns.contains(&".claude/commands/"));
        assert!(patterns.contains(&".claude/skills/"));
    }

    #[test]
    fn test_known_ignore_patterns_copilot() {
        let patterns = Config::known_ignore_patterns("copilot");
        assert!(patterns.contains(&".vscode/mcp.json"));
    }

    #[test]
    fn test_known_ignore_patterns_codex() {
        let patterns = Config::known_ignore_patterns("codex");
        assert!(patterns.contains(&".codex/config.toml"));
    }

    #[test]
    fn test_known_ignore_patterns_codex_aliases() {
        assert_eq!(
            Config::known_ignore_patterns("codex"),
            Config::known_ignore_patterns("codex-cli")
        );
        assert_eq!(
            Config::known_ignore_patterns("codex"),
            Config::known_ignore_patterns("codex_cli")
        );
    }

    #[test]
    fn test_known_ignore_patterns_gemini() {
        let patterns = Config::known_ignore_patterns("gemini");
        assert!(patterns.contains(&"GEMINI.md"));
        assert!(patterns.contains(&".gemini/settings.json"));
        assert!(patterns.contains(&".gemini/commands/"));
        assert!(patterns.contains(&".gemini/skills/"));
    }

    #[test]
    fn test_known_ignore_patterns_opencode() {
        let patterns = Config::known_ignore_patterns("opencode");
        assert!(patterns.contains(&"opencode.json"));
    }

    #[test]
    fn test_known_ignore_patterns_cursor() {
        let patterns = Config::known_ignore_patterns("cursor");
        assert!(patterns.contains(&".cursor/mcp.json"));
        assert!(patterns.contains(&".cursor/skills/"));
    }

    #[test]
    fn test_known_ignore_patterns_vscode() {
        let patterns = Config::known_ignore_patterns("vscode");
        assert!(patterns.contains(&".vscode/mcp.json"));
    }

    #[test]
    fn test_known_ignore_patterns_case_insensitive() {
        // Should work with different cases
        assert_eq!(
            Config::known_ignore_patterns("CLAUDE"),
            Config::known_ignore_patterns("claude")
        );
        assert_eq!(
            Config::known_ignore_patterns("Claude"),
            Config::known_ignore_patterns("claude")
        );
        assert_eq!(
            Config::known_ignore_patterns("CoPiLoT"),
            Config::known_ignore_patterns("copilot")
        );
    }

    #[test]
    fn test_known_ignore_patterns_unknown_agent() {
        let patterns = Config::known_ignore_patterns("unknown-agent");
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_all_gitignore_entries_includes_known_patterns() {
        let toml = r#"
            [agents.claude]
            enabled = true
            
            [agents.claude.targets.instructions]
            source = "AGENTS.md"
            destination = "CLAUDE.md"
            type = "symlink"

            [agents.opencode]
            enabled = true
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        // Should include target destinations
        assert!(entries.contains(&"/CLAUDE.md".to_string()));

        // Should include known patterns for Claude
        assert!(entries.contains(&"/.mcp.json".to_string()));
        assert!(entries.contains(&".claude/commands/".to_string()));
        assert!(entries.contains(&".claude/skills/".to_string()));

        // Should include known patterns for OpenCode
        assert!(entries.contains(&"/opencode.json".to_string()));
    }

    #[test]
    fn test_all_gitignore_entries_disabled_agents_no_known_patterns() {
        let toml = r#"
            [agents.claude]
            enabled = false

            [agents.opencode]
            enabled = true
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        // Should NOT include Claude patterns (disabled)
        assert!(!entries.contains(&".mcp.json".to_string()));
        assert!(!entries.contains(&".claude/commands/".to_string()));

        // Should include OpenCode patterns (enabled)
        assert!(entries.contains(&"/opencode.json".to_string()));
    }

    #[test]
    fn test_all_gitignore_entries_deduplicates_known_patterns() {
        let toml = r#"
            [gitignore]
            entries = [".mcp.json", "opencode.json"]

            [agents.claude]
            enabled = true

            [agents.opencode]
            enabled = true
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        // Manual bare entries remain verbatim while managed known patterns are root-scoped
        assert_eq!(entries.iter().filter(|e| *e == ".mcp.json").count(), 1);
        assert_eq!(entries.iter().filter(|e| *e == "opencode.json").count(), 1);
        assert_eq!(entries.iter().filter(|e| *e == "/.mcp.json").count(), 1);
        assert_eq!(entries.iter().filter(|e| *e == "/opencode.json").count(), 1);
    }

    #[test]
    fn test_all_gitignore_entries_manual_entries_plus_known() {
        let toml = r#"
            [gitignore]
            entries = ["custom-file.txt", "another-file.md"]

            [agents.claude]
            enabled = true
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        // Should include manual entries
        assert!(entries.contains(&"custom-file.txt".to_string()));
        assert!(entries.contains(&"another-file.md".to_string()));

        // Should include known patterns
        assert!(entries.contains(&"/.mcp.json".to_string()));
    }

    #[test]
    fn test_all_gitignore_entries_includes_known_patterns_for_alias_agents() {
        let toml = r#"
            [agents.codex-cli]
            enabled = true
            [agents.codex-cli.targets.main]
            source = "AGENTS.md"
            destination = "AGENTS.md"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        assert!(entries.contains(&".codex/config.toml".to_string()));
        assert!(entries.contains(&"/AGENTS.md".to_string()));
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
            env: BTreeMap::from([("DEBUG".to_string(), "true".to_string())]),
            url: None,
            headers: BTreeMap::new(),
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

    // ==========================================================================
    // DEFAULT AGENTS TESTS
    // ==========================================================================

    #[test]
    fn test_default_agents_empty_by_default() {
        let toml = "";
        let config: Config = toml::from_str(toml).unwrap();

        assert!(config.default_agents.is_empty());
    }

    #[test]
    fn test_default_agents_parsing() {
        let toml = r#"
            default_agents = ["copilot", "claude", "gemini"]

            [agents.copilot]
            enabled = true

            [agents.claude]
            enabled = true

            [agents.gemini]
            enabled = true

            [agents.cursor]
            enabled = true
        "#;

        let config: Config = toml::from_str(toml).unwrap();

        assert_eq!(config.default_agents.len(), 3);
        assert!(config.default_agents.contains(&"copilot".to_string()));
        assert!(config.default_agents.contains(&"claude".to_string()));
        assert!(config.default_agents.contains(&"gemini".to_string()));
        assert!(!config.default_agents.contains(&"cursor".to_string()));
    }

    #[test]
    fn test_default_agents_single_agent() {
        let toml = r#"
            default_agents = ["claude"]
        "#;

        let config: Config = toml::from_str(toml).unwrap();

        assert_eq!(config.default_agents.len(), 1);
        assert_eq!(config.default_agents[0], "claude");
    }

    #[test]
    fn test_default_agents_with_other_config() {
        let toml = r#"
            source_dir = "."
            default_agents = ["copilot", "claude"]

            [gitignore]
            enabled = true

            [agents.copilot]
            enabled = true
            description = "GitHub Copilot"

            [agents.claude]
            enabled = true
            description = "Claude Code"
        "#;

        let config: Config = toml::from_str(toml).unwrap();

        assert_eq!(config.default_agents.len(), 2);
        assert_eq!(config.source_dir, ".");
        assert!(config.gitignore.enabled);
        assert_eq!(config.agents["copilot"].description, "GitHub Copilot");
    }

    #[test]
    fn test_all_gitignore_entries_includes_backup_patterns() {
        let toml = r#"
            [agents.test]
            enabled = true
            [agents.test.targets.main]
            source = "README.md"
            destination = "OUTPUT.md"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        assert!(entries.contains(&"/OUTPUT.md".to_string()));
        assert!(entries.contains(&"/OUTPUT.md.bak".to_string()));
        assert!(entries.contains(&".agents/skills/*.bak".to_string()));
    }

    #[test]
    fn test_all_gitignore_entries_manual_bare_entry_remains_unchanged() {
        let toml = r#"
            [gitignore]
            entries = ["AGENTS.md", "docs/AGENTS.md"]

            [agents.claude]
            enabled = true

            [agents.claude.targets.instructions]
            source = "AGENTS.md"
            destination = "AGENTS.md"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        assert!(entries.contains(&"AGENTS.md".to_string()));
        assert!(entries.contains(&"docs/AGENTS.md".to_string()));
        assert!(entries.contains(&"/AGENTS.md".to_string()));
    }

    #[test]
    fn test_all_gitignore_entries_root_level_known_patterns_are_root_scoped() {
        let toml = r#"
            [agents.claude]
            enabled = true

            [agents.gemini]
            enabled = true

            [agents.opencode]
            enabled = true

            [agents.warp]
            enabled = true
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        assert!(entries.contains(&"/.mcp.json".to_string()));
        assert!(entries.contains(&"/GEMINI.md".to_string()));
        assert!(entries.contains(&"/opencode.json".to_string()));
        assert!(entries.contains(&"/WARP.md".to_string()));
        assert!(entries.contains(&".claude/commands/".to_string()));
        assert!(entries.contains(&".opencode/command/".to_string()));
    }

    #[test]
    fn test_all_gitignore_entries_root_destinations_and_backups_are_root_scoped() {
        let toml = r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.instructions]
            source = "AGENTS.md"
            destination = "AGENTS.md"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        assert!(entries.contains(&"/AGENTS.md".to_string()));
        assert!(entries.contains(&"/AGENTS.md.bak".to_string()));
        assert!(!entries.contains(&"AGENTS.md.bak".to_string()));
    }

    // ==========================================================================
    // MODULE-MAP TESTS
    // ==========================================================================

    #[test]
    fn test_parse_module_map_sync_type() {
        let toml = r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "."
            destination = "."
            type = "module-map"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            config.agents["claude"].targets["modules"].sync_type,
            SyncType::ModuleMap
        );
    }

    #[test]
    fn test_parse_module_mapping_struct() {
        let toml = r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "."
            destination = "."
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "api-context.md"
            destination = "src/api"

            [[agents.claude.targets.modules.mappings]]
            source = "ui-context.md"
            destination = "src/ui"
            filename_override = "custom.md"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let mappings = &config.agents["claude"].targets["modules"].mappings;
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0].source, "api-context.md");
        assert_eq!(mappings[0].destination, "src/api");
        assert!(mappings[0].filename_override.is_none());
        assert_eq!(mappings[1].source, "ui-context.md");
        assert_eq!(mappings[1].destination, "src/ui");
        assert_eq!(mappings[1].filename_override.as_deref(), Some("custom.md"));
    }

    #[test]
    fn test_parse_target_config_without_mappings_defaults() {
        let toml = r#"
            [agents.test]
            enabled = true

            [agents.test.targets.main]
            source = "README.md"
            destination = "OUTPUT.md"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        // mappings should default to empty vec when not specified
        assert!(config.agents["test"].targets["main"].mappings.is_empty());
    }

    #[test]
    fn test_resolve_module_map_filename_override() {
        let mapping = ModuleMapping {
            source: "api-context.md".to_string(),
            destination: "src/api".to_string(),
            filename_override: Some("custom-name.md".to_string()),
        };
        assert_eq!(
            resolve_module_map_filename(&mapping, "claude"),
            "custom-name.md"
        );
    }

    #[test]
    fn test_resolve_module_map_filename_convention_claude() {
        let mapping = ModuleMapping {
            source: "api-context.md".to_string(),
            destination: "src/api".to_string(),
            filename_override: None,
        };
        assert_eq!(resolve_module_map_filename(&mapping, "claude"), "CLAUDE.md");
    }

    #[test]
    fn test_resolve_module_map_filename_convention_is_case_insensitive() {
        let mapping = ModuleMapping {
            source: "api-context.md".to_string(),
            destination: "src/api".to_string(),
            filename_override: None,
        };
        assert_eq!(resolve_module_map_filename(&mapping, "Claude"), "CLAUDE.md");
    }

    #[test]
    fn test_resolve_module_map_filename_convention_copilot() {
        let mapping = ModuleMapping {
            source: "api-context.md".to_string(),
            destination: "src/api".to_string(),
            filename_override: None,
        };
        assert_eq!(
            resolve_module_map_filename(&mapping, "copilot"),
            ".github/copilot-instructions.md"
        );
    }

    #[test]
    fn test_resolve_module_map_filename_fallback_unknown_agent() {
        let mapping = ModuleMapping {
            source: "api-context.md".to_string(),
            destination: "src/api".to_string(),
            filename_override: None,
        };
        // Unknown agent → falls back to source basename
        assert_eq!(
            resolve_module_map_filename(&mapping, "unknown-agent"),
            "api-context.md"
        );
    }

    #[test]
    fn test_resolve_module_map_filename_override_beats_convention() {
        let mapping = ModuleMapping {
            source: "api-context.md".to_string(),
            destination: "src/api".to_string(),
            filename_override: Some("MY-RULES.md".to_string()),
        };
        // Override should take precedence over claude convention
        assert_eq!(
            resolve_module_map_filename(&mapping, "claude"),
            "MY-RULES.md"
        );
    }

    #[test]
    fn test_all_gitignore_entries_module_map_expands_mappings() {
        let toml = r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "."
            destination = "."
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "api-context.md"
            destination = "src/api"

            [[agents.claude.targets.modules.mappings]]
            source = "ui-context.md"
            destination = "src/ui"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        // Each mapping should produce a gitignore entry with resolved filename
        assert!(
            entries.contains(&"src/api/CLAUDE.md".to_string()),
            "Expected src/api/CLAUDE.md in {:?}",
            entries
        );
        assert!(
            entries.contains(&"src/ui/CLAUDE.md".to_string()),
            "Expected src/ui/CLAUDE.md in {:?}",
            entries
        );
        // Backup patterns too
        assert!(entries.contains(&"src/api/CLAUDE.md.bak".to_string()));
        assert!(entries.contains(&"src/ui/CLAUDE.md.bak".to_string()));
    }

    #[test]
    fn test_all_gitignore_entries_module_map_disabled_agent_skipped() {
        let toml = r#"
            [agents.claude]
            enabled = false

            [agents.claude.targets.modules]
            source = "."
            destination = "."
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "api-context.md"
            destination = "src/api"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        assert!(
            !entries.contains(&"src/api/CLAUDE.md".to_string()),
            "Disabled agent's module-map entries should not appear in gitignore"
        );
    }

    #[test]
    fn test_all_gitignore_entries_module_map_with_filename_override() {
        let toml = r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "."
            destination = "."
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "api-context.md"
            destination = "src/api"
            filename_override = "CUSTOM.md"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        assert!(
            entries.contains(&"src/api/CUSTOM.md".to_string()),
            "Override filename should appear in gitignore, got {:?}",
            entries
        );
        assert!(
            !entries.contains(&"src/api/CLAUDE.md".to_string()),
            "Convention filename should NOT appear when override is set"
        );
    }

    #[test]
    fn test_all_gitignore_entries_module_map_deduplicates_expanded_entries() {
        let toml = r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "."
            destination = "."
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "AGENTS.md"
            destination = "src/shared"

            [agents.codex]
            enabled = true

            [agents.codex.targets.main]
            source = "AGENTS.md"
            destination = "src/shared/AGENTS.md"
            type = "symlink"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let entries = config.all_gitignore_entries();

        assert_eq!(
            entries
                .iter()
                .filter(|entry| *entry == "src/shared/AGENTS.md")
                .count(),
            1
        );
    }
}
