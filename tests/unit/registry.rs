use std::fs;
use tempfile::TempDir;

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
        installedAt: None,
        files: Some(vec!["SKILL.md".to_string()]),
        manifestHash: None,
    };

    agentsync::skills::registry::update_registry_entry(&path, "sample", entry).unwrap();

    let content2 = fs::read_to_string(&path).unwrap();
    assert!(content2.contains("sample"));
}
