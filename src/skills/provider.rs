use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::skills::catalog::EmbeddedSkillCatalog;

/// Provider trait for resolving skills
pub trait Provider {
    fn manifest(&self) -> Result<String>;
    fn resolve(&self, id: &str) -> Result<SkillInstallInfo>;

    fn recommendation_catalog(&self) -> Result<Option<ProviderCatalogMetadata>> {
        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct SkillInstallInfo {
    pub download_url: String,
    pub format: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderCatalogMetadata {
    pub provider: String,
    pub version: String,
    #[serde(default = "default_catalog_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub skills: Vec<ProviderCatalogSkill>,
    #[serde(default)]
    pub technologies: Vec<ProviderCatalogTechnology>,
    #[serde(default)]
    pub combos: Vec<ProviderCatalogCombo>,
}

fn default_catalog_schema_version() -> String {
    "v1".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderCatalogSkill {
    pub provider_skill_id: String,
    pub local_skill_id: String,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub archive_subpath: Option<String>,
    #[serde(default)]
    pub legacy_local_skill_ids: Vec<String>,
    #[serde(default)]
    pub install_source: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderCatalogTechnology {
    pub id: String,
    pub name: String,
    pub skills: Vec<String>,
    #[serde(default)]
    pub detect: Option<crate::skills::detect::DetectionRules>,
    #[serde(default)]
    pub min_confidence: Option<String>,
    #[serde(default)]
    pub reason_template: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderCatalogCombo {
    pub id: String,
    pub name: String,
    pub requires: Vec<String>,
    pub skills: Vec<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub reason_template: Option<String>,
}

#[derive(Deserialize, Debug)]
struct SearchResponse {
    skills: Vec<SearchSkill>,
}

#[derive(Deserialize, Debug)]
struct SearchSkill {
    id: String,
    source: String,
}

pub struct SkillsShProvider;

pub const DALLAY_AGENTS_SKILLS_PREFIX: &str = "dallay/agents-skills/";

/// Well-known repo names where skills live in a `skills/` subdirectory.
const SKILLS_REPO_NAMES: &[&str] = &["skills", "agent-skills", "agentic-skills", "agents-skills"];

fn repo_uses_skills_subdirectory(repo: &str) -> bool {
    SKILLS_REPO_NAMES.contains(&repo)
        || repo.ends_with("-skills")
        || repo.ends_with("-agent-skills")
        || repo.ends_with("-agentic-skills")
        || repo.ends_with("-agents-skills")
}

fn local_catalog_skill_source_dir(
    provider_skill_id: &str,
    local_skill_id: &str,
    project_root: Option<&Path>,
) -> Option<PathBuf> {
    // Try AGENTSYNC_TEST_SKILL_SOURCE_DIR with provider_skill_id (full path like "dallay/agents-skills/rust-async-patterns")
    if let Ok(path) = std::env::var("AGENTSYNC_TEST_SKILL_SOURCE_DIR") {
        // First try the full provider_skill_id path (e.g., "dallay/agents-skills/rust-async-patterns")
        let candidate = PathBuf::from(path.clone()).join(provider_skill_id);
        if candidate.exists() {
            return Some(candidate);
        }
        // Fall back to local_skill_id only (e.g., "rust-async-patterns")
        let fallback = PathBuf::from(path).join(local_skill_id);
        if fallback.exists() {
            return Some(fallback);
        }
    }

    // Try AGENTSYNC_LOCAL_SKILLS_REPO/skills/local_skill_id
    if let Ok(path) = std::env::var("AGENTSYNC_LOCAL_SKILLS_REPO") {
        return Some(PathBuf::from(path).join("skills").join(local_skill_id));
    }

    // Try project_parent/agents-skills/skills/local_skill_id
    project_root
        .and_then(Path::parent)
        .map(|parent| {
            parent
                .join("agents-skills")
                .join("skills")
                .join(local_skill_id)
        })
        .filter(|path| path.exists())
}

pub fn resolve_catalog_install_source(
    catalog: &EmbeddedSkillCatalog,
    provider: &dyn Provider,
    provider_skill_id: &str,
    local_skill_id: &str,
    project_root: Option<&Path>,
) -> Result<String> {
    if let Some(path) =
        local_catalog_skill_source_dir(provider_skill_id, local_skill_id, project_root)
    {
        return Ok(path.to_string_lossy().into_owned());
    }

    if let Some(install_source) = catalog.get_install_source(provider_skill_id) {
        return Ok(install_source.to_string());
    }

    Ok(provider.resolve(provider_skill_id)?.download_url)
}

impl SkillsShProvider {
    /// Resolve a catalog-style `owner/repo/skill-name` ID deterministically by
    /// constructing the GitHub download URL directly — no network call needed.
    fn resolve_deterministic(&self, id: &str) -> Result<SkillInstallInfo> {
        // Split into owner/repo and skill-name at the second '/' separator.
        let first_slash = id
            .find('/')
            .ok_or_else(|| anyhow::anyhow!("invalid skill id (missing owner): {}", id))?;
        let rest = &id[first_slash + 1..];
        let second_slash = rest
            .find('/')
            .ok_or_else(|| anyhow::anyhow!("invalid skill id (missing repo): {}", id))?;

        let owner = &id[..first_slash];
        let repo = &rest[..second_slash];
        let skill_name = &rest[second_slash + 1..];

        if owner.is_empty() || repo.is_empty() || skill_name.is_empty() {
            anyhow::bail!("invalid skill id (empty component): {}", id);
        }

        let embedded_catalog = EmbeddedSkillCatalog::default();
        let subpath = embedded_catalog
            .get_archive_subpath(id)
            .map(str::to_string)
            .unwrap_or_else(|| {
                if repo_uses_skills_subdirectory(repo) {
                    format!("skills/{skill_name}")
                } else {
                    skill_name.to_string()
                }
            });

        let mut final_url = format!("https://github.com/{owner}/{repo}/archive/HEAD.zip");
        if !subpath.is_empty() {
            final_url.push('#');
            final_url.push_str(&subpath);
        }

        Ok(SkillInstallInfo {
            download_url: final_url,
            format: "zip".to_string(),
        })
    }

    /// Resolve a simple skill ID (e.g., "rust-async-patterns") by searching the
    /// skills.sh API. This is the original behavior for non-catalog IDs.
    fn resolve_via_search(&self, id: &str) -> Result<SkillInstallInfo> {
        let url = format!("https://skills.sh/api/search?q={}", urlencoding::encode(id));

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        let resp = client.get(url).send()?.json::<SearchResponse>()?;

        // Find the best match (exact id match preferred)
        let skill = resp
            .skills
            .iter()
            .find(|s| s.id == id || s.id.split('/').next_back() == Some(id))
            .ok_or_else(|| anyhow::anyhow!("Skill not found on skills.sh: {}", id))?;

        // Construct GitHub zip URL — source is "owner/repo"
        let download_url = format!("https://github.com/{}", skill.source);

        // Robust subpath detection
        let subpath = if skill.id.starts_with(&skill.source) {
            let sub = &skill.id[skill.source.len()..];
            let sub = sub.trim_start_matches('/');
            if !sub.is_empty() {
                sub.to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // If the repo name is a well-known skills repo, prefix 'skills/'
        let final_subpath = if !subpath.is_empty() && !subpath.starts_with("skills/") {
            let repo_name = skill.source.split('/').next_back().unwrap_or("");
            if repo_uses_skills_subdirectory(repo_name) {
                format!("skills/{}", subpath)
            } else {
                subpath
            }
        } else {
            subpath
        };

        let mut final_url = format!("{}/archive/HEAD.zip", download_url);
        if !final_subpath.is_empty() {
            final_url.push('#');
            final_url.push_str(&final_subpath);
        }

        Ok(SkillInstallInfo {
            download_url: final_url,
            format: "zip".to_string(),
        })
    }
}

impl Provider for SkillsShProvider {
    fn manifest(&self) -> Result<String> {
        Ok("skills.sh".to_string())
    }

    fn resolve(&self, id: &str) -> Result<SkillInstallInfo> {
        // Deterministic path for catalog-style IDs with owner/repo/skill-name format.
        // If the ID contains at least 2 '/' separators, we can construct the download
        // URL directly without a network call to the skills.sh search API.
        let slash_count = id.chars().filter(|&c| c == '/').count();
        if slash_count >= 2 {
            return self.resolve_deterministic(id);
        }

        // Fallback: use skills.sh search API for simple IDs (e.g., "rust-async-patterns")
        self.resolve_via_search(id)
    }
}
