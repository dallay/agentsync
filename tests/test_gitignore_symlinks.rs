#![cfg(unix)]

//! Regression tests for gitignore entries that must match symlinked agent
//! directories. Git applies trailing-slash patterns to real directories only,
//! so fallback patterns for skills targets must not end with `/`.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::{Command, Output};

use agentsync::Config;
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to run git")
}

#[test]
fn opencode_skills_symlink_is_ignored_without_configured_skills_target() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    fs::create_dir_all(root.join(".agents/skills")).unwrap();
    fs::create_dir_all(root.join(".opencode")).unwrap();
    symlink("../.agents/skills", root.join(".opencode/skills")).unwrap();

    let config: Config = toml::from_str(
        r#"
        [gitignore]
        enabled = true

        [agents.opencode]
        enabled = true
        "#,
    )
    .unwrap();

    let entries = config.all_gitignore_entries();
    fs::write(root.join(".gitignore"), format!("{}\n", entries.join("\n"))).unwrap();

    let init = git(root, &["init", "-q"]);
    assert!(
        init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );

    let output = git(root, &["check-ignore", "-v", ".opencode/skills"]);
    assert!(
        output.status.success(),
        ".opencode/skills symlink should be ignored by {:?}; git stderr: {}",
        entries,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.starts_with(".gitignore:")
            && stdout.ends_with(":.opencode/skills\t.opencode/skills\n"),
        "expected the repository's .gitignore rule for .opencode/skills; got: {stdout:?}"
    );
}
