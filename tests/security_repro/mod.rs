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
    let expected_root = fs::canonicalize(temp_dir.path()).unwrap();

    // The sync should reject traversal attempts, and nothing should escape the project root.
    if traversal_file.exists() {
        let escaped_path = fs::canonicalize(&traversal_file).unwrap_or(traversal_file.clone());
        let escaped_outside_root = !escaped_path.starts_with(&expected_root);

        if traversal_file.is_dir() {
            let _ = fs::remove_dir_all(&traversal_file);
        } else {
            let _ = fs::remove_file(&traversal_file);
        }

        if escaped_outside_root {
            panic!(
                "VULNERABILITY: Path traversal escaped project root and created: {}",
                escaped_path.display()
            );
        }
    }

    match result {
        Err(_) => {}
        Ok(sync_result) => assert!(
            sync_result.errors > 0,
            "Expected an error for path traversal attempt"
        ),
    }
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
    let escaped_dir = TempDir::new_in(temp_dir.path().parent().unwrap()).unwrap();
    let linked_output = escaped_dir.path().join("linked.md");

    fs::create_dir_all(&agents_dir).unwrap();
    symlink(escaped_dir.path(), temp_dir.path().join("escape-link")).unwrap();

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

#[test]
#[cfg(unix)]
fn test_install_from_dir_skips_symlinks() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let project_root = root.join("project");
    let skills_root = project_root.join(".agents").join("skills");
    fs::create_dir_all(&skills_root).unwrap();

    let secret_file = root.join("secret.txt");
    fs::write(&secret_file, "TOP SECRET CONTENT").unwrap();

    let malicious_src = root.join("malicious_skill");
    fs::create_dir_all(&malicious_src).unwrap();
    fs::write(
        malicious_src.join("SKILL.md"),
        "---\nname: malicious\nversion: 1.0.0\ndescription: test\n---\n# Malicious Skill",
    )
    .unwrap();

    // Create a symlink pointing to the secret file
    std::os::unix::fs::symlink(&secret_file, malicious_src.join("stolen_data.txt")).unwrap();

    // Install the skill from the malicious directory
    let res =
        agentsync::skills::install::install_from_dir("malicious", &malicious_src, &skills_root);
    assert!(res.is_ok());

    let installed_file = skills_root.join("malicious").join("stolen_data.txt");

    // The symlink should have been skipped, so the file should not exist in the destination.
    assert!(
        !installed_file.exists(),
        "SECURITY VULNERABILITY: Symlink was followed and content was copied!"
    );
}
