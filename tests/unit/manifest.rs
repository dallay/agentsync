use std::fs;
use tempfile::TempDir;

#[test]
fn parse_valid_manifest() {
    let td = TempDir::new().unwrap();
    let path = td.path().join("SKILL.md");
    let content = "---\nname: sample-skill\ndescription: A skill\n---\n# Body";
    fs::write(&path, content).unwrap();

    let manifest = agentsync::skills::manifest::parse_skill_manifest(&path).unwrap();
    assert_eq!(manifest.name, "sample-skill");
}

#[test]
fn reject_invalid_name() {
    let td = TempDir::new().unwrap();
    let path = td.path().join("SKILL.md");
    let content = "---\nname: INVALID_NAME\n---\n# Body";
    fs::write(&path, content).unwrap();

    let res = agentsync::skills::manifest::parse_skill_manifest(&path);
    assert!(res.is_err());
}
