use anyhow::Result;
use serde::Deserialize;

/// Provider trait for resolving skills
pub trait Provider {
    fn manifest(&self) -> Result<String>;
    fn resolve(&self, id: &str) -> Result<SkillInstallInfo>;
}

#[derive(Debug, Clone)]
pub struct SkillInstallInfo {
    pub download_url: String,
    pub format: String,
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

impl Provider for SkillsShProvider {
    fn manifest(&self) -> Result<String> {
        Ok("skills.sh".to_string())
    }

    fn resolve(&self, id: &str) -> Result<SkillInstallInfo> {
        // Use tokio runtime to call API
        let url = format!("https://skills.sh/api/search?q={}", id);

        let client = reqwest::blocking::Client::new();
        let resp = client.get(url).send()?.json::<SearchResponse>()?;

        // Find the best match (exact id match preferred)
        let skill = resp
            .skills
            .iter()
            .find(|s| s.id == id || s.id.split('/').next_back() == Some(id))
            .ok_or_else(|| anyhow::anyhow!("Skill not found on skills.sh: {}", id))?;

        // Construct GitHub zip URL
        // source is "owner/repo"
        let download_url = format!("https://github.com/{}", skill.source);

        // Robust subpath detection
        // For antfu/skills/vitest with source antfu/skills, subpath is skills/vitest
        // For astrolicious/agent-skills/astro with source astrolicious/agent-skills, subpath is skills/astro
        let subpath = if skill.id.starts_with(&skill.source) {
            let sub = &skill.id[skill.source.len()..];
            let sub = sub.trim_start_matches('/');

            // Heuristic: Many repos have a 'skills/' prefix that might be omitted in the search ID
            // but is present in the repo structure. Or the repo name itself IS 'skills'.
            if !sub.is_empty() {
                // We'll pass the subpath as is, but our unpacker will be smart
                sub.to_string()
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        };

        // If the repo name is 'skills' or 'agent-skills', we might need to prefix 'skills/'
        let final_subpath = if !subpath.is_empty() && !subpath.starts_with("skills/") {
            let repo_name = skill.source.split('/').next_back().unwrap_or("");
            if repo_name == "skills" || repo_name == "agent-skills" || repo_name == "agentic-skills"
            {
                format!("skills/{}", subpath)
            } else {
                subpath
            }
        } else {
            subpath
        };

        let mut final_url = format!("{}/archive/refs/heads/main.zip", download_url);
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
