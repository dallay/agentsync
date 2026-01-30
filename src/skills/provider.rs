use anyhow::Result;

/// Provider trait for resolving skills
pub trait Provider {
    fn manifest(&self) -> Result<String>;
    fn resolve(&self, id: &str) -> Result<SkillInstallInfo>;
}

#[derive(Debug)]
pub struct SkillInstallInfo {
    pub download_url: String,
    pub format: String,
}
