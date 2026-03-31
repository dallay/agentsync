use crate::skills::catalog::{
    CatalogSkillMetadata, ResolvedSkillCatalog, load_catalog, recommend_skills,
};
use crate::skills::detect::{FileSystemRepoDetector, RepoDetector};
use crate::skills::install::blocking_fetch_and_install_skill;
use crate::skills::provider::{Provider, SkillsShProvider};
use crate::skills::registry::{InstalledSkillState, read_installed_skill_states};
use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TechnologyId {
    Rust,
    #[serde(rename = "node_typescript")]
    NodeTypeScript,
    Astro,
    #[serde(rename = "github_actions")]
    GitHubActions,
    Docker,
    Make,
    Python,
}

impl TechnologyId {
    pub fn as_human_label(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::NodeTypeScript => "Node/TypeScript",
            Self::Astro => "Astro",
            Self::GitHubActions => "GitHub Actions",
            Self::Docker => "Docker",
            Self::Make => "Make",
            Self::Python => "Python",
        }
    }

    pub fn from_catalog_key(value: &str) -> Option<Self> {
        match value {
            "rust" => Some(Self::Rust),
            "node_typescript" => Some(Self::NodeTypeScript),
            "astro" => Some(Self::Astro),
            "github_actions" => Some(Self::GitHubActions),
            "docker" => Some(Self::Docker),
            "make" => Some(Self::Make),
            "python" => Some(Self::Python),
            _ => None,
        }
    }
}

impl fmt::Display for TechnologyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_human_label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionConfidence {
    Low,
    Medium,
    High,
}

impl DetectionConfidence {
    pub fn as_human_label(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn from_catalog_key(value: &str) -> Option<Self> {
        match value {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DetectionEvidence {
    pub marker: String,
    pub path: PathBuf,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TechnologyDetection {
    pub technology: TechnologyId,
    pub confidence: DetectionConfidence,
    pub root_relative_paths: Vec<PathBuf>,
    pub evidence: Vec<DetectionEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSuggestion {
    pub skill_id: String,
    pub title: String,
    pub summary: String,
    pub reasons: Vec<String>,
    pub matched_technologies: Vec<TechnologyId>,
    pub installed: bool,
    pub installed_version: Option<String>,
    pub catalog_source: String,
}

impl SkillSuggestion {
    pub fn new(metadata: &CatalogSkillMetadata, catalog: &ResolvedSkillCatalog) -> Self {
        Self {
            skill_id: metadata.skill_id.clone(),
            title: metadata.title.clone(),
            summary: metadata.summary.clone(),
            reasons: Vec::new(),
            matched_technologies: Vec::new(),
            installed: false,
            installed_version: None,
            catalog_source: format!("{}:{}", catalog.source_name(), catalog.metadata_version()),
        }
    }

    pub fn add_match(&mut self, detection: &TechnologyDetection, reason_template: &str) {
        if let Err(index) = self
            .matched_technologies
            .binary_search(&detection.technology)
        {
            self.matched_technologies
                .insert(index, detection.technology);
        }

        let evidence = detection
            .evidence
            .first()
            .map(|evidence| evidence.path.display().to_string())
            .unwrap_or_else(|| detection.technology.as_human_label().to_string());
        let reason = reason_template
            .replace("{technology}", detection.technology.as_human_label())
            .replace("{evidence}", &evidence);

        if let Err(index) = self.reasons.binary_search(&reason) {
            self.reasons.insert(index, reason);
        }
    }

    pub fn annotate_installed_state(&mut self, installed_skill: Option<&InstalledSkillState>) {
        if let Some(installed_skill) = installed_skill {
            self.installed = installed_skill.installed;
            self.installed_version = installed_skill.version.clone();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SuggestSummary {
    pub detected_count: usize,
    pub recommended_count: usize,
    pub installable_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestResponse {
    pub detections: Vec<TechnologyDetection>,
    pub recommendations: Vec<SkillSuggestion>,
    pub summary: SuggestSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SuggestJsonResponse {
    pub detections: Vec<SuggestJsonDetection>,
    pub recommendations: Vec<SuggestJsonRecommendation>,
    pub summary: SuggestSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SuggestJsonDetection {
    pub technology: TechnologyId,
    pub confidence: DetectionConfidence,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SuggestJsonRecommendation {
    pub skill_id: String,
    pub matched_technologies: Vec<TechnologyId>,
    pub reasons: Vec<String>,
    pub installed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestInstallMode {
    Interactive,
    InstallAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestInstallStatus {
    Installed,
    AlreadyInstalled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SuggestInstallResult {
    pub skill_id: String,
    pub status: SuggestInstallStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SuggestInstallJsonResponse {
    #[serde(flatten)]
    pub suggest: SuggestJsonResponse,
    pub mode: SuggestInstallMode,
    pub selected_skill_ids: Vec<String>,
    pub results: Vec<SuggestInstallResult>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SuggestionService;

impl SuggestionService {
    pub fn suggest(&self, project_root: &Path) -> Result<SuggestResponse> {
        let detector = FileSystemRepoDetector;
        let provider = SkillsShProvider;
        self.suggest_with(project_root, &detector, Some(&provider))
    }

    pub fn suggest_with<D: RepoDetector>(
        &self,
        project_root: &Path,
        detector: &D,
        provider: Option<&dyn Provider>,
    ) -> Result<SuggestResponse> {
        let catalog = load_catalog(provider)?;
        let detections = detector.detect(project_root)?;
        let installed_skill_states = read_installed_skill_states(
            &project_root
                .join(".agents")
                .join("skills")
                .join("registry.json"),
        )?;

        let mut recommendations = recommend_skills(&catalog, &detections);
        for recommendation in &mut recommendations {
            recommendation
                .annotate_installed_state(installed_skill_states.get(&recommendation.skill_id));
        }

        let summary = SuggestSummary {
            detected_count: detections.len(),
            recommended_count: recommendations.len(),
            installable_count: recommendations
                .iter()
                .filter(|suggestion| !suggestion.installed)
                .count(),
        };

        Ok(SuggestResponse {
            detections,
            recommendations,
            summary,
        })
    }

    pub fn install_all(
        &self,
        project_root: &Path,
        response: &SuggestResponse,
    ) -> Result<SuggestInstallJsonResponse> {
        let provider = SkillsShProvider;
        self.install_all_with(project_root, response, &provider)
    }

    pub fn install_all_with(
        &self,
        project_root: &Path,
        response: &SuggestResponse,
        provider: &dyn Provider,
    ) -> Result<SuggestInstallJsonResponse> {
        let selected_skill_ids = response
            .recommendations
            .iter()
            .map(|recommendation| recommendation.skill_id.clone())
            .collect::<Vec<_>>();

        self.install_selected_with(
            project_root,
            response,
            provider,
            SuggestInstallMode::InstallAll,
            &selected_skill_ids,
            |skill_id, source, target_root| {
                blocking_fetch_and_install_skill(skill_id, source, target_root)
                    .map_err(|error| anyhow!(error))
            },
        )
    }

    pub fn install_selected_with<F>(
        &self,
        project_root: &Path,
        response: &SuggestResponse,
        provider: &dyn Provider,
        mode: SuggestInstallMode,
        selected_skill_ids: &[String],
        mut install_fn: F,
    ) -> Result<SuggestInstallJsonResponse>
    where
        F: FnMut(&str, &str, &Path) -> Result<()>,
    {
        let selected_skill_ids = dedupe_preserve_order(selected_skill_ids);
        let recommendation_map = response
            .recommendations
            .iter()
            .map(|recommendation| (recommendation.skill_id.as_str(), recommendation))
            .collect::<BTreeMap<_, _>>();

        for skill_id in &selected_skill_ids {
            if !recommendation_map.contains_key(skill_id.as_str()) {
                bail!("requested skill is not part of the current recommendation set: {skill_id}");
            }
        }

        let target_root = project_root.join(".agents").join("skills");
        std::fs::create_dir_all(&target_root)?;
        let registry_path = target_root.join("registry.json");
        let mut installed_state = read_installed_skill_states(&registry_path)?;

        let mut results = Vec::new();
        for recommendation in &response.recommendations {
            if !selected_skill_ids.contains(&recommendation.skill_id) {
                continue;
            }

            if installed_state
                .get(&recommendation.skill_id)
                .is_some_and(|state| state.installed)
            {
                results.push(SuggestInstallResult {
                    skill_id: recommendation.skill_id.clone(),
                    status: SuggestInstallStatus::AlreadyInstalled,
                    error_message: None,
                });
                continue;
            }

            match provider.resolve(&recommendation.skill_id) {
                Ok(resolved) => match install_fn(
                    &recommendation.skill_id,
                    &resolved.download_url,
                    &target_root,
                ) {
                    Ok(()) => {
                        installed_state.insert(
                            recommendation.skill_id.clone(),
                            InstalledSkillState {
                                installed: true,
                                version: None,
                            },
                        );
                        results.push(SuggestInstallResult {
                            skill_id: recommendation.skill_id.clone(),
                            status: SuggestInstallStatus::Installed,
                            error_message: None,
                        });
                    }
                    Err(error) => results.push(SuggestInstallResult {
                        skill_id: recommendation.skill_id.clone(),
                        status: SuggestInstallStatus::Failed,
                        error_message: Some(error.to_string()),
                    }),
                },
                Err(error) => results.push(SuggestInstallResult {
                    skill_id: recommendation.skill_id.clone(),
                    status: SuggestInstallStatus::Failed,
                    error_message: Some(error.to_string()),
                }),
            }
        }

        let mut suggest = response.to_json_response();
        for recommendation in &mut suggest.recommendations {
            if results.iter().any(|result| {
                result.skill_id == recommendation.skill_id
                    && matches!(
                        result.status,
                        SuggestInstallStatus::Installed | SuggestInstallStatus::AlreadyInstalled
                    )
            }) {
                recommendation.installed = true;
            }
        }
        suggest.summary.installable_count = suggest
            .recommendations
            .iter()
            .filter(|recommendation| !recommendation.installed)
            .count();

        Ok(SuggestInstallJsonResponse {
            suggest,
            mode,
            selected_skill_ids,
            results,
        })
    }
}

impl SuggestResponse {
    pub fn to_json_response(&self) -> SuggestJsonResponse {
        SuggestJsonResponse {
            detections: self
                .detections
                .iter()
                .map(|detection| SuggestJsonDetection {
                    technology: detection.technology,
                    confidence: detection.confidence,
                    evidence: detection
                        .evidence
                        .iter()
                        .map(|evidence| evidence.path.display().to_string())
                        .collect(),
                })
                .collect(),
            recommendations: self
                .recommendations
                .iter()
                .map(|recommendation| SuggestJsonRecommendation {
                    skill_id: recommendation.skill_id.clone(),
                    matched_technologies: recommendation.matched_technologies.clone(),
                    reasons: recommendation.reasons.clone(),
                    installed: recommendation.installed,
                })
                .collect(),
            summary: self.summary.clone(),
        }
    }

    pub fn render_human(&self) -> String {
        let mut lines = Vec::<String>::new();

        if self.detections.is_empty() {
            lines.push("Detected technologies: none".to_string());
        } else {
            lines.push("Detected technologies:".to_string());
            for detection in &self.detections {
                let evidence = detection
                    .evidence
                    .iter()
                    .map(|evidence| evidence.path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!(
                    "- {} ({}): {}",
                    detection.technology, detection.confidence, evidence
                ));
            }
        }

        if self.recommendations.is_empty() {
            lines.push("Recommended skills: none".to_string());
        } else {
            lines.push("Recommended skills:".to_string());
            for recommendation in &self.recommendations {
                let installed = if recommendation.installed {
                    match recommendation.installed_version.as_deref() {
                        Some(version) => format!("installed ({version})"),
                        None => "installed".to_string(),
                    }
                } else {
                    "not installed".to_string()
                };

                lines.push(format!(
                    "- {} — {} [{}]",
                    recommendation.skill_id, recommendation.title, installed
                ));
                lines.push(format!("  {}", recommendation.summary));
                for reason in &recommendation.reasons {
                    lines.push(format!("  reason: {}", reason));
                }
            }
        }

        lines.push(format!(
            "Summary: {} detected, {} recommended, {} installable",
            self.summary.detected_count,
            self.summary.recommended_count,
            self.summary.installable_count
        ));

        lines.join("\n")
    }

    pub fn installable_recommendations(&self) -> Vec<&SkillSuggestion> {
        self.recommendations
            .iter()
            .filter(|recommendation| !recommendation.installed)
            .collect()
    }
}

impl SuggestInstallJsonResponse {
    pub fn render_human(&self) -> String {
        let mut lines = Vec::new();

        if self.suggest.detections.is_empty() {
            lines.push("Detected technologies: none".to_string());
        } else {
            lines.push("Detected technologies:".to_string());
            for detection in &self.suggest.detections {
                lines.push(format!(
                    "- {} ({}): {}",
                    detection.technology.as_human_label(),
                    detection.confidence,
                    detection.evidence.join(", ")
                ));
            }
        }

        if self.suggest.recommendations.is_empty() {
            lines.push("Recommended skills: none".to_string());
        } else {
            lines.push("Recommended skills:".to_string());
            for recommendation in &self.suggest.recommendations {
                let installed = if recommendation.installed {
                    "installed"
                } else {
                    "not installed"
                };
                lines.push(format!("- {} [{}]", recommendation.skill_id, installed));
                for reason in &recommendation.reasons {
                    lines.push(format!("  reason: {}", reason));
                }
            }
        }

        lines.push(format!(
            "Summary: {} detected, {} recommended, {} installable",
            self.suggest.summary.detected_count,
            self.suggest.summary.recommended_count,
            self.suggest.summary.installable_count
        ));
        lines.push(format!("Install mode: {}", self.mode.as_human_label()));

        if self.selected_skill_ids.is_empty() {
            lines.push("Selected skills: none".to_string());
        } else {
            lines.push(format!(
                "Selected skills: {}",
                self.selected_skill_ids.join(", ")
            ));
        }

        if self.results.is_empty() {
            lines.push("Install results: none".to_string());
        } else {
            lines.push("Install results:".to_string());
            for result in &self.results {
                let mut line = format!("- {}: {}", result.skill_id, result.status.as_human_label());
                if let Some(error_message) = &result.error_message {
                    line.push_str(&format!(" ({error_message})"));
                }
                lines.push(line);
            }
        }

        lines.join("\n")
    }
}

pub fn annotate_recommendations(
    recommendations: &mut [SkillSuggestion],
    installed_skill_states: &BTreeMap<String, InstalledSkillState>,
) {
    for recommendation in recommendations {
        recommendation
            .annotate_installed_state(installed_skill_states.get(&recommendation.skill_id));
    }
}

impl SuggestInstallMode {
    fn as_human_label(self) -> &'static str {
        match self {
            Self::Interactive => "interactive",
            Self::InstallAll => "install-all",
        }
    }
}

impl SuggestInstallStatus {
    fn as_human_label(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::AlreadyInstalled => "already installed",
            Self::Failed => "failed",
        }
    }
}

fn dedupe_preserve_order(skill_ids: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();

    for skill_id in skill_ids {
        if seen.insert(skill_id.as_str()) {
            unique.push(skill_id.clone());
        }
    }

    unique
}

impl fmt::Display for DetectionConfidence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_human_label())
    }
}
