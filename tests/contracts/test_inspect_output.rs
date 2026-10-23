use serde_json::Value;
use sha2::{Digest, Sha256};
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
url = "https://user:CANARY_USERINFO_SECRET@example.invalid/path?token=CANARY_URL_SECRET&api_key=CANARY_QUERY_KEY_SECRET"
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

fn inspect_plugin_project() -> TempDir {
    let project = project();
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugin-marketplace");
    copy_tree(&fixture, &project.path().join("marketplace"));
    let config_path = project.path().join(".agents/agentsync.toml");
    let mut config = std::fs::read_to_string(&config_path).unwrap();
    config.push_str(
        r#"
[plugins]
enabled = true
lockfile = "plugins.lock.toml"
allowed_mcp = ["plugin/internal/engineering/safe-fixture"]

[plugins.marketplaces.internal]
source = "../marketplace"
reference = "main"

[[plugins.selections]]
marketplace = "internal"
plugin = "engineering"
"#,
    );
    std::fs::write(config_path, config).unwrap();
    let add = Command::new(binary())
        .args(["plugin", "--project-root"])
        .arg(project.path())
        .args(["add", "internal/engineering"])
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "plugin add failed: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    project
}

fn inspect_git_plugin_project() -> (TempDir, std::path::PathBuf) {
    let project = inspect_plugin_project();
    let lock_path = project.path().join(".agents/plugins.lock.toml");
    let mut lock = agentsync::plugins::PluginLock::load(&lock_path).unwrap();
    let location = "https://example.invalid/marketplace.git";
    let revision = "0123456789abcdef0123456789abcdef01234567";
    for plugin in lock.plugins.values_mut() {
        plugin.source.kind = agentsync::plugins::LockedSourceKind::Git;
        plugin.source.location = location.into();
        plugin.source.revision = revision.into();
        plugin.provenance.resolved_revision = revision.into();
    }
    lock.save_atomic(&lock_path).unwrap();
    let digest = Sha256::digest(location.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let snapshot = project
        .path()
        .join(".agents/.agentsync-plugin-sources")
        .join(format!("{}-{revision}", &digest[..16]));
    (project, snapshot)
}

fn copy_tree(source: &std::path::Path, target: &std::path::Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &target_path);
        } else {
            std::fs::create_dir_all(target_path.parent().unwrap()).unwrap();
            std::fs::copy(source_path, target_path).unwrap();
        }
    }
}

fn inspect(project: &TempDir, agent: &str, strict: bool) -> std::process::Output {
    let mut command = Command::new(binary());
    command.args([
        "inspect",
        "--agent",
        if agent == "human" { "opencode" } else { agent },
        "--project-root",
    ]);
    command.arg(project.path());
    if agent != "human" {
        command.arg("--json");
    }
    if strict {
        command.arg("--strict");
    }
    command.output().unwrap()
}

#[test]
fn inspect_projects_approved_plugin_mcp_and_reports_pending_plugin_mcp() {
    let project = inspect_plugin_project();

    let config_path = project.path().join(".agents/agentsync.toml");
    let config = std::fs::read_to_string(&config_path).unwrap().replace(
        "allowed_mcp = [\"plugin/internal/engineering/safe-fixture\"]",
        "allowed_mcp = []",
    );
    std::fs::write(config_path, config).unwrap();

    let output = inspect(&project, "opencode", false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let servers = value["mcp"]["servers"].as_array().unwrap();
    let plugin_server = servers
        .iter()
        .find(|server| server["name"] == "plugin/internal/engineering/safe-fixture")
        .unwrap();
    assert_eq!(plugin_server["origin"], "plugin");
    assert_eq!(plugin_server["state"], "configured");
    assert_eq!(plugin_server["included"], false);
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| {
                diagnostic["code"] == "pending-plugin-approval"
                    && diagnostic["severity"] == "warning"
            })
    );

    let config_path = project.path().join(".agents/agentsync.toml");
    let config = std::fs::read_to_string(&config_path).unwrap().replace(
        "allowed_mcp = []",
        "allowed_mcp = [\"plugin/internal/engineering/safe-fixture\"]",
    );
    std::fs::write(config_path, config).unwrap();
    let output = inspect(&project, "opencode", false);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let plugin_server = value["mcp"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|server| server["name"] == "plugin/internal/engineering/safe-fixture")
        .unwrap();
    assert_eq!(plugin_server["state"], "projected");
    assert_eq!(plugin_server["included"], true);
}

#[test]
fn inspect_reports_plugin_mcp_collision_with_explicit_server() {
    let project = inspect_plugin_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let mut config = std::fs::read_to_string(&config_path).unwrap();
    config.push_str(
        r#"
[mcp_servers."plugin/internal/engineering/safe-fixture"]
command = "/bin/true"
"#,
    );
    std::fs::write(&config_path, &config).unwrap();

    let output = inspect(&project, "opencode", false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let plugin_server = value["mcp"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|server| {
            server["name"] == "plugin/internal/engineering/safe-fixture"
                && server["origin"] == "plugin"
        })
        .unwrap();
    assert_eq!(plugin_server["included"], false);
    assert_eq!(plugin_server["state"], "conflicted");
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| {
                diagnostic["code"] == "mcp-name-collision" && diagnostic["severity"] == "error"
            })
    );

    let config = config.replace(
        "allowed_mcp = [\"plugin/internal/engineering/safe-fixture\"]",
        "allowed_mcp = []",
    );
    std::fs::write(config_path, config).unwrap();
    let output = inspect(&project, "opencode", false);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let plugin_server = value["mcp"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|server| server["origin"] == "plugin")
        .unwrap();
    assert_eq!(plugin_server["included"], false);
    assert_eq!(plugin_server["state"], "conflicted");
}

#[test]
fn inspect_reports_plugin_error_when_lockfile_is_missing() {
    let project = inspect_plugin_project();
    std::fs::remove_file(project.path().join(".agents/plugins.lock.toml")).unwrap();

    let output = inspect(&project, "opencode", false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| {
                diagnostic["code"] == "unsupported-plugin-component"
                    && diagnostic["severity"] == "warning"
            })
    );
}

#[test]
fn inspect_reports_missing_plugin_snapshot_from_git_lock_entry() {
    let (project, snapshot) = inspect_git_plugin_project();
    assert!(!snapshot.exists());

    let output = inspect(&project, "opencode", false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| {
                diagnostic["code"] == "missing-plugin-snapshot"
                    && diagnostic["severity"] == "warning"
            }),
        "diagnostics: {}",
        value["diagnostics"]
    );
}

#[test]
fn inspect_classifies_plugin_failures_by_the_actual_error() {
    let (project, snapshot) = inspect_git_plugin_project();
    let fixture = project.path().join("marketplace");
    copy_tree(&fixture, &snapshot);
    let before = std::fs::read(project.path().join(".agents/plugins.lock.toml")).unwrap();
    let manifest = snapshot.join(".agents/plugins/marketplace.json");
    assert!(manifest.is_file());
    std::fs::write(&manifest, "invalid json").unwrap();

    let output = inspect(&project, "opencode", false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostics = value["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"] == "unsupported-plugin-component")
    );
    assert!(
        !diagnostics
            .iter()
            .any(|d| d["code"] == "missing-plugin-snapshot")
    );
    assert_eq!(std::fs::read(manifest).unwrap(), b"invalid json");
    assert_eq!(
        std::fs::read(project.path().join(".agents/plugins.lock.toml")).unwrap(),
        before
    );
    assert!(!project.path().join(".agents/plugins").exists());
}

#[test]
fn inspect_reports_stale_approvals_before_missing_git_snapshot() {
    let (project, snapshot) = inspect_git_plugin_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let config = std::fs::read_to_string(&config_path).unwrap().replace(
        "plugin/internal/engineering/safe-fixture",
        "plugin/internal/engineering/unknown",
    );
    std::fs::write(config_path, config).unwrap();
    let output = inspect(&project, "opencode", false);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostics = value["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"] == "unsupported-plugin-component")
    );
    assert!(
        !diagnostics
            .iter()
            .any(|d| d["code"] == "missing-plugin-snapshot")
    );
    assert!(!snapshot.exists());
}

#[test]
fn inspect_reports_invalid_plugin_lock_path_without_aborting() {
    let project = inspect_plugin_project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let config = std::fs::read_to_string(&config_path).unwrap().replace(
        "lockfile = \"plugins.lock.toml\"",
        "lockfile = \"../plugins.lock.toml\"",
    );
    std::fs::write(config_path, config).unwrap();
    let output = inspect(&project, "opencode", false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostic = value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "unsupported-plugin-component")
        .unwrap();
    assert!(diagnostic["path"].is_null());
    assert!(!project.path().join("plugins.lock.toml").exists());
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
    let url = value["mcp"]["servers"][0]["url"].as_str().unwrap();
    assert!(url.contains("example.invalid/path"), "redacted URL: {url}");
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
fn inspect_module_map_projects_each_mapping_as_child() {
    let project = TempDir::new().unwrap();
    let agents = project.path().join(".agents/source");
    std::fs::create_dir_all(&agents).unwrap();
    std::fs::write(agents.join("first.md"), "first\n").unwrap();
    std::fs::write(agents.join("second.md"), "second\n").unwrap();
    std::fs::write(
        project.path().join(".agents/agentsync.toml"),
        r#"
source_dir = "source"
[agents.codex]
enabled = true
[agents.codex.targets.modules]
source = "placeholder-source"
destination = "placeholder-destination"
type = "module-map"
[[agents.codex.targets.modules.mappings]]
source = "first.md"
destination = "src/one"
[[agents.codex.targets.modules.mappings]]
source = "second.md"
destination = "src/two"
"#,
    )
    .unwrap();
    let apply = Command::new(binary())
        .current_dir(project.path())
        .arg("apply")
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let output = inspect(&project, "codex", false);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let item = value["items"].as_array().unwrap().first().unwrap();
    let children = item["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0]["source"], ".agents/source/first.md");
    assert_eq!(children[0]["destination"], "src/one/AGENTS.md");
    assert_eq!(children[1]["source"], ".agents/source/second.md");
    assert_eq!(children[1]["destination"], "src/two/AGENTS.md");

    std::fs::remove_file(agents.join("first.md")).unwrap();
    std::fs::remove_file(agents.join("second.md")).unwrap();
    let output = inspect(&project, "codex", false);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let broken_paths = value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "broken-symlink")
        .map(|d| d["path"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(broken_paths, ["src/one/AGENTS.md", "src/two/AGENTS.md"]);
}

#[test]
fn inspect_human_diagnostics_show_code_severity_target_and_path() {
    let project = project();
    std::fs::remove_file(project.path().join(".agents/AGENTS.md")).unwrap();
    let output = inspect(&project, "human", false);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("MissingSource [Error]"));
    assert!(stdout.contains("target=instructions"));
    assert!(stdout.contains("path=.agents/AGENTS.md"));
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
    #[cfg(unix)]
    {
        let path = project.path().join("AGENTS.md");
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
fn inspect_malformed_existing_mcp_config_falls_back_to_projection() {
    let project = project();
    std::fs::create_dir_all(project.path().join(".codex")).unwrap();
    std::fs::write(
        project.path().join(".codex/config.toml"),
        "mcp_servers = [\n",
    )
    .unwrap();

    let output = inspect(&project, "codex", false);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["mcp"]["servers"][0]["name"], "fixture");
    assert_eq!(value["mcp"]["servers"][0]["state"], "projected");
    assert!(
        value["mcp"]["existing_server_names"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn inspect_disabled_server_is_configured_and_not_included() {
    let project = project();
    let config_path = project.path().join(".agents/agentsync.toml");
    let config = std::fs::read_to_string(&config_path).unwrap().replace(
        "command = \"fixture-mcp\"",
        "command = \"fixture-mcp\"\ndisabled = true",
    );
    std::fs::write(config_path, config).unwrap();

    let output = inspect(&project, "codex", false);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let server = &value["mcp"]["servers"][0];
    assert_eq!(server["disabled"], true);
    assert_eq!(server["included"], false);
    assert_eq!(server["state"], "configured");
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "disabled-mcp-server")
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
