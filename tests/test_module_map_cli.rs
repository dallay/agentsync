use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

#[cfg(unix)]
fn agentsync_bin() -> &'static str {
    env!("CARGO_BIN_EXE_agentsync")
}

#[cfg(unix)]
fn run_agentsync(project_root: &Path, args: &[&str]) -> Output {
    Command::new(agentsync_bin())
        .current_dir(project_root)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to run agentsync {:?}: {error}", args))
}

#[cfg(unix)]
fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed: status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
fn write_module_map_fixture(project_root: &Path) {
    let agents_dir = project_root.join(".agents");
    let claude_dir = agents_dir.join("claude");

    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(claude_dir.join("api-context.md"), "# API\n").unwrap();
    fs::write(claude_dir.join("ui-context.md"), "# UI\n").unwrap();

    fs::write(
        agents_dir.join("agentsync.toml"),
        r#"source_dir = "claude"

[gitignore]
enabled = false

[agents.claude]
enabled = true

[agents.claude.targets.modules]
source = "placeholder-source"
destination = "placeholder-destination"
type = "module-map"

[[agents.claude.targets.modules.mappings]]
source = "api-context.md"
destination = "src/api"

[[agents.claude.targets.modules.mappings]]
source = "ui-context.md"
destination = "src/ui"
"#,
    )
    .unwrap();
}

#[cfg(unix)]
fn assert_symlink_points_to(path: &Path, expected_target: &Path) {
    assert!(path.is_symlink(), "expected symlink at {}", path.display());
    let target = fs::read_link(path).unwrap();
    assert_eq!(target, expected_target);
}

#[test]
#[cfg(unix)]
fn test_module_map_cli_placeholder_happy_path() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_module_map_fixture(project_root);

    let apply = run_agentsync(project_root, &["apply"]);
    assert_success(&apply, "agentsync apply");

    let api_dest = project_root.join("src/api/CLAUDE.md");
    let ui_dest = project_root.join("src/ui/CLAUDE.md");
    assert_symlink_points_to(&api_dest, Path::new("../../.agents/claude/api-context.md"));
    assert_symlink_points_to(&ui_dest, Path::new("../../.agents/claude/ui-context.md"));

    let status = run_agentsync(project_root, &["status", "--json"]);
    assert_success(&status, "agentsync status --json");
    let entries: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    let entries = entries.as_array().unwrap();
    assert_eq!(entries.len(), 2);

    let destinations: Vec<_> = entries
        .iter()
        .map(|entry| entry["destination"].as_str().unwrap().to_string())
        .collect();
    assert!(
        destinations
            .iter()
            .any(|dest| dest.ends_with("src/api/CLAUDE.md"))
    );
    assert!(
        destinations
            .iter()
            .any(|dest| dest.ends_with("src/ui/CLAUDE.md"))
    );
    assert!(
        entries
            .iter()
            .all(|entry| entry["exists"].as_bool() == Some(true))
    );
    assert!(
        entries
            .iter()
            .all(|entry| entry["is_symlink"].as_bool() == Some(true))
    );
    assert!(entries.iter().any(|entry| {
        entry["expected_source"]
            .as_str()
            .is_some_and(|source| source.ends_with(".agents/claude/api-context.md"))
    }));
    assert!(entries.iter().any(|entry| {
        entry["expected_source"]
            .as_str()
            .is_some_and(|source| source.ends_with(".agents/claude/ui-context.md"))
    }));

    let doctor = run_agentsync(project_root, &["doctor"]);
    assert_success(&doctor, "agentsync doctor");
    let doctor_stdout = String::from_utf8_lossy(&doctor.stdout);
    assert!(doctor_stdout.contains("No issues found"), "{doctor_stdout}");
    assert!(!doctor_stdout.contains("Missing source for agent claude (target modules)"));

    let clean = run_agentsync(project_root, &["clean"]);
    assert_success(&clean, "agentsync clean");
    assert!(
        !api_dest.exists(),
        "expected {} to be removed",
        api_dest.display()
    );
    assert!(
        !ui_dest.exists(),
        "expected {} to be removed",
        ui_dest.display()
    );
    assert!(
        String::from_utf8_lossy(&clean.stdout).contains("Removed: 2"),
        "{}",
        String::from_utf8_lossy(&clean.stdout)
    );
}
