use crate::skills::install::SkillInstallError;

use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
}

/// Parse SKILL.md and perform basic validation according to the spec:
/// - Frontmatter YAML exists
/// - `name` field present and matches required pattern
/// - `version` field present and must be valid semver
pub fn parse_skill_manifest(path: &Path) -> Result<SkillManifest, SkillInstallError> {
    let content = fs::read_to_string(path).map_err(|_| SkillInstallError::ManifestRead)?;

    // Find first YAML frontmatter block
    let re = Regex::new(r"(?s)^---\s*(?P<yaml>.*?)\s*---").unwrap();
    let caps = re
        .captures(&content)
        .ok_or(SkillInstallError::ManifestParse)?;

    let yaml = &caps["yaml"];
    let manifest: SkillManifest =
        serde_yaml::from_str(yaml).map_err(|_| SkillInstallError::ManifestParse)?;

    // Validate name: lowercase alnum and hyphens
    let name_re = Regex::new(r"^[a-z0-9-]+$").unwrap();
    if !name_re.is_match(&manifest.name) {
        return Err(SkillInstallError::Validation);
    }

    // Validate version: must be valid semver
    if semver::Version::parse(&manifest.version).is_err() {
        return Err(SkillInstallError::Validation);
    }

    Ok(manifest)
}
