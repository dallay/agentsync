use agentsync::config::Config;
use agentsync::plugins::{PluginManager, PluginSelection};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugin-marketplace")
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &target_path);
        } else {
            fs::create_dir_all(target_path.parent().unwrap()).unwrap();
            fs::copy(source_path, target_path).unwrap();
        }
    }
}

fn setup_project() -> (TempDir, Config) {
    let project = TempDir::new().unwrap();
    let marketplace = project.path().join("marketplace");
    copy_tree(&fixture_root(), &marketplace);
    let agents = project.path().join(".agents");
    fs::create_dir_all(&agents).unwrap();
    fs::write(
        agents.join("agentsync.toml"),
        r#"
[plugins]
enabled = true
lockfile = "plugins.lock.toml"

[plugins.marketplaces.internal]
source = "../marketplace"
reference = "main"

[[plugins.selections]]
marketplace = "internal"
plugin = "engineering"
"#,
    )
    .unwrap();
    let config_path = agents.join("agentsync.toml");
    (project, Config::load(&config_path).unwrap())
}

#[test]
fn plugin_add_writes_lock_materializes_skill_and_returns_mcp_without_execution() {
    let (project, config) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let manager = PluginManager::new(project.path().to_path_buf(), config_path, config.plugins);
    let selection = PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "engineering".to_string(),
    };

    let result = manager.add(&selection).unwrap();

    assert_eq!(result.mcp_servers.len(), 1);
    assert!(
        result
            .mcp_servers
            .contains_key("plugin/internal/engineering/safe-fixture")
    );
    assert!(
        project
            .path()
            .join(".agents/skills/review/SKILL.md")
            .is_file()
    );
    assert!(
        project
            .path()
            .join(".agents/skills/review/references/REFERENCE.md")
            .is_file()
    );
    assert!(project.path().join(".agents/plugins.lock.toml").is_file());
    assert!(
        !project
            .path()
            .join("marketplace/plugins/engineering/hook-ran.txt")
            .exists()
    );

    let second = manager.apply(false).unwrap();
    assert_eq!(second.created, 0);
    assert_eq!(second.updated, 0);
    assert_eq!(second.skipped, 1);

    let registry = agentsync::skills::registry::read_registry(
        &project.path().join(".agents/skills/registry.json"),
    )
    .unwrap();
    let entry = registry.skills.unwrap().remove("review").unwrap();
    assert_eq!(entry.marketplace.as_deref(), Some("internal"));
    assert_eq!(entry.plugin.as_deref(), Some("engineering"));
    assert!(entry.plugin_revision.is_some());
}

#[test]
fn plugin_add_registers_a_missing_selection_in_project_config() {
    let (project, _) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let mut config_body = fs::read_to_string(&config_path).unwrap();
    config_body = config_body.replace(
        "\n[[plugins.selections]]\nmarketplace = \"internal\"\nplugin = \"engineering\"\n",
        "\n",
    );
    fs::write(&config_path, config_body).unwrap();
    let config = Config::load(&config_path).unwrap();
    assert!(config.plugins.selections.is_empty());
    let manager = PluginManager::new(
        project.path().to_path_buf(),
        config_path.clone(),
        config.plugins,
    );

    manager
        .add(&PluginSelection {
            marketplace: "internal".to_string(),
            plugin: "engineering".to_string(),
        })
        .unwrap();

    let updated = fs::read_to_string(config_path).unwrap();
    assert!(updated.contains("[[plugins.selections]]"));
    assert!(updated.contains("marketplace = \"internal\""));
    assert!(updated.contains("plugin = \"engineering\""));
}

#[test]
fn identical_skills_are_deduplicated_and_removed_only_after_last_owner() {
    let (project, config) = setup_project();
    let manifest_path = project
        .path()
        .join("marketplace/.agents/plugins/marketplace.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest["plugins"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "name": "engineering-copy",
            "source": "./plugins/engineering",
            "version": "1.2.3"
        }));
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let config_path = project.path().join(".agents/agentsync.toml");
    let manager = PluginManager::new(project.path().to_path_buf(), config_path, config.plugins);
    let engineering = PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "engineering".to_string(),
    };
    let copy = PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "engineering-copy".to_string(),
    };
    manager.add(&engineering).unwrap();
    let copy_result = manager.add(&copy).unwrap();
    assert_eq!(copy_result.created, 0);
    assert_eq!(copy_result.updated, 0);
    assert_eq!(copy_result.skipped, 2);

    let registry_path = project.path().join(".agents/skills/registry.json");
    let registry = agentsync::skills::registry::read_registry(&registry_path).unwrap();
    let owners = registry
        .skills
        .unwrap()
        .remove("review")
        .unwrap()
        .plugin_owners
        .unwrap();
    assert_eq!(owners.len(), 2);

    manager.remove(&copy, false).unwrap();
    assert!(
        project
            .path()
            .join(".agents/skills/review/SKILL.md")
            .is_file()
    );
    manager.remove(&engineering, false).unwrap();
    assert!(!project.path().join(".agents/skills/review").exists());
}

#[test]
fn plugin_apply_detects_local_source_drift_without_rewriting_content() {
    let (project, config) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let manager = PluginManager::new(project.path().to_path_buf(), config_path, config.plugins);
    let selection = PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "engineering".to_string(),
    };
    manager.add(&selection).unwrap();
    let installed = project.path().join(".agents/skills/review/SKILL.md");
    let before = fs::read_to_string(&installed).unwrap();

    fs::write(
        project
            .path()
            .join("marketplace/plugins/engineering/skills/review/SKILL.md"),
        "changed source",
    )
    .unwrap();

    let error = manager.apply(false).expect_err("drift must fail closed");
    assert!(error.to_string().contains("drift"));
    assert_eq!(before, fs::read_to_string(installed).unwrap());
}

#[test]
fn plugin_add_rejects_unmanaged_skill_collision_and_rolls_back_lock() {
    let (project, config) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let target = project.path().join(".agents/skills/review");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("SKILL.md"), "unmanaged content").unwrap();
    let manager = PluginManager::new(project.path().to_path_buf(), config_path, config.plugins);

    let error = manager
        .add(&PluginSelection {
            marketplace: "internal".to_string(),
            plugin: "engineering".to_string(),
        })
        .expect_err("unmanaged skill collision must fail");
    assert!(error.to_string().contains("collision"));
    assert_eq!(
        fs::read_to_string(target.join("SKILL.md")).unwrap(),
        "unmanaged content"
    );
    assert!(!project.path().join(".agents/plugins.lock.toml").exists());
}

#[test]
fn plugin_add_rolls_back_a_new_selection_when_materialization_fails() {
    let (project, _) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let mut config_body = fs::read_to_string(&config_path).unwrap();
    config_body = config_body.replace(
        "\n[[plugins.selections]]\nmarketplace = \"internal\"\nplugin = \"engineering\"\n",
        "\n",
    );
    fs::write(&config_path, &config_body).unwrap();
    let config = Config::load(&config_path).unwrap();
    let target = project.path().join(".agents/skills/review");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("SKILL.md"), "unmanaged content").unwrap();
    let manager = PluginManager::new(
        project.path().to_path_buf(),
        config_path.clone(),
        config.plugins,
    );

    let error = manager
        .add(&PluginSelection {
            marketplace: "internal".to_string(),
            plugin: "engineering".to_string(),
        })
        .expect_err("failed materialization must roll back a new selection");
    assert!(error.to_string().contains("collision"));
    assert!(
        !fs::read_to_string(&config_path)
            .unwrap()
            .contains("[[plugins.selections]]")
    );
    assert!(!project.path().join(".agents/plugins.lock.toml").exists());
}

#[test]
fn plugin_add_does_not_leave_earlier_skills_after_a_later_failure() {
    let (project, _) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let source_skill = project
        .path()
        .join("marketplace/plugins/engineering/skills/later");
    fs::create_dir_all(&source_skill).unwrap();
    fs::write(
        source_skill.join("SKILL.md"),
        "---\nname: later\nversion: 1.0.0\n---\nlater\n",
    )
    .unwrap();
    let unmanaged = project.path().join(".agents/skills/later");
    fs::create_dir_all(&unmanaged).unwrap();
    fs::write(unmanaged.join("SKILL.md"), "unmanaged").unwrap();
    let config = Config::load(&config_path).unwrap();
    let manager = PluginManager::new(project.path().to_path_buf(), config_path, config.plugins);

    let error = manager
        .add(&PluginSelection {
            marketplace: "internal".to_string(),
            plugin: "engineering".to_string(),
        })
        .expect_err("later unmanaged collision must fail");
    assert!(error.to_string().contains("collision"));
    assert!(!project.path().join(".agents/skills/review").exists());
    assert_eq!(
        fs::read_to_string(unmanaged.join("SKILL.md")).unwrap(),
        "unmanaged"
    );
    assert!(!project.path().join(".agents/plugins.lock.toml").exists());
}

#[test]
fn plugin_add_rolls_back_when_project_config_becomes_invalid() {
    let (project, mut config) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    config.plugins.selections.clear();
    fs::write(&config_path, "[plugins\ninvalid").unwrap();
    let manager = PluginManager::new(
        project.path().to_path_buf(),
        config_path.clone(),
        config.plugins,
    );

    assert!(
        manager
            .add(&PluginSelection {
                marketplace: "internal".to_string(),
                plugin: "engineering".to_string(),
            })
            .is_err()
    );
    assert!(!project.path().join(".agents/plugins.lock.toml").exists());
    assert_eq!(
        fs::read_to_string(config_path).unwrap(),
        "[plugins\ninvalid"
    );
}

#[test]
fn plugin_with_unsupported_hooks_is_rejected_and_hook_is_not_run() {
    let (project, mut config) = setup_project();
    config.plugins.selections = vec![PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "unsafe".to_string(),
    }];
    let config_path = project.path().join(".agents/agentsync.toml");
    let manager = PluginManager::new(project.path().to_path_buf(), config_path, config.plugins);
    let selection = PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "unsafe".to_string(),
    };

    let error = manager
        .add(&selection)
        .expect_err("hooks must be unsupported");
    assert!(error.to_string().contains("unsupported"));
    assert!(
        !project
            .path()
            .join("marketplace/plugins/unsafe/hook-ran.txt")
            .exists()
    );
}

#[test]
fn plugin_apply_rejects_new_unsupported_components_without_execution() {
    let (project, config) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let manager = PluginManager::new(project.path().to_path_buf(), config_path, config.plugins);
    let selection = PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "engineering".to_string(),
    };
    manager.add(&selection).unwrap();
    fs::create_dir_all(project.path().join("marketplace/plugins/engineering/hooks")).unwrap();

    let error = manager
        .apply(false)
        .expect_err("new unsupported components must fail closed");
    assert!(error.to_string().contains("unsupported"));
    assert!(
        !project
            .path()
            .join("marketplace/plugins/engineering/hook-ran.txt")
            .exists()
    );
}

#[cfg(unix)]
#[test]
fn plugin_remove_rejects_unmanaged_materialized_skill() {
    let (project, config) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let manager = PluginManager::new(project.path().to_path_buf(), config_path, config.plugins);
    let selection = PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "engineering".to_string(),
    };
    manager.add(&selection).unwrap();
    fs::remove_file(project.path().join(".agents/skills/registry.json")).unwrap();

    let error = manager
        .remove(&selection, false)
        .expect_err("removal must not delete an unmanaged skill");
    assert!(error.to_string().contains("unmanaged"));
    assert!(project.path().join(".agents/skills/review").is_dir());
    assert!(project.path().join(".agents/plugins.lock.toml").is_file());
}

#[cfg(unix)]
#[test]
fn plugin_remove_rejects_symlinked_owned_skill_destination() {
    use std::os::unix::fs::symlink;

    let (project, config) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let manager = PluginManager::new(project.path().to_path_buf(), config_path, config.plugins);
    let selection = PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "engineering".to_string(),
    };
    manager.add(&selection).unwrap();
    fs::remove_dir_all(project.path().join(".agents/skills/review")).unwrap();
    fs::create_dir_all(project.path().join("outside")).unwrap();
    symlink(
        project.path().join("outside"),
        project.path().join(".agents/skills/review"),
    )
    .unwrap();

    let error = manager
        .remove(&selection, false)
        .expect_err("owned symlink destinations must fail closed");
    assert!(error.to_string().contains("unsafe"));
    assert!(project.path().join(".agents/plugins.lock.toml").is_file());
}

#[test]
fn plugin_remove_handles_missing_materialized_skill_with_registry_owner() {
    let (project, config) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let manager = PluginManager::new(
        project.path().to_path_buf(),
        config_path.clone(),
        config.plugins,
    );
    let selection = PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "engineering".to_string(),
    };
    manager.add(&selection).unwrap();
    fs::remove_dir_all(project.path().join(".agents/skills/review")).unwrap();

    let result = manager.remove(&selection, false).unwrap();
    assert_eq!(result.removed, 0);
    assert!(project.path().join(".agents/plugins.lock.toml").is_file());
    assert!(
        !fs::read_to_string(project.path().join(".agents/plugins.lock.toml"))
            .unwrap()
            .contains("engineering")
    );
    assert!(
        !fs::read_to_string(config_path)
            .unwrap()
            .contains("engineering")
    );
}

#[test]
fn plugin_apply_and_remove_dry_runs_do_not_mutate_materialized_state() {
    let (project, config) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let manager = PluginManager::new(
        project.path().to_path_buf(),
        config_path.clone(),
        config.plugins,
    );
    let selection = PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "engineering".to_string(),
    };
    manager.add(&selection).unwrap();
    let skill = project.path().join(".agents/skills/review/SKILL.md");
    let before_skill = fs::read_to_string(&skill).unwrap();
    let before_config = fs::read(&config_path).unwrap();
    let before_lock = fs::read(project.path().join(".agents/plugins.lock.toml")).unwrap();

    let apply = manager.apply(true).unwrap();
    assert_eq!(apply.updated, 1);
    assert_eq!(apply.skipped, 0);
    let remove = manager.remove(&selection, true).unwrap();
    assert_eq!(remove.removed, 1);
    assert_eq!(before_skill, fs::read_to_string(&skill).unwrap());
    assert_eq!(before_config, fs::read(&config_path).unwrap());
    assert_eq!(
        before_lock,
        fs::read(project.path().join(".agents/plugins.lock.toml")).unwrap()
    );
}

#[test]
fn plugin_apply_updates_managed_skill_content() {
    let (project, config) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let manager = PluginManager::new(project.path().to_path_buf(), config_path, config.plugins);
    let selection = PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "engineering".to_string(),
    };
    manager.add(&selection).unwrap();
    let skill = project.path().join(".agents/skills/review/SKILL.md");
    fs::write(&skill, "---\nname: changed\n---\nlocal edit\n").unwrap();

    let result = manager.apply(false).unwrap();
    assert_eq!(result.updated, 1);
    assert_eq!(result.created, 0);
    assert!(fs::read_to_string(skill).unwrap().contains("Review"));
}

#[test]
fn plugin_apply_detects_mcp_lock_drift_without_network_or_execution() {
    let (project, config) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let manager = PluginManager::new(project.path().to_path_buf(), config_path, config.plugins);
    let selection = PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "engineering".to_string(),
    };
    manager.add(&selection).unwrap();
    let lock_path = project.path().join(".agents/plugins.lock.toml");
    let mut lock = manager.load_lock().unwrap();
    lock.plugins
        .get_mut(&selection.key())
        .unwrap()
        .mcp_servers
        .clear();
    lock.save_atomic(&lock_path).unwrap();

    let error = manager
        .apply(false)
        .expect_err("MCP lock drift must fail closed");
    assert!(error.to_string().contains("MCP declaration drift"));
    assert!(
        project
            .path()
            .join(".agents/skills/review/SKILL.md")
            .is_file()
    );
}

#[test]
fn plugin_apply_rejects_locked_skill_set_and_content_drift() {
    let (project, config) = setup_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let manager = PluginManager::new(project.path().to_path_buf(), config_path, config.plugins);
    let selection = PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "engineering".to_string(),
    };
    manager.add(&selection).unwrap();
    let lock_path = project.path().join(".agents/plugins.lock.toml");

    let mut lock = manager.load_lock().unwrap();
    lock.plugins
        .get_mut(&selection.key())
        .unwrap()
        .skills
        .clear();
    lock.save_atomic(&lock_path).unwrap();
    let error = manager
        .apply(false)
        .expect_err("skill set drift must fail closed");
    assert!(error.to_string().contains("skill set drift"));

    let mut lock = manager.load_lock().unwrap();
    let plugin = lock.plugins.get_mut(&selection.key()).unwrap();
    plugin.skills = vec![agentsync::plugins::LockedSkill {
        id: "review".to_string(),
        path: "skills/review".to_string(),
        content_sha256: "f".repeat(64),
    }];
    lock.save_atomic(&lock_path).unwrap();
    let error = manager
        .apply(false)
        .expect_err("skill content drift must fail closed");
    assert!(error.to_string().contains("skill content drift"));
}
