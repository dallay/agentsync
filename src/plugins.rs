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
use std::path::{Component, Path, PathBuf};
use tempfile::{NamedTempFile, TempDir};
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
        let temporary = NamedTempFile::new_in(parent).with_context(|| {
            format!(
                "failed to create temporary plugin lockfile in {}",
                parent.display()
            )
        })?;
        fs::write(temporary.path(), body).with_context(|| {
            format!(
                "failed to write temporary plugin lockfile: {}",
                temporary.path().display()
            )
        })?;
        temporary
            .persist(path)
            .map_err(|error| error.error)
            .with_context(|| format!("failed to replace plugin lockfile: {}", path.display()))?;
        Ok(())
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

            if dry_run {
                result.updated += discovered.skills.len();
                result.skipped += usize::from(discovered.skills.is_empty());
            } else {
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
                    materialize_skill(&self.project_root, locked, skill, &mut result)?;
                }
            }
            drop(source);
        }

        Ok(result)
    }

    /// Resolve and lock a selected plugin from the configured marketplace.
    pub fn add(&self, selection: &PluginSelection) -> Result<PluginApplyResult> {
        self.lock_selection(selection)
    }

    pub fn update(&self, selection: &PluginSelection) -> Result<PluginApplyResult> {
        self.lock_selection(selection)
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

    fn lock_selection(&self, selection: &PluginSelection) -> Result<PluginApplyResult> {
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
        let source = resolve_marketplace_source(&self.config_path, marketplace, true)?;
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
                    !Path::new(&source.location).is_absolute()
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
                Ok(ResolvedSource { root, temp: None })
            }
            LockedSourceKind::Git => {
                let root = self.git_source_cache_path(source)?;
                ensure!(
                    root.is_dir(),
                    "offline Git plugin source snapshot is unavailable: {} (run `agentsync plugin update` first)",
                    root.display()
                );
                Ok(ResolvedSource { root, temp: None })
            }
        }
    }
}

struct ResolvedSource {
    root: PathBuf,
    temp: Option<TempDir>,
}

impl ResolvedSource {
    fn root(&self) -> &Path {
        &self.root
    }
}

impl Drop for ResolvedSource {
    fn drop(&mut self) {
        let _ = self.temp.take();
    }
}

#[derive(Debug)]
struct DiscoveredSkill {
    id: String,
    path: PathBuf,
    relative_path: String,
    content_sha256: String,
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

fn resolve_marketplace_source(
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
            !Path::new(source).is_absolute() && !source.contains(':'),
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
    let revision = resolve_git_reference(source, reference)?;
    let archive = github_archive_url(source, &revision)?;
    let temp = blocking_fetch_archive(&archive)?;
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
    ensure!(
        plugin_root.is_dir(),
        "plugin source is not a directory: {}",
        plugin_root.display()
    );

    let plugin_manifest_path = plugin_root.join(".claude-plugin/plugin.json");
    let plugin_manifest = if plugin_manifest_path.is_file() {
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
    for entry in
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
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

fn add_selection_to_config(config_path: &Path, selection: &PluginSelection) -> Result<()> {
    let content = fs::read_to_string(config_path).with_context(|| {
        format!(
            "failed to read config for plugin selection: {}",
            config_path.display()
        )
    })?;
    let document: toml::Value = toml::from_str(&content)
        .with_context(|| format!("failed to parse plugin config: {}", config_path.display()))?;
    let already_selected = document
        .get("plugins")
        .and_then(|plugins| plugins.get("selections"))
        .and_then(toml::Value::as_array)
        .is_some_and(|selections| {
            selections.iter().any(|value| {
                value.get("marketplace").and_then(toml::Value::as_str)
                    == Some(selection.marketplace.as_str())
                    && value.get("plugin").and_then(toml::Value::as_str)
                        == Some(selection.plugin.as_str())
            })
        });
    if already_selected {
        return Ok(());
    }
    let separator = if content.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let body = format!(
        "{}{separator}[[plugins.selections]]\nmarketplace = {:?}\nplugin = {:?}\n",
        content, selection.marketplace, selection.plugin
    );
    write_atomic_file(config_path, body.as_bytes())
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
    let content = fs::read_to_string(config_path).with_context(|| {
        format!(
            "failed to read config for plugin removal: {}",
            config_path.display()
        )
    })?;
    let lines: Vec<&str> = content.lines().collect();
    let mut output = Vec::with_capacity(lines.len());
    let mut index = 0;
    let mut removed = false;

    while index < lines.len() {
        if lines[index].trim() != "[[plugins.selections]]" {
            output.push(lines[index]);
            index += 1;
            continue;
        }

        let start = index;
        index += 1;
        while index < lines.len()
            && !lines[index]
                .trim_start()
                .starts_with("[[plugins.selections]]")
            && !lines[index].trim_start().starts_with('[')
        {
            index += 1;
        }
        let block = lines[start..index].join("\n");
        let value: toml::Value = toml::from_str(&block).with_context(|| {
            format!(
                "invalid plugin selection block in {}",
                config_path.display()
            )
        })?;
        let matches = value
            .get("plugins")
            .and_then(|plugins| plugins.get("selections"))
            .and_then(toml::Value::as_array)
            .and_then(|selections| selections.first())
            .is_some_and(|selection_value| {
                selection_value
                    .get("marketplace")
                    .and_then(toml::Value::as_str)
                    == Some(selection.marketplace.as_str())
                    && selection_value.get("plugin").and_then(toml::Value::as_str)
                        == Some(selection.plugin.as_str())
            });
        if matches {
            removed = true;
        } else {
            output.extend(lines[start..index].iter().copied());
        }
    }

    if !removed {
        return Ok(());
    }
    let body = format!("{}\n", output.join("\n"));
    write_atomic_file(config_path, body.as_bytes())
}

fn write_atomic_file(path: &Path, body: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temporary file in {}", parent.display()))?;
    fs::write(temporary.path(), body).with_context(|| {
        format!(
            "failed to write temporary file: {}",
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
                !Path::new(&source.location).is_absolute(),
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
        !Path::new(path).is_absolute(),
        "absolute paths are not allowed: {path}"
    );
    ensure!(
        !path.contains(':'),
        "drive-prefixed paths are not allowed: {path}"
    );
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
    source.starts_with('.') || source.starts_with('/') || !source.contains("://")
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
            let bytes = fs::read(entry.path())?;
            entries.push((relative, bytes));
        }
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = Sha256::new();
    for (path, bytes) in entries {
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

fn resolve_git_reference(repository: &str, reference: &str) -> Result<String> {
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
    let client = reqwest::blocking::Client::builder()
        .user_agent("agentsync-plugin-resolver")
        .build()
        .context("failed to create GitHub API client")?;
    let mut request = client.get(endpoint);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        request = request.bearer_auth(token);
    }
    let response: Value = request
        .send()
        .context("failed to resolve Git reference")?
        .error_for_status()
        .context("Git reference resolution failed")?
        .json()
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

fn blocking_fetch_archive(url: &str) -> Result<TempDir> {
    let future = crate::skills::install::fetch_and_unpack_to_tempdir(url);
    let result = match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => tokio::runtime::Runtime::new()?.block_on(future),
    }?;
    Ok(result)
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

    #[test]
    fn mutable_git_reference_resolves_to_a_commit_shape() {
        assert_eq!(
            resolve_git_reference(
                "https://github.com/example/repo",
                "0123456789abcdef0123456789abcdef01234567"
            )
            .unwrap(),
            "0123456789abcdef0123456789abcdef01234567"
        );
        assert!(resolve_git_reference("https://gitlab.com/example/repo", "main").is_err());
        assert!(resolve_git_reference("https://github.com/example/repo", "HEAD").is_err());
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
        assert!(normalize_relative_path("./skills/demo").is_ok());
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
