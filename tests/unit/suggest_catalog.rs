use agentsync::skills::catalog::{EmbeddedSkillCatalog, load_catalog, recommend_skills};
use agentsync::skills::provider::{
    Provider, ProviderCatalogMetadata, ProviderCatalogRule, ProviderCatalogSkill, SkillInstallInfo,
};
use agentsync::skills::registry::InstalledSkillState;
use agentsync::skills::suggest::{
    DetectionConfidence, DetectionEvidence, SuggestionService, TechnologyDetection, TechnologyId,
    annotate_recommendations,
};
use anyhow::Result;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tempfile::TempDir;

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
    let catalog = load_catalog(Some(&provider));

    assert_eq!(catalog.source_name(), "embedded");
    assert_eq!(catalog.metadata_version(), "v1");
    assert!(catalog.get_skill("docker-expert").is_some());
}

#[test]
fn uses_provider_catalog_when_metadata_is_available() {
    let provider = FakeCatalogProvider;
    let catalog = load_catalog(Some(&provider));

    assert_eq!(catalog.source_name(), "fake-provider");
    assert_eq!(catalog.metadata_version(), "2026.03");
    assert!(catalog.get_skill("custom-skill").is_some());
}

#[test]
fn suggest_reports_detections_when_catalog_has_no_matching_rules() {
    let temp_dir = TempDir::new().unwrap();
    let service = SuggestionService;
    let detector = StaticDetector::new(vec![detection(
        TechnologyId::Rust,
        DetectionConfidence::High,
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

struct EmptyCatalogProvider;

impl Provider for EmptyCatalogProvider {
    fn manifest(&self) -> Result<String> {
        Ok("empty".to_string())
    }

    fn resolve(&self, _id: &str) -> Result<SkillInstallInfo> {
        unreachable!()
    }
}

struct FakeCatalogProvider;

impl Provider for FakeCatalogProvider {
    fn manifest(&self) -> Result<String> {
        Ok("fake".to_string())
    }

    fn resolve(&self, _id: &str) -> Result<SkillInstallInfo> {
        unreachable!()
    }

    fn recommendation_catalog(&self) -> Result<Option<ProviderCatalogMetadata>> {
        Ok(Some(ProviderCatalogMetadata {
            provider: "fake-provider".to_string(),
            version: "2026.03".to_string(),
            skills: vec![ProviderCatalogSkill {
                skill_id: "custom-skill".to_string(),
                title: "Custom Skill".to_string(),
                summary: "Provider-backed test metadata".to_string(),
            }],
            rules: vec![ProviderCatalogRule {
                skill_id: "custom-skill".to_string(),
                technologies: vec!["rust".to_string()],
                min_confidence: "medium".to_string(),
                reason_template: "Recommended because {technology} was detected from {evidence}."
                    .to_string(),
            }],
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
    fn detect(&self, _project_root: &std::path::Path) -> Result<Vec<TechnologyDetection>> {
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
            version: "1".to_string(),
            skills: vec![],
            rules: vec![],
        }))
    }
}
