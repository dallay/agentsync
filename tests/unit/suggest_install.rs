use agentsync::skills::provider::{Provider, SkillInstallInfo};
use agentsync::skills::suggest::{
    DetectionConfidence, DetectionEvidence, SuggestInstallMode, SuggestInstallPhase,
    SuggestInstallProgressEvent, SuggestInstallProgressReporter, SuggestInstallStatus,
    SuggestionService, TechnologyDetection, TechnologyId,
};
use anyhow::Result;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[test]
fn guided_install_only_installs_selected_recommendations() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();

    let provider = LocalSkillProvider::new(
        root,
        &[
            (
                "dallay/agents-skills/rust-async-patterns",
                "rust-async-patterns",
            ),
            ("dallay/agents-skills/docker-expert", "docker-expert"),
        ],
    );
    let service = SuggestionService;
    let response = service
        .suggest_with(root, &StaticDetector::rust_and_docker(), Some(&provider))
        .unwrap();

    let install_response = service
        .install_selected_with(
            root,
            &response,
            &provider,
            SuggestInstallMode::Interactive,
            &["rust-async-patterns".to_string()],
            |skill_id, source, target_root| {
                agentsync::skills::install::blocking_fetch_and_install_skill(
                    skill_id,
                    source,
                    target_root,
                )
                .map_err(|error| anyhow::anyhow!(error))
            },
        )
        .unwrap();

    assert_eq!(
        install_response.selected_skill_ids,
        vec!["rust-async-patterns"]
    );
    assert_eq!(install_response.results.len(), 1);
    assert_eq!(install_response.results[0].skill_id, "rust-async-patterns");
    assert_eq!(
        install_response.results[0].status,
        SuggestInstallStatus::Installed
    );
    assert_eq!(install_response.results[0].error_message, None);
    assert!(root.join(".agents/skills/rust-async-patterns").exists());
    assert!(!root.join(".agents/skills/docker-expert").exists());
}

#[test]
fn install_all_skips_already_installed_recommendations() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();

    let skills_dir = root.join(".agents/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::create_dir_all(skills_dir.join("docker-expert")).unwrap();
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

    let provider = LocalSkillProvider::new(
        root,
        &[
            (
                "dallay/agents-skills/rust-async-patterns",
                "rust-async-patterns",
            ),
            ("dallay/agents-skills/docker-expert", "docker-expert"),
        ],
    );
    let service = SuggestionService;
    let response = service
        .suggest_with(root, &StaticDetector::rust_and_docker(), Some(&provider))
        .unwrap();

    let installable_ids = response
        .installable_recommendations()
        .into_iter()
        .map(|recommendation| recommendation.skill_id.clone())
        .collect::<Vec<_>>();
    assert_eq!(installable_ids, vec!["rust-async-patterns"]);

    let install_response = service
        .install_all_with(root, &response, &provider)
        .unwrap();

    let statuses = install_response
        .results
        .iter()
        .map(|result| (result.skill_id.as_str(), result.status))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        statuses.get("rust-async-patterns"),
        Some(&SuggestInstallStatus::Installed)
    );
    assert_eq!(
        statuses.get("docker-expert"),
        Some(&SuggestInstallStatus::AlreadyInstalled)
    );
    assert!(root.join(".agents/skills/rust-async-patterns").exists());

    let updated_response = service
        .suggest_with(root, &StaticDetector::rust_and_docker(), Some(&provider))
        .unwrap();
    let rust = updated_response
        .recommendations
        .iter()
        .find(|recommendation| recommendation.skill_id == "rust-async-patterns")
        .unwrap();
    assert!(rust.installed);
}

#[test]
fn install_flow_rechecks_registry_before_installing() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();

    let provider = LocalSkillProvider::new(
        root,
        &[(
            "dallay/agents-skills/rust-async-patterns",
            "rust-async-patterns",
        )],
    );
    let service = SuggestionService;
    let response = service
        .suggest_with(root, &StaticDetector::rust_only(), Some(&provider))
        .unwrap();

    let skills_dir = root.join(".agents/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::create_dir_all(skills_dir.join("rust-async-patterns")).unwrap();
    fs::write(
        skills_dir.join("registry.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "last_updated": "2026-03-31T00:00:00Z",
            "skills": {
                "rust-async-patterns": {
                    "name": "rust-async-patterns",
                    "version": "9.9.9"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let install_response = service
        .install_selected_with(
            root,
            &response,
            &provider,
            SuggestInstallMode::InstallAll,
            &["rust-async-patterns".to_string()],
            |_skill_id, _source, _target_root| {
                panic!("install should be skipped after registry recheck")
            },
        )
        .unwrap();

    assert_eq!(install_response.results.len(), 1);
    assert_eq!(install_response.results[0].skill_id, "rust-async-patterns");
    assert_eq!(
        install_response.results[0].status,
        SuggestInstallStatus::AlreadyInstalled
    );
    assert_eq!(install_response.results[0].error_message, None);
}

#[test]
fn suggest_marks_canonical_vue_skill_as_installed_when_legacy_alias_exists() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::write(
        root.join("package.json"),
        r#"{"dependencies":{"vue":"^3.4.0"}}"#,
    )
    .unwrap();

    let skills_dir = root.join(".agents/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::create_dir_all(skills_dir.join("antfu-vue")).unwrap();
    fs::write(
        skills_dir.join("registry.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "last_updated": "2026-04-06T00:00:00Z",
            "skills": {
                "antfu-vue": {
                    "name": "antfu-vue",
                    "version": "1.0.0"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let response = SuggestionService
        .suggest(root)
        .expect("vue recommendation should be generated");

    let vue = response
        .recommendations
        .iter()
        .find(|recommendation| recommendation.provider_skill_id == "antfu/skills/vue")
        .expect("antfu vue recommendation should exist");

    assert_eq!(vue.skill_id, "vue");
    assert!(vue.installed);
    assert_eq!(vue.installed_version.as_deref(), Some("1.0.0"));
}

#[test]
fn install_flow_skips_canonical_vue_when_legacy_alias_is_installed() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::write(
        root.join("package.json"),
        r#"{"dependencies":{"vue":"^3.4.0"}}"#,
    )
    .unwrap();

    let skills_dir = root.join(".agents/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::create_dir_all(skills_dir.join("antfu-vue")).unwrap();
    fs::write(
        skills_dir.join("registry.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "last_updated": "2026-04-06T00:00:00Z",
            "skills": {
                "antfu-vue": {
                    "name": "antfu-vue",
                    "version": "1.0.0"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let provider = LocalSkillProvider::new(root, &[("antfu/skills/vue", "vue")]);
    let response = SuggestionService
        .suggest(root)
        .expect("vue recommendation should be generated");

    let install_response = SuggestionService
        .install_selected_with(
            root,
            &response,
            &provider,
            SuggestInstallMode::InstallAll,
            &["vue".to_string()],
            |_skill_id, _source, _target_root| {
                panic!("canonical vue install should be skipped when legacy alias exists")
            },
        )
        .unwrap();

    assert_eq!(install_response.results.len(), 1);
    assert_eq!(install_response.results[0].skill_id, "vue");
    assert_eq!(
        install_response.results[0].status,
        SuggestInstallStatus::AlreadyInstalled
    );
}

#[test]
fn install_flow_records_failures_and_continues() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();
    fs::write(root.join("Makefile"), "all:\n\t@true\n").unwrap();

    let provider = PartiallyFailingProvider::new(root);
    let service = SuggestionService;
    let response = service
        .suggest_with(
            root,
            &StaticDetector::rust_docker_and_make(),
            Some(&provider),
        )
        .unwrap();

    let install_response = service
        .install_selected_with(
            root,
            &response,
            &provider,
            SuggestInstallMode::InstallAll,
            &[
                "rust-async-patterns".to_string(),
                "docker-expert".to_string(),
                "makefile".to_string(),
            ],
            |skill_id, source, target_root| {
                if skill_id == "docker-expert" {
                    anyhow::bail!("simulated install failure for {skill_id}");
                }

                agentsync::skills::install::blocking_fetch_and_install_skill(
                    skill_id,
                    source,
                    target_root,
                )
                .map_err(|error| anyhow::anyhow!(error))
            },
        )
        .unwrap();

    let statuses = install_response
        .results
        .iter()
        .map(|result| {
            (
                result.skill_id.as_str(),
                (result.status, result.error_message.as_deref()),
            )
        })
        .collect::<BTreeMap<_, _>>();

    assert_eq!(
        statuses.get("rust-async-patterns"),
        Some(&(
            SuggestInstallStatus::Failed,
            Some("simulated resolve failure for rust-async-patterns")
        ))
    );
    assert_eq!(
        statuses.get("docker-expert"),
        Some(&(
            SuggestInstallStatus::Failed,
            Some("simulated install failure for docker-expert")
        ))
    );
    assert_eq!(
        statuses.get("makefile"),
        Some(&(SuggestInstallStatus::Installed, None))
    );
    assert!(!root.join(".agents/skills/rust-async-patterns").exists());
    assert!(!root.join(".agents/skills/docker-expert").exists());
    assert!(root.join(".agents/skills/makefile").exists());
}

#[test]
fn install_flow_emits_progress_events_in_success_order() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();

    let provider = LocalSkillProvider::new(
        root,
        &[(
            "dallay/agents-skills/rust-async-patterns",
            "rust-async-patterns",
        )],
    );
    let service = SuggestionService;
    let response = service
        .suggest_with(root, &StaticDetector::rust_only(), Some(&provider))
        .unwrap();
    let mut reporter = RecordingReporter::default();

    let install_response = service
        .install_selected_with_reporter(
            root,
            &response,
            &provider,
            SuggestInstallMode::Interactive,
            &["rust-async-patterns".to_string()],
            &mut reporter,
            |skill_id, source, target_root| {
                agentsync::skills::install::blocking_fetch_and_install_skill(
                    skill_id,
                    source,
                    target_root,
                )
                .map_err(|error| anyhow::anyhow!(error))
            },
        )
        .unwrap();

    assert_eq!(install_response.results.len(), 1);
    assert_eq!(
        reporter.events,
        vec![
            SuggestInstallProgressEvent::Resolving {
                skill_id: "rust-async-patterns".to_string(),
            },
            SuggestInstallProgressEvent::Installing {
                skill_id: "rust-async-patterns".to_string(),
            },
            SuggestInstallProgressEvent::Installed {
                skill_id: "rust-async-patterns".to_string(),
            },
        ]
    );
}

#[test]
fn install_flow_emits_skip_event_after_registry_recheck() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();

    let provider = LocalSkillProvider::new(
        root,
        &[(
            "dallay/agents-skills/rust-async-patterns",
            "rust-async-patterns",
        )],
    );
    let service = SuggestionService;
    let response = service
        .suggest_with(root, &StaticDetector::rust_only(), Some(&provider))
        .unwrap();

    let skills_dir = root.join(".agents/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(
        skills_dir.join("registry.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "last_updated": "2026-03-31T00:00:00Z",
            "skills": {
                "rust-async-patterns": {
                    "name": "rust-async-patterns",
                    "version": "9.9.9"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let mut reporter = RecordingReporter::default();
    service
        .install_selected_with_reporter(
            root,
            &response,
            &provider,
            SuggestInstallMode::InstallAll,
            &["rust-async-patterns".to_string()],
            &mut reporter,
            |_skill_id, _source, _target_root| {
                panic!("install should be skipped after registry recheck")
            },
        )
        .unwrap();

    assert_eq!(
        reporter.events,
        vec![SuggestInstallProgressEvent::SkippedAlreadyInstalled {
            skill_id: "rust-async-patterns".to_string(),
        }]
    );
}

#[test]
fn install_flow_emits_resolve_failure_event() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();

    let provider = PartiallyFailingProvider::new(root);
    let service = SuggestionService;
    let response = service
        .suggest_with(root, &StaticDetector::rust_only(), Some(&provider))
        .unwrap();
    let mut reporter = RecordingReporter::default();

    service
        .install_selected_with_reporter(
            root,
            &response,
            &provider,
            SuggestInstallMode::InstallAll,
            &["rust-async-patterns".to_string()],
            &mut reporter,
            |_skill_id, _source, _target_root| Ok(()),
        )
        .unwrap();

    assert_eq!(
        reporter.events,
        vec![
            SuggestInstallProgressEvent::Resolving {
                skill_id: "rust-async-patterns".to_string(),
            },
            SuggestInstallProgressEvent::Failed {
                skill_id: "rust-async-patterns".to_string(),
                phase: SuggestInstallPhase::Resolve,
                message: "simulated resolve failure for rust-async-patterns".to_string(),
            },
        ]
    );
}

#[test]
fn install_flow_emits_install_failure_event() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
    fs::write(root.join("Dockerfile"), "FROM scratch\n").unwrap();

    let provider = LocalSkillProvider::new(
        root,
        &[("dallay/agents-skills/docker-expert", "docker-expert")],
    );
    let service = SuggestionService;
    let response = service
        .suggest_with(root, &StaticDetector::rust_and_docker(), Some(&provider))
        .unwrap();
    let mut reporter = RecordingReporter::default();

    service
        .install_selected_with_reporter(
            root,
            &response,
            &provider,
            SuggestInstallMode::InstallAll,
            &["docker-expert".to_string()],
            &mut reporter,
            |_skill_id, _source, _target_root| {
                anyhow::bail!("simulated install failure for docker-expert")
            },
        )
        .unwrap();

    assert_eq!(
        reporter.events,
        vec![
            SuggestInstallProgressEvent::Resolving {
                skill_id: "docker-expert".to_string(),
            },
            SuggestInstallProgressEvent::Installing {
                skill_id: "docker-expert".to_string(),
            },
            SuggestInstallProgressEvent::Failed {
                skill_id: "docker-expert".to_string(),
                phase: SuggestInstallPhase::Install,
                message: "simulated install failure for docker-expert".to_string(),
            },
        ]
    );
}

struct StaticDetector {
    detections: Vec<TechnologyDetection>,
}

impl StaticDetector {
    fn rust_and_docker() -> Self {
        Self {
            detections: vec![
                detection(
                    TechnologyId::new(TechnologyId::RUST),
                    DetectionConfidence::High,
                    "Cargo.toml",
                ),
                detection(
                    TechnologyId::new(TechnologyId::DOCKER),
                    DetectionConfidence::High,
                    "Dockerfile",
                ),
            ],
        }
    }

    fn rust_only() -> Self {
        Self {
            detections: vec![detection(
                TechnologyId::new(TechnologyId::RUST),
                DetectionConfidence::High,
                "Cargo.toml",
            )],
        }
    }

    fn rust_docker_and_make() -> Self {
        Self {
            detections: vec![
                detection(
                    TechnologyId::new(TechnologyId::RUST),
                    DetectionConfidence::High,
                    "Cargo.toml",
                ),
                detection(
                    TechnologyId::new(TechnologyId::DOCKER),
                    DetectionConfidence::High,
                    "Dockerfile",
                ),
                detection(
                    TechnologyId::new(TechnologyId::MAKE),
                    DetectionConfidence::High,
                    "Makefile",
                ),
            ],
        }
    }
}

impl agentsync::skills::detect::RepoDetector for StaticDetector {
    fn detect(&self, _project_root: &Path) -> Result<Vec<TechnologyDetection>> {
        Ok(self.detections.clone())
    }
}

#[derive(Default)]
struct RecordingReporter {
    events: Vec<SuggestInstallProgressEvent>,
}

impl SuggestInstallProgressReporter for RecordingReporter {
    fn on_event(&mut self, event: SuggestInstallProgressEvent) {
        self.events.push(event);
    }
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

struct LocalSkillProvider {
    sources: BTreeMap<String, PathBuf>,
}

impl LocalSkillProvider {
    fn new(root: &Path, skills: &[(&str, &str)]) -> Self {
        let sources_root = root.join("skill-sources");
        fs::create_dir_all(&sources_root).unwrap();

        let mut sources = BTreeMap::new();
        for (provider_skill_id, local_skill_id) in skills {
            let source_dir = sources_root.join(local_skill_id);
            fs::create_dir_all(&source_dir).unwrap();
            fs::write(
                source_dir.join("SKILL.md"),
                format!("---\nname: {local_skill_id}\nversion: 1.0.0\n---\n# {local_skill_id}\n"),
            )
            .unwrap();
            sources.insert((*provider_skill_id).to_string(), source_dir);
        }

        Self { sources }
    }
}

impl Provider for LocalSkillProvider {
    fn manifest(&self) -> Result<String> {
        Ok("local-test-provider".to_string())
    }

    fn resolve(&self, id: &str) -> Result<SkillInstallInfo> {
        let source = self
            .sources
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("missing local source for {id}"))?;
        Ok(SkillInstallInfo {
            download_url: source.display().to_string(),
            format: "dir".to_string(),
        })
    }
}

struct PartiallyFailingProvider {
    inner: LocalSkillProvider,
}

impl PartiallyFailingProvider {
    fn new(root: &Path) -> Self {
        Self {
            inner: LocalSkillProvider::new(
                root,
                &[
                    ("dallay/agents-skills/docker-expert", "docker-expert"),
                    ("dallay/agents-skills/makefile", "makefile"),
                ],
            ),
        }
    }
}

impl Provider for PartiallyFailingProvider {
    fn manifest(&self) -> Result<String> {
        self.inner.manifest()
    }

    fn resolve(&self, id: &str) -> Result<SkillInstallInfo> {
        if id == "dallay/agents-skills/rust-async-patterns" {
            anyhow::bail!("simulated resolve failure for rust-async-patterns");
        }

        self.inner.resolve(id)
    }
}
