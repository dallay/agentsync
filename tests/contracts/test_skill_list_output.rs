use std::process::Command;
use tempfile::TempDir;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_agentsync")
}

#[cfg(unix)]
#[test]
fn skill_list_json_reports_missing_manifest_and_broken_symlink() {
    let project = TempDir::new().unwrap();
    let skills = project.path().join(".agents/skills");
    std::fs::create_dir_all(skills.join("missing-manifest")).unwrap();
    std::os::unix::fs::symlink("does-not-exist", skills.join("broken-link")).unwrap();

    let output = Command::new(binary())
        .args(["skill", "--project-root"])
        .arg(project.path())
        .args(["list", "--json"])
        .output()
        .expect("run agentsync skill list");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["skills"][0]["id"], "broken-link");
    assert_eq!(value["skills"][0]["state"], "broken_symlink");
    assert_eq!(value["skills"][1]["id"], "missing-manifest");
    assert_eq!(value["skills"][1]["state"], "missing_manifest");
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| { diagnostic == "broken symlink: .agents/skills/broken-link" })
    );
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| {
                diagnostic == "missing SKILL.md: .agents/skills/missing-manifest"
            })
    );
}

#[test]
fn skill_list_json_empty_contract() {
    let project = TempDir::new().unwrap();
    let output = Command::new(binary())
        .args(["skill", "--project-root"])
        .arg(project.path())
        .args(["list", "--json"])
        .output()
        .expect("run agentsync skill list");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["skills"], serde_json::json!([]));
    assert_eq!(value["diagnostics"], serde_json::json!([]));
    assert!(!project.path().join(".agents/skills").exists());
}

fn write_skill(project: &TempDir, directory: &str, manifest: &str) {
    let path = project.path().join(".agents/skills").join(directory);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("SKILL.md"), manifest).unwrap();
}

#[test]
fn skill_list_json_mixed_project_contract() {
    let project = TempDir::new().unwrap();
    let skills = project.path().join(".agents/skills");
    std::fs::create_dir_all(&skills).unwrap();
    write_skill(
        &project,
        "direct-skill",
        "---\nname: direct-skill\nversion: 1.2.3\n---\n",
    );
    write_skill(&project, "manual-skill", "---\nname: manual-skill\n---\n");
    write_skill(&project, "malformed-skill", "no frontmatter");
    std::fs::write(
        skills.join("registry.json"),
        r#"{"schemaVersion":1,"skills":{"direct-skill":{"name":"direct-skill","version":"1.2.3","description":null,"provider":"source","source":"https://user:secret@example.test/direct?token=abc","installedAt":null,"files":null,"manifestHash":null},"plugin-skill":{"name":"plugin-skill","version":null,"description":null,"provider":null,"source":null,"installedAt":null,"files":null,"manifestHash":null,"pluginOwners":[{"marketplace":"m","plugin":"p","revision":"r"}]},"missing-skill":{"name":"missing-skill","version":null,"description":null,"provider":null,"source":"https://user:secret@example.test/missing?token=abc","installedAt":null,"files":null,"manifestHash":null}}}"#,
    )
    .unwrap();
    write_skill(&project, "plugin-skill", "---\nname: plugin-skill\n---\n");

    let output = Command::new(binary())
        .args(["skill", "--project-root"])
        .arg(project.path())
        .args(["list", "--json"])
        .output()
        .expect("run agentsync skill list");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let entries = value["skills"].as_array().unwrap();
    let ids: Vec<&str> = entries
        .iter()
        .map(|entry| entry["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec![
            "direct-skill",
            "malformed-skill",
            "manual-skill",
            "missing-skill",
            "plugin-skill",
        ]
    );
    assert_eq!(entries[0]["source_kind"], "direct");
    assert_eq!(entries[0]["version"], "1.2.3");
    assert_eq!(entries[0]["provenance"], "https://example.test/direct");
    assert!(
        !entries[0]["provenance"]
            .as_str()
            .unwrap()
            .contains("secret")
    );
    assert!(!entries[0]["provenance"].as_str().unwrap().contains("token"));
    assert_eq!(entries[0]["state"], "installed");
    assert_eq!(entries[1]["state"], "malformed");
    assert_eq!(entries[1]["version"], serde_json::Value::Null);
    assert_eq!(entries[2]["source_kind"], "manual/untracked");
    assert_eq!(entries[2]["provenance"], serde_json::Value::Null);
    assert_eq!(entries[3]["state"], "missing");
    assert_eq!(entries[3]["provenance"], "https://example.test/missing");
    assert!(
        !entries[3]["provenance"]
            .as_str()
            .unwrap()
            .contains("secret")
    );
    assert!(!entries[3]["provenance"].as_str().unwrap().contains("token"));
    assert_eq!(entries[4]["source_kind"], "plugin");
    assert!(!value["diagnostics"].as_array().unwrap().is_empty());
}

#[test]
fn skill_list_json_is_deterministic_and_read_only() {
    let project = TempDir::new().unwrap();
    write_skill(&project, "alpha", "---\nname: alpha\n---\n");
    write_skill(&project, "beta", "---\nname: beta\n---\n");

    let run = || {
        Command::new(binary())
            .args(["skill", "--project-root"])
            .arg(project.path())
            .args(["list", "--json"])
            .output()
            .expect("run agentsync skill list")
            .stdout
    };
    assert_eq!(run(), run());

    let before = std::fs::read_dir(project.path().join(".agents/skills"))
        .unwrap()
        .count();
    let _ = run();
    let after = std::fs::read_dir(project.path().join(".agents/skills"))
        .unwrap()
        .count();
    assert_eq!(before, after);
}

#[test]
fn skill_list_json_reports_malformed_manifest_state_and_diagnostic() {
    let project = TempDir::new().unwrap();
    let skill = project.path().join(".agents/skills/malformed");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(skill.join("SKILL.md"), "---\nname: invalid name\n---\n").unwrap();

    let output = Command::new(binary())
        .args(["skill", "--project-root"])
        .arg(project.path())
        .args(["list", "--json"])
        .output()
        .expect("run agentsync skill list");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["skills"][0]["id"], "malformed");
    assert_eq!(value["skills"][0]["state"], "malformed");
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| { diagnostic == "malformed SKILL.md: .agents/skills/malformed" })
    );
}

#[test]
fn skill_list_json_scan_failure_has_distinct_diagnostic_and_preserves_error() {
    let project = TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".agents")).unwrap();
    std::fs::write(project.path().join(".agents/skills"), "not a directory").unwrap();

    let output = Command::new(binary())
        .args(["skill", "--project-root"])
        .arg(project.path())
        .args(["list", "--json"])
        .output()
        .expect("run agentsync skill list");

    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["diagnostic_code"], "inventory_unreadable");
    assert_eq!(
        value["diagnostics"][0],
        "skill inventory could not be scanned"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a directory"));
}

#[test]
fn skill_list_json_corrupt_registry_reports_redacted_diagnostic_and_fails() {
    let project = TempDir::new().unwrap();
    let skills = project.path().join(".agents/skills");
    std::fs::create_dir_all(&skills).unwrap();
    std::fs::write(skills.join("registry.json"), "not json").unwrap();

    let output = Command::new(binary())
        .args(["skill", "--project-root"])
        .arg(project.path())
        .args(["list", "--json"])
        .output()
        .expect("run agentsync skill list");

    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["skills"], serde_json::json!([]));
    assert_eq!(value["diagnostic_code"], "registry_invalid");
    assert_eq!(
        value["diagnostics"][0],
        "installed skill registry could not be read or parsed"
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("not json"));
}

#[test]
fn skill_list_json_reports_duplicate_manifest_ids() {
    let project = TempDir::new().unwrap();
    write_skill(&project, "first-copy", "---\nname: shared-skill\n---\n");
    write_skill(&project, "second-copy", "---\nname: shared-skill\n---\n");

    let output = Command::new(binary())
        .args(["skill", "--project-root"])
        .arg(project.path())
        .args(["list", "--json"])
        .output()
        .expect("run agentsync skill list");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["skills"][0]["id"], "first-copy");
    assert_eq!(value["skills"][0]["state"], "duplicate_id");
    assert_eq!(value["skills"][1]["id"], "second-copy");
    assert_eq!(value["skills"][1]["state"], "duplicate_id");
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| {
                diagnostic
                    == "duplicate skill ID 'shared-skill' in directories: first-copy, second-copy"
            })
    );
}
