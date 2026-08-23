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
