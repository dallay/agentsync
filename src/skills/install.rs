use flate2::read::GzDecoder;
// futures_util::StreamExt is used locally where needed
use anyhow::Error as AnyhowError;
use reqwest::{Client, Error as ReqwestError};
use std::path::{Path, PathBuf};
use tar::Archive;
use tempfile::TempDir;
use thiserror::Error;
use tracing::debug;
use walkdir::WalkDir;
use zip::read::ZipArchive;
use zip::result::ZipError;

#[derive(Debug, Error)]
pub enum SkillInstallError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Network error: {0}")]
    Network(#[from] ReqwestError),
    #[error("Zip archive error: {0}")]
    ZipArchive(#[from] ZipError),
    // Tar errors map to Io variant
    #[error("Registry error: {0}")]
    Registry(#[from] AnyhowError),
    #[error("Path traversal attempt in archive: {0}")]
    PathTraversal(String),
    #[error("Validation failed: {0}")]
    Validation(String),
    #[error("Other error: {0}")]
    Other(String),
}

// Module-level helper to recursively copy directories
fn copy_dir_recursively(
    src: &std::path::Path,
    dst: &std::path::Path,
) -> Result<(), SkillInstallError> {
    std::fs::create_dir_all(dst).map_err(SkillInstallError::Io)?;
    for entry in std::fs::read_dir(src).map_err(SkillInstallError::Io)? {
        let entry = entry.map_err(SkillInstallError::Io)?;
        let file_type = entry.file_type().map_err(SkillInstallError::Io)?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursively(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path).map_err(SkillInstallError::Io)?;
        }
    }
    Ok(())
}

/// Synchronously fetch, extract, and copy a skill source (dir, archive, or URL) into the target directory and validate manifest.
pub fn blocking_fetch_and_install_skill(
    skill_id: &str,
    source: &str,
    target_root: &std::path::Path,
) -> Result<(), SkillInstallError> {
    // Use existing Tokio runtime when available to avoid panics inside runtime
    let tempdir = match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(fetch_and_unpack_to_tempdir(source))?,
        Err(_) => {
            let rt = tokio::runtime::Runtime::new().map_err(SkillInstallError::Io)?;
            rt.block_on(fetch_and_unpack_to_tempdir(source))?
        }
    };

    let best_dir = find_best_skill_dir(tempdir.path(), skill_id);
    debug!(
        skill_id = %skill_id,
        temp_path = %tempdir.path().display(),
        best_dir = %best_dir.display(),
        "found best skill directory"
    );

    let skill_dir = target_root.join(skill_id);
    debug!(skill_id = %skill_id, target_root = %target_root.display(), source = %source, "install");
    if skill_dir.exists() {
        std::fs::remove_dir_all(&skill_dir).map_err(SkillInstallError::Io)?;
    }
    copy_dir_recursively(&best_dir, &skill_dir)?;
    // Validate manifest
    let manifest_path = skill_dir.join("SKILL.md");
    let parsed = crate::skills::manifest::parse_skill_manifest(&manifest_path)?;
    // Write registry entry
    let entry = crate::skills::registry::SkillEntry {
        name: Some(parsed.name.clone()),
        description: parsed.description.clone(),
        // registry expects Option<String> for version; propagate directly
        version: parsed.version.clone(),
        provider: None,
        source: Some(source.to_string()),
        installed_at: Some(chrono::Utc::now().to_rfc3339()),
        files: None,         // Could enumerate actual files
        manifest_hash: None, // Could hash contents
    };
    let registry_path = target_root.join("registry.json");
    debug!(registry_path = %registry_path.display(), "writing registry.json");
    // update_registry_entry returns anyhow::Error on failure; wrap via SkillInstallError::Registry
    crate::skills::registry::update_registry_entry(&registry_path, skill_id, entry)
        .map_err(SkillInstallError::Registry)?;
    Ok(())
}

/// Find the best directory within the unpacked archive to install as a skill.
///
/// It takes `temp_path` and `skill_id`, checks for `SKILL.md` at the root,
/// searches subdirectories for `SKILL.md`, prioritizes a manifest whose parent
/// directory name matches `skill_id`, returns the sole manifest directory if
/// only one is found, and falls back to returning `temp_path`.
fn find_best_skill_dir(temp_path: &Path, skill_id: &str) -> PathBuf {
    // 1. Check if SKILL.md is at the root
    if temp_path.join("SKILL.md").exists() {
        return temp_path.to_path_buf();
    }

    // 2. Search for SKILL.md in subdirectories
    let mut all_manifests = Vec::new();
    for entry in WalkDir::new(temp_path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_name() == "SKILL.md" {
            let parent = entry.path().parent().unwrap().to_path_buf();
            // If the parent directory name matches the skill_id, we prioritize it
            if parent.file_name().and_then(|n| n.to_str()) == Some(skill_id) {
                return parent;
            }
            all_manifests.push(parent);
        }
    }

    // If there's only one manifest found, use its directory
    if all_manifests.len() == 1 {
        return all_manifests.remove(0);
    }

    // Fallback to original path
    temp_path.to_path_buf()
}

pub async fn fetch_and_unpack_to_tempdir(url: &str) -> Result<TempDir, SkillInstallError> {
    use std::io::Cursor;
    use std::path::Path;
    let tmp = tempfile::TempDir::new().map_err(SkillInstallError::Io)?;
    let is_file = url.starts_with("file://");
    // is_local: either absolute unix path or Windows drive letter (C:)
    let is_local = url.starts_with('/') || url.chars().nth(1) == Some(':');
    let client = if !is_file && !is_local {
        Some(Client::new())
    } else {
        None
    };
    let (data, ext) = if is_file || is_local {
        // Safely strip file:// prefix
        let path_str = if is_file {
            url.strip_prefix("file://").unwrap_or("")
        } else {
            url
        };
        if path_str.is_empty() {
            return Err(SkillInstallError::Validation("empty file:// path".into()));
        }
        let path = Path::new(path_str);
        let ext = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if path.is_dir() {
            // Local directory: copy recursively to tempdir and return
            copy_dir_recursively(path, tmp.path())?;
            return Ok(tmp);
        }
        let data = std::fs::read(path).map_err(SkillInstallError::Io)?;
        (data, ext)
    } else {
        let ext = {
            let parts: Vec<_> = url.split('.').collect();
            if let Some(last) = parts.last() {
                last.to_ascii_lowercase()
            } else {
                "".to_string()
            }
        };
        let client = client.ok_or_else(|| SkillInstallError::Other("no client".into()))?;
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(SkillInstallError::Network)?
            .error_for_status()
            .map_err(SkillInstallError::Network)?;
        // Stream response to a temp file instead of buffering in memory
        let stdfile = std::fs::File::create(tmp.path().join("download.tmp"))
            .map_err(SkillInstallError::Io)?;
        let mut tmpfile = tokio::fs::File::from_std(stdfile);
        let mut stream = resp.bytes_stream();
        use futures_util::StreamExt as _;
        use tokio::io::AsyncWriteExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(SkillInstallError::Network)?;
            tmpfile
                .write_all(&chunk)
                .await
                .map_err(SkillInstallError::Io)?;
        }
        tmpfile.flush().await.map_err(SkillInstallError::Io)?;
        // Re-open and read into memory for legacy unpacking logic where needed
        let data = std::fs::read(tmp.path().join("download.tmp")).map_err(SkillInstallError::Io)?;
        (data, ext)
    };
    // Unpack archive type into the tempdir
    // Determine source name for archive-type heuristics (handles local file paths too)
    let source_name = url.to_string();
    let is_tar_gz = source_name.ends_with(".tar.gz") || source_name.ends_with(".tgz");

    if ext == "zip" {
        let reader = Cursor::new(&data);
        let mut zip = ZipArchive::new(reader).map_err(SkillInstallError::ZipArchive)?;
        for i in 0..zip.len() {
            let mut file = zip.by_index(i).map_err(SkillInstallError::ZipArchive)?;
            let filename = file.name();
            // Reject absolute paths and path traversal attempts
            if filename.starts_with('/') || filename.contains("..") {
                return Err(SkillInstallError::PathTraversal(filename.to_string()));
            }
            let outpath = tmp.path().join(filename);
            if file.is_dir() {
                std::fs::create_dir_all(&outpath).map_err(SkillInstallError::Io)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    std::fs::create_dir_all(parent).map_err(SkillInstallError::Io)?;
                }
                let mut out = std::fs::File::create(&outpath).map_err(SkillInstallError::Io)?;
                std::io::copy(&mut file, &mut out).map_err(SkillInstallError::Io)?;
            }
        }
    } else if is_tar_gz {
        let reader = Cursor::new(&data);
        let gz = GzDecoder::new(reader);
        let mut archive = Archive::new(gz);
        for entry in archive.entries().map_err(SkillInstallError::Io)? {
            let mut entry = entry.map_err(SkillInstallError::Io)?;
            let path = entry.path().map_err(SkillInstallError::Io)?;
            if path.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            }) {
                return Err(SkillInstallError::PathTraversal(
                    path.to_string_lossy().into_owned(),
                ));
            }
            let outpath = tmp.path().join(&path);
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).map_err(SkillInstallError::Io)?;
            }
            entry.unpack(&outpath).map_err(SkillInstallError::Io)?;
        }
    } else {
        return Err(SkillInstallError::Other("unknown archive format".into()));
    }
    Ok(tmp)
}
