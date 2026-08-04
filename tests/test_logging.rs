#![cfg(unix)]
//! Black-box tests for structured diagnostics (#499).
//!
//! Core contract: log events ALWAYS go to stderr; functional stdout (human
//! output and the `--json` contracts) is never polluted by tracing lines.

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
        .unwrap_or_else(|error| panic!("failed to run agentsync {:?}: {error}", args))
}

/// Write a minimal `.agents` fixture that `apply` can sync without network access.
fn write_apply_fixture(root: &Path) {
    let agents_dir = root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("AGENTS.md"), "# Agent instructions\n").unwrap();
    fs::write(
        agents_dir.join("agentsync.toml"),
        r#"
        [agents.claude]
        enabled = true
        [agents.claude.targets.instructions]
        source = "AGENTS.md"
        destination = "CLAUDE.md"
        type = "symlink"
    "#,
    )
    .unwrap();
}

/// Assert that `stdout` contains no tracing output: no ANSI escapes, no level
/// markers, and no leading ISO timestamps.
fn assert_stdout_has_no_tracing(stdout: &str) {
    for line in stdout.lines() {
        assert!(
            !line.contains("\u{1b}["),
            "stdout line contains a tracing ANSI escape: {line:?}"
        );
        for marker in [" INFO ", " WARN ", " ERROR ", " DEBUG ", " TRACE "] {
            assert!(
                !line.contains(marker),
                "stdout line contains a tracing level marker: {line:?}"
            );
        }
        let bytes = line.as_bytes();
        let timestamp_headed =
            bytes.len() >= 5 && bytes[..4].iter().all(u8::is_ascii_digit) && bytes[4] == b'-';
        assert!(
            !timestamp_headed,
            "stdout line starts with a tracing timestamp: {line:?}"
        );
    }
}

/// Parse newline-delimited JSON log events from stderr.
fn json_events(stderr: &str) -> Vec<serde_json::Value> {
    stderr
        .lines()
        .filter(|line| line.starts_with('{'))
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect()
}

/// Collect every (key, value) pair from the `fields` object of an event and
/// from the span objects (`span`/`spans`). Robust to how tracing-subscriber
/// distributes span fields: nested under `span.fields` or flat on the span
/// object (name/operation/outcome) depending on format version.
fn span_field_pairs(event: &serde_json::Value) -> Vec<(String, serde_json::Value)> {
    let mut pairs = Vec::new();
    let mut collect = |obj: Option<&serde_json::Map<String, serde_json::Value>>| {
        if let Some(fields) = obj {
            for (key, value) in fields {
                pairs.push((key.clone(), value.clone()));
            }
        }
    };
    collect(event.get("fields").and_then(serde_json::Value::as_object));
    if let Some(span) = event.get("span").and_then(serde_json::Value::as_object) {
        collect(span.get("fields").and_then(serde_json::Value::as_object));
        collect(Some(span));
    }
    if let Some(spans) = event.get("spans").and_then(serde_json::Value::as_array) {
        for span in spans {
            let span = span.as_object().expect("span list entries are objects");
            collect(span.get("fields").and_then(serde_json::Value::as_object));
            collect(Some(span));
        }
    }
    pairs
}

fn has_field(event: &serde_json::Value, key: &str, value: &str) -> bool {
    span_field_pairs(event).contains(&(
        key.to_string(),
        serde_json::Value::String(value.to_string()),
    ))
}

fn has_field_with_suffix(event: &serde_json::Value, key: &str, suffix: &str) -> bool {
    span_field_pairs(event).iter().any(|(field, value)| {
        field == key && value.as_str().is_some_and(|value| value.ends_with(suffix))
    })
}

#[test]
fn apply_default_logs_are_human_and_never_on_stdout() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_apply_fixture(project_root);

    let output = run_agentsync(project_root, &["apply"]);
    assert!(
        output.status.success(),
        "apply failed: status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Functional stdout is intact (the latent #499 bug would mix tracing into it).
    assert!(stdout.contains("Sync"), "stdout: {stdout}");
    assert!(stdout.contains("claude"), "stdout: {stdout}");
    assert_stdout_has_no_tracing(&stdout);

    // stderr carries no functional output; a clean apply emits no events in
    // the default human mode, so stderr stays empty.
    assert!(
        !stderr.contains("Sync"),
        "functional text leaked to stderr: {stderr}"
    );
    assert!(
        !stderr.contains("claude"),
        "functional text leaked to stderr: {stderr}"
    );
}

#[test]
#[cfg(unix)]
fn warn_event_routes_to_stderr_not_stdout_and_stays_plain_when_piped() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // An invalid user config template at $XDG_CONFIG_HOME makes `init` emit a
    // tracing WARN while still completing successfully.
    let xdg_home = project_root.join("xdg");
    let xdg_agentsync = xdg_home.join("agentsync");
    fs::create_dir_all(&xdg_agentsync).unwrap();
    fs::write(xdg_agentsync.join("config.toml"), "not = [valid toml").unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(project_root)
        .env("XDG_CONFIG_HOME", &xdg_home)
        .args(["init"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "init failed: status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The WARN event is human-readable on stderr...
    assert!(
        stderr.contains("User config template is invalid TOML"),
        "WARN missing from stderr: {stderr}"
    );
    // ...with no ANSI escapes when stderr is piped (spec scenario)...
    assert!(
        !stderr.contains("\u{1b}["),
        "piped stderr must not contain ANSI escapes: {stderr:?}"
    );
    // ...and never leaks to stdout.
    assert_stdout_has_no_tracing(&stdout);
    assert!(
        !stdout.contains("User config template is invalid TOML"),
        "WARN leaked to stdout: {stdout}"
    );
}

#[test]
#[cfg(unix)]
fn apply_json_emits_root_span_with_operation() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_apply_fixture(project_root);

    let output = run_agentsync(project_root, &["--log-format", "json", "apply"]);
    assert!(
        output.status.success(),
        "apply failed: status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // JSON events are on stderr, never on stdout.
    assert!(
        !stdout.lines().any(|line| line.starts_with('{')),
        "JSON log events leaked to stdout: {stdout}"
    );
    let events = json_events(&stderr);
    assert!(
        !events.is_empty(),
        "no JSON log events on stderr for --log-format json: {stderr}"
    );

    // The root span for the subcommand carries operation=apply.
    let has_root_span = events
        .iter()
        .any(|event| has_field(event, "operation", "apply"));
    assert!(
        has_root_span,
        "no event carries a root span with operation=apply: {stderr}"
    );
}

#[test]
#[cfg(unix)]
fn apply_json_emits_agent_and_target_spans() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_apply_fixture(project_root);

    let output = run_agentsync(project_root, &["apply", "--log-format", "json"]);
    assert!(
        output.status.success(),
        "apply failed: status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let events = json_events(&stderr);
    assert!(
        events.len() >= 2,
        "expected span-close events for agent and target: {stderr}"
    );

    // The per-agent span carries agent_id, the per-target span carries
    // target and the configured destination path.
    let has_agent_id = events
        .iter()
        .any(|event| has_field(event, "agent_id", "claude"));
    assert!(has_agent_id, "no span carries agent_id=claude: {stderr}");
    let has_target = events
        .iter()
        .any(|event| has_field(event, "target", "instructions"));
    assert!(has_target, "no span carries target=instructions: {stderr}");
    let has_path = events
        .iter()
        .any(|event| has_field(event, "path", "CLAUDE.md"));
    assert!(has_path, "no span carries path=CLAUDE.md: {stderr}");

    // A freshly-created symlink closes the target span with outcome=created.
    let has_created_outcome = events
        .iter()
        .any(|event| has_field(event, "outcome", "created"));
    assert!(
        has_created_outcome,
        "no span carries outcome=created: {stderr}"
    );
}

#[test]
#[cfg(unix)]
fn apply_json_mcp_span_never_leaks_env_headers_or_url() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // MCP fixture with a remote server carrying a credential-style header, a
    // server env map with a token, and a URL with userinfo + query — nothing
    // may appear in log events.
    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("AGENTS.md"), "# Agent instructions\n").unwrap();
    fs::write(
        agents_dir.join("agentsync.toml"),
        r#"
        [mcp]
        enabled = true

        [mcp_servers.remote]
        url = "https://user:pass@example.com/mcp-stream?token=abc123"
        type = "sse"
        headers = { Authorization = "Bearer leak-check-token-42" }

        [mcp_servers.filesystem]
        command = "npx"
        args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
        env = { API_TOKEN = "leak-check-token-42" }

        [agents.claude]
        enabled = true
        [agents.claude.targets.instructions]
        source = "AGENTS.md"
        destination = "CLAUDE.md"
        type = "symlink"
    "#,
    )
    .unwrap();

    let output = run_agentsync(project_root, &["apply", "--log-format", "json"]);
    assert!(
        output.status.success(),
        "apply failed: status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let events = json_events(&stderr);
    assert!(!events.is_empty(), "no JSON log events on stderr: {stderr}");

    // The per-agent MCP generation emits a span with operation=mcp.
    let has_mcp_span = events
        .iter()
        .any(|event| has_field(event, "operation", "mcp"));
    assert!(has_mcp_span, "no span carries operation=mcp: {stderr}");

    // env / headers values, Authorization headers, and URL userinfo+query are
    // never emitted in log events.
    for secret in [
        "leak-check-token-42",
        "Bearer ",
        "user:pass",
        "token=abc123",
    ] {
        assert!(
            !stderr.contains(secret),
            "secret leaked into stderr: {secret:?}\n{stderr}"
        );
    }
}

#[test]
#[cfg(unix)]
fn skill_install_json_emits_span_with_skill_id() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Offline local-dir skill install via --source.
    let source_root = root.join("skill-sources");
    fs::create_dir_all(source_root.join("test-skill")).unwrap();
    fs::write(
        source_root.join("test-skill").join("SKILL.md"),
        "---\nname: test-skill\nversion: 1.0.0\n---\n# Test Skill\n",
    )
    .unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .args([
            "skill",
            "install",
            "test-skill",
            "--source",
            source_root.join("test-skill").to_str().unwrap(),
            "--json",
            "--log-format",
            "json",
        ])
        .output()
        .unwrap_or_else(|error| panic!("failed to run agentsync skill install: {error}"));

    assert!(
        output.status.success(),
        "skill install failed: status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let events = json_events(&stderr);
    assert!(!events.is_empty(), "no JSON log events on stderr: {stderr}");

    // The install emits a span with operation=skill_install and skill_id.
    let has_install_span = events
        .iter()
        .any(|event| has_field(event, "operation", "skill_install"));
    assert!(
        has_install_span,
        "no span carries operation=skill_install: {stderr}"
    );
    let has_skill_id = events
        .iter()
        .any(|event| has_field(event, "skill_id", "test-skill"));
    assert!(
        has_skill_id,
        "no span carries skill_id=test-skill: {stderr}"
    );
}

#[test]
#[cfg(unix)]
fn apply_debug_level_shows_skips_on_stderr() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Two enabled agents; copilot is filtered out via --agents claude.
    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("AGENTS.md"), "# Agent instructions\n").unwrap();
    fs::write(
        agents_dir.join("agentsync.toml"),
        r#"
        [agents.claude]
        enabled = true
        [agents.claude.targets.instructions]
        source = "AGENTS.md"
        destination = "CLAUDE.md"
        type = "symlink"
        [agents.copilot]
        enabled = true
        [agents.copilot.targets.instructions]
        source = "AGENTS.md"
        destination = ".github/copilot-instructions.md"
        type = "symlink"
    "#,
    )
    .unwrap();

    let output = run_agentsync(
        project_root,
        &["apply", "--agents", "claude", "--log-level", "debug"],
    );
    assert!(
        output.status.success(),
        "apply failed: status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The skip diagnostic moved off functional stdout...
    assert!(
        !stdout.contains("Skipping"),
        "skip diagnostic still on stdout: {stdout}"
    );
    // ...and is visible on stderr at debug level, naming the skipped agent.
    assert!(
        stderr.contains("Skipping"),
        "skip diagnostic not visible on stderr: {stderr}"
    );
    assert!(
        stderr.contains("copilot"),
        "skip diagnostic should name the skipped agent: {stderr}"
    );
}

#[test]
#[cfg(unix)]
fn clean_json_emits_removed_span_with_operation_and_path() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    write_apply_fixture(root);

    let apply = run_agentsync(root, &["apply"]);
    assert!(
        apply.status.success(),
        "apply failed: {}",
        String::from_utf8_lossy(&apply.stderr)
    );

    let output = run_agentsync(root, &["clean", "--log-format", "json"]);
    assert!(
        output.status.success(),
        "clean failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let events = json_events(&stderr);
    assert!(
        events.iter().any(|event| {
            has_field(event, "operation", "remove")
                && has_field_with_suffix(event, "path", "CLAUDE.md")
                && has_field(event, "outcome", "removed")
        }),
        "removed clean span missing: {stderr}"
    );
}

#[test]
#[cfg(unix)]
fn apply_json_failed_target_emits_error_outcome_with_context() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let agents_dir = root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("AGENTS.md"), "# instructions\n").unwrap();
    fs::write(
        agents_dir.join("agentsync.toml"),
        r#"
        [agents.claude]
        enabled = true
        [agents.claude.targets.instructions]
        source = "AGENTS.md"
        destination = "/outside/CLAUDE.md"
        type = "symlink"
    "#,
    )
    .unwrap();

    let output = run_agentsync(root, &["apply", "--log-format", "json"]);
    assert!(
        !output.status.success(),
        "apply target errors must exit nonzero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let events = json_events(&stderr);
    assert!(
        events.iter().any(|event| {
            has_field(event, "agent_id", "claude")
                && has_field(event, "path", "/outside/CLAUDE.md")
                && has_field(event, "outcome", "error")
        }),
        "failed target context/outcome missing: {stderr}"
    );
}

#[test]
#[cfg(unix)]
fn apply_json_mcp_failure_emits_agent_and_config_path() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let agents_dir = root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("AGENTS.md"), "# instructions\n").unwrap();
    fs::write(root.join(".mcp.json"), "not valid json").unwrap();
    fs::write(
        agents_dir.join("agentsync.toml"),
        r#"
        [mcp]
        enabled = true
        [mcp_servers.test]
        command = "test-server"
        [agents.claude]
        enabled = true
        [agents.claude.targets.instructions]
        source = "AGENTS.md"
        destination = "CLAUDE.md"
        type = "symlink"
    "#,
    )
    .unwrap();

    let output = run_agentsync(root, &["apply", "--log-format", "json"]);
    assert!(
        output.status.success(),
        "MCP generation preserves apply summary behavior: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let events = json_events(&stderr);
    assert!(
        events.iter().any(|event| {
            has_field(event, "agent", "Claude Code")
                && has_field_with_suffix(event, "config_path", ".mcp.json")
        }),
        "MCP failure context missing: {stderr}"
    );
    assert!(
        events.iter().any(|event| {
            has_field(event, "operation", "mcp") && has_field(event, "outcome", "error")
        }),
        "MCP error outcome missing: {stderr}"
    );
}

#[test]
fn apply_json_failure_exits_nonzero_and_keeps_error_protocol_on_stderr() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let agents_dir = root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("AGENTS.md"), "# instructions\n").unwrap();
    fs::write(
        agents_dir.join("agentsync.toml"),
        r#"
        [agents.claude]
        enabled = true
        [agents.claude.targets.instructions]
        source = "AGENTS.md"
        destination = "/outside/CLAUDE.md"
        type = "symlink"
    "#,
    )
    .unwrap();

    let output = run_agentsync(root, &["apply", "--log-format", "json"]);
    assert!(!output.status.success(), "failed apply must exit nonzero");
    assert_stdout_has_no_tracing(&String::from_utf8_lossy(&output.stdout));
    let stderr = String::from_utf8_lossy(&output.stderr);
    let events = json_events(&stderr);
    assert!(!events.is_empty(), "expected JSON error events: {stderr}");
    assert!(
        events.iter().any(|event| {
            has_field(event, "operation", "apply") && has_field(event, "outcome", "error")
        }),
        "root apply error span missing: {stderr}"
    );
    assert!(
        events
            .iter()
            .any(|event| has_field(event, "outcome", "error")),
        "target error event missing: {stderr}"
    );
    assert!(
        stderr
            .lines()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok()),
        "stderr must contain only JSON events: {stderr}"
    );
    assert!(
        !stderr.contains("Error: "),
        "plaintext top-level error leaked: {stderr}"
    );
}

#[test]
fn status_json_failure_exits_nonzero_with_parseable_stdout_and_json_diagnostics() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    write_apply_fixture(root);

    let output = run_agentsync(root, &["status", "--log-format", "json", "--json"]);
    assert!(!output.status.success(), "drifted status must exit nonzero");
    let entries: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("status --json stdout must remain parseable");
    assert!(
        entries.as_array().is_some(),
        "status output must be an array"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let events = json_events(&stderr);
    assert!(
        !events.is_empty(),
        "expected JSON status diagnostics: {stderr}"
    );
    assert!(
        events.iter().any(|event| {
            has_field(event, "operation", "status") && has_field(event, "outcome", "error")
        }),
        "root status error span missing: {stderr}"
    );
    assert!(
        stderr
            .lines()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok()),
        "stderr must contain only JSON events: {stderr}"
    );
    assert!(
        !stderr.contains("Error: "),
        "plaintext top-level error leaked: {stderr}"
    );
}

#[test]
#[cfg(unix)]
fn skill_suggest_json_warn_stays_on_stderr_and_stdout_remains_parseable() {
    let temp_dir = TempDir::new().unwrap();
    let output = Command::new(agentsync_bin())
        .current_dir(temp_dir.path())
        .env("AGENTSYNC_TEST_INVALID_RECOMMENDATION_CATALOG", "1")
        .args(["skill", "suggest", "--json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "suggest failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("skill suggest functional JSON must remain parseable");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Falling back to embedded recommendation catalog"),
        "expected real fallback WARN: {stderr}"
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Falling back"));
}

#[test]
#[cfg(unix)]
fn rust_log_debug_emits_debug_and_log_level_warn_overrides_it() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    write_apply_fixture(root);

    let debug = Command::new(agentsync_bin())
        .current_dir(root)
        .env("RUST_LOG", "debug")
        .args(["apply", "--agents", "other", "--log-format", "json"])
        .output()
        .unwrap();
    let debug_stderr = String::from_utf8_lossy(&debug.stderr);
    assert!(
        debug_stderr
            .lines()
            .any(|line| line.contains("Skipping agent")),
        "RUST_LOG=debug did not emit debug event: {debug_stderr}"
    );

    let warn = Command::new(agentsync_bin())
        .current_dir(root)
        .env("RUST_LOG", "debug")
        .args([
            "apply",
            "--agents",
            "other",
            "--log-level",
            "warn",
            "--log-format",
            "json",
        ])
        .output()
        .unwrap();
    let warn_stderr = String::from_utf8_lossy(&warn.stderr);
    assert!(
        !warn_stderr.contains("Skipping agent"),
        "--log-level warn did not override RUST_LOG=debug: {warn_stderr}"
    );
}
