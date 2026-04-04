use agentsync::skills::provider::{Provider, SkillsShProvider};
use agentsync::skills::registry;
use agentsync::skills::suggest::{SuggestInstallMode, SuggestionService};
use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use dialoguer::{MultiSelect, theme::ColorfulTheme};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::path::{Component, Path};
use tracing::error;

#[derive(Subcommand, Debug)]
pub enum SkillCommand {
    /// Install a skill from skills.sh or a custom provider
    Install(SkillInstallArgs),
    /// Update a skill to latest version
    Update(SkillUpdateArgs),
    /// Uninstall a skill
    Uninstall(SkillUninstallArgs),
    /// Suggest repo-aware skills without installing them
    Suggest(SkillSuggestArgs),
    /// List installed skills
    List,
}

/// Arguments for installing a skill
#[derive(Args, Debug)]
pub struct SkillInstallArgs {
    /// Skill id to install
    pub skill_id: String,
    /// Optional source (dir, archive, or URL)
    #[arg(long)]
    pub source: Option<String>,
    /// Output JSON instead of human-friendly
    #[arg(long)]
    pub json: bool,
}

/// Arguments for updating a skill
#[derive(Args, Debug)]
pub struct SkillUpdateArgs {
    /// Skill id to update
    pub skill_id: String,
    /// Optional source (dir, archive, or URL)
    #[arg(long)]
    pub source: Option<String>,
    /// Output JSON instead of human-friendly
    #[arg(long)]
    pub json: bool,
}

/// Arguments for uninstalling a skill
#[derive(Args, Debug)]
pub struct SkillUninstallArgs {
    /// Skill id to uninstall
    pub skill_id: String,
    /// Output JSON instead of human-friendly
    #[arg(long)]
    pub json: bool,
}

/// Arguments for suggesting skills
#[derive(Args, Debug)]
pub struct SkillSuggestArgs {
    /// Output JSON instead of human-friendly
    #[arg(long)]
    pub json: bool,
    /// Install recommended skills using a guided flow
    #[arg(long)]
    pub install: bool,
    /// Select all recommended skills instead of using the guided prompt
    #[arg(long, requires = "install")]
    pub all: bool,
}

pub fn run_update(args: SkillUpdateArgs, project_root: PathBuf) -> Result<()> {
    let target_root = project_root.join(".agents").join("skills");
    std::fs::create_dir_all(&target_root)?;

    let skill_id = &args.skill_id;
    // Validate skill_id to prevent path traversal or invalid path segments
    validate_skill_id(skill_id)?;

    let source = resolve_source(skill_id, args.source.clone())?;
    let update_source_path = std::path::Path::new(&source);
    let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.block_on(agentsync::skills::update::update_skill_async(
            skill_id,
            &target_root,
            update_source_path,
        ))
    } else {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(agentsync::skills::update::update_skill_async(
            skill_id,
            &target_root,
            update_source_path,
        ))
    };
    match result {
        Ok(()) => {
            if args.json {
                let registry_path = project_root
                    .join(".agents")
                    .join("skills")
                    .join("registry.json");
                let reg_res = registry::read_registry(&registry_path);
                let entry = reg_res
                    .as_ref()
                    .ok()
                    .and_then(|reg| reg.skills.as_ref())
                    .and_then(|s| s.get(skill_id));

                if let Some(skill) = entry {
                    let output = serde_json::json!({
                        "id": skill_id,
                        "name": skill.name,
                        "description": skill.description,
                        "version": skill.version,
                        "files": skill.files,
                        "manifest_hash": skill.manifest_hash,
                        "installed_at": skill.installed_at,
                        "status": "updated"
                    });
                    println!("{}", serde_json::to_string(&output)?);
                } else {
                    if let Err(ref e) = reg_res {
                        tracing::warn!(
                            ?e,
                            "Failed to read registry after update, falling back to minimal response"
                        );
                    }
                    let output = serde_json::json!({
                        "id": skill_id,
                        "status": "updated"
                    });
                    println!("{}", serde_json::to_string(&output)?);
                }
            } else {
                println!("Updated {}", skill_id);
            }
            Ok(())
        }
        Err(e) => {
            let err_string = e.to_string();
            let code = "update_error";
            let remediation = remediation_for_error(&err_string);

            if args.json {
                let output = serde_json::json!({
                    "error": err_string,
                    "code": code,
                    "remediation": remediation
                });
                println!("{}", serde_json::to_string(&output)?);
                Err(e.into())
            } else {
                error!(%code, %err_string, "Update failed");
                println!("Hint: {}", remediation);
                Err(e.into())
            }
        }
    }
}

pub fn run_skill(cmd: SkillCommand, project_root: PathBuf) -> Result<()> {
    match cmd {
        SkillCommand::Install(args) => run_install(args, project_root),
        SkillCommand::Update(args) => run_update(args, project_root),
        SkillCommand::Uninstall(args) => run_uninstall(args, project_root),
        SkillCommand::Suggest(args) => run_suggest(args, project_root),
        SkillCommand::List => {
            // Signal failure until List is implemented so CLI exits non-zero
            bail!("list command not implemented")
        }
    }
}

pub fn run_suggest(args: SkillSuggestArgs, project_root: PathBuf) -> Result<()> {
    let service = SuggestionService;
    let result = (|| -> Result<()> {
        let response = service.suggest(&project_root)?;

        if !args.install {
            if args.json {
                println!("{}", serde_json::to_string(&response.to_json_response())?);
            } else {
                println!("{}", response.render_human());
            }
            return Ok(());
        }

        let install_response = if args.all {
            let provider = SuggestInstallProvider::default();
            service.install_all_with(&project_root, &response, &provider)
        } else {
            ensure_interactive_install_supported()?;
            let selected_skill_ids = prompt_for_recommended_skills(&response)?;
            let provider = SuggestInstallProvider::default();
            service.install_selected_with(
                &project_root,
                &response,
                &provider,
                SuggestInstallMode::Interactive,
                &selected_skill_ids,
                |skill_id, source, target_root| {
                    agentsync::skills::install::blocking_fetch_and_install_skill(
                        skill_id,
                        source,
                        target_root,
                    )
                    .map_err(|error| anyhow::anyhow!(error))
                },
            )
        }?;

        if args.json {
            println!("{}", serde_json::to_string(&install_response)?);
        } else {
            println!("{}", install_response.render_human());
        }

        Ok(())
    })();

    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            let error_message = error.to_string();
            let (code, remediation) = if error_message
                .contains("not part of the current recommendation set")
            {
                (
                    "invalid_suggestion_selection",
                    "Run 'agentsync skill suggest --json' to inspect available recommended skill ids.",
                )
            } else if error_message.contains("interactive terminal") {
                (
                    "interactive_tty_required",
                    "Run 'agentsync skill suggest --install --all' for a non-interactive install path.",
                )
            } else if args.install {
                ("install_error", remediation_for_error(&error_message))
            } else {
                (
                    "suggest_error",
                    "Verify the project root is readable and try again. Use --project-root to point to the repository you want to inspect.",
                )
            };

            if args.json {
                let output = serde_json::json!({
                    "error": error_message,
                    "code": code,
                    "remediation": remediation,
                });
                println!("{}", serde_json::to_string(&output)?);
            } else {
                error!(%code, error = %error_message, "Suggest failed");
                println!("Hint: {}", remediation);
            }

            Err(error)
        }
    }
}

fn ensure_interactive_install_supported() -> Result<()> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        return Ok(());
    }

    Err(anyhow::anyhow!(
        "guided recommendation install requires an interactive terminal"
    ))
}

fn prompt_for_recommended_skills(
    response: &agentsync::skills::suggest::SuggestResponse,
) -> Result<Vec<String>> {
    let installable = response.installable_recommendations();
    if installable.is_empty() {
        return Ok(Vec::new());
    }

    let items = installable
        .iter()
        .map(|recommendation| format!("{} — {}", recommendation.skill_id, recommendation.summary))
        .collect::<Vec<_>>();
    let defaults = vec![true; items.len()];
    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select recommended skills to install")
        .items(&items)
        .defaults(&defaults)
        .interact()?;

    Ok(selections
        .into_iter()
        .map(|index| installable[index].skill_id.clone())
        .collect())
}

pub fn run_install(args: SkillInstallArgs, project_root: PathBuf) -> Result<()> {
    let target_root = project_root.join(".agents").join("skills");
    std::fs::create_dir_all(&target_root)?;

    // For this example we'll accept local ZIP/path as "skill_id" (for test and demo), real impl will use resolver
    // Accept: skill.zip / skill.tar.gz (local file path or URL); fallback: skill_id as path
    let skill_id = &args.skill_id;
    // Validate skill_id to prevent path traversal or invalid path segments
    validate_skill_id(skill_id)?;

    let source = resolve_source(skill_id, args.source.clone())?;

    // Unified logic: install from archive, URL, or local directory
    tracing::debug!(
        skill_id = %skill_id,
        source = %source,
        target_root = %target_root.display(),
        "install"
    );
    let result = agentsync::skills::install::blocking_fetch_and_install_skill(
        skill_id,
        &source,
        &target_root,
    );
    match result {
        Ok(()) => {
            if args.json {
                let registry_path = project_root
                    .join(".agents")
                    .join("skills")
                    .join("registry.json");
                let reg_res = registry::read_registry(&registry_path);
                let entry = reg_res
                    .as_ref()
                    .ok()
                    .and_then(|reg| reg.skills.as_ref())
                    .and_then(|s| s.get(&args.skill_id));

                if let Some(skill) = entry {
                    let output = serde_json::json!({
                        "id": &args.skill_id,
                        "name": skill.name,
                        "description": skill.description,
                        "version": skill.version,
                        "files": skill.files,
                        "manifest_hash": skill.manifest_hash,
                        "installed_at": skill.installed_at,
                        "status": "installed"
                    });
                    println!("{}", serde_json::to_string(&output)?);
                } else {
                    if let Err(ref e) = reg_res {
                        tracing::warn!(
                            ?e,
                            "Failed to read registry after install, falling back to minimal response"
                        );
                    }
                    let output = serde_json::json!({
                        "id": &args.skill_id,
                        "status": "installed"
                    });
                    println!("{}", serde_json::to_string(&output)?);
                }
            } else {
                println!("Installed {}", args.skill_id);
            }
            Ok(())
        }
        Err(e) => {
            let e: anyhow::Error = e.into();
            // Try to downcast to SkillInstallError to extract code/remediation
            let (err_string, code, remediation);
            err_string = e.to_string();
            code = "install_error";
            remediation = remediation_for_error(&err_string);

            if args.json {
                let output = serde_json::json!({
                    "error": err_string,
                    "code": code,
                    "remediation": remediation
                });
                println!("{}", serde_json::to_string(&output)?);
                Err(e)
            } else {
                error!(%code, %err_string, "Install failed");
                println!("Hint: {}", remediation);
                Err(e)
            }
        }
    }
}

struct SuggestInstallProvider {
    fallback: SkillsShProvider,
}

impl Default for SuggestInstallProvider {
    fn default() -> Self {
        Self {
            fallback: SkillsShProvider,
        }
    }
}

impl Provider for SuggestInstallProvider {
    fn manifest(&self) -> Result<String> {
        self.fallback.manifest()
    }

    fn resolve(&self, id: &str) -> Result<agentsync::skills::provider::SkillInstallInfo> {
        if let Ok(source_root) = std::env::var("AGENTSYNC_TEST_SKILL_SOURCE_DIR") {
            // or a simple local name. Use the full ID to find the local source directory.
            let source_path = PathBuf::from(source_root).join(id);
            if source_path.exists() {
                return Ok(agentsync::skills::provider::SkillInstallInfo {
                    download_url: source_path.display().to_string(),
                    format: "dir".to_string(),
                });
            }
        }

        self.fallback.resolve(id)
    }

    fn recommendation_catalog(
        &self,
    ) -> Result<Option<agentsync::skills::provider::ProviderCatalogMetadata>> {
        self.fallback.recommendation_catalog()
    }
}

pub fn run_uninstall(args: SkillUninstallArgs, project_root: PathBuf) -> Result<()> {
    let target_root = project_root.join(".agents").join("skills");

    let skill_id = &args.skill_id;

    // Validate skill_id to prevent path traversal or invalid path segments
    validate_skill_id(skill_id)?;

    let result = agentsync::skills::uninstall::uninstall_skill(skill_id, &target_root);

    match result {
        Ok(()) => {
            if args.json {
                let output = serde_json::json!({
                    "id": skill_id,
                    "status": "uninstalled"
                });
                println!("{}", serde_json::to_string(&output)?);
            } else {
                println!("Uninstalled {}", skill_id);
            }
            Ok(())
        }
        Err(e) => {
            let err_string = e.to_string();
            let (code, remediation) = match e {
                agentsync::skills::uninstall::SkillUninstallError::NotFound(_) => {
                    ("skill_not_found", "Try 'list' to verify installed skills")
                }
                agentsync::skills::uninstall::SkillUninstallError::Validation(_) => (
                    "validation_error",
                    "Ensure the skill ID is valid (no special characters, not '.' or '..')",
                ),
                _ => (
                    "uninstall_error",
                    "Ensure you have proper permissions to remove files",
                ),
            };

            if args.json {
                let output = serde_json::json!({
                    "error": err_string,
                    "code": code,
                    "remediation": remediation
                });
                println!("{}", serde_json::to_string(&output)?);
                Err(anyhow::anyhow!(e))
            } else {
                error!(%code, %err_string, "Uninstall failed");
                match e {
                    agentsync::skills::uninstall::SkillUninstallError::NotFound(_) => {
                        println!(
                            "Hint: Skill '{}' is not installed. Use 'list' to see installed skills.",
                            skill_id
                        );
                    }
                    _ => {
                        println!("Hint: Ensure you have proper permissions to remove files.");
                    }
                }
                Err(anyhow::anyhow!(e))
            }
        }
    }
}

fn resolve_source(skill_id: &str, source_arg: Option<String>) -> Result<String> {
    if let Some(s) = source_arg {
        // Check if it's a GitHub URL that needs conversion to ZIP format
        if let Some(github_url) = try_convert_github_url(&s) {
            tracing::info!(original = %s, converted = %github_url, "Converted GitHub URL to ZIP format");
            return Ok(github_url);
        }
        return Ok(s);
    }

    // If it doesn't look like a URL or a path, try to resolve via skills.sh
    if !skill_id.contains("://") && !skill_id.starts_with('/') && !skill_id.starts_with('.') {
        let provider = SkillsShProvider;
        match provider.resolve(skill_id) {
            Ok(info) => Ok(info.download_url),
            Err(e) => {
                tracing::warn!(%skill_id, ?e, "Failed to resolve skill via skills.sh");
                Err(anyhow::anyhow!(
                    "failed to resolve skill '{}' via skills.sh: {}",
                    skill_id,
                    e
                ))
            }
        }
    } else {
        Ok(skill_id.to_string())
    }
}

/// Attempts to convert a GitHub URL to a downloadable ZIP URL.
///
/// Supports the following GitHub URL formats:
/// - `https://github.com/owner/repo` → `https://github.com/owner/repo/archive/HEAD.zip`
/// - `https://github.com/owner/repo/tree/branch/path` → `https://github.com/owner/repo/archive/refs/heads/branch.zip#path`
/// - `https://github.com/owner/repo/blob/branch/path/file` → `https://github.com/owner/repo/archive/refs/heads/branch.zip#path`
///
/// **Limitation:** Branch names containing slashes (e.g., `feature/auth`) cannot be reliably
/// distinguished from subpaths without accessing the GitHub API. In such cases, the function
/// assumes the first segment after `tree/` or `blob/` is the branch name. For branches with
/// slashes, the resulting URL may be incorrect. If API access becomes available in the future,
/// this function could use the GitHub refs API to resolve the correct branch name via
/// longest-prefix matching.
///
/// Returns `None` if the URL is not a GitHub URL or already points to an archive.
fn try_convert_github_url(url: &str) -> Option<String> {
    // Parse the URL to properly handle query strings and fragments
    let parsed = url::Url::parse(url).ok()?;

    // Check if it's already an archive URL by examining the path component
    let path = parsed.path();
    if path.ends_with(".zip") || path.ends_with(".tar.gz") || path.ends_with(".tgz") {
        return None;
    }

    // Only process github.com URLs
    if parsed.host_str() != Some("github.com") {
        return None;
    }

    // Get the path segments
    let segments: Vec<&str> = parsed
        .path_segments()
        .map(|s| s.collect())
        .unwrap_or_default();

    // Minimum: owner/repo (at least 2 segments)
    if segments.len() < 2 {
        return None;
    }

    let owner = segments[0];
    let repo = segments[1];

    // Check if it's a tree or blob URL with subpath
    if segments.len() >= 4 && (segments[2] == "tree" || segments[2] == "blob") {
        let branch = segments[3];
        // The rest is the path within the repo
        let subpath = segments[4..].join("/");

        // If it's a blob URL pointing to a file, get the parent directory
        let final_subpath = if segments[2] == "blob" {
            if subpath.contains('/') {
                // Remove the filename to get the directory
                let path_parts: Vec<&str> = subpath.split('/').collect();
                if path_parts.len() > 1 {
                    path_parts[..path_parts.len() - 1].join("/")
                } else {
                    subpath
                }
            } else {
                // Blob pointing to a file at repo root (e.g., README.md)
                // Return empty string so no fragment is added
                String::new()
            }
        } else {
            subpath
        };

        let mut zip_url = format!(
            "https://github.com/{}/{}/archive/refs/heads/{}.zip",
            owner, repo, branch
        );

        if !final_subpath.is_empty() {
            zip_url.push('#');
            zip_url.push_str(&final_subpath);
        }

        return Some(zip_url);
    }

    // Simple repo URL: github.com/owner/repo
    if segments.len() == 2 {
        return Some(format!(
            "https://github.com/{}/{}/archive/HEAD.zip",
            owner, repo
        ));
    }

    // Repo URL with trailing segments but not tree/blob
    // e.g., github.com/owner/repo/ (with trailing slash)
    if segments.len() > 2 && segments[2].is_empty() {
        return Some(format!(
            "https://github.com/{}/{}/archive/HEAD.zip",
            owner, repo
        ));
    }

    None
}

fn remediation_for_error(msg: &str) -> &str {
    if msg.contains("manifest") {
        "Check the SKILL.md syntax, frontmatter, and ensure the 'name' field matches requirements. See agentsync docs/spec for manifest schema."
    } else if msg.contains("network") || msg.contains("download") || msg.contains("HTTP") {
        "Check your network connection and ensure the skill source URL is correct."
    } else if msg.contains("archive") {
        "Verify the skill archive is a valid zip or tar.gz. Try re-downloading or using a fresh archive."
    } else if msg.contains("permission") {
        "Check your file permissions or try running as administrator/root."
    } else if msg.contains("registry") {
        "There was a problem updating the registry. Ensure you have write access and the registry file isn't corrupted."
    } else {
        "See above error message. If unsure, run with increased verbosity or check the documentation."
    }
}

fn validate_skill_id(skill_id: &str) -> Result<()> {
    if skill_id.is_empty() {
        return Err(anyhow::anyhow!("skill id must not be empty"));
    }

    // Quick reject any obvious separators
    if skill_id.contains('/') || skill_id.contains('\\') {
        return Err(anyhow::anyhow!(
            "invalid skill id: must be a single path segment without '/' or '\\'"
        ));
    }

    let p = Path::new(skill_id);

    // Reject absolute paths or paths that start with a prefix/root (drive letter on Windows)
    if p.is_absolute() {
        return Err(anyhow::anyhow!(
            "invalid skill id: must not be an absolute path"
        ));
    }

    // Ensure components() yields exactly one Component::Normal
    let mut count_normal = 0usize;
    for comp in p.components() {
        match comp {
            Component::Normal(_) => count_normal += 1,
            // Any other component is invalid (RootDir, Prefix, CurDir, ParentDir)
            other => {
                return Err(anyhow::anyhow!(
                    "invalid skill id: contains invalid path component: {:?}",
                    other
                ));
            }
        }
    }

    if count_normal != 1 {
        return Err(anyhow::anyhow!(
            "invalid skill id: must be a single path segment"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    // PathBuf no longer used in tests

    #[test]
    fn validate_skill_id_accepts_simple_names() {
        assert!(validate_skill_id("weather-skill").is_ok());
        assert!(validate_skill_id("hello").is_ok());
        assert!(validate_skill_id("a").is_ok());
        assert!(validate_skill_id("skill_123").is_ok());
    }

    #[test]
    fn validate_skill_id_rejects_invalid_inputs() {
        // empty
        assert!(validate_skill_id("").is_err());

        // separators
        assert!(validate_skill_id("foo/bar").is_err());
        assert!(validate_skill_id("foo\\bar").is_err());

        // relative/cur/parent
        assert!(validate_skill_id(".").is_err());
        assert!(validate_skill_id("..").is_err());

        // absolute path (unix)
        assert!(validate_skill_id("/abs/path").is_err());

        // absolute path (windows)
        assert!(validate_skill_id("C:\\path").is_err());
        assert!(validate_skill_id("C:/path").is_err());
    }

    #[test]
    fn run_skill_list_returns_error() {
        let project_root = std::env::temp_dir();
        let res = run_skill(SkillCommand::List, project_root);
        assert!(res.is_err(), "list should return error until implemented");
    }

    #[test]
    fn github_url_converter_simple_repo() {
        let result = try_convert_github_url("https://github.com/obra/superpowers");
        assert_eq!(
            result,
            Some("https://github.com/obra/superpowers/archive/HEAD.zip".to_string())
        );
    }

    #[test]
    fn github_url_converter_tree_with_subpath() {
        let result = try_convert_github_url(
            "https://github.com/obra/superpowers/tree/main/skills/systematic-debugging",
        );
        assert_eq!(
            result,
            Some("https://github.com/obra/superpowers/archive/refs/heads/main.zip#skills/systematic-debugging".to_string())
        );
    }

    #[test]
    fn github_url_converter_blob_extracts_directory() {
        // Blob URLs should extract the parent directory
        let result = try_convert_github_url(
            "https://github.com/obra/superpowers/blob/main/skills/systematic-debugging/SKILL.md",
        );
        assert_eq!(
            result,
            Some("https://github.com/obra/superpowers/archive/refs/heads/main.zip#skills/systematic-debugging".to_string())
        );
    }

    #[test]
    fn github_url_converter_already_zip_returns_none() {
        // Already a ZIP URL should return None
        let result = try_convert_github_url(
            "https://github.com/obra/superpowers/archive/refs/heads/main.zip",
        );
        assert_eq!(result, None);
    }

    #[test]
    fn github_url_converter_tar_gz_returns_none() {
        // Already a tar.gz URL should return None
        let result = try_convert_github_url("https://example.com/archive.tar.gz");
        assert_eq!(result, None);
    }

    #[test]
    fn github_url_converter_non_github_returns_none() {
        // Non-GitHub URLs should return None
        let result = try_convert_github_url("https://gitlab.com/user/repo");
        assert_eq!(result, None);
    }

    #[test]
    fn github_url_converter_tree_deeply_nested() {
        // Deeply nested paths
        let result = try_convert_github_url(
            "https://github.com/owner/repo/tree/develop/skills/category/my-skill",
        );
        assert_eq!(
            result,
            Some("https://github.com/owner/repo/archive/refs/heads/develop.zip#skills/category/my-skill".to_string())
        );
    }

    #[test]
    fn github_url_converter_blob_root_file() {
        // Blob URL pointing to a file at repository root should return ZIP URL without fragment
        let result = try_convert_github_url("https://github.com/owner/repo/blob/main/README.md");
        assert_eq!(
            result,
            Some("https://github.com/owner/repo/archive/refs/heads/main.zip".to_string())
        );
    }

    #[test]
    fn github_url_converter_zip_with_query_string() {
        // ZIP URL with query string should return None (already an archive)
        let result =
            try_convert_github_url("https://github.com/owner/repo/archive/HEAD.zip?token=abc123");
        assert_eq!(result, None);
    }

    #[test]
    fn github_url_converter_zip_with_fragment() {
        // ZIP URL with fragment should return None (already an archive)
        let result =
            try_convert_github_url("https://github.com/owner/repo/archive/HEAD.zip#subpath");
        assert_eq!(result, None);
    }
}
