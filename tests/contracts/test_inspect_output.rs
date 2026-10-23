use serde_json::Value;
use std::process::Command;
use tempfile::TempDir;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_agentsync")
}

fn project() -> TempDir {
    let project = TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".agents/skills/demo")).unwrap();
    std::fs::create_dir_all(project.path().join(".agents/commands")).unwrap();
    std::fs::write(
        project.path().join(".agents/AGENTS.md"),
        "fixture instructions\n",
    )
    .unwrap();
    std::fs::write(
        project.path().join(".agents/skills/demo/SKILL.md"),
        "fixture skill\n",
    )
    .unwrap();
    std::fs::write(
        project.path().join(".agents/commands/review.md"),
        "fixture command\n",
    )
    .unwrap();
    std::fs::write(
        project.path().join(".agents/agentsync.toml"),
        r#"
source_dir = "."
[mcp]
enabled = true
merge_strategy = "merge"
[mcp_servers.fixture]
command = "fixture-mcp"
args = ["--token", "CANARY_ARG_SECRET", "--api-key=CANARY_INLINE_SECRET", "${MCP_TOKEN}"]
url = "https://user:CANARY_USERINFO_SECRET@example.invalid/path?token=CANARY_URL_SECRET"
[mcp_servers.fixture.env]
API_TOKEN = "CANARY_ENV_SECRET"
LOG_LEVEL = "debug"
[agents.opencode]
enabled = true
[agents.opencode.targets.instructions]
source = "AGENTS.md"
destination = "AGENTS.md"
type = "symlink"
[agents.opencode.targets.skills]
source = "skills"
destination = ".opencode/skills"
type = "symlink-contents"
[agents.opencode.targets.commands]
source = "commands"
destination = ".opencode/command"
type = "symlink-contents"
[agents.codex]
enabled = true
[agents.codex.targets.instructions]
source = "AGENTS.md"
destination = "AGENTS.md"
type = "symlink"
[agents.codex.targets.skills]
source = "skills"
destination = ".codex/skills"
type = "symlink-contents"
[agents.codex.targets.commands]
source = "commands"
destination = ".codex/commands"
type = "symlink-contents"
"#,
    )
    .unwrap();
    project
}

fn inspect(project: &TempDir, agent: &str, strict: bool) -> std::process::Output {
    let mut command = Command::new(binary());
    command.args(["inspect", "--agent", agent, "--json", "--project-root"]);
    command.arg(project.path());
    if strict {
        command.arg("--strict");
    }
    command.output().unwrap()
}

#[test]
fn inspect_json_is_stable_read_only_and_secret_safe() {
    let project = project();
    let before = std::fs::read_dir(project.path()).unwrap().count();
    let first = inspect(&project, "opencode", false);
    let second = inspect(&project, "opencode", false);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(first.stdout, second.stdout);
    assert!(first.stderr.is_empty());
    let value: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["scope"], "agentsync-managed");
    assert!(
        value["scope_notice"]
            .as_str()
            .unwrap()
            .contains("not a claim")
    );
    assert!(
        value["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["kind"] == "instructions")
    );
    assert!(
        value["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["kind"] == "skills")
    );
    assert!(
        value["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["kind"] == "commands")
    );
    assert!(
        String::from_utf8_lossy(&first.stdout)
            .find("CANARY_ENV_SECRET")
            .is_none()
    );
    assert!(
        String::from_utf8_lossy(&first.stdout)
            .find("CANARY_ARG_SECRET")
            .is_none()
    );
    assert!(
        String::from_utf8_lossy(&first.stdout)
            .find("CANARY_INLINE_SECRET")
            .is_none()
    );
    assert!(!String::from_utf8_lossy(&first.stdout).contains("${MCP_TOKEN}"));
    assert!(!String::from_utf8_lossy(&first.stdout).contains("CANARY_USERINFO_SECRET"));
    assert!(!String::from_utf8_lossy(&first.stdout).contains("CANARY_URL_SECRET"));
    assert_eq!(std::fs::read_dir(project.path()).unwrap().count(), before);
    assert!(!project.path().join("AGENTS.md").exists());
    assert!(!project.path().join(".opencode").exists());
}

#[test]
fn inspect_codex_projects_canonical_sources_and_destinations() {
    let project = project();
    let output = inspect(&project, "codex", false);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let instructions = value["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["kind"] == "instructions")
        .unwrap();
    assert_eq!(instructions["source"], ".agents/AGENTS.md");
    assert_eq!(instructions["destination"], "AGENTS.md");
    assert_eq!(instructions["sync_mode"], "symlink");
    assert_eq!(value["mcp"]["capability"], "managed");
    assert_eq!(value["mcp"]["servers"][0]["name"], "fixture");
}

#[test]
fn inspect_default_is_report_only_and_strict_fails_for_missing_source() {
    let project = project();
    std::fs::remove_file(project.path().join(".agents/AGENTS.md")).unwrap();
    let default = inspect(&project, "codex", false);
    assert!(default.status.success());
    let value: Value = serde_json::from_slice(&default.stdout).unwrap();
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "missing-source")
    );
    let strict = inspect(&project, "codex", true);
    assert_eq!(strict.status.code(), Some(1));
}

#[test]
fn inspect_broken_symlink_has_distinct_typed_diagnostic() {
    let project = project();
    let path = project.path().join("AGENTS.md");
    #[cfg(unix)]
    std::os::unix::fs::symlink("missing-source.md", &path).unwrap();
    let output = inspect(&project, "codex", false);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "broken-symlink")
    );
}

#[test]
fn inspect_unknown_agent_reports_unknown_managed_scope_without_failure() {
    let project = project();
    let output = inspect(&project, "unconfigured", false);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["items"], serde_json::json!([]));
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "agent-not-configured")
    );
}

#[test]
fn inspect_reports_overwrite_without_claiming_unmanaged_servers_survive() {
    let project = project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let mut config = std::fs::read_to_string(&config_path).unwrap();
    config = config.replace(
        "merge_strategy = \"merge\"",
        "merge_strategy = \"overwrite\"",
    );
    std::fs::write(&config_path, config).unwrap();
    std::fs::create_dir_all(project.path().join(".codex")).unwrap();
    std::fs::write(
        project.path().join(".codex/config.toml"),
        "[mcp_servers.local-only]\ncommand = 'local'\n",
    )
    .unwrap();

    let output = inspect(&project, "codex", false);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["mcp"]["merge_strategy"], "overwrite");
    assert!(
        value["mcp"]["existing_server_names"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn inspect_disabled_mcp_does_not_claim_servers_are_projected() {
    let project = project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let config = std::fs::read_to_string(&config_path).unwrap().replace(
        "enabled = true\nmerge_strategy",
        "enabled = false\nmerge_strategy",
    );
    std::fs::write(config_path, config).unwrap();

    let output = inspect(&project, "codex", false);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value["mcp"]["servers"].as_array().unwrap().is_empty());
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| { diagnostic["code"] == "disabled-mcp-server" })
    );
}

#[test]
fn inspect_redacts_sensitive_assignment_variants() {
    let project = project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let config = std::fs::read_to_string(&config_path).unwrap().replace(
        "\"${MCP_TOKEN}\"]",
        "\"--authorization:CANARY_AUTH_SECRET\", \"--client-secret=CANARY_CLIENT_SECRET\", \"${MCP_TOKEN}\"]",
    );
    std::fs::write(config_path, config).unwrap();

    let output = inspect(&project, "codex", false);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("CANARY_AUTH_SECRET"));
    assert!(!stdout.contains("CANARY_CLIENT_SECRET"));
}
