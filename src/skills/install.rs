use flate2::read::GzDecoder;
// futures_util::StreamExt is used locally where needed
use anyhow::Error as AnyhowError;
use reqwest::{Client, Error as ReqwestError};
use tar::Archive;
use tempfile::TempDir;
use thiserror::Error;
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
pub fn install_from_dir(
    skill_id: &str,
    src_dir: &std::path::Path,
    target_root: &std::path::Path,
) -> Result<(), SkillInstallError> {
    let skill_dir = target_root.join(skill_id);

    // Create a temporary directory for rollback if anything fails
    let temp_skill_dir = tempfile::TempDir::new().map_err(SkillInstallError::Io)?;
    let staging_dir = temp_skill_dir.path();

    copy_dir_recursively(src_dir, staging_dir)?;

    // Validate manifest
    let manifest_path = staging_dir.join("SKILL.md");
    let parsed = crate::skills::manifest::parse_skill_manifest(&manifest_path)?;

    // If validation passes, move to final location
    if skill_dir.exists() {
        std::fs::remove_dir_all(&skill_dir).map_err(SkillInstallError::Io)?;
    }
    std::fs::create_dir_all(&skill_dir).map_err(SkillInstallError::Io)?;
    copy_dir_recursively(staging_dir, &skill_dir)?;
    let entry = crate::skills::registry::SkillEntry {
        name: Some(parsed.name.clone()),
        description: parsed.description.clone(),
        version: parsed.version.clone(),
        provider: None,
        source: Some(src_dir.to_string_lossy().into_owned()),
        installed_at: Some(chrono::Utc::now().to_rfc3339()),
        files: None,
        manifest_hash: None,
    };
    let registry_path = target_root.join("registry.json");
    crate::skills::registry::update_registry_entry(&registry_path, skill_id, entry)
        .map_err(SkillInstallError::Registry)?;
    Ok(())
}

pub fn install_from_zip(
    skill_id: &str,
    reader: impl std::io::Read + std::io::Seek,
    target_root: &std::path::Path,
) -> Result<(), SkillInstallError> {
    let tmp = tempfile::TempDir::new().map_err(SkillInstallError::Io)?;
    let mut zip = ZipArchive::new(reader).map_err(SkillInstallError::ZipArchive)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(SkillInstallError::ZipArchive)?;
        let filename = file.name();
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

    install_from_dir(skill_id, tmp.path(), target_root)
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

    install_from_dir(skill_id, tempdir.path(), target_root)
}

/// Fetch a skill archive (HTTP, file://, or local file path) and unpack to a temp directory, returning the directory path.
pub async fn fetch_and_unpack_to_tempdir(url: &str) -> Result<TempDir, SkillInstallError> {
    use std::io::Cursor;
    use std::path::Path;

    // Support subpaths via fragments, e.g. https://example.com/archive.zip#subpath
    let (url_base, subpath) = match url.find('#') {
        Some(pos) => (&url[..pos], Some(&url[pos + 1..])),
        None => (url, None),
    };

    let tmp = tempfile::TempDir::new().map_err(SkillInstallError::Io)?;
    let is_file = url_base.starts_with("file://");
    // is_local: either absolute unix path or Windows drive letter (C:)
    let is_local = url_base.starts_with('/') || url_base.chars().nth(1) == Some(':');
    let client = if !is_file && !is_local {
        Some(Client::new())
    } else {
        None
    };
    let (data, ext) = if is_file || is_local {
        // Safely strip file:// prefix
        let path_str = if is_file {
            url_base.strip_prefix("file://").unwrap_or("")
        } else {
            url_base
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
            let parts: Vec<_> = url_base.split('.').collect();
            if let Some(last) = parts.last() {
                last.to_ascii_lowercase()
            } else {
                "".to_string()
            }
        };
        let client = client.ok_or_else(|| SkillInstallError::Other("no client".into()))?;
        let resp = client
            .get(url_base)
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
    let source_name = url_base.to_string();
    let is_tar_gz = source_name.ends_with(".tar.gz") || source_name.ends_with(".tgz");

    if ext == "zip" {
        let reader = Cursor::new(&data);
        let mut zip = ZipArchive::new(reader).map_err(SkillInstallError::ZipArchive)?;

        // Find if there is a common root directory (like GitHub zips do)
        let common_root = if !zip.is_empty() {
            let first_name = zip
                .by_index(0)
                .map_err(SkillInstallError::ZipArchive)?
                .name()
                .to_string();
            let root = first_name.split('/').next().unwrap_or("");
            if !root.is_empty() && zip.file_names().all(|n| n.starts_with(root)) {
                Some(root.to_string())
            } else {
                None
            }
        } else {
            None
        };

        for i in 0..zip.len() {
            let mut file = zip.by_index(i).map_err(SkillInstallError::ZipArchive)?;
            let full_name = file.name();

            // Reject absolute paths and path traversal attempts
            if full_name.starts_with('/') || full_name.contains("..") {
                return Err(SkillInstallError::PathTraversal(full_name.to_string()));
            }

            // Strip common root if present
            let rel_path = if let Some(ref root) = common_root {
                if full_name.starts_with(root) {
                    full_name[root.len()..].trim_start_matches('/')
                } else {
                    full_name
                }
            } else {
                full_name
            };

            // If a subpath is requested, filter and strip it
            let final_rel_path = if let Some(sub) = subpath {
                if let Some(stripped) = rel_path.strip_prefix(sub) {
                    stripped.trim_start_matches('/')
                } else {
                    continue; // Skip files not in subpath
                }
            } else {
                rel_path
            };

            if final_rel_path.is_empty() {
                continue;
            }

            let outpath = tmp.path().join(final_rel_path);
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

        let entries: Vec<_> = archive.entries().map_err(SkillInstallError::Io)?.collect();

        // Find if there is a common root directory
        let common_root = if !entries.is_empty() {
            let first_path = entries[0]
                .as_ref()
                .map_err(|e| SkillInstallError::Other(e.to_string()))?
                .path()
                .map_err(SkillInstallError::Io)?;
            let root = first_path.components().next().and_then(|c| match c {
                std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                _ => None,
            });

            if let Some(ref r) = root {
                let all_start_with = entries.iter().all(|e| {
                    if let Ok(entry) = e {
                        if let Ok(path) = entry.path() {
                            path.starts_with(r)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                });
                if all_start_with {
                    Some(r.clone())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        for entry in entries {
            let mut entry = entry.map_err(SkillInstallError::Io)?;
            let full_path = entry.path().map_err(SkillInstallError::Io)?;

            if full_path.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            }) {
                return Err(SkillInstallError::PathTraversal(
                    full_path.to_string_lossy().into_owned(),
                ));
            }

            // Strip common root if present
            let rel_path = if let Some(ref root) = common_root {
                if full_path.starts_with(root) {
                    full_path
                        .strip_prefix(root)
                        .unwrap_or(&full_path)
                        .to_path_buf()
                } else {
                    full_path.to_path_buf()
                }
            } else {
                full_path.to_path_buf()
            };

            // If a subpath is requested, filter and strip it
            let final_rel_path = if let Some(sub) = subpath {
                let sub_path = std::path::Path::new(sub);
                if rel_path.starts_with(sub_path) {
                    rel_path
                        .strip_prefix(sub_path)
                        .unwrap_or(&rel_path)
                        .to_path_buf()
                } else {
                    continue; // Skip files not in subpath
                }
            } else {
                rel_path
            };

            if final_rel_path.as_os_str().is_empty() {
                continue;
            }

            let outpath = tmp.path().join(final_rel_path);
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
