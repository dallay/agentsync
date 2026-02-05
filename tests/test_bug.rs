use agentsync::config::McpServerConfig;
use agentsync::mcp::{ClaudeCodeFormatter, McpFormatter};
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

fn create_test_server() -> McpServerConfig {
    McpServerConfig {
        command: Some("new-cmd".to_string()),
        args: vec![],
        env: std::collections::HashMap::new(),
        url: None,
        headers: std::collections::HashMap::new(),
        transport_type: None,
        disabled: false,
    }
}

// Simple test to reproduce the cleanup bug
#[test]
fn test_merge_cleanup_bug() -> Result<()> {
    let temp_dir = TempDir::new().unwrap();

    // Create existing config with server1
    let existing = r#"{
        "mcpServers": {
            "server1": {
                "command": "old-cmd"
            },
            "server2": {
                "command": "old-cmd"
            }
        }
    }"#;
    fs::write(temp_dir.path().join(".mcp.json"), existing).unwrap();

    // New config only has server3 (server1 and server2 should be REMOVED)
    let new_servers = HashMap::from([("server3".to_string(), create_test_server())]);

    let formatter = ClaudeCodeFormatter;

    // This should call cleanup and remove server1 and server2
    let refs = new_servers.iter().map(|(k, v)| (k.clone(), v)).collect();
    let result = formatter.cleanup_removed_servers(existing, &refs).unwrap();

    // Parse result and verify
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let mcp_servers = parsed.get("mcpServers").unwrap().as_object().unwrap();

    // server1 and server2 should be GONE, server3 should exist
    assert!(mcp_servers.get("server1").is_none());
    assert!(mcp_servers.get("server2").is_none());
    assert!(mcp_servers.get("server3").is_some());

    Ok(())
}
