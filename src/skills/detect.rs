use crate::skills::catalog::ResolvedSkillCatalog;
use crate::skills::suggest::{
    DetectionConfidence, DetectionEvidence, TechnologyDetection, TechnologyId,
};
use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;
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

/// Detection rules parsed from the catalog's `[technologies.detect]` block.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct DetectionRules {
    /// Exact package names to look for in package.json dependencies/devDependencies
    #[serde(default)]
    pub packages: Option<Vec<String>>,
    /// Regex patterns to match against package names (e.g., "^@azure/")
    #[serde(default)]
    pub package_patterns: Option<Vec<String>>,
    /// Config files whose existence indicates the technology
    #[serde(default)]
    pub config_files: Option<Vec<String>>,
    /// Rules for scanning file content
    #[serde(default)]
    pub config_file_content: Option<ConfigFileContentRules>,
    /// File extensions to scan for (e.g., [".html", ".css", ".tsx"] for web frontend detection)
    #[serde(default)]
    pub file_extensions: Option<Vec<String>>,
}

/// Metadata about the repository collected in a single pass to optimize detection.
struct RepoMetadata {
    /// All relative paths found during a single-pass walk (max depth 3)
    paths: BTreeSet<PathBuf>,
    /// Set of relative paths that are directories
    dirs: BTreeSet<PathBuf>,
    /// Map of file extension (e.g., ".rs") to the first relative path found with it.
    /// Used to quickly evaluate file_extensions rules.
    extensions: BTreeMap<String, PathBuf>,
}

impl RepoMetadata {
    fn collect(project_root: &Path) -> Self {
        let mut paths = BTreeSet::new();
        let mut dirs = BTreeSet::new();
        let mut extensions = BTreeMap::new();

        for entry in WalkDir::new(project_root)
            .max_depth(3)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|entry| !should_ignore_entry(project_root, entry))
            .flatten()
        {
            let Ok(relative) = entry.path().strip_prefix(project_root) else {
                continue;
            };

            if relative.as_os_str().is_empty() {
                continue;
            }

            let relative_buf = relative.to_path_buf();
            paths.insert(relative_buf.clone());

            if entry.file_type().is_dir() {
                dirs.insert(relative_buf.clone());
            }

            if entry.file_type().is_file()
                && let Some(ext) = relative.extension().and_then(|e| e.to_str())
            {
                let dot_ext = format!(".{ext}");
                // Store first occurrence for deterministic evidence
                extensions
                    .entry(dot_ext)
                    .or_insert_with(|| relative_buf.clone());
                extensions.entry(ext.to_string()).or_insert(relative_buf);
            }
        }

        Self {
            paths,
            dirs,
            extensions,
        }
    }
}

/// Rules for detecting technologies by scanning file content.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ConfigFileContentRules {
    /// Specific files to read (relative to project root)
    #[serde(default)]
    pub files: Option<Vec<String>>,
    /// String patterns to search for within file content
    pub patterns: Vec<String>,
    /// Whether to scan Gradle build files (build.gradle.kts, settings.gradle, etc.)
    #[serde(default)]
    pub scan_gradle_layout: Option<bool>,
}

pub trait RepoDetector {
    fn detect(&self, project_root: &Path) -> Result<Vec<TechnologyDetection>>;
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

// ---------------------------------------------------------------------------
// CatalogDrivenDetector — evaluates data-driven detection rules from catalog
// ---------------------------------------------------------------------------

struct CompiledDetectionRules {
    packages: Option<Vec<String>>,
    package_patterns: Option<Vec<Regex>>,
    config_files: Option<Vec<String>>,
    config_file_content: Option<CompiledConfigFileContentRules>,
    file_extensions: Option<Vec<String>>,
}

struct CompiledConfigFileContentRules {
    files: Option<Vec<String>>,
    patterns: Vec<Regex>,
    scan_gradle_layout: bool,
}

/// Detector that evaluates data-driven detection rules from the catalog.
pub struct CatalogDrivenDetector {
    rules: Vec<(TechnologyId, CompiledDetectionRules)>,
}

impl CatalogDrivenDetector {
    /// Build a detector by compiling all detection rules from the catalog.
    /// Technologies with invalid regex patterns are skipped with a warning.
    pub fn new(catalog: &ResolvedSkillCatalog) -> Result<Self> {
        let mut rules = Vec::new();

        for (tech_id, entry) in catalog.technologies() {
            let Some(detect) = &entry.detect else {
                continue;
            };

            match Self::compile_rules(detect, tech_id) {
                Ok(compiled) => rules.push((tech_id.clone(), compiled)),
                Err(error) => {
                    warn!(
                        technology = %tech_id,
                        error = %error,
                        "Skipping technology with invalid detection rules"
                    );
                }
            }
        }

        Ok(Self { rules })
    }

    fn compile_rules(
        rules: &DetectionRules,
        tech_id: &TechnologyId,
    ) -> Result<CompiledDetectionRules> {
        let package_patterns = rules
            .package_patterns
            .as_ref()
            .map(|patterns| {
                patterns
                    .iter()
                    .map(|pattern| {
                        Regex::new(pattern).with_context(|| {
                            format!(
                                "invalid package_pattern regex '{pattern}' for technology {tech_id}"
                            )
                        })
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;

        let config_file_content = rules
            .config_file_content
            .as_ref()
            .map(|content_rules| {
                let patterns = content_rules
                    .patterns
                    .iter()
                    .map(|pattern| {
                        Regex::new(pattern).with_context(|| {
                            format!(
                                "invalid config_file_content pattern '{pattern}' \
                                 for technology {tech_id}"
                            )
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;

                Ok::<_, anyhow::Error>(CompiledConfigFileContentRules {
                    files: content_rules.files.clone(),
                    patterns,
                    scan_gradle_layout: content_rules.scan_gradle_layout.unwrap_or(false),
                })
            })
            .transpose()?;

        Ok(CompiledDetectionRules {
            packages: rules.packages.clone(),
            package_patterns,
            config_files: rules.config_files.clone(),
            config_file_content,
            file_extensions: rules.file_extensions.clone(),
        })
    }
}

impl RepoDetector for CatalogDrivenDetector {
    fn detect(&self, project_root: &Path) -> Result<Vec<TechnologyDetection>> {
        if self.rules.is_empty() {
            return Ok(Vec::new());
        }

        // Phase 1: Collect metadata and package names
        let metadata = RepoMetadata::collect(project_root);
        let all_packages = collect_package_names(project_root);

        let mut detections = Vec::new();

        // Phase 2: Evaluate each technology's rules
        for (tech_id, compiled) in &self.rules {
            if let Some(detection) =
                evaluate_rules(project_root, tech_id, compiled, &all_packages, &metadata)
            {
                detections.push(detection);
            }
        }

        Ok(detections)
    }
}

fn evaluate_rules(
    project_root: &Path,
    tech_id: &TechnologyId,
    rules: &CompiledDetectionRules,
    all_packages: &BTreeSet<String>,
    metadata: &RepoMetadata,
) -> Option<TechnologyDetection> {
    // Check packages (exact match)
    if let Some(packages) = &rules.packages {
        for package in packages {
            if all_packages.contains(package) {
                return Some(make_detection(
                    tech_id,
                    DetectionConfidence::High,
                    package,
                    &format!("package '{package}' found in dependencies"),
                ));
            }
        }
    }

    // Check package_patterns (regex match)
    if let Some(patterns) = &rules.package_patterns {
        for regex in patterns {
            for package in all_packages {
                if regex.is_match(package) {
                    return Some(make_detection(
                        tech_id,
                        DetectionConfidence::Medium,
                        package,
                        &format!("package '{package}' matches pattern '{regex}'"),
                    ));
                }
            }
        }
    }

    // Check config_files (existence)
    if let Some(config_files) = &rules.config_files {
        for file in config_files {
            let path = PathBuf::from(file);
            // Check cache first (hot path for shallow markers), fallback to fs for deeply nested ones
            if metadata.paths.contains(&path) || project_root.join(&path).exists() {
                return Some(make_detection(
                    tech_id,
                    DetectionConfidence::High,
                    file,
                    &format!("config file '{file}' exists"),
                ));
            }
        }
    }

    // Check config_file_content (read files, search patterns)
    if let Some(content_rules) = &rules.config_file_content {
        let files_to_scan = gather_content_scan_files(project_root, content_rules, metadata);
        for file_path in &files_to_scan {
            let absolute = project_root.join(file_path);
            if let Ok(content) = fs::read_to_string(&absolute) {
                for pattern in &content_rules.patterns {
                    if pattern.is_match(&content) {
                        let display = file_path.display().to_string();
                        return Some(make_detection(
                            tech_id,
                            DetectionConfidence::Medium,
                            &display,
                            &format!("pattern '{}' found in '{}'", pattern, display),
                        ));
                    }
                }
            }
        }
    }

    // Check file_extensions (lookup in metadata)
    if let Some(extensions) = &rules.file_extensions {
        for ext in extensions {
            if let Some(path) = metadata.extensions.get(ext) {
                let display = path.display().to_string();
                return Some(make_detection(
                    tech_id,
                    DetectionConfidence::Medium,
                    &display,
                    &format!("file with extension '{ext}' found"),
                ));
            }
        }
    }

    None
}

fn make_detection(
    tech_id: &TechnologyId,
    confidence: DetectionConfidence,
    marker: &str,
    notes: &str,
) -> TechnologyDetection {
    let path = PathBuf::from(marker);
    TechnologyDetection {
        technology: tech_id.clone(),
        confidence,
        root_relative_paths: vec![path.clone()],
        evidence: vec![DetectionEvidence {
            marker: marker.to_string(),
            path,
            notes: Some(notes.to_string()),
        }],
    }
}

fn gather_content_scan_files(
    project_root: &Path,
    rules: &CompiledConfigFileContentRules,
    metadata: &RepoMetadata,
) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if rules.scan_gradle_layout {
        // Root-level Gradle files
        for name in &[
            "build.gradle.kts",
            "build.gradle",
            "settings.gradle.kts",
            "settings.gradle",
            "gradle/libs.versions.toml",
        ] {
            let path = PathBuf::from(name);
            if metadata.paths.contains(&path) {
                files.push(path);
            }
        }

        // Immediate subdirectory build files
        // Find directories in metadata.paths with depth 1
        let root_dirs: BTreeSet<PathBuf> = metadata
            .dirs
            .iter()
            .filter(|p| {
                p.parent() == Some(Path::new(""))
                    && !IGNORED_DIRS.contains(&p.to_str().unwrap_or(""))
            })
            .cloned()
            .collect();

        for dir in root_dirs {
            for build_file in &["build.gradle.kts", "build.gradle"] {
                let path = dir.join(build_file);
                if metadata.paths.contains(&path) {
                    files.push(path);
                }
            }
        }
    }

    if let Some(explicit_files) = &rules.files {
        for file in explicit_files {
            let path = PathBuf::from(file);
            if (metadata.paths.contains(&path) || project_root.join(&path).exists())
                && !files.contains(&path)
            {
                files.push(path);
            }
        }
    }

    files
}

// ---------------------------------------------------------------------------
// Package.json parsing and workspace resolution
// ---------------------------------------------------------------------------

fn collect_package_names(project_root: &Path) -> BTreeSet<String> {
    let mut all_packages = BTreeSet::new();

    // Parse root package.json
    if let Some(deps) = parse_package_json_deps(&project_root.join("package.json")) {
        all_packages.extend(deps);
    }

    // Resolve workspaces and parse each workspace's package.json
    let workspace_dirs = resolve_workspaces(project_root);
    for workspace_dir in workspace_dirs {
        let pkg_path = workspace_dir.join("package.json");
        if let Some(deps) = parse_package_json_deps(&pkg_path) {
            all_packages.extend(deps);
        }
    }

    all_packages
}

fn parse_package_json_deps(path: &Path) -> Option<BTreeSet<String>> {
    let content = fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let obj = json.as_object()?;

    let mut deps = BTreeSet::new();
    for key in &["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(section) = obj.get(*key).and_then(|v| v.as_object()) {
            for dep_name in section.keys() {
                deps.insert(dep_name.clone());
            }
        }
    }

    Some(deps)
}

fn resolve_workspaces(project_root: &Path) -> Vec<PathBuf> {
    // Try pnpm-workspace.yaml first
    let pnpm_path = project_root.join("pnpm-workspace.yaml");
    if let Ok(content) = fs::read_to_string(&pnpm_path) {
        let patterns = parse_pnpm_workspace_yaml(&content);
        if !patterns.is_empty() {
            return expand_workspace_patterns(project_root, &patterns);
        }
    }

    // Try package.json workspaces field
    let pkg_path = project_root.join("package.json");
    if let Ok(content) = fs::read_to_string(&pkg_path)
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(workspaces) = json.get("workspaces")
    {
        let patterns = parse_package_json_workspaces(workspaces);
        if !patterns.is_empty() {
            return expand_workspace_patterns(project_root, &patterns);
        }
    }

    Vec::new()
}

fn parse_pnpm_workspace_yaml(content: &str) -> Vec<String> {
    // Use serde_yaml to parse properly
    #[derive(Deserialize)]
    struct PnpmWorkspace {
        #[serde(default)]
        packages: Vec<String>,
    }

    serde_yaml::from_str::<PnpmWorkspace>(content)
        .map(|ws| ws.packages)
        .unwrap_or_default()
}

fn parse_package_json_workspaces(workspaces: &serde_json::Value) -> Vec<String> {
    // Array form: "workspaces": ["packages/*", "apps/*"]
    if let Some(arr) = workspaces.as_array() {
        return arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
    }

    // Object form (Yarn): "workspaces": { "packages": ["packages/*"] }
    if let Some(obj) = workspaces.as_object()
        && let Some(packages) = obj.get("packages").and_then(|v| v.as_array())
    {
        return packages
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
    }

    Vec::new()
}

fn expand_workspace_patterns(project_root: &Path, patterns: &[String]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    for pattern in patterns {
        // Strip trailing /* or /** for simple glob expansion
        let base = pattern
            .trim_end_matches("/**")
            .trim_end_matches("/*")
            .trim_end_matches('/');

        let base_path = project_root.join(base);

        if pattern.contains('*') {
            // Glob: list directories under the base path
            if let Ok(entries) = fs::read_dir(&base_path) {
                for entry in entries.flatten() {
                    if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                        let dir = entry.path();
                        if dir.join("package.json").exists() {
                            dirs.push(dir);
                        }
                    }
                }
            }
        } else {
            // Exact path: check if it has a package.json
            if base_path.join("package.json").exists() {
                dirs.push(base_path);
            }
        }
    }

    dirs
}
