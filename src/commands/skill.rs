use agentsync::skills::provider::{Provider, SkillsShProvider};
use agentsync::skills::registry;
use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use std::path::PathBuf;
use std::path::{Component, Path};
use tracing::error;

#[derive(Subcommand, Debug)]
pub enum SkillCommand {
    /// Install a skill from skills.sh or a custom provider
    Install(SkillInstallArgs),
    /// Update a skill to latest version
    Update(SkillUpdateArgs),
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
        SkillCommand::List => {
            // Signal failure until List is implemented so CLI exits non-zero
            bail!("list command not implemented")
        }
    }
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

fn resolve_source(skill_id: &str, source_arg: Option<String>) -> Result<String> {
    if let Some(s) = source_arg {
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
}
