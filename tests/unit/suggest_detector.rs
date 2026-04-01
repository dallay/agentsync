use agentsync::skills::catalog::EmbeddedSkillCatalog;
use agentsync::skills::detect::{CatalogDrivenDetector, RepoDetector};
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
    // CatalogDrivenDetector checks config_files at root level only,
    // so astro.config.mjs must be at the root (not nested).
    fs::write(root.join("astro.config.mjs"), "export default {}\n").unwrap();
    fs::create_dir_all(root.join(".github/workflows")).unwrap();
    fs::write(root.join(".github/workflows/ci.yml"), "name: ci\n").unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();
    fs::write(root.join("Makefile"), "all:\n\ttrue\n").unwrap();
    fs::write(root.join("pyproject.toml"), "[project]\nname='demo'\n").unwrap();

    let detector = catalog_detector();
    let detections = detector.detect(root).unwrap();
    let technologies = detections
        .iter()
        .map(|detection| detection.technology.clone())
        .collect::<Vec<_>>();

    assert!(technologies.contains(&TechnologyId::new(TechnologyId::RUST)));
    assert!(technologies.contains(&TechnologyId::new(TechnologyId::NODE_TYPESCRIPT)));
    assert!(technologies.contains(&TechnologyId::new(TechnologyId::ASTRO)));
    assert!(technologies.contains(&TechnologyId::new(TechnologyId::GITHUB_ACTIONS)));
    assert!(technologies.contains(&TechnologyId::new(TechnologyId::DOCKER)));
    assert!(technologies.contains(&TechnologyId::new(TechnologyId::MAKE)));
    assert!(technologies.contains(&TechnologyId::new(TechnologyId::PYTHON)));

    let rust = detections
        .iter()
        .find(|detection| detection.technology == TechnologyId::new(TechnologyId::RUST))
        .unwrap();
    assert_eq!(rust.confidence, DetectionConfidence::High);
    assert!(
        rust.evidence
            .iter()
            .any(|evidence| evidence.marker == "Cargo.toml")
    );

    // CatalogDrivenDetector returns High confidence for config_files matches
    let astro = detections
        .iter()
        .find(|detection| detection.technology == TechnologyId::new(TechnologyId::ASTRO))
        .unwrap();
    assert_eq!(astro.confidence, DetectionConfidence::High);
}

#[test]
fn prunes_ignored_directories_and_does_not_detect_nested_config_files() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // package.json inside node_modules should be ignored — no root package.json means
    // no NODE_TYPESCRIPT detection from config_files, and node_modules is pruned from
    // file extension scanning.
    fs::create_dir_all(root.join("node_modules/fake-app")).unwrap();
    fs::write(
        root.join("node_modules/fake-app/package.json"),
        "{\"name\":\"fake\"}\n",
    )
    .unwrap();
    // Dockerfile only in a non-root subdir — catalog checks root config_files only,
    // so Docker should NOT be detected.
    fs::create_dir_all(root.join("tests/e2e")).unwrap();
    fs::write(root.join("tests/e2e/Dockerfile"), "FROM scratch\n").unwrap();

    let detector = catalog_detector();
    let detections = detector.detect(root).unwrap();

    assert!(
        detections
            .iter()
            .all(|detection| detection.technology
                != TechnologyId::new(TechnologyId::NODE_TYPESCRIPT)),
        "node_modules/fake-app/package.json should not trigger node_typescript detection"
    );

    assert!(
        detections
            .iter()
            .all(|detection| detection.technology != TechnologyId::new(TechnologyId::DOCKER)),
        "Dockerfile in tests/e2e/ should not trigger docker detection (only root config_files checked)"
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

    let detector = catalog_detector();
    let detections = detector.detect(root).unwrap();

    // Rust must still be detected from root Cargo.toml
    assert!(
        detections
            .iter()
            .any(|detection| detection.technology == TechnologyId::new(TechnologyId::RUST))
    );
    // node_typescript must not be detected: no root package.json, and the nested one is
    // behind an unreadable directory (file extension scan gracefully skips it)
    assert!(
        detections
            .iter()
            .all(|detection| detection.technology
                != TechnologyId::new(TechnologyId::NODE_TYPESCRIPT))
    );
}

// ---------------------------------------------------------------------------
// CatalogDrivenDetector tests
// ---------------------------------------------------------------------------

fn catalog_detector() -> CatalogDrivenDetector {
    let catalog = EmbeddedSkillCatalog::default();
    CatalogDrivenDetector::new(&catalog).expect("embedded catalog should compile detection rules")
}

fn detected_technology_ids(
    detections: &[agentsync::skills::suggest::TechnologyDetection],
) -> Vec<String> {
    detections
        .iter()
        .map(|d| d.technology.as_ref().to_string())
        .collect()
}

#[test]
fn catalog_driven_detects_packages_from_package_json() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(
        root.join("package.json"),
        r#"{"dependencies": {"react": "^18.0.0", "next": "^14.0.0"}}"#,
    )
    .unwrap();

    let detector = catalog_detector();
    let detections = detector.detect(root).unwrap();
    let ids = detected_technology_ids(&detections);

    assert!(
        ids.contains(&"react".to_string()),
        "should detect react, got: {ids:?}"
    );
    assert!(
        ids.contains(&"nextjs".to_string()),
        "should detect nextjs, got: {ids:?}"
    );
}

#[test]
fn catalog_driven_detects_config_file_existence() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("next.config.mjs"), "export default {};\n").unwrap();

    let detector = catalog_detector();
    let detections = detector.detect(root).unwrap();
    let ids = detected_technology_ids(&detections);

    assert!(
        ids.contains(&"nextjs".to_string()),
        "should detect nextjs from config file, got: {ids:?}"
    );
}

#[test]
fn catalog_driven_detects_config_file_content() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(
        root.join("wrangler.json"),
        r#"{"durable_objects": {"bindings": []}}"#,
    )
    .unwrap();

    let detector = catalog_detector();
    let detections = detector.detect(root).unwrap();
    let ids = detected_technology_ids(&detections);

    assert!(
        ids.contains(&"cloudflare_durable_objects".to_string()),
        "should detect cloudflare_durable_objects from wrangler.json content, got: {ids:?}"
    );
}

#[test]
fn catalog_driven_detects_gradle_layout() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(
        root.join("build.gradle.kts"),
        r#"plugins { id("com.android.application") }"#,
    )
    .unwrap();

    let detector = catalog_detector();
    let detections = detector.detect(root).unwrap();
    let ids = detected_technology_ids(&detections);

    assert!(
        ids.contains(&"android".to_string()),
        "should detect android from Gradle build file, got: {ids:?}"
    );
}

#[test]
fn catalog_driven_detects_package_patterns() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(
        root.join("package.json"),
        r#"{"dependencies": {"@azure/storage-blob": "^12.0.0"}}"#,
    )
    .unwrap();

    let detector = catalog_detector();
    let detections = detector.detect(root).unwrap();
    let ids = detected_technology_ids(&detections);
    let azure = detections
        .iter()
        .find(|detection| detection.technology == TechnologyId::new("azure"))
        .unwrap();

    assert!(
        ids.contains(&"azure".to_string()),
        "should detect azure from @azure/ package pattern, got: {ids:?}"
    );
    assert_eq!(azure.confidence, DetectionConfidence::Medium);
}

#[test]
fn catalog_driven_detects_file_extensions() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/index.tsx"),
        "export default function App() {}\n",
    )
    .unwrap();

    let detector = catalog_detector();
    let detections = detector.detect(root).unwrap();
    let ids = detected_technology_ids(&detections);

    assert!(
        ids.contains(&"web_frontend".to_string()),
        "should detect web_frontend from .tsx file extension, got: {ids:?}"
    );
}

#[test]
fn catalog_driven_detects_workspace_packages() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(
        root.join("pnpm-workspace.yaml"),
        "packages:\n  - \"packages/*\"\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("packages/app")).unwrap();
    fs::write(
        root.join("packages/app/package.json"),
        r#"{"dependencies": {"vue": "^3.0.0"}}"#,
    )
    .unwrap();

    let detector = catalog_detector();
    let detections = detector.detect(root).unwrap();
    let ids = detected_technology_ids(&detections);

    assert!(
        ids.contains(&"vue".to_string()),
        "should detect vue from workspace package.json, got: {ids:?}"
    );
}

#[test]
fn catalog_driven_empty_project_has_no_detections() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let detector = catalog_detector();
    let detections = detector.detect(root).unwrap();

    assert!(
        detections.is_empty(),
        "empty project should produce no detections, got: {:?}",
        detected_technology_ids(&detections)
    );
}

// ---------------------------------------------------------------------------
// file_extensions — incidental-dir confidence behaviour
// ---------------------------------------------------------------------------

/// Documents current behaviour: `file_extensions` scanning does not apply
/// incidental-dir confidence weighting, so a .tsx file found only inside
/// `tests/fixtures/` yields the same `Medium` confidence as one in `src/`.
/// If the incidental-dir weighting is ever added back, update the assertion.
#[test]
fn file_extensions_detection_in_incidental_dirs_yields_medium_confidence() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::create_dir_all(root.join("tests/fixtures")).unwrap();
    fs::write(
        root.join("tests/fixtures/component.tsx"),
        "export default () => null;\n",
    )
    .unwrap();

    let detector = catalog_detector();
    let detections = detector.detect(root).unwrap();

    let web_frontend = detections
        .iter()
        .find(|d| d.technology == TechnologyId::new("web_frontend"));

    assert!(
        web_frontend.is_some(),
        "web_frontend should be detected from .tsx in tests/fixtures/, got: {:?}",
        detected_technology_ids(&detections)
    );
    assert_eq!(
        web_frontend.unwrap().confidence,
        DetectionConfidence::Medium,
        "file_extensions detection currently yields Medium regardless of incidental path"
    );
}

// ---------------------------------------------------------------------------
// Workspace resolution — package.json workspaces field
// ---------------------------------------------------------------------------

#[test]
fn detects_packages_from_package_json_workspace_array() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // npm / Yarn classic array form: "workspaces": ["apps/*", "packages/*"]
    fs::write(
        root.join("package.json"),
        r#"{"workspaces": ["apps/*", "packages/*"]}"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("apps/web")).unwrap();
    fs::write(
        root.join("apps/web/package.json"),
        r#"{"dependencies": {"vue": "^3.0.0"}}"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("packages/ui")).unwrap();
    fs::write(
        root.join("packages/ui/package.json"),
        r#"{"dependencies": {"@angular/core": "^17.0.0"}}"#,
    )
    .unwrap();

    let detector = catalog_detector();
    let detections = detector.detect(root).unwrap();
    let ids = detected_technology_ids(&detections);

    assert!(
        ids.contains(&"vue".to_string()),
        "should detect vue from apps/web/package.json via package.json workspaces, got: {ids:?}"
    );
    assert!(
        ids.contains(&"angular".to_string()),
        "should detect angular from packages/ui/package.json via package.json workspaces, got: {ids:?}"
    );
}

#[test]
fn detects_packages_from_exact_workspace_path() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Exact (non-glob) path — no wildcard
    fs::write(
        root.join("pnpm-workspace.yaml"),
        "packages:\n  - \"backend\"\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("backend")).unwrap();
    fs::write(
        root.join("backend/package.json"),
        r#"{"dependencies": {"vue": "^3.0.0"}}"#,
    )
    .unwrap();

    let detector = catalog_detector();
    let detections = detector.detect(root).unwrap();
    let ids = detected_technology_ids(&detections);

    assert!(
        ids.contains(&"vue".to_string()),
        "should detect vue from exact-path workspace backend/package.json, got: {ids:?}"
    );
}
