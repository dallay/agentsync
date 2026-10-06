use std::fs;
use std::path::{Path, PathBuf};
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
fn write_fixture(project_root: &Path) {
    fs::create_dir_all(project_root.join(".agents")).unwrap();
    fs::write(project_root.join(".agents/AGENTS.md"), "# hello\n").unwrap();
    fs::write(
        project_root.join(".agents/agentsync.toml"),
        "source_dir = \".\"\n\n[agents.claude]\nenabled = true\n\n[agents.claude.targets.instructions]\nsource = \"AGENTS.md\"\ndestination = \"CLAUDE.md\"\ntype = \"symlink\"\n",
    )
    .unwrap();
    fs::write(project_root.join("CLAUDE.md"), "original\n").unwrap();
}

#[test]
#[cfg(unix)]
fn test_revert_restores_pre_apply_file() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_fixture(project_root);

    let apply = run_agentsync(project_root, &["apply"]);
    assert!(
        apply.status.success(),
        "apply failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&apply.stdout),
        String::from_utf8_lossy(&apply.stderr)
    );
    assert!(project_root.join("CLAUDE.md").is_symlink());
    assert!(project_root.join("CLAUDE.md.bak").exists());

    let revert = run_agentsync(project_root, &["revert"]);
    assert!(
        revert.status.success(),
        "revert failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&revert.stdout),
        String::from_utf8_lossy(&revert.stderr)
    );
    assert_eq!(
        fs::read_to_string(project_root.join("CLAUDE.md")).unwrap(),
        "original\n"
    );
    assert!(!project_root.join("CLAUDE.md").is_symlink());
    assert!(!project_root.join("CLAUDE.md.bak").exists());
    // Managed gitignore block is gone after an unfiltered revert.
    let gitignore = project_root.join(".gitignore");
    if gitignore.exists() {
        assert!(
            !fs::read_to_string(&gitignore).unwrap().contains("START"),
            "managed block should be removed"
        );
    }
}

#[test]
#[cfg(unix)]
fn core_revert_discloses_mcp_phase_is_not_included_after_config_changes() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_fixture(project_root);
    let config_path = project_root.join(".agents/agentsync.toml");
    let config = fs::read_to_string(&config_path).unwrap();
    fs::write(
        &config_path,
        format!(
            "{config}\n[mcp]\nenabled = true\n\n[mcp_servers.fixture]\ncommand = \"fixture-server\"\n"
        ),
    )
    .unwrap();

    let apply = run_agentsync(project_root, &["apply"]);
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let mcp_config = project_root.join(".mcp.json");
    assert!(mcp_config.exists());

    let config = fs::read_to_string(&config_path).unwrap();
    let updated_config = config.replace(
        "[mcp]\nenabled = true\n\n[mcp_servers.fixture]\ncommand = \"fixture-server\"\n",
        "[mcp]\nenabled = false\n",
    );
    assert_ne!(updated_config, config);
    fs::write(&config_path, updated_config).unwrap();

    let revert = run_agentsync(project_root, &["revert"]);
    assert!(
        revert.status.success(),
        "{}",
        String::from_utf8_lossy(&revert.stderr)
    );
    let output = String::from_utf8_lossy(&revert.stdout);
    assert!(
        output.contains("MCP config files are not inspected or restored"),
        "core-only scope must be explicit even when MCP is now disabled: {output}"
    );
    assert!(output.contains("Symlink revert complete"), "{output}");
    assert!(
        mcp_config.exists(),
        "core-only revert must not modify MCP files"
    );
    let gitignore = project_root.join(".gitignore");
    if gitignore.exists() {
        assert!(!fs::read_to_string(gitignore).unwrap().contains("START"));
    }
}

#[test]
#[cfg(unix)]
fn test_revert_dry_run_changes_nothing() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_fixture(project_root);
    let apply = run_agentsync(project_root, &["apply"]);
    assert!(apply.status.success());

    let revert = run_agentsync(project_root, &["revert", "--dry-run"]);
    assert!(
        revert.status.success(),
        "revert --dry-run failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&revert.stdout),
        String::from_utf8_lossy(&revert.stderr)
    );
    // Symlink AND backup both still present.
    assert!(project_root.join("CLAUDE.md").is_symlink());
    assert!(project_root.join("CLAUDE.md.bak").exists());
}

#[test]
#[cfg(unix)]
fn test_revert_keep_backups_preserves_bak() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_fixture(project_root);
    assert!(run_agentsync(project_root, &["apply"]).status.success());

    let revert = run_agentsync(project_root, &["revert", "--keep-backups"]);
    assert!(
        revert.status.success(),
        "revert --keep-backups failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&revert.stdout),
        String::from_utf8_lossy(&revert.stderr)
    );
    assert_eq!(
        fs::read_to_string(project_root.join("CLAUDE.md")).unwrap(),
        "original\n"
    );
    assert_eq!(
        fs::read_to_string(project_root.join("CLAUDE.md.bak")).unwrap(),
        "original\n"
    );
}

#[test]
#[cfg(unix)]
fn test_revert_keeps_gitignore_when_destination_errors() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    fs::create_dir_all(project_root.join(".agents")).unwrap();
    fs::write(
        project_root.join(".agents/agentsync.toml"),
        "source_dir = \".\"\n\n[agents.claude]\nenabled = true\n\n[agents.claude.targets.instructions]\nsource = \"AGENTS.md\"\ndestination = \"/etc/passwd\"\ntype = \"symlink\"\n",
    )
    .unwrap();
    fs::write(
        project_root.join(".gitignore"),
        "# START AI Agent Symlinks\nCLAUDE.md\n# END AI Agent Symlinks\n",
    )
    .unwrap();

    let revert = run_agentsync(project_root, &["revert"]);

    assert!(
        !revert.status.success(),
        "unsafe destination should be an error"
    );
    let gitignore = fs::read_to_string(project_root.join(".gitignore")).unwrap();
    assert!(
        gitignore.contains("# START AI Agent Symlinks"),
        "managed gitignore block must remain when revert reports an error"
    );
}

#[test]
#[cfg(unix)]
fn test_revert_refuses_to_overwrite_user_file() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_fixture(project_root);
    assert!(run_agentsync(project_root, &["apply"]).status.success());
    assert!(project_root.join("CLAUDE.md").is_symlink());
    assert!(project_root.join("CLAUDE.md.bak").exists());

    // User replaces the managed symlink with a regular file after apply.
    fs::remove_file(project_root.join("CLAUDE.md")).unwrap();
    fs::write(project_root.join("CLAUDE.md"), "user edit\n").unwrap();
    assert!(!project_root.join("CLAUDE.md").is_symlink());

    let _ = run_agentsync(project_root, &["revert"]);
    // User content is preserved and the backup is left in place for manual recovery.
    assert_eq!(
        fs::read_to_string(project_root.join("CLAUDE.md")).unwrap(),
        "user edit\n"
    );
    assert_eq!(
        fs::read_to_string(project_root.join("CLAUDE.md.bak")).unwrap(),
        "original\n"
    );
}

#[cfg(unix)]
fn write_two_agent_fixture(project_root: &Path) {
    fs::create_dir_all(project_root.join(".agents")).unwrap();
    fs::write(project_root.join(".agents/AGENTS.md"), "# hello\n").unwrap();
    fs::write(
        project_root.join(".agents/agentsync.toml"),
        "source_dir = \".\"\n\n[agents.claude]\nenabled = true\n\n[agents.claude.targets.instructions]\nsource = \"AGENTS.md\"\ndestination = \"CLAUDE.md\"\ntype = \"symlink\"\n\n[agents.copilot]\nenabled = true\n\n[agents.copilot.targets.instructions]\nsource = \"AGENTS.md\"\ndestination = \"COPILOT.md\"\ntype = \"symlink\"\n",
    )
    .unwrap();
    fs::write(project_root.join("CLAUDE.md"), "original-claude\n").unwrap();
    fs::write(project_root.join("COPILOT.md"), "original-copilot\n").unwrap();
}

#[test]
#[cfg(unix)]
fn test_revert_agents_filter_limits_scope() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_two_agent_fixture(project_root);
    assert!(run_agentsync(project_root, &["apply"]).status.success());
    assert!(project_root.join("CLAUDE.md").is_symlink());
    assert!(project_root.join("COPILOT.md").is_symlink());

    let revert = run_agentsync(project_root, &["revert", "--agents", "claude"]);
    assert!(
        revert.status.success(),
        "revert --agents failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&revert.stdout),
        String::from_utf8_lossy(&revert.stderr)
    );
    // Claude restored; copilot untouched.
    assert_eq!(
        fs::read_to_string(project_root.join("CLAUDE.md")).unwrap(),
        "original-claude\n"
    );
    assert!(project_root.join("COPILOT.md").is_symlink());
    assert!(project_root.join("COPILOT.md.bak").exists());
}

#[test]
#[cfg(unix)]
fn test_revert_leaves_repointed_symlink_untouched() {
    use std::os::unix::fs::symlink;

    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    write_fixture(project_root);
    assert!(run_agentsync(project_root, &["apply"]).status.success());
    assert!(project_root.join("CLAUDE.md").is_symlink());
    assert!(project_root.join("CLAUDE.md.bak").exists());

    // User repoints the managed symlink elsewhere after apply.
    fs::remove_file(project_root.join("CLAUDE.md")).unwrap();
    fs::write(project_root.join("ELSEWHERE.md"), "elsewhere\n").unwrap();
    symlink("ELSEWHERE.md", project_root.join("CLAUDE.md")).unwrap();
    assert!(project_root.join("CLAUDE.md").is_symlink());

    let revert = run_agentsync(project_root, &["revert"]);
    assert!(
        revert.status.success(),
        "revert failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&revert.stdout),
        String::from_utf8_lossy(&revert.stderr)
    );
    // Neither the repointed link nor its backup is touched, and the skip is
    // warned about on stdout.
    assert!(project_root.join("CLAUDE.md").is_symlink());
    assert_eq!(
        fs::read_link(project_root.join("CLAUDE.md")).unwrap(),
        PathBuf::from("ELSEWHERE.md")
    );
    assert_eq!(
        fs::read_to_string(project_root.join("CLAUDE.md.bak")).unwrap(),
        "original\n"
    );
    let stdout = String::from_utf8_lossy(&revert.stdout);
    assert!(
        stdout.contains("Skipping unmanaged symlink"),
        "expected unmanaged-symlink skip warning, got:\n{stdout}"
    );
    let gitignore = fs::read_to_string(project_root.join(".gitignore")).unwrap();
    assert!(
        gitignore.contains("# START AI Agent Symlinks"),
        "managed gitignore block must remain when revert skips a destination"
    );
}
