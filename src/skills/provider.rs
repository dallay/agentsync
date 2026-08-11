use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::skills::catalog::EmbeddedSkillCatalog;
use crate::skills::registry::{RegistryDocument, RegistryEntry};

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
    #[serde(default)]
    pub registry_entry_id: Option<String>,
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

/// Provider for curated entries. Local content is authoritative; remote fallback is opt-in and
/// always points at the immutable commit recorded by the registry.
pub struct PinnedProvider<'a> {
    registry: RegistryDocument,
    local_root: &'a Path,
    allow_remote_fallback: bool,
}

impl<'a> PinnedProvider<'a> {
    pub fn new(registry: RegistryDocument, local_root: &'a Path) -> Self {
        Self {
            registry,
            local_root,
            allow_remote_fallback: false,
        }
    }

    pub fn without_remote_fallback(mut self) -> Self {
        self.allow_remote_fallback = false;
        self
    }

    pub fn allow_remote_fallback(mut self) -> Self {
        self.allow_remote_fallback = true;
        self
    }

    fn entry(&self, id: &str) -> Result<&RegistryEntry> {
        let matches: Vec<_> = self
            .registry
            .entries
            .values()
            .filter(|entry| entry.provider_skill_id == id || entry.local_skill_id == id)
            .collect();
        match matches.as_slice() {
            [] => anyhow::bail!("curated skill is not registered: {id}"),
            [entry] => Ok(entry),
            _ => anyhow::bail!("ambiguous curated skill identifier: {id}"),
        }
    }
}

impl Provider for PinnedProvider<'_> {
    fn manifest(&self) -> Result<String> {
        Ok("curated-registry".to_string())
    }

    fn resolve(&self, id: &str) -> Result<SkillInstallInfo> {
        let entry = self.entry(id)?;
        let local = self.local_root.join(&entry.local_skill_id);
        if local.is_dir() {
            return Ok(SkillInstallInfo {
                download_url: local.display().to_string(),
                format: "dir".to_string(),
            });
        }
        if !self.allow_remote_fallback {
            anyhow::bail!(
                "curated skill `{id}` unavailable offline: no local content and remote fallback is disabled"
            );
        }
        let repository = entry
            .source
            .repository
            .trim_end_matches('/')
            .trim_end_matches(".git");
        let archive = format!("{repository}/archive/{}.zip", entry.source.commit);
        Ok(SkillInstallInfo {
            download_url: format!("{archive}#{}", entry.source.subpath),
            format: "zip".to_string(),
        })
    }
}

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
        return local_skills_repo_source_dir(Path::new(&path), local_skill_id);
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

fn local_skills_repo_source_dir(repo_root: &Path, local_skill_id: &str) -> Option<PathBuf> {
    let candidate = repo_root.join("skills").join(local_skill_id);
    candidate.exists().then_some(candidate)
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
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => handle.block_on(resolve_via_search_http(
                &url,
                std::time::Duration::from_secs(10),
                id,
            )),
            Err(_) => {
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| anyhow::anyhow!("failed to create runtime: {}", e))?;
                rt.block_on(resolve_via_search_http(
                    &url,
                    std::time::Duration::from_secs(10),
                    id,
                ))
            }
        }
    }
}

/// Async HTTP fetch for skills.sh search. Returns a Result with context-bearing errors.
async fn resolve_via_search_http(
    url: &str,
    timeout: Duration,
    _id: &str,
) -> Result<SkillInstallInfo> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .with_context(|| format!("skills.sh search failed for url={}", url))?;
    let resp: SearchResponse = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("skills.sh search failed for url={}", url))?
        .error_for_status()
        .with_context(|| format!("skills.sh search failed for url={}", url))?
        .json()
        .await
        .with_context(|| format!("skills.sh search failed for url={}", url))?;

    // Find the best match (exact id match preferred)
    // Note: id parameter reserved for future exact-match scoring
    let skill = resp
        .skills
        .iter()
        .find(|s| s.id == _id || s.id.split('/').next_back() == Some(_id))
        .ok_or_else(|| anyhow::anyhow!("Skill not found on skills.sh: {}", _id))?;

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

    fn recommendation_catalog(&self) -> Result<Option<ProviderCatalogMetadata>> {
        // Deterministic subprocess hook for the CLI contract test that verifies
        // catalog fallback warnings stay on stderr. It is inert unless explicitly
        // enabled by a test environment.
        #[cfg(debug_assertions)]
        if std::env::var_os("AGENTSYNC_TEST_INVALID_RECOMMENDATION_CATALOG").is_some() {
            anyhow::bail!("invalid recommendation catalog test fixture")
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::local_skills_repo_source_dir;
    use super::resolve_via_search_http;
    use std::fs;
    use tempfile::TempDir;

    /// Start a TCP server that delays its response.
    async fn spawn_delayed_server(delay_secs: u64) -> std::net::SocketAddr {
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let _handle = std::thread::spawn(move || {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind to succeed");
            let addr = listener.local_addr().expect("local_addr");
            let _ = ready_tx.send(addr); // oneshot send cannot block
            let (mut conn, _) = listener.accept().expect("accept");
            std::thread::sleep(std::time::Duration::from_secs(delay_secs));
            use std::io::{Read, Write};
            let _ = conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello");
            let _ = conn.flush();
            let mut dummy = [0u8; 256];
            let _ = conn.read(&mut dummy);
        });
        ready_rx.await.expect("addr received")
    }

    /// Start a TCP server that returns non-JSON.
    async fn spawn_non_json_server() -> std::net::SocketAddr {
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let _handle = std::thread::spawn(move || {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind to succeed");
            let addr = listener.local_addr().expect("local_addr");
            let _ = ready_tx.send(addr); // oneshot send cannot block
            let (mut conn, _) = listener.accept().expect("accept");
            use std::io::{Read, Write};
            let mut dummy = [0u8; 512];
            let _ = conn.read(&mut dummy);
            let _ = conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nnot json!!!");
            let _ = conn.flush();
        });
        ready_rx.await.expect("addr received")
    }

    #[tokio::test]
    async fn test_resolve_via_search_timeout() {
        let addr = spawn_delayed_server(10).await;
        let url = format!("http://{}/api/search", addr);

        // Test the async version directly with a short timeout
        let result =
            resolve_via_search_http(&url, std::time::Duration::from_millis(50), "test-skill").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("timed out")
                || err.to_string().contains("timeout")
                || err.to_string().contains("skills.sh search failed"),
            "expected timeout or context error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_resolve_via_search_invalid_response() {
        let addr = spawn_non_json_server().await;
        let url = format!("http://{}/api/search", addr);

        let result =
            resolve_via_search_http(&url, std::time::Duration::from_secs(5), "test-skill").await;
        assert!(result.is_err());
        // Should be a parse/format error since the response isn't valid JSON
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("skills.sh search failed"),
            "expected context-bearing error about skills.sh search, got: {}",
            err
        );
    }

    /// Start a TCP server that returns an HTTP 500 with a non-JSON body.
    async fn spawn_error_server() -> std::net::SocketAddr {
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let _handle = std::thread::spawn(move || {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind to succeed");
            let addr = listener.local_addr().expect("local_addr");
            let _ = ready_tx.send(addr); // oneshot send cannot block
            let (mut conn, _) = listener.accept().expect("accept");
            use std::io::{Read, Write};
            let mut dummy = [0u8; 512];
            let _ = conn.read(&mut dummy);
            let _ = conn.write_all(
                b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 12\r\n\r\nserver boom!",
            );
            let _ = conn.flush();
        });
        ready_rx.await.expect("addr received")
    }

    #[tokio::test]
    async fn test_resolve_via_search_http_error_status() {
        let addr = spawn_error_server().await;
        let url = format!("http://{}/api/search", addr);

        // A 5xx with a non-JSON body must surface as an HTTP status error
        // (via error_for_status), not as a JSON parse or not-found error.
        let result =
            resolve_via_search_http(&url, std::time::Duration::from_secs(5), "test-skill").await;
        assert!(result.is_err());
        // Use the alternate Display ({:#}) to include the full anyhow cause chain.
        let err = format!("{:#}", result.unwrap_err());
        assert!(
            err.contains("status") || err.contains("500"),
            "expected HTTP status error, got: {}",
            err
        );
        assert!(
            !err.contains("failed to parse") && !err.contains("Skill not found"),
            "must not report a JSON parse or not-found error for HTTP 500, got: {}",
            err
        );
    }

    #[test]
    fn ignores_missing_skill_in_local_skills_repository() {
        let repo = TempDir::new().expect("temporary repository should be created");
        fs::create_dir_all(repo.path().join("skills")).expect("skills directory should be created");
        assert_eq!(
            local_skills_repo_source_dir(repo.path(), "missing-skill"),
            None
        );
    }
}
