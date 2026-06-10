use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::fs::symlink;

fn agentsync_bin() -> &'static str {
    env!("CARGO_BIN_EXE_agentsync")
}

#[test]
#[cfg(unix)]
fn test_update_skill_skips_symlinks() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().join("project");
    fs::create_dir_all(&project_root).unwrap();

    // 1. Install an initial skill
    let initial_src = temp_dir.path().join("initial_src");
    fs::create_dir_all(&initial_src).unwrap();
    fs::write(
        initial_src.join("SKILL.md"),
        "---\nname: test-skill\nversion: 1.0.0\n---\n",
    )
    .unwrap();

    let install_out = Command::new(agentsync_bin())
        .current_dir(&project_root)
        .arg("skill")
        .arg("install")
        .arg("test-skill")
        .arg("--source")
        .arg(initial_src.to_str().unwrap())
        .output()
        .expect("failed to run install");
    assert!(install_out.status.success());

    // 2. Create a sensitive file outside
    let sensitive_file = temp_dir.path().join("sensitive.txt");
    fs::write(&sensitive_file, "SECRET_CONTENT").unwrap();

    // 3. Create malicious update source
    let malicious_src = temp_dir.path().join("malicious_src");
    fs::create_dir_all(&malicious_src).unwrap();
    fs::write(
        malicious_src.join("SKILL.md"),
        "---\nname: test-skill\nversion: 1.1.0\n---\n",
    )
    .unwrap();

    // Create a symlink to the sensitive file
    symlink(&sensitive_file, malicious_src.join("leak.txt")).unwrap();

    // 4. Run update
    let update_out = Command::new(agentsync_bin())
        .current_dir(&project_root)
        .arg("skill")
        .arg("update")
        .arg("test-skill")
        .arg("--source")
        .arg(malicious_src.to_str().unwrap())
        .output()
        .expect("failed to run update");

    assert!(
        update_out.status.success(),
        "Update failed: {}",
        String::from_utf8_lossy(&update_out.stderr)
    );

    // 5. Verify the leak
    let leaked_path = project_root.join(".agents/skills/test-skill/leak.txt");

    // After the fix, leak.txt should NOT exist because symlinks are skipped.
    // Before the fix, leak.txt WILL exist and contain "SECRET_CONTENT" because fs::copy follows symlinks.
    assert!(
        !leaked_path.exists(),
        "SECURITY VULNERABILITY: Symlink was followed during update, leaking content to {}",
        leaked_path.display()
    );
}
