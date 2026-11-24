//! Reproducible environment lock.
//!
//! Generates a deterministic lockfile containing SHA-256 hashes of all
//! AgentSync-managed configuration files, skills, plugins, and MCP servers.
//! The lockfile enables drift detection and CI verification.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::profile::profiles_dir_for_project;

/// Schema version for the environment lockfile.
pub const ENV_LOCK_SCHEMA_VERSION: u32 = 1;

/// Default lockfile name.
pub const ENV_LOCK_FILE: &str = "agentsync.lock.json";

/// A single hashed entry in the lockfile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockEntry {
    /// Path relative to project root.
    pub path: String,
    /// SHA-256 hex digest of the file contents.
    pub hash: String,
}

/// Category of locked items.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum LockCategory {
    Config,
    Skill,
    Plugin,
    Mcp,
    Profile,
    Other,
}

/// The environment lockfile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvLock {
    /// Schema version.
    pub schema_version: u32,
    /// Lock entries grouped by category.
    pub entries: BTreeMap<LockCategory, Vec<LockEntry>>,
    /// Total number of entries.
    pub total: usize,
}

impl Default for EnvLock {
    fn default() -> Self {
        Self {
            schema_version: ENV_LOCK_SCHEMA_VERSION,
            entries: BTreeMap::new(),
            total: 0,
        }
    }
}

impl EnvLock {
    /// Add an entry to the lock.
    pub fn add_entry(&mut self, category: LockCategory, path: String, hash: String) {
        self.entries
            .entry(category)
            .or_default()
            .push(LockEntry { path, hash });
        self.total += 1;
    }

    /// Get the lockfile path for a project.
    pub fn lockfile_path(project_root: &Path) -> PathBuf {
        project_root.join(".agents").join(ENV_LOCK_FILE)
    }

    /// Load a lockfile from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = Self::lockfile_path(project_root);
        // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- path is fixed lockfile location within project
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read lockfile: {}", path.display()))?;
        let lock: EnvLock = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse lockfile: {}", path.display()))?;
        Ok(lock)
    }

    /// Save a lockfile to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let path = Self::lockfile_path(project_root);
        let dir = path.parent().ok_or_else(|| {
            anyhow::anyhow!("Lockfile path has no parent directory: {}", path.display())
        })?;
        fs::create_dir_all(dir)
            .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
        let content = serde_json::to_string_pretty(self).context("Failed to serialize lockfile")?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write lockfile: {}", path.display()))?;
        Ok(())
    }
}

/// Result of a lock verification.
#[derive(Debug, Clone)]
pub struct VerifyResult {
    /// Whether the environment matches the lockfile.
    pub matches: bool,
    /// Files that were added since the lock was generated.
    pub added: Vec<String>,
    /// Files that were modified since the lock was generated.
    pub modified: Vec<String>,
    /// Files that were removed since the lock was generated.
    pub removed: Vec<String>,
}

/// Generate a lockfile from the current project state.
pub fn generate_lock(project_root: &Path, config: &Config) -> Result<EnvLock> {
    let mut lock = EnvLock::default();

    // Hash the main config file
    let config_path = project_root.join(".agents").join("agentsync.toml");
    if config_path.exists()
        && let Some(hash) = hash_file(&config_path)?
    {
        lock.add_entry(
            LockCategory::Config,
            config_path
                .strip_prefix(project_root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".agents/agentsync.toml".to_string()),
            hash,
        );
    }

    // Hash skills
    let skills_dir = project_root.join(".agents").join("skills");
    if skills_dir.exists() {
        hash_directory_recursively(&mut lock, &skills_dir, project_root, LockCategory::Skill)?;
    }

    // Hash plugins
    let plugins_dir = project_root.join(".agents").join("plugins");
    if plugins_dir.exists() {
        hash_directory_recursively(&mut lock, &plugins_dir, project_root, LockCategory::Plugin)?;
    }

    // Hash profiles
    let profiles_dir = profiles_dir_for_project(project_root);
    if profiles_dir.exists() {
        hash_directory_recursively(
            &mut lock,
            &profiles_dir,
            project_root,
            LockCategory::Profile,
        )?;
    }

    // Hash MCP configs if defined
    if !config.mcp_servers.is_empty() {
        for (name, server) in &config.mcp_servers {
            if !server.disabled {
                let mut hasher = Sha256::new();
                hasher.update(name.as_bytes());
                if let Some(ref cmd) = server.command {
                    hasher.update(cmd.as_bytes());
                }
                for arg in &server.args {
                    hasher.update(arg.as_bytes());
                }
                // Skip env vars for reproducibility (they may contain secrets)
                let result = hasher.finalize();
                let hash: String = result.iter().map(|b| format!("{:02x}", b)).collect();
                lock.add_entry(LockCategory::Mcp, format!("mcp_servers/{}", name), hash);
            }
        }
    }

    // Sort entries for determinism
    for entries in lock.entries.values_mut() {
        entries.sort_by(|a, b| a.path.cmp(&b.path));
    }

    Ok(lock)
}

/// Verify the current environment against a lockfile.
pub fn verify_lock(project_root: &Path, config: &Config, lock: &EnvLock) -> Result<VerifyResult> {
    let current = generate_lock(project_root, config)?;

    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut removed = Vec::new();

    // Build maps for comparison
    let current_map: BTreeMap<String, String> = current
        .entries
        .values()
        .flatten()
        .map(|e| (e.path.clone(), e.hash.clone()))
        .collect();

    let lock_map: BTreeMap<String, String> = lock
        .entries
        .values()
        .flatten()
        .map(|e| (e.path.clone(), e.hash.clone()))
        .collect();

    // Find added and modified
    for (path, hash) in &current_map {
        match lock_map.get(path) {
            Some(lock_hash) => {
                if lock_hash != hash {
                    modified.push(path.clone());
                }
            }
            None => {
                added.push(path.clone());
            }
        }
    }

    // Find removed
    for path in lock_map.keys() {
        if !current_map.contains_key(path) {
            removed.push(path.clone());
        }
    }

    added.sort();
    modified.sort();
    removed.sort();

    let matches = added.is_empty() && modified.is_empty() && removed.is_empty();

    Ok(VerifyResult {
        matches,
        added,
        modified,
        removed,
    })
}

/// Hash a file and return its SHA-256 hex digest.
fn hash_file(path: &Path) -> Result<Option<String>> {
    let metadata = fs::metadata(path)?;
    if metadata.is_dir() {
        return Ok(None);
    }

    // Skip large files
    if metadata.len() > 10 * 1024 * 1024 {
        return Ok(None);
    }

    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- path is within project directory tree
    let content = fs::read(path)
        .with_context(|| format!("Failed to read file for hashing: {}", path.display()))?;

    let mut hasher = Sha256::new();
    hasher.update(&content);
    let result = hasher.finalize();
    let hash: String = result.iter().map(|b| format!("{:02x}", b)).collect();
    Ok(Some(hash))
}

/// Recursively hash all files in a directory.
fn hash_directory_recursively(
    lock: &mut EnvLock,
    dir: &Path,
    project_root: &Path,
    category: LockCategory,
) -> Result<()> {
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- dir is within project directory tree
    let entries = fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            hash_directory_recursively(lock, &path, project_root, category)?;
        } else if path.is_file() {
            // Skip lock files themselves
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && (name == ENV_LOCK_FILE || name.ends_with(".lock"))
            {
                continue;
            }

            if let Some(hash) = hash_file(&path)? {
                let relative = path
                    .strip_prefix(project_root)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path.to_string_lossy().to_string());
                lock.add_entry(category, relative, hash);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_project(temp_dir: &TempDir) -> PathBuf {
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        // Create a config file
        let config_path = agents_dir.join("agentsync.toml");
        fs::write(&config_path, "source_dir = \".\"\n").unwrap();

        // Create a skill
        let skills_dir = agents_dir.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("test-skill.md"), "# Test Skill\n").unwrap();

        temp_dir.path().to_path_buf()
    }

    #[test]
    fn test_generate_lock() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = setup_test_project(&temp_dir);

        let config = Config::default();
        let lock = generate_lock(&project_root, &config).unwrap();

        assert_eq!(lock.schema_version, ENV_LOCK_SCHEMA_VERSION);
        assert!(lock.total > 0);
        assert!(lock.entries.contains_key(&LockCategory::Config));
    }

    #[test]
    fn test_lock_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = setup_test_project(&temp_dir);

        let config = Config::default();
        let lock = generate_lock(&project_root, &config).unwrap();
        lock.save(&project_root).unwrap();

        let loaded = EnvLock::load(&project_root).unwrap();
        assert_eq!(lock, loaded);
    }

    #[test]
    fn test_verify_lock_no_drift() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = setup_test_project(&temp_dir);

        let config = Config::default();
        let lock = generate_lock(&project_root, &config).unwrap();

        let result = verify_lock(&project_root, &config, &lock).unwrap();
        assert!(result.matches);
        assert!(result.added.is_empty());
        assert!(result.modified.is_empty());
        assert!(result.removed.is_empty());
    }

    #[test]
    fn test_verify_lock_detects_addition() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = setup_test_project(&temp_dir);

        let config = Config::default();
        let lock = generate_lock(&project_root, &config).unwrap();

        // Add a new file
        let skills_dir = project_root.join(".agents").join("skills");
        fs::write(skills_dir.join("new-skill.md"), "# New Skill\n").unwrap();

        let result = verify_lock(&project_root, &config, &lock).unwrap();
        assert!(!result.matches);
        assert!(!result.added.is_empty());
    }

    #[test]
    fn test_verify_lock_detects_modification() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = setup_test_project(&temp_dir);

        let config = Config::default();
        let lock = generate_lock(&project_root, &config).unwrap();

        // Modify a file
        let skills_dir = project_root.join(".agents").join("skills");
        fs::write(skills_dir.join("test-skill.md"), "# Modified Skill\n").unwrap();

        let result = verify_lock(&project_root, &config, &lock).unwrap();
        assert!(!result.matches);
        assert!(!result.modified.is_empty());
    }

    #[test]
    fn test_verify_lock_detects_removal() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = setup_test_project(&temp_dir);

        let config = Config::default();
        let lock = generate_lock(&project_root, &config).unwrap();

        // Remove a file
        let skills_dir = project_root.join(".agents").join("skills");
        fs::remove_file(skills_dir.join("test-skill.md")).unwrap();

        let result = verify_lock(&project_root, &config, &lock).unwrap();
        assert!(!result.matches);
        assert!(!result.removed.is_empty());
    }

    #[test]
    fn test_lock_deterministic() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = setup_test_project(&temp_dir);

        let config = Config::default();
        let lock1 = generate_lock(&project_root, &config).unwrap();
        let lock2 = generate_lock(&project_root, &config).unwrap();

        assert_eq!(
            serde_json::to_string(&lock1).unwrap(),
            serde_json::to_string(&lock2).unwrap()
        );
    }
}
