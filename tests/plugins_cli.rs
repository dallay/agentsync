use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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

fn setup_project() -> TempDir {
    let project = TempDir::new().unwrap();
    copy_tree(&fixture_root(), &project.path().join("marketplace"));
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
    project
}

fn run_plugin(project: &TempDir, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_agentsync"))
        .args(["plugin", "--project-root", project.path().to_str().unwrap()])
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn plugin_cli_add_status_list_and_remove_are_deterministic() {
    let project = setup_project();

    let add = run_plugin(&project, &["add", "internal/engineering"]);
    assert!(add.status.success(), "add failed: {:?}", add);
    assert!(String::from_utf8_lossy(&add.stdout).contains("added internal/engineering"));

    let list = run_plugin(&project, &["list", "--json"]);
    assert!(list.status.success(), "list failed: {:?}", list);
    let list_json: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert!(list_json["plugins"]["internal/engineering"].is_object());

    let status = run_plugin(&project, &["status", "--json"]);
    assert!(status.status.success(), "status failed: {:?}", status);
    let status_json: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status_json["status"], "ok");

    let remove = run_plugin(&project, &["remove", "internal/engineering"]);
    assert!(remove.status.success(), "remove failed: {:?}", remove);
    assert!(String::from_utf8_lossy(&remove.stdout).contains("removed internal/engineering"));
    assert!(!project.path().join(".agents/skills/review").exists());

    let config = fs::read_to_string(project.path().join(".agents/agentsync.toml")).unwrap();
    assert!(!config.contains("marketplace = \"internal\""));
    assert!(!config.contains("plugin = \"engineering\""));
    let lock = fs::read_to_string(project.path().join(".agents/plugins.lock.toml")).unwrap();
    assert!(!lock.contains("engineering"));
}

#[test]
fn plugin_cli_covers_human_json_update_and_invalid_selection_paths() {
    let project = setup_project();

    let empty_list = run_plugin(&project, &["list"]);
    assert!(empty_list.status.success());
    assert!(String::from_utf8_lossy(&empty_list.stdout).contains("No repository-owned plugins"));

    let invalid = run_plugin(&project, &["add", "invalid"]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("marketplace/plugin"));

    let add = run_plugin(&project, &["add", "internal/engineering", "--json"]);
    assert!(add.status.success(), "add failed: {:?}", add);
    let add_json: serde_json::Value = serde_json::from_slice(&add.stdout).unwrap();
    assert_eq!(add_json["status"], "added");

    let list = run_plugin(&project, &["list"]);
    assert!(list.status.success());
    assert!(String::from_utf8_lossy(&list.stdout).contains("internal/engineering"));

    let update = run_plugin(&project, &["update", "internal/engineering", "--json"]);
    assert!(update.status.success(), "update failed: {:?}", update);
    let update_json: serde_json::Value = serde_json::from_slice(&update.stdout).unwrap();
    assert_eq!(update_json["status"], "updated");

    let status = run_plugin(&project, &["status"]);
    assert!(status.status.success(), "status failed: {:?}", status);
    assert!(String::from_utf8_lossy(&status.stdout).contains("Plugin sources are locked"));

    let remove = run_plugin(&project, &["remove", "internal/engineering", "--json"]);
    assert!(remove.status.success(), "remove failed: {:?}", remove);
    let remove_json: serde_json::Value = serde_json::from_slice(&remove.stdout).unwrap();
    assert_eq!(remove_json["status"], "removed");
}
