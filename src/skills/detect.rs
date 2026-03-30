use crate::skills::suggest::{
    DetectionConfidence, DetectionEvidence, TechnologyDetection, TechnologyId,
};
use anyhow::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

const IGNORED_DIRS: &[&str] = &[
    ".git",
    ".agents",
    "node_modules",
    "target",
    "dist",
    "build",
    ".astro",
    ".next",
    ".turbo",
    ".pnpm-store",
];

const INCIDENTAL_DIRS: &[&str] = &[
    "test",
    "tests",
    "spec",
    "specs",
    "fixture",
    "fixtures",
    "example",
    "examples",
    "sample",
    "samples",
    "e2e",
    "__tests__",
];

pub trait RepoDetector {
    fn detect(&self, project_root: &Path) -> Result<Vec<TechnologyDetection>>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FileSystemRepoDetector;

impl RepoDetector for FileSystemRepoDetector {
    fn detect(&self, project_root: &Path) -> Result<Vec<TechnologyDetection>> {
        let mut detections = BTreeMap::<TechnologyId, DetectionAccumulator>::new();

        for entry in WalkDir::new(project_root)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|entry| !should_ignore_entry(project_root, entry))
        {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            let relative_path = match path.strip_prefix(project_root) {
                Ok(relative_path) => relative_path.to_path_buf(),
                Err(_) => continue,
            };

            if let Some((technology, marker, notes)) = match_marker(&relative_path) {
                let confidence = detection_confidence(&relative_path);
                detections
                    .entry(technology)
                    .or_insert_with(|| DetectionAccumulator::new(technology))
                    .record(relative_path, marker, notes, confidence);
            }
        }

        Ok(detections
            .into_values()
            .map(DetectionAccumulator::finish)
            .collect())
    }
}

fn should_ignore_entry(project_root: &Path, entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }

    if entry.depth() == 0 {
        return false;
    }

    entry
        .path()
        .strip_prefix(project_root)
        .ok()
        .and_then(|relative_path| relative_path.file_name())
        .and_then(|name| name.to_str())
        .is_some_and(|name| IGNORED_DIRS.contains(&name))
}

fn match_marker(relative_path: &Path) -> Option<(TechnologyId, String, Option<String>)> {
    let file_name = relative_path.file_name()?.to_str()?;
    let notes = detection_notes(relative_path);

    if file_name == "Cargo.toml" {
        return Some((TechnologyId::Rust, file_name.to_string(), notes));
    }

    if file_name == "package.json" || file_name == "tsconfig.json" {
        return Some((TechnologyId::NodeTypeScript, file_name.to_string(), notes));
    }

    if file_name.starts_with("astro.config.") {
        return Some((TechnologyId::Astro, file_name.to_string(), notes));
    }

    if is_github_actions_workflow(relative_path) {
        return Some((
            TechnologyId::GitHubActions,
            ".github/workflows".to_string(),
            None,
        ));
    }

    if is_docker_marker(file_name) {
        return Some((TechnologyId::Docker, file_name.to_string(), notes));
    }

    if file_name == "Makefile" || file_name == "GNUmakefile" {
        return Some((TechnologyId::Make, file_name.to_string(), notes));
    }

    if is_python_marker(file_name) {
        return Some((TechnologyId::Python, file_name.to_string(), notes));
    }

    None
}

fn detection_confidence(relative_path: &Path) -> DetectionConfidence {
    if is_github_actions_workflow(relative_path) {
        return DetectionConfidence::High;
    }

    let depth = relative_path.components().count();
    if depth == 1 {
        return DetectionConfidence::High;
    }

    if is_incidental_path(relative_path) {
        return DetectionConfidence::Low;
    }

    DetectionConfidence::Medium
}

fn detection_notes(relative_path: &Path) -> Option<String> {
    match detection_confidence(relative_path) {
        DetectionConfidence::Medium => Some("nested first-party marker".to_string()),
        DetectionConfidence::Low => Some("incidental marker in test/example path".to_string()),
        DetectionConfidence::High => None,
    }
}

fn is_incidental_path(relative_path: &Path) -> bool {
    relative_path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .any(|component| INCIDENTAL_DIRS.contains(&component))
}

fn is_github_actions_workflow(relative_path: &Path) -> bool {
    let mut components = relative_path.components();
    matches!(
        (
            components.next().and_then(component_to_str),
            components.next().and_then(component_to_str),
            components.next().and_then(component_to_str),
            components.next(),
        ),
        (Some(".github"), Some("workflows"), Some(file_name), None)
            if file_name.ends_with(".yml") || file_name.ends_with(".yaml")
    )
}

fn is_docker_marker(file_name: &str) -> bool {
    file_name.starts_with("Dockerfile")
        || matches!(file_name, "compose.yml" | "compose.yaml")
        || (file_name.starts_with("docker-compose")
            && (file_name.ends_with(".yml") || file_name.ends_with(".yaml")))
}

fn is_python_marker(file_name: &str) -> bool {
    file_name == "pyproject.toml"
        || file_name == "uv.lock"
        || file_name == "poetry.lock"
        || (file_name.starts_with("requirements") && file_name.ends_with(".txt"))
}

fn component_to_str(component: std::path::Component<'_>) -> Option<&str> {
    component.as_os_str().to_str()
}

#[derive(Debug)]
struct DetectionAccumulator {
    technology: TechnologyId,
    confidence: DetectionConfidence,
    root_relative_paths: Vec<PathBuf>,
    evidence: Vec<DetectionEvidence>,
}

impl DetectionAccumulator {
    fn new(technology: TechnologyId) -> Self {
        Self {
            technology,
            confidence: DetectionConfidence::Low,
            root_relative_paths: Vec::new(),
            evidence: Vec::new(),
        }
    }

    fn record(
        &mut self,
        relative_path: PathBuf,
        marker: String,
        notes: Option<String>,
        confidence: DetectionConfidence,
    ) {
        if confidence > self.confidence {
            self.confidence = confidence;
        }

        if !self.root_relative_paths.contains(&relative_path) {
            self.root_relative_paths.push(relative_path.clone());
            self.root_relative_paths.sort();
        }

        self.evidence.push(DetectionEvidence {
            marker,
            path: relative_path,
            notes,
        });
        self.evidence
            .sort_by(|left, right| left.path.cmp(&right.path));
    }

    fn finish(self) -> TechnologyDetection {
        TechnologyDetection {
            technology: self.technology,
            confidence: self.confidence,
            root_relative_paths: self.root_relative_paths,
            evidence: self.evidence,
        }
    }
}
