//! End-to-End CLI Tests for AgentSync
//!
//! These tests verify the complete CLI behavior by running the binary
//! and checking outputs and file system changes.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

fn agentsync_cmd() -> Command {
    Command::cargo_bin("agentsync").unwrap()
}

fn setup_project_with_config(temp_dir: &TempDir) {
    let agents_dir = temp_dir.path().join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();

    // Create source file
    fs::write(agents_dir.join("AGENTS.md"), "# Test Agent Instructions").unwrap();

    // Create config
    let config = r#"
        source_dir = "."
        
        [gitignore]
        enabled = true
        marker = "AI Agent Symlinks"
        
        [agents.claude]
        enabled = true
        description = "Claude Code"
        
        [agents.claude.targets.instructions]
        source = "AGENTS.md"
        destination = "CLAUDE.md"
        type = "symlink"
        
        [agents.copilot]
        enabled = true
        description = "GitHub Copilot"
        
        [agents.copilot.targets.instructions]
        source = "AGENTS.md"
        destination = ".github/copilot-instructions.md"
        type = "symlink"
    "#;
    fs::write(agents_dir.join("agentsync.toml"), config).unwrap();
}

// =============================================================================
// INIT COMMAND TESTS
// =============================================================================

#[test]
fn test_cli_init_creates_files() {
    let temp_dir = TempDir::new().unwrap();

    agentsync_cmd()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialization complete"));

    // Verify files were created
    assert!(temp_dir.path().join(".agents").exists());
    assert!(temp_dir.path().join(".agents/agentsync.toml").exists());
    assert!(temp_dir.path().join(".agents/AGENTS.md").exists());
}

#[test]
fn test_cli_init_shows_next_steps() {
    let temp_dir = TempDir::new().unwrap();

    agentsync_cmd()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Next steps"))
        .stdout(predicate::str::contains(".agents/AGENTS.md"))
        .stdout(predicate::str::contains("agentsync apply"));
}

#[test]
fn test_cli_init_without_force_warns_existing() {
    let temp_dir = TempDir::new().unwrap();

    // First init
    agentsync_cmd()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success();

    // Second init without force should warn
    agentsync_cmd()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));
}

#[test]
fn test_cli_init_with_force_overwrites() {
    let temp_dir = TempDir::new().unwrap();

    // First init
    agentsync_cmd()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success();

    // Modify the config
    let config_path = temp_dir.path().join(".agents/agentsync.toml");
    fs::write(&config_path, "# Modified content").unwrap();

    // Second init WITH force
    agentsync_cmd()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--force")
        .assert()
        .success();

    // Config should be restored to default
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("[agents.claude]"));
}

#[test]
fn test_cli_init_uses_current_dir_by_default() {
    let temp_dir = TempDir::new().unwrap();

    agentsync_cmd()
        .current_dir(temp_dir.path())
        .arg("init")
        .assert()
        .success();

    assert!(temp_dir.path().join(".agents/agentsync.toml").exists());
}

// =============================================================================
// APPLY COMMAND TESTS
// =============================================================================

#[test]
#[cfg(unix)]
fn test_cli_apply_creates_symlinks() {
    let temp_dir = TempDir::new().unwrap();
    setup_project_with_config(&temp_dir);

    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Sync complete"));

    // Verify symlinks were created
    assert!(temp_dir.path().join("CLAUDE.md").is_symlink());
    assert!(
        temp_dir
            .path()
            .join(".github/copilot-instructions.md")
            .is_symlink()
    );
}

#[test]
#[cfg(unix)]
fn test_cli_apply_shows_stats() {
    let temp_dir = TempDir::new().unwrap();
    setup_project_with_config(&temp_dir);

    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Created:"))
        .stdout(predicate::str::contains("Updated:"))
        .stdout(predicate::str::contains("Skipped:"));
}

#[test]
#[cfg(unix)]
fn test_cli_apply_dry_run() {
    let temp_dir = TempDir::new().unwrap();
    setup_project_with_config(&temp_dir);

    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("dry-run"));

    // Symlinks should NOT be created
    assert!(!temp_dir.path().join("CLAUDE.md").exists());
}

#[test]
#[cfg(unix)]
fn test_cli_apply_verbose() {
    let temp_dir = TempDir::new().unwrap();
    setup_project_with_config(&temp_dir);

    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains("Using config:"));
}

#[test]
#[cfg(unix)]
fn test_cli_apply_filter_agents() {
    let temp_dir = TempDir::new().unwrap();
    setup_project_with_config(&temp_dir);

    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--agents")
        .arg("claude")
        .assert()
        .success();

    // Only claude should be created
    assert!(temp_dir.path().join("CLAUDE.md").is_symlink());
    assert!(
        !temp_dir
            .path()
            .join(".github/copilot-instructions.md")
            .exists()
    );
}

#[test]
#[cfg(unix)]
fn test_cli_apply_updates_gitignore() {
    let temp_dir = TempDir::new().unwrap();
    setup_project_with_config(&temp_dir);

    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success();

    // Verify .gitignore was updated
    let gitignore_path = temp_dir.path().join(".gitignore");
    assert!(gitignore_path.exists());

    let content = fs::read_to_string(&gitignore_path).unwrap();
    assert!(content.contains("CLAUDE.md"));
    assert!(content.contains("# START AI Agent Symlinks"));
}

#[test]
#[cfg(unix)]
fn test_cli_apply_no_gitignore_flag() {
    let temp_dir = TempDir::new().unwrap();
    setup_project_with_config(&temp_dir);

    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--no-gitignore")
        .assert()
        .success();

    // Symlinks should exist but .gitignore should NOT
    assert!(temp_dir.path().join("CLAUDE.md").is_symlink());
    assert!(!temp_dir.path().join(".gitignore").exists());
}

#[test]
#[cfg(unix)]
fn test_cli_apply_with_clean_flag() {
    let temp_dir = TempDir::new().unwrap();
    setup_project_with_config(&temp_dir);

    // First apply
    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success();

    // Second apply with clean
    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--clean")
        .assert()
        .success()
        .stdout(predicate::str::contains("Cleaning existing symlinks"));

    // Symlinks should still exist (recreated after clean)
    assert!(temp_dir.path().join("CLAUDE.md").is_symlink());
}

#[test]
fn test_cli_apply_no_config_fails() {
    let temp_dir = TempDir::new().unwrap();

    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Could not find agentsync.toml"));
}

#[test]
fn test_cli_apply_with_explicit_config() {
    let temp_dir = TempDir::new().unwrap();
    setup_project_with_config(&temp_dir);

    let config_path = temp_dir.path().join(".agents/agentsync.toml");

    agentsync_cmd()
        .arg("apply")
        .arg("--config")
        .arg(&config_path)
        .assert()
        .success();
}

// =============================================================================
// CLEAN COMMAND TESTS
// =============================================================================

#[test]
#[cfg(unix)]
fn test_cli_clean_removes_symlinks() {
    let temp_dir = TempDir::new().unwrap();
    setup_project_with_config(&temp_dir);

    // First apply to create symlinks
    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success();

    assert!(temp_dir.path().join("CLAUDE.md").is_symlink());

    // Clean
    agentsync_cmd()
        .arg("clean")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Clean complete"));

    // Symlinks should be removed
    assert!(!temp_dir.path().join("CLAUDE.md").exists());
}

#[test]
#[cfg(unix)]
fn test_cli_clean_dry_run() {
    let temp_dir = TempDir::new().unwrap();
    setup_project_with_config(&temp_dir);

    // Apply
    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success();

    // Clean dry-run
    agentsync_cmd()
        .arg("clean")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("Would remove"));

    // Symlinks should STILL exist
    assert!(temp_dir.path().join("CLAUDE.md").is_symlink());
}

#[test]
#[cfg(unix)]
fn test_cli_clean_shows_removed_count() {
    let temp_dir = TempDir::new().unwrap();
    setup_project_with_config(&temp_dir);

    // Apply
    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success();

    // Clean
    agentsync_cmd()
        .arg("clean")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed:"));
}

// =============================================================================
// HELP AND VERSION TESTS
// =============================================================================

#[test]
fn test_cli_help() {
    agentsync_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Sync AI agent configurations"))
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("apply"))
        .stdout(predicate::str::contains("clean"));
}

#[test]
fn test_cli_version() {
    agentsync_cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("agentsync"));
}

#[test]
fn test_cli_init_help() {
    agentsync_cmd()
        .arg("init")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialize"))
        .stdout(predicate::str::contains("--force"))
        .stdout(predicate::str::contains("--path"));
}

#[test]
fn test_cli_apply_help() {
    agentsync_cmd()
        .arg("apply")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Apply configuration"))
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("--verbose"))
        .stdout(predicate::str::contains("--agents"))
        .stdout(predicate::str::contains("--no-gitignore"));
}

#[test]
fn test_cli_clean_help() {
    agentsync_cmd()
        .arg("clean")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Remove"))
        .stdout(predicate::str::contains("--dry-run"));
}

// =============================================================================
// ERROR HANDLING TESTS
// =============================================================================

#[test]
fn test_cli_invalid_subcommand() {
    agentsync_cmd()
        .arg("invalid")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn test_cli_apply_invalid_path() {
    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg("/nonexistent/path/that/does/not/exist")
        .assert()
        .failure();
}

// =============================================================================
// INTEGRATION WORKFLOW TESTS
// =============================================================================

#[test]
#[cfg(unix)]
fn test_full_workflow_init_apply_clean() {
    let temp_dir = TempDir::new().unwrap();

    // Step 1: Init
    agentsync_cmd()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success();

    assert!(temp_dir.path().join(".agents/AGENTS.md").exists());

    // Step 2: Apply
    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success();

    assert!(temp_dir.path().join("CLAUDE.md").is_symlink());
    assert!(
        temp_dir
            .path()
            .join(".github/copilot-instructions.md")
            .is_symlink()
    );
    assert!(temp_dir.path().join("AGENTS.md").is_symlink());

    // Step 3: Clean
    agentsync_cmd()
        .arg("clean")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success();

    assert!(!temp_dir.path().join("CLAUDE.md").exists());
    assert!(
        !temp_dir
            .path()
            .join(".github/copilot-instructions.md")
            .exists()
    );

    // Source files should still exist
    assert!(temp_dir.path().join(".agents/AGENTS.md").exists());
    assert!(temp_dir.path().join(".agents/agentsync.toml").exists());
}

#[test]
#[cfg(unix)]
fn test_apply_idempotent() {
    let temp_dir = TempDir::new().unwrap();
    setup_project_with_config(&temp_dir);

    // Apply twice
    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success();

    agentsync_cmd()
        .arg("apply")
        .arg("--path")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Skipped:"));

    // Symlinks should still be valid
    assert!(temp_dir.path().join("CLAUDE.md").is_symlink());

    // Reading through symlink should work
    let content = fs::read_to_string(temp_dir.path().join("CLAUDE.md")).unwrap();
    assert!(content.contains("Test Agent Instructions"));
}
