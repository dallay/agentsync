use std::fs;
use std::io::Write;
use tempfile::TempDir;

/// Utility: copy fixture skill to a temp location (simulates a downloaded or unpacked skill)
fn prepare_fixture_skill(dest_dir: &std::path::Path) {
    let fixture_root = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/sample-skill");

    // Debug output to see what's happening
    println!("DEBUG: fixture_root: {}", fixture_root.display());
    println!(
        "DEBUG: SKILL.md exists? {}",
        fixture_root.join("SKILL.md").exists()
    );

    let dest_skill = dest_dir.join("sample-skill");
    fs::create_dir_all(dest_skill.join("assets")).unwrap();

    fs::copy(fixture_root.join("SKILL.md"), dest_skill.join("SKILL.md")).unwrap();
    fs::copy(
        fixture_root.join("assets/icon.png"),
        dest_skill.join("assets/icon.png"),
    )
    .unwrap();
}

#[test]
fn integration_skill_install_fixture() {
    // Integration covers fixture with assets, not just SKILL.md
    let temp = TempDir::new().unwrap();
    let target_root = temp.path().join(".agents/skills");
    fs::create_dir_all(&target_root).unwrap();

    // Prepare fixture contents (simulate fetch/unarchive logic)
    prepare_fixture_skill(&target_root);

    // Use library install function (simulates what CLI would do internally)
    agentsync::skills::install::install_from_dir(
        "sample-skill",
        &target_root.join("sample-skill"),
        &target_root,
    )
    .unwrap();

    // Assert manifest file exists
    let installed_skill_md = target_root.join("sample-skill/SKILL.md");
    assert!(
        installed_skill_md.exists(),
        "SKILL.md should exist after install"
    );

    // Assert asset file exists
    let installed_icon = target_root.join("sample-skill/assets/icon.png");
    assert!(
        installed_icon.exists(),
        "icon.png should exist after install"
    );

    // [Optional/future] Assert registry file contains sample-skill (if/when implemented)
    let registry_path = target_root.join("registry.json");
    if registry_path.exists() {
        let registry = fs::read_to_string(&registry_path).expect("registry.json to exist");
        assert!(
            registry.contains("sample-skill"),
            "Registry must contain entry for sample-skill"
        );
    }
}

#[test]
fn integration_skill_install_invalid_manifest_rollback() {
    // Setup a temp dir for the skill install
    let temp = TempDir::new().unwrap();
    let target_root = temp.path().join(".agents/skills");
    fs::create_dir_all(&target_root).unwrap();

    // Prepare fixture, but intentionally use an invalid manifest name (not matching folder name is one thing, but let's make it fail regex)
    let fixture_root = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/sample-skill");
    let dest_skill = target_root.join("sample-skill");
    fs::create_dir_all(dest_skill.join("assets")).unwrap();

    // Write invalid SKILL.md (invalid name for regex)
    fs::write(
        dest_skill.join("SKILL.md"),
        "---\nname: \"Invalid Name!\"\n---\n# body",
    )
    .unwrap();
    fs::copy(
        fixture_root.join("assets/icon.png"),
        dest_skill.join("assets/icon.png"),
    )
    .unwrap();

    // Try to install via ZIP logic, expect error and cleanup
    // Create a zip
    let skill_path = target_root.join("sample-skill");
    let zipfile = temp.path().join("sample-skill.zip");
    {
        let file = fs::File::create(&zipfile).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::<()>::default();
        // Add SKILL.md
        let manifest_bytes = fs::read(skill_path.join("SKILL.md")).unwrap();
        zip.start_file("SKILL.md", options).unwrap();
        zip.write_all(&manifest_bytes).unwrap();
        // Add assets/icon.png
        zip.add_directory("assets", options).unwrap();
        let icon_bytes = fs::read(skill_path.join("assets/icon.png")).unwrap();
        zip.start_file("assets/icon.png", options).unwrap();
        zip.write_all(&icon_bytes).unwrap();
        zip.finish().unwrap();
    }

    // Now extract/install using our install_from_zip logic
    let file = fs::File::open(&zipfile).unwrap();
    let result =
        agentsync::skills::install::install_from_zip("sample-skill-invalid", file, &target_root);
    assert!(
        result.is_err(),
        "Install should fail for invalid manifest (name field)"
    );
    // Directory should not exist after failure (rollback)
    let broken_dir = target_root.join("sample-skill-invalid");
    assert!(
        !broken_dir.exists(),
        "Broken install directory should be cleaned up"
    );
}
