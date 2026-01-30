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
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read registry: {}", path.display()))?;
    let reg: Registry = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse registry JSON: {}", path.display()))?;
    Ok(reg)
}

/// Update or insert a skill entry into the registry. If the registry file does not exist,
/// a new registry will be created.
pub fn update_registry_entry(path: &Path, skill_id: &str, entry: SkillEntry) -> Result<()> {
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
