use agentsync::config::McpServerConfig;
use agentsync::mcp::{ClaudeCodeFormatter, McpFormatter};
use anyhow::Result;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn agentsync_bin() -> &'static str {
    env!("CARGO_BIN_EXE_agentsync")
}

fn run_agentsync(project_root: &Path, args: &[&str]) -> Output {
    Command::new(agentsync_bin())
        .current_dir(project_root)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to run agentsync {args:?}: {error}"))
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed: status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_apply_fixture(project_root: &Path, gitignore_enabled: bool, marker: &str) {
    fs::create_dir_all(project_root.join(".agents/claude")).unwrap();
    fs::write(project_root.join(".agents/claude/AGENTS.md"), "# Agent\n").unwrap();
    fs::write(
        project_root.join(".agents/agentsync.toml"),
        format!(
            r#"source_dir = "claude"

[gitignore]
enabled = {gitignore_enabled}
marker = "{marker}"

[agents.claude]
enabled = true

[agents.claude.targets.instructions]
source = "AGENTS.md"
destination = "AGENTS.md"
type = "symlink"
"#
        ),
    )
    .unwrap();
}

fn stale_block(marker: &str) -> String {
    format!(
        "node_modules/\n# START {marker}\n/AGENTS.md\n/AGENTS.md.bak\n/.mcp.json\n# END {marker}\ndist/\n"
    )
}

fn create_test_server() -> McpServerConfig {
    McpServerConfig {
        command: Some("new-cmd".to_string()),
        args: vec![],
        env: std::collections::BTreeMap::new(),
        url: None,
        headers: std::collections::BTreeMap::new(),
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
    let new_servers = BTreeMap::from([("server3".to_string(), create_test_server())]);

    let formatter = ClaudeCodeFormatter;

    // This should call cleanup and remove server1 and server2
    let refs = new_servers.iter().map(|(k, v)| (k.as_str(), v)).collect();
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

#[test]
fn test_apply_removes_stale_gitignore_block_when_disabled() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_apply_fixture(project_root, false, "AI Agent Symlinks");
    fs::write(
        project_root.join(".gitignore"),
        stale_block("AI Agent Symlinks"),
    )
    .unwrap();

    let output = run_agentsync(project_root, &["apply"]);
    assert_success(&output, "agentsync apply");

    let gitignore = fs::read_to_string(project_root.join(".gitignore")).unwrap();
    assert_eq!(gitignore, "node_modules/\ndist/\n");
}

#[test]
fn test_apply_cleanup_is_idempotent_when_disabled() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_apply_fixture(project_root, false, "AI Agent Symlinks");
    fs::write(
        project_root.join(".gitignore"),
        stale_block("AI Agent Symlinks"),
    )
    .unwrap();

    let first = run_agentsync(project_root, &["apply"]);
    assert_success(&first, "initial agentsync apply");
    let cleaned = fs::read_to_string(project_root.join(".gitignore")).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));
    let mtime_before = fs::metadata(project_root.join(".gitignore"))
        .unwrap()
        .modified()
        .unwrap();
    let second = run_agentsync(project_root, &["apply"]);
    assert_success(&second, "repeat agentsync apply");
    let mtime_after = fs::metadata(project_root.join(".gitignore"))
        .unwrap()
        .modified()
        .unwrap();
    let current = fs::read_to_string(project_root.join(".gitignore")).unwrap();

    assert_eq!(current, cleaned);
    assert_eq!(mtime_before, mtime_after);
}

#[test]
fn test_apply_disabled_gitignore_dry_run_and_no_gitignore_variants() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_apply_fixture(project_root, false, "Custom Marker");
    let original = "target/\n# START Other Marker\nkeep\n# END Other Marker\n# START Custom Marker\nremove\n# END Custom Marker\n".to_string();
    fs::write(project_root.join(".gitignore"), &original).unwrap();

    let dry_run = run_agentsync(project_root, &["apply", "--dry-run"]);
    assert_success(&dry_run, "agentsync apply --dry-run");
    let dry_run_stdout = String::from_utf8_lossy(&dry_run.stdout);
    assert!(dry_run_stdout.contains("Would remove managed .gitignore section"));
    assert_eq!(
        fs::read_to_string(project_root.join(".gitignore")).unwrap(),
        original
    );

    let no_gitignore = run_agentsync(project_root, &["apply", "--no-gitignore"]);
    assert_success(&no_gitignore, "agentsync apply --no-gitignore");
    assert_eq!(
        fs::read_to_string(project_root.join(".gitignore")).unwrap(),
        original
    );

    let dry_run_no_gitignore =
        run_agentsync(project_root, &["apply", "--dry-run", "--no-gitignore"]);
    assert_success(
        &dry_run_no_gitignore,
        "agentsync apply --dry-run --no-gitignore",
    );
    let dry_run_no_gitignore_stdout = String::from_utf8_lossy(&dry_run_no_gitignore.stdout);
    assert!(!dry_run_no_gitignore_stdout.contains("Would remove managed .gitignore section"));
    assert_eq!(
        fs::read_to_string(project_root.join(".gitignore")).unwrap(),
        original
    );
}

#[test]
fn test_apply_disabled_gitignore_dry_run_without_matching_block_is_silent_noop() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_apply_fixture(project_root, false, "Custom Marker");
    let original = "target/\n# START Other Marker\nkeep\n# END Other Marker\n";
    fs::write(project_root.join(".gitignore"), original).unwrap();

    let output = run_agentsync(project_root, &["apply", "--dry-run"]);
    assert_success(&output, "agentsync apply --dry-run without matching block");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("Would remove managed .gitignore section"));
    assert_eq!(
        fs::read_to_string(project_root.join(".gitignore")).unwrap(),
        original
    );
}
