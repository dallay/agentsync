use crate::skills::catalog::ResolvedSkillCatalog;
use crate::skills::suggest::{
    DetectionConfidence, DetectionEvidence, TechnologyDetection, TechnologyId,
};
use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
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

/// Known project manifest files for nested project discovery (issue #409)
const PROJECT_MANIFEST_FILES: &[&str] = &[
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "Cargo.toml",
    "go.mod",
    "package.json",
    "pyproject.toml",
    "Pipfile",
    "requirements.txt",
    "setup.py",
    "composer.json",
    "Gemfile",
    "mix.exs",
    "pubspec.yaml",
    "Package.swift",
    "Dockerfile",
];

/// Maximum depth for nested project discovery (issue #409).
/// Limit to 4 levels to avoid scanning too deep and hitting test/fixture directories.
const MAX_DISCOVER_DEPTH: usize = 4;

/// Directory names that indicate test/fixture content, not standalone projects
const TEST_DIR_NAMES: &[&str] = &["tests", "test", "__tests__", "fixtures", "examples"];

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

pub type ContentCache = HashMap<PathBuf, Option<Rc<str>>>;

pub(crate) fn get_file_content(path: &Path, cache: &mut ContentCache) -> Option<Rc<str>> {
    if let Some(cached) = cache.get(path) {
        return cached.as_ref().map(Rc::clone);
    }

    let result = fs::read_to_string(path).ok().map(|content| {
        let rc_content: Rc<str> = Rc::from(content);
        Rc::clone(&rc_content)
    });

    cache.insert(path.to_path_buf(), result.clone());
    result
}

/// Metadata about the repository collected in a single pass to optimize detection.
struct RepoMetadata {
    /// All relative paths found during a single-pass walk (max depth MAX_DISCOVER_DEPTH).
    /// Uses HashSet for O(1) existence checks during rule evaluation.
    paths: HashSet<PathBuf>,
    /// Set of relative paths that are directories.
    dirs: HashSet<PathBuf>,
    /// Immediate subdirectories of the project root (depth 1), cached for fast Gradle scanning.
    root_dirs: Vec<PathBuf>,
    /// Map of file extension (e.g., ".rs") to the first relative path found with it.
    /// Used to quickly evaluate file_extensions rules.
    extensions: HashMap<String, PathBuf>,
    /// Relative paths to subdirectories that appear to be standalone projects (issue #409).
    nested_projects: Vec<PathBuf>,
}

impl RepoMetadata {
    /// Collects filesystem metadata and nested project directories beneath the project root.
    ///
    /// The traversal is bounded by the configured discovery depth and excludes ignored directories.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let metadata = RepoMetadata::collect(Path::new("."));
    /// assert!(metadata.paths.iter().all(|path| !path.is_absolute()));
    /// ```
    fn collect(project_root: &Path) -> Self {
        let mut paths = HashSet::new();
        let mut dirs = HashSet::new();
        let mut root_dirs = Vec::new();
        let mut extensions = HashMap::new();
        let mut nested_projects = BTreeSet::new();

        for entry in WalkDir::new(project_root)
            .max_depth(MAX_DISCOVER_DEPTH)
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

            Self::process_dir_entry(&entry, &relative_buf, &mut root_dirs, &mut dirs);

            if entry.file_type().is_file() {
                Self::check_nested_project(relative, &mut nested_projects);
                Self::record_extension(relative, &relative_buf, &mut extensions);
            }

            paths.insert(relative_buf);
        }

        Self {
            paths,
            dirs,
            root_dirs,
            extensions,
            nested_projects: nested_projects.into_iter().collect(),
        }
    }

    /// Records a directory in the directory set and records depth-one directories as root directories.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use std::collections::HashSet;
    /// use std::path::PathBuf;
    /// use walkdir::WalkDir;
    ///
    /// let mut root_dirs = Vec::new();
    /// let mut dirs = HashSet::new();
    ///
    /// for entry in WalkDir::new(".") {
    ///     let entry = entry.unwrap();
    ///     let relative_path = PathBuf::from(entry.path());
    ///     process_dir_entry(&entry, &relative_path, &mut root_dirs, &mut dirs);
    /// }
    /// ```
    fn process_dir_entry(
        entry: &walkdir::DirEntry,
        relative_buf: &Path,
        root_dirs: &mut Vec<PathBuf>,
        dirs: &mut HashSet<PathBuf>,
    ) {
        if entry.file_type().is_dir() {
            if entry.depth() == 1 {
                root_dirs.push(relative_buf.to_path_buf());
            }
            dirs.insert(relative_buf.to_path_buf());
        }
    }

    /// Records the parent directory of a nested project manifest unless it is at the repository root or in a test directory.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut nested_projects = BTreeSet::new();
    ///
    /// check_nested_project(Path::new("frontend/package.json"), &mut nested_projects);
    ///
    /// assert!(nested_projects.contains(Path::new("frontend")));
    /// ```
    fn check_nested_project(relative: &Path, nested_projects: &mut BTreeSet<PathBuf>) {
        let file_name = relative.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !PROJECT_MANIFEST_FILES.contains(&file_name) {
            return;
        }
        let Some(dir) = relative.parent() else { return };
        if dir.as_os_str().is_empty() {
            return;
        }
        let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !TEST_DIR_NAMES.contains(&dir_name) {
            nested_projects.insert(dir.to_path_buf());
        }
    }

    /// Records the first path associated with a file extension.
    ///
    /// The extension is stored with and without its leading dot. Existing entries
    /// are preserved.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::path::Path;
    ///
    /// let mut extensions = HashMap::new();
    /// record_extension(Path::new("src/main.rs"), Path::new("src/main.rs"), &mut extensions);
    ///
    /// assert_eq!(extensions.get("rs"), Some(&"src/main.rs".into()));
    /// assert_eq!(extensions.get(".rs"), Some(&"src/main.rs".into()));
    /// ```
    fn record_extension(
        relative: &Path,
        relative_buf: &Path,
        extensions: &mut HashMap<String, PathBuf>,
    ) {
        let Some(ext) = relative.extension().and_then(|e| e.to_str()) else {
            return;
        };
        if extensions.contains_key(ext) {
            return;
        }
        let dot_ext = format!(".{ext}");
        extensions.insert(dot_ext, relative_buf.to_path_buf());
        extensions.insert(ext.to_string(), relative_buf.to_path_buf());
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
    fn detect(
        &self,
        project_root: &Path,
        cache: &mut ContentCache,
    ) -> Result<Vec<TechnologyDetection>>;
}

fn should_ignore_entry(_project_root: &Path, entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }

    if entry.depth() == 0 {
        return false;
    }

    // Optimization: Use entry.file_name() directly to avoid expensive path manipulations
    // and redundant strip_prefix calls during the walk.
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| IGNORED_DIRS.contains(&name))
}

// ---------------------------------------------------------------------------
// CatalogDrivenDetector — evaluates data-driven detection rules from catalog
// ---------------------------------------------------------------------------

struct CompiledDetectionRules {
    packages: Option<Vec<String>>,
    package_patterns: Option<Vec<Regex>>,
    /// Pre-converted PathBufs to avoid repeated heap allocations during detection.
    config_files: Option<Vec<PathBuf>>,
    config_file_content: Option<CompiledConfigFileContentRules>,
    file_extensions: Option<Vec<String>>,
}

struct CompiledConfigFileContentRules {
    /// Pre-converted PathBufs to avoid repeated heap allocations during detection.
    files: Option<Vec<PathBuf>>,
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

                let files = content_rules
                    .files
                    .as_ref()
                    .map(|files| files.iter().map(PathBuf::from).collect());

                Ok::<_, anyhow::Error>(CompiledConfigFileContentRules {
                    files,
                    patterns,
                    scan_gradle_layout: content_rules.scan_gradle_layout.unwrap_or(false),
                })
            })
            .transpose()?;

        let config_files = rules
            .config_files
            .as_ref()
            .map(|files| files.iter().map(PathBuf::from).collect());

        Ok(CompiledDetectionRules {
            packages: rules.packages.clone(),
            package_patterns,
            config_files,
            config_file_content,
            file_extensions: rules.file_extensions.clone(),
        })
    }
}

impl RepoDetector for CatalogDrivenDetector {
    /// Detects catalog-defined technologies in a project and its discovered nested projects.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let mut cache = ContentCache::default();
    /// let detections = detector
    ///     .detect(std::path::Path::new("."), &mut cache)
    ///     .unwrap();
    /// assert!(detections.iter().all(|d| !d.technology_id.is_empty()));
    /// ```
    ///
    /// # Returns
    ///
    /// A list of technology detections for the project and nested projects.
    fn detect(
        &self,
        project_root: &Path,
        cache: &mut ContentCache,
    ) -> Result<Vec<TechnologyDetection>> {
        if self.rules.is_empty() {
            return Ok(Vec::new());
        }

        // Optimization: Perform a single metadata collection for the root project.
        // This metadata now includes integrated discovery of nested projects.
        let metadata = RepoMetadata::collect(project_root);
        let all_packages = collect_package_names_with_nested(project_root, &metadata, cache);

        let mut detections = Vec::new();

        // Phase 1: Evaluate root project
        for (tech_id, compiled) in &self.rules {
            if let Some(detection) = evaluate_rules(
                project_root,
                tech_id,
                compiled,
                &all_packages,
                &metadata,
                cache,
            ) {
                detections.push(detection);
            }
        }

        // Phase 2: Scan nested projects (issue #409)
        detect_nested_projects(project_root, &metadata, &self.rules, cache, &mut detections);

        Ok(detections)
    }
}

/// Detects technologies used by nested projects and appends new detections with paths relative to the repository root.
///
/// Existing detections take precedence, so a technology is recorded only once.
///
/// # Examples
///
/// ```ignore
/// detect_nested_projects(
///     project_root,
///     metadata,
///     &rules,
///     &mut cache,
///     &mut detections,
/// );
/// ```
fn detect_nested_projects(
project_root: &Path,
metadata: &RepoMetadata,
rules: &[(TechnologyId, CompiledDetectionRules)],
cache: &mut ContentCache,
detections: &mut Vec<TechnologyDetection>,
) {
fn detect_nested_projects(
    project_root: &Path,
    metadata: &RepoMetadata,
    rules: &[(TechnologyId, CompiledDetectionRules)],
    cache: &mut ContentCache,
    detections: &mut Vec<TechnologyDetection>,
) {
    for rel_nested_dir in &metadata.nested_projects {
        let nested_dir = project_root.join(rel_nested_dir);
        let nested_meta = RepoMetadata::collect(&nested_dir);
        let nested_pkgs = collect_package_names(&nested_dir, &nested_meta, cache);

        for (tech_id, compiled) in rules {
            if detections.iter().any(|d| d.technology == *tech_id) {
                continue;
            }

            if let Some(detection) = evaluate_rules(
                &nested_dir,
                tech_id,
                compiled,
                &nested_pkgs,
                &nested_meta,
                cache,
            ) {
                detections.push(adjust_detection(detection, rel_nested_dir));
            }
        }
    }
}

/// Adjusts a nested detection so its paths are relative to the repository root.
///
/// # Examples
///
/// ```
/// # let detection: TechnologyDetection = todo!();
/// let adjusted = adjust_detection(detection, Path::new("packages/app"));
/// # let _ = adjusted;
/// ```
fn adjust_detection(detection: TechnologyDetection, offset: &Path) -> TechnologyDetection {
    TechnologyDetection {
        technology: detection.technology,
        confidence: detection.confidence,
        root_relative_paths: detection
            .root_relative_paths
            .iter()
            .map(|p| offset.join(p))
            .collect(),
        evidence: detection
            .evidence
            .iter()
            .map(|e| DetectionEvidence {
                marker: e.marker.clone(),
                path: offset.join(&e.path),
                notes: e.notes.clone(),
            })
            .collect(),
    }
}

/// Evaluates detection rules in precedence order and returns the first matching technology detection.
///
/// Package matches take precedence over configuration-file and extension matches. Returns `None`
/// when no configured rule matches the project metadata.
///
/// # Examples
///
/// ```rust,ignore
/// let detection = evaluate_rules(
///     project_root,
///     &technology_id,
///     &rules,
///     &all_packages,
///     &metadata,
///     &mut cache,
/// );
///
/// assert!(detection.is_some());
/// ```
///
/// # Arguments
///
/// * `project_root` - Root directory used to resolve configuration files.
/// * `tech_id` - Technology identifier associated with the rules.
/// * `rules` - Compiled rules used for detection.
/// * `all_packages` - Dependency names discovered in the project.
/// * `metadata` - Collected project filesystem metadata.
/// * `cache` - Cache used when reading configuration-file contents.
///
/// # Returns
///
/// The first matching technology detection, or `None` when no rule matches.
fn evaluate_rules(
    project_root: &Path,
    tech_id: &TechnologyId,
    rules: &CompiledDetectionRules,
    all_packages: &BTreeSet<String>,
    metadata: &RepoMetadata,
    cache: &mut ContentCache,
) -> Option<TechnologyDetection> {
    if let Some(d) = check_exact_packages(tech_id, rules, all_packages) {
        return Some(d);
    }
    if let Some(d) = check_package_patterns(tech_id, rules, all_packages) {
        return Some(d);
    }
    if let Some(d) = check_config_files(tech_id, rules, project_root, metadata) {
        return Some(d);
    }
    if let Some(d) = check_config_file_content(tech_id, rules, project_root, metadata, cache) {
        return Some(d);
    }
    check_file_extensions(tech_id, rules, metadata)
}

/// Detects a technology when one of its exact package names appears in the dependency set.
///
/// # Returns
///
/// A high-confidence detection for the first matching package, or `None` when no exact
/// package rule matches.
///
/// # Examples
///
/// ```rust,ignore
/// let detection = check_exact_packages(&tech_id, &rules, &all_packages);
/// assert_eq!(detection.unwrap().confidence, DetectionConfidence::High);
/// ```
fn check_exact_packages(
    tech_id: &TechnologyId,
    rules: &CompiledDetectionRules,
    all_packages: &BTreeSet<String>,
) -> Option<TechnologyDetection> {
    let packages = rules.packages.as_ref()?;
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
    None
}

/// Finds a package-pattern rule that matches one of the detected package names.
///
/// Matching uses the configured pattern order and returns the first matching package
/// with medium detection confidence.
///
/// # Returns
///
/// A detection for the first matching package, or `None` when no pattern matches.
///
/// # Examples
///
/// ```ignore
/// let detection = check_package_patterns(&tech_id, &rules, &all_packages);
/// assert!(detection.is_some());
/// ```
fn check_package_patterns(
    tech_id: &TechnologyId,
    rules: &CompiledDetectionRules,
    all_packages: &BTreeSet<String>,
) -> Option<TechnologyDetection> {
    let patterns = rules.package_patterns.as_ref()?;
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
    None
}

/// Detects a technology when one of its configured files exists in the project.
///
/// # Examples
///
/// ```
/// # // Example usage within the detector module.
/// # let detection = check_config_files(&tech_id, &rules, project_root, &metadata);
/// # assert!(detection.is_some() || detection.is_none());
/// ```
fn check_config_files(
    tech_id: &TechnologyId,
    rules: &CompiledDetectionRules,
    project_root: &Path,
    metadata: &RepoMetadata,
) -> Option<TechnologyDetection> {
    let config_files = rules.config_files.as_ref()?;
    for path in config_files {
        if metadata.paths.contains(path) || project_root.join(path).exists() {
            let display = path.display().to_string();
            return Some(make_detection(
                tech_id,
                DetectionConfidence::High,
                &display,
                &format!("config file '{}' exists", display),
            ));
        }
    }
    None
}

/// Finds a configuration-file content pattern that identifies a technology.
///
/// Scans the applicable project files and returns the first matching detection.
///
/// # Examples
///
/// ```ignore
/// let detection = check_config_file_content(
///     &tech_id,
///     &rules,
///     project_root,
///     &metadata,
///     &mut cache,
/// );
/// assert!(detection.is_some());
/// ```
///
/// # Returns
///
/// A medium-confidence detection for the first matching pattern, or `None` if
/// no configured file contains a matching pattern.
fn check_config_file_content(
    tech_id: &TechnologyId,
    rules: &CompiledDetectionRules,
    project_root: &Path,
    metadata: &RepoMetadata,
    cache: &mut ContentCache,
) -> Option<TechnologyDetection> {
    let content_rules = rules.config_file_content.as_ref()?;
    let files_to_scan = gather_content_scan_files(project_root, content_rules, metadata);
    for file_path in &files_to_scan {
        let absolute = project_root.join(file_path);
        let Some(content) = get_file_content(&absolute, cache) else {
            continue;
        };
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
    None
}

/// Identifies the repository technology from a matching file extension.
///
/// # Examples
///
/// ```ignore
/// let detection = check_file_extensions(&tech_id, &rules, &metadata);
/// assert!(detection.is_some());
/// ```
fn check_file_extensions(
    tech_id: &TechnologyId,
    rules: &CompiledDetectionRules,
    metadata: &RepoMetadata,
) -> Option<TechnologyDetection> {
    let extensions = rules.file_extensions.as_ref()?;
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

/// Collects the files to scan for configuration-content detection.
///
/// Includes files from the configured Gradle layout and explicitly configured
/// paths, while retaining only files recognized by the repository metadata.
///
/// # Examples
///
/// ```ignore
/// let files = gather_content_scan_files(project_root, &rules, &metadata);
/// assert!(files.iter().all(|path| path.is_file()));
/// ```
fn gather_content_scan_files(
    project_root: &Path,
    rules: &CompiledConfigFileContentRules,
    metadata: &RepoMetadata,
) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if rules.scan_gradle_layout {
        gather_gradle_files(metadata, &mut files);
    }

    if let Some(explicit_files) = &rules.files {
        gather_explicit_files(project_root, explicit_files, metadata, &mut files);
    }

    files
}

/// Collects recognized Gradle build and version catalog files present in the repository metadata.
///
/// Files at the repository root and in its immediate root directories are appended to `files`.
///
/// # Examples
///
/// ```
/// let metadata = RepoMetadata::default();
/// let mut files = Vec::new();
///
/// gather_gradle_files(&metadata, &mut files);
/// assert!(files.is_empty());
/// ```
fn gather_gradle_files(metadata: &RepoMetadata, files: &mut Vec<PathBuf>) {
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

    for dir in &metadata.root_dirs {
        for build_file in &["build.gradle.kts", "build.gradle"] {
            let path = dir.join(build_file);
            if metadata.paths.contains(&path) {
                files.push(path);
            }
        }
    }
}

/// Adds existing explicit files to the collection, avoiding duplicates.
///
/// # Arguments
///
/// * `project_root` - Root directory used to resolve explicit file paths.
/// * `explicit_files` - Paths explicitly selected for content scanning.
/// * `metadata` - Repository metadata containing discovered paths.
/// * `files` - Collection to which eligible paths are added.
///
/// # Examples
///
/// ```rust,ignore
/// gather_explicit_files(&project_root, &explicit_files, &metadata, &mut files);
/// ```
fn gather_explicit_files(
    project_root: &Path,
    explicit_files: &[PathBuf],
    metadata: &RepoMetadata,
    files: &mut Vec<PathBuf>,
) {
    for path in explicit_files {
        if (metadata.paths.contains(path) || project_root.join(path).exists())
            && !files.contains(path)
        {
            files.push(path.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// Package.json parsing and workspace resolution
// ---------------------------------------------------------------------------

fn collect_package_names(
    project_root: &Path,
    metadata: &RepoMetadata,
    cache: &mut ContentCache,
) -> BTreeSet<String> {
    let mut all_packages = BTreeSet::new();

    // Parse root package.json
    let root_pkg_path = Path::new("package.json");
    if metadata.paths.contains(root_pkg_path)
        && let Some(deps) = parse_package_json_deps(&project_root.join(root_pkg_path), cache)
    {
        all_packages.extend(deps);
    }

    // Resolve workspaces and parse each workspace's package.json
    let workspace_dirs = resolve_workspaces(project_root, metadata, cache);
    for workspace_dir in workspace_dirs {
        let pkg_path = workspace_dir.join("package.json");
        // We still check existence here because resolve_workspaces ensures they existed,
        // but parse_package_json_deps does its own read.
        if let Some(deps) = parse_package_json_deps(&pkg_path, cache) {
            all_packages.extend(deps);
        }
    }

    let requirements_path = Path::new("requirements.txt");
    if metadata.paths.contains(requirements_path)
        && let Some(deps) =
            parse_requirements_txt_deps(&project_root.join(requirements_path), cache)
    {
        all_packages.extend(deps);
    }

    let pyproject_path = Path::new("pyproject.toml");
    if metadata.paths.contains(pyproject_path)
        && let Some(deps) = parse_pyproject_toml_deps(&project_root.join(pyproject_path), cache)
    {
        all_packages.extend(deps);
    }

    let pipfile_path = Path::new("Pipfile");
    if metadata.paths.contains(pipfile_path)
        && let Some(deps) = parse_pipfile_deps(&project_root.join(pipfile_path), cache)
    {
        all_packages.extend(deps);
    }

    all_packages
}

fn parse_package_json_deps(path: &Path, cache: &mut ContentCache) -> Option<BTreeSet<String>> {
    let content = get_file_content(path, cache)?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let serde_json::Value::Object(mut obj) = json else {
        return None;
    };

    let mut deps = BTreeSet::new();
    for key in &["dependencies", "devDependencies", "peerDependencies"] {
        // Optimization: Use obj.remove() to take ownership of the dependency section
        // and iterate over owned keys to avoid cloning dependency names.
        if let Some(serde_json::Value::Object(section)) = obj.remove(*key) {
            for (dep_name, _) in section {
                deps.insert(dep_name);
            }
        }
    }

    Some(deps)
}

fn parse_requirements_txt_deps(path: &Path, cache: &mut ContentCache) -> Option<BTreeSet<String>> {
    let mut deps = BTreeSet::new();
    let mut visited = HashSet::new();
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    parse_requirements_file(path, root, &mut deps, &mut visited, cache).ok()?;
    Some(deps)
}

fn parse_requirements_file(
    path: &Path,
    root: &Path,
    deps: &mut BTreeSet<String>,
    visited: &mut HashSet<PathBuf>,
    cache: &mut ContentCache,
) -> Result<()> {
    let path = canonical_existing_path(path)?;
    let root = canonical_existing_path(root)?;
    if !path.starts_with(&root) {
        return Ok(());
    }
    if !visited.insert(path.clone()) {
        return Ok(());
    }

    let content = get_file_content(&path, cache)
        .with_context(|| format!("Failed to read requirements file: {}", path.display()))?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    for raw_line in content.lines() {
        parse_requirement_line(raw_line, &root, base_dir, deps, visited, cache)?;
    }

    Ok(())
}

fn parse_requirement_line(
    raw_line: &str,
    root: &Path,
    base_dir: &Path,
    deps: &mut BTreeSet<String>,
    visited: &mut HashSet<PathBuf>,
    cache: &mut ContentCache,
) -> Result<()> {
    let raw_line = raw_line.trim();
    if raw_line.starts_with('#') {
        return Ok(());
    }

    if let Some((_, egg)) = raw_line.split_once("#egg=") {
        if let Some(name) = normalize_python_requirement_name(egg) {
            deps.insert(name);
        }
        return Ok(());
    }

    let line = raw_line.split('#').next().unwrap_or_default().trim();
    if line.is_empty() {
        return Ok(());
    }

    if let Some(include_path) = requirement_include_path(line) {
        let include_path = known_child_path(base_dir, include_path);
        parse_requirements_file(&include_path, root, deps, visited, cache)?;
        return Ok(());
    }

    if let Some((_, egg)) = line.split_once("#egg=") {
        if let Some(name) = normalize_python_requirement_name(egg) {
            deps.insert(name);
        }
        return Ok(());
    }

    if line.starts_with('-') {
        return Ok(());
    }

    if let Some(name) = normalize_python_requirement_name(line) {
        deps.insert(name);
    }

    Ok(())
}

fn requirement_include_path(line: &str) -> Option<&str> {
    line.strip_prefix("-r ")
        .or_else(|| line.strip_prefix("--requirement "))
        .or_else(|| line.strip_prefix("--requirement="))
        .map(str::trim)
        .filter(|path| !path.is_empty())
}

fn known_child_path(root: &Path, child: &str) -> PathBuf {
    root.join(child)
}

fn canonical_existing_path(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .with_context(|| format!("failed to resolve path {}", path.display()))
}

/// Parses a `pyproject.toml` file and collects its declared Python dependencies.
///
/// Returns `None` when the file cannot be resolved, read, or parsed. Otherwise, returns the
/// dependency names declared using supported PEP 621 and Poetry formats.
///
/// # Examples
///
/// ```
/// let mut cache = ContentCache::default();
/// let dependencies = parse_pyproject_toml_deps(
///     std::path::Path::new("pyproject.toml"),
///     &mut cache,
/// );
/// assert!(dependencies.is_some() || dependencies.is_none());
/// ```
fn parse_pyproject_toml_deps(path: &Path, cache: &mut ContentCache) -> Option<BTreeSet<String>> {
    let path = canonical_existing_path(path).ok()?;
    let content = get_file_content(&path, cache)?;
    let value: toml::Value = toml::from_str(&content).ok()?;
    let mut deps = BTreeSet::new();

    collect_pep621_deps(&value, &mut deps);
    collect_poetry_deps(&value, &mut deps);

    Some(deps)
}

/// Collects PEP 621 project and optional dependency names from a TOML document.
///
/// # Arguments
///
/// * `value` - TOML data containing an optional `[project]` table.
/// * `deps` - Set to which normalized dependency names are added.
///
/// # Examples
///
/// ```
/// let value: toml::Value = toml::from_str(
///     r#"
///     [project]
///     dependencies = ["requests>=2"]
///
///     [project.optional-dependencies]
///     test = ["pytest"]
///     "#,
/// ).unwrap();
/// let mut deps = std::collections::BTreeSet::new();
///
/// collect_pep621_deps(&value, &mut deps);
///
/// assert!(deps.contains("requests"));
/// assert!(deps.contains("pytest"));
/// ```
fn collect_pep621_deps(value: &toml::Value, deps: &mut BTreeSet<String>) {
    let Some(project) = value.get("project").and_then(|v| v.as_table()) else {
        return;
    };
    if let Some(dependencies) = project.get("dependencies").and_then(|v| v.as_array()) {
        collect_python_dependency_array(dependencies, deps);
    }
    if let Some(optional) = project
        .get("optional-dependencies")
        .and_then(|v| v.as_table())
    {
        for dependencies in optional.values().filter_map(|v| v.as_array()) {
            collect_python_dependency_array(dependencies, deps);
        }
    }
}

/// Collects Poetry dependency names from a TOML document into a set.
///
/// Dependencies from the main, grouped, and development Poetry sections are included.
///
/// # Examples
///
/// ```
/// let value: toml::Value = r#"
/// [tool.poetry.dependencies]
/// python = "^3.11"
/// requests = "^2.31"
///
/// [tool.poetry.dev-dependencies]
/// pytest = "^7"
/// "#
/// .parse()
/// .unwrap();
/// let mut dependencies = std::collections::BTreeSet::new();
///
/// collect_poetry_deps(&value, &mut dependencies);
///
/// assert!(dependencies.contains("python"));
/// assert!(dependencies.contains("requests"));
/// assert!(dependencies.contains("pytest"));
/// ```
fn collect_poetry_deps(value: &toml::Value, deps: &mut BTreeSet<String>) {
    let Some(poetry) = value
        .get("tool")
        .and_then(|v| v.get("poetry"))
        .and_then(|v| v.as_table())
    else {
        return;
    };
    if let Some(dependencies) = poetry.get("dependencies").and_then(|v| v.as_table()) {
        collect_python_dependency_table(dependencies, deps);
    }
    if let Some(group) = poetry.get("group").and_then(|v| v.as_table()) {
        for dependencies in group.values().filter_map(|group| {
            group
                .get("dependencies")
                .and_then(|dependencies| dependencies.as_table())
        }) {
            collect_python_dependency_table(dependencies, deps);
        }
    }
    if let Some(dev_dependencies) = poetry.get("dev-dependencies").and_then(|v| v.as_table()) {
        collect_python_dependency_table(dev_dependencies, deps);
    }
}

/// Parses package and development dependencies from a Pipfile.
///
/// Returns `None` when the file cannot be resolved, read, or parsed as TOML.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
///
/// let mut cache = ContentCache::default();
/// let dependencies = parse_pipfile_deps(Path::new("Pipfile"), &mut cache);
/// assert!(dependencies.is_some());
/// ```
fn parse_pipfile_deps(path: &Path, cache: &mut ContentCache) -> Option<BTreeSet<String>> {
    let path = canonical_existing_path(path).ok()?;
    let content = get_file_content(&path, cache)?;
    let value: toml::Value = toml::from_str(&content).ok()?;
    let mut deps = BTreeSet::new();

    for section in ["packages", "dev-packages"] {
        if let Some(dependencies) = value.get(section).and_then(|v| v.as_table()) {
            collect_python_dependency_table(dependencies, &mut deps);
        }
    }

    Some(deps)
}

fn collect_python_dependency_array(values: &[toml::Value], deps: &mut BTreeSet<String>) {
    for dependency in values.iter().filter_map(|v| v.as_str()) {
        if let Some(name) = normalize_python_requirement_name(dependency) {
            deps.insert(name);
        }
    }
}

fn collect_python_dependency_table(
    table: &toml::map::Map<String, toml::Value>,
    deps: &mut BTreeSet<String>,
) {
    for package in table.keys() {
        if package != "python" {
            deps.insert(package.to_ascii_lowercase());
        }
    }
}

fn normalize_python_requirement_name(requirement: &str) -> Option<String> {
    let trimmed = requirement.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() {
        return None;
    }

    let end = trimmed
        .find(|c: char| {
            c.is_whitespace() || matches!(c, '[' | '<' | '>' | '=' | '!' | '~' | ';' | ',')
        })
        .unwrap_or(trimmed.len());
    let name = trimmed[..end].trim();

    if name.is_empty() {
        None
    } else {
        Some(name.to_ascii_lowercase())
    }
}

fn resolve_workspaces(
    project_root: &Path,
    metadata: &RepoMetadata,
    cache: &mut ContentCache,
) -> Vec<PathBuf> {
    // Try pnpm-workspace.yaml first
    let pnpm_rel = Path::new("pnpm-workspace.yaml");
    if metadata.paths.contains(pnpm_rel)
        && let Some(content) = get_file_content(&project_root.join(pnpm_rel), cache)
    {
        let patterns = parse_pnpm_workspace_yaml(&content);
        if !patterns.is_empty() {
            return expand_workspace_patterns(project_root, &patterns, metadata);
        }
    }

    // Try package.json workspaces field
    let pkg_rel = Path::new("package.json");
    if metadata.paths.contains(pkg_rel)
        && let Some(content) = get_file_content(&project_root.join(pkg_rel), cache)
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(workspaces) = json.get("workspaces")
    {
        let patterns = parse_package_json_workspaces(workspaces);
        if !patterns.is_empty() {
            return expand_workspace_patterns(project_root, &patterns, metadata);
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

/// Expands workspace path patterns into existing workspace directories containing a `package.json` manifest.
///
/// Supports exact paths and one-level wildcard directory patterns relative to the project root.
///
/// # Examples
///
/// ```
/// let metadata = RepoMetadata::default();
/// let workspaces = expand_workspace_patterns(
///     Path::new("."),
///     &["packages/*".to_owned()],
///     &metadata,
/// );
/// ```
fn expand_workspace_patterns(
    project_root: &Path,
    patterns: &[String],
    metadata: &RepoMetadata,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    for pattern in patterns {
        let base = pattern
            .trim_end_matches("/**")
            .trim_end_matches("/*")
            .trim_end_matches('/');

        let base_rel = Path::new(base);

        if pattern.contains('*') {
            expand_glob_workspace(project_root, base_rel, metadata, &mut dirs);
        } else {
            expand_exact_workspace(project_root, base_rel, metadata, &mut dirs);
        }
    }

    dirs
}

/// Expands a one-level workspace wildcard by adding child directories that contain a `package.json` manifest.
///
/// # Arguments
///
/// * `project_root` - Absolute root directory of the project.
/// * `base_rel` - Relative directory containing the workspace’s immediate children.
/// * `metadata` - Repository metadata used to identify directories and manifests.
/// * `dirs` - Collection to which matching workspace directories are appended.
///
/// # Examples
///
/// ```rust,ignore
/// expand_glob_workspace(&project_root, Path::new("packages"), &metadata, &mut dirs);
/// assert!(dirs.iter().all(|dir| dir.join("package.json").exists()));
/// ```
fn expand_glob_workspace(
    project_root: &Path,
    base_rel: &Path,
    metadata: &RepoMetadata,
    dirs: &mut Vec<PathBuf>,
) {
    for dir_rel in &metadata.dirs {
        if dir_rel.parent() != Some(base_rel) {
            continue;
        }
        let manifest = dir_rel.join("package.json");
        if metadata.paths.contains(&manifest) || project_root.join(&manifest).exists() {
            dirs.push(project_root.join(dir_rel));
        }
    }
}

/// Adds an exact workspace directory when its `package.json` manifest exists.
///
/// # Examples
///
/// ```no_run
/// expand_exact_workspace(
///     project_root,
///     base_rel,
///     metadata,
///     &mut workspace_dirs,
/// );
/// ```
fn expand_exact_workspace(
    project_root: &Path,
    base_rel: &Path,
    metadata: &RepoMetadata,
    dirs: &mut Vec<PathBuf>,
) {
    let manifest = base_rel.join("package.json");
    if metadata.paths.contains(&manifest) || project_root.join(&manifest).exists() {
        dirs.push(project_root.join(base_rel));
    }
}

/// Collects package names including from nested projects.
///
/// NOTE: At root-phase (Phase 1), only package.json deps are merged from nested projects
/// because they're needed for package_patterns detection (e.g., "@azure/*" matches).
/// Other ecosystems (Python, Java, etc.) are handled later in Phase 2
/// where each nested project gets full `collect_package_names()` processing.
fn collect_package_names_with_nested(
    project_root: &Path,
    metadata: &RepoMetadata,
    cache: &mut ContentCache,
) -> BTreeSet<String> {
    let mut pkgs = collect_package_names(project_root, metadata, cache);
    for rel_nested in &metadata.nested_projects {
        if let Some(deps) =
            parse_package_json_deps(&project_root.join(rel_nested).join("package.json"), cache)
        {
            pkgs.extend(deps);
        }
    }
    pkgs
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn collect_package_names_reads_requirements_txt_dependencies() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("requirements.txt"),
            "django>=4.2\nfastapi[standard]==0.115.0\n# -e git+https://example.com/demo.git#egg=flask\n",
        )
        .unwrap();

        let mut cache = ContentCache::new();
        let metadata = RepoMetadata::collect(temp.path());
        let packages = collect_package_names(temp.path(), &metadata, &mut cache);

        assert!(packages.contains("django"));
        assert!(packages.contains("fastapi"));
        assert!(!packages.contains("flask"));
    }

    #[test]
    fn collect_package_names_reads_nested_requirements_txt_dependencies() {
        let mut cache = ContentCache::new();
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join("requirements")).unwrap();
        fs::write(
            temp.path().join("requirements.txt"),
            "-r requirements/base.txt\n--requirement=requirements/dev.txt\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("requirements/base.txt"),
            "django>=4.2\n-r cycle.txt\n",
        )
        .unwrap();
        fs::write(temp.path().join("requirements/dev.txt"), "pytest>=8\n").unwrap();
        fs::write(
            temp.path().join("requirements/cycle.txt"),
            "-r ../requirements.txt\n",
        )
        .unwrap();

        let metadata = RepoMetadata::collect(temp.path());
        let packages = collect_package_names(temp.path(), &metadata, &mut cache);

        assert!(packages.contains("django"));
        assert!(packages.contains("pytest"));
    }

    #[test]
    fn collect_package_names_reads_pyproject_dependencies() {
        let mut cache = ContentCache::new();
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("pyproject.toml"),
            r#"
[project]
dependencies = [
  "django>=4.2",
  "fastapi[standard]==0.115.0",
]

[project.optional-dependencies]
test = ["pytest>=8", "requests"]

[tool.poetry.dependencies]
python = "^3.12"
pydantic = "^2"
sqlalchemy = { version = "^2", extras = ["asyncio"] }

[tool.poetry.group.dev.dependencies]
pandas = "^2"
"#,
        )
        .unwrap();

        let metadata = RepoMetadata::collect(temp.path());
        let packages = collect_package_names(temp.path(), &metadata, &mut cache);

        for package in [
            "django",
            "fastapi",
            "pytest",
            "requests",
            "pydantic",
            "sqlalchemy",
            "pandas",
        ] {
            assert!(
                packages.contains(package),
                "missing {package}: {packages:?}"
            );
        }
    }

    #[test]
    fn collect_package_names_reads_pipfile_dependencies() {
        let mut cache = ContentCache::new();
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("Pipfile"),
            r#"
[packages]
flask = "*"
celery = { version = "*", extras = ["redis"] }

[dev-packages]
pytest = "*"
"#,
        )
        .unwrap();

        let metadata = RepoMetadata::collect(temp.path());
        let packages = collect_package_names(temp.path(), &metadata, &mut cache);

        assert!(packages.contains("flask"));
        assert!(packages.contains("celery"));
        assert!(packages.contains("pytest"));
    }

    #[test]
    fn discover_nested_projects_finds_dockerfile_in_subdirectory() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join("services/api")).unwrap();
        fs::write(temp.path().join("services/api/Dockerfile"), "FROM node:20").unwrap();

        let metadata = RepoMetadata::collect(temp.path());

        assert!(
            metadata
                .nested_projects
                .iter()
                .any(|p| p.ends_with("services/api"))
        );
    }

    #[test]
    fn discover_nested_projects_finds_multiple_manifests() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join("backend")).unwrap();
        fs::create_dir_all(temp.path().join("frontend")).unwrap();
        fs::write(temp.path().join("backend/pom.xml"), "<project/>").unwrap();
        fs::write(temp.path().join("frontend/package.json"), "{}").unwrap();

        let metadata = RepoMetadata::collect(temp.path());

        assert_eq!(metadata.nested_projects.len(), 2);
    }

    #[test]
    fn collect_package_names_with_nested_includes_nested_packages() {
        let mut cache = ContentCache::new();
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{"dependencies": {"express": "^4.0"}}"#,
        )
        .unwrap();
        fs::create_dir_all(temp.path().join("services/api")).unwrap();
        fs::write(
            temp.path().join("services/api/package.json"),
            r#"{"dependencies": {"axios": "^1.0"}}"#,
        )
        .unwrap();

        let metadata = RepoMetadata::collect(temp.path());
        let packages = collect_package_names_with_nested(temp.path(), &metadata, &mut cache);

        assert!(packages.contains("express"));
        assert!(packages.contains("axios"));
    }
}
