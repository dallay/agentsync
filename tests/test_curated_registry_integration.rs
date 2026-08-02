use agentsync::skills::catalog::parse_catalog;
use agentsync::skills::install::install_from_dir_verified;
use agentsync::skills::provider::{PinnedProvider, Provider};
use agentsync::skills::registry::load_curated_registry;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn catalog_registry_provider_install_is_offline_and_preserves_runtime_ids() {
    let catalog = parse_catalog(
        r#"
version = "v1"

[[skills]]
provider_skill_id = "owner/valid-skill"
local_skill_id = "valid-skill"
title = "Valid Skill"
summary = "Fixture"
registry_entry_id = "valid-skill"
"#,
        "fixture",
        "v1",
    )
    .unwrap();
    let registry =
        load_curated_registry(Path::new("tests/fixtures/curated-skills/registry.v1.toml")).unwrap();
    let definition = catalog.get_skill_definition("owner/valid-skill").unwrap();
    assert_eq!(definition.local_skill_id, "valid-skill");
    assert_eq!(definition.registry_entry_id.as_deref(), Some("valid-skill"));

    let source_root = Path::new("tests/fixtures/curated-skills");
    let provider = PinnedProvider::new(registry.clone(), source_root).without_remote_fallback();
    let resolved = provider.resolve("owner/valid-skill").unwrap();
    assert_eq!(resolved.format, "dir");
    assert!(!resolved.download_url.contains("HEAD"));

    let target = TempDir::new().unwrap();
    install_from_dir_verified(
        "valid-skill",
        Path::new(&resolved.download_url),
        target.path(),
        &registry.entries["valid-skill"],
    )
    .unwrap();

    assert!(target.path().join("valid-skill/SKILL.md").exists());
    let installed =
        agentsync::skills::registry::read_registry(&target.path().join("registry.json")).unwrap();
    assert!(installed.skills.unwrap().contains_key("valid-skill"));
}
