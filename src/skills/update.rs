//! Skill update logic for AgentSync: safely apply a new version, validate, rollback on failure.

use std::path::Path;
use thiserror::Error;
use tracing::debug;

use crate::skills::install::SkillInstallError;

#[derive(Debug, Error)]
pub enum SkillUpdateError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Install/fetch error: {0}")]
    Install(#[from] SkillInstallError),
    #[error("Registry error: {0}")]
    Registry(#[from] anyhow::Error),
    #[error("Atomic update failed")]
    Atomic,
    #[error("Validation failed: {0}")]
    Validation(String),
}

/// Updates an installed skill from a local or remote source.
///
/// The candidate version must be newer than the currently installed version.
/// The existing skill and registry entry are restored if installation fails.
///
/// # Arguments
///
/// * `skill_id` - Identifier of the skill to update.
/// * `target_root` - Directory containing the installed skill and registry.
/// * `update_source` - Path or URL identifying the candidate skill.
///
/// # Errors
///
/// Returns an error if the source cannot be resolved, the candidate version is
/// invalid or not newer, or backup, installation, validation, or registry
/// operations fail.
///
/// # Examples
///
/// ```
/// # use std::path::Path;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// update_skill_async(
///     "example",
///     Path::new("./skills"),
///     Path::new("./example-update"),
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn update_skill_async(
    skill_id: &str,
    target_root: &Path,
    update_source: &Path,
) -> Result<(), SkillUpdateError> {
    let local_dir = resolve_update_source(update_source).await?;
    let _temp_holder = local_dir.1;
    let local_dir = local_dir.0;

    let skill_dir = target_root.join(skill_id);
    let backup_dir = target_root.join(format!("{}.bak", skill_id));
    let registry_path = target_root.join("registry.json");

    let current_version = resolve_current_version(skill_id, &skill_dir, &registry_path);
    validate_version_upgrade(&local_dir, &current_version)?;

    create_backup(&skill_dir, &backup_dir)?;

    install_updated_skill(
        skill_id,
        &local_dir,
        &skill_dir,
        &backup_dir,
        &registry_path,
    )
}

/// Resolves a local path or fetches a remote archive into a temporary directory.
///
/// Remote URLs and archive paths are unpacked into a temporary directory whose
/// lifetime is retained by the returned `TempDir`.
///
/// # Returns
///
/// A tuple containing the resolved skill path and the temporary directory that
/// owns it, if the source was fetched remotely.
///
/// # Examples
///
/// ```
/// # async fn example() -> Result<(), SkillUpdateError> {
/// let (path, _temporary_dir) =
///     resolve_update_source(std::path::Path::new("./skill")).await?;
/// assert_eq!(path, std::path::PathBuf::from("./skill"));
/// # Ok(())
/// # }
/// ```
async fn resolve_update_source(
    update_source: &Path,
) -> Result<(std::path::PathBuf, Option<tempfile::TempDir>), SkillUpdateError> {
    use crate::skills::install::fetch_and_unpack_to_tempdir;
    let s = update_source.to_string_lossy().to_string();
    let use_remote = s.starts_with("http://")
        || s.starts_with("https://")
        || s.ends_with(".zip")
        || s.ends_with(".tar.gz");

    if use_remote {
        let td = fetch_and_unpack_to_tempdir(&s).await?;
        let path = td.path().to_path_buf();
        Ok((path, Some(td)))
    } else {
        Ok((update_source.to_path_buf(), None))
    }
}

/// Resolves the installed version of a skill from the registry or its `SKILL.md` manifest.
///
/// # Examples
///
/// ```
/// use std::path::Path;
///
/// let version = resolve_current_version(
///     "example-skill",
///     Path::new("missing-skill"),
///     Path::new("missing-registry.json"),
/// );
///
/// assert_eq!(version, None);
/// ```
fn resolve_current_version(
    skill_id: &str,
    skill_dir: &Path,
    registry_path: &Path,
) -> Option<String> {
    debug!(registry_path = %registry_path.display(), exists = %registry_path.exists(), "update registry check");
    if registry_path.exists() {
        let reg_contents =
            std::fs::read_to_string(registry_path).unwrap_or_else(|_| "<read error>".to_string());
        debug!(contents = %reg_contents, "registry contents after install");
    }

    // Try registry first
    if registry_path.exists()
        && let Some(version) = crate::skills::registry::read_registry(registry_path)
            .ok()
            .and_then(|reg| reg.skills)
            .and_then(|skills| skills.get(skill_id).cloned())
            .and_then(|entry| entry.version)
    {
        return Some(version);
    }

    // Fallback: try SKILL.md in existing skill_dir
    if skill_dir.exists() {
        let manifest_path = skill_dir.join("SKILL.md");
        if let Some(version) = manifest_path
            .exists()
            .then(|| crate::skills::manifest::parse_skill_manifest(&manifest_path).ok())
            .flatten()
            .and_then(|m| m.version.clone())
        {
            return Some(version);
        }
    }

    None
}

/// Validates that a skill manifest contains a semantic version newer than the installed version.
///
/// A missing or invalid installed version is treated as `0.0.0`.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
///
/// let installed_version = Some(String::from("1.2.0"));
/// validate_version_upgrade(Path::new("/path/to/skill"), &installed_version)?;
/// # Ok::<(), SkillUpdateError>(())
/// ```
fn validate_version_upgrade(
local_dir: &Path,
current_version: &Option<String>,
) -> Result<(), SkillUpdateError> {
fn validate_version_upgrade(
    local_dir: &Path,
    current_version: &Option<String>,
) -> Result<(), SkillUpdateError> {
    let update_manifest_path = local_dir.join("SKILL.md");
    let update_manifest = crate::skills::manifest::parse_skill_manifest(&update_manifest_path)?;
    let update_version_str = update_manifest
        .version
        .as_deref()
        .ok_or_else(|| SkillUpdateError::Validation("missing version in SKILL.md".into()))?;
    let new_version = semver::Version::parse(update_version_str)
        .map_err(|_| SkillUpdateError::Validation("invalid semver in SKILL.md".into()))?;
    let installed_version = match current_version {
        Some(verstr) => {
            semver::Version::parse(verstr).unwrap_or_else(|_| semver::Version::new(0, 0, 0))
        }
        None => semver::Version::new(0, 0, 0),
    };
    debug!(installed = %installed_version, candidate = %new_version, "Skill update version check");
    if new_version <= installed_version {
        debug!(new = %new_version, installed = %installed_version, "rejecting update: candidate <= installed");
        return Err(SkillUpdateError::Validation(format!(
            "Update rejected: version {} is not greater than installed {}",
            new_version, installed_version
        )));
    }
    Ok(())
}

/// Moves an existing skill directory to a backup location, replacing any existing backup.
///
/// If the skill directory does not exist, no action is taken.
///
/// # Errors
///
/// Returns [`SkillUpdateError::Atomic`] if removing the existing backup or moving the skill fails.
///
/// # Examples
///
/// ```
/// use std::fs;
///
/// let root = std::env::temp_dir().join("skill-update-example");
/// let skill_dir = root.join("skill");
/// let backup_dir = root.join("backup");
/// fs::create_dir_all(&skill_dir).unwrap();
///
/// create_backup(&skill_dir, &backup_dir).unwrap();
///
/// assert!(!skill_dir.exists());
/// assert!(backup_dir.exists());
/// fs::remove_dir_all(root).unwrap();
/// ```
fn create_backup(skill_dir: &Path, backup_dir: &Path) -> Result<(), SkillUpdateError> {
    use std::fs;
    if skill_dir.exists() {
        if backup_dir.exists() {
            fs::remove_dir_all(backup_dir).map_err(|_| SkillUpdateError::Atomic)?;
        }
        fs::rename(skill_dir, backup_dir).map_err(|_| SkillUpdateError::Atomic)?;
    }
    Ok(())
}

/// Installs an updated skill and records its manifest in the registry.
///
/// Restores the previous skill and registry entry when manifest validation or registry updates fail.
///
/// # Errors
///
/// Returns [`SkillUpdateError::Io`] if copying the candidate skill fails, [`SkillUpdateError::Install`] if its manifest is invalid, [`SkillUpdateError::Registry`] if the registry cannot be updated, or [`SkillUpdateError::Atomic`] if replacing the existing skill fails.
///
/// # Examples
///
/// ```no_run
/// # use std::path::Path;
/// # let candidate = Path::new("/tmp/candidate-skill");
/// # let skill = Path::new("/tmp/skills/example");
/// # let backup = Path::new("/tmp/skills/example.backup");
/// # let registry = Path::new("/tmp/skills/registry.json");
/// install_updated_skill("example", candidate, skill, backup, registry)?;
/// # Ok::<(), SkillUpdateError>(())
/// ```
fn install_updated_skill(
    skill_id: &str,
    local_dir: &Path,
    skill_dir: &Path,
    backup_dir: &Path,
    registry_path: &Path,
) -> Result<(), SkillUpdateError> {
    use std::fs;

    if skill_dir.exists() {
        fs::remove_dir_all(skill_dir).map_err(|_| SkillUpdateError::Atomic)?;
    }
    copy_dir_all(local_dir, skill_dir).map_err(SkillUpdateError::Io)?;

    // Validate the new skill manifest
    let manifest_path = skill_dir.join("SKILL.md");
    let manifest = match crate::skills::manifest::parse_skill_manifest(&manifest_path) {
        Ok(manifest) => manifest,
        Err(e) => {
            let _ = fs::remove_dir_all(skill_dir);
            if backup_dir.exists() {
                let _ = fs::rename(backup_dir, skill_dir);
            }
            return Err(SkillUpdateError::Install(e));
        }
    };

    // Save previous registry entry for rollback
    let old_registry_entry: Option<crate::skills::registry::SkillEntry> =
        read_old_registry_entry(skill_id, registry_path);

    let new_entry = crate::skills::registry::SkillEntry {
        name: Some(manifest.name.clone()),
        description: manifest.description.clone(),
        version: manifest.version.clone(),
        provider: None,
        source: None,
        installed_at: Some(chrono::Utc::now().to_rfc3339()),
        files: None,
        manifest_hash: None,
    };

    if let Err(e) =
        crate::skills::registry::update_registry_entry(registry_path, skill_id, new_entry)
    {
        let _ = fs::remove_dir_all(skill_dir);
        if backup_dir.exists() {
            let _ = fs::rename(backup_dir, skill_dir);
        }
        if let Some(old_entry) = old_registry_entry {
            let _ =
                crate::skills::registry::update_registry_entry(registry_path, skill_id, old_entry);
        }
        return Err(SkillUpdateError::Registry(e));
    }

    if backup_dir.exists() {
        let _ = fs::remove_dir_all(backup_dir);
    }
    Ok(())
}

/// Retrieves a skill's registry entry from the registry file.
///
/// # Examples
///
/// ```
/// use std::path::Path;
///
/// let entry = read_old_registry_entry("example-skill", Path::new("missing-registry.json"));
/// assert!(entry.is_none());
/// ```
///
/// # Returns
///
/// The registry entry for `skill_id`, or `None` if the registry or skill entry is unavailable.
fn read_old_registry_entry(
    skill_id: &str,
    registry_path: &Path,
) -> Option<crate::skills::registry::SkillEntry> {
    if !registry_path.exists() {
        return None;
    }
    let reg = crate::skills::registry::read_registry(registry_path).ok()?;
    let skills = reg.skills?;
    skills.get(skill_id).cloned()
}

/// Recursively copies a directory and its regular contents to a destination, skipping symbolic links.
///
/// # Errors
///
/// Returns an I/O error if the source cannot be read or the destination cannot be created or written.
///
/// # Examples
///
/// ```
/// # use std::fs;
/// # use std::path::PathBuf;
/// # let root = std::env::temp_dir().join(format!("copy-dir-all-{}", std::process::id()));
/// # let src = root.join("src");
/// # let dst = root.join("dst");
/// # fs::create_dir_all(&src).unwrap();
/// # fs::write(src.join("SKILL.md"), "content").unwrap();
/// copy_dir_all(&src, &dst).unwrap();
/// assert_eq!(fs::read_to_string(dst.join("SKILL.md")).unwrap(), "content");
/// # fs::remove_dir_all(root).unwrap();
/// ```
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    use std::fs;
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;

        // SECURITY: Skip symbolic links to prevent following them outside the update source.
        // This prevents information disclosure if a malicious update source contains
        // symlinks to sensitive host files (e.g., ~/.ssh/id_rsa).
        if ty.is_symlink() {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
