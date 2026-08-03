//! Symbolic link creation and management
//!
//! Handles creating, updating, and removing symbolic links
//! for AI agent configuration synchronization.

use anyhow::{Context, Result};
use colored::Colorize;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::config::{Config, TargetConfig};

#[cfg(test)]
use discovery::matches_path_glob;
use discovery::matches_pattern;
#[cfg(test)]
use discovery::path_glob_match_iter;

mod apply;
mod clean;
mod discovery;
mod paths;
mod symlinks;

const COMPRESSED_AGENTS_MD_NAME: &str = "AGENTS.compact.md";

/// Result of checking an existing symlink at a destination.
enum ExistingSymlinkAction {
    /// Symlink already points to the correct target.
    AlreadyCorrect,
    /// Symlink was removed (or would be in dry-run) and needs recreation.
    Updated,
}

type NestedGlobKey = (PathBuf, String, Vec<String>);
type NestedGlobMatches = Rc<Vec<(PathBuf, PathBuf)>>;

/// Options for the sync operation
#[derive(Debug, Default)]
pub struct SyncOptions {
    /// Remove existing symlinks before creating new ones
    pub clean: bool,
    /// Show what would be done without making changes
    pub dry_run: bool,
    /// Show detailed output
    pub verbose: bool,
    /// Filter to specific agents
    pub agents: Option<Vec<String>>,
}

/// Result of a sync operation
#[derive(Debug, Default)]
pub struct SyncResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub removed: usize,
    pub errors: usize,
}

#[derive(Debug)]
struct ResolvedSource {
    path: PathBuf,
    exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymlinkContentsChildExpectation {
    pub name: String,
    pub source_path: PathBuf,
    pub expected_source_path: PathBuf,
}

/// Performs the synchronization of agent configurations
pub struct Linker {
    config: Config,
    #[allow(dead_code)]
    config_path: PathBuf,
    project_root: PathBuf,
    source_dir: PathBuf,
    path_cache: RefCell<HashMap<PathBuf, Rc<PathBuf>>>,
    compression_cache: RefCell<HashMap<PathBuf, Rc<str>>>,
    /// Cache for NestedGlob discovery results: (search_root, pattern, excludes) -> [(full_path, rel_path)]
    glob_cache: RefCell<HashMap<NestedGlobKey, NestedGlobMatches>>,
    ensured_dirs: RefCell<HashSet<PathBuf>>,
    ensured_compressed: RefCell<HashSet<PathBuf>>,
    canonical_project_root: RefCell<Option<Rc<PathBuf>>>,
}

impl Linker {
    /// Create a new linker from a configuration
    pub fn new(config: Config, config_path: PathBuf) -> Self {
        let project_root = Config::project_root(&config_path);
        let source_dir = config.source_dir(&config_path);

        Self {
            config,
            config_path,
            project_root,
            source_dir,
            path_cache: RefCell::new(HashMap::new()),
            compression_cache: RefCell::new(HashMap::new()),
            glob_cache: RefCell::new(HashMap::new()),
            ensured_dirs: RefCell::new(HashSet::new()),
            ensured_compressed: RefCell::new(HashSet::new()),
            canonical_project_root: RefCell::new(None),
        }
    }

    /// Get the project root path
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Provides access to the linker's configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// let config = linker.config();
    /// assert_eq!(config, linker.config());
    /// ```
    ///
    /// @returns The linker's configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Clears cached nested-glob discovery results after filesystem mutations.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// linker.invalidate_glob_cache();
    /// ```
    fn invalidate_glob_cache(&self) {
        self.glob_cache.borrow_mut().clear();
    }

    /// Determines which source path status checks should expect for a target.
    ///
    /// When `AGENTS.md` compression applies, an existing `AGENTS.compact.md` is
    /// preferred. If the compact file does not exist, the original source path is
    /// used when present.
    ///
    /// # Arguments
    ///
    /// * `source` - The source path to resolve.
    /// * `target` - The target configuration that determines whether compression applies.
    ///
    /// # Returns
    ///
    /// The expected source path, or `None` when the source does not exist.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let expected = linker.expected_source_path(source, target);
    /// ```
    pub fn expected_source_path(&self, source: &Path, target: &TargetConfig) -> Option<PathBuf> {
        // expected_source_path feeds status/entry_is_problematic; when should_compress_agents_md
        // applies, only return compressed_agents_md_path if it already exists.
        if self.should_compress_agents_md(source, target) {
            if source.exists() {
                let compressed = Self::compressed_agents_md_path(source);
                if compressed.exists() {
                    Some(compressed)
                } else {
                    Some(source.to_path_buf())
                }
            } else {
                None
            }
        } else if source.exists() {
            Some(source.to_path_buf())
        } else {
            None
        }
    }

    /// Determines the source entries managed by a `symlink-contents` target.
    ///
    /// Returns `None` when `source_dir` does not exist or is not a directory. Otherwise,
    /// returns matching entries sorted by name, excluding `AGENTS.compact.md` when agent
    /// compression is enabled.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # let linker: Linker = todo!();
    /// # let target: TargetConfig = todo!();
    /// let result = linker.symlink_contents_expected_children(
    ///     std::path::Path::new("config"),
    ///     &target,
    /// );
    /// assert!(result.is_ok());
    /// ```
    pub fn symlink_contents_expected_children(
        &self,
        source_dir: &Path,
        target: &TargetConfig,
    ) -> Result<Option<Vec<SymlinkContentsChildExpectation>>> {
        if !source_dir.exists() || !source_dir.is_dir() {
            return Ok(None);
        }

        let mut children = Vec::new();

        for entry in fs::read_dir(source_dir)
            .with_context(|| format!("Failed to read source directory: {}", source_dir.display()))?
        {
            let entry = entry
                .with_context(|| format!("Failed to read entry in: {}", source_dir.display()))?;
            let file_name = entry.file_name();
            let item_name = file_name.to_string_lossy();

            if let Some(pat) = target.pattern.as_deref()
                && !matches_pattern(&item_name, pat)
            {
                continue;
            }

            // Skip AGENTS.compact.md when compression is enabled to avoid false drift in status
            if self.config.compress_agents_md && item_name == "AGENTS.compact.md" {
                continue;
            }

            let source_path = entry.path();
            if let Some(expected_source_path) = self.expected_source_path(&source_path, target) {
                children.push(SymlinkContentsChildExpectation {
                    name: item_name.into_owned(),
                    source_path,
                    expected_source_path,
                });
            }
        }

        children.sort_by(|left, right| left.name.cmp(&right.name));

        Ok(Some(children))
    }

    /// Ensures that a directory is available for synchronization.
    ///
    /// In dry-run mode, reports missing directories without creating them. Repeated
    /// requests for the same directory avoid redundant filesystem operations.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// linker.ensure_directory(Path::new("generated"), &options)?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    fn ensure_directory(&self, dir: &Path, options: &SyncOptions) -> Result<()> {
        let mut ensured = self.ensured_dirs.borrow_mut();
        if !ensured.contains(dir) {
            if !dir.exists() {
                if options.dry_run {
                    if options.verbose {
                        println!("  {} Would create directory: {}", "→".cyan(), dir.display());
                    }
                } else {
                    self.revalidate_path(dir)?;
                    fs::create_dir_all(dir).with_context(|| {
                        format!("Failed to create directory: {}", dir.display())
                    })?;
                    if options.verbose {
                        println!("  {} Created directory: {}", "✔".green(), dir.display());
                    }
                }
            }
            ensured.insert(dir.to_path_buf());
        }
        Ok(())
    }

    /// Sync MCP configurations for enabled agents
    ///
    /// # Arguments
    /// * `dry_run` - Show what would be done without making changes
    /// * `agents_filter` - Optional filter for specific agents (from CLI --agents or default_agents)
    pub fn sync_mcp(
        &self,
        dry_run: bool,
        agents_filter: Option<&Vec<String>>,
    ) -> Result<crate::mcp::McpSyncResult> {
        use crate::mcp::McpGenerator;

        if !self.config.mcp.enabled {
            return Ok(crate::mcp::McpSyncResult::default());
        }

        if self.config.mcp_servers.is_empty() {
            return Ok(crate::mcp::McpSyncResult::default());
        }

        // Determine which agents should receive MCP configs
        // Only generate MCP configs for agents explicitly configured AND enabled
        let enabled_agents = McpGenerator::get_enabled_agents_from_config(&self.config.agents);

        // If no agents are explicitly configured for MCP, return early
        if enabled_agents.is_empty() {
            return Ok(crate::mcp::McpSyncResult::default());
        }

        // Apply agent filtering (from CLI --agents or default_agents config)
        let filtered_agents: Vec<_> = if let Some(filter) = agents_filter {
            enabled_agents
                .into_iter()
                .filter(|agent| filter.iter().any(|f| mcp_agent_matches_filter(*agent, f)))
                .collect()
        } else if !self.config.default_agents.is_empty() {
            // Apply default_agents filtering
            enabled_agents
                .into_iter()
                .filter(|agent| {
                    self.config
                        .default_agents
                        .iter()
                        .any(|f| mcp_agent_matches_filter(*agent, f))
                })
                .collect()
        } else {
            enabled_agents
        };

        if filtered_agents.is_empty() {
            return Ok(crate::mcp::McpSyncResult::default());
        }

        let generator = McpGenerator::new(
            self.config.mcp_servers.clone(),
            self.config.mcp.merge_strategy,
        );
        generator.generate_all(&self.project_root, &filtered_agents, dry_run)
    }
}

/// Match MCP agents against CLI/default filter values.
/// Supports canonical IDs (e.g. "codex") and aliases (e.g. "codex-cli"),
/// while preserving legacy substring matching for unknown/custom filters.
fn mcp_agent_matches_filter(agent: crate::mcp::McpAgent, filter: &str) -> bool {
    crate::agent_ids::mcp_filter_matches(agent.id(), filter)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    // ==========================================================================
    // PATTERN MATCHING TESTS
    // ==========================================================================

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("test.md", "*.md"));
        assert!(matches_pattern("test.md", "test.*"));
        assert!(matches_pattern("test.md", "test.md"));
        assert!(matches_pattern("test.md", "????.md"));
        assert!(!matches_pattern("test.md", "*.txt"));
        assert!(!matches_pattern("test.md", "foo.*"));
        assert!(matches_pattern("a", "*"));
        assert!(matches_pattern("", "*"));
        assert!(!matches_pattern("", "?"));
    }

    #[test]
    fn test_pattern_matching_asterisk_middle() {
        assert!(matches_pattern("test-file.md", "test-*.md"));
        assert!(matches_pattern("test-.md", "test-*.md"));
        assert!(matches_pattern("test-abc-xyz.md", "test-*.md"));
        assert!(!matches_pattern("test.md", "test-*.md"));
    }

    #[test]
    fn test_pattern_matching_multiple_asterisks() {
        assert!(matches_pattern("abc.def.txt", "*.*.*"));
        assert!(matches_pattern("a.b.c", "*.*.*"));
        assert!(!matches_pattern("a.b", "*.*.*"));
    }

    #[test]
    fn test_pattern_matching_question_marks() {
        assert!(matches_pattern("abc", "???"));
        assert!(!matches_pattern("ab", "???"));
        assert!(!matches_pattern("abcd", "???"));
        assert!(matches_pattern("a1c", "a?c"));
    }

    #[test]
    fn test_pattern_matching_mixed() {
        assert!(matches_pattern("file123.txt", "file???.txt"));
        assert!(matches_pattern("file123.txt", "file*.txt"));
        assert!(matches_pattern("file123.txt", "*123*"));
        assert!(matches_pattern("a", "?"));
    }

    #[test]
    fn test_pattern_matching_edge_cases() {
        assert!(matches_pattern("", ""));
        assert!(!matches_pattern("a", ""));
        assert!(!matches_pattern("", "a"));
        assert!(matches_pattern("*", "*"));
        assert!(matches_pattern("?", "?"));
    }

    // ==========================================================================
    // PATH GLOB MATCHING TESTS
    // ==========================================================================

    #[test]
    fn test_path_glob_double_star_matches_nested() {
        // **/AGENTS.md should match at any depth
        assert!(matches_path_glob("AGENTS.md", "**/AGENTS.md"));
        assert!(matches_path_glob("foo/AGENTS.md", "**/AGENTS.md"));
        assert!(matches_path_glob("foo/bar/AGENTS.md", "**/AGENTS.md"));
        assert!(matches_path_glob("a/b/c/AGENTS.md", "**/AGENTS.md"));
    }

    #[test]
    fn test_path_glob_double_star_does_not_match_wrong_name() {
        assert!(!matches_path_glob("foo/OTHER.md", "**/AGENTS.md"));
        assert!(!matches_path_glob("AGENTS.txt", "**/AGENTS.md"));
    }

    #[test]
    fn test_path_glob_single_star_does_not_cross_separator() {
        assert!(matches_path_glob("foo/AGENTS.md", "*/AGENTS.md"));
        assert!(!matches_path_glob("foo/bar/AGENTS.md", "*/AGENTS.md"));
    }

    #[test]
    fn test_path_glob_exact_match() {
        assert!(matches_path_glob("clients/AGENTS.md", "clients/AGENTS.md"));
        assert!(!matches_path_glob("other/AGENTS.md", "clients/AGENTS.md"));
    }

    #[test]
    fn test_path_glob_double_star_in_middle() {
        assert!(matches_path_glob(
            "clients/agent-runtime/AGENTS.md",
            "clients/**/AGENTS.md"
        ));
        assert!(matches_path_glob(
            "clients/AGENTS.md",
            "clients/**/AGENTS.md"
        ));
        assert!(!matches_path_glob(
            "other/agent-runtime/AGENTS.md",
            "clients/**/AGENTS.md"
        ));
    }

    #[test]
    fn test_path_glob_exclusion_patterns() {
        assert!(matches_path_glob(
            "node_modules/foo/bar.md",
            "node_modules/**"
        ));
        assert!(matches_path_glob("target/debug/foo.md", "**/target/**"));
        assert!(!matches_path_glob("src/main.rs", "node_modules/**"));
    }

    // ==========================================================================
    // DESTINATION TEMPLATE TESTS
    // ==========================================================================

    #[test]
    fn test_expand_destination_template_root_file() {
        let rel = Path::new("AGENTS.md");
        // {relative_path} for a root-level file is "." to avoid a leading slash
        assert_eq!(
            Linker::expand_destination_template("{relative_path}/{file_name}", rel),
            "./AGENTS.md"
        );
        assert_eq!(
            Linker::expand_destination_template("{file_name}", rel),
            "AGENTS.md"
        );
        assert_eq!(Linker::expand_destination_template("{stem}", rel), "AGENTS");
        assert_eq!(Linker::expand_destination_template("{ext}", rel), "md");
    }

    #[test]
    fn test_expand_destination_template_nested_file() {
        let rel = Path::new("clients/agent-runtime/AGENTS.md");
        assert_eq!(
            Linker::expand_destination_template("{relative_path}/CLAUDE.md", rel),
            "clients/agent-runtime/CLAUDE.md"
        );
        assert_eq!(
            Linker::expand_destination_template("{relative_path}/{file_name}", rel),
            "clients/agent-runtime/AGENTS.md"
        );
    }

    fn create_test_config() -> Config {
        let toml = r#"
            source_dir = "."
            
            [agents.test]
            enabled = true
            description = "Test Agent"
            
            [agents.test.targets.main]
            source = "AGENTS.md"
            destination = "TEST.md"
            type = "symlink"
        "#;
        toml::from_str(toml).unwrap()
    }

    #[test]
    fn test_linker_new() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        fs::write(&config_path, "").unwrap();

        let config = create_test_config();
        let linker = Linker::new(config, config_path.clone());

        assert_eq!(linker.project_root(), temp_dir.path());
    }

    #[test]
    fn test_linker_project_root_accessor() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("agentsync.toml");
        fs::write(&config_path, "").unwrap();

        let config = create_test_config();
        let linker = Linker::new(config, config_path);

        assert_eq!(linker.project_root(), temp_dir.path());
    }

    #[test]
    fn test_linker_config_accessor() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("agentsync.toml");
        fs::write(&config_path, "").unwrap();

        let config = create_test_config();
        let linker = Linker::new(config, config_path);

        assert!(linker.config().agents.contains_key("test"));
    }

    #[test]
    fn test_ensure_safe_destination_rejects_empty_and_parent_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        fs::write(&config_path, "").unwrap();

        let linker = Linker::new(create_test_config(), config_path);

        assert!(linker.ensure_safe_destination("").is_err());
        assert!(linker.ensure_safe_destination(".").is_err());
        assert!(linker.ensure_safe_destination("../escape.md").is_err());
    }

    #[test]
    fn test_ensure_safe_destination_rejects_absolute_path_and_accepts_valid_relative() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        fs::write(&config_path, "").unwrap();

        let linker = Linker::new(create_test_config(), config_path);

        let absolute = temp_dir.path().join("absolute.md");
        assert!(
            linker
                .ensure_safe_destination(&absolute.display().to_string())
                .is_err()
        );

        let valid = linker.ensure_safe_destination("nested/output.md").unwrap();
        assert_eq!(valid, temp_dir.path().join("nested/output.md"));
    }

    #[test]
    #[cfg(unix)]
    fn test_ensure_safe_destination_rejects_symlink_ancestor_escape() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let escaped_dir = TempDir::new_in(temp_dir.path().parent().unwrap()).unwrap();
        fs::create_dir_all(&agents_dir).unwrap();
        symlink(escaped_dir.path(), temp_dir.path().join("escape-link")).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        fs::write(&config_path, "").unwrap();

        let linker = Linker::new(create_test_config(), config_path);

        assert!(
            linker
                .ensure_safe_destination("escape-link/linked.md")
                .is_err()
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_ensure_safe_destination_uses_fresh_canonicalization() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let safe_dir = temp_dir.path().join("safe-link-target");
        let escaped_dir = TempDir::new_in(temp_dir.path().parent().unwrap()).unwrap();
        fs::create_dir_all(&agents_dir).unwrap();
        fs::create_dir_all(&safe_dir).unwrap();

        let link_path = temp_dir.path().join("dynamic-link");
        symlink(&safe_dir, &link_path).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        fs::write(&config_path, "").unwrap();

        let linker = Linker::new(create_test_config(), config_path);

        assert!(
            linker
                .ensure_safe_destination("dynamic-link/linked.md")
                .is_ok()
        );

        fs::remove_file(&link_path).unwrap();
        symlink(escaped_dir.path(), &link_path).unwrap();

        assert!(
            linker
                .ensure_safe_destination("dynamic-link/linked.md")
                .is_err()
        );
    }

    // ==========================================================================
    // SYMLINK CREATION TESTS
    // ==========================================================================

    #[test]
    #[cfg(unix)]
    fn test_sync_creates_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        // Create source file
        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        // Create config
        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            
            [agents.test]
            enabled = true
            
            [agents.test.targets.main]
            source = "AGENTS.md"
            destination = "TEST.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let options = SyncOptions::default();
        let result = linker.sync(&options).unwrap();

        assert_eq!(result.created, 1);

        // Verify symlink was created
        let dest = temp_dir.path().join("TEST.md");
        assert!(dest.is_symlink());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_compresses_agents_md_when_enabled() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let source_file = agents_dir.join("AGENTS.md");
        fs::write(
            &source_file,
            "## Title  \n\n\nSome   text\twith   spacing.\n```rust\nfn  main() {}\n```\n",
        )
        .unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            compress_agents_md = true

            [agents.test]
            enabled = true

            [agents.test.targets.main]
            source = "AGENTS.md"
            destination = "TEST.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync(&SyncOptions::default()).unwrap();

        assert_eq!(result.created, 1);

        let dest = temp_dir.path().join("TEST.md");
        assert!(dest.is_symlink());

        let compressed = agents_dir.join("AGENTS.compact.md");
        assert!(compressed.exists());

        let link_target = fs::read_link(&dest).unwrap();
        let linked = dest.parent().unwrap().join(link_target);
        let linked_canon = fs::canonicalize(linked).unwrap();
        let compressed_canon = fs::canonicalize(compressed).unwrap();
        assert_eq!(linked_canon, compressed_canon);

        let compressed_content = fs::read_to_string(agents_dir.join("AGENTS.compact.md")).unwrap();
        assert!(compressed_content.contains("Some text with spacing."));
        assert!(compressed_content.contains("fn  main() {}"));
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_dry_run_does_not_create_files() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        // Create source file
        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.test]
            enabled = true
            [agents.test.targets.main]
            source = "AGENTS.md"
            destination = "TEST.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let options = SyncOptions {
            dry_run: true,
            ..Default::default()
        };
        linker.sync(&options).unwrap();

        // Symlink should NOT exist
        let dest = temp_dir.path().join("TEST.md");
        assert!(!dest.exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_skips_disabled_agents() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.disabled]
            enabled = false
            [agents.disabled.targets.main]
            source = "AGENTS.md"
            destination = "DISABLED.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let options = SyncOptions::default();
        let result = linker.sync(&options).unwrap();

        assert_eq!(result.created, 0);
        assert!(!temp_dir.path().join("DISABLED.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_filters_by_agent_name() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            
            [agents.claude]
            enabled = true
            [agents.claude.targets.main]
            source = "AGENTS.md"
            destination = "CLAUDE.md"
            type = "symlink"
            
            [agents.copilot]
            enabled = true
            [agents.copilot.targets.main]
            source = "AGENTS.md"
            destination = "COPILOT.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        // Only sync claude
        let options = SyncOptions {
            agents: Some(vec!["claude".to_string()]),
            ..Default::default()
        };
        let result = linker.sync(&options).unwrap();

        assert_eq!(result.created, 1);
        assert!(temp_dir.path().join("CLAUDE.md").exists());
        assert!(!temp_dir.path().join("COPILOT.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_filters_by_agent_name_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            
            [agents.GitHub-Copilot]
            enabled = true
            [agents.GitHub-Copilot.targets.main]
            source = "AGENTS.md"
            destination = "COPILOT.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        // Should match case-insensitively
        let options = SyncOptions {
            agents: Some(vec!["copilot".to_string()]),
            ..Default::default()
        };
        let result = linker.sync(&options).unwrap();

        assert_eq!(result.created, 1);
        assert!(temp_dir.path().join("COPILOT.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_uses_default_agents_when_no_cli_filter() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            default_agents = ["claude", "copilot"]
            
            [agents.claude]
            enabled = true
            [agents.claude.targets.main]
            source = "AGENTS.md"
            destination = "CLAUDE.md"
            type = "symlink"
            
            [agents.copilot]
            enabled = true
            [agents.copilot.targets.main]
            source = "AGENTS.md"
            destination = "COPILOT.md"
            type = "symlink"
            
            [agents.cursor]
            enabled = true
            [agents.cursor.targets.main]
            source = "AGENTS.md"
            destination = "CURSOR.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        // No CLI filter - should use default_agents
        let options = SyncOptions::default();
        let result = linker.sync(&options).unwrap();

        assert_eq!(result.created, 2);
        assert!(temp_dir.path().join("CLAUDE.md").exists());
        assert!(temp_dir.path().join("COPILOT.md").exists());
        assert!(!temp_dir.path().join("CURSOR.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_cli_agents_overrides_default_agents() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            default_agents = ["claude"]
            
            [agents.claude]
            enabled = true
            [agents.claude.targets.main]
            source = "AGENTS.md"
            destination = "CLAUDE.md"
            type = "symlink"
            
            [agents.copilot]
            enabled = true
            [agents.copilot.targets.main]
            source = "AGENTS.md"
            destination = "COPILOT.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        // CLI filter should override default_agents
        let options = SyncOptions {
            agents: Some(vec!["copilot".to_string()]),
            ..Default::default()
        };
        let result = linker.sync(&options).unwrap();

        assert_eq!(result.created, 1);
        assert!(!temp_dir.path().join("CLAUDE.md").exists());
        assert!(temp_dir.path().join("COPILOT.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_default_agents_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            default_agents = ["CLAUDE", "COPILOT"]
            
            [agents.claude-code]
            enabled = true
            [agents.claude-code.targets.main]
            source = "AGENTS.md"
            destination = "CLAUDE.md"
            type = "symlink"
            
            [agents.GitHub-Copilot]
            enabled = true
            [agents.GitHub-Copilot.targets.main]
            source = "AGENTS.md"
            destination = "COPILOT.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        // Should match case-insensitively using default_agents
        let options = SyncOptions::default();
        let result = linker.sync(&options).unwrap();

        assert_eq!(result.created, 2);
        assert!(temp_dir.path().join("CLAUDE.md").exists());
        assert!(temp_dir.path().join("COPILOT.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_cli_filter_supports_aliases() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."

            [agents.codex]
            enabled = true
            [agents.codex.targets.main]
            source = "AGENTS.md"
            destination = "CODEX.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let options = SyncOptions {
            agents: Some(vec!["codex-cli".to_string()]),
            ..Default::default()
        };
        let result = linker.sync(&options).unwrap();

        assert_eq!(result.created, 1);
        assert!(temp_dir.path().join("CODEX.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_default_agents_support_aliases() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            default_agents = ["codex-cli"]

            [agents.codex]
            enabled = true
            [agents.codex.targets.main]
            source = "AGENTS.md"
            destination = "CODEX.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync(&SyncOptions::default()).unwrap();

        assert_eq!(result.created, 1);
        assert!(temp_dir.path().join("CODEX.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_all_enabled_when_no_default_agents_and_no_cli() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            
            [agents.claude]
            enabled = true
            [agents.claude.targets.main]
            source = "AGENTS.md"
            destination = "CLAUDE.md"
            type = "symlink"
            
            [agents.copilot]
            enabled = true
            [agents.copilot.targets.main]
            source = "AGENTS.md"
            destination = "COPILOT.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        // No default_agents and no CLI filter - should process all enabled
        let options = SyncOptions::default();
        let result = linker.sync(&options).unwrap();

        assert_eq!(result.created, 2);
        assert!(temp_dir.path().join("CLAUDE.md").exists());
        assert!(temp_dir.path().join("COPILOT.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_skips_missing_source() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        // DON'T create source file
        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.test]
            enabled = true
            [agents.test.targets.main]
            source = "NONEXISTENT.md"
            destination = "TEST.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let options = SyncOptions::default();
        let result = linker.sync(&options).unwrap();

        assert_eq!(result.skipped, 1);
        assert_eq!(result.created, 0);
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_symlink_contents_skips_circular_destination() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let skills_dir = agents_dir.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        // Create some skill directories
        let skill1 = skills_dir.join("skill1");
        let skill2 = skills_dir.join("skill2");
        fs::create_dir_all(&skill1).unwrap();
        fs::create_dir_all(&skill2).unwrap();
        fs::write(skill1.join("SKILL.md"), "# Skill 1").unwrap();
        fs::write(skill2.join("SKILL.md"), "# Skill 2").unwrap();

        // Create destination in project root as a symlink pointing to .agents/skills
        let dest_skills = temp_dir.path().join("dest_skills");

        // Create destination as a symlink pointing back to source
        #[cfg(unix)]
        std::os::unix::fs::symlink(".agents/skills", &dest_skills).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.opencode]
            enabled = true
            [agents.opencode.targets.skills]
            source = "skills"
            destination = "dest_skills"
            type = "symlink-contents"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let options = SyncOptions::default();
        let result = linker.sync(&options).unwrap();

        // Should skip the target because destination is a symlink to source
        assert_eq!(result.skipped, 1);
        assert_eq!(result.created, 0);

        // Verify that source directories are still intact (not converted to symlinks)
        assert!(skill1.is_dir());
        assert!(skill2.is_dir());
        assert!(!skill1.is_symlink());
        assert!(!skill2.is_symlink());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_creates_parent_directories() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.test]
            enabled = true
            [agents.test.targets.main]
            source = "AGENTS.md"
            destination = "deep/nested/dir/TEST.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let options = SyncOptions::default();
        linker.sync(&options).unwrap();

        let dest = temp_dir.path().join("deep/nested/dir/TEST.md");
        assert!(dest.is_symlink());
    }

    // ==========================================================================
    // SYMLINK CONTENTS TESTS
    // ==========================================================================

    #[test]
    #[cfg(unix)]
    fn test_sync_symlink_contents() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let skills_dir = agents_dir.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        // Create multiple source files
        fs::write(skills_dir.join("skill1.md"), "# Skill 1").unwrap();
        fs::write(skills_dir.join("skill2.md"), "# Skill 2").unwrap();
        fs::write(skills_dir.join("readme.txt"), "Not a skill").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.test]
            enabled = true
            [agents.test.targets.skills]
            source = "skills"
            destination = "output_skills"
            type = "symlink-contents"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let options = SyncOptions::default();
        let result = linker.sync(&options).unwrap();

        assert_eq!(result.created, 3);

        let output_dir = temp_dir.path().join("output_skills");
        assert!(output_dir.join("skill1.md").is_symlink());
        assert!(output_dir.join("skill2.md").is_symlink());
        assert!(output_dir.join("readme.txt").is_symlink());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_symlink_contents_with_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let skills_dir = agents_dir.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        fs::write(skills_dir.join("skill1.md"), "# Skill 1").unwrap();
        fs::write(skills_dir.join("skill2.md"), "# Skill 2").unwrap();
        fs::write(skills_dir.join("readme.txt"), "Not included").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.test]
            enabled = true
            [agents.test.targets.skills]
            source = "skills"
            destination = "output_skills"
            type = "symlink-contents"
            pattern = "*.md"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let options = SyncOptions::default();
        let result = linker.sync(&options).unwrap();

        // Only .md files should be linked
        assert_eq!(result.created, 2);

        let output_dir = temp_dir.path().join("output_skills");
        assert!(output_dir.join("skill1.md").is_symlink());
        assert!(output_dir.join("skill2.md").is_symlink());
        assert!(!output_dir.join("readme.txt").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_symlink_contents_compresses_agents_md() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let instructions_dir = agents_dir.join("instructions");
        fs::create_dir_all(&instructions_dir).unwrap();

        fs::write(
            instructions_dir.join("AGENTS.md"),
            "## Title  \n\nSome   text\n```txt\n  keep\n```\n",
        )
        .unwrap();
        fs::write(instructions_dir.join("OTHER.md"), "# Other").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            compress_agents_md = true

            [agents.test]
            enabled = true

            [agents.test.targets.main]
            source = "instructions"
            destination = "output"
            type = "symlink-contents"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        linker.sync(&SyncOptions::default()).unwrap();

        let compressed = instructions_dir.join("AGENTS.compact.md");
        assert!(compressed.exists());

        let dest = temp_dir.path().join("output").join("AGENTS.md");
        assert!(dest.is_symlink());

        let link_target = fs::read_link(&dest).unwrap();
        let linked = dest.parent().unwrap().join(link_target);
        let linked_canon = fs::canonicalize(linked).unwrap();
        let compressed_canon = fs::canonicalize(compressed).unwrap();
        assert_eq!(linked_canon, compressed_canon);
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_symlink_directory_for_skills() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let skills_dir = agents_dir.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        // Create skill subdirectories with SKILL.md files
        let debugging_dir = skills_dir.join("debugging");
        fs::create_dir_all(&debugging_dir).unwrap();
        fs::write(debugging_dir.join("SKILL.md"), "# Debugging skill").unwrap();

        let testing_dir = skills_dir.join("testing");
        fs::create_dir_all(&testing_dir).unwrap();
        fs::write(testing_dir.join("SKILL.md"), "# Testing skill").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.test]
            enabled = true
            [agents.test.targets.skills]
            source = "skills"
            destination = "output_skills"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let options = SyncOptions::default();
        let result = linker.sync(&options).unwrap();

        assert_eq!(result.created, 1);

        // Destination should be a symlink (not a real directory)
        let dest = temp_dir.path().join("output_skills");
        assert!(dest.is_symlink(), "Expected output_skills to be a symlink");

        // Symlink should resolve to the source skills directory
        let target = fs::read_link(&dest).unwrap();
        let target_str = target.to_string_lossy();
        assert!(
            target_str.contains("skills"),
            "Expected symlink to point to skills dir, got '{target_str}'"
        );

        // Skill subdirectories should be accessible through the symlink
        assert!(dest.join("debugging").exists());
        assert!(dest.join("debugging/SKILL.md").exists());
        assert!(dest.join("testing").exists());
        assert!(dest.join("testing/SKILL.md").exists());

        // Verify contents are readable
        let content = fs::read_to_string(dest.join("debugging/SKILL.md")).unwrap();
        assert_eq!(content, "# Debugging skill");
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_symlink_directory_upgrades_existing_dir() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let skills_dir = agents_dir.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        // Create skill subdirectories
        let debugging_dir = skills_dir.join("debugging");
        fs::create_dir_all(&debugging_dir).unwrap();
        fs::write(debugging_dir.join("SKILL.md"), "# Debugging skill").unwrap();

        // Pre-create output_skills as a REAL directory with old files
        // (simulates the old symlink-contents layout)
        let output_skills = temp_dir.path().join("output_skills");
        fs::create_dir_all(&output_skills).unwrap();
        fs::write(output_skills.join("old-file.txt"), "old content").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.test]
            enabled = true
            [agents.test.targets.skills]
            source = "skills"
            destination = "output_skills"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync(&SyncOptions::default()).unwrap();

        // The existing dir was backed up and replaced
        assert!(result.updated >= 1);

        let backup_path = temp_dir.path().join("output_skills.bak");
        assert!(
            backup_path.exists(),
            "Expected backup directory at {}",
            backup_path.display()
        );

        // The backup contains the old files
        assert!(
            backup_path.join("old-file.txt").exists(),
            "Backup should contain old-file.txt"
        );
        let backup_content = fs::read_to_string(backup_path.join("old-file.txt")).unwrap();
        assert_eq!(backup_content, "old content");

        // output_skills is now a symlink
        let dest = temp_dir.path().join("output_skills");
        assert!(dest.is_symlink(), "Expected output_skills to be a symlink");

        // Skill subdirectories are accessible through the symlink
        assert!(dest.join("debugging").exists());
        assert!(dest.join("debugging/SKILL.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_symlink_directory_replaces_existing_backup() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let skills_dir = agents_dir.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let debugging_dir = skills_dir.join("debugging");
        fs::create_dir_all(&debugging_dir).unwrap();
        fs::write(debugging_dir.join("SKILL.md"), "# Debugging skill").unwrap();

        let output_skills = temp_dir.path().join("output_skills");
        fs::create_dir_all(&output_skills).unwrap();
        fs::write(output_skills.join("current-file.txt"), "current content").unwrap();

        let existing_backup = temp_dir.path().join("output_skills.bak");
        fs::create_dir_all(&existing_backup).unwrap();
        fs::write(existing_backup.join("stale-file.txt"), "stale content").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.test]
            enabled = true
            [agents.test.targets.skills]
            source = "skills"
            destination = "output_skills"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        linker.sync(&SyncOptions::default()).unwrap();

        assert!(
            existing_backup.exists(),
            "Expected backup directory to exist"
        );
        assert!(
            existing_backup.join("current-file.txt").exists(),
            "Expected existing backup to be replaced with the latest content"
        );
        assert!(
            !existing_backup.join("stale-file.txt").exists(),
            "Expected stale backup content to be removed"
        );
    }

    // ==========================================================================
    // CLEAN TESTS
    // ==========================================================================

    #[test]
    #[cfg(unix)]
    fn test_clean_removes_symlinks() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.test]
            enabled = true
            [agents.test.targets.main]
            source = "AGENTS.md"
            destination = "TEST.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path.clone());

        // First sync to create symlinks
        linker.sync(&SyncOptions::default()).unwrap();
        assert!(temp_dir.path().join("TEST.md").is_symlink());

        // Now clean
        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);
        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 1);
        assert!(!temp_dir.path().join("TEST.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_clean_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.test]
            enabled = true
            [agents.test.targets.main]
            source = "AGENTS.md"
            destination = "TEST.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path.clone());

        // First sync
        linker.sync(&SyncOptions::default()).unwrap();

        // Clean with dry_run
        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);
        let options = SyncOptions {
            dry_run: true,
            ..Default::default()
        };
        let result = linker.clean(&options).unwrap();

        assert_eq!(result.removed, 1);
        // Symlink should STILL exist
        assert!(temp_dir.path().join("TEST.md").is_symlink());
    }

    #[test]
    #[cfg(unix)]
    fn test_clean_symlink_contents() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let skills_dir = agents_dir.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        fs::write(skills_dir.join("skill1.md"), "# Skill 1").unwrap();
        fs::write(skills_dir.join("skill2.md"), "# Skill 2").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.test]
            enabled = true
            [agents.test.targets.skills]
            source = "skills"
            destination = "output_skills"
            type = "symlink-contents"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path.clone());

        // First sync
        linker.sync(&SyncOptions::default()).unwrap();

        // Clean
        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);
        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 2);
    }

    // ==========================================================================
    // UPDATE/REPLACE TESTS
    // ==========================================================================

    #[test]
    #[cfg(unix)]
    fn test_sync_updates_existing_symlink_with_different_target() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        // Create two source files
        let source1 = agents_dir.join("source1.md");
        let source2 = agents_dir.join("source2.md");
        fs::write(&source1, "# Source 1").unwrap();
        fs::write(&source2, "# Source 2").unwrap();

        let dest = temp_dir.path().join("TEST.md");

        // Create initial symlink to source1
        std::os::unix::fs::symlink(&source1, &dest).unwrap();

        // Config points to source2
        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.test]
            enabled = true
            [agents.test.targets.main]
            source = "source2.md"
            destination = "TEST.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync(&SyncOptions::default()).unwrap();

        assert_eq!(result.updated, 1);
        assert_eq!(result.created, 0);

        // Symlink should now point to source2
        let target = fs::read_link(&dest).unwrap();
        assert!(target.to_string_lossy().contains("source2.md"));
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_skips_already_correct_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "# Test").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            [agents.test]
            enabled = true
            [agents.test.targets.main]
            source = "AGENTS.md"
            destination = "TEST.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path.clone());

        // First sync
        let result1 = linker.sync(&SyncOptions::default()).unwrap();
        assert_eq!(result1.created, 1);

        // Second sync should skip
        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);
        let result2 = linker.sync(&SyncOptions::default()).unwrap();

        assert_eq!(result2.created, 0);
        assert_eq!(result2.updated, 0);
        assert_eq!(result2.skipped, 1);
    }

    // ==========================================================================
    // SYNC OPTIONS TESTS
    // ==========================================================================

    #[test]
    fn test_sync_options_default() {
        let options = SyncOptions::default();

        assert!(!options.clean);
        assert!(!options.dry_run);
        assert!(!options.verbose);
        assert!(options.agents.is_none());
    }

    // ==========================================================================
    // SYNC RESULT TESTS
    // ==========================================================================

    #[test]
    fn test_sync_result_default() {
        let result = SyncResult::default();

        assert_eq!(result.created, 0);
        assert_eq!(result.updated, 0);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.removed, 0);
        assert_eq!(result.errors, 0);
    }

    // ==========================================================================
    // MCP SYNC TESTS
    // ==========================================================================

    #[test]
    fn test_sync_mcp_disabled_returns_empty() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            
            [mcp]
            enabled = false
            
            [mcp_servers.test]
            command = "test"
            
            [agents.test]
            enabled = true
            [agents.test.targets.main]
            source = "AGENTS.md"
            destination = "TEST.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync_mcp(false, None).unwrap();

        // Should return empty result when MCP is disabled
        assert_eq!(result.created, 0);
        assert_eq!(result.updated, 0);
    }

    #[test]
    fn test_sync_mcp_no_servers_returns_empty() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            
            [mcp]
            enabled = true
            
            [agents.test]
            enabled = true
            [agents.test.targets.main]
            source = "AGENTS.md"
            destination = "TEST.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync_mcp(false, None).unwrap();

        // Should return empty when no MCP servers defined
        assert_eq!(result.created, 0);
    }

    #[test]
    fn test_sync_mcp_creates_config_files() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            
            [mcp]
            enabled = true
            
            [mcp_servers.filesystem]
            command = "npx"
            args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
            
            [agents.claude]
            enabled = true
            [agents.claude.targets.main]
            source = "AGENTS.md"
            destination = "CLAUDE.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync_mcp(false, None).unwrap();

        // Should create MCP config for Claude
        assert!(result.created > 0);
        let mcp_config_path = temp_dir.path().join(".mcp.json");
        assert!(mcp_config_path.exists());

        // Verify content
        let content = fs::read_to_string(&mcp_config_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        let servers = parsed.get("mcpServers").expect("mcpServers key missing");
        let filesystem = servers
            .get("filesystem")
            .expect("filesystem server missing");

        assert_eq!(filesystem.get("command").unwrap().as_str().unwrap(), "npx");

        let args = filesystem.get("args").unwrap().as_array().unwrap();
        assert_eq!(args.len(), 3);
        assert_eq!(args[0].as_str().unwrap(), "-y");
        assert_eq!(
            args[1].as_str().unwrap(),
            "@modelcontextprotocol/server-filesystem"
        );
        assert_eq!(args[2].as_str().unwrap(), ".");
    }

    #[test]
    fn test_sync_mcp_creates_codex_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."

            [mcp]
            enabled = true

            [mcp_servers.filesystem]
            command = "npx"
            args = ["-y", "@modelcontextprotocol/server-filesystem", "."]

            [agents.codex]
            enabled = true
            [agents.codex.targets.main]
            source = "AGENTS.md"
            destination = "AGENTS.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync_mcp(false, None).unwrap();

        // Should create MCP config for Codex
        assert_eq!(result.created, 1);
        assert_eq!(result.updated, 0);

        let codex_config_path = temp_dir.path().join(".codex/config.toml");
        assert!(codex_config_path.exists());

        // Verify TOML content
        let content = fs::read_to_string(&codex_config_path).unwrap();
        let parsed: toml::Value = toml::from_str(&content).unwrap();
        let mcp_servers = parsed
            .get("mcp_servers")
            .and_then(|v| v.as_table())
            .expect("mcp_servers table missing");
        let filesystem = mcp_servers
            .get("filesystem")
            .and_then(|v| v.as_table())
            .expect("filesystem server missing");

        assert_eq!(
            filesystem
                .get("command")
                .and_then(|v| v.as_str())
                .expect("filesystem command missing"),
            "npx"
        );
        let args = filesystem
            .get("args")
            .and_then(|v| v.as_array())
            .expect("filesystem args missing");
        assert_eq!(args.len(), 3);
    }

    #[test]
    fn test_sync_mcp_only_creates_for_configured_agents() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        // Only configure claude and copilot; other MCP-capable agents should NOT get configs.
        let config_content = r#"
            source_dir = "."

            [mcp]
            enabled = true

            [mcp_servers.filesystem]
            command = "npx"
            args = ["-y", "@modelcontextprotocol/server-filesystem", "."]

            [agents.claude]
            enabled = true
            [agents.claude.targets.main]
            source = "AGENTS.md"
            destination = "CLAUDE.md"
            type = "symlink"

            [agents.copilot]
            enabled = true
            [agents.copilot.targets.main]
            source = "AGENTS.md"
            destination = "COPILOT.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync_mcp(false, None).unwrap();

        // Should create exactly 2 MCP configs (claude and copilot)
        assert_eq!(result.created, 2);
        assert_eq!(result.updated, 0);

        // Verify claude config exists
        let claude_config = temp_dir.path().join(".mcp.json");
        assert!(claude_config.exists(), "Claude MCP config should exist");

        // Verify copilot config exists (now at .vscode/mcp.json per GitHub docs)
        let copilot_config = temp_dir.path().join(".vscode/mcp.json");
        assert!(
            copilot_config.exists(),
            "Copilot MCP config should exist at .vscode/mcp.json"
        );

        // Note: VS Code shares the same config path as Copilot (.vscode/mcp.json)
        // So if Copilot is configured, the file will exist at that path

        // Verify cursor config does NOT exist (not configured)
        let cursor_config = temp_dir.path().join(".cursor/mcp.json");
        assert!(
            !cursor_config.exists(),
            "Cursor MCP config should NOT exist for unconfigured agent"
        );

        // Verify gemini config does NOT exist (not configured)
        let gemini_config = temp_dir.path().join(".gemini/settings.json");
        assert!(
            !gemini_config.exists(),
            "Gemini MCP config should NOT exist for unconfigured agent"
        );

        // Verify opencode config does NOT exist (not configured)
        let opencode_config = temp_dir.path().join("opencode.json");
        assert!(
            !opencode_config.exists(),
            "OpenCode MCP config should NOT exist for unconfigured agent"
        );

        // Verify codex config does NOT exist (not configured)
        let codex_config = temp_dir.path().join(".codex/config.toml");
        assert!(
            !codex_config.exists(),
            "Codex MCP config should NOT exist for unconfigured agent"
        );
    }

    #[test]
    fn test_sync_mcp_cli_filter_supports_aliases() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."

            [mcp]
            enabled = true

            [mcp_servers.filesystem]
            command = "npx"
            args = ["-y", "@modelcontextprotocol/server-filesystem", "."]

            [agents.codex-cli]
            enabled = true
            [agents.codex-cli.targets.main]
            source = "AGENTS.md"
            destination = "AGENTS.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let filter = vec!["codex-cli".to_string()];
        let result = linker.sync_mcp(false, Some(&filter)).unwrap();

        assert_eq!(result.created, 1);
        assert!(temp_dir.path().join(".codex/config.toml").exists());
    }

    #[test]
    fn test_sync_mcp_default_agents_support_aliases() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            default_agents = ["codex-cli"]

            [mcp]
            enabled = true

            [mcp_servers.filesystem]
            command = "npx"
            args = ["-y", "@modelcontextprotocol/server-filesystem", "."]

            [agents.codex-cli]
            enabled = true
            [agents.codex-cli.targets.main]
            source = "AGENTS.md"
            destination = "AGENTS.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync_mcp(false, None).unwrap();

        assert_eq!(result.created, 1);
        assert!(temp_dir.path().join(".codex/config.toml").exists());
    }

    #[test]
    fn test_sync_mcp_no_agents_configured_returns_empty() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        // No agents configured at all - only MCP servers
        let config_content = r#"
            source_dir = "."
            
            [mcp]
            enabled = true
            
            [mcp_servers.filesystem]
            command = "npx"
            args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync_mcp(false, None).unwrap();

        // Should return empty result when no agents are configured
        assert_eq!(result.created, 0);
        assert_eq!(result.updated, 0);
        assert_eq!(result.skipped, 0);

        // Verify no MCP configs were created
        assert!(!temp_dir.path().join(".mcp.json").exists());
        assert!(!temp_dir.path().join(".vscode/mcp.json").exists());
        assert!(!temp_dir.path().join(".cursor/mcp.json").exists());
        assert!(!temp_dir.path().join(".codex/config.toml").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_sync_resets_caches_between_runs() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        // Create initial source file
        let source_file = agents_dir.join("AGENTS.md");
        fs::write(&source_file, "initial content").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."
            compress_agents_md = true
            [agents.test]
            enabled = true
            [agents.test.targets.main]
            source = "AGENTS.md"
            destination = "TEST.md"
            type = "symlink"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        // First run
        linker.sync(&SyncOptions::default()).unwrap();
        let compressed_v1 = agents_dir.join("AGENTS.compact.md");
        let mtime_v1 = fs::metadata(&compressed_v1).unwrap().modified().unwrap();

        // Mutate filesystem: update source file
        // Sleep briefly to ensure mtime change if filesystem has low resolution
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&source_file, "updated content").unwrap();

        // Second run on SAME linker instance
        linker.sync(&SyncOptions::default()).unwrap();
        let mtime_v2 = fs::metadata(&compressed_v1).unwrap().modified().unwrap();

        // If cache was NOT cleared, compression would be skipped and mtime would match v1
        // because we check content equality before writing.
        // But since we updated the source, if cache is cleared, it re-reads, re-compresses,
        // sees content is different, and writes new file.
        assert!(
            mtime_v2 > mtime_v1,
            "Cache should have been cleared, leading to file update"
        );

        let content_v2 = fs::read_to_string(&compressed_v1).unwrap();
        assert_eq!(content_v2.trim(), "updated content");
    }

    // ==========================================================================
    // NESTED GLOB TESTS
    // ==========================================================================

    #[test]
    #[cfg(unix)]
    fn test_nested_glob_creates_symlinks_for_discovered_files() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        // Create nested AGENTS.md files
        let sub1 = temp_dir.path().join("clients").join("agent-runtime");
        let sub2 = temp_dir.path().join("modules").join("core-kmp");
        fs::create_dir_all(&sub1).unwrap();
        fs::create_dir_all(&sub2).unwrap();
        fs::write(sub1.join("AGENTS.md"), "# Rust instructions").unwrap();
        fs::write(sub2.join("AGENTS.md"), "# Kotlin instructions").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."

            [agents.claude]
            enabled = true

            [agents.claude.targets.nested]
            source = "."
            pattern = "**/AGENTS.md"
            exclude = [".agents/**"]
            destination = "{relative_path}/CLAUDE.md"
            type = "nested-glob"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync(&SyncOptions::default()).unwrap();

        assert_eq!(result.created, 2);
        assert!(
            temp_dir
                .path()
                .join("clients/agent-runtime/CLAUDE.md")
                .is_symlink()
        );
        assert!(
            temp_dir
                .path()
                .join("modules/core-kmp/CLAUDE.md")
                .is_symlink()
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_nested_glob_invalidates_cache_after_compressed_write() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        fs::write(&config_path, "source_dir = \".\"\n").unwrap();

        let search_root = temp_dir.path().join("workspace");
        let old_dir = search_root.join("old");
        let new_dir = search_root.join("new");
        fs::create_dir_all(&old_dir).unwrap();
        fs::write(old_dir.join("legacy.compact.md"), "# old").unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let dry_run_options = SyncOptions {
            dry_run: true,
            ..Default::default()
        };

        linker
            .process_nested_glob(
                &search_root,
                "**/*.compact.md",
                &[],
                "linked/{relative_path}/{file_name}",
                &dry_run_options,
            )
            .unwrap();

        fs::rename(
            old_dir.join("legacy.compact.md"),
            old_dir.join("legacy.md.bak"),
        )
        .unwrap();

        let source = temp_dir.path().join("AGENTS.md");
        fs::write(&source, "# compressed").unwrap();
        let compressed_dest = new_dir.join(COMPRESSED_AGENTS_MD_NAME);
        linker
            .write_compressed_agents_md(&source, &compressed_dest, &SyncOptions::default())
            .unwrap();

        let result = linker
            .process_nested_glob(
                &search_root,
                "**/*.compact.md",
                &[],
                "linked/{relative_path}/{file_name}",
                &SyncOptions::default(),
            )
            .unwrap();

        assert_eq!(result.created, 1);

        let new_link = temp_dir.path().join("linked/new/AGENTS.compact.md");
        let old_link = temp_dir.path().join("linked/old/legacy.compact.md");
        assert!(
            new_link.is_symlink(),
            "Expected fresh nested-glob discovery"
        );
        assert!(
            !old_link.exists(),
            "Expected removed files to be omitted from refreshed nested-glob discovery"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_nested_glob_excludes_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        // Create an AGENTS.md that should be discovered
        let sub1 = temp_dir.path().join("clients");
        fs::create_dir_all(&sub1).unwrap();
        fs::write(sub1.join("AGENTS.md"), "# Instructions").unwrap();

        // Create one inside node_modules that should be excluded
        let node_modules = temp_dir.path().join("node_modules").join("some-pkg");
        fs::create_dir_all(&node_modules).unwrap();
        fs::write(node_modules.join("AGENTS.md"), "# Should be excluded").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."

            [agents.claude]
            enabled = true

            [agents.claude.targets.nested]
            source = "."
            pattern = "**/AGENTS.md"
            exclude = [".agents/**", "node_modules/**"]
            destination = "{relative_path}/CLAUDE.md"
            type = "nested-glob"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync(&SyncOptions::default()).unwrap();

        // Only the non-excluded file should be linked
        assert_eq!(result.created, 1);
        assert!(temp_dir.path().join("clients/CLAUDE.md").is_symlink());
        assert!(
            !temp_dir
                .path()
                .join("node_modules/some-pkg/CLAUDE.md")
                .exists()
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_nested_glob_dry_run_does_not_create_files() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let sub1 = temp_dir.path().join("clients");
        fs::create_dir_all(&sub1).unwrap();
        fs::write(sub1.join("AGENTS.md"), "# Instructions").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."

            [agents.claude]
            enabled = true

            [agents.claude.targets.nested]
            source = "."
            pattern = "**/AGENTS.md"
            exclude = [".agents/**"]
            destination = "{relative_path}/CLAUDE.md"
            type = "nested-glob"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let options = SyncOptions {
            dry_run: true,
            ..Default::default()
        };
        linker.sync(&options).unwrap();

        assert!(!temp_dir.path().join("clients/CLAUDE.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_nested_glob_clean_removes_symlinks() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let sub1 = temp_dir.path().join("clients");
        fs::create_dir_all(&sub1).unwrap();
        fs::write(sub1.join("AGENTS.md"), "# Instructions").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."

            [agents.claude]
            enabled = true

            [agents.claude.targets.nested]
            source = "."
            pattern = "**/AGENTS.md"
            exclude = [".agents/**"]
            destination = "{relative_path}/CLAUDE.md"
            type = "nested-glob"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        // First sync to create symlinks
        linker.sync(&SyncOptions::default()).unwrap();
        assert!(temp_dir.path().join("clients/CLAUDE.md").is_symlink());

        // Clean should remove them
        let result = linker.clean(&SyncOptions::default()).unwrap();
        assert_eq!(result.removed, 1);
        assert!(!temp_dir.path().join("clients/CLAUDE.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_nested_glob_skips_missing_search_root() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."

            [agents.claude]
            enabled = true

            [agents.claude.targets.nested]
            source = "nonexistent-dir"
            pattern = "**/AGENTS.md"
            destination = "{relative_path}/CLAUDE.md"
            type = "nested-glob"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync(&SyncOptions::default()).unwrap();
        assert_eq!(result.skipped, 1);
        assert_eq!(result.created, 0);
    }

    #[test]
    #[cfg(unix)]
    fn test_nested_glob_sync_skips_invalid_expanded_destination() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(temp_dir.path().join("AGENTS.md"), "# Instructions").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."

            [agents.claude]
            enabled = true

            [agents.claude.targets.nested]
            source = "."
            pattern = "**/AGENTS.md"
            exclude = [".agents/**"]
            destination = "{relative_path}"
            type = "nested-glob"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync(&SyncOptions::default()).unwrap();

        assert_eq!(result.created, 0);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    #[cfg(unix)]
    fn test_nested_glob_clean_skips_invalid_expanded_destination() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(temp_dir.path().join("AGENTS.md"), "# Instructions").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."

            [agents.claude]
            enabled = true

            [agents.claude.targets.nested]
            source = "."
            pattern = "**/AGENTS.md"
            exclude = [".agents/**"]
            destination = "{relative_path}"
            type = "nested-glob"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
    }

    #[test]
    #[cfg(unix)]
    fn test_nested_glob_sync_skips_empty_expanded_destination() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::create_dir_all(temp_dir.path().join("clients")).unwrap();
        fs::write(temp_dir.path().join("clients/AGENTS"), "# Instructions").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."

            [agents.claude]
            enabled = true

            [agents.claude.targets.nested]
            source = "."
            pattern = "clients/AGENTS"
            exclude = [".agents/**"]
            destination = "{ext}"
            type = "nested-glob"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker
            .sync(&SyncOptions {
                verbose: true,
                ..Default::default()
            })
            .unwrap();

        assert_eq!(result.created, 0);
        assert_eq!(result.skipped, 1);
    }

    // =========================================================================
    // MODULE-MAP INTEGRATION TESTS
    // =========================================================================

    #[test]
    #[cfg(unix)]
    fn test_module_map_creates_symlinks() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let claude_dir = agents_dir.join("claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("api-context.md"), "# API Context").unwrap();
        fs::write(claude_dir.join("ui-context.md"), "# UI Context").unwrap();

        // Create destination directories
        fs::create_dir_all(temp_dir.path().join("src/api")).unwrap();
        fs::create_dir_all(temp_dir.path().join("src/ui")).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "claude"

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
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync(&SyncOptions::default()).unwrap();
        assert_eq!(result.created, 2);

        // Convention filename for claude = CLAUDE.md
        assert!(temp_dir.path().join("src/api/CLAUDE.md").is_symlink());
        assert!(temp_dir.path().join("src/ui/CLAUDE.md").is_symlink());
    }

    #[test]
    #[cfg(unix)]
    fn test_module_map_filename_override() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let claude_dir = agents_dir.join("claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("api-context.md"), "# API").unwrap();

        fs::create_dir_all(temp_dir.path().join("src/api")).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "claude"

            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "."
            destination = "."
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "api-context.md"
            destination = "src/api"
            filename_override = "CUSTOM-RULES.md"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync(&SyncOptions::default()).unwrap();
        assert_eq!(result.created, 1);

        // Override should be used instead of convention
        assert!(temp_dir.path().join("src/api/CUSTOM-RULES.md").is_symlink());
        assert!(!temp_dir.path().join("src/api/CLAUDE.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_module_map_unknown_agent_uses_source_basename() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let custom_dir = agents_dir.join("custom-agent");
        fs::create_dir_all(&custom_dir).unwrap();
        fs::write(custom_dir.join("rules.md"), "# Rules").unwrap();

        fs::create_dir_all(temp_dir.path().join("src/api")).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "custom-agent"

            [agents.custom-agent]
            enabled = true

            [agents.custom-agent.targets.modules]
            source = "."
            destination = "."
            type = "module-map"

            [[agents.custom-agent.targets.modules.mappings]]
            source = "rules.md"
            destination = "src/api"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync(&SyncOptions::default()).unwrap();
        assert_eq!(result.created, 1);

        // Unknown agent → fallback to source basename
        assert!(temp_dir.path().join("src/api/rules.md").is_symlink());
    }

    #[test]
    #[cfg(unix)]
    fn test_module_map_nested_convention_path_creates_intermediate_directories() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let copilot_dir = agents_dir.join("copilot");
        fs::create_dir_all(&copilot_dir).unwrap();
        fs::write(copilot_dir.join("api-context.md"), "# API").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "copilot"

            [agents.copilot]
            enabled = true

            [agents.copilot.targets.modules]
            source = "placeholder"
            destination = "placeholder"
            type = "module-map"

            [[agents.copilot.targets.modules.mappings]]
            source = "api-context.md"
            destination = "src/api"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync(&SyncOptions::default()).unwrap();
        assert_eq!(result.created, 1);
        assert!(
            temp_dir
                .path()
                .join("src/api/.github/copilot-instructions.md")
                .is_symlink()
        );
        assert!(temp_dir.path().join("src/api/.github").is_dir());
    }

    #[test]
    #[cfg(unix)]
    fn test_module_map_missing_source_skipped_and_other_mappings_continue() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let claude_dir = agents_dir.join("claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("api-context.md"), "# API").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "claude"

            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "placeholder"
            destination = "placeholder"
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "api-context.md"
            destination = "src/api"

            [[agents.claude.targets.modules.mappings]]
            source = "missing.md"
            destination = "src/missing"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync(&SyncOptions::default()).unwrap();
        assert_eq!(result.created, 1);
        assert_eq!(result.skipped, 1);
        assert!(temp_dir.path().join("src/api/CLAUDE.md").is_symlink());
        assert!(!temp_dir.path().join("src/missing/CLAUDE.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_module_map_sync_is_idempotent_when_symlink_already_matches() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let claude_dir = agents_dir.join("claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("api-context.md"), "# API").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "claude"

            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "placeholder"
            destination = "placeholder"
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "api-context.md"
            destination = "src/api"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let first = linker.sync(&SyncOptions::default()).unwrap();
        let second = linker.sync(&SyncOptions::default()).unwrap();

        assert_eq!(first.created, 1);
        assert_eq!(second.created, 0);
        assert_eq!(second.skipped, 1);
        assert!(temp_dir.path().join("src/api/CLAUDE.md").is_symlink());
    }

    #[test]
    #[cfg(unix)]
    fn test_module_map_clean_removes_symlinks() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let claude_dir = agents_dir.join("claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("api-context.md"), "# API").unwrap();

        fs::create_dir_all(temp_dir.path().join("src/api")).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "claude"

            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "."
            destination = "."
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "api-context.md"
            destination = "src/api"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        // Sync first
        linker.sync(&SyncOptions::default()).unwrap();
        assert!(temp_dir.path().join("src/api/CLAUDE.md").is_symlink());

        // Clean should remove
        let result = linker.clean(&SyncOptions::default()).unwrap();
        assert_eq!(result.removed, 1);
        assert!(!temp_dir.path().join("src/api/CLAUDE.md").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_module_map_clean_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let claude_dir = agents_dir.join("claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("api-context.md"), "# API").unwrap();

        fs::create_dir_all(temp_dir.path().join("src/api")).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "claude"

            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "."
            destination = "."
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "api-context.md"
            destination = "src/api"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        // Sync first
        linker.sync(&SyncOptions::default()).unwrap();
        assert!(temp_dir.path().join("src/api/CLAUDE.md").is_symlink());

        // Dry-run clean should NOT remove
        let result = linker
            .clean(&SyncOptions {
                dry_run: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(result.removed, 1); // counted but not removed
        assert!(temp_dir.path().join("src/api/CLAUDE.md").is_symlink());
    }

    #[test]
    #[cfg(unix)]
    fn test_module_map_clean_skips_non_symlink_files() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::create_dir_all(temp_dir.path().join("src/api")).unwrap();
        let dest = temp_dir.path().join("src/api/CLAUDE.md");
        fs::write(&dest, "not a symlink").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."

            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "placeholder"
            destination = "placeholder"
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "api-context.md"
            destination = "src/api"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.clean(&SyncOptions::default()).unwrap();
        assert_eq!(result.removed, 0);
        assert!(dest.exists());
        assert!(!dest.is_symlink());
    }

    #[test]
    #[cfg(unix)]
    fn test_module_map_sync_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let claude_dir = agents_dir.join("claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("api-context.md"), "# API").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "claude"

            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "."
            destination = "."
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "api-context.md"
            destination = "src/api"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker
            .sync(&SyncOptions {
                dry_run: true,
                ..Default::default()
            })
            .unwrap();

        // Dry run should not create symlinks on disk
        assert!(!temp_dir.path().join("src/api/CLAUDE.md").exists());
        assert!(!temp_dir.path().join("src/api").exists());
        // dry_run still counts what *would* be created (consistent with create_symlink behavior)
        assert_eq!(result.created, 1);
    }

    #[test]
    #[cfg(unix)]
    fn test_module_map_empty_mappings() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "."

            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "."
            destination = "."
            type = "module-map"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        // Should not crash with no mappings
        let result = linker.sync(&SyncOptions::default()).unwrap();
        assert_eq!(result.created, 0);
        assert_eq!(result.errors, 0);
    }

    #[test]
    #[cfg(unix)]
    fn test_module_map_sync_skips_invalid_destination() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let claude_dir = agents_dir.join("claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("api-context.md"), "# API").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "claude"

            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "."
            destination = "."
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "api-context.md"
            destination = "../escape"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker
            .sync(&SyncOptions {
                verbose: true,
                ..Default::default()
            })
            .unwrap();

        assert_eq!(result.created, 0);
        assert_eq!(result.skipped, 1);
        assert!(
            !temp_dir
                .path()
                .parent()
                .unwrap()
                .join("escape/CLAUDE.md")
                .exists()
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_module_map_clean_skips_invalid_destination() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        let claude_dir = agents_dir.join("claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("api-context.md"), "# API").unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        let config_content = r#"
            source_dir = "claude"

            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "."
            destination = "."
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "api-context.md"
            destination = "../escape"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker
            .clean(&SyncOptions {
                verbose: true,
                ..Default::default()
            })
            .unwrap();

        assert_eq!(result.removed, 0);
        assert!(
            !temp_dir
                .path()
                .parent()
                .unwrap()
                .join("escape/CLAUDE.md")
                .exists()
        );
    }

    #[test]
    fn test_path_glob_match_iter_double_star_middle() {
        // Pattern **/foo/**/bar should match a/foo/b/bar
        let pattern = ["**", "foo", "**", "bar"];
        assert!(path_glob_match_iter("a/foo/b/bar".split('/'), &pattern));
        assert!(path_glob_match_iter("x/y/foo/z/w/bar".split('/'), &pattern));
        assert!(path_glob_match_iter("foo/bar".split('/'), &pattern));
        // Non-match: missing bar
        assert!(!path_glob_match_iter("a/foo/b/baz".split('/'), &pattern));
    }

    #[test]
    fn test_path_glob_match_iter_trailing_double_star_zero_segments() {
        // Pattern foo/** should match "foo" (trailing ** matches zero segments)
        let pattern = ["foo", "**"];
        assert!(path_glob_match_iter("foo".split('/'), &pattern));
        assert!(path_glob_match_iter("foo/bar".split('/'), &pattern));
        assert!(path_glob_match_iter("foo/bar/baz".split('/'), &pattern));
    }

    #[test]
    fn test_path_glob_match_iter_non_matching() {
        let pattern = ["src", "**", "*.rs"];
        assert!(!path_glob_match_iter("lib/foo.rs".split('/'), &pattern));
        assert!(!path_glob_match_iter("src/main.go".split('/'), &pattern));

        let pattern2 = ["foo", "bar"];
        assert!(!path_glob_match_iter("foo/baz".split('/'), &pattern2));
        assert!(!path_glob_match_iter("foo".split('/'), &pattern2));
    }
}
