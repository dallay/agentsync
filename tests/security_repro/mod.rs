use agentsync::config::Config;
use agentsync::linker::{Linker, SyncOptions};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
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
    let traversal_file = temp_dir.path().parent().unwrap().join(format!(
        "{}-traversal.md",
        temp_dir.path().file_name().unwrap().to_string_lossy()
    ));
    let traversal_destination = format!(
        "../{}",
        traversal_file.file_name().unwrap().to_string_lossy()
    );
    let config_content = format!(
        r#"
        source_dir = "."

        [agents.malicious]
        enabled = true

        [agents.malicious.targets.traversal]
        source = "AGENTS.md"
        destination = "{traversal_destination}"
        type = "symlink"
    "#
    );
    fs::write(&config_path, config_content).unwrap();

    let config = Config::load(&config_path).unwrap();
    let linker = Linker::new(config, config_path);

    let options = SyncOptions::default();
    let result = linker.sync(&options);

    // This should currently SUCCEED and create a file outside temp_dir,
    // which we want to prevent.

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
    let absolute_destination = temp_dir.path().join(format!(
        "{}-malicious.md",
        temp_dir.path().file_name().unwrap().to_string_lossy()
    ));
    let config_content = format!(
        r#"
        source_dir = "."

        [agents.malicious]
        enabled = true

        [agents.malicious.targets.absolute]
        source = "AGENTS.md"
        destination = "{}"
        type = "symlink"
    "#,
        absolute_destination.display()
    );
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
        !absolute_destination.exists(),
        "Absolute path should not have been created"
    );
}

#[test]
#[cfg(unix)]
fn test_symlinked_destination_ancestor_is_rejected() {
    let temp_dir = TempDir::new().unwrap();
    let agents_dir = temp_dir.path().join(".agents");
    let escaped_dir = temp_dir.path().parent().unwrap().join(format!(
        "{}-escape-target",
        temp_dir.path().file_name().unwrap().to_string_lossy()
    ));
    let linked_output = escaped_dir.join("linked.md");

    fs::create_dir_all(&agents_dir).unwrap();
    fs::create_dir_all(&escaped_dir).unwrap();
    symlink(&escaped_dir, temp_dir.path().join("escape-link")).unwrap();

    let source_file = agents_dir.join("AGENTS.md");
    fs::write(&source_file, "# Test").unwrap();

    let config_path = agents_dir.join("agentsync.toml");
    let config_content = r#"
        source_dir = "."

        [agents.malicious]
        enabled = true

        [agents.malicious.targets.symlink_ancestor]
        source = "AGENTS.md"
        destination = "escape-link/linked.md"
        type = "symlink"
    "#;
    fs::write(&config_path, config_content).unwrap();

    let config = Config::load(&config_path).unwrap();
    let linker = Linker::new(config, config_path);

    let sync_result = linker.sync(&SyncOptions::default()).unwrap();

    assert!(
        sync_result.errors > 0,
        "Expected an error for symlink-ancestor destination escape"
    );
    assert!(
        !linked_output.exists(),
        "Symlinked ancestor should not allow writes outside project root"
    );
}
