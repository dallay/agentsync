#[cfg(unix)]
use agentsync::skills::update::update_skill_async;
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(unix)]
use tempfile::TempDir;

#[tokio::test]
#[cfg(unix)]
async fn test_update_skill_skips_symlinks() {
    let td = TempDir::new().unwrap();
    let project_root = td.path().join("project");
    let target_root = project_root.join(".agents").join("skills");
    fs::create_dir_all(&target_root).unwrap();

    let skill_id = "test-skill";
    let skill_dir = target_root.join(skill_id);
    fs::create_dir_all(&skill_dir).unwrap();

    // Create v1 manifest in the current skill dir
    let v1_manifest = "---\nname: test-skill\ndescription: Test\nversion: 1.0.0\n---\n";
    fs::write(skill_dir.join("SKILL.md"), v1_manifest).unwrap();

    // Create a sensitive file OUTSIDE the update source
    let sensitive_file = td.path().join("sensitive.txt");
    fs::write(&sensitive_file, "SENSITIVE CONTENT").unwrap();

    // Create update source (v1.1.0) with a malicious symlink
    let update_src = td.path().join("update-src");
    fs::create_dir_all(&update_src).unwrap();
    let v2_manifest = "---\nname: test-skill\ndescription: Test\nversion: 1.1.0\n---\n";
    fs::write(update_src.join("SKILL.md"), v2_manifest).unwrap();

    // Malicious symlink pointing to sensitive.txt
    symlink(&sensitive_file, update_src.join("leaked.txt")).unwrap();

    // Perform update
    update_skill_async(skill_id, &target_root, &update_src)
        .await
        .expect("Update failed");

    // Verify if leaked.txt exists in the installed skill dir
    let installed_leaked = skill_dir.join("leaked.txt");

    if installed_leaked.exists() {
        let metadata = fs::symlink_metadata(&installed_leaked).unwrap();
        if !metadata.file_type().is_symlink() {
            panic!("SECURITY VULNERABILITY: Symlink was followed and content was copied!");
        }
    }

    // In a secure implementation, symlinks should be skipped entirely,
    // so installed_leaked should not exist.
    assert!(
        !installed_leaked.exists(),
        "SECURITY: Symlinks should be skipped during update"
    );
}
