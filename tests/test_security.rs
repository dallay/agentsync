use agentsync::config::Config;
use agentsync::linker::{Linker, SyncOptions};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_nested_glob_search_root_traversal() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().join("project");
    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();

    // Create a file OUTSIDE the project root
    let outside_dir = temp_dir.path().join("outside_dir");
    fs::create_dir_all(&outside_dir).unwrap();
    fs::write(outside_dir.join("AGENTS.md"), "outside").unwrap();

    let config_path = agents_dir.join("agentsync.toml");

    // We want to walk a directory OUTSIDE the project root
    let relative_outside = "../outside_dir";

    let toml = format!(
        r#"
        source_dir = "."
        [agents.malicious]
        enabled = true
        [agents.malicious.targets.nested]
        source = "{}"
        destination = "leaked/{{file_name}}"
        type = "nested-glob"
    "#,
        relative_outside
    );

    fs::write(&config_path, toml).unwrap();

    let config = Config::load(&config_path).unwrap();
    let linker = Linker::new(config, config_path.clone());

    let options = SyncOptions {
        verbose: true,
        ..Default::default()
    };
    let result = linker.sync(&options).unwrap();

    // The target should have failed due to unsafe search root
    assert!(
        result.errors > 0,
        "Sync should have errors for malicious search root"
    );

    let leaked_link = project_root.join("leaked").join("AGENTS.md");
    assert!(
        !leaked_link.exists(),
        "Should NOT have created a symlink to a file discovered outside project root"
    );
    assert!(
        !project_root.join("leaked").exists(),
        "Should NOT have created the leaked directory"
    );

    // Absolute paths should also be rejected.
    // Use replace to escape backslashes so the path is valid in a TOML basic string on Windows.
    let path_str = outside_dir.display().to_string().replace('\\', "\\\\");
    let absolute_toml = format!(
        r#"
        source_dir = "."
        [agents.malicious]
        enabled = true
        [agents.malicious.targets.nested]
        source = "{}"
        destination = "leaked/{{file_name}}"
        type = "nested-glob"
    "#,
        path_str
    );
    fs::write(&config_path, absolute_toml).unwrap();

    let absolute_config = Config::load(&config_path).unwrap();
    let absolute_linker = Linker::new(absolute_config, config_path);
    let absolute_result = absolute_linker.sync(&options).unwrap();
    assert!(
        absolute_result.errors > 0,
        "Sync should have errors for absolute-path search root"
    );

    // clean() must not traverse or remove anything outside the project.
    let clean_result = absolute_linker.clean(&SyncOptions::default()).unwrap();
    assert_eq!(
        clean_result.removed, 0,
        "Clean should not remove anything for an invalid search root"
    );
}

#[test]
fn test_symlink_source_traversal() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().join("project");
    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();

    // Create a sensitive file OUTSIDE the project root
    let root_parent = temp_dir.path();
    let sensitive_file = root_parent.join("sensitive.txt");
    fs::write(&sensitive_file, "SENSITIVE CONTENT").unwrap();

    let config_path = agents_dir.join("agentsync.toml");

    // Malicious source pointing outside project root using relative path
    let malicious_source = "../../sensitive.txt";

    let toml = format!(
        r#"
        source_dir = "."
        [agents.attacker]
        enabled = true
        [agents.attacker.targets.malicious]
        source = "{}"
        destination = "linked_sensitive"
        type = "symlink"
    "#,
        malicious_source
    );
    fs::write(&config_path, toml).unwrap();

    let config = Config::load(&config_path).unwrap();
    let linker = Linker::new(config, config_path);
    let options = SyncOptions {
        verbose: true,
        ..Default::default()
    };

    // The target should now fail due to unsafe source path
    let result = linker.sync(&options).unwrap();
    assert_eq!(result.errors, 1);

    let linked_file = project_root.join("linked_sensitive");
    assert!(!linked_file.exists());
}

#[test]
fn test_path_traversal_bypass_attempt() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().join("project");
    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();

    // Create a dummy dir and valid source file inside project root
    let dummy_dir = project_root.join("dummy");
    fs::create_dir_all(&dummy_dir).unwrap();
    fs::write(project_root.join("AGENTS.md"), "safe source").unwrap();

    let config_path = agents_dir.join("agentsync.toml");

    // Attempt to bypass check using non-existent directory + ParentDir components
    // Escape to a writable location outside project root so we can assert no mutation.
    let bypass_path = "../escaped/linked_outside";

    let toml = format!(
        r#"
        source_dir = "."
        [agents.attacker]
        enabled = true
        [agents.attacker.targets.bypass]
        source = "AGENTS.md"
        destination = "{}"
        type = "symlink"
    "#,
        bypass_path
    );
    fs::write(&config_path, toml).unwrap();

    let config = Config::load(&config_path).unwrap();
    let linker = Linker::new(config, config_path);
    let options = SyncOptions {
        verbose: true,
        ..Default::default()
    };

    let result = linker.sync(&options).unwrap();
    assert_eq!(
        result.errors, 1,
        "Sync should have errors for bypass path traversal attempt"
    );

    // Verify that no file or symlink was created outside project root
    let escaped_path = temp_dir.path().join("escaped").join("linked_outside");
    assert!(
        !escaped_path.exists(),
        "Sync must not create links/files outside project root"
    );
}

#[test]
fn test_skill_install_id_traversal() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().join("project");
    let agents_dir = project_root.join(".agents");
    let skills_dir = agents_dir.join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    let source_dir = temp_dir.path().join("source_skill");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join("SKILL.md"), "---\nname: test-skill\n---").unwrap();

    // Malicious skill ID attempting traversal
    let malicious_id = "../outside_skill";

    let result =
        agentsync::skills::install::install_from_dir(malicious_id, &source_dir, &skills_dir);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("invalid skill id"));

    let outside_path = agents_dir.join("outside_skill");
    assert!(!outside_path.exists());
}
