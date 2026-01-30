use std::fs;
use std::io::Cursor;
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
        let options = zip::write::FileOptions::default();
        zip.start_file("SKILL.md", options).unwrap();
        zip.write_all(b"---\nname: sample-skill\n---\n# body")
            .unwrap();
        zip.finish().unwrap();
    }

    let cursor = Cursor::new(buf);
    agentsync::skills::install::install_from_zip("sample-skill", cursor, &target).unwrap();

    assert!(target.join("sample-skill").join("SKILL.md").exists());
}
