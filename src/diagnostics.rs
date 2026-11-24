//! Shared diagnostic model for validation and security auditing.
//!
//! This module provides a versioned, deterministic diagnostic schema used by
//! both skill quality validation (#695) and static security audit (#688).
//! All diagnostics are offline, read-only, and never execute code.

use serde::Serialize;
use std::fmt;
use std::path::{Path, PathBuf};

/// Schema version for the diagnostic JSON contract.
pub const DIAGNOSTICS_SCHEMA_VERSION: u32 = 1;

/// Severity levels for diagnostics, ordered by priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// Informational finding, no action required.
    Info,
    /// Potential issue that should be reviewed.
    Warning,
    /// Error that prevents successful validation or indicates a problem.
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A single diagnostic finding from validation or security analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    /// Stable rule identifier (e.g., "manifest-missing-field", "credential-literal").
    pub rule_id: String,
    /// Severity of this finding.
    pub severity: Severity,
    /// Absolute or relative path to the source file that triggered this finding.
    pub source_path: String,
    /// Optional byte/line/column span for precise location.
    pub span: Option<Span>,
    /// Human-readable description of the finding.
    pub message: String,
    /// Safe evidence summary — never contains secrets or sensitive data.
    pub evidence: Option<String>,
    /// Stable fingerprint for deduplication across runs.
    pub fingerprint: String,
}

/// Source location span for precise error reporting.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize)]
pub struct Span {
    /// Line number (1-based).
    pub line: u32,
    /// Column number (1-based), optional.
    pub column: Option<u32>,
    /// Byte offset, optional.
    pub byte_offset: Option<usize>,
}

impl Span {
    pub fn line(line: u32) -> Self {
        Self {
            line,
            column: None,
            byte_offset: None,
        }
    }

    pub fn line_column(line: u32, column: u32) -> Self {
        Self {
            line,
            column: Some(column),
            byte_offset: None,
        }
    }
}

/// A modular validation rule that can be applied to files or directories.
pub trait ValidationRule: Send + Sync {
    /// Returns the stable rule identifier.
    fn rule_id(&self) -> &str;

    /// Returns the default severity for findings from this rule.
    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    /// Check a single file and return any diagnostics.
    fn check_file(&self, path: &Path, content: &[u8]) -> Vec<Diagnostic>;

    /// Check a directory and return any diagnostics.
    /// Default implementation checks each file recursively.
    fn check_directory(&self, _path: &Path) -> Vec<Diagnostic> {
        vec![]
    }
}

/// Bounded file traversal to prevent pathological inputs.
pub struct BoundedTraversal {
    /// Maximum recursion depth (default: 10).
    max_depth: usize,
    /// Maximum total content size in bytes (default: 10MB).
    max_content_size: usize,
    /// Maximum number of files to process (default: 1000).
    max_files: usize,
}

impl BoundedTraversal {
    pub fn new() -> Self {
        Self {
            max_depth: 10,
            max_content_size: 10 * 1024 * 1024, // 10MB
            max_files: 1000,
        }
    }

    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn max_content_size(mut self, size: usize) -> Self {
        self.max_content_size = size;
        self
    }

    pub fn max_files(mut self, count: usize) -> Self {
        self.max_files = count;
        self
    }

    /// Traverse a directory and yield file paths with their content, respecting bounds.
    pub fn traverse(
        &self,
        root: &Path,
        mut callback: impl FnMut(&Path, &[u8]) -> bool,
    ) -> Result<(), TraversalError> {
        let mut total_size = 0usize;
        let mut file_count = 0usize;

        Self::walk_dir(
            root,
            0,
            self,
            &mut total_size,
            &mut file_count,
            &mut callback,
        )?;

        Ok(())
    }

    fn walk_dir(
        path: &Path,
        depth: usize,
        bounds: &Self,
        total_size: &mut usize,
        file_count: &mut usize,
        callback: &mut impl FnMut(&Path, &[u8]) -> bool,
    ) -> Result<(), TraversalError> {
        if depth > bounds.max_depth {
            return Err(TraversalError::MaxDepthExceeded);
        }

        let metadata = path
            .metadata()
            .map_err(|_| TraversalError::PermissionDenied(path.to_path_buf()))?;

        if metadata.is_dir() {
            let entries = path
                .read_dir()
                .map_err(|_| TraversalError::PermissionDenied(path.to_path_buf()))?;

            for entry in entries {
                let entry = entry.map_err(|_| TraversalError::IoError)?;
                let child_path = entry.path();

                // Skip symlinks to prevent escape
                if child_path.is_symlink() {
                    continue;
                }

                Self::walk_dir(
                    &child_path,
                    depth + 1,
                    bounds,
                    total_size,
                    file_count,
                    callback,
                )?;
            }
        } else if metadata.is_file() {
            *file_count += 1;
            if *file_count > bounds.max_files {
                return Err(TraversalError::MaxFilesExceeded);
            }

            let file_size = metadata.len() as usize;
            if *total_size + file_size > bounds.max_content_size {
                return Err(TraversalError::MaxContentSizeExceeded);
            }

            let content = std::fs::read(path)
                .map_err(|_| TraversalError::PermissionDenied(path.to_path_buf()))?;
            *total_size += content.len();

            if !callback(path, &content) {
                return Ok(()); // Callback requested early termination
            }
        }

        Ok(())
    }
}

impl Default for BoundedTraversal {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors from bounded file traversal.
#[derive(Debug)]
pub enum TraversalError {
    MaxDepthExceeded,
    MaxFilesExceeded,
    MaxContentSizeExceeded,
    PermissionDenied(PathBuf),
    IoError,
}

impl fmt::Display for TraversalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaxDepthExceeded => write!(f, "maximum traversal depth exceeded"),
            Self::MaxFilesExceeded => write!(f, "maximum file count exceeded"),
            Self::MaxContentSizeExceeded => write!(f, "maximum content size exceeded"),
            Self::PermissionDenied(path) => write!(f, "permission denied: {}", path.display()),
            Self::IoError => write!(f, "I/O error during traversal"),
        }
    }
}

impl std::error::Error for TraversalError {}

/// Collection of diagnostics with metadata.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticReport {
    /// Schema version.
    pub schema_version: u32,
    /// List of all findings.
    pub findings: Vec<Diagnostic>,
    /// Summary counts by severity.
    pub summary: SeveritySummary,
}

/// Summary counts by severity level.
#[derive(Debug, Clone, Serialize)]
pub struct SeveritySummary {
    pub info: usize,
    pub warning: usize,
    pub error: usize,
}

impl DiagnosticReport {
    pub fn new(findings: Vec<Diagnostic>) -> Self {
        let mut summary = SeveritySummary {
            info: 0,
            warning: 0,
            error: 0,
        };

        for finding in &findings {
            match finding.severity {
                Severity::Info => summary.info += 1,
                Severity::Warning => summary.warning += 1,
                Severity::Error => summary.error += 1,
            }
        }

        Self {
            schema_version: DIAGNOSTICS_SCHEMA_VERSION,
            findings,
            summary,
        }
    }

    /// Returns true if there are any error-severity findings.
    pub fn has_errors(&self) -> bool {
        self.summary.error > 0
    }

    /// Returns true if there are any warning or error findings.
    pub fn has_warnings(&self) -> bool {
        self.summary.warning > 0 || self.summary.error > 0
    }

    /// Filter findings by minimum severity.
    pub fn filter_by_severity(&self, min_severity: Severity) -> Vec<&Diagnostic> {
        self.findings
            .iter()
            .filter(|f| f.severity >= min_severity)
            .collect()
    }
}

/// Generate a stable fingerprint for a diagnostic.
pub fn fingerprint(rule_id: &str, source_path: &str, span: &Option<Span>) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    rule_id.hash(&mut hasher);
    source_path.hash(&mut hasher);
    span.hash(&mut hasher);
    let hash = hasher.finish();
    format!("{:016x}", hash)
}

/// Redact sensitive values from evidence strings.
pub fn redact_evidence(evidence: &str) -> String {
    use regex::Regex;
    // Redact common secret patterns by replacing the value after the marker
    let sk_pattern = Regex::new(r"sk-\S+").unwrap();
    let bearer_pattern = Regex::new(r"Bearer\s+\S+").unwrap();
    let token_pattern = Regex::new(r"token=\S+").unwrap();

    let redacted = sk_pattern.replace_all(evidence, "sk-<redacted>");
    let redacted = bearer_pattern.replace_all(&redacted, "Bearer <redacted>");
    let redacted = token_pattern.replace_all(&redacted, "token=<redacted>");
    redacted.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }

    #[test]
    fn test_fingerprint_stability() {
        let fp1 = fingerprint("rule-1", "/path/to/file", &Some(Span::line(10)));
        let fp2 = fingerprint("rule-1", "/path/to/file", &Some(Span::line(10)));
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_uniqueness() {
        let fp1 = fingerprint("rule-1", "/path/to/file", &Some(Span::line(10)));
        let fp2 = fingerprint("rule-2", "/path/to/file", &Some(Span::line(10)));
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_diagnostic_report_summary() {
        let findings = vec![
            Diagnostic {
                rule_id: "test-1".into(),
                severity: Severity::Info,
                source_path: "/test/1".into(),
                span: None,
                message: "info".into(),
                evidence: None,
                fingerprint: "1".into(),
            },
            Diagnostic {
                rule_id: "test-2".into(),
                severity: Severity::Warning,
                source_path: "/test/2".into(),
                span: None,
                message: "warning".into(),
                evidence: None,
                fingerprint: "2".into(),
            },
            Diagnostic {
                rule_id: "test-3".into(),
                severity: Severity::Error,
                source_path: "/test/3".into(),
                span: None,
                message: "error".into(),
                evidence: None,
                fingerprint: "3".into(),
            },
        ];

        let report = DiagnosticReport::new(findings);
        assert_eq!(report.summary.info, 1);
        assert_eq!(report.summary.warning, 1);
        assert_eq!(report.summary.error, 1);
        assert!(report.has_errors());
        assert!(report.has_warnings());
    }

    #[test]
    fn test_redact_evidence() {
        assert_eq!(redact_evidence("key=sk-abc123"), "key=sk-<redacted>");
        assert_eq!(
            redact_evidence("Authorization: Bearer secret"),
            "Authorization: Bearer <redacted>"
        );
    }
}
