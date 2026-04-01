use agentsync::config::Config;
use agentsync::linker::{Linker, SyncOptions};
use std::fs;
use tempfile::TempDir;

#[test]
#[cfg(unix)]
fn test_path_traversal_in_symlink_destination_is_rejected() {
    let temp_dir = TempDir::new().unwrap();
    let agents_dir = temp_dir.path().join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();

    // Create source file
    let source_file = agents_dir.join("AGENTS.md");
    fs::write(&source_file, "# Test").unwrap();

    // Malicious config attempting to write outside project root
    let config_path = agents_dir.join("agentsync.toml");
    let config_content = r#"
        source_dir = "."

        [agents.malicious]
        enabled = true

        [agents.malicious.targets.traversal]
        source = "AGENTS.md"
        destination = "../traversal.md"
        type = "symlink"
    "#;
    fs::write(&config_path, config_content).unwrap();

    let config = Config::load(&config_path).unwrap();
    let linker = Linker::new(config, config_path);

    let options = SyncOptions::default();
    let result = linker.sync(&options);

    // This should currently SUCCEED and create a file outside temp_dir,
    // which we want to prevent.

    let traversal_file = temp_dir.path().parent().unwrap().join("traversal.md");

    // If the vulnerability exists, this file will be created as a symlink
    if traversal_file.is_symlink() {
        // Cleanup if it was created
        let _ = fs::remove_file(&traversal_file);
        panic!(
            "VULNERABILITY: Path traversal was successful! File created at: {}",
            traversal_file.display()
        );
    }

    // Once fixed, result should be Ok(SyncResult) but result.errors > 0 or it returns an Err.
    let sync_result = result.unwrap();
    assert!(
        sync_result.errors > 0,
        "Expected an error for path traversal attempt"
    );
}

#[test]
#[cfg(unix)]
fn test_absolute_path_in_symlink_destination_is_rejected() {
    let temp_dir = TempDir::new().unwrap();
    let agents_dir = temp_dir.path().join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();

    let source_file = agents_dir.join("AGENTS.md");
    fs::write(&source_file, "# Test").unwrap();

    let config_path = agents_dir.join("agentsync.toml");
    let config_content = r#"
        source_dir = "."

        [agents.malicious]
        enabled = true

        [agents.malicious.targets.absolute]
        source = "AGENTS.md"
        destination = "/tmp/malicious.md"
        type = "symlink"
    "#;
    fs::write(&config_path, config_content).unwrap();

    let config = Config::load(&config_path).unwrap();
    let linker = Linker::new(config, config_path);

    let options = SyncOptions::default();
    let sync_result = linker.sync(&options).unwrap();

    assert!(
        sync_result.errors > 0,
        "Expected an error for absolute path destination"
    );
    assert!(
        !std::path::Path::new("/tmp/malicious.md").exists(),
        "Absolute path should not have been created"
    );
}
