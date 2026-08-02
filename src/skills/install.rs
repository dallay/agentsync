use flate2::read::GzDecoder;
// futures_util::StreamExt is used locally where needed
use crate::skills::registry::RegistryEntry;
use anyhow::Error as AnyhowError;
use reqwest::{Client, Error as ReqwestError};
use sha2::{Digest, Sha256};
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
    path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains(':')
        || path
            .trim_end_matches(['/', '\\'])
            .split(['/', '\\'])
            .any(|part| part.is_empty() || part == "." || part == "..")
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

    // Create a temporary directory for rollback if anything fails
    let temp_skill_dir = tempfile::TempDir::new().map_err(SkillInstallError::Io)?;
    let staging_dir = temp_skill_dir.path();

    copy_dir_recursively(src_dir, staging_dir)?;
    install_staged(skill_id, staging_dir, target_root, src_dir)
}

fn install_staged(
    skill_id: &str,
    staging_dir: &Path,
    target_root: &Path,
    source: &Path,
) -> Result<(), SkillInstallError> {
    let skill_dir = target_root.join(skill_id);
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
        source: Some(source.to_string_lossy().into_owned()),
        installed_at: Some(chrono::Utc::now().to_rfc3339()),
        files: None,
        manifest_hash: None,
    };
    let registry_path = target_root.join("registry.json");
    crate::skills::registry::update_registry_entry(&registry_path, skill_id, entry)
        .map_err(SkillInstallError::Registry)?;
    Ok(())
}

/// Validate curated metadata in staging before invoking the existing atomic installer.
pub fn install_from_dir_verified(
    skill_id: &str,
    src_dir: &Path,
    target_root: &Path,
    expected: &RegistryEntry,
) -> Result<(), SkillInstallError> {
    if expected.local_skill_id != skill_id {
        return Err(SkillInstallError::Validation(format!(
            "registry local_skill_id `{}` does not match requested skill `{skill_id}`",
            expected.local_skill_id
        )));
    }
    let staged = TempDir::new().map_err(SkillInstallError::Io)?;
    copy_dir_recursively(src_dir, staged.path())?;
    for file in &expected.files {
        let relative = Path::new(&file.path);
        if archive_path_is_unsafe(&file.path) {
            return Err(SkillInstallError::Validation(format!(
                "unsafe expected path: {}",
                file.path
            )));
        }
        let path = staged.path().join(relative);
        let bytes = std::fs::read(&path)
            .map_err(|e| SkillInstallError::Validation(format!("{}: {e}", file.path)))?;
        let actual = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if actual != file.sha256.to_ascii_lowercase() {
            return Err(SkillInstallError::Validation(format!(
                "{} sha256 mismatch: expected {}, got {}",
                file.path, file.sha256, actual
            )));
        }
    }
    let manifest = crate::skills::manifest::parse_skill_manifest(&staged.path().join("SKILL.md"))?;
    if manifest.name != expected.manifest.name
        || manifest.version != expected.manifest.version
        || manifest.description != expected.manifest.description
    {
        return Err(SkillInstallError::Validation(
            "manifest does not match curated registry metadata".into(),
        ));
    }
    if expected.license.spdx.is_empty()
        || expected.license.spdx == "LicenseRef-Unknown"
        || expected.validation.status != "approved"
    {
        return Err(SkillInstallError::Validation(
            "curated registry license or validation policy rejected entry".into(),
        ));
    }
    install_staged(skill_id, staged.path(), target_root, src_dir)
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

/// Result of fetching a skill source — either we already copied a directory,
/// or we have archive bytes to unpack.
enum FetchedSource {
    DirectoryCopied,
    Archive(Vec<u8>),
}

pub async fn fetch_and_unpack_to_tempdir(url: &str) -> Result<TempDir, SkillInstallError> {
    use std::io::Cursor;

    // Support subpaths via fragments, e.g. https://example.com/archive.zip#subpath
    let (url_base, subpath) = match url.find('#') {
        Some(pos) => (&url[..pos], Some(&url[pos + 1..])),
        None => (url, None),
    };

    let tmp = tempfile::TempDir::new().map_err(SkillInstallError::Io)?;

    let (source, ext) = fetch_archive_data(url_base, tmp.path()).await?;

    let data = match source {
        FetchedSource::DirectoryCopied => return Ok(tmp),
        FetchedSource::Archive(bytes) => bytes,
    };

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

async fn fetch_archive_data(
    url_base: &str,
    tmp_path: &std::path::Path,
) -> Result<(FetchedSource, String), SkillInstallError> {
    let is_file = url_base.starts_with("file://");
    let is_local = url_base.starts_with('/') || url_base.chars().nth(1) == Some(':');

    if is_file || is_local {
        fetch_local_data(url_base, is_file, tmp_path)
    } else {
        fetch_remote_data(url_base, tmp_path).await
    }
}

fn fetch_local_data(
    url_base: &str,
    is_file: bool,
    tmp_path: &std::path::Path,
) -> Result<(FetchedSource, String), SkillInstallError> {
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
        return Ok((FetchedSource::DirectoryCopied, String::new()));
    }
    let data = std::fs::read(path).map_err(SkillInstallError::Io)?;
    Ok((FetchedSource::Archive(data), ext))
}

async fn fetch_remote_data(
    url_base: &str,
    tmp_path: &std::path::Path,
) -> Result<(FetchedSource, String), SkillInstallError> {
    const MAX_DOWNLOAD_SIZE: u64 = 100 * 1024 * 1024; // 100 MB

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

    // Check Content-Length header if available
    if let Some(content_length) = resp.content_length()
        && content_length > MAX_DOWNLOAD_SIZE
    {
        return Err(SkillInstallError::Other(format!(
            "download too large: {content_length} bytes exceeds {MAX_DOWNLOAD_SIZE} byte limit"
        )));
    }

    let stdfile =
        std::fs::File::create(tmp_path.join("download.tmp")).map_err(SkillInstallError::Io)?;
    let mut tmpfile = tokio::fs::File::from_std(stdfile);
    let mut stream = resp.bytes_stream();
    let mut total_bytes: u64 = 0;
    use futures_util::StreamExt as _;
    use tokio::io::AsyncWriteExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(SkillInstallError::Network)?;
        total_bytes += chunk.len() as u64;
        if total_bytes > MAX_DOWNLOAD_SIZE {
            let _ = std::fs::remove_file(tmp_path.join("download.tmp"));
            return Err(SkillInstallError::Other(format!(
                "download too large: exceeded {MAX_DOWNLOAD_SIZE} byte limit while streaming"
            )));
        }
        tmpfile
            .write_all(&chunk)
            .await
            .map_err(SkillInstallError::Io)?;
    }
    tmpfile.flush().await.map_err(SkillInstallError::Io)?;

    let data = std::fs::read(tmp_path.join("download.tmp")).map_err(SkillInstallError::Io)?;
    let _ = std::fs::remove_file(tmp_path.join("download.tmp"));
    Ok((FetchedSource::Archive(data), ext))
}

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
    if !root.is_empty()
        && zip
            .file_names()
            .all(|n| n == root || n.starts_with(&format!("{root}/")))
        && zip.file_names().any(|n| n.starts_with(&format!("{root}/")))
    {
        Ok(Some(root.to_string()))
    } else {
        Ok(None)
    }
}

fn zip_entry_rel_path<'a>(
    full_name: &'a str,
    common_root: Option<&str>,
    subpath: Option<&str>,
) -> Option<&'a str> {
    let rel_path = if let Some(root) = common_root {
        // Strip root only at a component boundary: "root/" prefix or exact match
        if let Some(rest) = full_name.strip_prefix(&format!("{root}/")) {
            rest
        } else if full_name == root {
            ""
        } else {
            full_name
        }
    } else {
        full_name
    };

    if let Some(sub) = subpath {
        // Normalize trailing slash: "src/" → "src"
        let sub = sub.trim_end_matches('/');
        // Strip subpath only at a component boundary
        if let Some(rest) = rel_path.strip_prefix(&format!("{sub}/")) {
            Some(rest)
        } else if rel_path == sub {
            Some("")
        } else {
            None
        }
    } else {
        Some(rel_path)
    }
}

fn unpack_tar_gz(
    reader: impl std::io::Read,
    dest: &std::path::Path,
    subpath: Option<&str>,
) -> Result<(), SkillInstallError> {
    // Limit decompressed size to prevent zip bombs
    const MAX_DECOMPRESSED_SIZE: u64 = 500 * 1024 * 1024; // 500 MB

    let mut decompressed = Vec::new();
    let gz = GzDecoder::new(reader);
    let mut limited = std::io::Read::take(gz, MAX_DECOMPRESSED_SIZE + 1);
    std::io::Read::read_to_end(&mut limited, &mut decompressed).map_err(SkillInstallError::Io)?;

    if decompressed.len() as u64 > MAX_DECOMPRESSED_SIZE {
        return Err(SkillInstallError::Other(format!(
            "decompressed archive too large: exceeds {MAX_DECOMPRESSED_SIZE} byte limit"
        )));
    }

    // First pass: determine common root
    let common_root = {
        let mut archive = Archive::new(std::io::Cursor::new(&decompressed));
        tar_common_root_from_archive(&mut archive)?
    };

    // Second pass: extract entries
    let mut archive = Archive::new(std::io::Cursor::new(&decompressed));
    for entry in archive.entries().map_err(SkillInstallError::Io)? {
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

fn tar_common_root_from_archive(
    archive: &mut Archive<impl std::io::Read>,
) -> Result<Option<String>, SkillInstallError> {
    let mut root: Option<String> = None;
    let mut has_nested = false;

    for entry in archive.entries().map_err(SkillInstallError::Io)? {
        let entry = entry.map_err(SkillInstallError::Io)?;
        let path = entry.path().map_err(SkillInstallError::Io)?;

        if path.components().count() > 1 {
            has_nested = true;
        }

        let first_component = path.components().next().and_then(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        });

        let Some(ref component) = first_component else {
            return Ok(None);
        };

        match &root {
            None => root = Some(component.clone()),
            Some(r) if r != component => return Ok(None),
            _ => {}
        }
    }

    if has_nested { Ok(root) } else { Ok(None) }
}

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
        let sub = sub.trim_end_matches('/');
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

    // --- zip_common_root tests ---

    fn make_zip(entries: &[&str]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = ZipWriter::new(Cursor::new(&mut buf));
            for entry in entries {
                zip.start_file(entry.to_string(), FileOptions::<()>::default())
                    .unwrap();
                zip.write_all(b"x").unwrap();
            }
            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_zip_common_root_single_prefix() {
        let buf = make_zip(&["foo/a.txt", "foo/b.txt"]);
        let mut zip = ZipArchive::new(Cursor::new(buf)).unwrap();
        assert_eq!(zip_common_root(&mut zip).unwrap(), Some("foo".to_string()));
    }

    #[test]
    fn test_zip_common_root_no_prefix() {
        let buf = make_zip(&["a.txt", "b.txt"]);
        let mut zip = ZipArchive::new(Cursor::new(buf)).unwrap();
        assert_eq!(zip_common_root(&mut zip).unwrap(), None);
    }

    #[test]
    fn test_zip_common_root_different_prefixes() {
        let buf = make_zip(&["foo/a.txt", "bar/b.txt"]);
        let mut zip = ZipArchive::new(Cursor::new(buf)).unwrap();
        assert_eq!(zip_common_root(&mut zip).unwrap(), None);
    }

    // --- zip_entry_rel_path tests ---

    #[test]
    fn test_zip_entry_rel_path_strips_root() {
        assert_eq!(
            zip_entry_rel_path("foo/a.txt", Some("foo"), None),
            Some("a.txt")
        );
    }

    #[test]
    fn test_zip_entry_rel_path_strips_subpath() {
        assert_eq!(
            zip_entry_rel_path("sub/file.txt", None, Some("sub")),
            Some("file.txt")
        );
    }

    #[test]
    fn test_zip_entry_rel_path_strips_root_and_subpath() {
        assert_eq!(
            zip_entry_rel_path("foo/sub/file.txt", Some("foo"), Some("sub")),
            Some("file.txt")
        );
    }

    #[test]
    fn test_zip_entry_rel_path_no_match_subpath() {
        assert_eq!(
            zip_entry_rel_path("other/file.txt", None, Some("sub")),
            None
        );
    }

    #[test]
    fn test_zip_entry_rel_path_component_boundary_safety() {
        // root="docs" should NOT strip from "docs2/file.txt"
        assert_eq!(
            zip_entry_rel_path("docs2/file.txt", Some("docs"), None),
            Some("docs2/file.txt")
        );
    }

    // --- tar_common_root tests ---

    fn make_tar_gz(entries: &[&str]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let encoder = flate2::write::GzEncoder::new(&mut buf, flate2::Compression::default());
            let mut builder = tar::Builder::new(encoder);
            for entry in entries {
                let content = b"x";
                let mut header = tar::Header::new_gnu();
                header.set_size(content.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder
                    .append_data(&mut header, *entry, &content[..])
                    .unwrap();
            }
            builder.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_tar_common_root_single_prefix() {
        let buf = make_tar_gz(&["foo/a.txt", "foo/b.txt"]);
        let mut decompressed = Vec::new();
        let mut gz = flate2::read::GzDecoder::new(Cursor::new(buf));
        std::io::Read::read_to_end(&mut gz, &mut decompressed).unwrap();
        let mut archive = tar::Archive::new(Cursor::new(&decompressed));
        assert_eq!(
            tar_common_root_from_archive(&mut archive).unwrap(),
            Some("foo".to_string())
        );
    }

    #[test]
    fn test_tar_common_root_no_prefix() {
        let buf = make_tar_gz(&["a.txt", "b.txt"]);
        let mut decompressed = Vec::new();
        let mut gz = flate2::read::GzDecoder::new(Cursor::new(buf));
        std::io::Read::read_to_end(&mut gz, &mut decompressed).unwrap();
        let mut archive = tar::Archive::new(Cursor::new(&decompressed));
        assert_eq!(tar_common_root_from_archive(&mut archive).unwrap(), None);
    }

    #[test]
    fn test_tar_common_root_different_prefixes() {
        let buf = make_tar_gz(&["foo/a.txt", "bar/b.txt"]);
        let mut decompressed = Vec::new();
        let mut gz = flate2::read::GzDecoder::new(Cursor::new(buf));
        std::io::Read::read_to_end(&mut gz, &mut decompressed).unwrap();
        let mut archive = tar::Archive::new(Cursor::new(&decompressed));
        assert_eq!(tar_common_root_from_archive(&mut archive).unwrap(), None);
    }

    // --- tar_entry_rel_path tests ---

    #[test]
    fn test_tar_entry_rel_path_strips_root() {
        let p = Path::new("foo/a.txt");
        assert_eq!(
            tar_entry_rel_path(p, Some("foo"), None),
            Some(PathBuf::from("a.txt"))
        );
    }

    #[test]
    fn test_tar_entry_rel_path_strips_subpath() {
        let p = Path::new("sub/file.txt");
        assert_eq!(
            tar_entry_rel_path(p, None, Some("sub")),
            Some(PathBuf::from("file.txt"))
        );
    }

    #[test]
    fn test_tar_entry_rel_path_strips_root_and_subpath() {
        let p = Path::new("foo/sub/file.txt");
        assert_eq!(
            tar_entry_rel_path(p, Some("foo"), Some("sub")),
            Some(PathBuf::from("file.txt"))
        );
    }

    #[test]
    fn test_tar_entry_rel_path_no_match_subpath() {
        let p = Path::new("other/file.txt");
        assert_eq!(tar_entry_rel_path(p, None, Some("sub")), None);
    }

    // --- fetch_local_data tests ---

    #[test]
    fn test_fetch_local_data_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.zip");
        std::fs::write(&file_path, b"fake zip data").unwrap();

        let (source, ext) =
            fetch_local_data(file_path.to_str().unwrap(), false, tmp.path()).unwrap();
        assert!(matches!(source, FetchedSource::Archive(data) if data == b"fake zip data"));
        assert_eq!(ext, "zip");
    }

    #[test]
    fn test_fetch_local_data_directory() {
        let src_dir = tempfile::tempdir().unwrap();
        std::fs::write(src_dir.path().join("SKILL.md"), b"name: test").unwrap();

        let dest_dir = tempfile::tempdir().unwrap();
        let (source, ext) =
            fetch_local_data(src_dir.path().to_str().unwrap(), false, dest_dir.path()).unwrap();
        assert!(matches!(source, FetchedSource::DirectoryCopied));
        assert_eq!(ext, "");
        // Verify the file was copied
        assert!(dest_dir.path().join("SKILL.md").exists());
    }

    #[test]
    fn test_fetch_local_data_file_uri() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("archive.tar.gz");
        std::fs::write(&file_path, b"fake tar data").unwrap();

        let uri = format!("file://{}", file_path.to_str().unwrap());
        let (source, ext) = fetch_local_data(&uri, true, tmp.path()).unwrap();
        assert!(matches!(source, FetchedSource::Archive(data) if data == b"fake tar data"));
        assert_eq!(ext, "gz");
    }

    // --- unpack_zip end-to-end tests ---

    #[test]
    fn test_unpack_zip_basic_extraction() {
        let mut buf = Vec::new();
        {
            let mut zip = ZipWriter::new(Cursor::new(&mut buf));
            zip.start_file("SKILL.md", FileOptions::<()>::default())
                .unwrap();
            zip.write_all(b"name: my-skill").unwrap();
            zip.start_file("src/main.rs", FileOptions::<()>::default())
                .unwrap();
            zip.write_all(b"fn main() {}").unwrap();
            zip.finish().unwrap();
        }

        let dest = tempfile::tempdir().unwrap();
        unpack_zip(Cursor::new(&buf), dest.path(), None).unwrap();

        assert!(dest.path().join("SKILL.md").exists());
        assert!(dest.path().join("src/main.rs").exists());
        assert_eq!(
            std::fs::read_to_string(dest.path().join("SKILL.md")).unwrap(),
            "name: my-skill"
        );
    }

    #[test]
    fn test_unpack_zip_with_common_root_stripping() {
        // Archive where all files share a common root "my-skill-v1/"
        let mut buf = Vec::new();
        {
            let mut zip = ZipWriter::new(Cursor::new(&mut buf));
            zip.add_directory("my-skill-v1/", FileOptions::<()>::default())
                .unwrap();
            zip.start_file("my-skill-v1/SKILL.md", FileOptions::<()>::default())
                .unwrap();
            zip.write_all(b"name: skill").unwrap();
            zip.start_file("my-skill-v1/lib.rs", FileOptions::<()>::default())
                .unwrap();
            zip.write_all(b"pub mod lib;").unwrap();
            zip.finish().unwrap();
        }

        let dest = tempfile::tempdir().unwrap();
        unpack_zip(Cursor::new(&buf), dest.path(), None).unwrap();

        // Common root "my-skill-v1" should be stripped
        assert!(dest.path().join("SKILL.md").exists());
        assert!(dest.path().join("lib.rs").exists());
    }

    #[test]
    fn test_unpack_zip_with_subpath_filter() {
        let mut buf = Vec::new();
        {
            let mut zip = ZipWriter::new(Cursor::new(&mut buf));
            zip.start_file("docs/readme.md", FileOptions::<()>::default())
                .unwrap();
            zip.write_all(b"docs").unwrap();
            zip.start_file("src/SKILL.md", FileOptions::<()>::default())
                .unwrap();
            zip.write_all(b"name: skill").unwrap();
            zip.start_file("src/code.rs", FileOptions::<()>::default())
                .unwrap();
            zip.write_all(b"code").unwrap();
            zip.finish().unwrap();
        }

        let dest = tempfile::tempdir().unwrap();
        unpack_zip(Cursor::new(&buf), dest.path(), Some("src")).unwrap();

        // Only files under "src/" should be extracted, with "src/" stripped
        assert!(dest.path().join("SKILL.md").exists());
        assert!(dest.path().join("code.rs").exists());
        assert!(!dest.path().join("docs").exists());
        assert!(!dest.path().join("readme.md").exists());
    }

    // --- unpack_tar_gz end-to-end tests ---

    #[test]
    fn test_unpack_tar_gz_basic_extraction() {
        let buf = make_tar_gz(&["SKILL.md", "src/main.rs"]);

        let dest = tempfile::tempdir().unwrap();
        unpack_tar_gz(Cursor::new(&buf), dest.path(), None).unwrap();

        assert!(dest.path().join("SKILL.md").exists());
        assert!(dest.path().join("src/main.rs").exists());
    }

    #[test]
    fn test_unpack_tar_gz_with_common_root_stripping() {
        let buf = make_tar_gz(&["root/SKILL.md", "root/lib.rs"]);

        let dest = tempfile::tempdir().unwrap();
        unpack_tar_gz(Cursor::new(&buf), dest.path(), None).unwrap();

        // Common root "root" should be stripped
        assert!(dest.path().join("SKILL.md").exists());
        assert!(dest.path().join("lib.rs").exists());
    }

    #[test]
    fn test_unpack_tar_gz_with_subpath_filter() {
        let buf = make_tar_gz(&[
            "root/docs/readme.md",
            "root/src/SKILL.md",
            "root/src/code.rs",
        ]);

        let dest = tempfile::tempdir().unwrap();
        unpack_tar_gz(Cursor::new(&buf), dest.path(), Some("src")).unwrap();

        // Only files under "src/" after root stripping
        assert!(dest.path().join("SKILL.md").exists());
        assert!(dest.path().join("code.rs").exists());
        assert!(!dest.path().join("docs").exists());
    }

    // --- find_best_skill_dir tests ---

    #[test]
    fn test_find_best_skill_dir_root_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("SKILL.md"), b"name: test").unwrap();
        assert_eq!(find_best_skill_dir(tmp.path(), "test"), tmp.path());
    }

    #[test]
    fn test_find_best_skill_dir_matching_subdir() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("my-skill");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("SKILL.md"), b"name: my-skill").unwrap();
        assert_eq!(find_best_skill_dir(tmp.path(), "my-skill"), sub);
    }

    #[test]
    fn test_find_best_skill_dir_sole_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("some-other-name");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("SKILL.md"), b"name: test").unwrap();
        // Only one manifest found, should return its parent even if name doesn't match
        assert_eq!(find_best_skill_dir(tmp.path(), "my-skill"), sub);
    }

    #[test]
    fn test_find_best_skill_dir_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        // No SKILL.md anywhere
        std::fs::write(tmp.path().join("README.md"), b"hello").unwrap();
        assert_eq!(find_best_skill_dir(tmp.path(), "test"), tmp.path());
    }

    // --- copy_dir_recursively tests ---

    #[test]
    fn test_copy_dir_recursively_basic() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("a.txt"), b"hello").unwrap();
        std::fs::create_dir_all(src.path().join("sub")).unwrap();
        std::fs::write(src.path().join("sub/b.txt"), b"world").unwrap();

        let dst = tempfile::tempdir().unwrap();
        let target = dst.path().join("output");
        copy_dir_recursively(src.path(), &target).unwrap();

        assert_eq!(
            std::fs::read_to_string(target.join("a.txt")).unwrap(),
            "hello"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("sub/b.txt")).unwrap(),
            "world"
        );
    }

    // --- archive_path_is_unsafe tests ---

    #[test]
    fn test_archive_path_is_unsafe_safe_paths() {
        assert!(!archive_path_is_unsafe("foo/bar.txt"));
        assert!(!archive_path_is_unsafe("SKILL.md"));
        assert!(!archive_path_is_unsafe("src/lib.rs"));
    }

    #[test]
    fn test_archive_path_is_unsafe_backslash_prefix() {
        assert!(archive_path_is_unsafe("\\\\server\\share"));
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
