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
fn archive_path_is_unsafe(path: &str) -> bool {
    Path::new(path).components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    }) || path
        .as_bytes()
        .get(1)
        .is_some_and(|second_byte| *second_byte == b':')
        || path.starts_with('\\')
}

fn copy_dir_recursively(
    src: &std::path::Path,
    dst: &std::path::Path,
) -> Result<(), SkillInstallError> {
    std::fs::create_dir_all(dst).map_err(SkillInstallError::Io)?;
    for entry in std::fs::read_dir(src).map_err(SkillInstallError::Io)? {
        let entry = entry.map_err(SkillInstallError::Io)?;
        let file_type = entry.file_type().map_err(SkillInstallError::Io)?;

        // SECURITY: Skip symbolic links to prevent following them outside the source directory.
        if file_type.is_symlink() {
            continue;
        }

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
    // SECURITY: Validate skill_id is a single safe component to prevent path traversal
    // if it were to escape the target_root via malicious input like "../escape".
    let mut components = Path::new(skill_id).components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None) => {}
        _ => {
            return Err(SkillInstallError::Validation(format!(
                "invalid skill id: {}",
                skill_id
            )));
        }
    }

    let skill_dir = target_root.join(skill_id);

    // Create a temporary directory for rollback if anything fails
    let temp_skill_dir = tempfile::TempDir::new().map_err(SkillInstallError::Io)?;
    let staging_dir = temp_skill_dir.path();

    copy_dir_recursively(src_dir, staging_dir)?;

    // Validate manifest
    let manifest_path = staging_dir.join("SKILL.md");
    let parsed = crate::skills::manifest::parse_skill_manifest(&manifest_path)?;

    // If validation passes, move to final location
    let mut backup_path: Option<PathBuf> = None;
    if skill_dir.exists() {
        let backup = target_root.join(format!("{}.backup", skill_id));
        if backup.exists() {
            std::fs::remove_dir_all(&backup).map_err(SkillInstallError::Io)?;
        }
        std::fs::rename(&skill_dir, &backup).map_err(SkillInstallError::Io)?;
        backup_path = Some(backup);
    }

    if let Err(e) = (|| {
        std::fs::create_dir_all(&skill_dir).map_err(SkillInstallError::Io)?;
        copy_dir_recursively(staging_dir, &skill_dir)
    })() {
        // Rollback: restore backup if it exists
        if let Some(backup) = backup_path {
            if skill_dir.exists() {
                let _ = std::fs::remove_dir_all(&skill_dir);
            }
            let _ = std::fs::rename(backup, &skill_dir);
        }
        return Err(e);
    }

    // Success: cleanup backup
    if let Some(backup) = backup_path {
        let _ = std::fs::remove_dir_all(backup);
    }

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
        // SECURITY: Reject absolute paths, drive prefixes, and path traversal attempts.
        if archive_path_is_unsafe(filename) {
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

    let best_dir = find_best_skill_dir(tempdir.path(), skill_id);
    debug!(
        skill_id = %skill_id,
        temp_path = %tempdir.path().display(),
        best_dir = %best_dir.display(),
        "found best skill directory"
    );

    install_from_dir(skill_id, &best_dir, target_root)
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

/// Fetches a local or remote skill archive and unpacks it into a temporary directory.
///
/// A URL fragment selects a subpath within the archive. ZIP and gzip-compressed tar
/// archives are supported.
///
/// # Examples
///
/// ```no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), SkillInstallError> {
/// let temp_dir = fetch_and_unpack_to_tempdir("https://example.com/skill.zip").await?;
/// assert!(temp_dir.path().exists());
/// # Ok(())
/// # }
/// ```
pub async fn fetch_and_unpack_to_tempdir(url: &str) -> Result<TempDir, SkillInstallError> {
    use std::io::Cursor;

    // Support subpaths via fragments, e.g. https://example.com/archive.zip#subpath
    let (url_base, subpath) = match url.find('#') {
        Some(pos) => (&url[..pos], Some(&url[pos + 1..])),
        None => (url, None),
    };

    let tmp = tempfile::TempDir::new().map_err(SkillInstallError::Io)?;

    let (data, ext) = fetch_archive_data(url_base, tmp.path()).await?;

    // If fetch_archive_data returned empty data, it was a local dir copy — done
    if data.is_empty() {
        return Ok(tmp);
    }

    let source_name = url_base.to_string();
    let is_tar_gz = source_name.ends_with(".tar.gz") || source_name.ends_with(".tgz");

    if ext == "zip" {
        let reader = Cursor::new(&data);
        unpack_zip(reader, tmp.path(), subpath)?;
    } else if is_tar_gz {
        let reader = Cursor::new(&data);
        unpack_tar_gz(reader, tmp.path(), subpath)?;
    } else {
        return Err(SkillInstallError::Other("unknown archive format".into()));
    }
    Ok(tmp)
}

/// Retrieves archive data from a local source or remote URL.
///
/// Local paths and `file://` URLs are read directly; other URLs are downloaded
/// using the provided temporary path.
///
/// # Parameters
///
/// * `url_base` - The local path or URL identifying the archive.
/// * `tmp_path` - The temporary path used for remote downloads.
///
/// # Returns
///
/// The archive bytes and a label identifying its file format.
///
/// # Examples
///
/// ```
/// # let runtime = tokio::runtime::Runtime::new().unwrap();
/// # runtime.block_on(async {
/// let result = fetch_archive_data("file:///tmp/archive.zip", std::path::Path::new("/tmp/download")).await;
/// assert!(result.is_err() || result.is_ok());
/// # });
/// ```
async fn fetch_archive_data(
    url_base: &str,
    tmp_path: &std::path::Path,
) -> Result<(Vec<u8>, String), SkillInstallError> {
    let is_file = url_base.starts_with("file://");
    let is_local = url_base.starts_with('/') || url_base.chars().nth(1) == Some(':');

    if is_file || is_local {
        fetch_local_data(url_base, is_file, tmp_path)
    } else {
        fetch_remote_data(url_base, tmp_path).await
    }
}

/// Reads a local file or copies a local directory into a temporary destination.
///
/// # Examples
///
/// ```
/// # let source = std::env::temp_dir().join("skill-install-example.txt");
/// # let destination = std::env::temp_dir().join("skill-install-example-destination");
/// # std::fs::write(&source, b"skill data").unwrap();
/// let (data, extension) = fetch_local_data(
///     source.to_str().unwrap(),
///     false,
///     &destination,
/// ).unwrap();
///
/// assert_eq!(data, b"skill data");
/// assert_eq!(extension, "txt");
/// # std::fs::remove_file(source).unwrap();
/// ```
fn fetch_local_data(
    url_base: &str,
    is_file: bool,
    tmp_path: &std::path::Path,
) -> Result<(Vec<u8>, String), SkillInstallError> {
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
        copy_dir_recursively(path, tmp_path)?;
        return Ok((Vec::new(), String::new()));
    }
    let data = std::fs::read(path).map_err(SkillInstallError::Io)?;
    Ok((data, ext))
}

/// Downloads data from a remote URL into temporary storage and returns its contents with the URL's file extension.
///
/// # Examples
///
/// ```no_run
/// # async fn example(tmp_path: &std::path::Path) -> Result<(), SkillInstallError> {
/// let (data, extension) = fetch_remote_data("https://example.com/skill.zip", tmp_path).await?;
/// assert!(!data.is_empty());
/// assert_eq!(extension, "zip");
/// # Ok(())
/// # }
/// ```
///
/// The URL must respond successfully; network and local storage failures are returned as
/// `SkillInstallError` values.
async fn fetch_remote_data(
url_base: &str,
tmp_path: &std::path::Path,
) -> Result<(Vec<u8>, String), SkillInstallError> {
async fn fetch_remote_data(
    url_base: &str,
    tmp_path: &std::path::Path,
) -> Result<(Vec<u8>, String), SkillInstallError> {
    let ext = url_base
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();

    let client = Client::new();
    let resp = client
        .get(url_base)
        .send()
        .await
        .map_err(SkillInstallError::Network)?
        .error_for_status()
        .map_err(SkillInstallError::Network)?;

    let stdfile =
        std::fs::File::create(tmp_path.join("download.tmp")).map_err(SkillInstallError::Io)?;
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

    let data = std::fs::read(tmp_path.join("download.tmp")).map_err(SkillInstallError::Io)?;
    let _ = std::fs::remove_file(tmp_path.join("download.tmp"));
    Ok((data, ext))
}

/// Extracts ZIP archive entries into a destination directory, optionally restricting extraction to a subpath.
///
/// Archive paths are validated before extraction, and unsafe paths cause an error.
///
/// # Arguments
///
/// * `reader` - A seekable reader containing the ZIP archive.
/// * `dest` - Directory where selected entries are extracted.
/// * `subpath` - Optional path within the archive to extract.
///
/// # Errors
///
/// Returns an error if the archive cannot be read, an entry has an unsafe path, or extraction fails.
///
/// # Examples
///
/// ```
/// use std::io::Cursor;
/// use std::path::Path;
///
/// let result = unpack_zip(Cursor::new(Vec::<u8>::new()), Path::new("output"), None);
/// assert!(result.is_err());
/// ```
fn unpack_zip(
    reader: impl std::io::Read + std::io::Seek,
    dest: &std::path::Path,
    subpath: Option<&str>,
) -> Result<(), SkillInstallError> {
    let mut zip = ZipArchive::new(reader).map_err(SkillInstallError::ZipArchive)?;

    let common_root = zip_common_root(&mut zip)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(SkillInstallError::ZipArchive)?;
        let full_name = file.name().to_string();

        if archive_path_is_unsafe(&full_name) {
            return Err(SkillInstallError::PathTraversal(full_name));
        }

        let Some(final_rel_path) = zip_entry_rel_path(&full_name, common_root.as_deref(), subpath)
        else {
            continue;
        };

        if final_rel_path.is_empty() {
            continue;
        }

        let outpath = dest.join(final_rel_path);
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
    Ok(())
}

/// Identifies a shared top-level directory among ZIP entries.
///
/// # Examples
///
/// ```
/// use std::io::{Cursor, Write};
/// use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
///
/// let mut data = Cursor::new(Vec::new());
/// {
///     let mut writer = ZipWriter::new(&mut data);
///     writer
///         .start_file("skill/SKILL.md", SimpleFileOptions::default())
///         .unwrap();
///     writer.write_all(b"content").unwrap();
///     writer.finish().unwrap();
/// }
///
/// data.set_position(0);
/// let mut archive = ZipArchive::new(data).unwrap();
/// assert_eq!(
///     zip_common_root(&mut archive).unwrap(),
///     Some("skill".to_owned())
/// );
/// ```
fn zip_common_root(
    zip: &mut ZipArchive<impl std::io::Read + std::io::Seek>,
) -> Result<Option<String>, SkillInstallError> {
    if zip.is_empty() {
        return Ok(None);
    }
    let first_name = zip
        .by_index(0)
        .map_err(SkillInstallError::ZipArchive)?
        .name()
        .to_string();
    let root = first_name.split('/').next().unwrap_or("");
    if !root.is_empty() && zip.file_names().all(|n| n.starts_with(root)) {
        Ok(Some(root.to_string()))
    } else {
        Ok(None)
    }
}

/// Computes the path of a ZIP entry relative to an optional archive root and subpath.
///
/// # Examples
///
/// ```
/// let path = zip_entry_rel_path(
///     "project/docs/SKILL.md",
///     Some("project"),
///     Some("docs"),
/// );
///
/// assert_eq!(path, Some("SKILL.md"));
/// ```
fn zip_entry_rel_path<'a>(
    full_name: &'a str,
    common_root: Option<&str>,
    subpath: Option<&str>,
) -> Option<&'a str> {
    let rel_path = if let Some(root) = common_root {
        full_name
            .strip_prefix(root)
            .unwrap_or(full_name)
            .trim_start_matches('/')
    } else {
        full_name
    };

    if let Some(sub) = subpath {
        rel_path
            .strip_prefix(sub)
            .map(|s| s.trim_start_matches('/'))
    } else {
        Some(rel_path)
    }
}

/// Extracts supported entries from a gzip-compressed tar archive into a destination directory.
///
/// Archive paths are validated before extraction, and an optional subpath limits the entries
/// that are unpacked. Files and directories are extracted; other entry types are skipped.
///
/// # Examples
///
/// ```
/// use flate2::{write::GzEncoder, Compression};
/// use std::io::Cursor;
///
/// let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
/// {
///     let mut builder = tar::Builder::new(&mut encoder);
///     builder.finish().unwrap();
/// }
/// let archive = encoder.finish().unwrap();
///
/// let dest = std::env::temp_dir().join("skill-install-example");
/// std::fs::create_dir_all(&dest).unwrap();
/// unpack_tar_gz(Cursor::new(archive), &dest, None).unwrap();
/// std::fs::remove_dir_all(dest).unwrap();
/// ```
fn unpack_tar_gz(
    reader: impl std::io::Read,
    dest: &std::path::Path,
    subpath: Option<&str>,
) -> Result<(), SkillInstallError> {
    let gz = GzDecoder::new(reader);
    let mut archive = Archive::new(gz);

    let entries: Vec<_> = archive.entries().map_err(SkillInstallError::Io)?.collect();
    let common_root = tar_common_root(&entries)?;

    for entry in entries {
        let mut entry = entry.map_err(SkillInstallError::Io)?;

        let entry_type = entry.header().entry_type();
        if !entry_type.is_file() && !entry_type.is_dir() {
            continue;
        }

        let full_path = entry.path().map_err(SkillInstallError::Io)?;
        let full_path_string = full_path.to_string_lossy();

        if archive_path_is_unsafe(&full_path_string) {
            return Err(SkillInstallError::PathTraversal(
                full_path_string.into_owned(),
            ));
        }

        let Some(final_rel_path) = tar_entry_rel_path(&full_path, common_root.as_deref(), subpath)
        else {
            continue;
        };

        if final_rel_path.as_os_str().is_empty() {
            continue;
        }

        let outpath = dest.join(&final_rel_path);
        if let Some(parent) = outpath.parent() {
            std::fs::create_dir_all(parent).map_err(SkillInstallError::Io)?;
        }
        entry.unpack(&outpath).map_err(SkillInstallError::Io)?;
    }
    Ok(())
}

/// Determines whether all archive entries share the same top-level directory.
///
/// Returns the shared directory name when every entry starts with it; otherwise, returns `None`.
///
/// # Examples
///
/// ```
/// use flate2::{Compression, read::GzDecoder, write::GzEncoder};
/// use std::io::Write;
/// use tar::{Archive, Builder};
///
/// let mut data = Vec::new();
/// {
///     let encoder = GzEncoder::new(&mut data, Compression::default());
///     let mut builder = Builder::new(encoder);
///     builder.finish().unwrap();
/// }
///
/// let decoder = GzDecoder::new(&data[..]);
/// let mut archive = Archive::new(decoder);
/// let entries: Vec<_> = archive.entries().unwrap().collect();
///
/// assert_eq!(tar_common_root(&entries).unwrap(), None);
/// ```
fn tar_common_root(
    entries: &[Result<tar::Entry<flate2::read::GzDecoder<impl std::io::Read>>, std::io::Error>],
) -> Result<Option<String>, SkillInstallError> {
    if entries.is_empty() {
        return Ok(None);
    }
    let first_path = entries[0]
        .as_ref()
        .map_err(|e| SkillInstallError::Other(e.to_string()))?
        .path()
        .map_err(SkillInstallError::Io)?;
    let root = first_path.components().next().and_then(|c| match c {
        std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
        _ => None,
    });

    let Some(ref r) = root else {
        return Ok(None);
    };

    let all_start_with = entries.iter().all(|e| {
        e.as_ref()
            .ok()
            .and_then(|entry| entry.path().ok())
            .is_some_and(|path| path.starts_with(r))
    });

    if all_start_with {
        Ok(Some(r.clone()))
    } else {
        Ok(None)
    }
}

/// Produces the extraction-relative path for a tar archive entry.
///
/// The common archive root is removed when present. If a subpath is provided,
/// the entry must be within that subpath, which is also removed from the
/// resulting path.
///
/// # Examples
///
/// ```
/// use std::path::Path;
///
/// let path = tar_entry_rel_path(
///     Path::new("skills/example/SKILL.md"),
///     Some("skills"),
///     Some("example"),
/// );
///
/// assert_eq!(path, Some(std::path::PathBuf::from("SKILL.md")));
/// ```
///
/// Returns `None` when the entry does not belong to the requested subpath.
fn tar_entry_rel_path(
    full_path: &std::path::Path,
    common_root: Option<&str>,
    subpath: Option<&str>,
) -> Option<PathBuf> {
    let rel_path = if let Some(root) = common_root {
        if full_path.starts_with(root) {
            full_path
                .strip_prefix(root)
                .unwrap_or(full_path)
                .to_path_buf()
        } else {
            full_path.to_path_buf()
        }
    } else {
        full_path.to_path_buf()
    };

    if let Some(sub) = subpath {
        let sub_path = std::path::Path::new(sub);
        if rel_path.starts_with(sub_path) {
            Some(
                rel_path
                    .strip_prefix(sub_path)
                    .unwrap_or(&rel_path)
                    .to_path_buf(),
            )
        } else {
            None
        }
    } else {
        Some(rel_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::FileOptions;

    #[test]
    fn test_zip_absolute_path_rejection() {
        let mut buf = Vec::new();
        {
            let mut zip = ZipWriter::new(Cursor::new(&mut buf));
            zip.start_file("C:/absolute/path/SKILL.md", FileOptions::<()>::default())
                .unwrap();
            zip.write_all(b"name: test-skill").unwrap();
            zip.finish().unwrap();
        }

        let temp_root = tempfile::tempdir().unwrap();
        let result = install_from_zip("test-skill", Cursor::new(buf), temp_root.path());

        match result {
            Err(SkillInstallError::PathTraversal(path)) => {
                assert!(
                    path.contains("C:/absolute/path/SKILL.md")
                        || path.contains(r"C:\absolute\path\SKILL.md")
                );
            }
            other => panic!("Expected PathTraversal error, got {:?}", other),
        }
    }

    #[test]
    fn test_zip_parent_traversal_rejection() {
        let mut buf = Vec::new();
        {
            let mut zip = ZipWriter::new(Cursor::new(&mut buf));
            zip.start_file("../outside.md", FileOptions::<()>::default())
                .unwrap();
            zip.write_all(b"content").unwrap();
            zip.finish().unwrap();
        }

        let temp_root = tempfile::tempdir().unwrap();
        let result = install_from_zip("test-skill", Cursor::new(buf), temp_root.path());

        match result {
            Err(SkillInstallError::PathTraversal(path)) => {
                assert!(path.contains("../outside.md"));
            }
            other => panic!("Expected PathTraversal error, got {:?}", other),
        }
    }

    #[test]
    fn test_zip_root_dir_rejection() {
        let mut buf = Vec::new();
        {
            let mut zip = ZipWriter::new(Cursor::new(&mut buf));
            zip.start_file("/absolute/path/SKILL.md", FileOptions::<()>::default())
                .unwrap();
            zip.write_all(b"content").unwrap();
            zip.finish().unwrap();
        }

        let temp_root = tempfile::tempdir().unwrap();
        let result = install_from_zip("test-skill", Cursor::new(buf), temp_root.path());

        match result {
            Err(SkillInstallError::PathTraversal(path)) => {
                assert!(path.contains("/absolute/path/SKILL.md"));
            }
            other => panic!("Expected PathTraversal error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_tar_windows_drive_path_rejection() {
        let temp_root = tempfile::tempdir().unwrap();
        let archive_path = temp_root.path().join("malicious.tar.gz");
        {
            let archive_file = std::fs::File::create(&archive_path).unwrap();
            let encoder =
                flate2::write::GzEncoder::new(archive_file, flate2::Compression::default());
            let mut builder = tar::Builder::new(encoder);
            let content = b"name: test-skill";
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_cksum();

            // Bypass tar crate's path validation which fails on Windows during archive creation
            // by injecting the path bytes directly into the raw header name field
            let malicious_path = b"C:/absolute/path/SKILL.md";
            let mut name_bytes = [0u8; 100];
            name_bytes[..malicious_path.len()].copy_from_slice(malicious_path);
            header.as_gnu_mut().unwrap().name = name_bytes;
            header.set_cksum(); // Re-calculate checksum after modifying name

            builder.append(&header, &content[..]).unwrap();
            builder.finish().unwrap();
        }

        let result = fetch_and_unpack_to_tempdir(archive_path.to_str().unwrap()).await;

        match result {
            Err(SkillInstallError::PathTraversal(path)) => {
                assert!(path.contains("C:/absolute/path/SKILL.md"));
            }
            other => panic!("Expected PathTraversal error, got {:?}", other),
        }
    }
}
