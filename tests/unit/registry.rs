use std::fs;
use std::path::Path;
use tempfile::TempDir;

use agentsync::skills::registry::{load_curated_registry, validate_curated_registry};

#[test]
fn write_and_read_registry() {
    let td = TempDir::new().unwrap();
    let path = td.path().join("registry.json");

    agentsync::skills::registry::write_registry(&path).unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("schemaVersion"));

    // Now test update_registry_entry
    let entry = agentsync::skills::registry::SkillEntry {
        name: Some("sample".to_string()),
        version: Some("1.0".to_string()),
        description: None,
        provider: Some("skills.sh".to_string()),
        source: Some("owner/repo".to_string()),
        installed_at: None,
        files: Some(vec!["SKILL.md".to_string()]),
        manifest_hash: None,
    };

    agentsync::skills::registry::update_registry_entry(&path, "sample", entry).unwrap();

    let content2 = fs::read_to_string(&path).unwrap();
    assert!(content2.contains("sample"));
}

#[test]
fn curated_registry_loads_typed_metadata_in_deterministic_order() {
    let registry =
        load_curated_registry(Path::new("tests/fixtures/curated-skills/registry.v1.toml")).unwrap();

    assert_eq!(registry.schema_version, "v1");
    let ids: Vec<_> = registry.entries.keys().cloned().collect();
    assert_eq!(ids, vec!["valid-skill"]);
    assert_eq!(registry.entries["valid-skill"].source.commit.len(), 40);
    assert_eq!(registry.entries["valid-skill"].files[0].path, "SKILL.md");
}

#[test]
fn curated_registry_validation_reports_invalid_fields() {
    let error = load_curated_registry(Path::new(
        "tests/fixtures/curated-skills/invalid-registry.toml",
    ))
    .expect_err("invalid registry must be rejected");
    let message = format!("{error:#}");

    assert!(message.contains("source.commit"), "diagnostic: {message}");
}

#[test]
fn curated_registry_validation_accepts_complete_entry() {
    let registry =
        load_curated_registry(Path::new("tests/fixtures/curated-skills/registry.v1.toml")).unwrap();
    validate_curated_registry(&registry).unwrap();
}

#[test]
fn shipped_curated_registry_contains_reviewed_local_skills() {
    let registry = load_curated_registry(Path::new("src/skills/registry.v1.toml"))
        .expect("shipped registry should contain valid curated entries");

    assert!(registry.entries.contains_key("accessibility"));
    assert!(registry.entries.contains_key("docker-expert"));
    assert!(registry.entries.values().all(|entry| {
        entry.source.commit.len() == 40 && entry.files.iter().any(|file| file.path == "SKILL.md")
    }));
}
