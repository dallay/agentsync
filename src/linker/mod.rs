//! Linker façade for synchronizing AI-agent configuration files.
//!
//! [`Linker`] owns shared state and the public API while focused child modules
//! implement apply, clean, discovery, path safety, and symlink mutation.

use anyhow::{Context, Result};
use colored::Colorize;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::config::{Config, TargetConfig};
use crate::mcp_ownership::{LockedOwnership, McpOwnershipStore, OwnershipRecord, sha256_bytes};

#[cfg(test)]
use discovery::matches_path_glob;
use discovery::matches_pattern;
#[cfg(test)]
use discovery::path_glob_match_iter;
pub use timing::TimingSink;

mod apply;
mod clean;
mod discovery;
mod enumerate;
mod paths;
mod quarantine;
#[cfg(test)]
mod quarantine_tests;
mod revert;
mod symlinks;
pub mod timing;

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
type NestedGlobCacheValue = (NestedGlobMatches, bool);
#[cfg(test)]
type CleanBeforeReadContentsHook = Rc<dyn Fn(&Path)>;
#[cfg(test)]
type ReadContentsAfterMetadataHook = Rc<dyn Fn(&Path)>;
#[cfg(test)]
type ProjectRootBeforeOpenHook = Box<dyn FnOnce(&Path)>;
#[cfg(test)]
type QuarantineBeforeMoveHook = Rc<dyn Fn(&Path)>;
#[cfg(test)]
type QuarantineAfterMoveHook = Rc<dyn Fn(&Path)>;

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
    /// Keep .bak backups after a revert restore instead of consuming them
    pub keep_backups: bool,
}

/// Result of a sync operation
#[derive(Debug, Default)]
pub struct SyncResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub removed: usize,
    pub restored: usize,
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
    /// Canonicalize cache. `BTreeMap` (sorted keys) so scoped invalidation can
    /// remove a path prefix via a contiguous range scan in O(log n + m) instead
    /// of scanning the whole map per mutation.
    path_cache: RefCell<BTreeMap<PathBuf, Rc<PathBuf>>>,
    compression_cache: RefCell<HashMap<PathBuf, Rc<str>>>,
    /// Cache for NestedGlob discovery results: (search_root, pattern, excludes) -> [(full_path, rel_path)]
    glob_cache: RefCell<HashMap<NestedGlobKey, NestedGlobCacheValue>>,
    ensured_dirs: RefCell<HashSet<PathBuf>>,
    ensured_compressed: RefCell<HashSet<PathBuf>>,
    canonical_project_root: RefCell<Option<Rc<PathBuf>>>,
    project_root_capability: RefCell<Option<cap_std::fs::Dir>>,
    /// Timing sink for the developer-only benchmark harness. `None` in normal
    /// runs, where the guarded spans short-circuit without any `Instant::now`
    /// cost.
    timing: RefCell<Option<Rc<RefCell<TimingSink>>>>,
    #[cfg(test)]
    mcp_ownership_data_root: Option<PathBuf>,
    #[cfg(test)]
    clean_before_read_contents_hook: RefCell<Option<CleanBeforeReadContentsHook>>,
    #[cfg(test)]
    read_contents_after_metadata_hook: RefCell<Option<ReadContentsAfterMetadataHook>>,
    #[cfg(test)]
    project_root_before_open_hook: RefCell<Option<ProjectRootBeforeOpenHook>>,
    #[cfg(test)]
    quarantine_before_move_hook: RefCell<Option<QuarantineBeforeMoveHook>>,
    #[cfg(test)]
    quarantine_before_container_move_hook: RefCell<Option<QuarantineBeforeMoveHook>>,
    #[cfg(test)]
    quarantine_after_move_hook: RefCell<Option<QuarantineAfterMoveHook>>,
    #[cfg(test)]
    clean_metadata_error_path: RefCell<Option<PathBuf>>,
    #[cfg(test)]
    revert_metadata_error_path: RefCell<Option<PathBuf>>,
    #[cfg(test)]
    nested_glob_walk_override: RefCell<Option<Box<dyn discovery::NestedGlobWalkIterator>>>,
    #[cfg(test)]
    symlink_contents_source_entries_override: RefCell<Option<Vec<PathBuf>>>,
}

impl Linker {
    /// Create a new linker from a configuration
    pub fn new(config: Config, config_path: PathBuf) -> Self {
        let project_root = Config::project_root(&config_path);
        let source_dir = config.source_dir(&config_path);
        #[cfg(test)]
        let mcp_ownership_data_root = {
            let parent = project_root.parent().unwrap_or(&project_root);
            let project_name = project_root
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("root"));
            Some(parent.join(".agentsync-test-local-data").join(project_name))
        };

        Self {
            config,
            config_path,
            project_root,
            source_dir,
            path_cache: RefCell::new(BTreeMap::new()),
            compression_cache: RefCell::new(HashMap::new()),
            glob_cache: RefCell::new(HashMap::new()),
            ensured_dirs: RefCell::new(HashSet::new()),
            ensured_compressed: RefCell::new(HashSet::new()),
            canonical_project_root: RefCell::new(None),
            project_root_capability: RefCell::new(None),
            timing: RefCell::new(None),
            #[cfg(test)]
            mcp_ownership_data_root,
            #[cfg(test)]
            clean_before_read_contents_hook: RefCell::new(None),
            #[cfg(test)]
            read_contents_after_metadata_hook: RefCell::new(None),
            #[cfg(test)]
            project_root_before_open_hook: RefCell::new(None),
            #[cfg(test)]
            quarantine_before_move_hook: RefCell::new(None),
            #[cfg(test)]
            quarantine_before_container_move_hook: RefCell::new(None),
            #[cfg(test)]
            quarantine_after_move_hook: RefCell::new(None),
            #[cfg(test)]
            clean_metadata_error_path: RefCell::new(None),
            #[cfg(test)]
            revert_metadata_error_path: RefCell::new(None),
            #[cfg(test)]
            nested_glob_walk_override: RefCell::new(None),
            #[cfg(test)]
            symlink_contents_source_entries_override: RefCell::new(None),
        }
    }

    /// Inject an isolated local-data root for unit tests.
    #[cfg(test)]
    pub(crate) fn with_mcp_ownership_data_root_for_tests(mut self, data_root: PathBuf) -> Self {
        self.mcp_ownership_data_root = Some(data_root);
        self
    }

    /// Install (or remove) the wall-clock timing sink used by the developer
    /// benchmark harness.
    ///
    /// Bench-only: normal operation never calls this, so `timing_span` returns
    /// `None` and the sync engine records no timings and pays no
    /// `Instant::now` cost.
    pub fn set_timing(&self, sink: Option<Rc<RefCell<TimingSink>>>) {
        *self.timing.borrow_mut() = sink;
    }

    /// Get the project root path
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Get the config
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get the path of the loaded `agentsync.toml` config file.
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Drop discovery caches after filesystem mutations that can affect later
    /// nested-glob walks. Symlink mutations do NOT require this as NestedGlob
    /// discovery uses follow_links(false).
    fn invalidate_glob_cache(&self) {
        self.glob_cache.borrow_mut().clear();
    }

    /// Resolve the expected source path for status checks.
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

    /// Derive the child entries that a `symlink-contents` target manages.
    pub fn symlink_contents_expected_children(
        &self,
        source_dir: &Path,
        target: &TargetConfig,
        agent_name: &str,
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

            // Skip the compressed file when compression is enabled to avoid false drift in status
            if self.config.compress_agents_md && item_name == COMPRESSED_AGENTS_MD_NAME {
                continue;
            }

            let source_path = entry.path();
            if let Some(expected_source_path) = self.expected_source_path(&source_path, target) {
                children.push(SymlinkContentsChildExpectation {
                    name: if crate::agent_ids::canonical_any_agent_id(agent_name) == Some("zcode")
                        && target.destination.ends_with(".zcode/commands")
                    {
                        crate::zcode_command_destination(&item_name)
                    } else {
                        item_name.into_owned()
                    },
                    source_path,
                    expected_source_path,
                });
            }
        }

        children.sort_by(|left, right| left.name.cmp(&right.name));

        Ok(Some(children))
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
        self.sync_mcp_with_servers(dry_run, agents_filter, &BTreeMap::new())
    }

    /// Sync MCP configurations while adding repository-owned plugin servers.
    ///
    /// Plugin servers are kept separate from the parsed project config until this point so the
    /// existing user-owned `[mcp_servers.*]` contract remains unchanged.
    pub fn sync_mcp_with_servers(
        &self,
        dry_run: bool,
        agents_filter: Option<&Vec<String>>,
        plugin_servers: &BTreeMap<String, crate::config::McpServerConfig>,
    ) -> Result<crate::mcp::McpSyncResult> {
        self.sync_mcp_with_servers_with_rebase(dry_run, agents_filter, plugin_servers, false)
    }

    /// Sync MCP configs, optionally allowing an explicit ownership-journal rebase
    /// after the user has edited a config since the previous apply.
    pub fn sync_mcp_with_servers_with_rebase(
        &self,
        dry_run: bool,
        agents_filter: Option<&Vec<String>>,
        plugin_servers: &BTreeMap<String, crate::config::McpServerConfig>,
        rebase_mcp_journal: bool,
    ) -> Result<crate::mcp::McpSyncResult> {
        use crate::mcp::McpGenerator;

        if !self.config.mcp.enabled {
            return Ok(crate::mcp::McpSyncResult::default());
        }

        if self.config.mcp_servers.is_empty() && plugin_servers.is_empty() {
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
        let filtered_agents = self.filtered_mcp_agents(enabled_agents, agents_filter);

        if filtered_agents.is_empty() {
            return Ok(crate::mcp::McpSyncResult::default());
        }

        let mut servers = self.config.mcp_servers.clone();
        for (name, server) in plugin_servers {
            if let Some(existing) = servers.get(name) {
                anyhow::ensure!(
                    existing == server,
                    "MCP server collision between project configuration and plugin: {name}"
                );
            } else {
                servers.insert(name.clone(), server.clone());
            }
        }

        let generator = McpGenerator::new(servers, self.config.mcp.merge_strategy);
        let configured_agents: Vec<_> = self
            .config
            .agents
            .keys()
            .filter_map(|name| crate::mcp::McpAgent::from_id(name))
            .collect();
        #[cfg(test)]
        let ownership_data_root = self.mcp_ownership_data_root.as_deref();
        #[cfg(not(test))]
        let ownership_data_root: Option<&Path> = None;
        generator.generate_all_with_ownership(
            &self.project_root,
            &filtered_agents,
            &configured_agents,
            dry_run,
            ownership_data_root,
            rebase_mcp_journal,
        )
    }

    /// Agents selected for an MCP operation, honoring CLI --agents then
    /// default_agents. Shared by sync and revert.
    fn filtered_mcp_agents(
        &self,
        enabled_agents: Vec<crate::mcp::McpAgent>,
        agents_filter: Option<&Vec<String>>,
    ) -> Vec<crate::mcp::McpAgent> {
        if let Some(filter) = agents_filter {
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
        }
    }

    /// Restore MCP files from the apply-time ownership journal.
    ///
    /// Unlike apply, this uses the recorded agent IDs and does not require those
    /// agents or their server definitions to remain enabled in the current config.
    pub fn restore_mcp_ownership(
        &self,
        dry_run: bool,
        agents_filter: Option<&Vec<String>>,
    ) -> Result<crate::mcp::McpSyncResult> {
        let store = self.open_mcp_ownership_store()?;
        let mut result = crate::mcp::McpSyncResult::default();
        if dry_run {
            let Some(manifest) = store.read_existing_read_only()? else {
                self.warn_if_legacy_mcp_is_unowned(&mut result, agents_filter);
                return Ok(result);
            };
            restore_ownership_records(
                &manifest.configs,
                OwnershipRestoreContext {
                    store: &store,
                    project_root: &self.project_root,
                    filters: agents_filter.or_else(|| {
                        (!self.config.default_agents.is_empty())
                            .then_some(&self.config.default_agents)
                    }),
                    dry_run: true,
                    locked: None,
                    result: &mut result,
                },
            )?;
            return Ok(result);
        }

        if store.read_existing()?.is_none() {
            self.warn_if_legacy_mcp_is_unowned(&mut result, agents_filter);
            return Ok(result);
        }
        let mut locked = store.lock()?;
        let records = locked.manifest().configs.clone();
        restore_ownership_records(
            &records,
            OwnershipRestoreContext {
                store: &store,
                project_root: &self.project_root,
                filters: agents_filter.or_else(|| {
                    (!self.config.default_agents.is_empty()).then_some(&self.config.default_agents)
                }),
                dry_run: false,
                locked: Some(&mut locked),
                result: &mut result,
            },
        )?;
        locked.persist_or_remove_empty()?;
        Ok(result)
    }

    fn warn_if_legacy_mcp_is_unowned(
        &self,
        result: &mut crate::mcp::McpSyncResult,
        agents_filter: Option<&Vec<String>>,
    ) {
        let filters = agents_filter.or_else(|| {
            (!self.config.default_agents.is_empty()).then_some(&self.config.default_agents)
        });
        let has_existing_selected_mcp_destination = self
            .config
            .agents
            .keys()
            .filter_map(|name| crate::mcp::McpAgent::from_id(name))
            .any(|agent| {
                let selected = filters.is_none_or(|filters| {
                    filters
                        .iter()
                        .any(|filter| mcp_agent_matches_filter(agent, filter))
                });
                selected
                    && agent
                        .resolved_config_path(&self.project_root)
                        .is_some_and(|path| fs::symlink_metadata(path).is_ok())
            });
        let has_configured_servers = self
            .config
            .mcp_servers
            .values()
            .any(|server| !server.disabled)
            || (self.config.plugins.enabled
                && !self.config.plugins.selections.is_empty()
                && !self.config.plugins.allowed_mcp.is_empty());
        if self.config.mcp.enabled
            && has_configured_servers
            && has_existing_selected_mcp_destination
        {
            println!(
                "  {} No MCP ownership journal found; leaving existing MCP configs unchanged",
                "!".yellow()
            );
            tracing::warn!(
                "No MCP ownership journal found; legacy MCP configs were left unchanged"
            );
            result.skipped += 1;
        }
    }

    fn open_mcp_ownership_store(&self) -> Result<McpOwnershipStore> {
        #[cfg(test)]
        if let Some(data_root) = self.mcp_ownership_data_root.as_deref() {
            return McpOwnershipStore::open_at(&self.project_root, data_root);
        }
        McpOwnershipStore::open(&self.project_root)
    }
}

struct OwnershipRestoreContext<'a> {
    store: &'a McpOwnershipStore,
    project_root: &'a Path,
    filters: Option<&'a Vec<String>>,
    dry_run: bool,
    locked: Option<&'a mut LockedOwnership>,
    result: &'a mut crate::mcp::McpSyncResult,
}

fn restore_ownership_records(
    records: &BTreeMap<String, OwnershipRecord>,
    context: OwnershipRestoreContext<'_>,
) -> Result<()> {
    let OwnershipRestoreContext {
        store,
        project_root,
        filters,
        dry_run,
        mut locked,
        result,
    } = context;
    for (config_id, record) in records {
        let selected = record.agent_ids.iter().all(|agent_id| {
            filters.is_none_or(|filters| {
                filters
                    .iter()
                    .any(|filter| crate::agent_ids::mcp_filter_matches(agent_id, filter))
            })
        });
        if !selected {
            println!(
                "  {} Skipping MCP restore for shared config: not all recorded agents are selected",
                "!".yellow()
            );
            result.skipped += 1;
            continue;
        }

        let (agent, path) =
            match resolve_ownership_record_path(store, project_root, config_id, record) {
                Ok(resolved) => resolved,
                Err(error) => {
                    println!(
                        "  {} Cannot safely restore MCP config: {error}",
                        "!".yellow()
                    );
                    tracing::error!(config_id, error = %error, "Invalid MCP ownership record");
                    result.errors += 1;
                    continue;
                }
            };

        // Non-dry-run restore already holds this project's journal lock. Keep the
        // acquisition order journal -> destination, and never take another journal
        // lock while this per-record destination guard is held.
        let _destination_lock = if dry_run {
            None
        } else {
            let lock = match crate::mcp::validate_mcp_config_path(agent, project_root, &path)
                .and_then(|()| store.lock_config_path(&path))
            {
                Ok(lock) => lock,
                Err(error) => {
                    println!(
                        "  {} Cannot lock MCP config for restore; retaining ownership record: {}",
                        "!".yellow(),
                        path.display()
                    );
                    tracing::warn!(
                        config_path = %path.display(),
                        error = %error,
                        "Could not lock MCP config destination for restore"
                    );
                    result.errors += 1;
                    continue;
                }
            };
            Some(lock)
        };

        let current = match read_mcp_config_bytes_if_regular(agent, project_root, &path) {
            Ok(current) => current,
            Err(error) => {
                println!(
                    "  {} Cannot safely restore MCP config: {error}",
                    "!".yellow()
                );
                tracing::warn!(config_path = %path.display(), error = %error, "MCP restore rejected config path");
                result.errors += 1;
                continue;
            }
        };
        let current_hash = current.as_deref().map(sha256_bytes);
        let original_hash = record
            .original_content
            .as_deref()
            .map(|content| sha256_bytes(content.as_bytes()));
        if current_hash == original_hash {
            if !dry_run && let Some(locked) = locked.as_deref_mut() {
                locked.remove_record(project_root, &path);
            }
            result.updated += 1;
            continue;
        }

        let matches_recorded_state = current_hash.as_deref()
            == Some(record.applied_sha256.as_str())
            || record
                .pre_write_sha256
                .as_deref()
                .is_some_and(|pre_write_hash| current_hash.as_deref() == Some(pre_write_hash));
        if !matches_recorded_state {
            println!(
                "  {} Skipping MCP restore; config changed since apply: {}",
                "!".yellow(),
                path.display()
            );
            tracing::warn!(config_path = %path.display(), "MCP config no longer matches its ownership journal");
            result.skipped += 1;
            continue;
        }

        if dry_run {
            println!(
                "  {} Would restore MCP config: {}",
                "→".cyan(),
                path.display()
            );
            result.updated += 1;
            continue;
        }

        // Re-check immediately before mutation. This also refuses a symlink
        // swapped in after the initial metadata inspection.
        let latest = match read_mcp_config_bytes_if_regular(agent, project_root, &path) {
            Ok(latest) => latest,
            Err(error) => {
                println!(
                    "  {} Cannot safely restore MCP config: {error}",
                    "!".yellow()
                );
                result.errors += 1;
                continue;
            }
        };
        if latest != current {
            println!(
                "  {} Skipping MCP restore; config changed during revert: {}",
                "!".yellow(),
                path.display()
            );
            result.skipped += 1;
            continue;
        }

        let restore_result = crate::mcp::validate_mcp_config_path(agent, project_root, &path)
            .and_then(|()| match record.original_content.as_deref() {
                Some(original) => restore_mcp_config_bytes(
                    agent,
                    project_root,
                    &path,
                    original.as_bytes(),
                    current.as_deref(),
                )
                .map_err(|error| match error {
                    RestoreMcpConfigError::Validation(error)
                    | RestoreMcpConfigError::Mutation(error) => error,
                }),
                None => remove_generated_mcp_config_with_hook(
                    agent,
                    project_root,
                    &path,
                    current.as_deref(),
                    || Ok(()),
                ),
            });
        match restore_result {
            Ok(RestoreMcpConfigOutcome::Changed) => {
                println!(
                    "  {} Skipping MCP restore; config changed during staging: {}",
                    "!".yellow(),
                    path.display()
                );
                tracing::warn!(config_path = %path.display(), "MCP config changed during restore staging");
                result.skipped += 1;
                continue;
            }
            Err(error) => {
                println!(
                    "  {} Failed to restore MCP config; retaining ownership record: {}",
                    "!".yellow(),
                    path.display()
                );
                tracing::warn!(
                    config_path = %path.display(),
                    error = %error,
                    "Failed to restore MCP config; retaining ownership record"
                );
                result.errors += 1;
                continue;
            }
            Ok(RestoreMcpConfigOutcome::Restored) => {}
        }
        if let Some(locked) = locked.as_deref_mut() {
            locked.remove_record(project_root, &path);
        }
        println!("  {} Restored MCP config: {}", "✔".green(), path.display());
        result.updated += 1;
    }
    Ok(())
}

fn resolve_ownership_record_path(
    store: &McpOwnershipStore,
    project_root: &Path,
    config_id: &str,
    record: &OwnershipRecord,
) -> Result<(crate::mcp::McpAgent, PathBuf)> {
    anyhow::ensure!(
        !record.agent_ids.is_empty(),
        "ownership record has no agent IDs"
    );
    let mut candidate: Option<(crate::mcp::McpAgent, PathBuf)> = None;
    for agent_id in &record.agent_ids {
        let agent = crate::mcp::McpAgent::from_id(agent_id)
            .ok_or_else(|| anyhow::anyhow!("unknown MCP owner ID {agent_id:?}"))?;
        anyhow::ensure!(
            agent.id() == agent_id,
            "non-canonical MCP owner ID {agent_id:?}"
        );
        let path = agent.resolved_config_path(project_root).ok_or_else(|| {
            anyhow::anyhow!("MCP owner {agent_id:?} has no resolvable config path")
        })?;
        let path_id = store.config_path_id(project_root, &path);
        anyhow::ensure!(
            path_id == config_id,
            "ownership record path hash does not match its owner IDs"
        );
        if let Some((_, first_path)) = &candidate {
            anyhow::ensure!(
                store.config_path_id(project_root, first_path) == path_id,
                "MCP owner IDs resolve to different config paths"
            );
        } else {
            candidate = Some((agent, path));
        }
    }
    candidate.ok_or_else(|| anyhow::anyhow!("ownership record has no resolvable config path"))
}

fn read_mcp_config_bytes_if_regular(
    agent: crate::mcp::McpAgent,
    project_root: &Path,
    path: &Path,
) -> Result<Option<Vec<u8>>> {
    crate::mcp::validate_mcp_config_path(agent, project_root, path)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Failed to inspect MCP config: {}", path.display()));
        }
    };
    if metadata.file_type().is_symlink() {
        anyhow::bail!("refusing symlinked MCP config: {}", path.display());
    }
    if !metadata.is_file() {
        anyhow::bail!("MCP config is not a regular file: {}", path.display());
    }
    fs::read(path)
        .with_context(|| format!("Failed to read MCP config: {}", path.display()))
        .map(Some)
}

#[derive(Debug)]
enum RestoreMcpConfigError {
    Validation(anyhow::Error),
    Mutation(anyhow::Error),
}

#[derive(Debug, PartialEq, Eq)]
enum RestoreMcpConfigOutcome {
    Restored,
    Changed,
}

fn remove_generated_mcp_config_with_hook<F>(
    agent: crate::mcp::McpAgent,
    project_root: &Path,
    path: &Path,
    expected_current: Option<&[u8]>,
    before_final_check: F,
) -> Result<RestoreMcpConfigOutcome>
where
    F: FnOnce() -> Result<()>,
{
    before_final_check()?;
    let latest = read_mcp_config_bytes_if_regular(agent, project_root, path)?;
    if latest.as_deref() != expected_current {
        return Ok(RestoreMcpConfigOutcome::Changed);
    }
    fs::remove_file(path)
        .with_context(|| format!("Failed to remove generated MCP config: {}", path.display()))?;
    Ok(RestoreMcpConfigOutcome::Restored)
}

impl From<anyhow::Error> for RestoreMcpConfigError {
    fn from(error: anyhow::Error) -> Self {
        Self::Mutation(error)
    }
}

fn write_staged_mcp_restore_file<F>(
    temp_file: &mut tempfile::NamedTempFile,
    content: &[u8],
    restrict_before_write: F,
) -> Result<()>
where
    F: FnOnce(&Path) -> Result<()>,
{
    restrict_before_write(temp_file.path())?;
    use std::io::Write;
    temp_file
        .write_all(content)
        .context("Failed to write restored MCP config")
}

fn restore_mcp_config_bytes(
    agent: crate::mcp::McpAgent,
    project_root: &Path,
    path: &Path,
    content: &[u8],
    expected_current: Option<&[u8]>,
) -> std::result::Result<RestoreMcpConfigOutcome, RestoreMcpConfigError> {
    restore_mcp_config_bytes_with_hook(agent, project_root, path, content, expected_current, || {
        Ok(())
    })
}

fn restore_mcp_config_bytes_with_hook<F>(
    agent: crate::mcp::McpAgent,
    project_root: &Path,
    path: &Path,
    content: &[u8],
    expected_current: Option<&[u8]>,
    after_staging: F,
) -> std::result::Result<RestoreMcpConfigOutcome, RestoreMcpConfigError>
where
    F: FnOnce() -> Result<()>,
{
    crate::mcp::validate_mcp_config_path(agent, project_root, path)
        .map_err(RestoreMcpConfigError::Validation)?;
    let parent = path.parent().ok_or_else(|| {
        RestoreMcpConfigError::Validation(anyhow::anyhow!("Invalid MCP config path"))
    })?;
    let mut temp_file = tempfile::NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "Failed to create MCP restore temp file in {}",
            parent.display()
        )
    })?;
    write_staged_mcp_restore_file(
        &mut temp_file,
        content,
        crate::mcp::set_restricted_mcp_staging_permissions,
    )?;
    temp_file
        .as_file()
        .sync_all()
        .context("Failed to sync restored MCP config")?;
    after_staging()?;
    let current = read_mcp_config_bytes_if_regular(agent, project_root, path)
        .map_err(RestoreMcpConfigError::Validation)?;
    if current.as_deref() != expected_current {
        return Ok(RestoreMcpConfigOutcome::Changed);
    }
    temp_file
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| {
            format!(
                "Failed to atomically restore MCP config: {}",
                path.display()
            )
        })?;
    Ok(RestoreMcpConfigOutcome::Restored)
}

/// Match MCP agents against CLI/default filter values.
/// Supports canonical IDs (e.g. "codex") and aliases (e.g. "codex-cli"),
/// while preserving legacy substring matching for unknown/custom filters.
fn mcp_agent_matches_filter(agent: crate::mcp::McpAgent, filter: &str) -> bool {
    crate::agent_ids::mcp_filter_matches(agent.id(), filter)
}

/// Revert-only agent selection: CLI `--agents` / `default_agents` filters
/// still apply, but `enabled = false` does NOT exclude the agent. Revert must
/// process disabled agents so stale links and `.bak` backups left behind when
/// an agent was disabled after `apply` are still cleaned up. Apply and clean
/// keep using `agent_selected` and are untouched.
pub fn revert_agent_selected(
    config: &crate::config::Config,
    agent_name: &str,
    options: &SyncOptions,
) -> bool {
    if let Some(ref filter) = options.agents {
        return filter
            .iter()
            .any(|f| crate::agent_ids::sync_filter_matches(agent_name, f));
    }
    if !config.default_agents.is_empty() {
        return config
            .default_agents
            .iter()
            .any(|f| crate::agent_ids::sync_filter_matches(agent_name, f));
    }
    true
}
/// Shared agent selection: CLI --agents > default_agents > all enabled agents.
// `pub` so callers outside the linker (e.g. CLI gates in `main.rs`) reuse the
// apply filter semantics. Revert uses `revert_agent_selected`, which does not
// exclude disabled agents.
pub fn agent_selected(
    config: &crate::config::Config,
    agent_name: &str,
    enabled: bool,
    options: &SyncOptions,
) -> bool {
    if !enabled {
        return false;
    }
    if let Some(ref filter) = options.agents {
        return filter
            .iter()
            .any(|f| crate::agent_ids::sync_filter_matches(agent_name, f));
    }
    if !config.default_agents.is_empty() {
        return config
            .default_agents
            .iter()
            .any(|f| crate::agent_ids::sync_filter_matches(agent_name, f));
    }
    true
}

/// Shared unit-test fixtures for the linker sibling modules (`clean`,
/// `revert`): a single `make_target`/`make_linker` pair instead of one copy
/// per file.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::config::{AgentConfig, SyncType};

    pub(crate) fn make_target(
        source: &str,
        destination: &str,
        sync_type: SyncType,
    ) -> TargetConfig {
        TargetConfig {
            source: source.to_string(),
            destination: destination.to_string(),
            sync_type,
            pattern: None,
            exclude: vec![],
            mappings: vec![],
        }
    }

    pub(crate) fn make_linker(
        project_root: &Path,
        agent_enabled: bool,
        target: TargetConfig,
    ) -> Linker {
        let mut targets = BTreeMap::new();
        targets.insert("target".to_string(), target);

        let agent_config = AgentConfig {
            enabled: agent_enabled,
            description: String::new(),
            targets,
        };

        let mut agents = BTreeMap::new();
        agents.insert("test".to_string(), agent_config);

        let config = Config {
            source_dir: ".agents".to_string(),
            compress_agents_md: false,
            default_agents: vec![],
            agents,
            gitignore: Default::default(),
            mcp: Default::default(),
            mcp_servers: Default::default(),
            plugins: Default::default(),
        };

        let config_path = project_root.join("agentsync.toml");
        Linker::new(config, config_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    #[test]
    fn default_mcp_ownership_data_roots_are_isolated_per_project() {
        let first_project = TempDir::new().unwrap();
        let second_project = TempDir::new().unwrap();
        let make_linker = |project_root: &Path| {
            test_support::make_linker(
                project_root,
                false,
                test_support::make_target(
                    "source",
                    "destination",
                    crate::config::SyncType::Symlink,
                ),
            )
        };

        let first = make_linker(first_project.path());
        let second = make_linker(second_project.path());

        assert_ne!(
            first.mcp_ownership_data_root,
            second.mcp_ownership_data_root
        );
    }

    #[test]
    fn legacy_mcp_warning_requires_an_active_configured_destination() {
        let temp_dir = TempDir::new().unwrap();
        let mut linker = test_support::make_linker(
            temp_dir.path(),
            true,
            test_support::make_target("source", "destination", crate::config::SyncType::Symlink),
        );
        linker.config.mcp_servers.insert(
            "filesystem".to_string(),
            crate::config::McpServerConfig {
                command: Some("fixture-server".to_string()),
                args: Vec::new(),
                env: BTreeMap::new(),
                url: None,
                headers: BTreeMap::new(),
                transport_type: None,
                disabled: false,
            },
        );

        let mut result = crate::mcp::McpSyncResult::default();
        linker.warn_if_legacy_mcp_is_unowned(&mut result, None);
        assert_eq!(
            result.skipped, 0,
            "an unsupported agent has no MCP destination"
        );

        let test_agent = linker.config.agents.remove("test").unwrap();
        linker
            .config
            .agents
            .insert("claude".to_string(), test_agent);
        linker.config.mcp.enabled = false;
        linker.warn_if_legacy_mcp_is_unowned(&mut result, None);
        assert_eq!(
            result.skipped, 0,
            "disabled MCP does not need an ownership journal"
        );

        linker.config.mcp.enabled = true;
        linker
            .config
            .mcp_servers
            .get_mut("filesystem")
            .unwrap()
            .disabled = true;
        linker.warn_if_legacy_mcp_is_unowned(&mut result, None);
        assert_eq!(
            result.skipped, 0,
            "disabled servers have no MCP destination"
        );

        linker
            .config
            .mcp_servers
            .get_mut("filesystem")
            .unwrap()
            .disabled = false;
        linker.warn_if_legacy_mcp_is_unowned(&mut result, None);
        assert_eq!(
            result.skipped, 0,
            "an active MCP server without an existing config entry is not a skipped restore"
        );

        fs::write(temp_dir.path().join(".mcp.json"), "{}")
            .expect("create the existing Claude MCP destination");
        linker.warn_if_legacy_mcp_is_unowned(&mut result, None);
        assert_eq!(
            result.skipped, 1,
            "active MCP destinations without a journal are skipped"
        );
    }

    #[test]
    fn revert_reconciles_write_ahead_record_before_second_apply_write() {
        let temp_dir = TempDir::new().unwrap();
        let project_root_path = temp_dir.path().join("project");
        fs::create_dir_all(&project_root_path).unwrap();
        let project_root = project_root_path.as_path();
        let config_path = project_root.join("agentsync.toml");
        fs::write(&config_path, "").unwrap();
        let data_root = temp_dir.path().join("local-data");
        let linker = test_support::make_linker(
            project_root,
            true,
            test_support::make_target("source", "destination", crate::config::SyncType::Symlink),
        )
        .with_mcp_ownership_data_root_for_tests(data_root.clone());
        let store = McpOwnershipStore::open_at(project_root, &data_root).unwrap();
        let config_path = project_root.join(".mcp.json");
        let mut locked = store.lock().unwrap();
        locked
            .record_before_write(
                project_root,
                &config_path,
                BTreeSet::from(["claude".to_string()]),
                Some("first baseline"),
                "first applied",
            )
            .unwrap();
        locked
            .record_before_write(
                project_root,
                &config_path,
                BTreeSet::from(["claude".to_string()]),
                Some("first applied"),
                "second applied",
            )
            .unwrap();
        locked.persist().unwrap();
        drop(locked);
        fs::write(&config_path, "first applied").unwrap();

        let result = linker.restore_mcp_ownership(false, None).unwrap();

        assert_eq!(fs::read_to_string(&config_path).unwrap(), "first baseline");
        assert_eq!(result.updated, 1);
        assert!(store.read_existing().unwrap().is_none());
    }

    #[test]
    fn restore_rejects_unregistered_serialized_owner_id() {
        let temp_dir = TempDir::new().unwrap();
        let project_root_path = temp_dir.path().join("project");
        fs::create_dir_all(&project_root_path).unwrap();
        let project_root = project_root_path.as_path();
        let config_path = project_root.join("agentsync.toml");
        fs::write(&config_path, "").unwrap();
        let data_root = temp_dir.path().join("local-data");
        let linker = test_support::make_linker(
            project_root,
            true,
            test_support::make_target("source", "destination", crate::config::SyncType::Symlink),
        )
        .with_mcp_ownership_data_root_for_tests(data_root.clone());
        let store = McpOwnershipStore::open_at(project_root, &data_root).unwrap();
        let config_path = project_root.join(".mcp.json");
        fs::write(&config_path, "applied config").unwrap();
        let mut locked = store.lock().unwrap();
        locked
            .record_before_write(
                project_root,
                &config_path,
                BTreeSet::from(["claude".to_string()]),
                Some("original config"),
                "applied config",
            )
            .unwrap();
        let config_id = store.config_path_id(project_root, &config_path);
        locked
            .manifest_mut()
            .configs
            .get_mut(&config_id)
            .unwrap()
            .agent_ids = BTreeSet::from(["unknown-agent".to_string()]);
        locked.persist().unwrap();
        drop(locked);

        let result = linker.restore_mcp_ownership(false, None).unwrap();

        assert_eq!(result.errors, 1);
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "applied config");
        assert!(store.read_existing().unwrap().is_some());
    }

    #[test]
    fn staged_mcp_restore_preserves_edit_that_arrives_before_publish() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let config_path = project_root.join(".mcp.json");
        let applied = b"applied config";
        let original = b"original config";
        let external_edit = b"external edit";
        fs::write(&config_path, applied).unwrap();

        let result = restore_mcp_config_bytes_with_hook(
            crate::mcp::McpAgent::ClaudeCode,
            project_root,
            &config_path,
            original,
            Some(applied),
            || fs::write(&config_path, external_edit).map_err(Into::into),
        )
        .unwrap();

        assert_eq!(result, RestoreMcpConfigOutcome::Changed);
        assert_eq!(fs::read(&config_path).unwrap(), external_edit);
    }

    #[test]
    fn staged_mcp_restore_restricts_permissions_before_writing_snapshot_bytes() {
        let temp_dir = TempDir::new().unwrap();
        let content = b"snapshot credentials";
        let mut temp_file = tempfile::NamedTempFile::new_in(temp_dir.path()).unwrap();
        let mut restriction_checked = false;

        write_staged_mcp_restore_file(&mut temp_file, content, |staging_path| {
            restriction_checked = true;
            assert!(staging_path.exists());
            assert_eq!(fs::metadata(staging_path)?.len(), 0);
            Ok(())
        })
        .unwrap();

        assert!(restriction_checked);
        assert_eq!(fs::read(temp_file.path()).unwrap(), content);
    }

    #[test]
    fn generated_mcp_remove_preserves_edit_detected_by_final_check() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let config_path = project_root.join(".mcp.json");
        let applied = b"generated config";
        let external_edit = b"external edit";
        fs::write(&config_path, applied).unwrap();

        let result = remove_generated_mcp_config_with_hook(
            crate::mcp::McpAgent::ClaudeCode,
            project_root,
            &config_path,
            Some(applied),
            || fs::write(&config_path, external_edit).map_err(Into::into),
        )
        .unwrap();

        assert_eq!(result, RestoreMcpConfigOutcome::Changed);
        assert_eq!(fs::read(&config_path).unwrap(), external_edit);
    }

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
    fn sync_refuses_existing_and_dangling_symlink_compressed_outputs() {
        use std::os::unix::fs::symlink;

        for dangling in [false, true] {
            let temp_dir = TempDir::new().unwrap();
            let project_root = temp_dir.path();
            let agents_dir = project_root.join(".agents");
            fs::create_dir_all(&agents_dir).unwrap();
            fs::write(agents_dir.join("AGENTS.md"), "# current instructions\n").unwrap();

            let victim = project_root.join(if dangling {
                "dangling-target.md"
            } else {
                "existing-target.md"
            });
            if !dangling {
                fs::write(&victim, "do not overwrite this file\n").unwrap();
            }
            symlink(&victim, agents_dir.join("AGENTS.compact.md")).unwrap();

            let config_path = agents_dir.join("agentsync.toml");
            fs::write(
                &config_path,
                r#"
                    source_dir = "."
                    compress_agents_md = true

                    [agents.test]
                    enabled = true

                    [agents.test.targets.main]
                    source = "AGENTS.md"
                    destination = "TEST.md"
                    type = "symlink"
                "#,
            )
            .unwrap();
            let config = Config::load(&config_path).unwrap();
            let linker = Linker::new(config, config_path);

            let result = linker.sync(&SyncOptions::default()).unwrap();

            if dangling {
                assert!(!victim.exists(), "a dangling target must not be created");
            } else {
                assert_eq!(
                    fs::read_to_string(&victim).unwrap(),
                    "do not overwrite this file\n"
                );
            }
            assert_eq!(
                result.errors, 1,
                "compressed output symlinks must be refused"
            );
            assert!(
                agents_dir.join("AGENTS.compact.md").is_symlink(),
                "the attacker-controlled link must remain untouched"
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn sync_does_not_modify_other_hardlinks_to_compressed_output() {
        use std::os::unix::fs::MetadataExt;

        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let agents_dir = project_root.join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(agents_dir.join("AGENTS.md"), "# current instructions\n").unwrap();
        let other_link = project_root.join("important-user-file.md");
        let compressed_output = agents_dir.join("AGENTS.compact.md");
        fs::write(&other_link, "do not modify the shared inode\n").unwrap();
        fs::hard_link(&other_link, &compressed_output).unwrap();

        let config_path = agents_dir.join("agentsync.toml");
        fs::write(
            &config_path,
            r#"
                source_dir = "."
                compress_agents_md = true

                [agents.test]
                enabled = true

                [agents.test.targets.main]
                source = "AGENTS.md"
                destination = "TEST.md"
                type = "symlink"
            "#,
        )
        .unwrap();
        let config = Config::load(&config_path).unwrap();
        let linker = Linker::new(config, config_path);

        let result = linker.sync(&SyncOptions::default()).unwrap();

        assert_eq!(result.errors, 0);
        assert_eq!(
            fs::read_to_string(&other_link).unwrap(),
            "do not modify the shared inode\n",
            "regenerating compressed output must not overwrite another hard link"
        );
        assert_eq!(
            fs::read_to_string(&compressed_output).unwrap(),
            "# current instructions\n"
        );
        assert_ne!(
            fs::metadata(&other_link).unwrap().ino(),
            fs::metadata(&compressed_output).unwrap().ino(),
            "the generated output should no longer share the user's inode"
        );
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
    fn compressed_agents_output_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command;

        const CHILD_PATH_ENV: &str = "AGENTSYNC_COMPRESSED_OUTPUT_UMASK_TEST_PATH";

        if let Some(path) = std::env::var_os(CHILD_PATH_ENV) {
            let project_root = Path::new(&path);
            let agents_dir = project_root.join(".agents");
            fs::create_dir_all(&agents_dir).unwrap();
            let workspace_dir = project_root.join("workspace");
            fs::create_dir_all(&workspace_dir).unwrap();
            let config_path = agents_dir.join("agentsync.toml");
            fs::write(&config_path, "source_dir = \".\"\n").unwrap();

            let source = project_root.join("AGENTS.md");
            fs::write(&source, "# instructions\n").unwrap();
            let dest = project_root.join("workspace/AGENTS.compact.md");
            let linker = Linker::new(Config::load(&config_path).unwrap(), config_path);

            // SAFETY: This subprocess runs only this test, so changing its umask
            // cannot affect other test threads or processes.
            unsafe {
                umask(0o777);
            }

            linker
                .write_compressed_agents_md(&source, &dest, &SyncOptions::default())
                .unwrap();

            assert_eq!(
                fs::metadata(dest).unwrap().permissions().mode() & 0o777,
                0o600,
                "compressed AGENTS.md output must be exactly owner-only despite umask 0777"
            );
            return;
        }

        let temp_dir = TempDir::new().unwrap();
        let test_name = std::thread::current().name().unwrap().to_owned();
        let output = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(test_name)
            .arg("--nocapture")
            .env(CHILD_PATH_ENV, temp_dir.path())
            .env(
                "LLVM_PROFILE_FILE",
                temp_dir.path().join("compressed-output-%p.profraw"),
            )
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "isolated umask test subprocess failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    unsafe extern "C" {
        fn umask(mask: u32) -> u32;
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
        assert_eq!(result.skipped, 1);
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
