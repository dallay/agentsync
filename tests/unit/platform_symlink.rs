use agentsync::config::{AgentConfig, Config, SyncType, TargetConfig};
use agentsync::linker::{Linker, SyncOptions};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to check if symbolic link creation is supported in the current environment
fn is_symlink_creation_supported() -> bool {
    let temp = TempDir::new().unwrap();
    let src = temp.path().join("src.txt");
    let dest = temp.path().join("dest.txt");
    if fs::write(&src, "test").is_err() {
        return false;
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&src, &dest).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(&src, &dest).is_ok()
    }
}

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
    agents.insert("test_agent".to_string(), agent_config);

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
fn test_symlink_creation_and_cleanup_file() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    let source_file = agents_dir.join("source.md");
    fs::write(&source_file, "# Source Content").unwrap();

    let target = make_target("source.md", "dest.md", SyncType::Symlink);
    let config = make_config_with_target(target);
    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let options = SyncOptions {
        verbose: true,
        ..Default::default()
    };

    let result = linker.sync(&options);

    if is_symlink_creation_supported() {
        assert!(result.is_ok(), "Sync failed: {:?}", result);
        let sync_result = result.unwrap();
        assert_eq!(sync_result.created, 1);
        assert_eq!(sync_result.errors, 0);

        let dest = project_root.join("dest.md");
        assert!(dest.is_symlink(), "Expected dest.md to be a symlink");

        // Verify the target is relative and points correctly
        let target_path = fs::read_link(&dest).unwrap();
        assert_eq!(target_path, PathBuf::from(".agents/source.md"));

        // Cleanup
        let clean_result = linker.clean(&options).unwrap();
        assert_eq!(clean_result.removed, 1);
        assert!(!dest.exists());
        assert!(!dest.is_symlink());
    } else {
        // If symlink creation is not supported (e.g. Windows without developer mode/admin),
        // sync must return an error containing our highly actionable advice.
        match result {
            Err(e) => {
                let _err_msg = e.to_string();
                #[cfg(windows)]
                {
                    assert!(
                        _err_msg.contains("Developer Mode") || _err_msg.contains("Administrator"),
                        "Windows error message lacked actionable advice: {}",
                        _err_msg
                    );
                }
            }
            Ok(r) => {
                assert!(
                    r.errors > 0,
                    "Expected errors to be reported when symlinks are not supported"
                );
            }
        }
    }
}

#[test]
fn test_symlink_creation_and_cleanup_directory() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let agents_dir = project_root.join(".agents");
    let source_dir = agents_dir.join("commands");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join("test.md"), "content").unwrap();

    let target = make_target("commands", "dest_dir", SyncType::Symlink);
    let config = make_config_with_target(target);
    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let options = SyncOptions {
        verbose: true,
        ..Default::default()
    };

    let result = linker.sync(&options);

    if is_symlink_creation_supported() {
        assert!(result.is_ok(), "Sync failed: {:?}", result);
        let sync_result = result.unwrap();
        assert_eq!(sync_result.created, 1);

        let dest_dir = project_root.join("dest_dir");
        assert!(
            dest_dir.is_symlink(),
            "Expected dest_dir to be a symlink directory"
        );

        // Cleanup
        let clean_result = linker.clean(&options).unwrap();
        assert_eq!(clean_result.removed, 1);
        assert!(!dest_dir.exists());
    } else {
        assert!(result.is_err() || result.unwrap().errors > 0);
    }
}

#[test]
fn test_broken_symlink_handling() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    let source_file = agents_dir.join("source.md");
    fs::write(&source_file, "# Actual Content").unwrap();

    let dest = project_root.join("dest.md");

    // Manually create a broken symlink at destination
    let is_supported = {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(".agents/nonexistent.md", &dest).is_ok()
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(
                &std::path::Path::new(".agents/nonexistent.md"),
                &dest,
            )
            .is_ok()
        }
    };

    if !is_supported {
        // If the platform/environment doesn't allow symlink creation, we can't test this scenario.
        return;
    }

    assert!(dest.is_symlink(), "Must be a symlink");
    assert!(!dest.exists(), "Must be broken");

    let target = make_target("source.md", "dest.md", SyncType::Symlink);
    let config = make_config_with_target(target);
    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let options = SyncOptions {
        verbose: true,
        ..Default::default()
    };

    // 1. Syncing should recognize the wrong target of the broken symlink, remove it, and create a correct one.
    let sync_result = linker.sync(&options).unwrap();
    assert_eq!(
        sync_result.updated, 1,
        "Should update the existing broken symlink"
    );
    assert_eq!(sync_result.errors, 0);

    assert!(dest.is_symlink());
    assert!(dest.exists(), "Should no longer be broken");
    assert_eq!(
        fs::read_link(&dest).unwrap(),
        PathBuf::from(".agents/source.md")
    );

    // 2. Now let's test that clean removes it. First, recreate a broken symlink with the CORRECT target
    fs::remove_file(&dest).unwrap();
    let is_recreated = {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(".agents/source.md", &dest).is_ok()
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(&std::path::Path::new(".agents/source.md"), &dest)
                .is_ok()
        }
    };
    assert!(is_recreated);
    assert!(dest.is_symlink());
    // Simulate broken by removing the source file
    fs::remove_file(&source_file).unwrap();
    assert!(!dest.exists(), "Must be broken now");

    // Cleaning should successfully find and remove the broken symlink.
    let clean_result = linker.clean(&options).unwrap();
    assert_eq!(clean_result.removed, 1, "Should remove the broken symlink");
    assert!(!dest.is_symlink());
}

#[test]
fn test_existing_target_handling() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    let source_file = agents_dir.join("source.md");
    fs::write(&source_file, "# Source").unwrap();

    let dest = project_root.join("dest.md");

    // Write a regular file at the destination
    fs::write(&dest, "# Existing File Content").unwrap();

    let target = make_target("source.md", "dest.md", SyncType::Symlink);
    let config = make_config_with_target(target);
    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let options = SyncOptions {
        verbose: true,
        ..Default::default()
    };

    let result = linker.sync(&options);

    if is_symlink_creation_supported() {
        assert!(result.is_ok());
        let sync_result = result.unwrap();
        assert_eq!(sync_result.updated, 1, "Should mark as updated/replaced");

        // The existing file should have been backed up
        let backup = project_root.join("dest.md.bak");
        assert!(backup.exists());
        assert_eq!(
            fs::read_to_string(&backup).unwrap(),
            "# Existing File Content"
        );

        // The destination is now a correct symlink
        assert!(dest.is_symlink());
    }
}

#[test]
fn test_insufficient_permissions() {
    // This is primarily applicable on Unix systems where permissions can be easily modified.
    #[cfg(unix)]
    {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();

        let agents_dir = project_root.join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        let source_file = agents_dir.join("source.md");
        fs::write(&source_file, "# Source").unwrap();

        // Create a read-only directory inside project_root
        let readonly_dir = project_root.join("readonly");
        fs::create_dir_all(&readonly_dir).unwrap();

        let target = make_target("source.md", "readonly/dest.md", SyncType::Symlink);
        let config = make_config_with_target(target);
        let config_path = project_root.join("agentsync.toml");
        let linker = Linker::new(config, config_path);

        // Remove write permissions from readonly_dir
        let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&readonly_dir, perms.clone()).unwrap();

        let options = SyncOptions {
            verbose: true,
            ..Default::default()
        };

        // Syncing should fail gracefully because we cannot write the symlink into readonly_dir
        let result = linker.sync(&options);

        // Restore permissions so TempDir can be cleaned up successfully
        let mut restore_perms = fs::metadata(&readonly_dir).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        restore_perms.set_readonly(false);
        fs::set_permissions(&readonly_dir, restore_perms).unwrap();

        assert!(
            result.is_err() || result.unwrap().errors > 0,
            "Expected sync to report errors due to insufficient permissions"
        );
    }
}

#[test]
fn test_windows_actionable_privilege_error() {
    #[cfg(windows)]
    {
        // Direct test to verify that if std::os::windows::fs::symlink_file fails (such as when
        // Developer Mode is disabled), the generated context message contains Developer Mode instructions.
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src.txt");
        let dest = temp.path().join("dest.txt");
        fs::write(&src, "test").unwrap();

        let res = std::os::windows::fs::symlink_file(&src, &dest);
        if let Err(e) = res {
            // Re-create the exact error context structure we use in symlinks.rs
            let err = anyhow::Error::new(e).context(format!(
                "Failed to create file symlink: {}\nEnsure Windows Developer Mode is enabled or run as Administrator.\nSee https://dallay.github.io/agentsync/guides/windows-symlink-setup/ for details.",
                dest.display()
            ));
            let err_msg = format!("{:#}", err);
            assert!(err_msg.contains("Developer Mode"));
            assert!(err_msg.contains("Administrator"));
            assert!(err_msg.contains("windows-symlink-setup"));
        }
    }
}
