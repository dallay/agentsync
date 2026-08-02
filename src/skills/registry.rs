use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct Registry {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    pub last_updated: Option<String>,
    pub skills: Option<BTreeMap<String, SkillEntry>>,
}

/// Versioned metadata describing a curated skill. This is deliberately separate from the
/// installed `registry.json` contract above.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RegistryDocument {
    pub schema_version: String,
    pub entries: BTreeMap<String, RegistryEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    pub provider_skill_id: String,
    pub local_skill_id: String,
    pub source: SourcePin,
    pub manifest: ManifestExpectation,
    pub files: Vec<FileHash>,
    pub license: LicenseEvidence,
    pub validation: ValidationMetadata,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SourcePin {
    pub repository: String,
    pub commit: String,
    pub subpath: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct FileHash {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ManifestExpectation {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct LicenseEvidence {
    pub spdx: String,
    pub source: String,
    pub attribution_required: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ValidationMetadata {
    pub status: String,
    pub validator: String,
}

/// Load and validate a curated registry document from TOML.
pub fn load_curated_registry(path: &Path) -> Result<RegistryDocument> {
    let safe_path = validated_registry_path(path, false)?;
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path —
    // `validated_registry_path` rejects traversal and resolves the parent before this read.
    let content = fs::read_to_string(&safe_path)
        .with_context(|| format!("failed to read curated registry: {}", path.display()))?;
    let registry: RegistryDocument = toml::from_str(&content)
        .with_context(|| format!("failed to parse curated registry: {}", path.display()))?;
    validate_curated_registry(&registry)
        .with_context(|| format!("invalid curated registry: {}", path.display()))?;
    Ok(registry)
}

/// Write a deterministic lockfile from a validated registry manifest.
pub fn sync_curated_registry(manifest_path: &Path, lock_path: &Path) -> Result<()> {
    let registry = load_curated_registry(manifest_path)?;
    let body = toml::to_string_pretty(&registry)
        .context("failed to serialize curated registry lockfile")?;
    let safe_lock_path = validated_registry_path(lock_path, true)?;
    let parent = safe_lock_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("lockfile path has no parent: {}", lock_path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create lockfile directory: {}", parent.display()))?;
    let temporary = tempfile::NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "failed to create temporary lockfile in {}",
            parent.display()
        )
    })?;
    fs::write(temporary.path(), body).with_context(|| {
        format!(
            "failed to write temporary lockfile: {}",
            temporary.path().display()
        )
    })?;
    temporary
        .persist(&safe_lock_path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to replace lockfile: {}", lock_path.display()))?;
    Ok(())
}

fn validated_registry_path(path: &Path, allow_missing: bool) -> Result<std::path::PathBuf> {
    anyhow::ensure!(
        !path.as_os_str().is_empty(),
        "registry path must not be empty"
    );
    anyhow::ensure!(
        path.components()
            .all(|component| !matches!(component, std::path::Component::ParentDir)),
        "registry path contains unsafe components: {}",
        path.display()
    );

    let parent = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .with_context(|| format!("failed to resolve registry path parent: {}", path.display()))?;
    let safe_path = parent.join(path.file_name().unwrap());
    if !allow_missing {
        anyhow::ensure!(
            safe_path.is_file(),
            "registry path is not a regular file: {}",
            path.display()
        );
    }
    Ok(safe_path)
}

/// Validate the metadata needed to resolve a curated entry deterministically.
pub fn validate_curated_registry(registry: &RegistryDocument) -> Result<()> {
    anyhow::ensure!(
        registry.schema_version == "v1",
        "schema_version: unsupported value `{}` (expected v1)",
        registry.schema_version
    );
    anyhow::ensure!(!registry.entries.is_empty(), "entries: must not be empty");

    for (key, entry) in &registry.entries {
        anyhow::ensure!(!key.is_empty(), "entries: key must not be empty");
        anyhow::ensure!(
            key == &entry.local_skill_id,
            "entries.{key}.local_skill_id: must match entry key"
        );
        anyhow::ensure!(
            !entry.provider_skill_id.is_empty(),
            "entries.{key}.provider_skill_id: required"
        );
        validate_id(
            &format!("entries.{key}.local_skill_id"),
            &entry.local_skill_id,
        )?;
        validate_commit(
            &format!("entries.{key}.source.commit"),
            &entry.source.commit,
        )?;
        validate_safe_path(
            &format!("entries.{key}.source.subpath"),
            &entry.source.subpath,
        )?;
        anyhow::ensure!(
            !entry.source.repository.is_empty(),
            "entries.{key}.source.repository: required"
        );
        validate_id(
            &format!("entries.{key}.manifest.name"),
            &entry.manifest.name,
        )?;
        anyhow::ensure!(
            !entry.files.is_empty(),
            "entries.{key}.files: must not be empty"
        );
        let mut has_manifest = false;
        for (index, file) in entry.files.iter().enumerate() {
            let field = format!("entries.{key}.files[{index}]");
            validate_safe_path(&format!("{field}.path"), &file.path)?;
            anyhow::ensure!(
                is_hex(&file.sha256, 64),
                "{field}.sha256: expected 64 hexadecimal characters"
            );
            has_manifest |= file.path == "SKILL.md";
        }
        anyhow::ensure!(has_manifest, "entries.{key}.files: must declare SKILL.md");
        validate_spdx(&format!("entries.{key}.license.spdx"), &entry.license.spdx)?;
        anyhow::ensure!(
            !entry.license.source.is_empty(),
            "entries.{key}.license.source: required"
        );
        anyhow::ensure!(
            entry.validation.status == "approved",
            "entries.{key}.validation.status: must be approved"
        );
        anyhow::ensure!(
            !entry.validation.validator.is_empty(),
            "entries.{key}.validation.validator: required"
        );
    }
    Ok(())
}

fn validate_id(field: &str, value: &str) -> Result<()> {
    anyhow::ensure!(!value.is_empty(), "{field}: required");
    anyhow::ensure!(
        value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'),
        "{field}: must use lowercase slug characters"
    );
    Ok(())
}

fn validate_commit(field: &str, value: &str) -> Result<()> {
    anyhow::ensure!(
        is_hex(value, 40),
        "{field}: expected full 40-character commit SHA"
    );
    Ok(())
}

fn validate_safe_path(field: &str, value: &str) -> Result<()> {
    anyhow::ensure!(!value.is_empty(), "{field}: required");
    anyhow::ensure!(
        !value.starts_with('/')
            && !value.starts_with('\\')
            && !value.contains(':')
            && value
                .split(['/', '\\'])
                .all(|part| !part.is_empty() && part != "." && part != ".."),
        "{field}: path must be relative and traversal-free"
    );
    Ok(())
}

fn validate_spdx(field: &str, value: &str) -> Result<()> {
    const APPROVED: &[&str] = &[
        "MIT",
        "Apache-2.0",
        "BSD-2-Clause",
        "BSD-3-Clause",
        "ISC",
        "MPL-2.0",
        "LGPL-2.1-only",
        "LGPL-3.0-only",
        "GPL-2.0-only",
        "GPL-3.0-only",
    ];
    anyhow::ensure!(!value.is_empty(), "{field}: required");
    for token in value.split_whitespace() {
        let token = token.trim_matches(['(', ')']);
        if matches!(token, "AND" | "OR") {
            continue;
        }
        anyhow::ensure!(
            APPROVED.contains(&token),
            "{field}: unapproved SPDX identifier or expression"
        );
    }
    Ok(())
}

fn is_hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|c| c.is_ascii_hexdigit())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillEntry {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub provider: Option<String>,
    pub source: Option<String>,
    #[serde(rename = "installedAt")]
    pub installed_at: Option<String>,
    pub files: Option<Vec<String>>,
    #[serde(rename = "manifestHash")]
    pub manifest_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledSkillState {
    pub installed: bool,
    pub version: Option<String>,
}

pub fn write_registry(path: &Path) -> Result<()> {
    let reg = Registry {
        schema_version: 1,
        last_updated: Some(Utc::now().to_rfc3339()),
        skills: Some(BTreeMap::new()),
    };
    let body = serde_json::to_string_pretty(&reg)?;
    fs::create_dir_all(path.parent().unwrap()).with_context(|| {
        format!(
            "failed to create registry dir: {}",
            path.parent().unwrap().display()
        )
    })?;
    fs::write(path, body)
        .with_context(|| format!("failed to write registry: {}", path.display()))?;
    Ok(())
}

/// Read the registry from disk and deserialize it.
pub fn read_registry(path: &Path) -> Result<Registry> {
    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path —
    // installed registry paths are supplied by the application and skill IDs are validated before writes.
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read registry: {}", path.display()))?;
    let reg: Registry = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse registry JSON: {}", path.display()))?;
    Ok(reg)
}

/// Update or insert a skill entry into the registry. If the registry file does not exist,
/// a new registry will be created.
pub fn update_registry_entry(path: &Path, skill_id: &str, entry: SkillEntry) -> Result<()> {
    anyhow::ensure!(
        !skill_id.is_empty()
            && skill_id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'),
        "invalid skill id: {skill_id}"
    );
    let mut reg = if path.exists() {
        read_registry(path)?
    } else {
        Registry {
            schema_version: 1,
            last_updated: None,
            skills: Some(BTreeMap::new()),
        }
    };

    if reg.skills.is_none() {
        reg.skills = Some(BTreeMap::new());
    }

    reg.skills
        .as_mut()
        .unwrap()
        .insert(skill_id.to_string(), entry);
    reg.last_updated = Some(Utc::now().to_rfc3339());

    let body = serde_json::to_string_pretty(&reg)?;
    fs::create_dir_all(path.parent().unwrap()).with_context(|| {
        format!(
            "failed to create registry dir: {}",
            path.parent().unwrap().display()
        )
    })?;
    fs::write(path, body)
        .with_context(|| format!("failed to write registry: {}", path.display()))?;
    Ok(())
}

pub fn read_installed_skill_states(path: &Path) -> Result<BTreeMap<String, InstalledSkillState>> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }

    let registry = read_registry(path)?;
    let states = registry
        .skills
        .unwrap_or_default()
        .into_iter()
        .map(|(skill_id, entry)| {
            (
                skill_id,
                InstalledSkillState {
                    installed: true,
                    version: entry.version,
                },
            )
        })
        .collect();

    Ok(states)
}
