use agentsync::skills::registry::load_curated_registry;
use std::fs;
use std::io::{Cursor, Write};
use tempfile::TempDir;

#[test]
fn install_from_zip_safety() {
    let td = TempDir::new().unwrap();
    let target = td.path().join(".agents").join("skills");
    fs::create_dir_all(&target).unwrap();

    // Create a small zip buffer containing SKILL.md
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("SKILL.md", options).unwrap();
        zip.write_all(b"---\nname: sample-skill\n---\n# body")
            .unwrap();
        zip.finish().unwrap();
    }

    let cursor = Cursor::new(buf);
    agentsync::skills::install::install_from_zip("sample-skill", cursor, &target).unwrap();

    assert!(target.join("sample-skill").join("SKILL.md").exists());
}

#[test]
fn curated_install_rejects_hash_mismatch_without_touching_existing_skill() {
    let td = TempDir::new().unwrap();
    let source = td.path().join("source");
    let target = td.path().join("target");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&target).unwrap();
    fs::write(
        source.join("SKILL.md"),
        "---\nname: valid-skill\nversion: 1.0.0\n---\nnew",
    )
    .unwrap();
    fs::create_dir_all(target.join("valid-skill")).unwrap();
    fs::write(target.join("valid-skill").join("SKILL.md"), "old").unwrap();

    let registry = load_curated_registry(std::path::Path::new(
        "tests/fixtures/curated-skills/registry.v1.toml",
    ))
    .unwrap();
    let entry = &registry.entries["valid-skill"];
    let error = agentsync::skills::install::install_from_dir_verified(
        "valid-skill",
        &source,
        &target,
        entry,
    )
    .unwrap_err();

    assert!(error.to_string().contains("sha256"));
    assert_eq!(
        fs::read_to_string(target.join("valid-skill/SKILL.md")).unwrap(),
        "old"
    );
    assert!(!target.join("registry.json").exists());
}

#[test]
fn curated_install_accepts_normalized_manifest_and_file_hashes() {
    let td = TempDir::new().unwrap();
    let source = td.path().join("source");
    let target = td.path().join("target");
    fs::create_dir_all(&source).unwrap();
    let content = "---\nname: valid-skill\nversion: 1.0.0\ndescription: A deterministic curated skill fixture.\n---\n# body";
    fs::write(source.join("SKILL.md"), content).unwrap();
    let hash = {
        use sha2::{Digest, Sha256};
        Sha256::digest(content.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let registry = agentsync::skills::registry::RegistryDocument {
        schema_version: "v1".into(),
        entries: [(
            "valid-skill".into(),
            agentsync::skills::registry::RegistryEntry {
                provider_skill_id: "owner/valid-skill".into(),
                local_skill_id: "valid-skill".into(),
                source: agentsync::skills::registry::SourcePin {
                    repository: "https://github.com/example/skills".into(),
                    commit: "0123456789abcdef0123456789abcdef0123456a".into(),
                    subpath: "skills/valid-skill".into(),
                },
                manifest: agentsync::skills::registry::ManifestExpectation {
                    name: "valid-skill".into(),
                    version: Some("1.0.0".into()),
                    description: Some("A deterministic curated skill fixture.".into()),
                },
                files: vec![agentsync::skills::registry::FileHash {
                    path: "SKILL.md".into(),
                    sha256: hash,
                }],
                license: agentsync::skills::registry::LicenseEvidence {
                    spdx: "MIT".into(),
                    source: "LICENSE".into(),
                    attribution_required: false,
                },
                validation: agentsync::skills::registry::ValidationMetadata {
                    status: "approved".into(),
                    validator: "test".into(),
                },
            },
        )]
        .into_iter()
        .collect(),
    };
    agentsync::skills::install::install_from_dir_verified(
        "valid-skill",
        &source,
        &target,
        &registry.entries["valid-skill"],
    )
    .unwrap();
    assert!(target.join("valid-skill/SKILL.md").exists());
}
