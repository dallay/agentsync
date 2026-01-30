use std::fs;
use std::io::Write;
use std::process::Command;
use tempfile::TempDir;

/// Test updating a skill from a missing/non-existent source path.
#[test]
fn test_update_missing_source_contract() {
    let td = TempDir::new().unwrap();
    let cwd = td.path();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let exe = if cfg!(windows) {
        "agentsync.exe"
    } else {
        "agentsync"
    };
    let target = std::path::Path::new(manifest_dir)
        .join("target")
        .join("debug")
        .join(exe);
    let target = target.to_str().expect("failed to convert path to str");
    let output = Command::new(target)
        .current_dir(cwd)
        .arg("skill")
        .arg("update")
        .arg("sample-skill")
        .arg("--source")
        .arg("/this/path/does/not/exist")
        .arg("--json")
        .output()
        .expect("failed to run binary");
    assert!(!output.status.success());
    let out = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(
        v.get("error").is_some(),
        "No 'error' field for failed update"
    );
    assert!(v.get("code").is_some(), "No 'code' field for failed update");
    assert!(
        v.get("remediation").is_some(),
        "No 'remediation' field for failed update"
    );
}

/// Test updating from a directory with an invalid/broken manifest (SKILL.md).
#[test]
fn test_update_broken_manifest_contract() {
    let td = TempDir::new().unwrap();
    let test_dir = td.path().join("broken-skill");
    fs::create_dir_all(&test_dir).unwrap();
    // Write a corrupted SKILL.md
    let mut f = fs::File::create(test_dir.join("SKILL.md")).unwrap();
    f.write_all(b"not frontmatter\n====\nthis is broken")
        .unwrap();
    let cwd = td.path();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let exe = if cfg!(windows) {
        "agentsync.exe"
    } else {
        "agentsync"
    };
    let target = std::path::Path::new(manifest_dir)
        .join("target")
        .join("debug")
        .join(exe);
    let target = target.to_str().expect("failed to convert path to str");
    let output = Command::new(target)
        .current_dir(cwd)
        .arg("skill")
        .arg("update")
        .arg("sample-skill")
        .arg("--source")
        .arg(test_dir.to_str().unwrap())
        .arg("--json")
        .output()
        .expect("failed to run binary");
    assert!(!output.status.success());
    let out = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(
        v.get("error").is_some(),
        "No 'error' field for failed update"
    );
    assert!(v.get("code").is_some(), "No 'code' field for failed update");
    assert!(
        v.get("remediation").is_some(),
        "No 'remediation' field for failed update"
    );
    assert!(
        out.contains("manifest") || out.contains("parse"),
        "Expected parse/manifest error"
    );
}

/// Test attempting to update to the same version (should fail: no version bump).
#[test]
fn test_update_no_version_bump_contract() {
    let project_root =
        std::env::temp_dir().join(format!("agentsync_test_{}", rand::random::<u32>()));
    fs::create_dir_all(&project_root).unwrap();
    let install_src = project_root.join("src");
    fs::create_dir_all(&install_src).unwrap();
    let manifest = "---\nname: noroll-skill\ndescription: Test\nversion: 1.0.0\n---\n";
    fs::write(install_src.join("SKILL.md"), manifest).unwrap();
    // Install from src
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let exe = if cfg!(windows) {
        "agentsync.exe"
    } else {
        "agentsync"
    };
    let target = std::path::Path::new(manifest_dir)
        .join("target")
        .join("debug")
        .join(exe);
    let target = target.to_str().expect("failed to convert path to str");
    let install_out = Command::new(target)
        .current_dir(&project_root)
        .arg("skill")
        .arg("install")
        .arg("noroll-skill")
        .arg("--source")
        .arg(install_src.to_str().unwrap())
        .arg("--json")
        .output()
        .expect("failed to run agentsync install");
    assert!(
        install_out.status.success(),
        "Install failed: STDOUT: {} STDERR: {}",
        String::from_utf8_lossy(&install_out.stdout),
        String::from_utf8_lossy(&install_out.stderr)
    );
    // Debug: Check registry after install
    let registry_path = project_root
        .join(".agents")
        .join("skills")
        .join("registry.json");
    if !registry_path.exists() {
        let skills_dir = project_root.join(".agents").join("skills");
        let skills_files = std::fs::read_dir(&skills_dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|_| vec![]);
        println!(
            "No registry.json found after install. Skills dir files: {:?}",
            skills_files
        );
    }
    let reg = fs::read_to_string(&registry_path).expect("No registry.json found after install");
    println!("Registry after install: {}", reg);
    // Print registry before update
    let reg_before_update =
        fs::read_to_string(&registry_path).unwrap_or_else(|_| "<not found>".to_string());
    println!("Registry before update: {}", reg_before_update);
    // CLEANUP: delete the whole test temp directory
    // Update from a new source with the exact same SKILL.md (should be no version bump)
    let update_src = project_root.join("update-src");
    fs::create_dir_all(&update_src).unwrap();
    fs::write(update_src.join("SKILL.md"), manifest).unwrap();
    let src_files = std::fs::read_dir(&install_src)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|_| vec![]);
    println!("[TEST DEBUG] install_src files: {:?}", src_files);
    println!("[TEST DEBUG] project_root: {:?}", project_root);
    println!("[TEST DEBUG] install_src: {:?}", install_src);
    println!(
        "[TEST DEBUG] command: {} skill install noroll-skill --source {} --json",
        target,
        install_src.to_str().unwrap()
    );
    let output = Command::new(target)
        .current_dir(&project_root)
        .arg("skill")
        .arg("install")
        .arg("noroll-skill")
        .arg("--source")
        .arg(install_src.to_str().unwrap())
        .arg("--json")
        .output()
        .expect("Failed to run agentsync install");
    if !output.status.success() {
        eprintln!(
            "[INSTALL STDERR] {}",
            String::from_utf8_lossy(&output.stderr)
        );
        eprintln!(
            "[INSTALL STDOUT] {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    // Create v1 update source (older)
    let v1_src_td = TempDir::new().unwrap();
    let v1_src = v1_src_td.path();
    let v1_manifest = "---\nname: dskill\ndescription: Test\nversion: 1.0.0\n---\n";
    fs::create_dir_all(v1_src).unwrap();
    fs::write(v1_src.join("SKILL.md"), v1_manifest).unwrap();
    // Attempt to update (downgrade)
    let output = Command::new(target)
        .current_dir(&project_root)
        .arg("skill")
        .arg("update")
        .arg("noroll-skill")
        .arg("--source")
        .arg(update_src.to_str().unwrap())
        .arg("--json")
        .output()
        .expect("failed to run binary");
    eprintln!(
        "[UPDATE STDERR] {}",
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!(
        "[UPDATE STDOUT] {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(!output.status.success());
    let out = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(
        v.get("error").is_some(),
        "No 'error' field for failed update"
    );
    assert!(v.get("code").is_some(), "No 'code' field for failed update");
    assert!(
        v.get("remediation").is_some(),
        "No 'remediation' field for failed update"
    );
    assert!(
        out.contains("version") || out.contains("Validation"),
        "Expected version/validation error"
    );
}
