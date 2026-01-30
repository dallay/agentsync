use crate::skills::install::SkillInstallError;

use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
}

static FRONTMATTER_RE: OnceLock<Regex> = OnceLock::new();
static NAME_RE: OnceLock<Regex> = OnceLock::new();

fn frontmatter_re() -> &'static Regex {
    FRONTMATTER_RE.get_or_init(|| Regex::new(r"(?s)^---\s*(?P<yaml>.*?)\s*---").unwrap())
}

fn name_re() -> &'static Regex {
    NAME_RE.get_or_init(|| Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").unwrap())
}

/// Parse SKILL.md and perform basic validation according to the spec:
/// - Frontmatter YAML exists
/// - `name` field present and matches required pattern
/// - `version` field optional; if present, must be valid semver
pub fn parse_skill_manifest(path: &Path) -> Result<SkillManifest, SkillInstallError> {
    let content = fs::read_to_string(path)
        .map_err(|e| SkillInstallError::Validation(format!("manifest read error: {}", e)))?;

    // Find first YAML frontmatter block
    let caps = frontmatter_re()
        .captures(&content)
        .ok_or(SkillInstallError::Validation(
            "manifest parse error: missing frontmatter".into(),
        ))?;

    let yaml = &caps["yaml"];
    let manifest: SkillManifest = serde_yaml::from_str(yaml)
        .map_err(|e| SkillInstallError::Validation(format!("manifest parse error: {}", e)))?;

    // Validate name: stricter pattern (no leading/trailing/consecutive hyphens)
    if !name_re().is_match(&manifest.name) {
        return Err(SkillInstallError::Validation("invalid name".into()));
    }

    // Validate version if present
    if let Some(ver) = &manifest.version
        && semver::Version::parse(ver).is_err()
    {
        return Err(SkillInstallError::Validation("invalid semver".into()));
    }

    Ok(manifest)
}
