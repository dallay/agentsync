use agentsync::Linker;
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

#[test]
fn plugin_mcp_is_fanned_out_to_supported_agents_without_execution() {
    let project = TempDir::new().unwrap();
    let project_root = project.path().join("project");
    fs::create_dir_all(&project_root).unwrap();
    copy_tree(&fixture_root(), &project_root.join("marketplace"));
    let agents = project_root.join(".agents");
    fs::create_dir_all(&agents).unwrap();
    let config_path = agents.join("agentsync.toml");
    fs::write(
        &config_path,
        r#"
[mcp]
enabled = true

[agents.claude]
[agents.codex]
[agents.gemini]
[agents.opencode]

[plugins]
enabled = true
lockfile = "plugins.lock.toml"
allowed_mcp = ["plugin/internal/engineering/safe-fixture"]

[plugins.marketplaces.internal]
source = "../marketplace"

[[plugins.selections]]
marketplace = "internal"
plugin = "engineering"
"#,
    )
    .unwrap();

    let config = Config::load(&config_path).unwrap();
    let manager = PluginManager::new(
        Config::project_root(&config_path),
        config_path.clone(),
        config.plugins.clone(),
    );
    let plugin_result = manager.add(&PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "engineering".to_string(),
    });
    let plugin_result = plugin_result.unwrap();
    let previous_data_home = std::env::var_os("XDG_DATA_HOME");
    let data_root = project.path().join("local-data");
    // Keep the process-level data override beside, not inside, the project.
    unsafe { std::env::set_var("XDG_DATA_HOME", &data_root) };
    let linker = Linker::new(config, config_path);
    let sync_result = linker
        .sync_mcp_with_servers(false, None, &plugin_result.mcp_servers)
        .unwrap();
    assert_eq!(sync_result.errors, 0);
    assert!(sync_result.created + sync_result.updated >= 4);

    let expected_name = "plugin/internal/engineering/safe-fixture";
    let claude: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(project_root.join(".mcp.json")).unwrap()).unwrap();
    assert!(claude["mcpServers"][expected_name].is_object());

    let codex = fs::read_to_string(project_root.join(".codex/config.toml")).unwrap();
    assert!(codex.contains(expected_name));

    let gemini: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(project_root.join(".gemini/settings.json")).unwrap(),
    )
    .unwrap();
    assert!(gemini["mcpServers"][expected_name].is_object());

    let opencode: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(project_root.join("opencode.json")).unwrap())
            .unwrap();
    assert!(opencode["mcp"][expected_name].is_object());

    // Revert consumes the saved file snapshot; it does not materialize plugin
    // servers again or infer ownership from their names.
    let revert_result = linker.restore_mcp_ownership(false, None).unwrap();
    assert_eq!(revert_result.errors, 0);
    assert_eq!(revert_result.updated, 4);
    assert!(!project_root.join(".mcp.json").exists());
    assert!(!project_root.join(".codex/config.toml").exists());
    assert!(!project_root.join(".gemini/settings.json").exists());
    assert!(!project_root.join("opencode.json").exists());
    unsafe {
        if let Some(previous_data_home) = previous_data_home {
            std::env::set_var("XDG_DATA_HOME", previous_data_home);
        } else {
            std::env::remove_var("XDG_DATA_HOME");
        }
    }
}
