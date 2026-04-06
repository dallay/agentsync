//! End-to-end catalog installation verification.
//!
//! This suite is intentionally opt-in because it exercises every catalog entry,
//! including external providers that depend on network availability and third-party
//! repositories staying valid.

use agentsync::skills::catalog::EmbeddedSkillCatalog;
use agentsync::skills::install::blocking_fetch_and_install_skill;
use agentsync::skills::provider::{Provider, SkillsShProvider};
use agentsync::skills::registry::read_registry;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

const DALLAY_SKILLS_PREFIX: &str = "dallay/agents-skills/";

fn project_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn local_skill_source_dir(local_skill_id: &str) -> PathBuf {
    if let Ok(path) = std::env::var("AGENTSYNC_LOCAL_SKILLS_REPO") {
        return PathBuf::from(path).join("skills").join(local_skill_id);
    }

    let sibling_repo = project_root().parent().map(|parent| {
        parent
            .join("agents-skills")
            .join("skills")
            .join(local_skill_id)
    });
    if let Some(path) = sibling_repo
        && path.exists()
    {
        return path;
    }

    project_root()
        .join(".agents")
        .join("skills")
        .join(local_skill_id)
}

fn resolve_install_source(
    provider: &SkillsShProvider,
    provider_skill_id: &str,
    local_skill_id: &str,
) -> anyhow::Result<String> {
    let catalog = EmbeddedSkillCatalog::default();
    if let Some(install_source) = catalog.get_install_source(provider_skill_id) {
        return Ok(install_source.to_string());
    }

    if provider_skill_id.starts_with(DALLAY_SKILLS_PREFIX) {
        return Ok(local_skill_source_dir(local_skill_id)
            .to_string_lossy()
            .into_owned());
    }

    Ok(provider.resolve(provider_skill_id)?.download_url)
}

fn install_with_retry(skill_id: &str, source: &str, target_root: &Path) -> anyhow::Result<()> {
    match blocking_fetch_and_install_skill(skill_id, source, target_root) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            thread::sleep(Duration::from_secs(2));
            blocking_fetch_and_install_skill(skill_id, source, target_root).map_err(
                |second_error| {
                    anyhow::anyhow!("first attempt: {first_error}; retry: {second_error}")
                },
            )
        }
    }
}

#[test]
#[ignore]
fn every_catalog_skill_installs_successfully() {
    if std::env::var("RUN_E2E").is_err() {
        eprintln!("Skipping catalog installation test (set RUN_E2E=1 to enable)");
        return;
    }

    let catalog = EmbeddedSkillCatalog::default();
    let provider = SkillsShProvider;
    let mut failures = Vec::new();

    for definition in catalog.skill_definitions() {
        let temp = TempDir::new().expect("temp dir should be created");
        let target_root = temp.path().join(".agents").join("skills");
        std::fs::create_dir_all(&target_root).expect("target root should be created");

        let source = match resolve_install_source(
            &provider,
            &definition.provider_skill_id,
            &definition.local_skill_id,
        ) {
            Ok(source) => source,
            Err(error) => {
                failures.push(format!(
                    "{} [{}] failed to resolve source: {}",
                    definition.local_skill_id, definition.provider_skill_id, error
                ));
                continue;
            }
        };

        if let Err(error) = install_with_retry(&definition.local_skill_id, &source, &target_root) {
            failures.push(format!(
                "{} [{}] failed to install from {}: {}",
                definition.local_skill_id, definition.provider_skill_id, source, error
            ));
            continue;
        }

        let skill_dir = target_root.join(&definition.local_skill_id);
        let manifest_path = skill_dir.join("SKILL.md");
        if !manifest_path.exists() {
            failures.push(format!(
                "{} [{}] installed without SKILL.md at {}",
                definition.local_skill_id,
                definition.provider_skill_id,
                manifest_path.display()
            ));
            continue;
        }

        let registry_path = target_root.join("registry.json");
        match read_registry(&registry_path) {
            Ok(registry) => {
                let has_entry = registry
                    .skills
                    .unwrap_or_default()
                    .contains_key(&definition.local_skill_id);
                if !has_entry {
                    failures.push(format!(
                        "{} [{}] installed but registry.json is missing its canonical key",
                        definition.local_skill_id, definition.provider_skill_id
                    ));
                }
            }
            Err(error) => failures.push(format!(
                "{} [{}] installed but registry.json could not be read: {}",
                definition.local_skill_id, definition.provider_skill_id, error
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "{} catalog skills failed installation validation:\n- {}",
        failures.len(),
        failures.join("\n- ")
    );
}
