use flate2::read::GzDecoder;
use reqwest::Client;
use tar::Archive;
use tempfile::TempDir;
use zip::read::ZipArchive;

#[derive(Debug)]
pub enum SkillInstallError {
    IO,
    Network,
    ArchiveFormat,
    PathTraversal,
    Permission,
    ManifestRead,
    ManifestParse,
    Validation,
}

impl std::fmt::Display for SkillInstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillInstallError::IO => write!(f, "IO error"),
            SkillInstallError::Network => write!(f, "Network error"),
            SkillInstallError::ArchiveFormat => write!(f, "Archive format error"),
            SkillInstallError::PathTraversal => write!(f, "Path traversal attempt in archive"),
            SkillInstallError::Permission => write!(f, "Permission error (fs write)"),
            SkillInstallError::ManifestRead => write!(f, "Failed to read manifest"),
            SkillInstallError::ManifestParse => write!(f, "Manifest parse failed"),
            SkillInstallError::Validation => write!(f, "Manifest validation failed"),
        }
    }
}

impl std::error::Error for SkillInstallError {}

/// Synchronously fetch, extract, and copy a skill source (dir, archive, or URL) into the target directory and validate manifest.
pub fn blocking_fetch_and_install_skill(
    skill_id: &str,
    source: &str,
    target_root: &std::path::Path,
) -> Result<(), SkillInstallError> {
    let rt = tokio::runtime::Runtime::new().map_err(|_| SkillInstallError::IO)?;
    let tempdir = rt.block_on(fetch_and_unpack_to_tempdir(source))?;
    let skill_dir = target_root.join(skill_id);
    eprintln!("[DEBUG install] skill_id: {}", skill_id);
    eprintln!("[DEBUG install] target_root: {}", target_root.display());
    eprintln!("[DEBUG install] source: {}", source);
    eprintln!(
        "[DEBUG install] tempdir contents: {:?}",
        std::fs::read_dir(tempdir.path())
            .map(|rd| rd
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect::<Vec<_>>())
            .unwrap_or_else(|_| vec![])
    );
    if skill_dir.exists() {
        std::fs::remove_dir_all(&skill_dir).map_err(|_| SkillInstallError::Permission)?;
    }
    // Recursively copy contents of tempdir into skill_dir
    fn copy_dir_recursively(
        src: &std::path::Path,
        dst: &std::path::Path,
    ) -> Result<(), SkillInstallError> {
        std::fs::create_dir_all(dst).map_err(|_| SkillInstallError::Permission)?;
        for entry in std::fs::read_dir(src).map_err(|_| SkillInstallError::IO)? {
            let entry = entry.map_err(|_| SkillInstallError::IO)?;
            let file_type = entry.file_type().map_err(|_| SkillInstallError::IO)?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if file_type.is_dir() {
                copy_dir_recursively(&src_path, &dst_path)?;
            } else {
                std::fs::copy(&src_path, &dst_path).map_err(|_| SkillInstallError::IO)?;
            }
        }
        Ok(())
    }
    copy_dir_recursively(tempdir.path(), &skill_dir)?;
    // Validate manifest
    let manifest_path = skill_dir.join("SKILL.md");
    let parsed = crate::skills::manifest::parse_skill_manifest(&manifest_path)?;
    // Write registry entry
    let entry = crate::skills::registry::SkillEntry {
        name: Some(parsed.name.clone()),
        description: parsed.description.clone(),
        version: Some(parsed.version.clone()),
        provider: None,
        source: Some(source.to_string()),
        installed_at: Some(chrono::Utc::now().to_rfc3339()),
        files: None,         // Could enumerate actual files
        manifest_hash: None, // Could hash contents
    };
    let registry_path = target_root.join("registry.json");
    eprintln!(
        "[DEBUG install] Writing registry.json to: {}",
        registry_path.display()
    );
    crate::skills::registry::update_registry_entry(&registry_path, skill_id, entry)
        .map_err(|_| SkillInstallError::IO)?;
    Ok(())
}

/// Fetch a skill archive (HTTP, file://, or local file path) and unpack to a temp directory, returning the directory path.
pub async fn fetch_and_unpack_to_tempdir(url: &str) -> Result<TempDir, SkillInstallError> {
    use std::io::Cursor;
    use std::path::Path;
    let tmp = tempfile::TempDir::new().map_err(|_| SkillInstallError::IO)?;
    let is_file = url.starts_with("file://");
    let is_local = url.starts_with("/") || url.chars().nth(1) == Some(':');
    let client = Client::new();
    let (data, ext) = if is_file || is_local {
        let path = if is_file {
            Path::new(&url[7..])
        } else {
            Path::new(url)
        };
        let ext = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if path.is_dir() {
            // Local directory: copy recursively to tempdir and return
            fn copy_dir_recursively(
                src: &std::path::Path,
                dst: &std::path::Path,
            ) -> Result<(), SkillInstallError> {
                std::fs::create_dir_all(dst).map_err(|_| SkillInstallError::Permission)?;
                for entry in std::fs::read_dir(src).map_err(|_| SkillInstallError::IO)? {
                    let entry = entry.map_err(|_| SkillInstallError::IO)?;
                    let file_type = entry.file_type().map_err(|_| SkillInstallError::IO)?;
                    let src_path = entry.path();
                    let dst_path = dst.join(entry.file_name());
                    if file_type.is_dir() {
                        copy_dir_recursively(&src_path, &dst_path)?;
                    } else {
                        std::fs::copy(&src_path, &dst_path).map_err(|_| SkillInstallError::IO)?;
                    }
                }
                Ok(())
            }
            copy_dir_recursively(path, tmp.path())?;
            return Ok(tmp);
        }
        let data = std::fs::read(path).map_err(|_| SkillInstallError::IO)?;
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
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|_| SkillInstallError::Network)?
            .error_for_status()
            .map_err(|_| SkillInstallError::Network)?;
        let data = resp
            .bytes()
            .await
            .map_err(|_| SkillInstallError::Network)?
            .to_vec();
        (data, ext)
    };
    // Unpack archive type into the tempdir
    if ext == "zip" {
        let reader = Cursor::new(&data);
        let mut zip = ZipArchive::new(reader).map_err(|_| SkillInstallError::ArchiveFormat)?;
        for i in 0..zip.len() {
            let mut file = zip
                .by_index(i)
                .map_err(|_| SkillInstallError::ArchiveFormat)?;
            let outpath = tmp.path().join(file.name());
            if file.is_dir() {
                std::fs::create_dir_all(&outpath).map_err(|_| SkillInstallError::Permission)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    std::fs::create_dir_all(parent).map_err(|_| SkillInstallError::Permission)?;
                }
                let mut out =
                    std::fs::File::create(&outpath).map_err(|_| SkillInstallError::Permission)?;
                std::io::copy(&mut file, &mut out).map_err(|_| SkillInstallError::IO)?;
            }
        }
    } else if ext == "gz" || url.ends_with(".tar.gz") {
        let reader = Cursor::new(&data);
        let gz = GzDecoder::new(reader);
        let mut archive = Archive::new(gz);
        for entry in archive
            .entries()
            .map_err(|_| SkillInstallError::ArchiveFormat)?
        {
            let mut entry = entry.map_err(|_| SkillInstallError::ArchiveFormat)?;
            let path = entry.path().map_err(|_| SkillInstallError::ArchiveFormat)?;
            if path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err(SkillInstallError::PathTraversal);
            }
            let outpath = tmp.path().join(&path);
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).map_err(|_| SkillInstallError::Permission)?;
            }
            entry.unpack(&outpath).map_err(|_| SkillInstallError::IO)?;
        }
    } else {
        return Err(SkillInstallError::ArchiveFormat);
    }
    Ok(tmp)
}
