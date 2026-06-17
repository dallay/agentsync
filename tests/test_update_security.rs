use agentsync::skills::update::update_skill_async;
use std::fs;
use tempfile::TempDir;

#[cfg(unix)]
#[tokio::test]
async fn test_update_skill_skips_symlinks() {
    use std::os::unix::fs::symlink;

    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().join("project");
    let target_root = project_root.join(".agents").join("skills");
    fs::create_dir_all(&target_root).unwrap();

    // 1. Create a sensitive file OUTSIDE the update source
    let sensitive_file = temp_dir.path().join("sensitive.txt");
    fs::write(&sensitive_file, "SECRET CONTENT").unwrap();

    // 2. Create an update source directory
    let update_source = temp_dir.path().join("update_source");
    fs::create_dir_all(&update_source).unwrap();

    // Create a valid SKILL.md in update source with frontmatter
    let manifest_content =
        "---\nname: test-skill\nversion: 1.0.0\ndescription: test\n---\n# Test Skill";
    fs::write(update_source.join("SKILL.md"), manifest_content).unwrap();

    // 3. Create a malicious symlink inside update source pointing to sensitive file
    let malicious_link = update_source.join("malicious_link");
    symlink(&sensitive_file, &malicious_link).unwrap();

    // 4. Run update_skill_async
    // We need an "installed" version to trigger update, or just update into empty
    // update_skill_async checks if new version > installed.
    // If not installed, it treats as 0.0.0.

    update_skill_async("test-skill", &target_root, &update_source)
        .await
        .unwrap();

    // 5. Verify results
    let installed_skill_dir = target_root.join("test-skill");
    assert!(installed_skill_dir.exists());
    assert!(installed_skill_dir.join("SKILL.md").exists());

    // SECURITY: The malicious_link should NOT have been copied
    let copied_link = installed_skill_dir.join("malicious_link");
    assert!(
        !copied_link.exists(),
        "Malicious symlink should NOT have been copied"
    );

    // Verify it didn't copy the content either (in case it followed it)
    // If it followed it, it might have created a regular file with the same name.
    assert!(
        !copied_link.is_file(),
        "Malicious symlink content should NOT have been copied as a file"
    );
}
