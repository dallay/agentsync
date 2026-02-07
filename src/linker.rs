//! Symbolic link creation and management
//!
//! Handles creating, updating, and removing symbolic links
//! for AI agent configuration synchronization.

use anyhow::{Context, Result};
use colored::Colorize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::config::{Config, SyncType, TargetConfig};

const COMPRESSED_AGENTS_MD_NAME: &str = "AGENTS.compact.md";

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

/// Performs the synchronization of agent configurations
pub struct Linker {
    config: Config,
    #[allow(dead_code)]
    config_path: PathBuf,
    project_root: PathBuf,
    source_dir: PathBuf,
    path_cache: RefCell<HashMap<PathBuf, PathBuf>>,
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
        }
    }

    /// Get the project root path
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Get the config
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Resolve the expected source path for status checks.
    pub fn expected_source_path(&self, source: &Path, target: &TargetConfig) -> Option<PathBuf> {
        // expected_source_path feeds status/entry_is_problematic; when should_compress_agents_md
        // applies, only return compressed_agents_md_path if it already exists.
        if self.should_compress_agents_md(source, target) {
            if source.exists() {
                let compressed = compressed_agents_md_path(source);
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

    /// Perform the sync operation
    pub fn sync(&self, options: &SyncOptions) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        if options.dry_run {
            println!("{}", "Running in dry-run mode\n".cyan());
        }

        for (agent_name, agent_config) in &self.config.agents {
            // Skip disabled agents
            if !agent_config.enabled {
                if options.verbose {
                    println!("  {} Skipping disabled agent: {}", "○".yellow(), agent_name);
                }
                continue;
            }

            // Filter by agent name if specified
            // Priority: CLI --agents flag > default_agents config > all enabled agents
            if let Some(ref filter) = options.agents {
                // Use CLI-provided agents (case-insensitive substring matching)
                if !filter
                    .iter()
                    .any(|f| agent_name.to_lowercase().contains(&f.to_lowercase()))
                {
                    if options.verbose {
                        println!("  {} Skipping filtered agent: {}", "○".yellow(), agent_name);
                    }
                    continue;
                }
            } else if !self.config.default_agents.is_empty() {
                // Use default_agents from config (case-insensitive substring matching)
                if !self
                    .config
                    .default_agents
                    .iter()
                    .any(|f| agent_name.to_lowercase().contains(&f.to_lowercase()))
                {
                    if options.verbose {
                        println!(
                            "  {} Skipping agent (not in default_agents): {}",
                            "○".yellow(),
                            agent_name
                        );
                    }
                    continue;
                }
            }
            // If neither --agents nor default_agents, process all enabled agents

            // Print agent header
            let desc = if agent_config.description.is_empty() {
                String::new()
            } else {
                format!(" - {}", agent_config.description)
            };
            println!("\n{}{}", agent_name.bold(), desc.dimmed());

            // Process each target
            for (target_name, target_config) in &agent_config.targets {
                if options.verbose {
                    println!("  Processing target: {}", target_name.dimmed());
                }

                match self.process_target(target_config, options) {
                    Ok(target_result) => {
                        result.created += target_result.created;
                        result.updated += target_result.updated;
                        result.skipped += target_result.skipped;
                    }
                    Err(e) => {
                        tracing::error!(target = %target_name, error = %e, "Error processing target");
                        result.errors += 1;
                    }
                }
            }
        }

        Ok(result)
    }

    /// Process a single target configuration
    fn process_target(&self, target: &TargetConfig, options: &SyncOptions) -> Result<SyncResult> {
        let source = self.source_dir.join(&target.source);
        let dest = self.project_root.join(&target.destination);

        match target.sync_type {
            SyncType::Symlink => {
                let resolved = self.resolve_source_path(&source, target, options)?;
                self.create_symlink(&resolved, &dest, options)
            }
            SyncType::SymlinkContents => self.create_symlinks_for_contents(
                &source,
                &dest,
                target.pattern.as_deref(),
                target,
                options,
            ),
        }
    }

    fn resolve_source_path(
        &self,
        source: &Path,
        target: &TargetConfig,
        options: &SyncOptions,
    ) -> Result<ResolvedSource> {
        if self.should_compress_agents_md(source, target) {
            if !source.exists() {
                return Ok(ResolvedSource {
                    path: source.to_path_buf(),
                    exists: false,
                });
            }

            let compressed = compressed_agents_md_path(source);

            if !options.dry_run {
                self.write_compressed_agents_md(source, &compressed)?;
            }

            let exists = options.dry_run || compressed.exists();
            return Ok(ResolvedSource {
                path: compressed,
                exists,
            });
        }

        Ok(ResolvedSource {
            path: source.to_path_buf(),
            exists: source.exists(),
        })
    }

    fn should_compress_agents_md(&self, source: &Path, target: &TargetConfig) -> bool {
        self.config.compress_agents_md
            && matches!(
                target.sync_type,
                SyncType::Symlink | SyncType::SymlinkContents
            )
            && is_agents_md_path(source)
    }

    fn write_compressed_agents_md(&self, source: &Path, dest: &Path) -> Result<()> {
        let content = fs::read_to_string(source)
            .with_context(|| format!("Failed to read AGENTS.md: {}", source.display()))?;
        let compressed = compress_agents_md_content(&content);

        if let Ok(existing) = fs::read_to_string(dest)
            && existing == compressed
        {
            return Ok(());
        }

        if let Some(parent) = dest.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        fs::write(dest, compressed)
            .with_context(|| format!("Failed to write compressed AGENTS.md: {}", dest.display()))
    }

    /// Create a single symlink
    fn create_symlink(
        &self,
        source: &ResolvedSource,
        dest: &Path,
        options: &SyncOptions,
    ) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        // Check if source exists
        if !source.exists {
            println!(
                "  {} Source does not exist: {}",
                "!".yellow(),
                source.path.display()
            );
            result.skipped += 1;
            return Ok(result);
        }

        // Create parent directory if needed
        if let Some(parent) = dest.parent()
            && !parent.exists()
        {
            if options.dry_run {
                if options.verbose {
                    println!(
                        "  {} Would create directory: {}",
                        "→".cyan(),
                        parent.display()
                    );
                }
            } else {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
                if options.verbose {
                    println!("  {} Created directory: {}", "✔".green(), parent.display());
                }
            }
        }

        // Calculate relative path from dest to source
        let allow_missing = options.dry_run && !source.path.exists();
        let relative_source = self.relative_path(dest, &source.path, allow_missing)?;

        // Handle existing destination
        if dest.is_symlink() {
            let current_target = fs::read_link(dest)?;
            if current_target == relative_source {
                if options.verbose {
                    println!("  {} Already linked: {}", "✔".green(), dest.display());
                }
                result.skipped += 1;
                return Ok(result);
            } else {
                // Wrong target, remove and recreate
                if options.dry_run {
                    println!(
                        "  {} Would update symlink: {} -> {}",
                        "→".cyan(),
                        dest.display(),
                        relative_source.display()
                    );
                } else {
                    fs::remove_file(dest)?;
                    if options.verbose {
                        println!(
                            "  {} Removed old symlink: {} (was -> {})",
                            "○".yellow(),
                            dest.display(),
                            current_target.display()
                        );
                    }
                }
                result.updated += 1;
            }
        } else if dest.exists() {
            // It's a regular file/directory - back it up
            if options.dry_run {
                println!(
                    "  {} Would backup and replace: {}",
                    "→".cyan(),
                    dest.display()
                );
            } else {
                let backup = PathBuf::from(format!(
                    "{}.bak.{}",
                    dest.display(),
                    chrono_lite_timestamp()
                ));
                fs::rename(dest, &backup)?;
                println!(
                    "  {} Backed up: {} -> {}",
                    "!".yellow(),
                    dest.display(),
                    backup.display()
                );
            }
            result.updated += 1;
        } else {
            result.created += 1;
        }

        // Create the symlink
        if options.dry_run {
            if result.created > 0 {
                println!(
                    "  {} Would link: {} -> {}",
                    "→".cyan(),
                    dest.display(),
                    relative_source.display()
                );
            }
        } else {
            #[cfg(unix)]
            std::os::unix::fs::symlink(&relative_source, dest)
                .with_context(|| format!("Failed to create symlink: {}", dest.display()))?;

            #[cfg(windows)]
            {
                if source.path.is_dir() {
                    std::os::windows::fs::symlink_dir(&relative_source, dest)?;
                } else {
                    std::os::windows::fs::symlink_file(&relative_source, dest)?;
                }
            }

            println!(
                "  {} Linked: {} -> {}",
                "✔".green(),
                dest.display(),
                relative_source.display()
            );
        }

        Ok(result)
    }

    /// Create symlinks for all contents of a directory
    fn create_symlinks_for_contents(
        &self,
        source_dir: &Path,
        dest_dir: &Path,
        pattern: Option<&str>,
        target: &TargetConfig,
        options: &SyncOptions,
    ) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        if !source_dir.exists() || !source_dir.is_dir() {
            println!(
                "  {} Source directory does not exist: {}",
                "!".yellow(),
                source_dir.display()
            );
            result.skipped += 1;
            return Ok(result);
        }

        // Create destination directory if needed
        if !dest_dir.exists() {
            if options.dry_run {
                if options.verbose {
                    println!(
                        "  {} Would create directory: {}",
                        "→".cyan(),
                        dest_dir.display()
                    );
                }
            } else {
                fs::create_dir_all(dest_dir)?;
                if options.verbose {
                    println!(
                        "  {} Created directory: {}",
                        "✔".green(),
                        dest_dir.display()
                    );
                }
            }
        }

        // Iterate through source directory contents
        for entry in WalkDir::new(source_dir).min_depth(1).max_depth(1) {
            let entry = entry?;
            let item_name = entry.file_name().to_string_lossy();

            // Apply pattern filter if specified
            if let Some(pat) = pattern
                && !matches_pattern(&item_name, pat)
            {
                continue;
            }

            let source_path = entry.path();
            let dest_path = dest_dir.join(entry.file_name());

            let resolved = self.resolve_source_path(source_path, target, options)?;
            let item_result = self.create_symlink(&resolved, &dest_path, options)?;
            result.created += item_result.created;
            result.updated += item_result.updated;
            result.skipped += item_result.skipped;
        }

        Ok(result)
    }

    /// Get the canonical path for a given path, using a cache to avoid
    /// redundant I/O operations.
    fn canonicalize_cached(&self, path: &Path) -> Result<PathBuf> {
        let mut cache = self.path_cache.borrow_mut();
        if let Some(cached) = cache.get(path) {
            return Ok(cached.clone());
        }

        let canonical = fs::canonicalize(path)?;
        cache.insert(path.to_path_buf(), canonical.clone());
        Ok(canonical)
    }

    /// Calculate relative path from dest to source
    fn relative_path(&self, from: &Path, to: &Path, allow_missing: bool) -> Result<PathBuf> {
        let from_dir = from.parent().unwrap_or(from);

        // Canonicalize paths for accurate relative calculation
        let from_abs = if from_dir.exists() {
            self.canonicalize_cached(from_dir)?
        } else {
            // If dest dir doesn't exist yet, use project root as base
            let relative = from_dir
                .strip_prefix(&self.project_root)
                .unwrap_or(from_dir);
            self.project_root.join(relative)
        };

        let to_abs = match self.canonicalize_cached(to) {
            Ok(path) => path,
            Err(_) if allow_missing => {
                if to.is_absolute() {
                    to.to_path_buf()
                } else {
                    self.project_root.join(to)
                }
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("Source path does not exist: {}", to.display()));
            }
        };

        // Use pathdiff to calculate relative path
        pathdiff::diff_paths(&to_abs, &from_abs)
            .ok_or_else(|| anyhow::anyhow!("Cannot calculate relative path"))
    }

    /// Clean all symlinks managed by this configuration
    pub fn clean(&self, options: &SyncOptions) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        println!("{}", "Cleaning managed symlinks...".cyan());

        for agent_config in self.config.agents.values() {
            for target_config in agent_config.targets.values() {
                let dest = self.project_root.join(&target_config.destination);

                if dest.is_symlink() {
                    if options.dry_run {
                        println!("  {} Would remove: {}", "→".cyan(), dest.display());
                    } else {
                        fs::remove_file(&dest)?;
                        println!("  {} Removed: {}", "✔".green(), dest.display());
                    }
                    result.removed += 1;
                } else if dest.is_dir() && target_config.sync_type == SyncType::SymlinkContents {
                    // For symlink-contents, remove symlinks inside the directory
                    for entry in WalkDir::new(&dest).min_depth(1).max_depth(1) {
                        let entry = entry?;
                        if entry.path().is_symlink() {
                            if options.dry_run {
                                println!(
                                    "  {} Would remove: {}",
                                    "→".cyan(),
                                    entry.path().display()
                                );
                            } else {
                                fs::remove_file(entry.path())?;
                                println!("  {} Removed: {}", "✔".green(), entry.path().display());
                            }
                            result.removed += 1;
                        }
                    }
                    // Try to remove the directory if empty
                    if !options.dry_run {
                        let _ = fs::remove_dir(&dest);
                    }
                }
            }
        }

        Ok(result)
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
                .filter(|agent| {
                    filter
                        .iter()
                        .any(|f| agent.id().to_lowercase().contains(&f.to_lowercase()))
                })
                .collect()
        } else if !self.config.default_agents.is_empty() {
            // Apply default_agents filtering
            enabled_agents
                .into_iter()
                .filter(|agent| {
                    self.config
                        .default_agents
                        .iter()
                        .any(|f| agent.id().to_lowercase().contains(&f.to_lowercase()))
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

fn is_agents_md_path(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "AGENTS.md")
}

fn compressed_agents_md_path(path: &Path) -> PathBuf {
    path.with_file_name(COMPRESSED_AGENTS_MD_NAME)
}

fn compress_agents_md_content(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    // Track the exact fence delimiter so only a matching fence closes the block.
    let mut fence_delim: Option<String> = None;
    let mut previous_blank = false;

    for line in input.lines() {
        let trimmed_end = line.trim_end_matches([' ', '\t']);
        let trimmed_start = trimmed_end.trim_start();
        let fence_delim_match = if trimmed_start.starts_with("```") {
            Some(
                trimmed_start
                    .chars()
                    .take_while(|c| *c == '`')
                    .collect::<String>(),
            )
        } else if trimmed_start.starts_with("~~~") {
            Some(
                trimmed_start
                    .chars()
                    .take_while(|c| *c == '~')
                    .collect::<String>(),
            )
        } else {
            None
        };
        let is_fence = fence_delim_match.is_some();

        if is_fence {
            let delim =
                fence_delim_match.expect("fence_delim_match should be set when is_fence is true");
            if fence_delim.is_none() {
                fence_delim = Some(delim);
            } else if fence_delim.as_ref() == Some(&delim) {
                fence_delim = None;
            }
            out.push_str(trimmed_end);
            out.push('\n');
            previous_blank = false;
            continue;
        }

        if fence_delim.is_some() {
            out.push_str(trimmed_end);
            out.push('\n');
            previous_blank = false;
            continue;
        }

        if trimmed_end.trim().is_empty() {
            if !previous_blank {
                out.push('\n');
                previous_blank = true;
            }
            continue;
        }

        previous_blank = false;
        let (leading, rest) = split_leading_whitespace(trimmed_end);
        let normalized = normalize_inline_whitespace(rest);
        out.push_str(leading);
        out.push_str(&normalized);
        out.push('\n');
    }

    out
}

fn split_leading_whitespace(line: &str) -> (&str, &str) {
    let idx = line
        .char_indices()
        .find(|(_, c)| *c != ' ' && *c != '\t')
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| line.len());
    line.split_at(idx)
}

fn normalize_inline_whitespace(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_whitespace = false;

    for ch in line.chars() {
        if ch == ' ' || ch == '\t' {
            if !in_whitespace {
                out.push(' ');
                in_whitespace = true;
            }
        } else {
            in_whitespace = false;
            out.push(ch);
        }
    }

    out
}

/// Simple glob pattern matching (supports * and ?)
/// This is an iterative implementation which is more performant than the previous
/// recursive one, especially for patterns with '*' since it avoids string
/// allocations and recursion. It uses backtracking with a stored star position.
/// This implementation is Unicode-aware.
fn matches_pattern(name: &str, pattern: &str) -> bool {
    let mut name_it = name.chars();
    let mut pattern_it = pattern.chars();

    let mut star_p_it = None;
    let mut star_n_it = None;

    loop {
        let s_char = name_it.clone().next();
        let p_char = pattern_it.clone().next();

        match (s_char, p_char) {
            (Some(s), Some(p)) if p == s || p == '?' => {
                name_it.next();
                pattern_it.next();
            }
            (_, Some('*')) => {
                pattern_it.next();
                star_p_it = Some(pattern_it.clone());
                star_n_it = Some(name_it.clone());
            }
            (Some(_), _) => {
                // Name has chars, but pattern doesn't match and is not '*'
                if let (Some(star_p), Some(star_n)) = (star_p_it.as_mut(), star_n_it.as_mut()) {
                    if star_n.next().is_none() {
                        return false;
                    }
                    name_it = star_n.clone();
                    pattern_it = star_p.clone();
                } else {
                    return false; // Mismatch and no star to backtrack to.
                }
            }
            (None, _) => {
                // Name is exhausted
                return pattern_it.all(|c| c == '*');
            }
        }
    }
}

/// Generate a simple timestamp without external dependencies
fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
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
    // LINKER CREATION TESTS
    // ==========================================================================

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
    // TIMESTAMP FUNCTION TESTS
    // ==========================================================================

    #[test]
    fn test_chrono_lite_timestamp() {
        let ts = chrono_lite_timestamp();

        // Should be a numeric string
        assert!(ts.chars().all(|c| c.is_ascii_digit()));

        // Should be a reasonable Unix timestamp (after year 2020)
        let ts_num: u64 = ts.parse().unwrap();
        assert!(ts_num > 1577836800); // 2020-01-01
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
    fn test_sync_mcp_only_creates_for_configured_agents() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        // Only configure claude and copilot - cursor, gemini, vscode, opencode should NOT get configs
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
    }
}
