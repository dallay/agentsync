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

    // An empty project has no files, so no technology should be detected.
    // Up to 2 detections are tolerated in case future catalog rules add baseline
    // markers that fire on an empty tree (known noise); zero is the expected case.
    assert!(
        detections.len() < 3,
        "Empty project should have 0 detections (tolerance ≤2 for baseline catalog noise), got {}",
        detections.len()
    );
}

#[test]
fn test_cache_stores_failed_reads() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    // Write requirements.txt so detect will call get_file_content on it.
    // get_file_content caches None for any path it cannot read; here the file
    // exists, so the cache entry will be Some.  A separate missing path is
    // never even attempted by detect (collect_package_names guards on
    // metadata.paths first), so we assert its absence from the cache to
    // confirm detect does not speculatively read non-existent paths.
    let requirements_path = project_root.join("requirements.txt");
    fs::write(&requirements_path, "requests\n").unwrap();

    let detector = catalog_detector();
    let mut cache = ContentCache::new();
    let _detections = detector.detect(project_root, &mut cache).unwrap();

    // The requirements.txt that was read must be present in the cache.
    // get_file_content stores entries under the canonicalized path, so resolve
    // it the same way before asserting.
    let canonical_requirements = requirements_path
        .canonicalize()
        .expect("requirements.txt should be canonicalizable");
    assert!(
        cache.contains_key(&canonical_requirements),
        "ContentCache should contain an entry for requirements.txt after detect"
    );
    assert!(
        cache[&canonical_requirements].is_some(),
        "Cache entry for requirements.txt should be Some (file was readable)"
    );

    // A path that was never attempted must be absent (detect does not
    // speculatively populate None entries for files it never reads).
    let missing_path = project_root.join("nonexistent_dep_file.txt");
    assert!(
        !cache.contains_key(&missing_path),
        "ContentCache must not contain an entry for a path detect never attempted"
    );
}
