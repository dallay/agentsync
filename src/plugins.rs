//! Repository-owned plugin discovery, locking, and materialization.
//!
//! This module intentionally does not invoke vendor CLIs or execute anything from a plugin
//! source.  It only reads manifests, validates content, copies skills, and returns MCP
//! declarations for the existing configuration generator.

use crate::config::McpServerConfig;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::future::Future;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tempfile::{NamedTempFile, TempDir};
use toml_edit::{DocumentMut, Item, Table, value};
use walkdir::WalkDir;

const PLUGIN_LOCK_SCHEMA_VERSION: &str = "v1";
const DEFAULT_PLUGIN_LOCKFILE: &str = "plugins.lock.toml";

fn default_plugin_lockfile() -> String {
    DEFAULT_PLUGIN_LOCKFILE.to_string()
}

/// Project-level plugin configuration from `agentsync.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginsConfig {
    /// Enable repository-owned plugin materialization. Disabled by default for compatibility.
    #[serde(default)]
    pub enabled: bool,
    /// Lockfile path relative to the config file's directory.
    #[serde(default = "default_plugin_lockfile")]
    pub lockfile: String,
    /// Named marketplace sources.
    #[serde(default)]
    pub marketplaces: BTreeMap<String, MarketplaceConfig>,
    /// Explicitly selected plugins.
    #[serde(default)]
    pub selections: Vec<PluginSelection>,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            lockfile: default_plugin_lockfile(),
            marketplaces: BTreeMap::new(),
            selections: Vec::new(),
        }
    }
}

/// A marketplace declaration. `reference` is used only by explicit add/update operations;
/// apply reads the immutable revision recorded in the lockfile.
#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceConfig {
    pub source: String,
    #[serde(default)]
    pub reference: Option<String>,
}

/// A selected plugin in a named marketplace.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct PluginSelection {
    pub marketplace: String,
    pub plugin: String,
}

impl PluginSelection {
    pub fn key(&self) -> String {
        format!("{}/{}", self.marketplace, self.plugin)
    }
}

/// The source kind recorded in a plugin lock entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LockedSourceKind {
    Local,
    Git,
}

/// An immutable source identity used during apply.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedSource {
    pub kind: LockedSourceKind,
    /// Relative local path or Git repository URL.
    pub location: String,
    /// Full Git commit SHA or `local:<tree-sha256>`.
    pub revision: String,
}

/// Public name for the immutable plugin source identity used by the project lockfile.
pub type PluginSource = LockedSource;

/// Provenance recorded for a materialized plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginProvenance {
    pub marketplace_manifest: String,
    pub plugin_path: String,
    pub resolved_revision: String,
    pub content_sha256: String,
}

/// A skill selected from a locked plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedSkill {
    pub id: String,
    pub path: String,
    pub content_sha256: String,
}

/// A plugin entry in the project lockfile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedPlugin {
    pub marketplace: String,
    pub plugin: String,
    pub version: Option<String>,
    pub source: LockedSource,
    pub content_sha256: String,
    pub skills: Vec<LockedSkill>,
    pub mcp_servers: Vec<String>,
    pub unsupported_components: Vec<String>,
    pub provenance: PluginProvenance,
}

impl LockedPlugin {
    pub fn key(&self) -> String {
        format!("{}/{}", self.marketplace, self.plugin)
    }
}

/// Deterministic project plugin lockfile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginLock {
    pub schema_version: String,
    pub plugins: BTreeMap<String, LockedPlugin>,
}

impl Default for PluginLock {
    fn default() -> Self {
        Self {
            schema_version: PLUGIN_LOCK_SCHEMA_VERSION.to_string(),
            plugins: BTreeMap::new(),
        }
    }
}

impl PluginLock {
    pub fn load(path: &Path) -> Result<Self> {
        // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- callers provide the lockfile path explicitly
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read plugin lockfile: {}", path.display()))?;
        let lock: Self = toml::from_str(&content)
            .with_context(|| format!("failed to parse plugin lockfile: {}", path.display()))?;
        lock.validate()?;
        Ok(lock)
    }

    pub fn save_atomic(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let body = toml::to_string_pretty(self).context("failed to serialize plugin lockfile")?;
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("plugin lockfile has no parent"))?;
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to create lockfile directory: {}", parent.display())
        })?;
        write_atomic_file(path, body.as_bytes())
            .with_context(|| format!("failed to replace plugin lockfile: {}", path.display()))
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == PLUGIN_LOCK_SCHEMA_VERSION,
            "unsupported plugin lock schema: {}",
            self.schema_version
        );
        for (key, plugin) in &self.plugins {
            validate_identifier("marketplace", &plugin.marketplace)?;
            validate_identifier("plugin", &plugin.plugin)?;
            ensure!(
                key == &plugin.key(),
                "plugin lock key does not match plugin identity: {key}"
            );
            validate_source(&plugin.source)?;
            validate_hash("content_sha256", &plugin.content_sha256)?;
            ensure!(
                plugin.provenance.content_sha256 == plugin.content_sha256,
                "plugin provenance hash does not match lock entry: {key}"
            );
            ensure!(
                plugin.provenance.resolved_revision == plugin.source.revision,
                "plugin provenance revision does not match lock entry: {key}"
            );
            let mut skill_ids = BTreeSet::new();
            for skill in &plugin.skills {
                validate_identifier("skill", &skill.id)?;
                ensure!(
                    skill_ids.insert(&skill.id),
                    "duplicate plugin skill: {}",
                    skill.id
                );
                validate_relative_path(&skill.path)?;
                validate_hash("skill content_sha256", &skill.content_sha256)?;
            }
            for server in &plugin.mcp_servers {
                ensure!(
                    !server.is_empty(),
                    "plugin MCP server name must not be empty"
                );
            }
            for component in &plugin.unsupported_components {
                ensure!(
                    !component.is_empty(),
                    "unsupported component name must not be empty"
                );
            }
            ensure!(
                plugin.unsupported_components.is_empty(),
                "plugin {} contains unsupported components: {}",
                key,
                plugin.unsupported_components.join(", ")
            );
            validate_relative_path(&plugin.provenance.marketplace_manifest)?;
            validate_relative_path(&plugin.provenance.plugin_path)?;
        }
        Ok(())
    }
}

/// The result of applying project-owned plugins.
#[derive(Debug, Default)]
pub struct PluginApplyResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub removed: usize,
    pub errors: usize,
    pub mcp_servers: BTreeMap<String, McpServerConfig>,
}

/// Repository-owned plugin operations.
#[derive(Clone)]
pub struct PluginManager {
    project_root: PathBuf,
    config_path: PathBuf,
    config: PluginsConfig,
}

impl PluginManager {
    pub fn new(project_root: PathBuf, config_path: PathBuf, config: PluginsConfig) -> Self {
        Self {
            project_root,
            config_path,
            config,
        }
    }

    pub fn lock_path(&self) -> Result<PathBuf> {
        validate_relative_path(&self.config.lockfile)?;
        Ok(self
            .config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&self.config.lockfile))
    }

    pub fn apply(&self, dry_run: bool) -> Result<PluginApplyResult> {
        if !self.config.enabled || self.config.selections.is_empty() {
            return Ok(PluginApplyResult::default());
        }

        let lock_path = self.lock_path()?;
        let lock = PluginLock::load(&lock_path).with_context(|| {
            format!(
                "plugin selections require an immutable lockfile; run `agentsync plugin add` or `agentsync plugin update` first ({})",
                lock_path.display()
            )
        })?;
        let selections: BTreeSet<_> = self
            .config
            .selections
            .iter()
            .map(PluginSelection::key)
            .collect();
        let mut result = PluginApplyResult::default();
        let mut pending_skills = Vec::new();

        for key in selections {
            let locked = lock
                .plugins
                .get(&key)
                .with_context(|| format!("plugin selection is missing from lockfile: {key}"))?;
            ensure!(
                locked.unsupported_components.is_empty(),
                "plugin {key} contains unsupported components: {}",
                locked.unsupported_components.join(", ")
            );
            let source = self.materialize_source(&locked.source)?;
            let discovered = discover_plugin(source.root(), &locked.marketplace, &locked.plugin)?;
            ensure!(
                discovered.content_sha256 == locked.content_sha256,
                "plugin content drift detected for {key}: expected {}, got {}",
                locked.content_sha256,
                discovered.content_sha256
            );
            ensure!(
                discovered.unsupported_components.is_empty(),
                "plugin {key} contains unsupported components: {}",
                discovered.unsupported_components.join(", ")
            );
            let discovered_skill_ids: BTreeSet<_> = discovered
                .skills
                .iter()
                .map(|skill| skill.id.as_str())
                .collect();
            let locked_skill_ids: BTreeSet<_> = locked
                .skills
                .iter()
                .map(|skill| skill.id.as_str())
                .collect();
            ensure!(
                discovered_skill_ids == locked_skill_ids,
                "plugin skill set drift detected for {key}"
            );
            let plugin_mcp =
                discovered.namespaced_mcp_servers(&locked.marketplace, &locked.plugin)?;
            let discovered_mcp_names: Vec<_> = discovered.mcp_servers.keys().cloned().collect();
            ensure!(
                discovered_mcp_names == locked.mcp_servers,
                "plugin MCP declaration drift detected for {key}"
            );
            for (name, server) in plugin_mcp {
                if result.mcp_servers.insert(name.clone(), server).is_some() {
                    bail!("duplicate plugin MCP server: {name}");
                }
            }

            for skill in &discovered.skills {
                let locked_skill = locked
                    .skills
                    .iter()
                    .find(|candidate| candidate.id == skill.id)
                    .with_context(|| {
                        format!("skill missing from plugin lock: {key}/{}", skill.id)
                    })?;
                ensure!(
                    locked_skill.content_sha256 == skill.content_sha256,
                    "skill content drift detected for {key}/{}",
                    skill.id
                );
                validate_skill_for_materialization(&self.project_root, locked, skill)?;
                if dry_run {
                    result.updated += 1;
                } else {
                    pending_skills.push((locked.clone(), skill.clone()));
                }
            }
            if dry_run && discovered.skills.is_empty() {
                result.skipped += 1;
            }
        }

        if dry_run {
            return Ok(result);
        }

        let skills = pending_skills
            .iter()
            .map(|(_, skill)| skill.clone())
            .collect::<Vec<_>>();
        let transaction = ApplyTransaction::begin(&self.project_root, &skills)?;
        for (locked, skill) in pending_skills {
            if let Err(error) = materialize_skill(&self.project_root, &locked, &skill, &mut result)
            {
                let rollback = transaction.rollback();
                return match rollback {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(anyhow::anyhow!(
                        "plugin apply failed: {error}; rollback failed: {rollback_error}"
                    )),
                };
            }
        }

        Ok(result)
    }

    /// Resolve and lock a selected plugin from the configured marketplace.
    pub fn add(&self, selection: &PluginSelection) -> Result<PluginApplyResult> {
        let manager = self.clone();
        let selection = selection.clone();
        run_async(move || async move { manager.add_async(&selection).await })
    }

    pub fn update(&self, selection: &PluginSelection) -> Result<PluginApplyResult> {
        let manager = self.clone();
        let selection = selection.clone();
        run_async(move || async move { manager.update_async(&selection).await })
    }

    pub async fn add_async(&self, selection: &PluginSelection) -> Result<PluginApplyResult> {
        self.lock_selection(selection).await
    }

    pub async fn update_async(&self, selection: &PluginSelection) -> Result<PluginApplyResult> {
        self.lock_selection(selection).await
    }

    pub fn load_lock(&self) -> Result<PluginLock> {
        PluginLock::load(&self.lock_path()?)
    }

    pub fn remove(&self, selection: &PluginSelection, dry_run: bool) -> Result<PluginApplyResult> {
        let lock_path = self.lock_path()?;
        let original_lock = PluginLock::load(&lock_path)?;
        let key = selection.key();
        let locked = original_lock
            .plugins
            .get(&key)
            .cloned()
            .with_context(|| format!("plugin is not locked: {key}"))?;
        let mut result = PluginApplyResult::default();
        if dry_run {
            result.removed = locked.skills.len();
            return Ok(result);
        }

        let target_root = self.project_root.join(".agents/skills");
        let registry_path = target_root.join("registry.json");
        let registry = crate::skills::registry::read_registry(&registry_path).ok();
        let mut targets = Vec::new();
        let mut registry_ids = Vec::new();
        let owner = crate::skills::registry::PluginOwner {
            marketplace: selection.marketplace.clone(),
            plugin: selection.plugin.clone(),
            revision: locked.source.revision.clone(),
        };
        for skill in &locked.skills {
            let target = target_root.join(&skill.id);
            let metadata = fs::symlink_metadata(&target).ok();
            let entry = registry
                .as_ref()
                .and_then(|registry| registry.skills.as_ref())
                .and_then(|skills| skills.get(&skill.id));
            let owners = entry.map(entry_plugin_owners).unwrap_or_default();
            let owned = owners.contains(&owner);
            if let Some(metadata) = metadata {
                ensure!(
                    owned,
                    "refusing to remove unmanaged skill: {}",
                    target.display()
                );
                if owners.len() == 1 {
                    ensure!(
                        !metadata.file_type().is_symlink() && metadata.is_dir(),
                        "refusing to remove unsafe skill destination: {}",
                        target.display()
                    );
                    targets.push((skill.id.clone(), target));
                }
            }
            if owned {
                registry_ids.push(skill.id.clone());
            }
        }

        let original_config = fs::read(&self.config_path).with_context(|| {
            format!(
                "failed to read config before removing plugin: {}",
                self.config_path.display()
            )
        })?;
        let original_registry = if registry_path.is_file() {
            Some(fs::read(&registry_path).with_context(|| {
                format!("failed to read skill registry: {}", registry_path.display())
            })?)
        } else {
            None
        };
        let backup = if targets.is_empty() {
            None
        } else {
            fs::create_dir_all(&target_root).with_context(|| {
                format!("failed to create skill root: {}", target_root.display())
            })?;
            Some(TempDir::new_in(&target_root).with_context(|| {
                format!(
                    "failed to create temporary plugin removal directory in {}",
                    target_root.display()
                )
            })?)
        };
        let mut backups = Vec::new();
        let operation = (|| -> Result<()> {
            if let Some(backup) = &backup {
                for (id, target) in &targets {
                    let backup_path = backup.path().join(id);
                    fs::rename(target, &backup_path).with_context(|| {
                        format!(
                            "failed to stage plugin-owned skill removal: {}",
                            target.display()
                        )
                    })?;
                    backups.push((target.clone(), backup_path));
                }
            }
            let registry_ids = registry_ids.iter().map(String::as_str).collect::<Vec<_>>();
            remove_plugin_owner_entries_atomic(&registry_path, &registry_ids, &owner)?;
            remove_selection_from_config(&self.config_path, selection)?;
            let mut lock = original_lock.clone();
            lock.plugins.remove(&key);
            lock.save_atomic(&lock_path)?;
            Ok(())
        })();
        if let Err(error) = operation {
            let rollback = (|| -> Result<()> {
                for (target, backup_path) in backups.iter().rev() {
                    if target.exists() {
                        remove_path_safely(target)?;
                    }
                    fs::rename(backup_path, target).with_context(|| {
                        format!("failed to restore plugin-owned skill: {}", target.display())
                    })?;
                }
                if let Some(original_registry) = &original_registry {
                    write_atomic_file(&registry_path, original_registry)?;
                } else if registry_path.exists() {
                    fs::remove_file(&registry_path)?;
                }
                write_atomic_file(&self.config_path, &original_config)?;
                original_lock.save_atomic(&lock_path)?;
                Ok(())
            })();
            if let Err(rollback_error) = rollback {
                return Err(anyhow::anyhow!(
                    "plugin removal failed: {error}; rollback failed: {rollback_error}"
                ));
            }
            return Err(error);
        }
        result.removed = targets.len();
        Ok(result)
    }

    async fn lock_selection(&self, selection: &PluginSelection) -> Result<PluginApplyResult> {
        validate_selection(selection)?;
        ensure!(
            self.config.enabled,
            "plugin materialization is disabled; set [plugins].enabled = true first"
        );
        let marketplace = self
            .config
            .marketplaces
            .get(&selection.marketplace)
            .with_context(|| format!("unknown plugin marketplace: {}", selection.marketplace))?;
        let source = resolve_marketplace_source_async(&self.config_path, marketplace, true).await?;
        let discovered = discover_plugin(source.root(), &selection.marketplace, &selection.plugin)?;
        ensure!(
            discovered.unsupported_components.is_empty(),
            "plugin {} contains unsupported components: {}",
            selection.key(),
            discovered.unsupported_components.join(", ")
        );
        let locked =
            discovered.to_locked_plugin(&selection.marketplace, &selection.plugin, &source)?;
        self.cache_marketplace_source(&source)?;
        let lock_path = self.lock_path()?;
        let previous_lock = if lock_path.exists() {
            Some(PluginLock::load(&lock_path)?)
        } else {
            None
        };
        let original_config = if self.config.selections.contains(selection) {
            None
        } else {
            Some(fs::read(&self.config_path).with_context(|| {
                format!(
                    "failed to read config before adding plugin selection: {}",
                    self.config_path.display()
                )
            })?)
        };
        let mut lock = previous_lock.clone().unwrap_or_default();
        lock.plugins.insert(locked.key(), locked);
        lock.save_atomic(&lock_path)?;
        if original_config.is_some()
            && let Err(error) = add_selection_to_config(&self.config_path, selection)
        {
            rollback_plugin_lock(&lock_path, previous_lock.as_ref())?;
            return Err(error);
        }
        let mut apply_config = self.config.clone();
        if !apply_config.selections.contains(selection) {
            apply_config.selections.push(selection.clone());
        }
        let apply_manager = Self::new(
            self.project_root.clone(),
            self.config_path.clone(),
            apply_config,
        );
        match apply_manager.apply(false) {
            Ok(result) => Ok(result),
            Err(error) => {
                rollback_plugin_lock(&lock_path, previous_lock.as_ref())?;
                if let Some(original) = original_config {
                    write_atomic_file(&self.config_path, &original).with_context(|| {
                        format!(
                            "failed to roll back plugin selection config: {}",
                            self.config_path.display()
                        )
                    })?;
                }
                Err(error)
            }
        }
    }

    fn git_source_cache_path(&self, source: &LockedSource) -> Result<PathBuf> {
        ensure!(
            source.kind == LockedSourceKind::Git,
            "Git source cache requested for a non-Git plugin source"
        );
        let mut hasher = Sha256::new();
        hasher.update(source.location.as_bytes());
        let repository_digest = format_digest(hasher.finalize());
        let parent = self.config_path.parent().unwrap_or_else(|| Path::new("."));
        Ok(parent.join(".agentsync-plugin-sources").join(format!(
            "{}-{}",
            &repository_digest[..16],
            source.revision
        )))
    }

    fn cache_marketplace_source(&self, source: &ResolvedMarketplaceSource) -> Result<()> {
        if source.locked_source.kind != LockedSourceKind::Git {
            return Ok(());
        }
        let destination = self.git_source_cache_path(&source.locked_source)?;
        if let Ok(metadata) = fs::symlink_metadata(&destination) {
            ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "refusing to use symlinked Git plugin source snapshot: {}",
                destination.display()
            );
            return Ok(());
        }
        let parent = destination
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Git plugin source cache has no parent"))?;
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create Git plugin source cache directory: {}",
                parent.display()
            )
        })?;
        let staging = TempDir::new_in(parent).with_context(|| {
            format!(
                "failed to create temporary Git plugin source cache in {}",
                parent.display()
            )
        })?;
        let staged = staging.path().join("source");
        copy_directory_without_symlinks(source.root(), &staged)?;
        fs::rename(&staged, &destination).with_context(|| {
            format!(
                "failed to materialize Git plugin source snapshot: {}",
                destination.display()
            )
        })?;
        Ok(())
    }

    fn materialize_source(&self, source: &LockedSource) -> Result<ResolvedSource> {
        match source.kind {
            LockedSourceKind::Local => {
                ensure!(
                    !is_absolute_path(&source.location)
                        && !source.location.contains("://")
                        && !source.location.contains(':'),
                    "locked local plugin source must be a relative path: {}",
                    source.location
                );
                let root = self
                    .config_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(&source.location)
                    .canonicalize()
                    .with_context(|| {
                        format!("plugin source is unavailable: {}", source.location)
                    })?;
                ensure!(
                    root.is_dir(),
                    "plugin local source is not a directory: {}",
                    root.display()
                );
                let actual = format!("local:{}", hash_tree(&root)?);
                ensure!(
                    actual == source.revision,
                    "local plugin source drift detected: {}",
                    root.display()
                );
                Ok(ResolvedSource { root })
            }
            LockedSourceKind::Git => {
                let root = self.git_source_cache_path(source)?;
                ensure!(
                    root.is_dir(),
                    "offline Git plugin source snapshot is unavailable: {} (run `agentsync plugin update` first)",
                    root.display()
                );
                Ok(ResolvedSource { root })
            }
        }
    }
}

struct ResolvedSource {
    root: PathBuf,
}

impl ResolvedSource {
    fn root(&self) -> &Path {
        &self.root
    }
}

#[derive(Debug, Clone)]
struct DiscoveredSkill {
    id: String,
    path: PathBuf,
    relative_path: String,
    content_sha256: String,
}

struct ApplyTransaction {
    target_root: PathBuf,
    registry_path: PathBuf,
    target_root_existed: bool,
    original_registry: Option<Vec<u8>>,
    _backup: TempDir,
    snapshots: Vec<(PathBuf, Option<PathBuf>)>,
}

impl ApplyTransaction {
    fn begin(project_root: &Path, skills: &[DiscoveredSkill]) -> Result<Self> {
        let target_root = project_root.join(".agents/skills");
        let target_root_metadata = fs::symlink_metadata(&target_root).ok();
        let target_root_existed = target_root_metadata.is_some();
        if let Some(metadata) = &target_root_metadata {
            ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "refusing to materialize skills through a symlinked root: {}",
                target_root.display()
            );
        }
        let registry_path = target_root.join("registry.json");
        let original_registry = if registry_path.is_file() {
            Some(fs::read(&registry_path).with_context(|| {
                format!("failed to read skill registry: {}", registry_path.display())
            })?)
        } else {
            None
        };
        let backup = TempDir::new().context("failed to create plugin apply rollback directory")?;
        let mut snapshots = Vec::new();
        let mut skill_ids = BTreeSet::new();
        for skill in skills {
            if !skill_ids.insert(&skill.id) {
                continue;
            }
            let target = target_root.join(&skill.id);
            let metadata = fs::symlink_metadata(&target).ok();
            let backup_path = if let Some(metadata) = metadata {
                ensure!(
                    metadata.is_dir() && !metadata.file_type().is_symlink(),
                    "refusing to snapshot unsafe skill destination: {}",
                    target.display()
                );
                let backup_path = backup.path().join(&skill.id);
                copy_directory_without_symlinks(&target, &backup_path)?;
                Some(backup_path)
            } else {
                None
            };
            snapshots.push((target, backup_path));
        }
        Ok(Self {
            target_root,
            registry_path,
            target_root_existed,
            original_registry,
            _backup: backup,
            snapshots,
        })
    }

    fn rollback(&self) -> Result<()> {
        for (target, backup_path) in self.snapshots.iter().rev() {
            if fs::symlink_metadata(target).is_ok() {
                remove_path_safely(target)?;
            }
            if let Some(backup_path) = backup_path {
                copy_directory_without_symlinks(backup_path, target)?;
            }
        }
        if let Some(original_registry) = &self.original_registry {
            write_atomic_file(&self.registry_path, original_registry)?;
        } else if fs::symlink_metadata(&self.registry_path).is_ok() {
            remove_path_safely(&self.registry_path)?;
        }
        if !self.target_root_existed
            && fs::symlink_metadata(&self.target_root)
                .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
            && fs::read_dir(&self.target_root)?.next().is_none()
        {
            fs::remove_dir(&self.target_root)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct DiscoveredPlugin {
    version: Option<String>,
    plugin_path: String,
    marketplace_manifest: String,
    content_sha256: String,
    skills: Vec<DiscoveredSkill>,
    mcp_servers: BTreeMap<String, McpServerConfig>,
    unsupported_components: Vec<String>,
}

impl DiscoveredPlugin {
    fn to_locked_plugin(
        &self,
        marketplace: &str,
        plugin: &str,
        source: &ResolvedMarketplaceSource,
    ) -> Result<LockedPlugin> {
        let skills = self
            .skills
            .iter()
            .map(|skill| LockedSkill {
                id: skill.id.clone(),
                path: skill.relative_path.clone(),
                content_sha256: skill.content_sha256.clone(),
            })
            .collect();
        let mcp_servers = self.mcp_servers.keys().cloned().collect::<Vec<_>>();
        let provenance = PluginProvenance {
            marketplace_manifest: self.marketplace_manifest.clone(),
            plugin_path: self.plugin_path.clone(),
            resolved_revision: source.locked_source.revision.clone(),
            content_sha256: self.content_sha256.clone(),
        };
        Ok(LockedPlugin {
            marketplace: marketplace.to_string(),
            plugin: plugin.to_string(),
            version: self.version.clone(),
            source: source.locked_source.clone(),
            content_sha256: self.content_sha256.clone(),
            skills,
            mcp_servers,
            unsupported_components: self.unsupported_components.clone(),
            provenance,
        })
    }

    fn namespaced_mcp_servers(
        &self,
        marketplace: &str,
        plugin: &str,
    ) -> Result<BTreeMap<String, McpServerConfig>> {
        let mut result = BTreeMap::new();
        for (name, server) in &self.mcp_servers {
            let key = format!("plugin/{marketplace}/{plugin}/{name}");
            ensure!(
                result.insert(key.clone(), server.clone()).is_none(),
                "duplicate plugin MCP server: {key}"
            );
        }
        Ok(result)
    }
}

struct ResolvedMarketplaceSource {
    root: PathBuf,
    locked_source: LockedSource,
    temp: Option<TempDir>,
}

impl ResolvedMarketplaceSource {
    fn root(&self) -> &Path {
        &self.root
    }
}

impl Drop for ResolvedMarketplaceSource {
    fn drop(&mut self) {
        let _ = self.temp.take();
    }
}

#[cfg(test)]
fn resolve_marketplace_source(
    config_path: &Path,
    marketplace: &MarketplaceConfig,
    allow_network: bool,
) -> Result<ResolvedMarketplaceSource> {
    let config_path = config_path.to_path_buf();
    let marketplace = marketplace.clone();
    run_async(move || async move {
        resolve_marketplace_source_async(&config_path, &marketplace, allow_network).await
    })
}

async fn resolve_marketplace_source_async(
    config_path: &Path,
    marketplace: &MarketplaceConfig,
    allow_network: bool,
) -> Result<ResolvedMarketplaceSource> {
    let source = marketplace.source.trim();
    ensure!(
        !source.is_empty(),
        "plugin marketplace source must not be empty"
    );
    if source.starts_with("file://") {
        bail!("file:// plugin marketplace sources are not supported");
    }
    if is_local_source(source) {
        ensure!(
            !is_absolute_path(source) && !source.contains(':'),
            "absolute plugin marketplace paths are not allowed"
        );
        let root = config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(source)
            .canonicalize()
            .with_context(|| format!("plugin marketplace source is unavailable: {source}"))?;
        ensure!(
            root.is_dir(),
            "plugin marketplace source is not a directory: {}",
            root.display()
        );
        let revision = format!("local:{}", hash_tree(&root)?);
        return Ok(ResolvedMarketplaceSource {
            root,
            locked_source: LockedSource {
                kind: LockedSourceKind::Local,
                location: source.to_string(),
                revision: revision.clone(),
            },
            temp: None,
        });
    }

    ensure!(allow_network, "network access is disabled during apply");
    let reference = marketplace
        .reference
        .as_deref()
        .filter(|reference| !reference.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Git plugin marketplace requires a reference"))?;
    let revision = resolve_git_reference(source, reference).await?;
    let archive = github_archive_url(source, &revision)?;
    let temp = crate::skills::install::fetch_and_unpack_to_tempdir(&archive)
        .await
        .context("failed to fetch plugin marketplace archive")?;
    Ok(ResolvedMarketplaceSource {
        root: temp.path().to_path_buf(),
        locked_source: LockedSource {
            kind: LockedSourceKind::Git,
            location: source.to_string(),
            revision: revision.clone(),
        },
        temp: Some(temp),
    })
}

fn discover_plugin(root: &Path, marketplace: &str, plugin_name: &str) -> Result<DiscoveredPlugin> {
    validate_identifier("marketplace", marketplace)?;
    validate_identifier("plugin", plugin_name)?;
    let manifest_candidates = [
        (
            ".agents/plugins/marketplace.json",
            root.join(".agents/plugins/marketplace.json"),
        ),
        (
            ".claude-plugin/marketplace.json",
            root.join(".claude-plugin/marketplace.json"),
        ),
    ];
    let mut selected_manifest = None;
    for (relative, path) in &manifest_candidates {
        if !path.is_file() {
            continue;
        }
        // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- path is selected from validated marketplace roots
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read marketplace manifest: {}", path.display()))?;
        let value = serde_json::from_str::<Value>(&content)
            .with_context(|| format!("failed to parse marketplace manifest: {}", path.display()))?;
        selected_manifest = Some(((*relative).to_string(), value));
        break;
    }
    let (manifest_path, manifest) = selected_manifest.ok_or_else(|| {
        anyhow::anyhow!(
            "plugin marketplace manifest not found in {}",
            root.display()
        )
    })?;
    let plugins = manifest
        .get("plugins")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            anyhow::anyhow!("marketplace manifest has no plugins array: {manifest_path}")
        })?;
    let entry = plugins
        .iter()
        .find(|entry| entry.get("name").and_then(Value::as_str) == Some(plugin_name))
        .with_context(|| format!("plugin not found in marketplace {marketplace}: {plugin_name}"))?;
    let source_path = entry
        .get("source")
        .map(parse_plugin_source)
        .transpose()?
        .flatten()
        .or_else(|| {
            entry
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .ok_or_else(|| anyhow::anyhow!("plugin entry has no local source: {plugin_name}"))?;
    let relative_plugin_path = normalize_relative_path(&source_path)?;
    let plugin_root = root.join(&relative_plugin_path);
    let plugin_metadata = fs::symlink_metadata(&plugin_root)
        .with_context(|| format!("plugin source is not accessible: {}", plugin_root.display()))?;
    ensure!(
        !plugin_metadata.file_type().is_symlink(),
        "plugin source must not be a symlink: {}",
        plugin_root.display()
    );
    ensure!(
        plugin_root.is_dir(),
        "plugin source is not a directory: {}",
        plugin_root.display()
    );

    let plugin_manifest_path = plugin_root.join(".claude-plugin/plugin.json");
    let plugin_manifest = if plugin_manifest_path.is_file() {
        // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- plugin_root is a resolved marketplace source
        let content = fs::read_to_string(&plugin_manifest_path).with_context(|| {
            format!(
                "failed to read plugin manifest: {}",
                plugin_manifest_path.display()
            )
        })?;
        Some(serde_json::from_str::<Value>(&content).with_context(|| {
            format!(
                "failed to parse plugin manifest: {}",
                plugin_manifest_path.display()
            )
        })?)
    } else {
        None
    };

    let mut unsupported_components = BTreeSet::new();
    for component in ["agents", "commands", "hooks", "lsp", "apps"] {
        if plugin_root.join(component).exists() {
            unsupported_components.insert(component.to_string());
        }
        if plugin_manifest
            .as_ref()
            .is_some_and(|manifest| manifest.get(component).is_some())
        {
            unsupported_components.insert(format!("plugin.json:{component}"));
        }
    }
    if plugin_manifest
        .as_ref()
        .is_some_and(|manifest| manifest.get("mcpServers").is_some())
    {
        unsupported_components.insert("plugin.json:mcpServers".to_string());
    }

    let skills = discover_skills(&plugin_root)?;
    let mcp_servers = read_plugin_mcp(&plugin_root)?;
    let version = plugin_manifest
        .as_ref()
        .and_then(|manifest| manifest.get("version"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            entry
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_string)
        });

    Ok(DiscoveredPlugin {
        version,
        plugin_path: relative_plugin_path,
        marketplace_manifest: manifest_path,
        content_sha256: hash_tree(&plugin_root)?,
        skills,
        mcp_servers,
        unsupported_components: unsupported_components.into_iter().collect(),
    })
}

fn discover_skills(plugin_root: &Path) -> Result<Vec<DiscoveredSkill>> {
    let skills_root = plugin_root.join("skills");
    if !skills_root.exists() {
        return Ok(Vec::new());
    }
    ensure!(
        skills_root.is_dir(),
        "plugin skills path is not a directory: {}",
        skills_root.display()
    );
    let mut skills = Vec::new();
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- skills_root is derived from a resolved plugin root
    for entry in fs::read_dir(&skills_root)
        .with_context(|| format!("failed to read plugin skills: {}", skills_root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "plugin skill symlink is not allowed: {}",
            path.display()
        );
        if !metadata.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        validate_identifier("skill", &id)?;
        ensure!(
            path.join("SKILL.md").is_file(),
            "plugin skill is missing SKILL.md: {}",
            path.display()
        );
        skills.push(DiscoveredSkill {
            id: id.clone(),
            path,
            relative_path: format!("skills/{id}"),
            content_sha256: hash_tree(&skills_root.join(&id))?,
        });
    }
    skills.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(skills)
}

fn read_plugin_mcp(plugin_root: &Path) -> Result<BTreeMap<String, McpServerConfig>> {
    let path = plugin_root.join(".mcp.json");
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- plugin_root is a resolved marketplace source
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read plugin MCP declaration: {}", path.display()))?;
    let value: Value = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse plugin MCP declaration: {}", path.display()))?;
    let servers = value
        .get("mcpServers")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("plugin .mcp.json must contain an mcpServers object"))?;
    let mut result = BTreeMap::new();
    for (name, value) in servers {
        validate_identifier("MCP server", name)?;
        let server: McpServerConfig = serde_json::from_value(value.clone())
            .with_context(|| format!("invalid plugin MCP server: {name}"))?;
        ensure!(
            result.insert(name.clone(), server).is_none(),
            "duplicate plugin MCP server: {name}"
        );
    }
    Ok(result)
}

fn parse_plugin_source(value: &Value) -> Result<Option<String>> {
    if let Some(source) = value.as_str() {
        return Ok(Some(source.to_string()));
    }
    if let Some(object) = value.as_object() {
        for key in ["source", "path"] {
            if let Some(source) = object.get(key).and_then(Value::as_str) {
                return Ok(Some(source.to_string()));
            }
        }
    }
    Ok(None)
}

fn validate_skill_for_materialization(
    project_root: &Path,
    locked: &LockedPlugin,
    skill: &DiscoveredSkill,
) -> Result<()> {
    crate::skills::manifest::parse_skill_manifest(&skill.path.join("SKILL.md"))
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let target_root = project_root.join(".agents/skills");
    let Some(root_metadata) = fs::symlink_metadata(&target_root).ok() else {
        return Ok(());
    };
    ensure!(
        root_metadata.is_dir() && !root_metadata.file_type().is_symlink(),
        "refusing to materialize skills through a symlinked root: {}",
        target_root.display()
    );
    let target = target_root.join(&skill.id);
    let Some(metadata) = fs::symlink_metadata(&target).ok() else {
        return Ok(());
    };
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "refusing to replace unsafe skill destination: {}",
        target.display()
    );
    if hash_tree(&target)? == skill.content_sha256 {
        return Ok(());
    }
    let registry_path = target_root.join("registry.json");
    let owned = crate::skills::registry::read_registry(&registry_path)
        .ok()
        .and_then(|registry| registry.skills)
        .and_then(|skills| skills.get(&skill.id).cloned())
        .is_some_and(|entry| entry_is_owned_by(&entry, locked));
    ensure!(
        owned,
        "skill collision with unmanaged content: {}",
        target.display()
    );
    Ok(())
}

fn materialize_skill(
    project_root: &Path,
    locked: &LockedPlugin,
    skill: &DiscoveredSkill,
    result: &mut PluginApplyResult,
) -> Result<()> {
    let target_root = project_root.join(".agents/skills");
    fs::create_dir_all(&target_root)
        .with_context(|| format!("failed to create skill root: {}", target_root.display()))?;
    let target_root_metadata = fs::symlink_metadata(&target_root)?;
    ensure!(
        target_root_metadata.is_dir() && !target_root_metadata.file_type().is_symlink(),
        "refusing to materialize skills through a symlinked root: {}",
        target_root.display()
    );
    let target = target_root.join(&skill.id);
    let target_metadata = fs::symlink_metadata(&target).ok();
    if let Some(metadata) = &target_metadata {
        ensure!(
            !metadata.file_type().is_symlink(),
            "refusing to replace symlinked skill: {}",
            target.display()
        );
        ensure!(
            metadata.is_dir(),
            "skill destination is not a directory: {}",
            target.display()
        );
    }
    let target_is_managed = if target_metadata.is_some() {
        let current_hash = hash_tree(&target)?;
        if current_hash == skill.content_sha256 {
            register_deduplicated_plugin_owner(
                &target_root.join("registry.json"),
                &skill.id,
                locked,
                &skill.content_sha256,
            )?;
            result.skipped += 1;
            return Ok(());
        }
        let registry_path = target_root.join("registry.json");
        let owned = crate::skills::registry::read_registry(&registry_path)
            .ok()
            .and_then(|registry| registry.skills)
            .and_then(|skills| skills.get(&skill.id).cloned())
            .is_some_and(|entry| entry_is_owned_by(&entry, locked));
        ensure!(
            owned,
            "skill collision with unmanaged content: {}",
            target.display()
        );
        true
    } else {
        false
    };

    let staging = TempDir::new_in(&target_root).with_context(|| {
        format!(
            "failed to create temporary skill directory in {}",
            target_root.display()
        )
    })?;
    let staged_target = staging.path().join(&skill.id);
    copy_directory_without_symlinks(&skill.path, &staged_target)?;
    let manifest = crate::skills::manifest::parse_skill_manifest(&staged_target.join("SKILL.md"))
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    if target_is_managed {
        remove_path_safely(&target)?;
        result.updated += 1;
    } else {
        result.created += 1;
    }
    fs::rename(&staged_target, &target).with_context(|| {
        format!(
            "failed to atomically install skill {} at {}",
            skill.id,
            target.display()
        )
    })?;
    let registry_path = target_root.join("registry.json");
    let entry = crate::skills::registry::SkillEntry {
        name: Some(manifest.name),
        description: manifest.description,
        version: manifest.version,
        provider: Some(format!("plugin/{}/{}", locked.marketplace, locked.plugin)),
        source: Some(locked.source.location.clone()),
        installed_at: Some(chrono::Utc::now().to_rfc3339()),
        files: None,
        manifest_hash: Some(skill.content_sha256.clone()),
        marketplace: Some(locked.marketplace.clone()),
        plugin: Some(locked.plugin.clone()),
        plugin_revision: Some(locked.source.revision.clone()),
        content_sha256: Some(skill.content_sha256.clone()),
        plugin_owners: Some(vec![crate::skills::registry::PluginOwner {
            marketplace: locked.marketplace.clone(),
            plugin: locked.plugin.clone(),
            revision: locked.source.revision.clone(),
        }]),
    };
    crate::skills::registry::update_registry_entry(&registry_path, &skill.id, entry)?;
    Ok(())
}

fn entry_plugin_owners(
    entry: &crate::skills::registry::SkillEntry,
) -> Vec<crate::skills::registry::PluginOwner> {
    if let Some(owners) = &entry.plugin_owners
        && !owners.is_empty()
    {
        return owners.clone();
    }
    match (&entry.marketplace, &entry.plugin, &entry.plugin_revision) {
        (Some(marketplace), Some(plugin), Some(revision)) => {
            vec![crate::skills::registry::PluginOwner {
                marketplace: marketplace.clone(),
                plugin: plugin.clone(),
                revision: revision.clone(),
            }]
        }
        _ => Vec::new(),
    }
}

fn entry_is_owned_by(entry: &crate::skills::registry::SkillEntry, locked: &LockedPlugin) -> bool {
    let owner = crate::skills::registry::PluginOwner {
        marketplace: locked.marketplace.clone(),
        plugin: locked.plugin.clone(),
        revision: locked.source.revision.clone(),
    };
    entry_plugin_owners(entry).contains(&owner)
}

fn register_deduplicated_plugin_owner(
    registry_path: &Path,
    skill_id: &str,
    locked: &LockedPlugin,
    content_sha256: &str,
) -> Result<()> {
    let Ok(mut registry) = crate::skills::registry::read_registry(registry_path) else {
        return Ok(());
    };
    let Some(skills) = registry.skills.as_mut() else {
        return Ok(());
    };
    let Some(entry) = skills.get_mut(skill_id) else {
        return Ok(());
    };
    let content_matches = entry.content_sha256.as_deref() == Some(content_sha256)
        || entry.manifest_hash.as_deref() == Some(content_sha256);
    if !content_matches {
        return Ok(());
    }
    let owner = crate::skills::registry::PluginOwner {
        marketplace: locked.marketplace.clone(),
        plugin: locked.plugin.clone(),
        revision: locked.source.revision.clone(),
    };
    let mut owners = entry_plugin_owners(entry);
    if owners.contains(&owner) {
        return Ok(());
    }
    owners.push(owner);
    owners.sort_by(|left, right| {
        (&left.marketplace, &left.plugin, &left.revision).cmp(&(
            &right.marketplace,
            &right.plugin,
            &right.revision,
        ))
    });
    entry.plugin_owners = Some(owners);
    crate::skills::registry::update_registry_entry(registry_path, skill_id, entry.clone())?;
    Ok(())
}

fn copy_directory_without_symlinks(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target).with_context(|| format!("failed to create {}", target.display()))?;
    for entry in fs::read_dir(source) // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
        .with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path)?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "plugin symlink is not allowed: {}",
            source_path.display()
        );
        if metadata.is_dir() {
            copy_directory_without_symlinks(&source_path, &target_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &target_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn remove_path_safely(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(path)?;
    } else if metadata.is_dir() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn rollback_plugin_lock(path: &Path, previous: Option<&PluginLock>) -> Result<()> {
    if let Some(previous) = previous {
        previous.save_atomic(path)
    } else if path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("failed to roll back plugin lockfile: {}", path.display()))
    } else {
        Ok(())
    }
}

fn run_async<F, Fut, T>(factory: F) -> Result<T>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new()
                .context("failed to create runtime for plugin operation")?;
            runtime.block_on(factory())
        })
        .join()
        .map_err(|_| anyhow::anyhow!("plugin operation runtime thread panicked"))?
    } else {
        let runtime = tokio::runtime::Runtime::new()
            .context("failed to create runtime for plugin operation")?;
        runtime.block_on(factory())
    }
}

fn add_selection_to_config(config_path: &Path, selection: &PluginSelection) -> Result<()> {
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- config_path is the discovered project config
    let content = fs::read_to_string(config_path).with_context(|| {
        format!(
            "failed to read config for plugin selection: {}",
            config_path.display()
        )
    })?;
    let mut document: DocumentMut = content
        .parse()
        .with_context(|| format!("failed to parse plugin config: {}", config_path.display()))?;
    let plugins = document["plugins"]
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("plugin config [plugins] must be a table"))?;
    let selections = plugins
        .entry("selections")
        .or_insert(Item::ArrayOfTables(toml_edit::ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .ok_or_else(|| {
            anyhow::anyhow!("plugin config [[plugins.selections]] must be an array of tables")
        })?;
    if selections.iter().any(|table| {
        table.get("marketplace").and_then(Item::as_str) == Some(selection.marketplace.as_str())
            && table.get("plugin").and_then(Item::as_str) == Some(selection.plugin.as_str())
    }) {
        return Ok(());
    }
    let mut entry = Table::new();
    entry["marketplace"] = value(selection.marketplace.clone());
    entry["plugin"] = value(selection.plugin.clone());
    selections.push(entry);
    write_atomic_file(config_path, document.to_string().as_bytes())
}

fn remove_plugin_owner_entries_atomic(
    registry_path: &Path,
    skill_ids: &[&str],
    owner: &crate::skills::registry::PluginOwner,
) -> Result<()> {
    if !registry_path.is_file() || skill_ids.is_empty() {
        return Ok(());
    }
    let mut registry = crate::skills::registry::read_registry(registry_path)?;
    let Some(skills) = registry.skills.as_mut() else {
        return Ok(());
    };
    let mut changed = false;
    for skill_id in skill_ids {
        let remove_entry = {
            let Some(entry) = skills.get_mut(*skill_id) else {
                continue;
            };
            let mut owners = entry_plugin_owners(entry);
            if !owners.contains(owner) {
                continue;
            }
            owners.retain(|candidate| candidate != owner);
            changed = true;
            if owners.is_empty() {
                true
            } else {
                owners.sort_by(|left, right| {
                    (&left.marketplace, &left.plugin, &left.revision).cmp(&(
                        &right.marketplace,
                        &right.plugin,
                        &right.revision,
                    ))
                });
                let primary = owners.first().expect("owners is non-empty").clone();
                entry.plugin_owners = Some(owners);
                entry.marketplace = Some(primary.marketplace);
                entry.plugin = Some(primary.plugin);
                entry.plugin_revision = Some(primary.revision);
                false
            }
        };
        if remove_entry {
            skills.remove(*skill_id);
        }
    }
    if !changed {
        return Ok(());
    }
    registry.last_updated = Some(chrono::Utc::now().to_rfc3339());
    let body =
        serde_json::to_vec_pretty(&registry).context("failed to serialize skill registry")?;
    write_atomic_file(registry_path, &body)
}

fn remove_selection_from_config(config_path: &Path, selection: &PluginSelection) -> Result<()> {
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- config_path is the discovered project config
    let content = fs::read_to_string(config_path).with_context(|| {
        format!(
            "failed to read config for plugin removal: {}",
            config_path.display()
        )
    })?;
    let mut document: DocumentMut = content
        .parse()
        .with_context(|| format!("failed to parse plugin config: {}", config_path.display()))?;
    let selections = document
        .get_mut("plugins")
        .and_then(Item::as_table_mut)
        .and_then(|plugins| plugins.get_mut("selections"))
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| anyhow::anyhow!("plugin selection not found: {}", selection.key()))?;
    let index = selections.iter().position(|table| {
        table.get("marketplace").and_then(Item::as_str) == Some(selection.marketplace.as_str())
            && table.get("plugin").and_then(Item::as_str) == Some(selection.plugin.as_str())
    });
    let Some(index) = index else {
        bail!("plugin selection not found: {}", selection.key());
    };
    selections.remove(index);
    write_atomic_file(config_path, document.to_string().as_bytes())
}

fn write_atomic_file(path: &Path, body: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temporary file in {}", parent.display()))?;
    temporary.as_file_mut().write_all(body).with_context(|| {
        format!(
            "failed to write temporary file: {}",
            temporary.path().display()
        )
    })?;
    temporary.as_file_mut().flush().with_context(|| {
        format!(
            "failed to flush temporary file: {}",
            temporary.path().display()
        )
    })?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to replace file: {}", path.display()))?;
    Ok(())
}

fn validate_selection(selection: &PluginSelection) -> Result<()> {
    validate_identifier("marketplace", &selection.marketplace)?;
    validate_identifier("plugin", &selection.plugin)
}

fn validate_source(source: &LockedSource) -> Result<()> {
    match source.kind {
        LockedSourceKind::Local => {
            ensure!(
                !is_absolute_path(&source.location),
                "locked local source must be relative"
            );
            ensure!(
                !source.location.contains("://") && !source.location.contains(':'),
                "locked local source must be a path: {}",
                source.location
            );
            ensure!(
                source.revision.starts_with("local:"),
                "invalid local plugin revision: {}",
                source.revision
            );
            validate_hash(
                "local revision",
                source.revision.trim_start_matches("local:"),
            )?;
        }
        LockedSourceKind::Git => {
            ensure!(
                source.location.starts_with("https://"),
                "Git plugin source must use HTTPS"
            );
            validate_commit(&source.revision)?;
        }
    }
    Ok(())
}

fn validate_identifier(kind: &str, value: &str) -> Result<()> {
    ensure!(!value.is_empty(), "{kind} must not be empty");
    ensure!(
        value
            .chars()
            .all(|character| character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_' | '.')),
        "invalid {kind}: {value}"
    );
    ensure!(value != "." && value != "..", "invalid {kind}: {value}");
    Ok(())
}

fn validate_relative_path(path: &str) -> Result<()> {
    ensure!(!path.is_empty(), "path must not be empty");
    ensure!(
        !is_absolute_path(path),
        "absolute paths are not allowed: {path}"
    );
    ensure!(
        !path.contains(':'),
        "drive-prefixed paths are not allowed: {path}"
    );
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- this only inspects path components; no filesystem access occurs
    for component in Path::new(path).components() {
        ensure!(
            !matches!(component, Component::ParentDir),
            "path traversal is not allowed: {path}"
        );
    }
    Ok(())
}

fn normalize_relative_path(path: &str) -> Result<String> {
    let normalized = path.strip_prefix("./").unwrap_or(path);
    validate_relative_path(normalized)?;
    Ok(normalized.to_string())
}

fn validate_hash(field: &str, value: &str) -> Result<()> {
    ensure!(
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{field} must be a SHA-256 hash"
    );
    Ok(())
}

fn validate_commit(value: &str) -> Result<()> {
    ensure!(
        value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Git revision must be a full 40-character commit SHA"
    );
    Ok(())
}

fn is_local_source(source: &str) -> bool {
    source.starts_with('.') || is_absolute_path(source) || !source.contains("://")
}

fn is_absolute_path(path: &str) -> bool {
    Path::new(path).is_absolute() || path.starts_with(['/', '\\'])
}

fn hash_tree(root: &Path) -> Result<String> {
    ensure!(root.is_dir(), "expected directory: {}", root.display());
    let mut entries = Vec::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        if entry.path() == root {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "symlinks are not allowed in plugin sources: {}",
            entry.path().display()
        );
        if metadata.is_file() {
            let relative = entry
                .path()
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            entries.push(relative);
        }
    }
    entries.sort();
    let mut hasher = Sha256::new();
    for path in entries {
        let bytes = fs::read(root.join(&path))?;
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }
    Ok(format_digest(hasher.finalize()))
}

fn format_digest(digest: impl AsRef<[u8]>) -> String {
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

async fn resolve_git_reference(repository: &str, reference: &str) -> Result<String> {
    ensure!(
        !reference.trim().eq_ignore_ascii_case("HEAD"),
        "Git reference HEAD is not allowed; use a branch, tag, or full commit SHA"
    );
    if reference.len() == 40 && reference.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(reference.to_ascii_lowercase());
    }
    let github = github_repo_parts(repository)?;
    let endpoint = format!(
        "https://api.github.com/repos/{}/{}/commits/{}",
        github.0,
        github.1,
        urlencoding::encode(reference)
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("agentsync-plugin-resolver")
        .build()
        .context("failed to create GitHub API client")?;
    let mut request = client.get(endpoint);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        request = request.bearer_auth(token);
    }
    let response: Value = request
        .send()
        .await
        .context("failed to resolve Git reference")?
        .error_for_status()
        .context("Git reference resolution failed")?
        .json()
        .await
        .context("invalid GitHub commit response")?;
    let sha = response
        .get("sha")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("GitHub commit response did not contain a SHA"))?;
    validate_commit(sha)?;
    Ok(sha.to_ascii_lowercase())
}

fn github_archive_url(repository: &str, commit: &str) -> Result<String> {
    let (owner, repo) = github_repo_parts(repository)?;
    validate_commit(commit)?;
    Ok(format!(
        "https://github.com/{owner}/{repo}/archive/{commit}.zip"
    ))
}

fn github_repo_parts(repository: &str) -> Result<(String, String)> {
    let parsed = url::Url::parse(repository).context("invalid Git repository URL")?;
    ensure!(parsed.scheme() == "https", "Git repository must use HTTPS");
    ensure!(
        parsed.host_str() == Some("github.com"),
        "only GitHub repositories are supported in this MVP"
    );
    let segments = parsed
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>())
        .unwrap_or_default();
    ensure!(
        segments.len() == 2,
        "GitHub repository URL must be https://github.com/<owner>/<repo>"
    );
    let repo = segments[1].trim_end_matches(".git");
    validate_identifier("GitHub owner", segments[0])?;
    validate_identifier("GitHub repository", repo)?;
    Ok((segments[0].to_string(), repo.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn lockfile_round_trip_is_deterministic() {
        let mut lock = PluginLock::default();
        let plugin = sample_locked_plugin("internal", "engineering");
        lock.plugins.insert(plugin.key(), plugin);
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("plugins.lock.toml");
        lock.save_atomic(&path).unwrap();
        let first = fs::read_to_string(&path).unwrap();
        let loaded = PluginLock::load(&path).unwrap();
        loaded.save_atomic(&path).unwrap();
        assert_eq!(first, fs::read_to_string(path).unwrap());
    }

    #[tokio::test]
    async fn mutable_git_reference_resolves_to_a_commit_shape() {
        assert_eq!(
            resolve_git_reference(
                "https://github.com/example/repo",
                "0123456789abcdef0123456789abcdef01234567"
            )
            .await
            .unwrap(),
            "0123456789abcdef0123456789abcdef01234567"
        );
        assert!(
            resolve_git_reference("https://gitlab.com/example/repo", "main")
                .await
                .is_err()
        );
        assert!(
            resolve_git_reference("https://github.com/example/repo", "HEAD")
                .await
                .is_err()
        );
    }

    #[test]
    fn apply_requires_a_local_snapshot_for_locked_git_sources() {
        let temp = TempDir::new().unwrap();
        let agents = temp.path().join(".agents");
        fs::create_dir_all(&agents).unwrap();
        let config_path = agents.join("agentsync.toml");
        let selection = PluginSelection {
            marketplace: "internal".to_string(),
            plugin: "engineering".to_string(),
        };
        let config = PluginsConfig {
            enabled: true,
            lockfile: "plugins.lock.toml".to_string(),
            marketplaces: BTreeMap::new(),
            selections: vec![selection.clone()],
        };
        let manager = PluginManager::new(temp.path().to_path_buf(), config_path, config);
        let mut plugin = sample_locked_plugin(&selection.marketplace, &selection.plugin);
        plugin.source.kind = LockedSourceKind::Git;
        plugin.source.location = "https://github.com/example/repo".to_string();
        plugin.source.revision = "0123456789abcdef0123456789abcdef01234567".to_string();
        plugin.provenance.resolved_revision = plugin.source.revision.clone();
        let mut lock = PluginLock::default();
        lock.plugins.insert(plugin.key(), plugin);
        lock.save_atomic(&manager.lock_path().unwrap()).unwrap();

        let error = manager
            .apply(true)
            .expect_err("apply must not fetch a Git source");
        assert!(
            error
                .to_string()
                .contains("offline Git plugin source snapshot")
        );
    }

    #[test]
    fn path_validation_rejects_traversal_and_absolute_paths() {
        assert!(validate_relative_path("../outside").is_err());
        assert!(validate_relative_path("/outside").is_err());
        assert!(validate_relative_path(r"\outside").is_err());
        assert!(validate_relative_path(r"C:\outside").is_err());
        assert!(normalize_relative_path("./skills/demo").is_ok());
    }

    #[test]
    fn plugin_lock_validation_rejects_inconsistent_entries() {
        let invalid = |lock: PluginLock| assert!(lock.validate().is_err());

        let lock = PluginLock {
            schema_version: "v2".to_string(),
            ..PluginLock::default()
        };
        invalid(lock);

        let mut lock = PluginLock::default();
        lock.plugins.insert(
            "wrong/key".to_string(),
            sample_locked_plugin("internal", "engineering"),
        );
        invalid(lock);

        let mut plugin = sample_locked_plugin("internal", "engineering");
        plugin.marketplace = "../internal".to_string();
        let mut lock = PluginLock::default();
        lock.plugins.insert(plugin.key(), plugin);
        invalid(lock);

        let mut plugin = sample_locked_plugin("internal", "engineering");
        plugin.source.location = "https://example.com/plugin".to_string();
        let mut lock = PluginLock::default();
        lock.plugins.insert(plugin.key(), plugin);
        invalid(lock);

        let mut plugin = sample_locked_plugin("internal", "engineering");
        plugin.provenance.content_sha256 = "f".repeat(64);
        let mut lock = PluginLock::default();
        lock.plugins.insert(plugin.key(), plugin);
        invalid(lock);

        let mut plugin = sample_locked_plugin("internal", "engineering");
        plugin.skills.push(plugin.skills[0].clone());
        let mut lock = PluginLock::default();
        lock.plugins.insert(plugin.key(), plugin);
        invalid(lock);

        let mut plugin = sample_locked_plugin("internal", "engineering");
        plugin.mcp_servers.push(String::new());
        let mut lock = PluginLock::default();
        lock.plugins.insert(plugin.key(), plugin);
        invalid(lock);

        let mut plugin = sample_locked_plugin("internal", "engineering");
        plugin.unsupported_components.push("hooks".to_string());
        let mut lock = PluginLock::default();
        lock.plugins.insert(plugin.key(), plugin);
        invalid(lock);
    }

    #[test]
    fn plugin_source_and_url_helpers_cover_supported_and_rejected_shapes() {
        assert_eq!(
            parse_plugin_source(&serde_json::json!("./plugin")).unwrap(),
            Some("./plugin".to_string())
        );
        assert_eq!(
            parse_plugin_source(&serde_json::json!({"path": "./plugin"})).unwrap(),
            Some("./plugin".to_string())
        );
        assert_eq!(
            parse_plugin_source(&serde_json::json!({"source": "./plugin"})).unwrap(),
            Some("./plugin".to_string())
        );
        assert_eq!(parse_plugin_source(&serde_json::json!(true)).unwrap(), None);

        assert!(is_local_source("../marketplace"));
        assert!(is_local_source("C:\\marketplace"));
        assert!(!is_local_source("https://github.com/example/repo"));
        assert_eq!(
            github_repo_parts("https://github.com/example/repo.git").unwrap(),
            ("example".to_string(), "repo".to_string())
        );
        assert_eq!(
            github_archive_url(
                "https://github.com/example/repo",
                "0123456789abcdef0123456789abcdef01234567"
            )
            .unwrap(),
            "https://github.com/example/repo/archive/0123456789abcdef0123456789abcdef01234567.zip"
        );
        assert!(github_repo_parts("https://gitlab.com/example/repo").is_err());
        assert!(github_repo_parts("http://github.com/example/repo").is_err());
        assert!(github_repo_parts("https://github.com/example/repo/extra").is_err());
        assert!(github_archive_url("https://github.com/example/repo", "short").is_err());

        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join(".agents/agentsync.toml");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let local = MarketplaceConfig {
            source: "../marketplace".to_string(),
            reference: None,
        };
        assert!(resolve_marketplace_source(&config_path, &local, false).is_err());
        let marketplace_file = temp.path().join("marketplace-file");
        fs::write(&marketplace_file, "not a directory").unwrap();
        let local_file = MarketplaceConfig {
            source: "../marketplace-file".to_string(),
            reference: None,
        };
        assert!(resolve_marketplace_source(&config_path, &local_file, true).is_err());
        let missing_reference = MarketplaceConfig {
            source: "https://github.com/example/repo".to_string(),
            reference: None,
        };
        assert!(resolve_marketplace_source(&config_path, &missing_reference, true).is_err());
        let blocked_network = MarketplaceConfig {
            source: "https://gitlab.com/example/repo".to_string(),
            reference: Some("main".to_string()),
        };
        assert!(resolve_marketplace_source(&config_path, &blocked_network, false).is_err());
        let file_url = MarketplaceConfig {
            source: "file://../marketplace".to_string(),
            reference: None,
        };
        assert!(resolve_marketplace_source(&config_path, &file_url, true).is_err());

        assert!(
            validate_source(&LockedSource {
                kind: LockedSourceKind::Local,
                location: "/absolute".to_string(),
                revision: "local:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
            })
            .is_err()
        );
        assert!(
            validate_source(&LockedSource {
                kind: LockedSourceKind::Local,
                location: "../marketplace".to_string(),
                revision: "local:short".to_string(),
            })
            .is_err()
        );
        assert!(
            validate_source(&LockedSource {
                kind: LockedSourceKind::Git,
                location: "http://github.com/example/repo".to_string(),
                revision: "short".to_string(),
            })
            .is_err()
        );
    }

    #[test]
    fn discovery_supports_claude_manifest_path_entries_and_empty_plugins() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let plugin_root = root.join("plugins/empty");
        fs::create_dir_all(plugin_root.join(".claude-plugin")).unwrap();
        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        fs::write(
            root.join(".claude-plugin/marketplace.json"),
            serde_json::to_vec(&serde_json::json!({
                "plugins": [{"name": "empty", "path": "./plugins/empty", "version": "2.0.0"}]
            }))
            .unwrap(),
        )
        .unwrap();

        let discovered = discover_plugin(root, "internal", "empty").unwrap();
        assert_eq!(discovered.version.as_deref(), Some("2.0.0"));
        assert_eq!(
            discovered.marketplace_manifest,
            ".claude-plugin/marketplace.json"
        );
        assert!(discovered.skills.is_empty());
        assert!(discovered.mcp_servers.is_empty());

        let plugin_manifest = serde_json::json!({"version": "3.0.0"});
        fs::write(
            plugin_root.join(".claude-plugin/plugin.json"),
            serde_json::to_vec(&plugin_manifest).unwrap(),
        )
        .unwrap();
        let discovered = discover_plugin(root, "internal", "empty").unwrap();
        assert_eq!(discovered.version.as_deref(), Some("3.0.0"));
    }

    #[test]
    fn discovery_rejects_invalid_skill_and_mcp_declarations() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let plugin_root = root.join("plugin");
        fs::create_dir_all(plugin_root.join("skills/missing")).unwrap();
        assert!(discover_skills(&plugin_root).is_err());
        let bad_root = root.join("bad-plugin");
        fs::create_dir_all(&bad_root).unwrap();
        fs::write(bad_root.join("skills"), "not a directory").unwrap();
        assert!(discover_skills(&bad_root).is_err());
        let mixed_root = root.join("mixed-plugin/skills");
        fs::create_dir_all(mixed_root.join("valid")).unwrap();
        fs::write(mixed_root.join("file.txt"), "ignored").unwrap();
        fs::write(
            mixed_root.join("valid/SKILL.md"),
            "---\nname: Valid\n---\nbody\n",
        )
        .unwrap();
        assert_eq!(
            discover_skills(mixed_root.parent().unwrap()).unwrap().len(),
            1
        );

        fs::write(plugin_root.join(".mcp.json"), "{}").unwrap();
        assert!(read_plugin_mcp(&plugin_root).is_err());
        fs::write(
            plugin_root.join(".mcp.json"),
            serde_json::to_vec(&serde_json::json!({"mcpServers": {"bad/name": {}}})).unwrap(),
        )
        .unwrap();
        assert!(read_plugin_mcp(&plugin_root).is_err());
    }

    #[test]
    fn plugin_manager_covers_offline_and_source_cache_paths() {
        let temp = TempDir::new().unwrap();
        let agents = temp.path().join(".agents");
        let marketplace = temp.path().join("marketplace");
        fs::create_dir_all(&agents).unwrap();
        fs::create_dir_all(&marketplace).unwrap();
        fs::write(marketplace.join("README.md"), "snapshot").unwrap();
        let config_path = agents.join("agentsync.toml");

        let disabled = PluginManager::new(
            temp.path().to_path_buf(),
            config_path.clone(),
            PluginsConfig::default(),
        );
        assert_eq!(disabled.apply(false).unwrap().skipped, 0);

        let selection = PluginSelection {
            marketplace: "internal".to_string(),
            plugin: "engineering".to_string(),
        };
        let config = PluginsConfig {
            enabled: true,
            lockfile: "plugins.lock.toml".to_string(),
            marketplaces: BTreeMap::new(),
            selections: vec![selection.clone()],
        };
        let manager = PluginManager::new(temp.path().to_path_buf(), config_path, config);
        assert!(manager.apply(false).is_err());
        PluginLock::default()
            .save_atomic(&manager.lock_path().unwrap())
            .unwrap();
        assert!(manager.apply(false).is_err());

        let revision = format!("local:{}", hash_tree(&marketplace).unwrap());
        let local = LockedSource {
            kind: LockedSourceKind::Local,
            location: "../marketplace".to_string(),
            revision,
        };
        let resolved = manager.materialize_source(&local).unwrap();
        assert_eq!(resolved.root(), marketplace.canonicalize().unwrap());
        assert!(
            manager
                .cache_marketplace_source(&ResolvedMarketplaceSource {
                    root: marketplace.clone(),
                    locked_source: local.clone(),
                    temp: None,
                })
                .is_ok()
        );
        assert!(manager.git_source_cache_path(&local).is_err());

        let git = LockedSource {
            kind: LockedSourceKind::Git,
            location: "https://github.com/example/repo".to_string(),
            revision: "0123456789abcdef0123456789abcdef01234567".to_string(),
        };
        let source = ResolvedMarketplaceSource {
            root: marketplace.clone(),
            locked_source: git.clone(),
            temp: None,
        };
        manager.cache_marketplace_source(&source).unwrap();
        let cache = manager.git_source_cache_path(&git).unwrap();
        assert!(cache.join("README.md").is_file());
        manager.cache_marketplace_source(&source).unwrap();
        assert!(manager.materialize_source(&git).unwrap().root().is_dir());

        let missing = LockedSource {
            kind: LockedSourceKind::Local,
            location: "../missing".to_string(),
            revision: "local:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
        };
        assert!(manager.materialize_source(&missing).is_err());
        let file_source = temp.path().join("file-source");
        fs::write(&file_source, "file").unwrap();
        let file_source = LockedSource {
            kind: LockedSourceKind::Local,
            location: "../file-source".to_string(),
            revision: "local:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
        };
        assert!(manager.materialize_source(&file_source).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let symlinked_git = LockedSource {
                kind: LockedSourceKind::Git,
                location: "https://github.com/example/other-repo".to_string(),
                revision: "0123456789abcdef0123456789abcdef01234568".to_string(),
            };
            let destination = manager.git_source_cache_path(&symlinked_git).unwrap();
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            symlink(&marketplace, &destination).unwrap();
            assert!(
                manager
                    .cache_marketplace_source(&ResolvedMarketplaceSource {
                        root: marketplace.clone(),
                        locked_source: symlinked_git,
                        temp: None,
                    })
                    .is_err()
            );
        }
    }

    #[test]
    fn discovery_rejects_missing_and_unsafe_marketplace_shapes() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        assert!(discover_plugin(root, "internal", "missing").is_err());

        fs::create_dir_all(root.join(".agents/plugins")).unwrap();
        fs::write(root.join(".agents/plugins/marketplace.json"), "{}").unwrap();
        assert!(discover_plugin(root, "internal", "missing").is_err());

        fs::write(
            root.join(".agents/plugins/marketplace.json"),
            serde_json::to_vec(&serde_json::json!({
                "plugins": [
                    {"name": "missing-source"},
                    {"name": "traversal", "source": "../outside"},
                    {"name": "file", "source": "./file"},
                    {"name": "bad-manifest", "source": "./bad-manifest"}
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(root.join("file"), "not a directory").unwrap();
        fs::create_dir_all(root.join("bad-manifest/.claude-plugin")).unwrap();
        fs::write(
            root.join("bad-manifest/.claude-plugin/plugin.json"),
            "invalid json",
        )
        .unwrap();
        assert!(discover_plugin(root, "internal", "missing-source").is_err());
        assert!(discover_plugin(root, "internal", "traversal").is_err());
        assert!(discover_plugin(root, "internal", "file").is_err());
        assert!(discover_plugin(root, "internal", "bad-manifest").is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let real_plugin = root.join("real-plugin");
            fs::create_dir_all(&real_plugin).unwrap();
            symlink(&real_plugin, root.join("symlink-plugin")).unwrap();
            fs::write(
                root.join(".agents/plugins/marketplace.json"),
                serde_json::to_vec(&serde_json::json!({
                    "plugins": [{"name": "symlink", "source": "./symlink-plugin"}]
                }))
                .unwrap(),
            )
            .unwrap();
            assert!(discover_plugin(root, "internal", "symlink").is_err());
        }
    }

    #[test]
    fn discovery_records_unsupported_plugin_components_without_executing_them() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let plugin = root.join("plugin");
        fs::create_dir_all(plugin.join(".claude-plugin")).unwrap();
        fs::create_dir_all(plugin.join("hooks")).unwrap();
        fs::write(
            root.join("marketplace.json"),
            serde_json::to_vec(&serde_json::json!({
                "plugins": [{"name": "unsafe", "source": "./plugin"}]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            plugin.join(".claude-plugin/plugin.json"),
            serde_json::to_vec(&serde_json::json!({
                "agents": [],
                "mcpServers": {}
            }))
            .unwrap(),
        )
        .unwrap();
        let discovered = discover_plugin(root, "internal", "unsafe");
        assert!(discovered.is_err(), "the preferred manifest is absent");

        fs::create_dir_all(root.join(".agents/plugins")).unwrap();
        fs::copy(
            root.join("marketplace.json"),
            root.join(".agents/plugins/marketplace.json"),
        )
        .unwrap();
        let discovered = discover_plugin(root, "internal", "unsafe").unwrap();
        assert!(
            discovered
                .unsupported_components
                .iter()
                .any(|component| component == "hooks")
        );
        assert!(
            discovered
                .unsupported_components
                .iter()
                .any(|component| component == "plugin.json:agents")
        );
        assert!(
            discovered
                .unsupported_components
                .iter()
                .any(|component| component == "plugin.json:mcpServers")
        );
    }

    #[test]
    fn helper_transactions_cover_owner_and_config_edge_cases() {
        let temp = TempDir::new().unwrap();
        let registry_path = temp.path().join("registry.json");
        let owner = crate::skills::registry::PluginOwner {
            marketplace: "internal".to_string(),
            plugin: "engineering".to_string(),
            revision: "revision".to_string(),
        };
        let other_owner = crate::skills::registry::PluginOwner {
            marketplace: "other".to_string(),
            plugin: "plugin".to_string(),
            revision: "other-revision".to_string(),
        };
        assert!(remove_plugin_owner_entries_atomic(&registry_path, &[], &owner).is_ok());
        assert!(remove_plugin_owner_entries_atomic(&registry_path, &["demo"], &owner).is_ok());

        fs::write(
            &registry_path,
            serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 1,
                "last_updated": null,
                "skills": null
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(remove_plugin_owner_entries_atomic(&registry_path, &["demo"], &owner).is_ok());
        let locked = sample_locked_plugin("internal", "engineering");
        assert!(
            register_deduplicated_plugin_owner(
                &registry_path,
                "demo",
                &locked,
                "f".repeat(64).as_str()
            )
            .is_ok()
        );

        let entry = crate::skills::registry::SkillEntry {
            name: None,
            version: None,
            description: None,
            provider: None,
            source: None,
            installed_at: None,
            files: None,
            manifest_hash: None,
            marketplace: Some(owner.marketplace.clone()),
            plugin: Some(owner.plugin.clone()),
            plugin_revision: Some(owner.revision.clone()),
            content_sha256: None,
            plugin_owners: Some(vec![owner.clone(), other_owner.clone()]),
        };
        crate::skills::registry::update_registry_entry(&registry_path, "demo", entry).unwrap();
        assert!(remove_plugin_owner_entries_atomic(&registry_path, &["missing"], &owner).is_ok());
        assert!(remove_plugin_owner_entries_atomic(&registry_path, &["demo"], &owner).is_ok());
        let registry = crate::skills::registry::read_registry(&registry_path).unwrap();
        let remaining = registry.skills.unwrap().remove("demo").unwrap();
        assert_eq!(remaining.plugin_owners.unwrap(), vec![other_owner.clone()]);
        assert_eq!(remaining.marketplace.as_deref(), Some("other"));

        let empty_entry = crate::skills::registry::SkillEntry {
            name: None,
            version: None,
            description: None,
            provider: None,
            source: None,
            installed_at: None,
            files: None,
            manifest_hash: None,
            marketplace: None,
            plugin: None,
            plugin_revision: None,
            content_sha256: None,
            plugin_owners: Some(Vec::new()),
        };
        assert!(entry_plugin_owners(&empty_entry).is_empty());

        let config_path = temp.path().join("agentsync.toml");
        fs::write(
            &config_path,
            "[plugins]\nenabled = true\n\n[[plugins.selections]]\nmarketplace = \"other\"\nplugin = \"plugin\"\n",
        )
        .unwrap();
        let selection = PluginSelection {
            marketplace: "internal".to_string(),
            plugin: "engineering".to_string(),
        };
        add_selection_to_config(&config_path, &selection).unwrap();
        add_selection_to_config(&config_path, &selection).unwrap();
        let nonmatching = PluginSelection {
            marketplace: "missing".to_string(),
            plugin: "plugin".to_string(),
        };
        let error = remove_selection_from_config(&config_path, &nonmatching).unwrap_err();
        assert!(error.to_string().contains("selection not found"));
        remove_selection_from_config(&config_path, &selection).unwrap();
        assert!(
            !fs::read_to_string(&config_path)
                .unwrap()
                .contains("marketplace = \"internal\"")
        );
        let missing_config = temp.path().join("missing-config.toml");
        assert!(add_selection_to_config(&missing_config, &selection).is_err());
        assert!(remove_selection_from_config(&missing_config, &selection).is_err());
        fs::write(&missing_config, "[[plugins.selections]]\nmarketplace = [").unwrap();
        assert!(remove_selection_from_config(&missing_config, &selection).is_err());
        let no_newline_config = temp.path().join("no-newline.toml");
        fs::write(&no_newline_config, "[plugins]\nenabled = true").unwrap();
        add_selection_to_config(&no_newline_config, &selection).unwrap();
        let preserved_config = temp.path().join("preserved-config.toml");
        fs::write(
            &preserved_config,
            "# keep this comment\n[plugins]\nenabled = true\n\n[[plugins.selections]]\n# keep this entry comment\nmarketplace = \"other\"\nplugin = \"plugin\"\n",
        )
        .unwrap();
        add_selection_to_config(&preserved_config, &selection).unwrap();
        let preserved = fs::read_to_string(&preserved_config).unwrap();
        assert!(preserved.contains("# keep this comment"));
        assert!(preserved.contains("# keep this entry comment"));
        assert_eq!(preserved.matches("[[plugins.selections]]").count(), 2);

        let rollback_path = temp.path().join("rollback.lock");
        rollback_plugin_lock(&rollback_path, None).unwrap();
        fs::write(&rollback_path, "stale").unwrap();
        rollback_plugin_lock(&rollback_path, None).unwrap();
        PluginLock::default().save_atomic(&rollback_path).unwrap();
        rollback_plugin_lock(&rollback_path, Some(&PluginLock::default())).unwrap();

        let removable_file = temp.path().join("removable-file");
        fs::write(&removable_file, "file").unwrap();
        remove_path_safely(&removable_file).unwrap();
        let removable_dir = temp.path().join("removable-dir");
        fs::create_dir_all(&removable_dir).unwrap();
        remove_path_safely(&removable_dir).unwrap();
        let copy_source = temp.path().join("copy-source");
        let copy_target = temp.path().join("copy-target");
        fs::create_dir_all(copy_source.join("nested")).unwrap();
        fs::write(copy_source.join("nested/file"), "content").unwrap();
        copy_directory_without_symlinks(&copy_source, &copy_target).unwrap();
        assert!(copy_target.join("nested/file").is_file());
        let missing_registry = temp.path().join("missing-registry.json");
        assert!(
            register_deduplicated_plugin_owner(
                &missing_registry,
                "demo",
                &sample_locked_plugin("internal", "engineering"),
                "f".repeat(64).as_str()
            )
            .is_ok()
        );
    }

    #[cfg(unix)]
    #[test]
    fn materialization_rejects_symlinked_and_non_directory_destinations() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source/demo");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: Demo\ndescription: Demo\n---\nbody\n",
        )
        .unwrap();
        let hash = hash_tree(&source).unwrap();
        let skill = DiscoveredSkill {
            id: "demo".to_string(),
            path: source,
            relative_path: "skills/demo".to_string(),
            content_sha256: hash,
        };
        let locked = sample_locked_plugin("internal", "engineering");

        let project_root = temp.path().join("symlink-root");
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        fs::create_dir_all(project_root.join("real-skills")).unwrap();
        symlink(
            project_root.join("real-skills"),
            project_root.join(".agents/skills"),
        )
        .unwrap();
        assert!(
            materialize_skill(
                &project_root,
                &locked,
                &skill,
                &mut PluginApplyResult::default()
            )
            .is_err()
        );

        let project_root = temp.path().join("symlink-target");
        fs::create_dir_all(project_root.join(".agents/skills")).unwrap();
        fs::create_dir_all(project_root.join("real-demo")).unwrap();
        symlink(
            project_root.join("real-demo"),
            project_root.join(".agents/skills/demo"),
        )
        .unwrap();
        assert!(
            materialize_skill(
                &project_root,
                &locked,
                &skill,
                &mut PluginApplyResult::default()
            )
            .is_err()
        );

        let skill_symlink_root = temp.path().join("skill-symlink");
        fs::create_dir_all(skill_symlink_root.join("skills")).unwrap();
        fs::create_dir_all(skill_symlink_root.join("real-skill")).unwrap();
        symlink(
            skill_symlink_root.join("real-skill"),
            skill_symlink_root.join("skills/link"),
        )
        .unwrap();
        assert!(discover_skills(&skill_symlink_root).is_err());

        let project_root = temp.path().join("file-target");
        fs::create_dir_all(project_root.join(".agents/skills")).unwrap();
        fs::write(project_root.join(".agents/skills/demo"), "file").unwrap();
        assert!(
            materialize_skill(
                &project_root,
                &locked,
                &skill,
                &mut PluginApplyResult::default()
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_helpers_reject_symlinks_and_register_plugin_owners() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("file.txt"), "content").unwrap();
        symlink(source.join("file.txt"), source.join("link.txt")).unwrap();
        assert!(copy_directory_without_symlinks(&source, &target).is_err());
        assert!(hash_tree(&source).is_err());

        let registry_path = temp.path().join("registry.json");
        crate::skills::registry::write_registry(&registry_path).unwrap();
        let hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let entry = crate::skills::registry::SkillEntry {
            name: None,
            version: None,
            description: None,
            provider: None,
            source: None,
            installed_at: None,
            files: None,
            manifest_hash: Some(hash.to_string()),
            marketplace: Some("legacy".to_string()),
            plugin: Some("old".to_string()),
            plugin_revision: Some("old-revision".to_string()),
            content_sha256: Some(hash.to_string()),
            plugin_owners: None,
        };
        crate::skills::registry::update_registry_entry(&registry_path, "demo", entry).unwrap();
        let locked = sample_locked_plugin("internal", "engineering");
        assert_eq!(
            entry_plugin_owners(
                &crate::skills::registry::read_registry(&registry_path)
                    .unwrap()
                    .skills
                    .unwrap()
                    .get("demo")
                    .unwrap()
                    .clone()
            )
            .len(),
            1
        );
        assert!(!entry_is_owned_by(
            &crate::skills::registry::read_registry(&registry_path)
                .unwrap()
                .skills
                .unwrap()
                .get("demo")
                .unwrap()
                .clone(),
            &locked
        ));
        assert!(
            register_deduplicated_plugin_owner(&registry_path, "missing", &locked, hash).is_ok()
        );
        assert!(
            register_deduplicated_plugin_owner(
                &registry_path,
                "demo",
                &locked,
                "f".repeat(64).as_str()
            )
            .is_ok()
        );
        assert!(register_deduplicated_plugin_owner(&registry_path, "demo", &locked, hash).is_ok());
        let registry = crate::skills::registry::read_registry(&registry_path).unwrap();
        assert_eq!(
            registry
                .skills
                .unwrap()
                .get("demo")
                .unwrap()
                .plugin_owners
                .as_ref()
                .unwrap()
                .len(),
            2
        );
    }

    fn sample_locked_plugin(marketplace: &str, plugin: &str) -> LockedPlugin {
        let revision = "local:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        LockedPlugin {
            marketplace: marketplace.to_string(),
            plugin: plugin.to_string(),
            version: Some("1.0.0".to_string()),
            source: LockedSource {
                kind: LockedSourceKind::Local,
                location: "../marketplace".to_string(),
                revision: revision.to_string(),
            },
            content_sha256: hash.to_string(),
            skills: vec![LockedSkill {
                id: "demo".to_string(),
                path: "skills/demo".to_string(),
                content_sha256: hash.to_string(),
            }],
            mcp_servers: vec!["filesystem".to_string()],
            unsupported_components: Vec::new(),
            provenance: PluginProvenance {
                marketplace_manifest: ".claude-plugin/marketplace.json".to_string(),
                plugin_path: "plugins/engineering".to_string(),
                resolved_revision: revision.to_string(),
                content_sha256: hash.to_string(),
            },
        }
    }
}
