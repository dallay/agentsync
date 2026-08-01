use agentsync::skills::update::update_skill_async;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_update_skill_skips_symlinks() {
    #[cfg(unix)]
    {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let target_root = project_root.join(".agents").join("skills");
        fs::create_dir_all(&target_root).unwrap();

        // 1. Create a sensitive file OUTSIDE the update source
        let sensitive_file = temp_dir.path().join("sensitive.txt");
        fs::write(&sensitive_file, "SENSITIVE CONTENT").unwrap();

        // 2. Pre-install a skill (v1.0.0)
        let skill_id = "test-skill";
        let skill_dir = target_root.join(skill_id);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test-skill\nversion: 1.0.0\n---",
        )
        .unwrap();

        // 3. Create a malicious update source (v2.0.0) with a symlink to the sensitive file
        let update_source = temp_dir.path().join("malicious-update");
        fs::create_dir_all(&update_source).unwrap();
        fs::write(
            update_source.join("SKILL.md"),
            "---\nname: test-skill\nversion: 2.0.0\n---",
        )
        .unwrap();

        std::os::unix::fs::symlink(&sensitive_file, update_source.join("leaked.txt")).unwrap();

        // 4. Run the update
        update_skill_async(skill_id, &target_root, &update_source)
            .await
            .expect("Update should succeed");

        // 5. Verify the symlink was NOT followed — the leaked file must not exist
        let leaked_path = skill_dir.join("leaked.txt");
        assert!(
            !leaked_path.exists(),
            "SECURITY VULNERABILITY: Symlink target was copied into the skill directory!"
        );

        // Also verify the skill itself was updated (SKILL.md content changed)
        let skill_content = fs::read_to_string(skill_dir.join("SKILL.md")).unwrap();
        assert!(
            skill_content.contains("2.0.0"),
            "Skill should have been updated to v2.0.0"
        );
    }
}
