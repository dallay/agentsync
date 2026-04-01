use agentsync::skills::provider::{
    Provider, ProviderCatalogMetadata, ProviderCatalogSkill, ProviderCatalogTechnology,
    SkillInstallInfo,
};
use agentsync::skills::suggest::{
    DetectionConfidence, DetectionEvidence, SuggestionService, TechnologyDetection, TechnologyId,
};
use anyhow::Result;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn agentsync_bin() -> &'static str {
    env!("CARGO_BIN_EXE_agentsync")
}

#[test]
fn skill_suggest_json_is_read_only_and_marks_installed_skills() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();

    let skills_dir = root.join(".agents/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    let registry_path = skills_dir.join("registry.json");
    let registry_body = serde_json::json!({
        "schemaVersion": 1,
        "last_updated": "2026-03-30T00:00:00Z",
        "skills": {
            "docker-expert": {
                "name": "docker-expert",
                "version": "1.2.3"
            }
        }
    });
    let registry_body = serde_json::to_string_pretty(&registry_body).unwrap();
    fs::write(&registry_path, &registry_body).unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .args(["skill", "suggest", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --json");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let recommendations = response["recommendations"].as_array().unwrap();
    let docker = recommendations
        .iter()
        .find(|recommendation| recommendation["skill_id"] == "docker-expert")
        .unwrap();
    assert_eq!(docker["installed"], true);

    let registry_after = fs::read_to_string(&registry_path).unwrap();
    assert_eq!(registry_after, registry_body);
    assert!(!skills_dir.join("rust-async-patterns").exists());
}

#[test]
fn skill_suggest_human_output_reports_empty_results() {
    let temp_dir = TempDir::new().unwrap();
    let output = Command::new(agentsync_bin())
        .current_dir(temp_dir.path())
        .args(["skill", "suggest"])
        .output()
        .expect("failed to run agentsync skill suggest");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Detected technologies: none"), "{stdout}");
    assert!(stdout.contains("Recommended skills: none"), "{stdout}");
    assert!(
        stdout.contains("Summary: 0 detected, 0 recommended, 0 installable"),
        "{stdout}"
    );
}

#[test]
fn skill_suggest_install_requires_tty_without_all_flag() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .args(["skill", "suggest", "--install", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --install --json");

    assert!(!output.status.success());
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["code"], "interactive_tty_required");
    assert!(
        response["remediation"]
            .as_str()
            .unwrap()
            .contains("--install --all")
    );
}

#[test]
fn skill_suggest_install_all_installs_pending_recommendations_and_skips_installed() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();

    let source_root = root.join("skill-sources");
    create_skill_source(&source_root, "rust-async-patterns");
    create_skill_source(&source_root, "docker-expert");

    let skills_dir = root.join(".agents/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(
        skills_dir.join("registry.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "last_updated": "2026-03-30T00:00:00Z",
            "skills": {
                "docker-expert": {
                    "name": "docker-expert",
                    "version": "1.2.3"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .env("AGENTSYNC_TEST_SKILL_SOURCE_DIR", &source_root)
        .args(["skill", "suggest", "--install", "--all", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --install --all --json");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["mode"], "install_all");
    let results = response["results"].as_array().unwrap();
    let rust = results
        .iter()
        .find(|result| result["skill_id"] == "rust-async-patterns")
        .unwrap();
    let docker = results
        .iter()
        .find(|result| result["skill_id"] == "docker-expert")
        .unwrap();
    assert_eq!(rust["status"], "installed");
    assert_eq!(docker["status"], "already_installed");
    assert!(root.join(".agents/skills/rust-async-patterns").exists());
}

#[test]
fn skill_suggest_install_all_is_a_no_op_when_everything_is_already_installed() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();

    let skills_dir = root.join(".agents/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    let registry_path = skills_dir.join("registry.json");
    let registry_body = serde_json::json!({
        "schemaVersion": 1,
        "last_updated": "2026-03-30T00:00:00Z",
        "skills": {
            "docker-expert": {
                "name": "docker-expert",
                "version": "1.2.3"
            },
            "rust-async-patterns": {
                "name": "rust-async-patterns",
                "version": "1.0.0"
            }
        }
    });
    let registry_body = serde_json::to_string_pretty(&registry_body).unwrap();
    fs::write(&registry_path, &registry_body).unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .args(["skill", "suggest", "--install", "--all", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --install --all --json");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["summary"]["installable_count"], 0);

    let results = response["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert!(
        results
            .iter()
            .all(|result| result["status"] == "already_installed")
    );

    let registry_after = fs::read_to_string(&registry_path).unwrap();
    assert_eq!(registry_after, registry_body);
    assert!(!skills_dir.join("docker-expert").exists());
    assert!(!skills_dir.join("rust-async-patterns").exists());
}

#[test]
fn skill_suggest_install_all_surfaces_direct_install_failure_semantics() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();

    let source_root = root.join("skill-sources");
    let failing_source = source_root.join("rust-async-patterns");
    fs::create_dir_all(&failing_source).unwrap();

    let suggest_output = Command::new(agentsync_bin())
        .current_dir(root)
        .env("AGENTSYNC_TEST_SKILL_SOURCE_DIR", &source_root)
        .args(["skill", "suggest", "--install", "--all", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --install --all --json");

    assert!(suggest_output.status.success());

    let direct_output = Command::new(agentsync_bin())
        .current_dir(root)
        .args([
            "skill",
            "install",
            "rust-async-patterns",
            "--source",
            failing_source.to_str().unwrap(),
            "--json",
        ])
        .output()
        .expect("failed to run direct agentsync skill install --json");

    assert!(!direct_output.status.success());

    let suggest_response: serde_json::Value =
        serde_json::from_slice(&suggest_output.stdout).unwrap();
    let direct_response: serde_json::Value = serde_json::from_slice(&direct_output.stdout).unwrap();

    let failed_result = suggest_response["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["skill_id"] == "rust-async-patterns")
        .unwrap();

    assert_eq!(failed_result["status"], "failed");
    assert_eq!(failed_result["error_message"], direct_response["error"]);
}

#[test]
fn suggestion_service_preserves_local_install_lookup_with_provider_overlay() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::create_dir_all(root.join(".agents/skills")).unwrap();
    fs::write(
        root.join(".agents/skills/registry.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "last_updated": "2026-03-30T00:00:00Z",
            "skills": {
                "custom-rust": {
                    "name": "custom-rust",
                    "version": "1.0.0"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let detector = StaticDetector;
    let provider = CanonicalOverlayProvider;
    let response = SuggestionService
        .suggest_with(root, &detector, Some(&provider))
        .unwrap();

    let recommendation = response
        .recommendations
        .iter()
        .find(|recommendation| recommendation.skill_id == "custom-rust")
        .unwrap();

    assert!(recommendation.installed);
    assert_eq!(recommendation.installed_version.as_deref(), Some("1.0.0"));
    assert_eq!(
        recommendation.matched_technologies,
        vec![TechnologyId::new(TechnologyId::RUST)]
    );
}

fn create_skill_source(source_root: &std::path::Path, skill_id: &str) {
    let source_dir = source_root.join(skill_id);
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("SKILL.md"),
        format!("---\nname: {skill_id}\nversion: 1.0.0\n---\n# {skill_id}\n"),
    )
    .unwrap();
}

struct StaticDetector;

impl agentsync::skills::detect::RepoDetector for StaticDetector {
    fn detect(&self, _project_root: &std::path::Path) -> Result<Vec<TechnologyDetection>> {
        Ok(vec![TechnologyDetection {
            technology: TechnologyId::new(TechnologyId::RUST),
            confidence: DetectionConfidence::High,
            root_relative_paths: vec!["Cargo.toml".into()],
            evidence: vec![DetectionEvidence {
                marker: "Cargo.toml".to_string(),
                path: "Cargo.toml".into(),
                notes: None,
            }],
        }])
    }
}

struct CanonicalOverlayProvider;

impl Provider for CanonicalOverlayProvider {
    fn manifest(&self) -> Result<String> {
        Ok("canonical-overlay".to_string())
    }

    fn resolve(&self, _id: &str) -> Result<SkillInstallInfo> {
        unreachable!()
    }

    fn recommendation_catalog(&self) -> Result<Option<ProviderCatalogMetadata>> {
        Ok(Some(ProviderCatalogMetadata {
            provider: "canonical-overlay".to_string(),
            version: "2026.03".to_string(),
            schema_version: "v1".to_string(),
            skills: vec![ProviderCatalogSkill {
                provider_skill_id: "acme/skills/custom-rust".to_string(),
                local_skill_id: "custom-rust".to_string(),
                title: "Custom Rust".to_string(),
                summary: "Custom Rust guidance".to_string(),
            }],
            technologies: vec![ProviderCatalogTechnology {
                id: "rust".to_string(),
                name: "Rust".to_string(),
                skills: vec!["acme/skills/custom-rust".to_string()],
                detect: None,
                min_confidence: Some("medium".to_string()),
                reason_template: None,
            }],
            combos: vec![],
        }))
    }
}
