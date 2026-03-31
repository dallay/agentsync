use agentsync::skills::catalog::{
    EmbeddedSkillCatalog, load_catalog, overlay_catalog, parse_catalog, parse_embedded_catalog,
    recommend_skills,
};
use agentsync::skills::provider::{
    Provider, ProviderCatalogCombo, ProviderCatalogMetadata, ProviderCatalogSkill,
    ProviderCatalogTechnology, SkillInstallInfo,
};
use agentsync::skills::registry::InstalledSkillState;
use agentsync::skills::suggest::{
    DetectionConfidence, DetectionEvidence, SuggestionService, TechnologyDetection, TechnologyId,
    annotate_recommendations,
};
use anyhow::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[test]
fn embedded_catalog_loads_expected_baseline_entries() {
    let catalog = EmbeddedSkillCatalog::default();

    let astro = catalog.get_technology(TechnologyId::Astro).unwrap();
    assert_eq!(astro.name, "Astro");
    assert_eq!(astro.skills.len(), 5);

    let combo = catalog.get_combo("astro-github-actions").unwrap();
    assert_eq!(
        combo.requires,
        vec![TechnologyId::Astro, TechnologyId::GitHubActions]
    );
    assert!(!combo.enabled);
}

#[test]
fn invalid_embedded_catalog_fails_explicitly() {
    let error = parse_embedded_catalog(
        r#"
version = "v1"

[[skills]]
provider_skill_id = "broken/example"
local_skill_id = "broken"
title = "Broken"
summary = "Broken"

[[technologies]]
id = "rust"
name = "Rust"
skills = []
"#,
    )
    .unwrap_err();

    assert!(error.to_string().contains("technology.skills"));
}

#[test]
fn merges_duplicate_skill_recommendations_across_multiple_technologies() {
    let catalog = EmbeddedSkillCatalog::default();
    let detections = vec![
        detection(
            TechnologyId::NodeTypeScript,
            DetectionConfidence::High,
            "package.json",
        ),
        detection(
            TechnologyId::Python,
            DetectionConfidence::High,
            "pyproject.toml",
        ),
    ];

    let recommendations = recommend_skills(&catalog, &detections);
    let best_practices = recommendations
        .iter()
        .find(|recommendation| recommendation.skill_id == "best-practices")
        .unwrap();

    assert_eq!(best_practices.matched_technologies.len(), 2);
    assert!(
        best_practices
            .matched_technologies
            .contains(&TechnologyId::NodeTypeScript)
    );
    assert!(
        best_practices
            .matched_technologies
            .contains(&TechnologyId::Python)
    );
    assert_eq!(best_practices.reasons.len(), 2);
}

#[test]
fn canonical_provider_skill_ids_use_local_aliases_in_recommendations() {
    let provider = CanonicalCatalogProvider;
    let catalog = load_catalog(Some(&provider)).unwrap();
    let detections = vec![detection(
        TechnologyId::Rust,
        DetectionConfidence::High,
        "Cargo.toml",
    )];

    let recommendations = recommend_skills(&catalog, &detections);
    let custom = recommendations
        .iter()
        .find(|recommendation| recommendation.skill_id == "custom-rust")
        .unwrap();

    assert_eq!(catalog.source_name(), "canonical-provider");
    assert_eq!(catalog.metadata_version(), "2026.03");
    assert_eq!(custom.skill_id, "custom-rust");
    assert!(provider.resolve(&custom.skill_id).is_ok());
    assert!(
        catalog
            .get_skill_definition("acme/skills/rust-custom")
            .is_some()
    );
    assert!(catalog.get_skill("custom-rust").is_some());
}

#[test]
fn annotates_installed_state_without_hiding_recommendations() {
    let catalog = EmbeddedSkillCatalog::default();
    let detections = vec![detection(
        TechnologyId::Docker,
        DetectionConfidence::High,
        "Dockerfile",
    )];
    let mut recommendations = recommend_skills(&catalog, &detections);
    let installed_states = BTreeMap::from([(
        "docker-expert".to_string(),
        InstalledSkillState {
            installed: true,
            version: Some("1.2.3".to_string()),
        },
    )]);

    annotate_recommendations(&mut recommendations, &installed_states);

    assert_eq!(recommendations.len(), 1);
    assert!(recommendations[0].installed);
    assert_eq!(
        recommendations[0].installed_version.as_deref(),
        Some("1.2.3")
    );
}

#[test]
fn falls_back_to_embedded_catalog_when_provider_has_no_metadata() {
    let provider = EmptyCatalogProvider;
    let catalog = load_catalog(Some(&provider)).unwrap();

    assert_eq!(catalog.source_name(), "embedded");
    assert_eq!(catalog.metadata_version(), "v1");
    assert!(catalog.get_skill("docker-expert").is_some());
}

#[test]
fn falls_back_to_embedded_catalog_when_provider_schema_is_invalid() {
    let provider = InvalidSchemaCatalogProvider;
    let catalog = load_catalog(Some(&provider)).unwrap();

    assert_eq!(catalog.source_name(), "embedded");
    assert!(catalog.get_skill("rust-async-patterns").is_some());
}

#[test]
fn provider_overlay_can_override_existing_technology_mapping() {
    let provider = OverrideCatalogProvider;
    let catalog = load_catalog(Some(&provider)).unwrap();
    let detections = vec![detection(
        TechnologyId::Rust,
        DetectionConfidence::High,
        "Cargo.toml",
    )];

    let recommendations = recommend_skills(&catalog, &detections);
    assert!(
        recommendations
            .iter()
            .any(|recommendation| recommendation.skill_id == "best-practices")
    );
    assert!(
        !recommendations
            .iter()
            .any(|recommendation| recommendation.skill_id == "rust-async-patterns")
    );
}

#[test]
fn provider_overlay_can_extend_baseline_with_new_supported_technology_entry() {
    let baseline = parse_catalog(
        r#"
version = "v1"

[[skills]]
provider_skill_id = "rust-async-patterns"
local_skill_id = "rust-async-patterns"
title = "Rust Async Patterns"
summary = "Rust async guidance"

[[skills]]
provider_skill_id = "makefile"
local_skill_id = "makefile"
title = "Makefile"
summary = "Make guidance"

[[technologies]]
id = "rust"
name = "Rust"
skills = ["rust-async-patterns"]
"#,
        "fixture",
        "fixture-v1",
    )
    .unwrap();

    let provider_catalog = ProviderCatalogMetadata {
        provider: "extension-provider".to_string(),
        version: "2026.03".to_string(),
        schema_version: "v1".to_string(),
        skills: vec![],
        technologies: vec![provider_technology("make", "Make", &["makefile"])],
        combos: vec![],
    };

    let catalog = overlay_catalog(baseline, provider_catalog)
        .unwrap()
        .unwrap();

    assert_eq!(catalog.source_name(), "extension-provider");
    assert!(catalog.get_technology(TechnologyId::Rust).is_some());
    let make = catalog.get_technology(TechnologyId::Make).unwrap();
    assert_eq!(make.name, "Make");
    assert_eq!(make.skills, vec!["makefile"]);
}

#[test]
fn provider_overlay_prefers_combo_override_by_stable_id() {
    let baseline = parse_catalog(
        r#"
version = "v1"

[[skills]]
provider_skill_id = "docker-expert"
local_skill_id = "docker-expert"
title = "Docker Expert"
summary = "Docker guidance"

[[skills]]
provider_skill_id = "best-practices"
local_skill_id = "best-practices"
title = "Best Practices"
summary = "Best practices guidance"

[[combos]]
id = "rust-docker"
name = "Rust + Docker"
requires = ["rust", "docker"]
skills = ["docker-expert"]
enabled = false
"#,
        "fixture",
        "fixture-v1",
    )
    .unwrap();

    let catalog = overlay_catalog(
        baseline,
        ProviderCatalogMetadata {
            provider: "combo-provider".to_string(),
            version: "2026.03".to_string(),
            schema_version: "v1".to_string(),
            skills: vec![],
            technologies: vec![],
            combos: vec![ProviderCatalogCombo {
                id: "rust-docker".to_string(),
                name: "Rust + Docker Override".to_string(),
                requires: vec!["rust".to_string(), "docker".to_string()],
                skills: vec!["best-practices".to_string()],
                enabled: Some(true),
                reason_template: Some("Provider combo override".to_string()),
            }],
        },
    )
    .unwrap()
    .unwrap();

    let combo = catalog.get_combo("rust-docker").unwrap();
    assert_eq!(combo.name, "Rust + Docker Override");
    assert_eq!(combo.skills, vec!["best-practices"]);
    assert!(combo.enabled);
    assert_eq!(
        catalog
            .combos()
            .filter(|combo| combo.id == "rust-docker")
            .count(),
        1
    );
}

#[test]
fn partially_invalid_provider_metadata_keeps_valid_overlay_entries() {
    let provider = PartiallyInvalidCatalogProvider;
    let catalog = load_catalog(Some(&provider)).unwrap();
    let detections = vec![
        detection(TechnologyId::Rust, DetectionConfidence::High, "Cargo.toml"),
        detection(
            TechnologyId::Docker,
            DetectionConfidence::High,
            "Dockerfile",
        ),
    ];

    let recommendations = recommend_skills(&catalog, &detections);
    assert!(
        recommendations
            .iter()
            .any(|recommendation| recommendation.skill_id == "custom-rust")
    );
    assert!(
        recommendations
            .iter()
            .any(|recommendation| recommendation.skill_id == "docker-expert")
    );
    assert!(catalog.get_combo("valid-provider-combo").is_some());
    assert!(catalog.get_combo("invalid-provider-combo").is_none());
}

#[test]
fn suggest_reports_detections_when_catalog_has_no_matching_rules() {
    let temp_dir = TempDir::new().unwrap();
    let service = SuggestionService;
    let detector = StaticDetector::new(vec![detection(
        TechnologyId::Rust,
        DetectionConfidence::Medium,
        "Cargo.toml",
    )]);
    let provider = NoMatchCatalogProvider;

    let response = service
        .suggest_with(temp_dir.path(), &detector, Some(&provider))
        .unwrap();

    assert_eq!(response.detections.len(), 1);
    assert_eq!(response.detections[0].technology, TechnologyId::Rust);
    assert!(response.recommendations.is_empty());
    assert_eq!(response.summary.detected_count, 1);
    assert_eq!(response.summary.recommended_count, 0);
    assert_eq!(response.summary.installable_count, 0);
}

#[test]
fn provider_detect_metadata_does_not_change_detection_results() {
    let temp_dir = TempDir::new().unwrap();
    let service = SuggestionService;
    let detector = StaticDetector::new(vec![detection(
        TechnologyId::Rust,
        DetectionConfidence::High,
        "Cargo.toml",
    )]);
    let provider = DetectMetadataCatalogProvider;

    let response = service
        .suggest_with(temp_dir.path(), &detector, Some(&provider))
        .unwrap();

    assert_eq!(response.detections.len(), 1);
    assert_eq!(response.detections[0].technology, TechnologyId::Rust);
    assert!(
        response
            .recommendations
            .iter()
            .any(|r| r.skill_id == "rust-async-patterns")
    );
    assert!(
        !response
            .recommendations
            .iter()
            .any(|r| r.skill_id == "makefile")
    );
}

fn detection(
    technology: TechnologyId,
    confidence: DetectionConfidence,
    path: &str,
) -> TechnologyDetection {
    TechnologyDetection {
        technology,
        confidence,
        root_relative_paths: vec![PathBuf::from(path)],
        evidence: vec![DetectionEvidence {
            marker: path.to_string(),
            path: PathBuf::from(path),
            notes: None,
        }],
    }
}

fn provider_skill(provider_skill_id: &str, local_skill_id: &str) -> ProviderCatalogSkill {
    ProviderCatalogSkill {
        provider_skill_id: provider_skill_id.to_string(),
        local_skill_id: local_skill_id.to_string(),
        title: local_skill_id.to_string(),
        summary: format!("Summary for {local_skill_id}"),
    }
}

fn provider_technology(id: &str, name: &str, skills: &[&str]) -> ProviderCatalogTechnology {
    ProviderCatalogTechnology {
        id: id.to_string(),
        name: name.to_string(),
        skills: skills.iter().map(|skill| skill.to_string()).collect(),
        detect: None,
        min_confidence: Some("medium".to_string()),
        reason_template: None,
    }
}

fn provider_combo(id: &str, requires: &[&str], skills: &[&str]) -> ProviderCatalogCombo {
    ProviderCatalogCombo {
        id: id.to_string(),
        name: id.to_string(),
        requires: requires.iter().map(|value| value.to_string()).collect(),
        skills: skills.iter().map(|skill| skill.to_string()).collect(),
        enabled: Some(false),
        reason_template: None,
    }
}

struct EmptyCatalogProvider;

impl Provider for EmptyCatalogProvider {
    fn manifest(&self) -> Result<String> {
        Ok("empty".to_string())
    }

    fn resolve(&self, id: &str) -> Result<SkillInstallInfo> {
        assert_eq!(id, "custom-rust");
        Ok(SkillInstallInfo {
            download_url: "https://example.com/acme/skills/rust-custom.zip".to_string(),
            format: "zip".to_string(),
        })
    }
}

struct CanonicalCatalogProvider;

impl Provider for CanonicalCatalogProvider {
    fn manifest(&self) -> Result<String> {
        Ok("canonical".to_string())
    }

    fn resolve(&self, id: &str) -> Result<SkillInstallInfo> {
        assert_eq!(id, "custom-rust");
        Ok(SkillInstallInfo {
            download_url: "https://example.com/acme/skills/rust-custom.zip".to_string(),
            format: "zip".to_string(),
        })
    }

    fn recommendation_catalog(&self) -> Result<Option<ProviderCatalogMetadata>> {
        Ok(Some(ProviderCatalogMetadata {
            provider: "canonical-provider".to_string(),
            version: "2026.03".to_string(),
            schema_version: "v1".to_string(),
            skills: vec![provider_skill("acme/skills/rust-custom", "custom-rust")],
            technologies: vec![provider_technology(
                "rust",
                "Rust",
                &["acme/skills/rust-custom"],
            )],
            combos: vec![],
        }))
    }
}

struct OverrideCatalogProvider;

impl Provider for OverrideCatalogProvider {
    fn manifest(&self) -> Result<String> {
        Ok("override".to_string())
    }

    fn resolve(&self, _id: &str) -> Result<SkillInstallInfo> {
        unreachable!()
    }

    fn recommendation_catalog(&self) -> Result<Option<ProviderCatalogMetadata>> {
        Ok(Some(ProviderCatalogMetadata {
            provider: "override-provider".to_string(),
            version: "2026.03".to_string(),
            schema_version: "v1".to_string(),
            skills: vec![provider_skill("best-practices", "best-practices")],
            technologies: vec![provider_technology("rust", "Rust", &["best-practices"])],
            combos: vec![],
        }))
    }
}

struct PartiallyInvalidCatalogProvider;

impl Provider for PartiallyInvalidCatalogProvider {
    fn manifest(&self) -> Result<String> {
        Ok("partial".to_string())
    }

    fn resolve(&self, _id: &str) -> Result<SkillInstallInfo> {
        unreachable!()
    }

    fn recommendation_catalog(&self) -> Result<Option<ProviderCatalogMetadata>> {
        Ok(Some(ProviderCatalogMetadata {
            provider: "partial-provider".to_string(),
            version: "2026.03".to_string(),
            schema_version: "v1".to_string(),
            skills: vec![
                provider_skill("acme/skills/rust-custom", "custom-rust"),
                provider_skill("acme/skills/bad", "docker-expert"),
            ],
            technologies: vec![
                provider_technology("rust", "Rust", &["acme/skills/rust-custom"]),
                provider_technology("docker", "Docker", &["missing/skill"]),
            ],
            combos: vec![
                provider_combo(
                    "valid-provider-combo",
                    &["rust", "docker"],
                    &["acme/skills/rust-custom"],
                ),
                provider_combo("invalid-provider-combo", &["rust"], &["missing/skill"]),
            ],
        }))
    }
}

struct InvalidSchemaCatalogProvider;

impl Provider for InvalidSchemaCatalogProvider {
    fn manifest(&self) -> Result<String> {
        Ok("invalid-schema".to_string())
    }

    fn resolve(&self, _id: &str) -> Result<SkillInstallInfo> {
        unreachable!()
    }

    fn recommendation_catalog(&self) -> Result<Option<ProviderCatalogMetadata>> {
        Ok(Some(ProviderCatalogMetadata {
            provider: "invalid-schema-provider".to_string(),
            version: "2026.03".to_string(),
            schema_version: "v99".to_string(),
            skills: vec![provider_skill("acme/skills/rust-custom", "custom-rust")],
            technologies: vec![provider_technology(
                "rust",
                "Rust",
                &["acme/skills/rust-custom"],
            )],
            combos: vec![],
        }))
    }
}

struct StaticDetector {
    detections: Vec<TechnologyDetection>,
}

impl StaticDetector {
    fn new(detections: Vec<TechnologyDetection>) -> Self {
        Self { detections }
    }
}

impl agentsync::skills::detect::RepoDetector for StaticDetector {
    fn detect(&self, _project_root: &Path) -> Result<Vec<TechnologyDetection>> {
        Ok(self.detections.clone())
    }
}

struct NoMatchCatalogProvider;

impl Provider for NoMatchCatalogProvider {
    fn manifest(&self) -> Result<String> {
        Ok("no-match".to_string())
    }

    fn resolve(&self, _id: &str) -> Result<SkillInstallInfo> {
        unreachable!()
    }

    fn recommendation_catalog(&self) -> Result<Option<ProviderCatalogMetadata>> {
        Ok(Some(ProviderCatalogMetadata {
            provider: "no-match-provider".to_string(),
            version: "2026.03".to_string(),
            schema_version: "v1".to_string(),
            skills: vec![provider_skill("acme/skills/no-match", "custom-python")],
            technologies: vec![ProviderCatalogTechnology {
                id: "rust".to_string(),
                name: "Rust".to_string(),
                skills: vec!["acme/skills/no-match".to_string()],
                detect: None,
                min_confidence: Some("high".to_string()),
                reason_template: None,
            }],
            combos: vec![],
        }))
    }
}

struct DetectMetadataCatalogProvider;

impl Provider for DetectMetadataCatalogProvider {
    fn manifest(&self) -> Result<String> {
        Ok("detect-metadata".to_string())
    }

    fn resolve(&self, _id: &str) -> Result<SkillInstallInfo> {
        unreachable!()
    }

    fn recommendation_catalog(&self) -> Result<Option<ProviderCatalogMetadata>> {
        let detect =
            toml::from_str::<toml::Value>("markers = ['Makefile']\npath_globs = ['frontend/**']")?;

        Ok(Some(ProviderCatalogMetadata {
            provider: "detect-metadata-provider".to_string(),
            version: "2026.03".to_string(),
            schema_version: "v1".to_string(),
            skills: vec![provider_skill("makefile", "makefile")],
            technologies: vec![ProviderCatalogTechnology {
                id: "make".to_string(),
                name: "Make".to_string(),
                skills: vec!["makefile".to_string()],
                detect: Some(detect),
                min_confidence: Some("low".to_string()),
                reason_template: None,
            }],
            combos: vec![],
        }))
    }
}
