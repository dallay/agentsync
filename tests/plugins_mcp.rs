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
    copy_tree(&fixture_root(), &project.path().join("marketplace"));
    let agents = project.path().join(".agents");
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
    let linker = Linker::new(config, config_path);
    let sync_result = linker
        .sync_mcp_with_servers(false, None, &plugin_result.mcp_servers)
        .unwrap();
    assert_eq!(sync_result.errors, 0);
    assert!(sync_result.created + sync_result.updated >= 4);

    let expected_name = "plugin/internal/engineering/safe-fixture";
    let claude: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(project.path().join(".mcp.json")).unwrap())
            .unwrap();
    assert!(claude["mcpServers"][expected_name].is_object());

    let codex = fs::read_to_string(project.path().join(".codex/config.toml")).unwrap();
    assert!(codex.contains(expected_name));

    let gemini: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(project.path().join(".gemini/settings.json")).unwrap(),
    )
    .unwrap();
    assert!(gemini["mcpServers"][expected_name].is_object());

    let opencode: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(project.path().join("opencode.json")).unwrap())
            .unwrap();
    assert!(opencode["mcp"][expected_name].is_object());
}
