use agentsync::skills::detect::{FileSystemRepoDetector, RepoDetector};
use agentsync::skills::suggest::{DetectionConfidence, TechnologyId};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(unix)]
struct PermissionRestoreGuard {
    path: PathBuf,
    permissions: fs::Permissions,
}

#[cfg(unix)]
impl Drop for PermissionRestoreGuard {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.path, self.permissions.clone());
    }
}

#[test]
fn detects_supported_phase_one_technologies() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
    fs::write(root.join("package.json"), "{}\n").unwrap();
    fs::create_dir_all(root.join("website/docs")).unwrap();
    fs::write(
        root.join("website/docs/astro.config.mjs"),
        "export default {}\n",
    )
    .unwrap();
    fs::create_dir_all(root.join(".github/workflows")).unwrap();
    fs::write(root.join(".github/workflows/ci.yml"), "name: ci\n").unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();
    fs::write(root.join("Makefile"), "all:\n\ttrue\n").unwrap();
    fs::write(root.join("pyproject.toml"), "[project]\nname='demo'\n").unwrap();

    let detections = FileSystemRepoDetector.detect(root).unwrap();
    let technologies = detections
        .iter()
        .map(|detection| detection.technology)
        .collect::<Vec<_>>();

    assert!(technologies.contains(&TechnologyId::Rust));
    assert!(technologies.contains(&TechnologyId::NodeTypeScript));
    assert!(technologies.contains(&TechnologyId::Astro));
    assert!(technologies.contains(&TechnologyId::GitHubActions));
    assert!(technologies.contains(&TechnologyId::Docker));
    assert!(technologies.contains(&TechnologyId::Make));
    assert!(technologies.contains(&TechnologyId::Python));

    let rust = detections
        .iter()
        .find(|detection| detection.technology == TechnologyId::Rust)
        .unwrap();
    assert_eq!(rust.confidence, DetectionConfidence::High);
    assert!(
        rust.evidence
            .iter()
            .any(|evidence| evidence.marker == "Cargo.toml")
    );

    let astro = detections
        .iter()
        .find(|detection| detection.technology == TechnologyId::Astro)
        .unwrap();
    assert_eq!(astro.confidence, DetectionConfidence::Medium);
}

#[test]
fn prunes_ignored_directories_and_marks_incidental_markers_low_confidence() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::create_dir_all(root.join("node_modules/fake-app")).unwrap();
    fs::write(
        root.join("node_modules/fake-app/package.json"),
        "{\"name\":\"fake\"}\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("tests/e2e")).unwrap();
    fs::write(root.join("tests/e2e/Dockerfile.e2e"), "FROM scratch\n").unwrap();

    let detections = FileSystemRepoDetector.detect(root).unwrap();

    assert!(
        detections
            .iter()
            .all(|detection| detection.technology != TechnologyId::NodeTypeScript)
    );

    let docker = detections
        .iter()
        .find(|detection| detection.technology == TechnologyId::Docker)
        .unwrap();
    assert_eq!(docker.confidence, DetectionConfidence::Low);
    assert!(
        docker
            .evidence
            .iter()
            .any(|evidence| evidence.path == std::path::Path::new("tests/e2e/Dockerfile.e2e"))
    );
}

#[cfg(unix)]
#[test]
fn skips_unreadable_nested_directories_without_failing_detection() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
    fs::create_dir_all(root.join("private/subdir")).unwrap();
    fs::write(root.join("private/subdir/package.json"), "{}\n").unwrap();

    let unreadable_dir = root.join("private");
    let original_permissions = fs::metadata(&unreadable_dir).unwrap().permissions();
    let _restore_permissions = PermissionRestoreGuard {
        path: unreadable_dir.clone(),
        permissions: original_permissions.clone(),
    };
    let mut unreadable_permissions = original_permissions.clone();
    unreadable_permissions.set_mode(0o000);
    fs::set_permissions(&unreadable_dir, unreadable_permissions).unwrap();

    let detections = FileSystemRepoDetector.detect(root).unwrap();

    assert!(
        detections
            .iter()
            .any(|detection| detection.technology == TechnologyId::Rust)
    );
    assert!(
        detections
            .iter()
            .all(|detection| detection.technology != TechnologyId::NodeTypeScript)
    );
}
