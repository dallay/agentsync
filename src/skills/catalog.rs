use crate::skills::provider::{Provider, ProviderCatalogMetadata};
use crate::skills::suggest::{
    DetectionConfidence, SkillSuggestion, TechnologyDetection, TechnologyId,
};
use std::collections::BTreeMap;
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogRule {
    pub skill_id: String,
    pub technologies: Vec<TechnologyId>,
    pub min_confidence: DetectionConfidence,
    pub reason_template: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSkillMetadata {
    pub skill_id: String,
    pub title: String,
    pub summary: String,
}

pub trait SkillCatalog {
    fn source_name(&self) -> &str;
    fn metadata_version(&self) -> &str;
    fn list_rules(&self) -> &[CatalogRule];
    fn get_skill(&self, skill_id: &str) -> Option<&CatalogSkillMetadata>;
}

#[derive(Debug, Clone)]
pub struct EmbeddedSkillCatalog {
    skills: Vec<CatalogSkillMetadata>,
    rules: Vec<CatalogRule>,
}

impl Default for EmbeddedSkillCatalog {
    fn default() -> Self {
        Self {
            skills: vec![
                skill(
                    "accessibility",
                    "Accessibility",
                    "Audit and improve web accessibility following WCAG guidance.",
                ),
                skill(
                    "best-practices",
                    "Best Practices",
                    "Apply modern development best practices for security, compatibility, and code quality.",
                ),
                skill(
                    "core-web-vitals",
                    "Core Web Vitals",
                    "Optimize LCP, INP, and CLS for better page experience.",
                ),
                skill(
                    "docker-expert",
                    "Docker Expert",
                    "Improve containerization, image optimization, and Compose workflows.",
                ),
                skill(
                    "frontend-design",
                    "Frontend Design",
                    "Create polished, production-grade frontend interfaces.",
                ),
                skill(
                    "github-actions",
                    "GitHub Actions",
                    "Build and review robust GitHub Actions workflows.",
                ),
                skill(
                    "makefile",
                    "Makefile",
                    "Author clean, maintainable, portable GNU Make automation.",
                ),
                skill(
                    "performance",
                    "Performance",
                    "Optimize application performance and loading behavior.",
                ),
                skill(
                    "pinned-tag",
                    "Pinned Tag",
                    "Pin mutable GitHub Actions tags to immutable commit SHAs.",
                ),
                skill(
                    "rust-async-patterns",
                    "Rust Async Patterns",
                    "Use Tokio and idiomatic async Rust implementation patterns.",
                ),
                skill(
                    "seo",
                    "SEO",
                    "Improve search engine visibility and metadata quality.",
                ),
            ],
            rules: vec![
                rule(
                    "rust-async-patterns",
                    vec![TechnologyId::Rust],
                    DetectionConfidence::Medium,
                    "Recommended because {technology} was detected from {evidence}.",
                ),
                rule(
                    "best-practices",
                    vec![TechnologyId::NodeTypeScript],
                    DetectionConfidence::Medium,
                    "Recommended because {technology} was detected from {evidence}.",
                ),
                rule(
                    "best-practices",
                    vec![TechnologyId::Python],
                    DetectionConfidence::Medium,
                    "Recommended because {technology} was detected from {evidence}.",
                ),
                rule(
                    "frontend-design",
                    vec![TechnologyId::Astro],
                    DetectionConfidence::Medium,
                    "Recommended because {technology} was detected from {evidence}.",
                ),
                rule(
                    "accessibility",
                    vec![TechnologyId::Astro],
                    DetectionConfidence::Medium,
                    "Recommended because {technology} was detected from {evidence}.",
                ),
                rule(
                    "performance",
                    vec![TechnologyId::Astro],
                    DetectionConfidence::Medium,
                    "Recommended because {technology} was detected from {evidence}.",
                ),
                rule(
                    "core-web-vitals",
                    vec![TechnologyId::Astro],
                    DetectionConfidence::Medium,
                    "Recommended because {technology} was detected from {evidence}.",
                ),
                rule(
                    "seo",
                    vec![TechnologyId::Astro],
                    DetectionConfidence::Medium,
                    "Recommended because {technology} was detected from {evidence}.",
                ),
                rule(
                    "github-actions",
                    vec![TechnologyId::GitHubActions],
                    DetectionConfidence::Medium,
                    "Recommended because {technology} workflows were detected from {evidence}.",
                ),
                rule(
                    "pinned-tag",
                    vec![TechnologyId::GitHubActions],
                    DetectionConfidence::Medium,
                    "Recommended because {technology} workflows were detected from {evidence}.",
                ),
                rule(
                    "docker-expert",
                    vec![TechnologyId::Docker],
                    DetectionConfidence::Medium,
                    "Recommended because {technology} was detected from {evidence}.",
                ),
                rule(
                    "makefile",
                    vec![TechnologyId::Make],
                    DetectionConfidence::Medium,
                    "Recommended because {technology} was detected from {evidence}.",
                ),
            ],
        }
    }
}

impl SkillCatalog for EmbeddedSkillCatalog {
    fn source_name(&self) -> &str {
        "embedded"
    }

    fn metadata_version(&self) -> &str {
        "v1"
    }

    fn list_rules(&self) -> &[CatalogRule] {
        &self.rules
    }

    fn get_skill(&self, skill_id: &str) -> Option<&CatalogSkillMetadata> {
        self.skills.iter().find(|skill| skill.skill_id == skill_id)
    }
}

#[derive(Debug, Clone)]
struct ProviderBackedCatalog {
    source_name: String,
    metadata_version: String,
    skills: Vec<CatalogSkillMetadata>,
    rules: Vec<CatalogRule>,
}

impl SkillCatalog for ProviderBackedCatalog {
    fn source_name(&self) -> &str {
        &self.source_name
    }

    fn metadata_version(&self) -> &str {
        &self.metadata_version
    }

    fn list_rules(&self) -> &[CatalogRule] {
        &self.rules
    }

    fn get_skill(&self, skill_id: &str) -> Option<&CatalogSkillMetadata> {
        self.skills.iter().find(|skill| skill.skill_id == skill_id)
    }
}

impl ProviderBackedCatalog {
    fn from_metadata(metadata: ProviderCatalogMetadata) -> Option<Self> {
        let skills = metadata
            .skills
            .into_iter()
            .map(|skill| CatalogSkillMetadata {
                skill_id: skill.skill_id,
                title: skill.title,
                summary: skill.summary,
            })
            .collect::<Vec<_>>();

        let rules = metadata
            .rules
            .into_iter()
            .filter_map(|rule| {
                let original_technologies = rule.technologies;
                let technologies = original_technologies
                    .iter()
                    .filter_map(|technology| TechnologyId::from_catalog_key(technology))
                    .collect::<Vec<_>>();

                if technologies.is_empty() {
                    warn!(
                        skill_id = %rule.skill_id,
                        ?original_technologies,
                        "Skipping provider recommendation rule without valid technologies"
                    );
                    return None;
                }

                let min_confidence = DetectionConfidence::from_catalog_key(&rule.min_confidence)?;

                Some(CatalogRule {
                    skill_id: rule.skill_id,
                    technologies,
                    min_confidence,
                    reason_template: rule.reason_template,
                })
            })
            .collect::<Vec<_>>();

        if rules.is_empty() {
            warn!(
                provider = %metadata.provider,
                version = %metadata.version,
                "Skipping provider recommendation catalog without usable rules"
            );
            return None;
        }

        Some(Self {
            source_name: metadata.provider,
            metadata_version: metadata.version,
            skills,
            rules,
        })
    }
}

pub fn load_catalog(provider: Option<&dyn Provider>) -> Box<dyn SkillCatalog> {
    if let Some(provider) = provider
        && let Ok(Some(metadata)) = provider.recommendation_catalog()
        && let Some(catalog) = ProviderBackedCatalog::from_metadata(metadata)
    {
        return Box::new(catalog);
    }

    Box::new(EmbeddedSkillCatalog::default())
}

pub fn recommend_skills(
    catalog: &dyn SkillCatalog,
    detections: &[TechnologyDetection],
) -> Vec<SkillSuggestion> {
    let detections_by_technology = detections
        .iter()
        .map(|detection| (detection.technology, detection))
        .collect::<BTreeMap<_, _>>();

    let mut suggestions = BTreeMap::<String, SkillSuggestion>::new();

    for rule in catalog.list_rules() {
        let matched_detections = rule
            .technologies
            .iter()
            .filter_map(|technology| detections_by_technology.get(technology).copied())
            .filter(|detection| detection.confidence >= rule.min_confidence)
            .collect::<Vec<_>>();

        if matched_detections.is_empty() {
            continue;
        }

        let Some(metadata) = catalog.get_skill(&rule.skill_id) else {
            continue;
        };

        let suggestion = suggestions
            .entry(rule.skill_id.clone())
            .or_insert_with(|| SkillSuggestion::new(metadata, catalog));

        for detection in matched_detections {
            suggestion.add_match(detection, &rule.reason_template);
        }
    }

    suggestions.into_values().collect()
}

fn skill(skill_id: &str, title: &str, summary: &str) -> CatalogSkillMetadata {
    CatalogSkillMetadata {
        skill_id: skill_id.to_string(),
        title: title.to_string(),
        summary: summary.to_string(),
    }
}

fn rule(
    skill_id: &str,
    technologies: Vec<TechnologyId>,
    min_confidence: DetectionConfidence,
    reason_template: &str,
) -> CatalogRule {
    CatalogRule {
        skill_id: skill_id.to_string(),
        technologies,
        min_confidence,
        reason_template: reason_template.to_string(),
    }
}
