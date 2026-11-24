//! Offline skill quality validation.
//!
//! This module provides structural and fixture-based checks for skills
//! without executing any code, contacting external services, or invoking
//! model APIs. All validation is completely offline and deterministic.

use crate::diagnostics::{
    BoundedTraversal, Diagnostic, DiagnosticReport, Severity, Span, TraversalError, ValidationRule,
    fingerprint, redact_evidence,
};
use std::path::{Path, PathBuf};

/// Validate a skill directory and return a diagnostic report.
pub fn validate_skill(skill_path: &Path) -> Result<DiagnosticReport, ValidationError> {
    let mut findings = Vec::new();

    // Check if the skill directory exists
    if !skill_path.exists() {
        return Err(ValidationError::NotFound(skill_path.to_path_buf()));
    }

    if !skill_path.is_dir() {
        return Err(ValidationError::NotADirectory(skill_path.to_path_buf()));
    }

    // Apply all validation rules
    let rules: Vec<Box<dyn ValidationRule>> = vec![
        Box::new(ManifestFrontmatterRule),
        Box::new(RequiredFieldsRule),
        Box::new(RelativeReferencesRule {
            root: skill_path.to_path_buf(),
        }),
        Box::new(PathEscapeRule {
            _root: skill_path.to_path_buf(),
        }),
        Box::new(BundleIntegrityRule {
            _root: skill_path.to_path_buf(),
        }),
    ];

    // Traverse the skill directory with bounds
    let traversal = BoundedTraversal::new()
        .max_depth(10)
        .max_content_size(5 * 1024 * 1024) // 5MB
        .max_files(500);

    // Check each file
    if let Err(err) = traversal.traverse(skill_path, |path, content| {
        for rule in &rules {
            findings.extend(rule.check_file(path, content));
        }
        true // Continue traversal
    }) {
        match err {
            TraversalError::MaxDepthExceeded => {
                findings.push(Diagnostic {
                    rule_id: "traversal-depth".into(),
                    severity: Severity::Error,
                    source_path: skill_path.to_string_lossy().into(),
                    span: None,
                    message: "Skill directory exceeds maximum traversal depth.".into(),
                    evidence: None,
                    fingerprint: fingerprint(
                        "traversal-depth",
                        &skill_path.to_string_lossy(),
                        &None,
                    ),
                });
            }
            TraversalError::MaxFilesExceeded => {
                findings.push(Diagnostic {
                    rule_id: "traversal-files".into(),
                    severity: Severity::Error,
                    source_path: skill_path.to_string_lossy().into(),
                    span: None,
                    message: "Skill contains too many files.".into(),
                    evidence: None,
                    fingerprint: fingerprint(
                        "traversal-files",
                        &skill_path.to_string_lossy(),
                        &None,
                    ),
                });
            }
            TraversalError::MaxContentSizeExceeded => {
                findings.push(Diagnostic {
                    rule_id: "traversal-size".into(),
                    severity: Severity::Error,
                    source_path: skill_path.to_string_lossy().into(),
                    span: None,
                    message: "Skill total content size exceeds limit.".into(),
                    evidence: None,
                    fingerprint: fingerprint(
                        "traversal-size",
                        &skill_path.to_string_lossy(),
                        &None,
                    ),
                });
            }
            _ => {}
        }
    }

    // Check directory-level rules
    for rule in &rules {
        findings.extend(rule.check_directory(skill_path));
    }

    Ok(DiagnosticReport::new(findings))
}

/// Error types for skill validation.
#[derive(Debug)]
pub enum ValidationError {
    NotFound(PathBuf),
    NotADirectory(PathBuf),
    IoError(std::io::Error),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "skill not found: {}", path.display()),
            Self::NotADirectory(path) => write!(f, "not a directory: {}", path.display()),
            Self::IoError(err) => write!(f, "I/O error: {}", err),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate that SKILL.md has valid frontmatter.
struct ManifestFrontmatterRule;

impl ValidationRule for ManifestFrontmatterRule {
    fn rule_id(&self) -> &str {
        "manifest-frontmatter"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check_file(&self, path: &Path, content: &[u8]) -> Vec<Diagnostic> {
        if path.file_name().and_then(|n| n.to_str()) != Some("SKILL.md") {
            return vec![];
        }

        let content_str = String::from_utf8_lossy(content);
        let mut findings = Vec::new();

        // Check for frontmatter delimiters
        if !content_str.starts_with("---") {
            findings.push(Diagnostic {
                rule_id: self.rule_id().into(),
                severity: self.default_severity(),
                source_path: path.to_string_lossy().into(),
                span: Some(Span::line(1)),
                message: "SKILL.md does not start with YAML frontmatter delimiter (---).".into(),
                evidence: None,
                fingerprint: fingerprint(self.rule_id(), &path.to_string_lossy(), &None),
            });
        }

        findings
    }
}

/// Validate required fields in SKILL.md frontmatter.
struct RequiredFieldsRule;

impl ValidationRule for RequiredFieldsRule {
    fn rule_id(&self) -> &str {
        "required-fields"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check_file(&self, path: &Path, content: &[u8]) -> Vec<Diagnostic> {
        if path.file_name().and_then(|n| n.to_str()) != Some("SKILL.md") {
            return vec![];
        }

        let content_str = String::from_utf8_lossy(content);
        let mut findings = Vec::new();

        // Extract frontmatter
        if let Some(frontmatter) = extract_frontmatter(&content_str) {
            // Check for required fields (name is required per Agent Skills spec)
            if !frontmatter.contains("name:") {
                findings.push(Diagnostic {
                    rule_id: self.rule_id().into(),
                    severity: self.default_severity(),
                    source_path: path.to_string_lossy().into(),
                    span: Some(Span::line(1)),
                    message: "SKILL.md frontmatter is missing required 'name' field.".into(),
                    evidence: None,
                    fingerprint: fingerprint(self.rule_id(), &path.to_string_lossy(), &None),
                });
            }
        }

        findings
    }
}

/// Validate that relative references exist.
struct RelativeReferencesRule {
    root: PathBuf,
}

impl ValidationRule for RelativeReferencesRule {
    fn rule_id(&self) -> &str {
        "relative-references"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_file(&self, path: &Path, content: &[u8]) -> Vec<Diagnostic> {
        if path.file_name().and_then(|n| n.to_str()) != Some("SKILL.md") {
            return vec![];
        }

        let content_str = String::from_utf8_lossy(content);
        let mut findings = Vec::new();

        // Extract frontmatter and check references
        if let Some(frontmatter) = extract_frontmatter(&content_str) {
            // Check for relative file references in the frontmatter
            for (key, value) in parse_simple_yaml(&frontmatter) {
                if (key == "reference" || key == "file" || key == "path")
                    && let Some(rel_path) = value.strip_prefix("./")
                {
                    let full_path = self.root.join(rel_path);
                    if !full_path.exists() {
                        findings.push(Diagnostic {
                            rule_id: self.rule_id().into(),
                            severity: self.default_severity(),
                            source_path: path.to_string_lossy().into(),
                            span: None,
                            message: format!("Relative reference '{}' does not exist.", rel_path),
                            evidence: Some(redact_evidence(&value)),
                            fingerprint: fingerprint(
                                self.rule_id(),
                                &path.to_string_lossy(),
                                &None,
                            ),
                        });
                    }
                }
            }
        }

        findings
    }
}

/// Validate that paths do not escape the skill root.
struct PathEscapeRule {
    _root: PathBuf,
}

impl ValidationRule for PathEscapeRule {
    fn rule_id(&self) -> &str {
        "path-escape"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check_file(&self, path: &Path, content: &[u8]) -> Vec<Diagnostic> {
        if path.file_name().and_then(|n| n.to_str()) != Some("SKILL.md") {
            return vec![];
        }

        let content_str = String::from_utf8_lossy(content);
        let mut findings = Vec::new();

        // Check for path traversal patterns
        if content_str.contains("../") || content_str.contains("..\\") {
            findings.push(Diagnostic {
                rule_id: self.rule_id().into(),
                severity: self.default_severity(),
                source_path: path.to_string_lossy().into(),
                span: None,
                message: "SKILL.md contains path traversal patterns (../).".into(),
                evidence: None,
                fingerprint: fingerprint(self.rule_id(), &path.to_string_lossy(), &None),
            });
        }

        findings
    }
}

/// Validate bundle integrity.
struct BundleIntegrityRule {
    _root: PathBuf,
}

impl ValidationRule for BundleIntegrityRule {
    fn rule_id(&self) -> &str {
        "bundle-integrity"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_directory(&self, path: &Path) -> Vec<Diagnostic> {
        let mut findings = Vec::new();

        // Check for SKILL.md existence
        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() {
            findings.push(Diagnostic {
                rule_id: self.rule_id().into(),
                severity: Severity::Error,
                source_path: path.to_string_lossy().into(),
                span: None,
                message: "Skill directory is missing SKILL.md.".into(),
                evidence: None,
                fingerprint: fingerprint(self.rule_id(), &path.to_string_lossy(), &None),
            });
        }

        // Check for duplicate skill IDs (if multiple SKILL.md files exist)
        let mut skill_names = Vec::new();
        if let Ok(entries) = path.read_dir() {
            for entry in entries.flatten() {
                let sub_dir = entry.path();
                if sub_dir.is_dir() {
                    let sub_skill_md = sub_dir.join("SKILL.md");
                    if sub_skill_md.exists()
                        && let Ok(content) = std::fs::read_to_string(&sub_skill_md)
                        && let Some(frontmatter) = extract_frontmatter(&content)
                    {
                        for (_, value) in parse_simple_yaml(&frontmatter) {
                            if !value.is_empty() {
                                skill_names.push(value.clone());
                            }
                        }
                    }
                }
            }
        }

        // Check for duplicates
        let mut seen = std::collections::HashSet::new();
        for name in &skill_names {
            if !seen.insert(name) {
                findings.push(Diagnostic {
                    rule_id: "duplicate-id".into(),
                    severity: Severity::Error,
                    source_path: path.to_string_lossy().into(),
                    span: None,
                    message: format!("Duplicate skill ID detected: {}", name),
                    evidence: None,
                    fingerprint: fingerprint("duplicate-id", &path.to_string_lossy(), &None),
                });
            }
        }

        findings
    }

    fn check_file(&self, _path: &Path, _content: &[u8]) -> Vec<Diagnostic> {
        vec![]
    }
}

/// Extract YAML frontmatter from a markdown string.
fn extract_frontmatter(content: &str) -> Option<String> {
    let mut lines = content.lines();

    // First line should be ---
    if lines.next() != Some("---") {
        return None;
    }

    let mut frontmatter = Vec::new();
    for line in lines {
        if line == "---" {
            return Some(frontmatter.join("\n"));
        }
        frontmatter.push(line);
    }

    None
}

/// Simple YAML parser for basic key-value pairs.
fn parse_simple_yaml(yaml: &str) -> Vec<(String, String)> {
    yaml.lines()
        .filter_map(|line| {
            let line = line.trim();
            if let Some((key, value)) = line.split_once(':') {
                Some((key.trim().to_string(), value.trim().to_string()))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_skill(dir: &Path, content: &str) {
        fs::write(dir.join("SKILL.md"), content).unwrap();
    }

    #[test]
    fn test_valid_skill() {
        let temp_dir = TempDir::new().unwrap();
        create_test_skill(
            temp_dir.path(),
            "---\nname: test-skill\nversion: 1.0.0\n---\n\n# Test Skill\n\nThis is a test skill.",
        );

        let report = validate_skill(temp_dir.path()).unwrap();
        assert!(!report.has_errors());
    }

    #[test]
    fn test_missing_frontmatter() {
        let temp_dir = TempDir::new().unwrap();
        create_test_skill(
            temp_dir.path(),
            "# Test Skill\n\nThis is a test skill without frontmatter.",
        );

        let report = validate_skill(temp_dir.path()).unwrap();
        assert!(report.has_errors());

        let frontmatter_errors = report
            .findings
            .iter()
            .filter(|f| f.rule_id == "manifest-frontmatter")
            .collect::<Vec<_>>();
        assert!(!frontmatter_errors.is_empty());
    }

    #[test]
    fn test_missing_name_field() {
        let temp_dir = TempDir::new().unwrap();
        create_test_skill(
            temp_dir.path(),
            "---\ndescription: test\n---\n\n# Test Skill",
        );

        let report = validate_skill(temp_dir.path()).unwrap();
        assert!(report.has_errors());

        let required_errors = report
            .findings
            .iter()
            .filter(|f| f.rule_id == "required-fields")
            .collect::<Vec<_>>();
        assert!(!required_errors.is_empty());
    }

    #[test]
    fn test_path_traversal() {
        let temp_dir = TempDir::new().unwrap();
        create_test_skill(
            temp_dir.path(),
            "---\nname: test\npath: ../escape\n---\n\n# Test",
        );

        let report = validate_skill(temp_dir.path()).unwrap();
        assert!(report.has_errors());

        let escape_errors = report
            .findings
            .iter()
            .filter(|f| f.rule_id == "path-escape")
            .collect::<Vec<_>>();
        assert!(!escape_errors.is_empty());
    }

    #[test]
    fn test_missing_skill_md() {
        let temp_dir = TempDir::new().unwrap();
        // Don't create SKILL.md

        let report = validate_skill(temp_dir.path()).unwrap();
        assert!(report.has_errors());

        let bundle_errors = report
            .findings
            .iter()
            .filter(|f| f.rule_id == "bundle-integrity")
            .collect::<Vec<_>>();
        assert!(!bundle_errors.is_empty());
    }
}
