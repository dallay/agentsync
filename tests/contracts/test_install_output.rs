use std::process::Command;
use tempfile::TempDir;

#[test]
fn install_json_contract() {
    // Run the dev install subcommand and capture JSON
    let td = TempDir::new().unwrap();
    let cwd = td.path();

    // Build the binary for test execution
    let _ = std::process::Command::new("cargo").arg("build").status();

    let target = if cfg!(windows) {
        "target/debug/agentsync.exe"
    } else {
        "target/debug/agentsync"
    };
    let output = Command::new(target)
        .current_dir(cwd)
        .arg("devinstall")
        .arg("sample-skill")
        .arg("--json")
        .output()
        .expect("failed to run binary");

    assert!(output.status.success());
    let out = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["id"], "sample-skill");
    assert_eq!(v["status"], "installed");
    assert!(v.get("name").is_some(), "Missing field 'name'");
    assert!(
        v.get("description").is_some(),
        "Missing field 'description'"
    );
    assert!(
        v.get("files").is_some() && v["files"].is_array(),
        "Missing or invalid 'files'"
    );
    assert!(
        v.get("manifest_hash").is_some(),
        "Missing field 'manifest_hash'"
    );
    assert!(
        v.get("installed_at").is_some(),
        "Missing field 'installed_at'"
    );
}

#[test]
fn install_json_error_contract() {
    // Attempt to install a skill from a nonexistent zip file
    let td = TempDir::new().unwrap();
    let cwd = td.path();
    let _ = std::process::Command::new("cargo").arg("build").status();
    let target = if cfg!(windows) {
        "target/debug/agentsync.exe"
    } else {
        "target/debug/agentsync"
    };
    let output = Command::new(target)
        .current_dir(cwd)
        .arg("devinstall")
        .arg("nonexistent-file.zip")
        .arg("--json")
        .output()
        .expect("failed to run binary");

    // Error scenario: should not succeed
    assert!(!output.status.success());
    let out = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(
        v.get("error").is_some(),
        "No 'error' field for failed install"
    );
    assert!(
        v.get("code").is_some(),
        "No 'code' field for failed install"
    );
    assert!(
        v.get("remediation").is_some(),
        "No 'remediation' field for failed install"
    );
    assert!(v["code"] != "unknown", "Error code should be classified");
    assert!(
        !v["error"].as_str().unwrap_or("").is_empty(),
        "Error message should not be empty"
    );
    assert!(
        !v["remediation"].as_str().unwrap_or("").is_empty(),
        "Remediation message should not be empty"
    );
}
