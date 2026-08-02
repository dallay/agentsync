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

pub async fn update_skill_async(
    skill_id: &str,
    target_root: &Path,
    update_source: &Path,
) -> Result<(), SkillUpdateError> {
    // _temp_guard must remain alive to prevent premature cleanup of temp directory
    let (local_dir, _temp_guard) = resolve_update_source(update_source).await?;

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

fn resolve_current_version(
    skill_id: &str,
    skill_dir: &Path,
    registry_path: &Path,
) -> Option<String> {
    debug!(registry_path = %registry_path.display(), exists = %registry_path.exists(), "update registry check");

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

/// Rolls back skill_dir by removing it and restoring from backup_dir if present.
fn rollback_skill_dir(skill_dir: &Path, backup_dir: &Path) {
    if skill_dir.exists() {
        let _ = std::fs::remove_dir_all(skill_dir);
    }
    if backup_dir.exists() {
        let _ = std::fs::rename(backup_dir, skill_dir);
    }
}

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
            rollback_skill_dir(skill_dir, backup_dir);
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
        rollback_skill_dir(skill_dir, backup_dir);
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

/// Recursively copies a directory (src) to dst.
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_rollback_skill_dir_restores_backup() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("my-skill");
        let backup_dir = tmp.path().join("my-skill.bak");

        // Create backup with content
        std::fs::create_dir_all(&backup_dir).unwrap();
        std::fs::write(backup_dir.join("SKILL.md"), "# Original").unwrap();

        // Create a "broken" skill_dir
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("broken.txt"), "bad").unwrap();

        rollback_skill_dir(&skill_dir, &backup_dir);

        // skill_dir should now contain original content
        assert!(skill_dir.join("SKILL.md").exists());
        assert!(!skill_dir.join("broken.txt").exists());
        assert!(!backup_dir.exists());
    }

    #[test]
    fn test_rollback_skill_dir_no_backup_just_removes() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("my-skill");
        let backup_dir = tmp.path().join("my-skill.bak");

        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("broken.txt"), "bad").unwrap();

        rollback_skill_dir(&skill_dir, &backup_dir);

        assert!(!skill_dir.exists());
        assert!(!backup_dir.exists());
    }

    #[test]
    fn test_rollback_skill_dir_nothing_exists() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("my-skill");
        let backup_dir = tmp.path().join("my-skill.bak");

        // Should not panic
        rollback_skill_dir(&skill_dir, &backup_dir);
        assert!(!skill_dir.exists());
    }

    #[test]
    fn test_create_backup_moves_skill_dir() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("my-skill");
        let backup_dir = tmp.path().join("my-skill.bak");

        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# Test").unwrap();

        create_backup(&skill_dir, &backup_dir).unwrap();

        assert!(!skill_dir.exists());
        assert!(backup_dir.join("SKILL.md").exists());
    }

    #[test]
    fn test_create_backup_replaces_existing_backup() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("my-skill");
        let backup_dir = tmp.path().join("my-skill.bak");

        // Old backup
        std::fs::create_dir_all(&backup_dir).unwrap();
        std::fs::write(backup_dir.join("old.md"), "old").unwrap();

        // Current skill
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("new.md"), "new").unwrap();

        create_backup(&skill_dir, &backup_dir).unwrap();

        assert!(!backup_dir.join("old.md").exists());
        assert!(backup_dir.join("new.md").exists());
    }

    #[test]
    fn test_create_backup_noop_when_skill_dir_missing() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("missing");
        let backup_dir = tmp.path().join("missing.bak");

        let result = create_backup(&skill_dir, &backup_dir);
        assert!(result.is_ok());
        assert!(!backup_dir.exists());
    }

    #[test]
    fn test_copy_dir_all_copies_recursively() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");

        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), "a").unwrap();
        std::fs::write(src.join("sub/b.txt"), "b").unwrap();

        copy_dir_all(&src, &dst).unwrap();

        assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "a");
        assert_eq!(std::fs::read_to_string(dst.join("sub/b.txt")).unwrap(), "b");
    }

    #[cfg(unix)]
    #[test]
    fn test_copy_dir_all_skips_symlinks() {
        use std::os::unix::fs as unix_fs;

        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");

        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("real.txt"), "real").unwrap();
        unix_fs::symlink("/etc/passwd", src.join("evil-link")).unwrap();

        copy_dir_all(&src, &dst).unwrap();

        assert!(dst.join("real.txt").exists());
        assert!(!dst.join("evil-link").exists());
    }
}
