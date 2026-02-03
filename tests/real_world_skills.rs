use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
#[ignore] // Se marca como ignore para que no corra en CI sin red, pero lo podemos forzar
fn test_install_real_skill_vitest_from_skills_sh() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // 1. Init agentsync
    let status = Command::new("cargo")
        .args(["run", "--", "init", "--path"])
        .arg(root)
        .status()
        .expect("failed to execute process");
    assert!(status.success());

    // 2. Install vitest skill
    let output = Command::new("cargo")
        .args(["run", "--", "skill", "--project-root"])
        .arg(root)
        .args(["install", "vitest"])
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT: {}", stdout);
    println!("STDERR: {}", stderr);

    assert!(
        output.status.success(),
        "Install failed.\nSTDOUT: {}\nSTDERR: {}",
        stdout,
        stderr
    );

    // 3. Verify files
    let skill_dir = root.join(".agents/skills/vitest");
    assert!(skill_dir.exists(), "Skill directory should be created");
    assert!(skill_dir.join("SKILL.md").exists(), "SKILL.md should exist");

    // Check registry
    let registry = fs::read_to_string(root.join(".agents/skills/registry.json")).unwrap();
    assert!(
        registry.contains("vitest"),
        "Registry should contain vitest"
    );
}

#[test]
#[ignore]
fn test_install_real_skill_astro_from_skills_sh() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // 1. Init
    let init_status = Command::new("cargo")
        .args(["run", "--", "init", "--path"])
        .arg(root)
        .status()
        .unwrap();
    assert!(
        init_status.success(),
        "Init failed in test_install_real_skill_astro_from_skills_sh"
    );

    // 2. Install astro
    let status = Command::new("cargo")
        .args(["run", "--", "skill", "--project-root"])
        .arg(root)
        .args(["install", "astro"])
        .status()
        .unwrap();

    assert!(status.success());

    // 3. Verify
    let skill_dir = root.join(".agents/skills/astro");
    assert!(skill_dir.exists());
    assert!(skill_dir.join("SKILL.md").exists());
}
