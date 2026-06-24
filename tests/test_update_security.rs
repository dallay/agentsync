use agentsync::skills::update::update_skill_async;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
#[cfg(unix)]
async fn test_update_skill_skips_symlinks() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // 1. Setup project structure
    let project_root = root.join("project");
    let target_root = project_root.join(".agents").join("skills");
    fs::create_dir_all(&target_root).unwrap();

    // 2. Create a "sensitive" file outside the project root
    let secret_file = root.join("secret.txt");
    fs::write(&secret_file, "TOP SECRET CONTENT").unwrap();

    // 3. Setup an existing skill (v1.0.0)
    let skill_id = "test-skill";
    let skill_dir = target_root.join(skill_id);
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: test-skill\nversion: 1.0.0\ndescription: v1\n---\n# v1",
    )
    .unwrap();

    // Initialize registry
    let registry_path = target_root.join("registry.json");
    let entry = agentsync::skills::registry::SkillEntry {
        name: Some("test-skill".to_string()),
        description: Some("v1".to_string()),
        version: Some("1.0.0".to_string()),
        provider: None,
        source: None,
        installed_at: None,
        files: None,
        manifest_hash: None,
    };
    agentsync::skills::registry::update_registry_entry(&registry_path, skill_id, entry).unwrap();

    // 4. Setup a malicious update source (v2.0.0) with a symlink to the secret file
    let malicious_update_src = root.join("malicious_update");
    fs::create_dir_all(&malicious_update_src).unwrap();
    fs::write(
        malicious_update_src.join("SKILL.md"),
        "---\nname: test-skill\nversion: 2.0.0\ndescription: v2 malicious\n---\n# v2 malicious",
    )
    .unwrap();

    // Create the malicious symlink
    let stolen_data_path = malicious_update_src.join("stolen_data.txt");
    std::os::unix::fs::symlink(&secret_file, &stolen_data_path).unwrap();

    // 5. Run the update
    let result = update_skill_async(skill_id, &target_root, &malicious_update_src).await;
    assert!(
        result.is_ok(),
        "Update should succeed even if it skips some files"
    );

    // 6. Verify that the symlink was NOT followed and the secret content was NOT copied
    let installed_stolen_file = skill_dir.join("stolen_data.txt");

    // In the vulnerable version, this file will exist because copy_dir_all followed the symlink
    // and copied the content of secret.txt into stolen_data.txt in the destination.
    assert!(
        !installed_stolen_file.exists(),
        "SECURITY VULNERABILITY: Symlink was followed and content was copied into the project during update!"
    );
}
