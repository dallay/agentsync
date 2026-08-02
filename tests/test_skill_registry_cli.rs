use std::process::Command;

fn agentsync_bin() -> &'static str {
    env!("CARGO_BIN_EXE_agentsync")
}

#[test]
fn registry_validate_command_accepts_shipped_registry() {
    let output = Command::new(agentsync_bin())
        .args(["skill", "registry", "validate"])
        .output()
        .expect("registry validate should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("valid"));
}

#[test]
fn registry_sync_command_refreshes_lockfile_atomically() {
    let temp = tempfile::TempDir::new().unwrap();
    let manifest = temp.path().join("registry.v1.toml");
    let lock = temp.path().join("registry.lock.toml");
    std::fs::copy("src/skills/registry.v1.toml", &manifest).unwrap();

    let output = Command::new(agentsync_bin())
        .args([
            "skill",
            "registry",
            "sync",
            "--manifest",
            manifest.to_str().unwrap(),
            "--lock",
            lock.to_str().unwrap(),
        ])
        .output()
        .expect("registry sync should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest_doc =
        toml::from_str::<toml::Value>(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
    let lock_doc = toml::from_str::<toml::Value>(&std::fs::read_to_string(lock).unwrap()).unwrap();
    assert_eq!(manifest_doc, lock_doc);
}
