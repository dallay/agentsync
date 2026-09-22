use agentsync::config::{McpMergeStrategy, McpServerConfig};
use agentsync::mcp::{McpAgent, McpGenerator};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn issue_595_registers_zcode_and_minimax() {
    assert_eq!(McpAgent::from_id("z-code"), Some(McpAgent::ZCode));
    assert_eq!(McpAgent::from_id("minimax-code"), Some(McpAgent::MiniMax));
    assert_eq!(McpAgent::from_id("zcode"), Some(McpAgent::ZCode));
    assert_eq!(McpAgent::from_id("minimax"), Some(McpAgent::MiniMax));
    assert_eq!(McpAgent::ZCode.id(), "zcode");
    assert_eq!(McpAgent::ZCode.name(), "Z-Code");
    assert_eq!(McpAgent::MiniMax.id(), "minimax");
    assert_eq!(McpAgent::MiniMax.name(), "MiniMax Code");
    assert_eq!(McpAgent::ZCode.config_path(), ".zcode/config.json");
    assert_eq!(McpAgent::MiniMax.config_path(), ".mcp.json");
}

#[test]
fn issue_595_zcode_commands_drop_agent_suffix() {
    assert_eq!(
        agentsync::zcode_command_destination("review.agent.md"),
        "review.md"
    );
    assert_eq!(
        agentsync::zcode_command_destination("simple.md"),
        "simple.md"
    );
}

#[cfg(unix)]
#[test]
fn issue_595_zcode_commands_sync_and_status_use_normalized_name() {
    use agentsync::config::Config;
    use agentsync::linker::{Linker, SyncOptions};
    let temp = TempDir::new().unwrap();
    let source_dir = temp.path().join(".agents");
    let command_dir = source_dir.join("commands");
    fs::create_dir_all(&command_dir).unwrap();
    fs::write(command_dir.join("review.agent.md"), "review").unwrap();
    let config_path = temp.path().join("agentsync.toml");
    fs::write(
        &config_path,
        r#"
            source_dir = ".agents"
            [agents.z-code]
            enabled = true
            [agents.z-code.targets.commands]
            source = "commands"
            destination = ".zcode/commands"
            type = "symlink-contents"
            pattern = "*.agent.md"
        "#,
    )
    .unwrap();

    let config = Config::load(&config_path).unwrap();
    let linker = Linker::new(config, config_path);
    linker.sync(&SyncOptions::default()).unwrap();

    let destination = temp.path().join(".zcode/commands/review.md");
    assert!(destination.is_symlink());
    assert!(!temp.path().join(".zcode/commands/review.agent.md").exists());

    let output = Command::new(env!("CARGO_BIN_EXE_agentsync"))
        .current_dir(temp.path())
        .arg("status")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn issue_595_zcode_mcp_preserves_unrelated_config() {
    let temp = TempDir::new().unwrap();
    let zcode_dir = temp.path().join(".zcode");
    fs::create_dir_all(&zcode_dir).unwrap();
    fs::write(
        zcode_dir.join("config.json"),
        serde_json::to_string_pretty(
            &json!({"model": "glm", "mcp": {"servers": {"old": {"command": "old"}}}}),
        )
        .unwrap(),
    )
    .unwrap();

    let server = McpServerConfig {
        command: Some("npx".to_string()),
        args: vec!["server".to_string()],
        url: None,
        headers: BTreeMap::new(),
        env: BTreeMap::new(),
        transport_type: None,
        disabled: false,
    };
    let generator = McpGenerator::new(
        [("filesystem".to_string(), server)],
        McpMergeStrategy::Merge,
    );

    generator
        .generate_for_agent(McpAgent::ZCode, temp.path(), false)
        .unwrap();

    let content = fs::read_to_string(zcode_dir.join("config.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(value["model"], "glm");
    assert!(value["mcp"]["servers"]["filesystem"].is_object());
}

#[test]
fn issue_595_zcode_mcp_format_and_minimax_mcp_format_are_supported() {
    let server = McpServerConfig {
        command: Some("npx".to_string()),
        args: vec!["server".to_string()],
        url: None,
        headers: BTreeMap::new(),
        env: BTreeMap::new(),
        transport_type: None,
        disabled: false,
    };
    let servers = BTreeMap::from([("filesystem".to_string(), server)]);
    let zcode = McpAgent::ZCode.formatter();
    let minimax = McpAgent::MiniMax.formatter();
    let zcode_servers = zcode.format(
        &servers
            .iter()
            .map(|(name, config)| (name.as_str(), config))
            .collect(),
    );
    let minimax_servers = minimax.format(
        &servers
            .iter()
            .map(|(name, config)| (name.as_str(), config))
            .collect(),
    );
    assert!(zcode_servers["mcp"]["servers"]["filesystem"].is_object());
    assert!(minimax_servers["mcpServers"]["filesystem"].is_object());

    let parsed_zcode = zcode.parse_existing(&zcode_servers.to_string()).unwrap();
    let parsed_minimax = minimax
        .parse_existing(&minimax_servers.to_string())
        .unwrap();
    assert!(parsed_zcode.contains_key("filesystem"));
    assert!(parsed_minimax.contains_key("filesystem"));
}

#[test]
fn issue_595_zcode_mcp_overwrite_accepts_config_without_servers() {
    let temp = TempDir::new().unwrap();
    let zcode_dir = temp.path().join(".zcode");
    fs::create_dir_all(&zcode_dir).unwrap();
    fs::write(
        zcode_dir.join("config.json"),
        serde_json::to_string_pretty(&json!({"model": "glm"})).unwrap(),
    )
    .unwrap();

    let server = McpServerConfig {
        command: Some("npx".to_string()),
        args: vec!["server".to_string()],
        url: None,
        headers: BTreeMap::new(),
        env: BTreeMap::new(),
        transport_type: None,
        disabled: false,
    };
    let generator = McpGenerator::new(
        [("filesystem".to_string(), server)],
        McpMergeStrategy::Overwrite,
    );

    generator
        .generate_for_agent(McpAgent::ZCode, temp.path(), false)
        .unwrap();

    let content = fs::read_to_string(zcode_dir.join("config.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(value["model"], "glm");
    assert!(value["mcp"]["servers"]["filesystem"].is_object());
}

#[test]
fn issue_595_zcode_mcp_accepts_config_without_servers() {
    let temp = TempDir::new().unwrap();
    let zcode_dir = temp.path().join(".zcode");
    fs::create_dir_all(&zcode_dir).unwrap();
    fs::write(
        zcode_dir.join("config.json"),
        serde_json::to_string_pretty(&json!({"model": "glm"})).unwrap(),
    )
    .unwrap();

    let server = McpServerConfig {
        command: Some("npx".to_string()),
        args: vec!["server".to_string()],
        url: None,
        headers: BTreeMap::new(),
        env: BTreeMap::new(),
        transport_type: None,
        disabled: false,
    };
    let generator = McpGenerator::new(
        [("filesystem".to_string(), server)],
        McpMergeStrategy::Merge,
    );

    generator
        .generate_for_agent(McpAgent::ZCode, temp.path(), false)
        .unwrap();

    let content = fs::read_to_string(zcode_dir.join("config.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(value["model"], "glm");
    assert!(value["mcp"]["servers"]["filesystem"].is_object());
}
