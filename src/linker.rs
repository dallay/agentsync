//! Symbolic link creation and management
//!
//! Handles creating, updating, and removing symbolic links
//! for AI agent configuration synchronization.

use anyhow::{Context, Result};
use colored::Colorize;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
#[cfg(windows)]
use std::os::windows::fs::FileTypeExt;
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
    compression_cache: RefCell<HashMap<PathBuf, String>>,
    ensured_dirs: RefCell<HashSet<PathBuf>>,
    ensured_compressed: RefCell<HashSet<PathBuf>>,
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
            ensured_dirs: RefCell::new(HashSet::new()),
            ensured_compressed: RefCell::new(HashSet::new()),
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
        // Clear caches at the start of every sync run to prevent stale state.
        self.compression_cache.borrow_mut().clear();
        self.ensured_dirs.borrow_mut().clear();
        self.ensured_compressed.borrow_mut().clear();
        self.path_cache.borrow_mut().clear();

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
                if !filter
                    .iter()
                    .any(|f| crate::agent_ids::sync_filter_matches(agent_name, f))
                {
                    if options.verbose {
                        println!("  {} Skipping filtered agent: {}", "○".yellow(), agent_name);
                    }
                    continue;
                }
            } else if !self.config.default_agents.is_empty()
                && !self
                    .config
                    .default_agents
                    .iter()
                    .any(|f| crate::agent_ids::sync_filter_matches(agent_name, f))
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

                match self.process_target(agent_name, target_config, options) {
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
    fn process_target(
        &self,
        agent_name: &str,
        target: &TargetConfig,
        options: &SyncOptions,
    ) -> Result<SyncResult> {
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
            SyncType::NestedGlob => {
                // For NestedGlob, `source` is relative to the project root (not source_dir).
                let search_root = self.project_root.join(&target.source);
                self.process_nested_glob(
                    &search_root,
                    target.pattern.as_deref().unwrap_or("**/AGENTS.md"),
                    &target.exclude,
                    &target.destination,
                    options,
                )
            }
            SyncType::ModuleMap => self.process_module_map(agent_name, target, options),
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
                self.write_compressed_agents_md(source, &compressed, options)?;
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

    /// Ensure a directory exists, using the ensured_dirs cache to avoid redundant I/O.
    /// Respects dry_run and verbose options.
    fn ensure_directory(&self, dir: &Path, options: &SyncOptions) -> Result<()> {
        let mut ensured = self.ensured_dirs.borrow_mut();
        if !ensured.contains(dir) {
            if !dir.exists() {
                if options.dry_run {
                    if options.verbose {
                        println!("  {} Would create directory: {}", "→".cyan(), dir.display());
                    }
                } else {
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

    fn write_compressed_agents_md(
        &self,
        source: &Path,
        dest: &Path,
        options: &SyncOptions,
    ) -> Result<()> {
        // Optimization: skip if this output file was already ensured in this run.
        if self.ensured_compressed.borrow().contains(dest) {
            return Ok(());
        }

        let compressed = {
            let mut cache = self.compression_cache.borrow_mut();
            if let Some(cached) = cache.get(source) {
                cached.clone()
            } else {
                let content = fs::read_to_string(source)
                    .with_context(|| format!("Failed to read AGENTS.md: {}", source.display()))?;
                let compressed = compress_agents_md_content(&content);
                cache.insert(source.to_path_buf(), compressed.clone());
                compressed
            }
        };

        if let Ok(existing) = fs::read_to_string(dest)
            && existing == compressed
        {
            self.ensured_compressed
                .borrow_mut()
                .insert(dest.to_path_buf());
            return Ok(());
        }

        if let Some(parent) = dest.parent() {
            self.ensure_directory(parent, options)?;
        }

        fs::write(dest, &compressed)
            .with_context(|| format!("Failed to write compressed AGENTS.md: {}", dest.display()))?;

        self.ensured_compressed
            .borrow_mut()
            .insert(dest.to_path_buf());
        Ok(())
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
        if let Some(parent) = dest.parent() {
            self.ensure_directory(parent, options)?;
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
                    remove_symlink(dest)?;
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
                let backup = backup_path_for_destination(dest);
                remove_existing_path(&backup)?;
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
        self.ensure_directory(dest_dir, options)?;

        // Iterate through source directory contents
        for entry in fs::read_dir(source_dir)
            .with_context(|| format!("Failed to read source directory: {}", source_dir.display()))?
        {
            let entry = entry
                .with_context(|| format!("Failed to read entry in: {}", source_dir.display()))?;
            let file_name = entry.file_name();
            let item_name = file_name.to_string_lossy();

            // Apply pattern filter if specified
            if let Some(pat) = pattern
                && !matches_pattern(&item_name, pat)
            {
                continue;
            }

            let source_path = entry.path();
            let dest_path = dest_dir.join(entry.file_name());

            let resolved = self.resolve_source_path(&source_path, target, options)?;
            let item_result = self.create_symlink(&resolved, &dest_path, options)?;
            result.created += item_result.created;
            result.updated += item_result.updated;
            result.skipped += item_result.skipped;
        }

        Ok(result)
    }

    /// Expand a destination template for a single discovered file.
    ///
    /// Replaces the following placeholders:
    /// * `{relative_path}` – parent directory of the matched file relative to
    ///   the search root (e.g. `clients/agent-runtime`).  When the file is
    ///   directly inside the search root, this is `.` (current directory).
    /// * `{file_name}` – the file's name (e.g. `AGENTS.md`)
    /// * `{stem}` – the file name without its extension (e.g. `AGENTS`)
    /// * `{ext}` – the file's extension without the dot (e.g. `md`)
    fn expand_destination_template(
        template: &str,
        rel_path: &Path, // path of the discovered file relative to search root
    ) -> String {
        let file_name = rel_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        let stem = rel_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let ext = rel_path
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default();

        // Use "." for files directly inside the search root so that
        // templates like "{relative_path}/CLAUDE.md" produce a valid relative
        // path ("./CLAUDE.md") rather than an absolute path ("/CLAUDE.md").
        let dir = rel_path
            .parent()
            .map(|p| {
                let s = p.to_string_lossy().into_owned();
                if s.is_empty() { ".".to_string() } else { s }
            })
            .unwrap_or_else(|| ".".to_string());

        template
            .replace("{relative_path}", &dir)
            .replace("{file_name}", &file_name)
            .replace("{stem}", &stem)
            .replace("{ext}", &ext)
    }

    /// Process a `NestedGlob` target: walk `search_root`, match files against
    /// `glob_pattern`, skip excluded paths, and create a symlink for each
    /// Discovers files in a directory tree matching a glob pattern and creates
    /// symlinks to each matched file.
    ///
    /// # Arguments
    /// * `search_root` - Directory to walk for file discovery
    /// * `glob_pattern` - Glob pattern with `**` support (e.g., `**/AGENTS.md`)
    /// * `excludes` - Optional list of glob patterns to exclude from syncing
    /// * `dest_template` - Destination path template supporting:
    ///   - `{relative_path}` - Path relative to search root (`.` for root files)
    ///   - `{file_name}` - Original filename with extension
    ///   - `{stem}` - Filename without extension
    ///   - `{ext}` - File extension (without leading dot)
    /// * `options` - Sync options controlling verbose output and dry-run mode
    ///
    /// # Behavior
    /// - Walks `search_root` recursively (following `follow_links = false` for safety)
    /// - Skips directories and non-file entries
    /// - Matches files against `glob_pattern` using `**` glob support
    /// - Applies exclusion patterns if file matches any exclude, it is skipped
    /// - Creates symlinks using `create_symlink()` which handles existing links
    ///
    /// # Performance Considerations
    /// - Uses `find()` instead of `any()` for exclusion checks to enable early-exit
    /// - Avoids exclusion iteration when `excludes` list is empty
    fn process_nested_glob(
        &self,
        search_root: &Path,
        glob_pattern: &str,
        excludes: &[String],
        dest_template: &str,
        options: &SyncOptions,
    ) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        if !search_root.exists() || !search_root.is_dir() {
            println!(
                "  {} Search root does not exist: {}",
                "!".yellow(),
                search_root.display()
            );
            result.skipped += 1;
            return Ok(result);
        }

        self.for_each_nested_glob_match(
            search_root,
            glob_pattern,
            excludes,
            options,
            |full_path, rel_path| {
                let dest_str = Self::expand_destination_template(dest_template, rel_path);
                if dest_str.is_empty() {
                    if options.verbose {
                        println!(
                            "  {} Destination template produced empty path for: {}",
                            "!".yellow(),
                            full_path.display()
                        );
                    }
                    result.skipped += 1;
                    return Ok(());
                }

                let dest = self.project_root.join(&dest_str);

                let resolved = ResolvedSource {
                    path: full_path.to_path_buf(),
                    exists: true,
                };

                let item_result = self.create_symlink(&resolved, &dest, options)?;
                result.created += item_result.created;
                result.updated += item_result.updated;
                result.skipped += item_result.skipped;

                Ok(())
            },
        )?;

        Ok(result)
    }

    fn for_each_nested_glob_match<F>(
        &self,
        search_root: &Path,
        glob_pattern: &str,
        excludes: &[String],
        options: &SyncOptions,
        mut on_match: F,
    ) -> Result<()>
    where
        F: FnMut(&Path, &Path) -> Result<()>,
    {
        let mut it = WalkDir::new(search_root).follow_links(false).into_iter();

        while let Some(entry) = it.next() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    if options.verbose {
                        let path = err
                            .path()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "<unknown path>".to_string());
                        println!(
                            "  {} WalkDir error while traversing {}: {}",
                            "!".yellow(),
                            path,
                            err
                        );
                    }
                    tracing::debug!(error = %err, path = ?err.path(), "WalkDir entry skipped during nested-glob traversal");
                    continue;
                }
            };

            let full_path = entry.path();
            let rel_path = match full_path.strip_prefix(search_root) {
                Ok(path) => path,
                Err(_) => continue,
            };

            let rel_str = rel_path
                .components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");

            if rel_str.is_empty() {
                continue;
            }

            if let Some(matched_exclude) = excludes
                .iter()
                .find(|exclude| matches_path_glob(&rel_str, exclude))
            {
                if options.verbose {
                    println!(
                        "  {} Excluded by '{}': {}",
                        "○".yellow(),
                        matched_exclude,
                        full_path.display()
                    );
                }

                if entry.file_type().is_dir() {
                    it.skip_current_dir();
                }
                continue;
            }

            if !entry.file_type().is_file() {
                continue;
            }

            if !matches_path_glob(&rel_str, glob_pattern) {
                continue;
            }

            on_match(full_path, rel_path)?;
        }

        Ok(())
    }

    /// Process a `module-map` target: iterate mappings and create a symlink
    /// for each one, resolving the destination filename from:
    /// 1. mapping.filename_override (explicit user choice)
    /// 2. agent_convention_filename (per-agent convention)
    /// 3. source file basename (fallback)
    fn process_module_map(
        &self,
        agent_name: &str,
        target: &TargetConfig,
        options: &SyncOptions,
    ) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        if target.mappings.is_empty() {
            if options.verbose {
                println!(
                    "  {} No mappings defined for module-map target",
                    "!".yellow()
                );
            }
            return Ok(result);
        }

        for mapping in &target.mappings {
            let source_path = self.source_dir.join(&mapping.source);

            // Resolve destination filename
            let filename = crate::config::resolve_module_map_filename(mapping, agent_name);

            // Validate: dest components must be relative and safe
            if let Some(err) =
                Self::validate_module_map_path_components(&mapping.destination, &filename)
            {
                if options.verbose {
                    println!(
                        "  {} Skipping mapping {}: {}",
                        "!".yellow(),
                        mapping.source,
                        err
                    );
                }
                result.skipped += 1;
                continue;
            }

            let dest = self.project_root.join(&mapping.destination).join(&filename);

            let resolved = ResolvedSource {
                path: source_path.clone(),
                exists: source_path.exists(),
            };

            let item_result = self.create_symlink(&resolved, &dest, options)?;
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

        for (agent_name, agent_config) in &self.config.agents {
            for target_config in agent_config.targets.values() {
                match target_config.sync_type {
                    SyncType::NestedGlob => {
                        // Re-discover the same files and remove the corresponding symlinks.
                        let search_root = self.project_root.join(&target_config.source);
                        if !search_root.exists() || !search_root.is_dir() {
                            continue;
                        }
                        let glob_pattern =
                            target_config.pattern.as_deref().unwrap_or("**/AGENTS.md");
                        let dest_template = &target_config.destination;
                        let excludes = &target_config.exclude;

                        self.for_each_nested_glob_match(
                            &search_root,
                            glob_pattern,
                            excludes,
                            options,
                            |_, rel_path| {
                                let dest_str =
                                    Self::expand_destination_template(dest_template, rel_path);
                                if dest_str.is_empty() {
                                    return Ok(());
                                }

                                let dest = self.project_root.join(&dest_str);
                                if dest.is_symlink() {
                                    if options.dry_run {
                                        println!(
                                            "  {} Would remove: {}",
                                            "→".cyan(),
                                            dest.display()
                                        );
                                    } else {
                                        fs::remove_file(&dest)?;
                                        println!("  {} Removed: {}", "✔".green(), dest.display());
                                    }
                                    result.removed += 1;
                                }

                                Ok(())
                            },
                        )?;
                    }
                    SyncType::SymlinkContents => {
                        let dest = self.project_root.join(&target_config.destination);
                        if dest.is_dir() {
                            // For symlink-contents, remove symlinks inside the directory
                            for entry in fs::read_dir(&dest).with_context(|| {
                                format!("Failed to read destination directory: {}", dest.display())
                            })? {
                                let entry = entry.with_context(|| {
                                    format!("Failed to read entry in: {}", dest.display())
                                })?;
                                if entry.path().is_symlink() {
                                    if options.dry_run {
                                        println!(
                                            "  {} Would remove: {}",
                                            "→".cyan(),
                                            entry.path().display()
                                        );
                                    } else {
                                        fs::remove_file(entry.path())?;
                                        println!(
                                            "  {} Removed: {}",
                                            "✔".green(),
                                            entry.path().display()
                                        );
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
                    SyncType::Symlink => {
                        let dest = self.project_root.join(&target_config.destination);
                        if dest.is_symlink() {
                            if options.dry_run {
                                println!("  {} Would remove: {}", "→".cyan(), dest.display());
                            } else {
                                remove_symlink(&dest)?;
                                println!("  {} Removed: {}", "✔".green(), dest.display());
                            }
                            result.removed += 1;
                        }
                    }
                    SyncType::ModuleMap => {
                        for mapping in &target_config.mappings {
                            let filename =
                                crate::config::resolve_module_map_filename(mapping, agent_name);

                            // Validate: dest components must be relative and safe
                            if let Some(err) = Self::validate_module_map_path_components(
                                &mapping.destination,
                                &filename,
                            ) {
                                if options.verbose {
                                    println!(
                                        "  {} Skipping mapping {}: {}",
                                        "!".yellow(),
                                        mapping.source,
                                        err
                                    );
                                }
                                continue;
                            }

                            let dest = self.project_root.join(&mapping.destination).join(&filename);

                            if dest.is_symlink() {
                                if options.dry_run {
                                    println!("  {} Would remove: {}", "→".cyan(), dest.display());
                                } else {
                                    fs::remove_file(&dest)?;
                                    println!("  {} Removed: {}", "✔".green(), dest.display());
                                }
                                result.removed += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    /// Validate that destination directory and filename components are safe for joining
    /// to project_root (i.e., they are relative and contain no parent-dir traversal).
    ///
    /// Returns `Some(error_message)` if validation fails, `None` if safe.
    fn validate_module_map_path_components(dest_dir: &str, filename: &str) -> Option<String> {
        // Check destination directory
        if Path::new(dest_dir).is_absolute() {
            return Some(format!("destination directory is absolute: {dest_dir}"));
        }
        if Path::new(dest_dir)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Some(format!(
                "destination directory contains parent-dir component: {dest_dir}"
            ));
        }
        // Check filename
        if Path::new(filename).is_absolute() {
            return Some(format!("filename is absolute: {filename}"));
        }
        if Path::new(filename)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Some(format!(
                "filename contains parent-dir component: {filename}"
            ));
        }
        None
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

fn is_agents_md_path(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "AGENTS.md")
}

fn compressed_agents_md_path(path: &Path) -> PathBuf {
    path.with_file_name(COMPRESSED_AGENTS_MD_NAME)
}

fn compress_agents_md_content(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    // Track the exact fence delimiter so only a matching fence closes the block.
    let mut fence_delim: Option<&str> = None;
    let mut previous_blank = false;

    for line in input.lines() {
        let trimmed_end = line.trim_end_matches([' ', '\t']);
        let trimmed_start = trimmed_end.trim_start();

        // Detect code fence delimiter (``` or ~~~) via slicing, no allocation.
        // fence_delim_match borrows from trimmed_start, avoiding a new String.
        let fence_delim_match = if trimmed_start.starts_with("```") {
            let len = trimmed_start
                .find(|c| c != '`')
                .unwrap_or(trimmed_start.len());
            Some(&trimmed_start[..len])
        } else if trimmed_start.starts_with("~~~") {
            let len = trimmed_start
                .find(|c| c != '~')
                .unwrap_or(trimmed_start.len());
            Some(&trimmed_start[..len])
        } else {
            None
        };
        let is_fence = fence_delim_match.is_some();

        if is_fence {
            let delim =
                fence_delim_match.expect("fence_delim_match should be set when is_fence is true");
            if fence_delim.is_none() {
                fence_delim = Some(delim);
            } else if fence_delim == Some(delim) {
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
        out.push_str(leading);
        normalize_inline_whitespace_to(rest, &mut out);
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

fn normalize_inline_whitespace_to(line: &str, out: &mut String) {
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

/// Path-aware glob pattern matching that supports the `**` double-star wildcard.
///
/// Rules:
/// * `**` (as a whole path segment) matches any number of path segments,
///   including zero segments.
/// * `*` within a segment matches any sequence of characters that does not
///   include a `/`.
/// * `?` within a segment matches any single character other than `/`.
///
/// The `path` argument must use `/` as the path separator.  Use
/// [`matches_pattern`] for single-segment (filename-only) matching.
fn matches_path_glob(path: &str, pattern: &str) -> bool {
    let path_parts: Vec<&str> = path.split('/').collect();
    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    path_glob_match(&path_parts, &pattern_parts)
}

/// Recursive helper for [`matches_path_glob`].
fn path_glob_match(path: &[&str], pattern: &[&str]) -> bool {
    match (path, pattern) {
        // Both exhausted – success.
        ([], []) => true,
        // Pattern exhausted but path still has segments – no match.
        (_, []) => false,
        // Leading `**` in pattern.
        ([_, ..], ["**", rest_pat @ ..]) => {
            // `**` can match zero segments …
            if path_glob_match(path, rest_pat) {
                return true;
            }
            // … or one or more segments.
            for i in 1..=path.len() {
                if path_glob_match(&path[i..], rest_pat) {
                    return true;
                }
            }
            false
        }
        // `**` at end of pattern matches all remaining path segments.
        (_, ["**"]) => true,
        // Pattern starts with `**` but path is empty – matches only if the
        // rest of the pattern also matches empty path.
        ([], ["**", rest_pat @ ..]) => path_glob_match(&[], rest_pat),
        // Normal segment: match the head segment with `matches_pattern` and
        // recurse on the tails.
        ([path_head, path_rest @ ..], [pat_head, pat_rest @ ..]) => {
            matches_pattern(path_head, pat_head) && path_glob_match(path_rest, pat_rest)
        }
        _ => false,
    }
}

fn backup_path_for_destination(dest: &Path) -> PathBuf {
    PathBuf::from(format!("{}.bak", dest.display()))
}

fn remove_existing_path(path: &Path) -> std::io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };

    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        remove_symlink(path)
    } else if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Remove a symlink, handling both file and directory symlinks cross-platform.
/// On Windows, directory symlinks require `fs::remove_dir()` instead of `fs::remove_file()`.
fn remove_symlink(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let meta = fs::symlink_metadata(path)?;
        if meta.file_type().is_symlink_dir() {
            return fs::remove_dir(path);
        }
    }
    fs::remove_file(path)
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
}
