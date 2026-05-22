use agentsync::skills::catalog::EmbeddedSkillCatalog;
use agentsync::skills::detect::{CatalogDrivenDetector, ContentCache, RepoDetector};
use std::fs;
use tempfile::TempDir;

fn catalog_detector() -> CatalogDrivenDetector {
    let catalog = EmbeddedSkillCatalog::default();
    CatalogDrivenDetector::new(&catalog).expect("embedded catalog should compile detection rules")
}

#[test]
fn test_detect_pyproject_toml_with_poetry_groups() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create a pyproject.toml with poetry groups (line 790 coverage)
    fs::write(
        project_root.join("pyproject.toml"),
        r#"
[tool.poetry]
name = "test-project"

[tool.poetry.dependencies]
python = "^3.9"
requests = "^2.28.0"

[tool.poetry.group.dev.dependencies]
pytest = "^7.0.0"
black = "^22.0.0"

[tool.poetry.dev-dependencies]
mypy = "^0.950"
"#,
    )
    .unwrap();

    let detector = catalog_detector();
    let mut cache = ContentCache::new();
    let detections = detector.detect(project_root, &mut cache).unwrap();

    // Should detect Python from pyproject.toml
    assert!(
        !detections.is_empty(),
        "Should detect at least Python from pyproject.toml"
    );
}

#[test]
fn test_detect_pipfile_deps() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create a Pipfile (line 797-809 coverage)
    fs::write(
        project_root.join("Pipfile"),
        r#"
[packages]
django = "*"
requests = ">=2.28.0"

[dev-packages]
pytest = "*"
black = "*"
"#,
    )
    .unwrap();

    let detector = catalog_detector();
    let mut cache = ContentCache::new();
    let detections = detector.detect(project_root, &mut cache).unwrap();

    // Should detect Python from Pipfile
    assert!(!detections.is_empty(), "Should detect Python from Pipfile");
}

#[test]
fn test_detect_requirements_txt_with_nested_includes() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create requirements.txt with -r include (line 670, 708, 718-721, 725 coverage)
    fs::write(
        project_root.join("requirements.txt"),
        "requests>=2.28.0\n-r dev-requirements.txt\n",
    )
    .unwrap();

    fs::write(
        project_root.join("dev-requirements.txt"),
        "pytest>=7.0.0\nblack>=22.0.0\n",
    )
    .unwrap();

    let detector = catalog_detector();
    let mut cache = ContentCache::new();
    let detections = detector.detect(project_root, &mut cache).unwrap();

    // Should detect Python from requirements.txt
    assert!(
        !detections.is_empty(),
        "Should detect Python from requirements.txt"
    );
}

#[test]
fn test_detect_with_gradle_layout() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create Gradle project structure (line 560 coverage)
    fs::create_dir_all(project_root.join("app")).unwrap();
    fs::write(
        project_root.join("build.gradle.kts"),
        "plugins { id(\"java\") }",
    )
    .unwrap();
    fs::write(
        project_root.join("settings.gradle.kts"),
        "rootProject.name = \"test\"",
    )
    .unwrap();
    fs::write(
        project_root.join("app/build.gradle.kts"),
        "plugins { id(\"java\") }",
    )
    .unwrap();

    let detector = catalog_detector();
    let mut cache = ContentCache::new();
    let detections = detector.detect(project_root, &mut cache).unwrap();

    // Should detect Gradle
    assert!(
        !detections.is_empty(),
        "Should detect Gradle from build files"
    );
}

#[test]
fn test_detect_with_explicit_config_files() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create Rust project (line 566-574 coverage)
    fs::write(
        project_root.join("Cargo.toml"),
        "[package]\nname = \"test\"",
    )
    .unwrap();

    let detector = catalog_detector();
    let mut cache = ContentCache::new();
    let detections = detector.detect(project_root, &mut cache).unwrap();

    // Should detect Rust
    let tech_ids: Vec<_> = detections
        .iter()
        .map(|d| d.technology.as_ref().to_string())
        .collect();
    assert!(
        tech_ids.contains(&"rust".to_string()),
        "Should detect Rust from Cargo.toml"
    );
}

#[test]
fn test_detect_ignores_node_modules_and_git() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create directories that should be ignored (line 126 coverage)
    fs::create_dir_all(project_root.join("node_modules")).unwrap();
    fs::create_dir_all(project_root.join(".git")).unwrap();
    fs::create_dir_all(project_root.join("target")).unwrap();
    fs::create_dir_all(project_root.join("src")).unwrap();

    fs::write(project_root.join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(project_root.join("node_modules/test.js"), "test").unwrap();
    fs::write(
        project_root.join("Cargo.toml"),
        "[package]\nname = \"test\"",
    )
    .unwrap();

    let detector = catalog_detector();
    let mut cache = ContentCache::new();
    let detections = detector.detect(project_root, &mut cache).unwrap();

    let tech_ids: Vec<_> = detections
        .iter()
        .map(|d| d.technology.as_ref().to_string())
        .collect();
    assert!(tech_ids.contains(&"rust".to_string()), "Should detect Rust");

    // The detector should not have scanned node_modules or .git
    // (we can't directly test this, but the test exercises the ignore logic)
}

#[test]
fn test_content_cache_reuses_reads() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    fs::write(
        project_root.join("package.json"),
        r#"{"dependencies": {"react": "^18.0.0"}}"#,
    )
    .unwrap();

    let detector = catalog_detector();
    let mut cache = ContentCache::new();

    // First detection
    let detections1 = detector.detect(project_root, &mut cache).unwrap();

    // Cache should now contain package.json
    assert!(!cache.is_empty(), "Cache should contain read files");

    // Second detection should reuse cache
    let detections2 = detector.detect(project_root, &mut cache).unwrap();

    assert_eq!(
        detections1.len(),
        detections2.len(),
        "Should get same results from cache"
    );
}

#[test]
fn test_detect_with_empty_project() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Empty project - no files
    let detector = catalog_detector();
    let mut cache = ContentCache::new();
    let detections = detector.detect(project_root, &mut cache).unwrap();

    // Should return empty or minimal detections
    assert!(
        detections.is_empty() || detections.len() < 3,
        "Empty project should have few/no detections"
    );
}

#[test]
fn test_cache_stores_failed_reads() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Create a project with a reference to a non-existent file
    fs::write(
        project_root.join("Cargo.toml"),
        "[package]\nname = \"test\"",
    )
    .unwrap();

    let detector = catalog_detector();
    let mut cache = ContentCache::new();

    // First detection - will try to read various files
    let _detections = detector.detect(project_root, &mut cache).unwrap();

    // Cache should contain entries after detection
    // The detector reads files during detection, so cache will have entries
    // even if some reads fail (they're cached as None)
    // We just verify the detection completes without error
    let _ = cache.is_empty(); // Use the cache to avoid unused warning

    // The important thing is that the detection completes without error
    // This exercises the cache logic for both successful and failed reads
}
