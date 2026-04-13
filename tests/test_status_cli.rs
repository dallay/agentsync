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

#[test]
#[cfg(unix)]
fn test_status_human_output_shows_recognized_skills_mode_hint() {
    use std::os::unix::fs as unix_fs;

    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    let skills_dir = project_root.join(".agents/skills");

    fs::create_dir_all(&skills_dir).unwrap();
    fs::create_dir_all(project_root.join(".claude")).unwrap();
    fs::write(skills_dir.join("SKILL.md"), "# skill\n").unwrap();
    unix_fs::symlink("../.agents/skills", project_root.join(".claude/skills")).unwrap();

    fs::write(
        project_root.join(".agents/agentsync.toml"),
        r#"
        [agents.claude]
        enabled = true

        [agents.claude.targets.skills]
        source = "skills"
        destination = ".claude/skills"
        type = "symlink-contents"
    "#,
    )
    .unwrap();

    let output = run_agentsync(project_root, &["status"]);
    assert!(
        output.status.success(),
        "agentsync status failed: status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("OK:"), "{stdout}");
    assert!(stdout.contains(".claude/skills"), "{stdout}");
    assert!(stdout.contains("Hint:"), "{stdout}");
    assert!(
        stdout.contains("configured as \"symlink-contents\""),
        "{stdout}"
    );
    assert!(stdout.contains("symlink-contents"), "{stdout}");
    assert!(stdout.contains("symlink"), "{stdout}");
    assert!(stdout.contains("Status: All good"), "{stdout}");
}
