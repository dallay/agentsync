// use agentsync::skills::install::SkillInstallError; (unused)
use agentsync::skills::registry;
use anyhow::Result;
use clap::{Args, Subcommand};
use std::path::PathBuf;
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
    let source = args.source.as_deref().unwrap_or(skill_id);
    let update_source_path = std::path::Path::new(source);
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
                        tracing::warn!(?e, "Failed to read registry after update, falling back to minimal response");
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
                println!("{}", serde_json::to_string(&output).unwrap());
                std::process::exit(1);
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
            println!("Listing skills not yet implemented");
            Ok(())
        }
    }
}

pub fn run_install(args: SkillInstallArgs, project_root: PathBuf) -> Result<()> {
    let target_root = project_root.join(".agents").join("skills");
    std::fs::create_dir_all(&target_root)?;

    // For this example we'll accept local ZIP/path as "skill_id" (for test and demo), real impl will use resolver
    // Accept: skill.zip / skill.tar.gz (local file path or URL); fallback: skill_id as path
    let skill_id = &args.skill_id;
    let source = args.source.clone().unwrap_or_else(|| skill_id.clone());
    let _is_zip = source.ends_with(".zip");
    let _is_targz = source.ends_with(".tar.gz") || source.ends_with(".tgz");
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
                        tracing::warn!(?e, "Failed to read registry after install, falling back to minimal response");
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
            if let Some(_skill_err) =
                e.downcast_ref::<agentsync::skills::install::SkillInstallError>()
            {
                err_string = e.to_string();
                code = "install_error";
                remediation = remediation_for_error(&err_string);
            } else {
                err_string = e.to_string();
                code = "unknown";
                remediation = remediation_for_error(&err_string);
            }

            if args.json {
                let output = serde_json::json!({
                    "error": err_string,
                    "code": code,
                    "remediation": remediation
                });
                println!("{}", serde_json::to_string(&output).unwrap());
                std::process::exit(1);
            } else {
                error!(%code, %err_string, "Install failed");
                println!("Hint: {}", remediation);
                Err(e)
            }
        }
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
