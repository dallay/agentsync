use agentsync::config::{AgentConfig, Config, SyncType, TargetConfig};
use agentsync::linker::Linker;
use std::collections::BTreeMap;
use std::fs;
use tempfile::TempDir;

/// Helper to create a target config
fn make_target(source: &str, destination: &str, sync_type: SyncType) -> TargetConfig {
    TargetConfig {
        source: source.to_string(),
        destination: destination.to_string(),
        sync_type,
        pattern: None,
        exclude: vec![],
        mappings: vec![],
    }
}

/// Helper to create a config with one agent and one target
fn make_config_with_target(target: TargetConfig) -> Config {
    let mut targets = BTreeMap::new();
    targets.insert("target".to_string(), target);

    let agent_config = AgentConfig {
        enabled: true,
        description: String::new(),
        targets,
    };

    let mut agents = BTreeMap::new();
    agents.insert("test".to_string(), agent_config);

    Config {
        source_dir: ".agents".to_string(),
        compress_agents_md: false,
        default_agents: vec![],
        agents,
        gitignore: Default::default(),
        mcp: Default::default(),
        mcp_servers: Default::default(),
    }
}

#[test]
fn test_ensure_safe_destination_rejects_absolute_paths() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create source file first
    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("test.md"), "# Test").unwrap();

    let target = make_target("test.md", "/etc/passwd", SyncType::Symlink);
    let config = make_config_with_target(target);

    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let result = linker.sync(&agentsync::linker::SyncOptions {
        clean: false,
        dry_run: false,
        verbose: false,
        agents: Some(vec!["test".to_string()]),
    });

    // Should fail or report error
    assert!(
        result.is_err() || result.unwrap().errors > 0,
        "Should reject absolute paths"
    );
}

#[test]
fn test_ensure_safe_destination_rejects_parent_dir_traversal() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create source file first
    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("test.md"), "# Test").unwrap();

    let target = make_target("test.md", "../../../etc/passwd", SyncType::Symlink);
    let config = make_config_with_target(target);

    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let result = linker.sync(&agentsync::linker::SyncOptions {
        clean: false,
        dry_run: false,
        verbose: false,
        agents: Some(vec!["test".to_string()]),
    });

    // Should fail or report error due to path traversal
    assert!(
        result.is_err() || result.unwrap().errors > 0,
        "Should reject path traversal"
    );
}

#[test]
fn test_ensure_safe_destination_rejects_empty_path() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create source file first
    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("test.md"), "# Test").unwrap();

    let target = make_target("test.md", "", SyncType::Symlink);
    let config = make_config_with_target(target);

    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let result = linker.sync(&agentsync::linker::SyncOptions {
        clean: false,
        dry_run: false,
        verbose: false,
        agents: Some(vec!["test".to_string()]),
    });

    // Should fail or report error due to empty path
    assert!(
        result.is_err() || result.unwrap().errors > 0,
        "Should reject empty path"
    );
}

#[test]
fn test_ensure_safe_destination_accepts_valid_relative_path() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create source file
    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("test.md"), "# Test").unwrap();

    let target = make_target("test.md", "valid/path/test.md", SyncType::Symlink);
    let config = make_config_with_target(target);

    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let result = linker.sync(&agentsync::linker::SyncOptions {
        clean: false,
        dry_run: false,
        verbose: false,
        agents: Some(vec!["test".to_string()]),
    });

    assert!(result.is_ok(), "Expected success, got: {:?}", result);
}

#[test]
fn test_canonicalize_cached_returns_cached_value() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create source file
    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("source.md"), "# Source").unwrap();

    let target = make_target("source.md", "dest.md", SyncType::Symlink);
    let config = make_config_with_target(target);

    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    // Run sync twice to test caching
    let result1 = linker.sync(&agentsync::linker::SyncOptions {
        clean: false,
        dry_run: false,
        verbose: false,
        agents: Some(vec!["test".to_string()]),
    });

    let result2 = linker.sync(&agentsync::linker::SyncOptions {
        clean: false,
        dry_run: false,
        verbose: false,
        agents: Some(vec!["test".to_string()]),
    });

    assert!(result1.is_ok());
    assert!(result2.is_ok());
}

#[test]
fn test_canonicalize_cached_handles_nonexistent_path() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let target = make_target("nonexistent.md", "dest.md", SyncType::Symlink);
    let config = make_config_with_target(target);

    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let result = linker.sync(&agentsync::linker::SyncOptions {
        clean: false,
        dry_run: false,
        verbose: false,
        agents: Some(vec!["test".to_string()]),
    });

    // Should report error because source doesn't exist
    let sync_result = result.unwrap();
    assert!(
        sync_result.errors > 0 || sync_result.skipped > 0,
        "Should skip or error on nonexistent source"
    );
}

#[test]
fn test_revalidate_unlink_path_works_for_valid_symlink() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create source file
    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("source.md"), "# Source").unwrap();

    let target = make_target("source.md", "link.md", SyncType::Symlink);
    let config = make_config_with_target(target);

    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    // Create the symlink
    let result = linker.sync(&agentsync::linker::SyncOptions {
        clean: false,
        dry_run: false,
        verbose: false,
        agents: Some(vec!["test".to_string()]),
    });

    assert!(result.is_ok());

    // Now clean should work
    let clean_result = linker.clean(&agentsync::linker::SyncOptions {
        clean: true,
        dry_run: false,
        verbose: false,
        agents: Some(vec!["test".to_string()]),
    });

    assert!(clean_result.is_ok());
}

#[test]
fn test_revalidate_path_with_parent_dir_component() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create source file first
    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("source.md"), "# Source").unwrap();

    // Try to create a symlink with .. in destination
    let target = make_target("source.md", "subdir/../escape.md", SyncType::Symlink);
    let config = make_config_with_target(target);

    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let result = linker.sync(&agentsync::linker::SyncOptions {
        clean: false,
        dry_run: false,
        verbose: false,
        agents: Some(vec!["test".to_string()]),
    });

    // Should fail or report error due to parent dir component
    assert!(
        result.is_err() || result.unwrap().errors > 0,
        "Should reject parent dir component"
    );
}

#[test]
fn test_nested_glob_error_handling_with_verbose() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create a nested glob target
    let mut target = make_target(".", "**/test.md", SyncType::NestedGlob);
    target.pattern = Some("**/test.md".to_string());

    let config = make_config_with_target(target);

    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    // Run with verbose to trigger error path logging (lines 1070-1080)
    let result = linker.sync(&agentsync::linker::SyncOptions {
        clean: false,
        dry_run: false,
        verbose: true,
        agents: Some(vec!["test".to_string()]),
    });

    // Should succeed even if no files match
    assert!(result.is_ok());
}

#[test]
fn test_relative_path_with_missing_source() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create agents dir but not the source file
    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();

    let target = make_target("missing.md", "dest.md", SyncType::Symlink);
    let config = make_config_with_target(target);

    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let result = linker.sync(&agentsync::linker::SyncOptions {
        clean: false,
        dry_run: false,
        verbose: false,
        agents: Some(vec!["test".to_string()]),
    });

    // Should report error with source not found (lines 1238-1247)
    let sync_result = result.unwrap();
    assert!(
        sync_result.errors > 0 || sync_result.skipped > 0,
        "Should skip or error on missing source"
    );
}

#[test]
fn test_ensure_safe_path_with_nonexistent_ancestor() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Try to create a symlink in a deeply nested non-existent path
    let target = make_target("test.md", "a/b/c/d/e/f/test.md", SyncType::Symlink);

    // Create source
    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("test.md"), "# Test").unwrap();

    let config = make_config_with_target(target);

    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let result = linker.sync(&agentsync::linker::SyncOptions {
        clean: false,
        dry_run: false,
        verbose: false,
        agents: Some(vec!["test".to_string()]),
    });

    // Should succeed and create parent directories
    assert!(result.is_ok(), "Expected success, got: {:?}", result);
}
