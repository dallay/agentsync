
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

    let toml = format!(r#"
        source_dir = "."
        [agents.malicious]
        enabled = true
        [agents.malicious.targets.nested]
        source = "{}"
        destination = "leaked/{{file_name}}"
        type = "nested-glob"
    "#, relative_outside);

    fs::write(&config_path, toml).unwrap();

    let config = Config::load(&config_path).unwrap();
    let linker = Linker::new(config, config_path);

    let options = SyncOptions { verbose: true, ..Default::default() };
    let result = linker.sync(&options).unwrap();

    // The target should have failed due to unsafe search root
    assert!(result.errors > 0, "Sync should have errors for malicious search root");

    let leaked_link = project_root.join("leaked").join("AGENTS.md");
    assert!(!leaked_link.exists(), "Should NOT have created a symlink to a file discovered outside project root");
}
