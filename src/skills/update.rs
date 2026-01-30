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
    use crate::skills::install::fetch_and_unpack_to_tempdir;
    let use_remote = {
        let s = update_source.to_string_lossy().to_string();
        s.starts_with("http://")
            || s.starts_with("https://")
            || s.ends_with(".zip")
            || s.ends_with(".tar.gz")
    };
    let local_dir: std::path::PathBuf;
    let mut _temp_holder;
    if use_remote {
        // Download and unpack to temp (propagate SkillInstallError -> SkillUpdateError::Install)
        let td = fetch_and_unpack_to_tempdir(&update_source.to_string_lossy()).await?;
        local_dir = td.path().to_path_buf();
        _temp_holder = Some(td);
    } else {
        local_dir = update_source.to_path_buf();
        _temp_holder = None;
    }

    use std::fs;
    // use std::path::PathBuf; (unused)

    // Paths
    let skill_dir = target_root.join(skill_id);
    let backup_dir = target_root.join(format!("{}.bak", skill_id));
    let registry_path = target_root.join("registry.json");

    // Version resolution: only update if new version > current
    // 1. Extract current version (from registry if present, else SKILL.md in skill_dir), or treat as "0.0.0" if not installed
    debug!(registry_path = %registry_path.display(), exists = %registry_path.exists(), "update registry check");
    if registry_path.exists() {
        let reg_contents =
            std::fs::read_to_string(&registry_path).unwrap_or_else(|_| "<read error>".to_string());
        debug!(contents = %reg_contents, "registry contents after install");
    }
    let mut current_version: Option<String> = None;
    // Try registry first
    if registry_path.exists() {
        if let Ok(reg) = crate::skills::registry::read_registry(&registry_path) {
            if let Some(skills) = reg.skills {
                if let Some(entry) = skills.get(skill_id) {
                    current_version = entry.version.clone();
                }
            }
        }
    }
    // Fallback: If not in registry, try SKILL.md in existing skill_dir
    if current_version.is_none() && skill_dir.exists() {
        let manifest_path = skill_dir.join("SKILL.md");
        if manifest_path.exists() {
            if let Ok(existing_manifest) =
                crate::skills::manifest::parse_skill_manifest(&manifest_path)
            {
                // existing_manifest.version is Option<String>; propagate directly
                current_version = existing_manifest.version.clone();
            }
        }
    }
    // Parse update candidate version from local_dir/SKILL.md
    let update_manifest_path = local_dir.join("SKILL.md");
    let update_manifest = crate::skills::manifest::parse_skill_manifest(&update_manifest_path)?;
    // update_manifest.version is Option<String>; require it for update resolution
    let update_version_str = update_manifest
        .version
        .as_deref()
        .ok_or_else(|| SkillUpdateError::Validation("missing version in SKILL.md".into()))?;
    let new_version = semver::Version::parse(update_version_str)
        .map_err(|_| SkillUpdateError::Validation("invalid semver in SKILL.md".into()))?;
    let installed_version = match current_version {
        Some(ref verstr) => {
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

    // Step 1: Atomically move skill_dir to backup_dir (if exists).
    if skill_dir.exists() {
        // Clean up previous backup if somehow it exists.
        if backup_dir.exists() {
            fs::remove_dir_all(&backup_dir).map_err(|_| SkillUpdateError::Atomic)?;
        }
        fs::rename(&skill_dir, &backup_dir).map_err(|_| SkillUpdateError::Atomic)?;
    }

    // Step 2: Copy source dir to skill_dir.
    // (We use copy to support cross-device; atomic rename if same device. Use copy_dir_recursively)
    if skill_dir.exists() {
        fs::remove_dir_all(&skill_dir).map_err(|_| SkillUpdateError::Atomic)?;
    }
    copy_dir_all(&local_dir, &skill_dir).map_err(SkillUpdateError::Io)?;

    // Step 3: Validate the new skill manifest. On failure, remove the new dir, restore backup.
    let manifest_path = skill_dir.join("SKILL.md");
    let manifest = match crate::skills::manifest::parse_skill_manifest(&manifest_path) {
        Ok(manifest) => manifest,
        Err(e) => {
            // Cleanup: remove failed new dir
            let _ = fs::remove_dir_all(&skill_dir);
            // Restore backup (if any) back to place
            if backup_dir.exists() {
                let _ = fs::rename(&backup_dir, &skill_dir);
            }
            return Err(SkillUpdateError::Install(e));
        }
    };

    // Step 4: Registry update with rollback
    // Save previous registry entry for this skill if exists
    let mut old_registry_entry: Option<crate::skills::registry::SkillEntry> = None;
    let registry_path = target_root.join("registry.json");
    if registry_path.exists() {
        if let Ok(reg) = crate::skills::registry::read_registry(&registry_path) {
            if let Some(skills) = reg.skills {
                if let Some(entry) = skills.get(skill_id) {
                    old_registry_entry = Some(entry.clone());
                }
            }
        }
    }
    // Build a new skill entry for registry update
    let new_entry = crate::skills::registry::SkillEntry {
        name: Some(manifest.name.clone()),
        description: manifest.description.clone(),
        // registry expects Option<String> for version; propagate directly
        version: manifest.version.clone(),
        provider: None,
        source: None,
        installed_at: Some(chrono::Utc::now().to_rfc3339()),
        files: None, // Could add list of files here if needed
        manifest_hash: None,
    };
    // Try registry update, rollback both skill dir and registry on failure
    if let Err(e) =
        crate::skills::registry::update_registry_entry(&registry_path, skill_id, new_entry)
    {
        // Remove broken new dir
        let _ = fs::remove_dir_all(&skill_dir);
        // Restore backup if possible
        if backup_dir.exists() {
            let _ = fs::rename(&backup_dir, &skill_dir);
        }
        // Try to restore previous registry entry if there was one
        if let Some(old_entry) = old_registry_entry {
            let _ =
                crate::skills::registry::update_registry_entry(&registry_path, skill_id, old_entry);
        }
        return Err(SkillUpdateError::Registry(e));
    }

    // Step 5: All OK, clean up backup
    if backup_dir.exists() {
        let _ = fs::remove_dir_all(&backup_dir);
    }
    // temp_holder will cleanup tempdir (if present) when dropped
    Ok(())
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
