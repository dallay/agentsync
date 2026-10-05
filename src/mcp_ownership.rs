use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const SCHEMA_VERSION: u32 = 1;
const PROJECT_CONFIG_PATH_DOMAIN: &[u8] = b"agentsync:mcp-config:project-relative:v1\0";
const GLOBAL_CONFIG_PATH_DOMAIN: &[u8] = b"agentsync:mcp-config:global-absolute:v1\0";
const CONFIG_DESTINATION_LOCK_DOMAIN: &[u8] = b"agentsync:mcp-config-destination-lock:v1\0";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OwnershipManifest {
    pub schema_version: u32,
    pub project_id: String,
    pub configs: BTreeMap<String, OwnershipRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OwnershipRecord {
    pub agent_ids: BTreeSet<String>,
    pub original_content: Option<String>,
    pub pre_write_sha256: Option<String>,
    pub applied_sha256: String,
}

pub(crate) struct McpOwnershipStore {
    data_root: PathBuf,
    state_path: PathBuf,
    lock_path: PathBuf,
    project_id: String,
}

pub(crate) struct LockedOwnership {
    _lock_file: File,
    data_root: PathBuf,
    state_path: PathBuf,
    project_id: String,
    manifest: OwnershipManifest,
}

pub(crate) struct ConfigPathLock {
    _lock_file: File,
}

impl McpOwnershipStore {
    pub(crate) fn open(project_root: &Path) -> Result<Self> {
        let data_root = dirs::data_local_dir()
            .ok_or_else(|| anyhow!("could not resolve the local data directory"))?;
        Self::open_with_data_root(project_root, &data_root)
    }

    #[cfg(test)]
    pub(crate) fn open_at(project_root: &Path, data_root: &Path) -> Result<Self> {
        Self::open_with_data_root(project_root, data_root)
    }

    fn open_with_data_root(project_root: &Path, data_root: &Path) -> Result<Self> {
        let project_root = project_root
            .canonicalize()
            .context("failed to canonicalize project root")?;
        let data_root = canonical_data_root(&project_root, data_root)?;
        let project_id = sha256_path(&project_root);
        let state_dir = state_directory_path(&data_root, &project_id);

        Ok(Self {
            data_root,
            state_path: state_dir.join("ownership.json"),
            lock_path: state_dir.join("ownership.lock"),
            project_id,
        })
    }

    pub(crate) fn lock(&self) -> Result<LockedOwnership> {
        ensure_state_directory(&self.data_root, &self.project_id)?;
        let lock_file = open_lock_file(&self.lock_path)?;
        lock_file
            .lock()
            .context("failed to lock MCP ownership store")?;

        let manifest = if let Some(metadata) = existing_non_symlink(&self.state_path)? {
            if !metadata.is_file() {
                bail!("MCP ownership manifest is not a regular file");
            }
            set_private_file_permissions(&self.state_path)?;
            let contents =
                fs::read(&self.state_path).context("failed to read MCP ownership manifest")?;
            parse_manifest(&contents, &self.project_id)?
        } else {
            OwnershipManifest {
                schema_version: SCHEMA_VERSION,
                project_id: self.project_id.clone(),
                configs: BTreeMap::new(),
            }
        };

        Ok(LockedOwnership {
            _lock_file: lock_file,
            data_root: self.data_root.clone(),
            state_path: self.state_path.clone(),
            project_id: self.project_id.clone(),
            manifest,
        })
    }

    /// Lock one normalized MCP destination shared by all projects using this data root.
    ///
    /// Callers that also hold a project ownership lock must acquire that lock first,
    /// then this destination lock. Never acquire another project ownership lock while
    /// holding a destination lock.
    pub(crate) fn lock_config_path(&self, path: &Path) -> Result<ConfigPathLock> {
        let destination = canonical_normalized_destination_path(path)?;
        let lock_id = sha256_native_path(CONFIG_DESTINATION_LOCK_DOMAIN, &destination);
        let lock_dir = ensure_destination_lock_directory(&self.data_root)?;
        let lock_path = lock_dir.join(format!("{lock_id}.lock"));
        let lock_file = open_lock_file(&lock_path)?;
        lock_file
            .lock()
            .context("failed to lock MCP config destination")?;
        Ok(ConfigPathLock {
            _lock_file: lock_file,
        })
    }

    pub(crate) fn read_existing(&self) -> Result<Option<OwnershipManifest>> {
        if existing_state_directory(&self.data_root, &self.project_id)?.is_none() {
            return Ok(None);
        }
        let Some(metadata) = existing_non_symlink(&self.state_path)? else {
            return Ok(None);
        };
        if !metadata.is_file() {
            bail!("MCP ownership manifest is not a regular file");
        }
        set_private_file_permissions(&self.state_path)?;
        let contents =
            fs::read(&self.state_path).context("failed to read MCP ownership manifest")?;
        parse_manifest(&contents, &self.project_id).map(Some)
    }

    pub(crate) fn project_id(&self) -> &str {
        &self.project_id
    }

    pub(crate) fn config_path_id(&self, project_root: &Path, path: &Path) -> String {
        config_path_id(project_root, path)
    }
}

impl LockedOwnership {
    pub(crate) fn manifest(&self) -> &OwnershipManifest {
        &self.manifest
    }

    pub(crate) fn manifest_mut(&mut self) -> &mut OwnershipManifest {
        &mut self.manifest
    }

    pub(crate) fn record_before_write(
        &mut self,
        project_root: &Path,
        config_path: &Path,
        agent_ids: BTreeSet<String>,
        current_content: Option<&str>,
        applied_content: &str,
    ) -> Result<()> {
        let config_id = config_path_id(project_root, config_path);
        let current_hash = current_content.map(sha256_text);
        let previous = self.manifest.configs.get(&config_id);
        let (original_content, agent_ids) = match previous {
            Some(previous)
                if current_hash == previous.pre_write_sha256
                    || current_hash.as_deref() == Some(previous.applied_sha256.as_str()) =>
            {
                let mut owners = previous.agent_ids.clone();
                owners.extend(agent_ids);
                (previous.original_content.clone(), owners)
            }
            Some(_) => bail!(
                "MCP config {} no longer matches its ownership journal; refusing to rebase ownership snapshot",
                config_path.display()
            ),
            None => (current_content.map(str::to_owned), agent_ids),
        };

        self.manifest.configs.insert(
            config_id,
            OwnershipRecord {
                agent_ids,
                original_content,
                pre_write_sha256: current_hash,
                applied_sha256: sha256_text(applied_content),
            },
        );
        Ok(())
    }

    pub(crate) fn add_owners_if_recorded(
        &mut self,
        project_root: &Path,
        config_path: &Path,
        current_content: Option<&str>,
        agent_ids: BTreeSet<String>,
    ) -> bool {
        let Some(current_hash) = current_content.map(sha256_text) else {
            return false;
        };
        let config_path = config_path_id(project_root, config_path);
        let Some(record) = self.manifest.configs.get_mut(&config_path) else {
            return false;
        };
        if current_hash != record.applied_sha256
            && Some(current_hash.as_str()) != record.pre_write_sha256.as_deref()
        {
            return false;
        }

        let previous_owner_count = record.agent_ids.len();
        record.agent_ids.extend(agent_ids);
        record.agent_ids.len() != previous_owner_count
    }

    pub(crate) fn remove_record(&mut self, project_root: &Path, config_path: &Path) {
        self.manifest
            .configs
            .remove(&config_path_id(project_root, config_path));
    }

    pub(crate) fn persist(&mut self) -> Result<()> {
        self.persist_with_prewrite_hook(|_| Ok(()))
    }

    fn persist_with_prewrite_hook<F>(&mut self, before_write: F) -> Result<()>
    where
        F: FnOnce(&Path) -> Result<()>,
    {
        if self.manifest.schema_version != SCHEMA_VERSION {
            bail!(
                "unsupported MCP ownership schema version {}",
                self.manifest.schema_version
            );
        }
        if self.manifest.project_id != self.project_id {
            bail!("MCP ownership manifest belongs to a different project");
        }
        let contents = serde_json::to_vec_pretty(&self.manifest)
            .context("failed to serialize MCP ownership manifest")?;
        let parent = self
            .state_path
            .parent()
            .ok_or_else(|| anyhow!("MCP ownership manifest has no parent directory"))?;
        if ensure_state_directory(&self.data_root, &self.project_id)? != parent {
            bail!("MCP ownership state directory does not match its project id");
        }
        existing_non_symlink(&self.state_path)?;
        let mut temp_file = NamedTempFile::new_in(parent)
            .context("failed to create temporary MCP ownership manifest")?;
        set_private_file_permissions(temp_file.path())?;
        before_write(temp_file.path())?;
        temp_file
            .write_all(&contents)
            .context("failed to write temporary MCP ownership manifest")?;
        temp_file
            .as_file()
            .sync_all()
            .context("failed to sync temporary MCP ownership manifest")?;
        temp_file
            .persist(&self.state_path)
            .map_err(|error| error.error)
            .context("failed to persist MCP ownership manifest")?;
        Ok(())
    }

    pub(crate) fn persist_or_remove_empty(&mut self) -> Result<()> {
        if !self.manifest.configs.is_empty() {
            return self.persist();
        }
        let Some(metadata) = existing_non_symlink(&self.state_path)? else {
            return Ok(());
        };
        if !metadata.is_file() {
            bail!("MCP ownership manifest is not a regular file");
        }
        fs::remove_file(&self.state_path).context("failed to remove empty MCP ownership manifest")
    }
}

fn sha256_text(value: &str) -> String {
    sha256_bytes(value.as_bytes())
}

fn config_path_id(project_root: &Path, path: &Path) -> String {
    let normalized_path = normalize_path(path);
    let project_relative_path = if path.is_absolute() {
        let normalized_root = normalize_path(project_root);
        let lexical_match = normalized_root
            .is_absolute()
            .then(|| normalized_path.strip_prefix(&normalized_root).ok())
            .flatten()
            .map(normalize_path);
        lexical_match.or_else(|| {
            project_root
                .canonicalize()
                .ok()
                .and_then(|root| normalized_path.strip_prefix(root).ok().map(normalize_path))
        })
    } else {
        let normalized_root = normalize_path(project_root);
        if normalized_root.as_os_str().is_empty() {
            Some(normalized_path.clone())
        } else {
            Some(
                normalized_path
                    .strip_prefix(&normalized_root)
                    .map(normalize_path)
                    .unwrap_or(normalized_path.clone()),
            )
        }
    };

    match project_relative_path {
        Some(path) => sha256_native_path(PROJECT_CONFIG_PATH_DOMAIN, &path),
        None => sha256_native_path(GLOBAL_CONFIG_PATH_DOMAIN, &normalized_path),
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized.file_name().is_some_and(|name| name != "..") {
                    normalized.pop();
                } else if !normalized.has_root() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn canonical_normalized_destination_path(path: &Path) -> Result<PathBuf> {
    use std::path::Component;

    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to resolve current directory for MCP destination")?
            .join(path)
    };
    let normalized_path = normalize_path(&absolute_path);
    match fs::canonicalize(&normalized_path) {
        Ok(path) => return Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("failed to canonicalize MCP destination path"),
    }

    let mut existing_ancestor = normalized_path.as_path();
    let mut missing_components = Vec::new();
    loop {
        match fs::symlink_metadata(existing_ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(Component::Normal(name)) = existing_ancestor.components().next_back()
                else {
                    bail!("MCP destination has a missing non-normal path component");
                };
                missing_components.push(name.to_os_string());
                existing_ancestor = existing_ancestor
                    .parent()
                    .ok_or_else(|| anyhow!("MCP destination has no existing ancestor"))?;
            }
            Err(error) => return Err(error).context("failed to inspect MCP destination path"),
        }
    }

    let mut destination = fs::canonicalize(existing_ancestor)
        .context("failed to canonicalize MCP destination ancestor")?;
    if !missing_components.is_empty()
        && !fs::metadata(&destination)
            .context("failed to inspect MCP destination ancestor")?
            .is_dir()
    {
        bail!("MCP destination ancestor is not a directory");
    }
    for component in missing_components.iter().rev() {
        destination.push(component);
    }
    Ok(destination)
}

#[cfg(unix)]
fn sha256_native_path(domain: &[u8], path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;

    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(path.as_os_str().as_bytes());
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(windows)]
fn sha256_native_path(domain: &[u8], path: &Path) -> String {
    use std::os::windows::ffi::OsStrExt;

    let mut digest = Sha256::new();
    digest.update(domain);
    for unit in path.as_os_str().encode_wide() {
        digest.update(unit.to_le_bytes());
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn sha256_native_path(domain: &[u8], path: &Path) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(path.to_string_lossy().as_bytes());
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn sha256_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn parse_manifest(contents: &[u8], project_id: &str) -> Result<OwnershipManifest> {
    let manifest: OwnershipManifest =
        serde_json::from_slice(contents).context("MCP ownership manifest is malformed")?;
    if manifest.schema_version != SCHEMA_VERSION {
        bail!(
            "unsupported MCP ownership schema version {}",
            manifest.schema_version
        );
    }
    if manifest.project_id != project_id {
        bail!("MCP ownership manifest belongs to a different project");
    }
    Ok(manifest)
}

#[cfg(unix)]
fn sha256_path(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;

    sha256_bytes(path.as_os_str().as_bytes())
}

#[cfg(windows)]
fn sha256_path(path: &Path) -> String {
    use std::os::windows::ffi::OsStrExt;

    let mut digest = Sha256::new();
    for unit in path.as_os_str().encode_wide() {
        digest.update(unit.to_le_bytes());
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn sha256_path(path: &Path) -> String {
    sha256_text(&path.to_string_lossy())
}

fn ensure_state_directory(data_root: &Path, project_id: &str) -> Result<PathBuf> {
    fs::create_dir_all(data_root).context("failed to create local data directory")?;
    let mut state_dir = data_root.to_path_buf();
    for component in ["agentsync", "mcp-ownership", project_id] {
        state_dir.push(component);
        match fs::create_dir(&state_dir) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error).context("failed to create MCP ownership state directory");
            }
        }
        let metadata = existing_non_symlink(&state_dir)?
            .ok_or_else(|| anyhow!("MCP ownership state directory disappeared"))?;
        if !metadata.is_dir() {
            bail!("MCP ownership state path is not a directory");
        }
        set_private_directory_permissions(&state_dir)?;
    }
    Ok(state_dir)
}

fn ensure_destination_lock_directory(data_root: &Path) -> Result<PathBuf> {
    fs::create_dir_all(data_root).context("failed to create local data directory for MCP locks")?;
    let metadata = existing_non_symlink(data_root)?
        .ok_or_else(|| anyhow!("MCP destination lock data directory disappeared"))?;
    if !metadata.is_dir() {
        bail!("MCP destination lock data path is not a directory");
    }

    let mut lock_dir = data_root.to_path_buf();
    for component in ["agentsync", "mcp-config-locks"] {
        lock_dir.push(component);
        match fs::create_dir(&lock_dir) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error).context("failed to create MCP destination lock directory");
            }
        }
        let metadata = existing_non_symlink(&lock_dir)?
            .ok_or_else(|| anyhow!("MCP destination lock directory disappeared"))?;
        if !metadata.is_dir() {
            bail!("MCP destination lock path is not a directory");
        }
        set_private_directory_permissions(&lock_dir)?;
    }
    Ok(lock_dir)
}

fn state_directory_path(data_root: &Path, project_id: &str) -> PathBuf {
    data_root
        .join("agentsync")
        .join("mcp-ownership")
        .join(project_id)
}

fn canonical_data_root(project_root: &Path, data_root: &Path) -> Result<PathBuf> {
    use std::path::Component;

    let absolute_data_root = if data_root.is_absolute() {
        data_root.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to resolve current directory")?
            .join(data_root)
    };
    let mut existing_ancestor = absolute_data_root.as_path();
    let mut missing_components = Vec::new();

    loop {
        match fs::symlink_metadata(existing_ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(Component::Normal(name)) = existing_ancestor.components().next_back()
                else {
                    bail!("local data directory has a missing non-normal path component");
                };
                missing_components.push(name.to_os_string());
                existing_ancestor = existing_ancestor
                    .parent()
                    .ok_or_else(|| anyhow!("local data directory has no existing ancestor"))?;
            }
            Err(error) => return Err(error).context("failed to inspect local data directory"),
        }
    }

    let mut resolved_data_root = fs::canonicalize(existing_ancestor)
        .context("failed to resolve local data directory ancestor")?;
    let ancestor_metadata = fs::metadata(&resolved_data_root)
        .context("failed to inspect resolved local data directory ancestor")?;
    if !ancestor_metadata.is_dir() {
        bail!("local data directory ancestor is not a directory");
    }
    for component in missing_components.iter().rev() {
        resolved_data_root.push(component);
    }

    if resolved_data_root.starts_with(project_root) {
        bail!("MCP ownership journal directory must be outside the project root");
    }

    Ok(resolved_data_root)
}

fn existing_state_directory(data_root: &Path, project_id: &str) -> Result<Option<PathBuf>> {
    let Some(metadata) = existing_non_symlink(data_root)? else {
        return Ok(None);
    };
    if !metadata.is_dir() {
        bail!("MCP ownership state path is not a directory");
    }

    let mut state_dir = data_root.to_path_buf();
    for component in ["agentsync", "mcp-ownership", project_id] {
        state_dir.push(component);
        let Some(metadata) = existing_non_symlink(&state_dir)? else {
            return Ok(None);
        };
        if !metadata.is_dir() {
            bail!("MCP ownership state path is not a directory");
        }
        set_private_directory_permissions(&state_dir)?;
    }
    Ok(Some(state_dir))
}

fn existing_non_symlink(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("refusing symlink in MCP ownership state");
        }
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("failed to inspect MCP ownership state"),
    }
}

fn open_lock_file(path: &Path) -> Result<File> {
    let mut create_options = OpenOptions::new();
    create_options.create_new(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        create_options.mode(0o600);
    }
    let file = match create_options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = existing_non_symlink(path)?
                .ok_or_else(|| anyhow!("MCP ownership lock disappeared"))?;
            if !metadata.is_file() {
                bail!("MCP ownership lock is not a regular file");
            }
            set_private_file_permissions(path)?;
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .context("failed to open MCP ownership lock")?
        }
        Err(error) => {
            return Err(error).context("failed to create MCP ownership lock");
        }
    };
    set_private_file_permissions(path)?;
    Ok(file)
}

fn set_private_directory_permissions(path: &Path) -> Result<()> {
    crate::mcp::set_restricted_directory_permissions(path)
        .context("failed to restrict MCP ownership directory")
}

fn set_private_file_permissions(path: &Path) -> Result<()> {
    crate::mcp::set_restricted_permissions(path).context("failed to restrict MCP ownership file")
}

#[cfg(test)]
mod tests {
    use super::{McpOwnershipStore, OwnershipRecord, sha256_text};
    use std::collections::BTreeSet;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn ownership_store_roundtrips_records() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        std::fs::create_dir_all(&project).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        let original = Some("{\"mcpServers\":{\"search\":{\"command\":\"old\"}}}".to_string());
        let existing_file = OwnershipRecord {
            agent_ids: BTreeSet::from(["claude".to_string()]),
            original_content: original.clone(),
            pre_write_sha256: Some(sha256_text(original.as_deref().unwrap())),
            applied_sha256: sha256_text("{\"mcpServers\":{\"search\":{\"command\":\"managed\"}}}"),
        };
        let new_file = OwnershipRecord {
            agent_ids: BTreeSet::from(["opencode".to_string()]),
            original_content: None,
            pre_write_sha256: None,
            applied_sha256: sha256_text("{\"mcp\":{\"search\":{\"command\":[\"managed\"]}}}"),
        };
        let claude_config_id = store.config_path_id(&project, &project.join(".mcp.json"));
        let opencode_config_id = store.config_path_id(&project, &project.join("opencode.json"));
        {
            let mut locked = store.lock().unwrap();
            locked
                .manifest_mut()
                .configs
                .insert(claude_config_id.clone(), existing_file);
            locked
                .manifest_mut()
                .configs
                .insert(opencode_config_id.clone(), new_file);
            locked.persist().unwrap();
        }
        let loaded = store.lock().unwrap();
        assert_eq!(
            loaded.manifest().configs[&claude_config_id].original_content,
            original
        );
        assert_eq!(
            loaded.manifest().configs[&opencode_config_id].original_content,
            None
        );
        assert_eq!(loaded.manifest().project_id, store.project_id());
    }

    #[test]
    fn opening_missing_ownership_store_has_no_filesystem_side_effect() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        fs::create_dir_all(&project).unwrap();

        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();

        assert_eq!(store.project_id().len(), 64);
        assert!(store.read_existing().unwrap().is_none());
        assert!(
            !data_root.exists(),
            "opening the store must not create paths"
        );
    }

    #[test]
    fn ownership_manifest_is_secured_before_snapshot_bytes_are_written() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        fs::create_dir_all(&project).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        let mut locked = store.lock().unwrap();

        locked
            .persist_with_prewrite_hook(|temp_path| {
                assert_eq!(
                    fs::metadata(temp_path)?.len(),
                    0,
                    "manifest bytes must not be written before permissions are enforced"
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn corrupted_manifest_is_rejected_without_overwriting() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        std::fs::create_dir_all(&project).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        drop(store.lock().unwrap());
        let corrupted = b"{\"schema_version\":";
        std::fs::write(&store.state_path, corrupted).unwrap();

        assert!(store.read_existing().is_err());
        assert!(store.lock().is_err());
        assert_eq!(std::fs::read(&store.state_path).unwrap(), corrupted);
    }

    #[test]
    fn manifest_with_different_project_id_is_rejected() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        std::fs::create_dir_all(&project).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        drop(store.lock().unwrap());
        std::fs::write(
            &store.state_path,
            r#"{"schema_version":1,"project_id":"different-project","configs":{}}"#,
        )
        .unwrap();

        assert!(store.read_existing().is_err());
        assert!(store.lock().is_err());
    }

    #[test]
    fn manifest_with_unsupported_schema_is_rejected() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        std::fs::create_dir_all(&project).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        drop(store.lock().unwrap());
        std::fs::write(
            &store.state_path,
            format!(
                "{{\"schema_version\":2,\"project_id\":\"{}\",\"configs\":{{}}}}",
                store.project_id()
            ),
        )
        .unwrap();

        assert!(store.read_existing().is_err());
        assert!(store.lock().is_err());
    }

    #[test]
    fn record_before_write_preserves_original_and_unions_owners_on_reapply() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        std::fs::create_dir_all(&project).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        let mut locked = store.lock().unwrap();
        let path = std::path::Path::new(".mcp.json");
        let config_id = store.config_path_id(&project, path);

        locked
            .record_before_write(
                &project,
                path,
                BTreeSet::from(["claude".to_string()]),
                Some("original"),
                "applied once",
            )
            .unwrap();
        locked
            .record_before_write(
                &project,
                path,
                BTreeSet::from(["opencode".to_string()]),
                Some("applied once"),
                "applied twice",
            )
            .unwrap();

        let record = &locked.manifest().configs[&config_id];
        let pre_write_hash = sha256_text("applied once");
        assert_eq!(record.original_content.as_deref(), Some("original"));
        assert_eq!(
            record.pre_write_sha256.as_deref(),
            Some(pre_write_hash.as_str())
        );
        assert_eq!(record.applied_sha256, sha256_text("applied twice"));
        assert_eq!(
            record.agent_ids,
            BTreeSet::from(["claude".into(), "opencode".into()])
        );
    }

    #[test]
    fn add_owners_if_recorded_preserves_snapshot_and_hashes() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        fs::create_dir_all(&project).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        let path = std::path::Path::new(".mcp.json");
        let config_id = store.config_path_id(&project, path);

        let mut locked = store.lock().unwrap();
        assert!(!locked.add_owners_if_recorded(
            &project,
            path,
            Some("original"),
            BTreeSet::from(["vscode".to_string()]),
        ));
        assert!(locked.manifest().configs.is_empty());

        locked
            .record_before_write(
                &project,
                path,
                BTreeSet::from(["vscode".to_string()]),
                Some("original"),
                "applied",
            )
            .unwrap();
        let original_content = locked.manifest().configs[&config_id]
            .original_content
            .clone();
        let pre_write_sha256 = locked.manifest().configs[&config_id]
            .pre_write_sha256
            .clone();
        let applied_sha256 = locked.manifest().configs[&config_id].applied_sha256.clone();

        assert!(locked.add_owners_if_recorded(
            &project,
            path,
            Some("applied"),
            BTreeSet::from(["copilot".to_string()]),
        ));
        let record = &locked.manifest().configs[&config_id];
        assert_eq!(record.original_content, original_content);
        assert_eq!(record.pre_write_sha256, pre_write_sha256);
        assert_eq!(record.applied_sha256, applied_sha256);
        assert_eq!(
            record.agent_ids,
            BTreeSet::from(["copilot".into(), "vscode".into()])
        );

        assert!(locked.add_owners_if_recorded(
            &project,
            path,
            Some("original"),
            BTreeSet::from(["cursor".to_string()]),
        ));
        assert_eq!(
            locked.manifest().configs[&config_id].agent_ids,
            BTreeSet::from(["copilot".into(), "cursor".into(), "vscode".into()])
        );

        assert!(!locked.add_owners_if_recorded(
            &project,
            path,
            Some("externally changed"),
            BTreeSet::from(["claude".to_string()]),
        ));
        assert_eq!(
            locked.manifest().configs[&config_id].agent_ids,
            BTreeSet::from(["copilot".into(), "cursor".into(), "vscode".into()])
        );
    }

    #[test]
    fn record_before_write_rejects_unrecognized_content_without_mutating_record() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        std::fs::create_dir_all(&project).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        let mut locked = store.lock().unwrap();
        let path = std::path::Path::new(".mcp.json");
        let config_id = store.config_path_id(&project, path);

        locked
            .record_before_write(
                &project,
                path,
                BTreeSet::from(["claude".to_string()]),
                Some("original"),
                "applied",
            )
            .unwrap();
        let original_record = locked.manifest().configs[&config_id].clone();
        let manifest_before = serde_json::to_vec(locked.manifest()).unwrap();

        let error = locked
            .record_before_write(
                &project,
                path,
                BTreeSet::from(["opencode".to_string()]),
                Some("foreign content"),
                "applied again",
            )
            .unwrap_err();

        assert!(error.to_string().contains("refusing to rebase"));
        assert_eq!(
            serde_json::to_vec(locked.manifest()).unwrap(),
            manifest_before,
            "a rejected reapply must leave the complete manifest unchanged"
        );
        let record = &locked.manifest().configs[&config_id];
        assert_eq!(record.original_content, original_record.original_content);
        assert_eq!(record.pre_write_sha256, original_record.pre_write_sha256);
        assert_eq!(record.applied_sha256, original_record.applied_sha256);
        assert_eq!(record.agent_ids, original_record.agent_ids);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_manifest_is_rejected() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        std::fs::create_dir_all(&project).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        drop(store.lock().unwrap());
        let external_manifest = temp.path().join("external.json");
        std::fs::write(
            &external_manifest,
            format!(
                "{{\"schema_version\":1,\"project_id\":\"{}\",\"configs\":{{}}}}",
                store.project_id()
            ),
        )
        .unwrap();
        symlink(&external_manifest, &store.state_path).unwrap();

        assert!(store.read_existing().is_err());
        assert!(store.lock().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_state_directory_is_rejected() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        let external_state_dir = temp.path().join("external-state");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&external_state_dir).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        let state_dir = store.state_path.parent().unwrap();
        let ownership_root = state_dir.parent().unwrap();
        std::fs::create_dir_all(ownership_root).unwrap();
        symlink(&external_state_dir, state_dir).unwrap();

        assert!(store.read_existing().is_err());
        assert!(store.lock().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_lock_is_rejected() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        std::fs::create_dir_all(&project).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        drop(store.lock().unwrap());
        let external_lock = temp.path().join("external.lock");
        std::fs::write(&external_lock, b"").unwrap();
        std::fs::remove_file(&store.lock_path).unwrap();
        symlink(&external_lock, &store.lock_path).unwrap();

        assert!(store.lock().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn ownership_store_uses_private_directory_and_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        std::fs::create_dir_all(&project).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        let mut locked = store.lock().unwrap();
        locked.persist().unwrap();

        let state_dir = store.state_path.parent().unwrap();
        assert_eq!(
            std::fs::metadata(state_dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&store.state_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&store.lock_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        drop(locked);
        std::fs::set_permissions(&store.state_path, std::fs::Permissions::from_mode(0o644))
            .unwrap();
        std::fs::set_permissions(state_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(store.read_existing().unwrap().is_some());
        assert_eq!(
            std::fs::metadata(state_dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&store.state_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn different_canonical_project_roots_use_separate_state() {
        let temp = TempDir::new().unwrap();
        let first_project = temp.path().join("first-project");
        let second_project = temp.path().join("second-project");
        let data_root = temp.path().join("local-data");
        std::fs::create_dir_all(&first_project).unwrap();
        std::fs::create_dir_all(&second_project).unwrap();
        let first_store = McpOwnershipStore::open_at(&first_project, &data_root).unwrap();
        let second_store = McpOwnershipStore::open_at(&second_project, &data_root).unwrap();
        assert_ne!(first_store.project_id(), second_store.project_id());
        assert_ne!(first_store.state_path, second_store.state_path);
    }

    #[cfg(unix)]
    #[test]
    fn ownership_store_distinguishes_non_utf8_project_roots() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let temp = TempDir::new().unwrap();
        let data_root = temp.path().join("local-data");
        let first_project = temp
            .path()
            .join(OsString::from_vec(b"project-\x80".to_vec()));
        let second_project = temp
            .path()
            .join(OsString::from_vec(b"project-\x81".to_vec()));
        fs::create_dir_all(&first_project).unwrap();
        fs::create_dir_all(&second_project).unwrap();

        let first_store = McpOwnershipStore::open_at(&first_project, &data_root).unwrap();
        let second_store = McpOwnershipStore::open_at(&second_project, &data_root).unwrap();

        assert_ne!(first_store.project_id(), second_store.project_id());
    }

    #[cfg(unix)]
    #[test]
    fn ownership_config_ids_distinguish_non_utf8_paths() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        fs::create_dir_all(&project).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        let first_config = project.join(OsString::from_vec(b"mcp-\x80.json".to_vec()));
        let second_config = project.join(OsString::from_vec(b"mcp-\x81.json".to_vec()));
        assert_ne!(
            store.config_path_id(&project, &first_config),
            store.config_path_id(&project, &second_config)
        );
        let mut locked = store.lock().unwrap();

        locked
            .record_before_write(
                &project,
                &first_config,
                BTreeSet::from(["claude".to_string()]),
                None,
                "first applied",
            )
            .unwrap();
        locked
            .record_before_write(
                &project,
                &second_config,
                BTreeSet::from(["claude".to_string()]),
                None,
                "second applied",
            )
            .unwrap();

        assert_eq!(locked.manifest().configs.len(), 2);
    }

    #[test]
    fn ownership_config_ids_are_relative_to_the_project_root() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        fs::create_dir_all(&project).unwrap();
        let relative_root_store =
            McpOwnershipStore::open_at(std::path::Path::new("."), &data_root).unwrap();
        let absolute_root_store = McpOwnershipStore::open_at(&project, &data_root).unwrap();

        assert_eq!(
            relative_root_store.config_path_id(
                std::path::Path::new("."),
                std::path::Path::new("./.mcp.json")
            ),
            absolute_root_store.config_path_id(&project, &project.join(".mcp.json"))
        );

        let global_config = temp.path().join("global-mcp.json");
        assert_eq!(
            relative_root_store.config_path_id(std::path::Path::new("."), &global_config),
            absolute_root_store.config_path_id(&project, &global_config)
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_data_root_uses_one_canonical_read_write_location() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let real_data_root = temp.path().join("real-data");
        let data_root_link = temp.path().join("data-link");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&real_data_root).unwrap();
        symlink(&real_data_root, &data_root_link).unwrap();

        let store = McpOwnershipStore::open_at(&project, &data_root_link).unwrap();
        let expected_root = real_data_root.canonicalize().unwrap();
        assert_eq!(
            store.state_path,
            expected_root
                .join("agentsync/mcp-ownership")
                .join(store.project_id())
                .join("ownership.json")
        );

        store.lock().unwrap().persist().unwrap();
        assert!(store.read_existing().unwrap().is_some());
    }

    #[cfg(unix)]
    #[test]
    fn missing_data_root_resolves_symlinked_existing_ancestor_without_creating_it() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let real_data_parent = temp.path().join("real-data-parent");
        let data_parent_link = temp.path().join("data-parent-link");
        let data_root = data_parent_link.join("missing/nested");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&real_data_parent).unwrap();
        symlink(&real_data_parent, &data_parent_link).unwrap();

        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        let expected_root = real_data_parent.join("missing/nested");
        assert_eq!(
            store.state_path,
            expected_root
                .join("agentsync/mcp-ownership")
                .join(store.project_id())
                .join("ownership.json")
        );
        assert!(store.read_existing().unwrap().is_none());
        assert!(!real_data_parent.join("missing").exists());
    }

    #[cfg(unix)]
    #[test]
    fn dangling_data_root_symlink_is_rejected_without_side_effects() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("dangling-data");
        fs::create_dir_all(&project).unwrap();
        symlink(temp.path().join("missing-target"), &data_root).unwrap();

        assert!(McpOwnershipStore::open_at(&project, &data_root).is_err());
        assert!(data_root.is_symlink());
    }

    #[test]
    fn data_root_inside_project_is_rejected_before_creating_state() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = project.join(".local-data");
        fs::create_dir_all(&project).unwrap();

        assert!(McpOwnershipStore::open_at(&project, &data_root).is_err());
        assert!(!data_root.exists());
    }

    #[test]
    fn data_root_with_non_directory_existing_ancestor_is_rejected() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let file_ancestor = temp.path().join("data-file");
        fs::create_dir_all(&project).unwrap();
        fs::write(&file_ancestor, b"not a directory").unwrap();
        let data_root = file_ancestor.join("nested");

        assert!(McpOwnershipStore::open_at(&project, &data_root).is_err());
        assert_eq!(fs::read(&file_ancestor).unwrap(), b"not a directory");
    }

    #[test]
    fn second_lock_handle_is_blocked_until_guard_is_dropped() {
        use std::fs::OpenOptions;
        use std::fs::TryLockError;

        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        std::fs::create_dir_all(&project).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();
        let first_guard = store.lock().unwrap();
        let second_handle = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&store.lock_path)
            .unwrap();

        assert!(matches!(
            second_handle.try_lock(),
            Err(TryLockError::WouldBlock)
        ));
        drop(first_guard);
        assert!(second_handle.try_lock().is_ok());
    }

    #[test]
    fn destination_locks_are_shared_across_projects_and_independent_by_path() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Duration;

        let temp = TempDir::new().unwrap();
        let first_project = temp.path().join("first-project");
        let second_project = temp.path().join("second-project");
        let data_root = temp.path().join("local-data");
        fs::create_dir_all(&first_project).unwrap();
        fs::create_dir_all(&second_project).unwrap();
        let first_store = McpOwnershipStore::open_at(&first_project, &data_root).unwrap();
        let second_store = McpOwnershipStore::open_at(&second_project, &data_root).unwrap();
        let global_config = temp.path().join("claude-desktop-mcp.json");
        let global_alias = temp.path().join("unused").join("..");
        let global_alias = global_alias.join("claude-desktop-mcp.json");
        let other_config = temp.path().join("other-mcp.json");

        assert!(first_store.read_existing().unwrap().is_none());
        assert!(second_store.read_existing().unwrap().is_none());
        assert!(
            !data_root.exists(),
            "opening and reading missing stores must not create lock state"
        );

        let first_guard = first_store.lock_config_path(&global_config).unwrap();

        let (same_started_tx, same_started_rx) = mpsc::channel();
        let (same_acquired_tx, same_acquired_rx) = mpsc::channel();
        let same_store = second_store;
        let same_path = global_alias;
        let same_lock_thread = thread::spawn(move || {
            same_started_tx.send(()).unwrap();
            let _guard = same_store.lock_config_path(&same_path).unwrap();
            same_acquired_tx.send(()).unwrap();
        });
        same_started_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        assert!(
            same_acquired_rx
                .recv_timeout(Duration::from_millis(100))
                .is_err()
        );

        let (other_started_tx, other_started_rx) = mpsc::channel();
        let (other_acquired_tx, other_acquired_rx) = mpsc::channel();
        let other_store = McpOwnershipStore::open_at(&first_project, &data_root).unwrap();
        let other_lock_thread = thread::spawn(move || {
            other_started_tx.send(()).unwrap();
            let _guard = other_store.lock_config_path(&other_config).unwrap();
            other_acquired_tx.send(()).unwrap();
        });
        other_started_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        let other_path_was_independent = other_acquired_rx
            .recv_timeout(Duration::from_secs(1))
            .is_ok();

        drop(first_guard);
        same_lock_thread.join().unwrap();
        other_lock_thread.join().unwrap();

        assert!(
            other_path_was_independent,
            "different destination paths must use independent locks"
        );
        assert!(
            same_acquired_rx
                .recv_timeout(Duration::from_secs(1))
                .is_ok(),
            "the shared destination lock must become available after its guard drops"
        );
    }

    #[cfg(unix)]
    #[test]
    fn destination_lock_files_are_private_persistent_and_reject_symlinks() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let data_root = temp.path().join("local-data");
        let destination = temp.path().join("secret-mcp-config.json");
        fs::create_dir_all(&project).unwrap();
        let store = McpOwnershipStore::open_at(&project, &data_root).unwrap();

        let guard = store.lock_config_path(&destination).unwrap();
        let lock_dir = data_root.join("agentsync/mcp-config-locks");
        let lock_path = fs::read_dir(&lock_dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert!(
            !lock_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .contains("secret")
        );
        assert!(fs::read(&lock_path).unwrap().is_empty());
        assert_eq!(
            fs::metadata(&lock_dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&lock_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        drop(guard);
        assert!(
            lock_path.exists(),
            "destination lock files must persist after unlock"
        );

        fs::remove_file(&lock_path).unwrap();
        let external_lock = temp.path().join("external.lock");
        fs::write(&external_lock, b"").unwrap();
        symlink(&external_lock, &lock_path).unwrap();
        assert!(store.lock_config_path(&destination).is_err());

        let second_data_root = temp.path().join("second-local-data");
        let second_project = temp.path().join("second-project");
        let external_lock_dir = temp.path().join("external-lock-dir");
        fs::create_dir_all(&second_project).unwrap();
        fs::create_dir_all(second_data_root.join("agentsync")).unwrap();
        fs::create_dir_all(&external_lock_dir).unwrap();
        symlink(
            &external_lock_dir,
            second_data_root.join("agentsync/mcp-config-locks"),
        )
        .unwrap();
        let second_store = McpOwnershipStore::open_at(&second_project, &second_data_root).unwrap();
        assert!(second_store.lock_config_path(&destination).is_err());
        assert_eq!(fs::read_dir(external_lock_dir).unwrap().count(), 0);
    }
}
