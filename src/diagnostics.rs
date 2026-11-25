//! Shared diagnostic model for validation and security auditing.
//!
//! This module provides a versioned, deterministic diagnostic schema used by
//! both skill quality validation (#695) and static security audit (#688).
//! All diagnostics are offline, read-only, and never execute code.

use serde::Serialize;
use std::fmt;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::ambient_authority;
use cap_std::fs::{Dir as CapabilityDir, OpenOptions as CapabilityOpenOptions};

/// Schema version for the diagnostic JSON contract.
pub const DIAGNOSTICS_SCHEMA_VERSION: u32 = 1;
/// Schema version for diagnostic reports that include bounded scan coverage.
pub const SCAN_REPORT_SCHEMA_VERSION: u32 = 2;

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
    /// Stable identity of the audited target, omitted from legacy diagnostics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
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
pub const DEFAULT_MAX_TRAVERSAL_ENTRIES: usize = 10_000;

pub struct BoundedTraversal {
    /// Maximum recursion depth (default: 10).
    max_depth: usize,
    /// Maximum total content size in bytes (default: 10MB).
    max_content_size: usize,
    /// Maximum size of one file in bytes (default: 10MB).
    max_file_size: usize,
    /// Maximum number of files to process (default: 1000).
    max_files: usize,
    /// Maximum total directory entries to enumerate (default: 10,000).
    max_entries: usize,
}

struct TraversalState<'a> {
    root: &'a Path,
    bounds: &'a BoundedTraversal,
    file_count: usize,
    bytes_consumed: u64,
    entry_count: usize,
    stopped: bool,
    report: TraversalReport,
}

impl BoundedTraversal {
    pub fn new() -> Self {
        Self {
            max_depth: 10,
            max_content_size: 10 * 1024 * 1024, // 10MB
            max_file_size: 10 * 1024 * 1024,    // 10MB
            max_files: 1000,
            max_entries: DEFAULT_MAX_TRAVERSAL_ENTRIES,
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

    pub fn max_file_size(mut self, size: usize) -> Self {
        self.max_file_size = size;
        self
    }

    pub fn max_files(mut self, count: usize) -> Self {
        self.max_files = count;
        self
    }

    pub fn max_entries(mut self, count: usize) -> Self {
        self.max_entries = count;
        self
    }

    /// Traverse a directory and yield file paths with their content, respecting bounds.
    pub fn traverse(
        &self,
        root: &Path,
        callback: impl FnMut(&Path, &[u8]) -> bool,
    ) -> Result<(), TraversalError> {
        let report = self.traverse_with_report(root, callback);
        match report.errors.first() {
            Some(error) => Err(match error.code {
                ScanErrorCode::InvalidTarget
                | ScanErrorCode::TargetNotFound
                | ScanErrorCode::PluginTrustUnavailable
                | ScanErrorCode::RootReadFailed => TraversalError::PermissionDenied(
                    error
                        .source_path
                        .as_deref()
                        .map(PathBuf::from)
                        .unwrap_or_default(),
                ),
                ScanErrorCode::MaxDepthExceeded => TraversalError::MaxDepthExceeded,
                ScanErrorCode::MaxFilesExceeded => TraversalError::MaxFilesExceeded,
                ScanErrorCode::MaxEntriesExceeded => TraversalError::MaxEntriesExceeded,
                ScanErrorCode::MaxContentSizeExceeded | ScanErrorCode::MaxFileSizeExceeded => {
                    TraversalError::MaxContentSizeExceeded
                }
                ScanErrorCode::SymlinkSkipped => TraversalError::SymlinkEncountered,
                ScanErrorCode::DirectoryReadFailed | ScanErrorCode::FileReadFailed => {
                    TraversalError::PermissionDenied(
                        error
                            .source_path
                            .as_deref()
                            .map(PathBuf::from)
                            .unwrap_or_default(),
                    )
                }
                ScanErrorCode::UnsupportedEntry
                | ScanErrorCode::UnsupportedContent
                | ScanErrorCode::CallbackStopped => TraversalError::IoError,
            }),
            None => Ok(()),
        }
    }

    /// Traverse a directory and return explicit coverage and safe scan errors.
    pub fn traverse_with_report(
        &self,
        root: &Path,
        mut callback: impl FnMut(&Path, &[u8]) -> bool,
    ) -> TraversalReport {
        let mut initial_report = TraversalReport::default();
        let root_metadata = match std::fs::symlink_metadata(root) {
            Ok(metadata) => metadata,
            Err(_) => {
                initial_report.errors.push(ScanError::new(
                    ScanErrorCode::RootReadFailed,
                    relative_source_path(root, root),
                ));
                return finalize_traversal_report(initial_report);
            }
        };
        if root_metadata.file_type().is_symlink() {
            initial_report.errors.push(ScanError::new(
                ScanErrorCode::SymlinkSkipped,
                relative_source_path(root, root),
            ));
            return finalize_traversal_report(initial_report);
        }
        if !root_metadata.is_dir() {
            initial_report.errors.push(ScanError::new(
                ScanErrorCode::RootReadFailed,
                relative_source_path(root, root),
            ));
            return finalize_traversal_report(initial_report);
        }
        let root_directory = match open_root_directory(root) {
            Ok(directory) => directory,
            Err(_) => {
                initial_report.errors.push(ScanError::new(
                    ScanErrorCode::RootReadFailed,
                    relative_source_path(root, root),
                ));
                return finalize_traversal_report(initial_report);
            }
        };
        let mut state = TraversalState {
            root,
            bounds: self,
            file_count: 0,
            bytes_consumed: 0,
            entry_count: 0,
            stopped: false,
            report: TraversalReport::default(),
        };

        Self::walk_dir(&root_directory, root, 0, &mut state, &mut callback);

        finalize_traversal_report(state.report)
    }

    fn walk_dir(
        directory: &CapabilityDir,
        path: &Path,
        depth: usize,
        state: &mut TraversalState<'_>,
        callback: &mut impl FnMut(&Path, &[u8]) -> bool,
    ) {
        if state.stopped {
            return;
        }

        if depth > state.bounds.max_depth {
            state.report.errors.push(ScanError::new(
                ScanErrorCode::MaxDepthExceeded,
                relative_source_path(state.root, path),
            ));
            return;
        }

        let metadata = match directory.metadata(".") {
            Ok(metadata) => metadata,
            Err(_) => {
                state.report.errors.push(ScanError::new(
                    if path == state.root {
                        ScanErrorCode::RootReadFailed
                    } else {
                        ScanErrorCode::FileReadFailed
                    },
                    relative_source_path(state.root, path),
                ));
                return;
            }
        };

        if metadata.is_dir() {
            let entries = match directory.entries() {
                Ok(entries) => entries,
                Err(_) => {
                    state.report.errors.push(ScanError::new(
                        ScanErrorCode::DirectoryReadFailed,
                        relative_source_path(state.root, path),
                    ));
                    return;
                }
            };
            let mut children = Vec::new();
            for entry in entries {
                if state.entry_count >= state.bounds.max_entries {
                    state.report.errors.push(ScanError::new(
                        ScanErrorCode::MaxEntriesExceeded,
                        relative_source_path(state.root, path),
                    ));
                    state.stopped = true;
                    return;
                }
                state.entry_count += 1;
                match entry {
                    Ok(entry) => children.push(entry),
                    Err(_) => state.report.errors.push(ScanError::new(
                        ScanErrorCode::DirectoryReadFailed,
                        relative_source_path(state.root, path),
                    )),
                }
            }
            children.sort_by_key(|entry| entry.file_name());

            for entry in children {
                let name = entry.file_name();
                let child_path = path.join(&name);
                let child_metadata = match directory.symlink_metadata(&name) {
                    Ok(metadata) => metadata,
                    Err(_) => {
                        state.report.errors.push(ScanError::new(
                            ScanErrorCode::FileReadFailed,
                            relative_source_path(state.root, &child_path),
                        ));
                        continue;
                    }
                };
                if child_metadata.file_type().is_symlink() {
                    state.report.coverage.files_discovered += 1;
                    state.report.coverage.files_skipped += 1;
                    state.report.errors.push(ScanError::new(
                        ScanErrorCode::SymlinkSkipped,
                        relative_source_path(state.root, &child_path),
                    ));
                    continue;
                }
                if child_metadata.is_dir() {
                    match directory.open_dir_nofollow(&name) {
                        Ok(child_directory) => {
                            Self::walk_dir(
                                &child_directory,
                                &child_path,
                                depth + 1,
                                state,
                                callback,
                            );
                        }
                        Err(_) => state.report.errors.push(ScanError::new(
                            if directory
                                .symlink_metadata(&name)
                                .is_ok_and(|m| m.file_type().is_symlink())
                            {
                                ScanErrorCode::SymlinkSkipped
                            } else {
                                ScanErrorCode::DirectoryReadFailed
                            },
                            relative_source_path(state.root, &child_path),
                        )),
                    }
                } else if child_metadata.is_file() {
                    if depth >= state.bounds.max_depth {
                        state.report.coverage.files_discovered += 1;
                        state.report.coverage.files_skipped += 1;
                        state.report.errors.push(ScanError::new(
                            ScanErrorCode::MaxDepthExceeded,
                            relative_source_path(state.root, &child_path),
                        ));
                        continue;
                    }
                    Self::scan_file(
                        directory,
                        &name,
                        &child_path,
                        child_metadata,
                        state,
                        callback,
                    );
                } else {
                    state.report.coverage.files_discovered += 1;
                    state.report.coverage.files_skipped += 1;
                    state.report.errors.push(ScanError::new(
                        ScanErrorCode::UnsupportedEntry,
                        relative_source_path(state.root, &child_path),
                    ));
                }
                if state.stopped {
                    break;
                }
            }
        } else {
            state.report.errors.push(ScanError::new(
                ScanErrorCode::UnsupportedEntry,
                relative_source_path(state.root, path),
            ));
        }
    }

    fn scan_file(
        directory: &CapabilityDir,
        name: &std::ffi::OsStr,
        path: &Path,
        metadata: cap_std::fs::Metadata,
        state: &mut TraversalState<'_>,
        callback: &mut impl FnMut(&Path, &[u8]) -> bool,
    ) {
        state.report.coverage.files_discovered += 1;
        if state.file_count >= state.bounds.max_files {
            state.report.coverage.files_skipped += 1;
            state.report.errors.push(ScanError::new(
                ScanErrorCode::MaxFilesExceeded,
                relative_source_path(state.root, path),
            ));
            state.stopped = true;
            return;
        }
        state.file_count += 1;

        let remaining_size =
            (state.bounds.max_content_size as u64).saturating_sub(state.bytes_consumed);
        if metadata.len() > state.bounds.max_file_size as u64 {
            state.report.coverage.files_skipped += 1;
            state.report.errors.push(ScanError::new(
                ScanErrorCode::MaxFileSizeExceeded,
                relative_source_path(state.root, path),
            ));
            return;
        }
        if metadata.len() > remaining_size {
            state.report.coverage.files_skipped += 1;
            state.report.errors.push(ScanError::new(
                ScanErrorCode::MaxContentSizeExceeded,
                relative_source_path(state.root, path),
            ));
            return;
        }

        let read_limit = (state.bounds.max_file_size as u64).min(remaining_size);
        let file = match open_entry_nofollow(directory, name) {
            Ok(file) => file,
            Err(_) => {
                state.report.coverage.files_skipped += 1;
                state.report.errors.push(ScanError::new(
                    ScanErrorCode::FileReadFailed,
                    relative_source_path(state.root, path),
                ));
                return;
            }
        };
        let file_metadata = match file.metadata() {
            Ok(metadata) if metadata.is_file() => metadata,
            _ => {
                state.report.coverage.files_skipped += 1;
                state.report.errors.push(ScanError::new(
                    ScanErrorCode::FileReadFailed,
                    relative_source_path(state.root, path),
                ));
                return;
            }
        };
        let content = match read_bounded(file, read_limit, &mut state.bytes_consumed) {
            Ok(content) => content,
            Err(_) => {
                state.report.coverage.files_skipped += 1;
                state.report.errors.push(ScanError::new(
                    ScanErrorCode::FileReadFailed,
                    relative_source_path(state.root, path),
                ));
                return;
            }
        };
        let current_metadata = directory.symlink_metadata(name);
        let file_changed = current_metadata
            .map(|current| {
                current.file_type().is_symlink()
                    || current.len() != metadata.len()
                    || current.len() != file_metadata.len()
            })
            .unwrap_or(true);
        if file_changed || content.len() as u64 != file_metadata.len() {
            state.report.coverage.files_skipped += 1;
            state.report.errors.push(ScanError::new(
                ScanErrorCode::FileReadFailed,
                relative_source_path(state.root, path),
            ));
            return;
        }

        state.report.coverage.files_scanned += 1;
        state.report.coverage.bytes_scanned += content.len() as u64;
        if !callback(path, &content) {
            state.report.errors.push(ScanError::new(
                ScanErrorCode::CallbackStopped,
                relative_source_path(state.root, path),
            ));
            state.stopped = true;
        }
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
    MaxEntriesExceeded,
    MaxContentSizeExceeded,
    SymlinkEncountered,
    PermissionDenied(PathBuf),
    IoError,
}

impl fmt::Display for TraversalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaxDepthExceeded => write!(f, "maximum traversal depth exceeded"),
            Self::MaxFilesExceeded => write!(f, "maximum file count exceeded"),
            Self::MaxEntriesExceeded => write!(f, "maximum directory entry count exceeded"),
            Self::MaxContentSizeExceeded => write!(f, "maximum content size exceeded"),
            Self::SymlinkEncountered => write!(f, "symbolic link encountered during traversal"),
            Self::PermissionDenied(_) => write!(f, "permission denied during traversal"),
            Self::IoError => write!(f, "I/O error during traversal"),
        }
    }
}

impl std::error::Error for TraversalError {}

/// Whether a bounded traversal inspected all content required by its target.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanStatus {
    #[default]
    Complete,
    Incomplete,
}

/// Stable categories for content that could not be scanned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanErrorCode {
    InvalidTarget,
    TargetNotFound,
    PluginTrustUnavailable,
    RootReadFailed,
    DirectoryReadFailed,
    FileReadFailed,
    MaxDepthExceeded,
    MaxFilesExceeded,
    MaxEntriesExceeded,
    MaxFileSizeExceeded,
    MaxContentSizeExceeded,
    SymlinkSkipped,
    UnsupportedEntry,
    UnsupportedContent,
    CallbackStopped,
}

impl ScanErrorCode {
    pub fn message(self) -> &'static str {
        match self {
            Self::InvalidTarget => "audit target identifier is invalid",
            Self::TargetNotFound => "requested audit target does not exist",
            Self::PluginTrustUnavailable => "plugin trust state could not be verified",
            Self::RootReadFailed => "scan root could not be read",
            Self::DirectoryReadFailed => "directory contents could not be read",
            Self::FileReadFailed => "file could not be read",
            Self::MaxDepthExceeded => "maximum traversal depth reached",
            Self::MaxFilesExceeded => "maximum file count reached",
            Self::MaxEntriesExceeded => "maximum directory entry count reached",
            Self::MaxFileSizeExceeded => "maximum file size exceeded",
            Self::MaxContentSizeExceeded => "maximum aggregate content size exceeded",
            Self::SymlinkSkipped => "symbolic link was not followed",
            Self::UnsupportedEntry => "unsupported file type skipped",
            Self::UnsupportedContent => "file content could not be inspected by enabled rules",
            Self::CallbackStopped => "traversal stopped by callback",
        }
    }
}

/// A safe traversal issue; paths are relative to the selected root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanError {
    pub code: ScanErrorCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub source_path: Option<String>,
}

impl ScanError {
    pub fn new(code: ScanErrorCode, source_path: Option<String>) -> Self {
        Self {
            code,
            target: None,
            source_path,
        }
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }
}

/// File-level counts returned by a bounded traversal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TraversalCoverage {
    pub files_discovered: usize,
    pub files_scanned: usize,
    pub files_skipped: usize,
    pub bytes_scanned: u64,
}

/// Target-level and file-level coverage for a complete audit invocation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ScanCoverage {
    pub targets_discovered: usize,
    pub targets_scanned: usize,
    pub files_discovered: usize,
    pub files_scanned: usize,
    pub files_skipped: usize,
    pub bytes_scanned: u64,
}

/// Coverage and safe errors from one bounded traversal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TraversalReport {
    pub status: ScanStatus,
    pub coverage: TraversalCoverage,
    pub errors: Vec<ScanError>,
}

pub(crate) fn open_entry_nofollow(
    directory: &cap_std::fs::Dir,
    name: &std::ffi::OsStr,
) -> std::io::Result<cap_std::fs::File> {
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    directory.open_with(name, &options)
}

pub(crate) fn open_root_directory(root: &Path) -> io::Result<CapabilityDir> {
    let absolute = if root.is_absolute() {
        root.to_owned()
    } else {
        std::env::current_dir()?.join(root)
    };
    open_absolute_directory_nofollow(&absolute)
}

#[cfg(unix)]
fn open_absolute_directory_nofollow(path: &Path) -> io::Result<CapabilityDir> {
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "directory path is not absolute",
        ));
    }
    walk_absolute_components(
        CapabilityDir::open_ambient_dir(Path::new("/"), ambient_authority())?,
        components,
    )
}

#[cfg(windows)]
fn open_absolute_directory_nofollow(path: &Path) -> io::Result<CapabilityDir> {
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "directory path has no volume prefix",
        ));
    };
    let mut volume_root = PathBuf::from(prefix.as_os_str());
    volume_root.push("\\");
    walk_absolute_components(
        CapabilityDir::open_ambient_dir(&volume_root, ambient_authority())?,
        components,
    )
}

#[cfg(any(unix, windows))]
fn walk_absolute_components(
    mut current: CapabilityDir,
    components: std::path::Components<'_>,
) -> io::Result<CapabilityDir> {
    let mut ancestors = Vec::new();
    for component in components {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => {
                let child = current.open_dir_nofollow(name)?;
                ancestors.push(current);
                current = child;
            }
            Component::ParentDir => {
                if let Some(parent) = ancestors.pop() {
                    current = parent;
                }
            }
            Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unexpected volume prefix",
                ));
            }
        }
    }
    Ok(current)
}

#[cfg(not(any(unix, windows)))]
fn open_absolute_directory_nofollow(_path: &Path) -> io::Result<CapabilityDir> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "no verified directory anchor on this platform",
    ))
}

fn finalize_traversal_report(mut report: TraversalReport) -> TraversalReport {
    report.errors.sort_by(|left, right| {
        left.source_path
            .cmp(&right.source_path)
            .then_with(|| left.code.cmp(&right.code))
    });
    if !report.errors.is_empty() {
        report.status = ScanStatus::Incomplete;
    }
    report
}

fn relative_source_path(root: &Path, path: &Path) -> Option<String> {
    let relative_path = path.strip_prefix(root).ok()?;
    let components = relative_path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    (!components.is_empty()).then(|| components.join("/"))
}

fn read_bounded(reader: impl Read, max_bytes: u64, consumed: &mut u64) -> std::io::Result<Vec<u8>> {
    let mut content = Vec::new();
    let mut limited = reader.take(max_bytes);
    let result = limited.read_to_end(&mut content);
    *consumed += max_bytes - limited.limit();
    result?;
    Ok(content)
}

/// Collection of diagnostics with metadata.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticReport {
    /// Schema version.
    pub schema_version: u32,
    /// List of all findings.
    pub findings: Vec<Diagnostic>,
    /// Summary counts by severity.
    pub summary: SeveritySummary,
    /// Completion state for reports that performed bounded scanning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ScanStatus>,
    /// Scan coverage, omitted for legacy validation reports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<ScanCoverage>,
    /// Stable, controlled reasons for content that was not fully inspected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scan_errors: Option<Vec<ScanError>>,
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
            status: None,
            coverage: None,
            scan_errors: None,
        }
    }

    /// Construct a versioned report with bounded scan coverage.
    pub fn with_scan(
        mut findings: Vec<Diagnostic>,
        status: ScanStatus,
        coverage: ScanCoverage,
        scan_errors: Vec<ScanError>,
    ) -> Self {
        findings.sort_by(|left, right| {
            left.target
                .cmp(&right.target)
                .then_with(|| left.source_path.cmp(&right.source_path))
                .then_with(|| {
                    left.span
                        .as_ref()
                        .map(|span| span.line)
                        .cmp(&right.span.as_ref().map(|span| span.line))
                })
                .then_with(|| left.rule_id.cmp(&right.rule_id))
                .then_with(|| left.fingerprint.cmp(&right.fingerprint))
        });
        findings.dedup_by(|left, right| left.fingerprint == right.fingerprint);
        let mut report = Self::new(findings);
        report.schema_version = SCAN_REPORT_SCHEMA_VERSION;
        report.status = Some(status);
        report.coverage = Some(coverage);
        report.scan_errors = Some(scan_errors);
        report
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
    use std::fs;
    use std::io::Cursor;
    use tempfile::TempDir;

    #[cfg(unix)]
    #[test]
    fn file_open_rejects_a_symlink_before_reading_its_target() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("outside.txt"), b"outside data").unwrap();
        symlink(
            temp_dir.path().join("outside.txt"),
            temp_dir.path().join("link.txt"),
        )
        .unwrap();
        let directory =
            cap_std::fs::Dir::open_ambient_dir(temp_dir.path(), cap_std::ambient_authority())
                .unwrap();

        assert!(open_entry_nofollow(&directory, std::ffi::OsStr::new("link.txt")).is_err());
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }

    #[cfg(unix)]
    #[test]
    fn traversal_does_not_follow_a_symlink_root() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("secret.txt"), "must not be read").unwrap();
        let link = temp_dir.path().join("root-link");
        symlink(&target, &link).unwrap();

        let mut visited = Vec::new();
        let result = BoundedTraversal::new().traverse(&link, |path, _| {
            visited.push(path.to_path_buf());
            true
        });

        assert!(result.is_err(), "a symlink root must be reported");
        assert!(
            visited.is_empty(),
            "the symlink destination must not be read"
        );
    }

    #[test]
    fn traversal_report_counts_and_sorts_files() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("z.txt"), "last").unwrap();
        fs::write(temp_dir.path().join("a.txt"), "first").unwrap();

        let mut visited = Vec::new();
        let report = BoundedTraversal::new().traverse_with_report(temp_dir.path(), |path, _| {
            visited.push(path.file_name().unwrap().to_string_lossy().into_owned());
            true
        });

        assert_eq!(visited, vec!["a.txt", "z.txt"]);
        assert_eq!(report.status, ScanStatus::Complete);
        assert_eq!(report.coverage.files_discovered, 2);
        assert_eq!(report.coverage.files_scanned, 2);
        assert_eq!(report.coverage.files_skipped, 0);
        assert_eq!(report.coverage.bytes_scanned, 9);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn traversal_report_skips_files_over_the_per_file_limit() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("a.txt"), "large").unwrap();
        fs::write(temp_dir.path().join("b.txt"), "ok").unwrap();

        let mut visited = Vec::new();
        let report = BoundedTraversal::new()
            .max_file_size(3)
            .traverse_with_report(temp_dir.path(), |path, _| {
                visited.push(path.file_name().unwrap().to_string_lossy().into_owned());
                true
            });

        assert_eq!(visited, vec!["b.txt"]);
        assert_eq!(report.status, ScanStatus::Incomplete);
        assert_eq!(report.coverage.files_discovered, 2);
        assert_eq!(report.coverage.files_scanned, 1);
        assert_eq!(report.coverage.files_skipped, 1);
        assert_eq!(report.coverage.bytes_scanned, 2);
        assert_eq!(report.errors[0].code, ScanErrorCode::MaxFileSizeExceeded);
        assert_eq!(report.errors[0].source_path.as_deref(), Some("a.txt"));
        assert!(!format!("{:?}", report.errors).contains(&temp_dir.path().display().to_string()));
    }

    #[test]
    fn traversal_report_enforces_aggregate_size_limit() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("a.txt"), "12").unwrap();
        fs::write(temp_dir.path().join("b.txt"), "345").unwrap();

        let mut visited = Vec::new();
        let report = BoundedTraversal::new()
            .max_content_size(4)
            .traverse_with_report(temp_dir.path(), |path, _| {
                visited.push(path.file_name().unwrap().to_string_lossy().into_owned());
                true
            });

        assert_eq!(visited, vec!["a.txt"]);
        assert_eq!(report.status, ScanStatus::Incomplete);
        assert_eq!(report.coverage.files_discovered, 2);
        assert_eq!(report.coverage.files_scanned, 1);
        assert_eq!(report.coverage.files_skipped, 1);
        assert_eq!(report.coverage.bytes_scanned, 2);
        assert_eq!(report.errors[0].code, ScanErrorCode::MaxContentSizeExceeded);
        assert_eq!(report.errors[0].source_path.as_deref(), Some("b.txt"));
    }

    #[test]
    fn traversal_report_enforces_file_count_and_depth_limits() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("a.txt"), "a").unwrap();
        fs::write(temp_dir.path().join("b.txt"), "b").unwrap();

        let mut visited = Vec::new();
        let report = BoundedTraversal::new().max_files(1).traverse_with_report(
            temp_dir.path(),
            |path, _| {
                visited.push(path.file_name().unwrap().to_string_lossy().into_owned());
                true
            },
        );

        assert_eq!(visited, vec!["a.txt"]);
        assert_eq!(report.status, ScanStatus::Incomplete);
        assert_eq!(report.coverage.files_discovered, 2);
        assert_eq!(report.coverage.files_scanned, 1);
        assert_eq!(report.coverage.files_skipped, 1);
        assert_eq!(report.errors[0].code, ScanErrorCode::MaxFilesExceeded);

        let nested = temp_dir.path().join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("deep.txt"), "deep").unwrap();
        let depth_report = BoundedTraversal::new()
            .max_depth(0)
            .traverse_with_report(temp_dir.path(), |_, _| true);

        assert_eq!(depth_report.status, ScanStatus::Incomplete);
        assert!(
            depth_report
                .errors
                .iter()
                .any(|error| error.code == ScanErrorCode::MaxDepthExceeded)
        );
    }

    #[test]
    fn traversal_report_caps_directory_entry_enumeration() {
        let temp_dir = TempDir::new().unwrap();
        for index in 0..10_001 {
            fs::write(temp_dir.path().join(format!("entry-{index:05}.txt")), "").unwrap();
        }

        let report = BoundedTraversal::new().traverse_with_report(temp_dir.path(), |_, _| true);

        assert_eq!(report.status, ScanStatus::Incomplete);
        assert!(
            report
                .errors
                .iter()
                .any(|error| { format!("{:?}", error.code) == "MaxEntriesExceeded" })
        );
    }

    #[test]
    fn traversal_report_uses_safe_errors_for_unreadable_roots() {
        let temp_dir = TempDir::new().unwrap();
        let missing_root = temp_dir.path().join("missing-secret-path");
        let report = BoundedTraversal::new().traverse_with_report(&missing_root, |_, _| true);

        assert_eq!(report.status, ScanStatus::Incomplete);
        assert_eq!(report.errors[0].code, ScanErrorCode::RootReadFailed);
        assert_eq!(report.errors[0].source_path, None);
        assert!(!format!("{:?}", report.errors).contains(&temp_dir.path().display().to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn traversal_report_skips_nested_symlinks_without_reading_targets() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().join("root");
        let outside = temp_dir.path().join("outside.txt");
        fs::create_dir(&root).unwrap();
        fs::write(&outside, "outside content").unwrap();
        symlink(&outside, root.join("link.txt")).unwrap();

        let mut visited = Vec::new();
        let report = BoundedTraversal::new().traverse_with_report(&root, |path, _| {
            visited.push(path.to_path_buf());
            true
        });

        assert!(visited.is_empty());
        assert_eq!(report.status, ScanStatus::Incomplete);
        assert_eq!(report.coverage.files_discovered, 1);
        assert_eq!(report.coverage.files_skipped, 1);
        assert_eq!(report.errors[0].code, ScanErrorCode::SymlinkSkipped);
        assert_eq!(report.errors[0].source_path.as_deref(), Some("link.txt"));
    }

    #[test]
    fn bounded_read_never_consumes_more_than_its_limit() {
        let mut source = Cursor::new(b"12345".to_vec());

        let mut consumed = 0;
        let content = read_bounded(&mut source, 3, &mut consumed).unwrap();
        assert_eq!(consumed, 3);

        assert_eq!(content, b"123");
        assert_eq!(source.position(), 3);
    }

    #[test]
    fn partial_read_errors_still_consume_budget() {
        struct Fails;
        impl Read for Fails {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("read failed"))
            }
        }
        let mut consumed = 4;
        assert!(read_bounded(Cursor::new(b"abc").chain(Fails), 20, &mut consumed).is_err());
        assert_eq!(consumed, 7);
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
                target: None,
                severity: Severity::Info,
                source_path: "/test/1".into(),
                span: None,
                message: "info".into(),
                evidence: None,
                fingerprint: "1".into(),
            },
            Diagnostic {
                rule_id: "test-2".into(),
                target: None,
                severity: Severity::Warning,
                source_path: "/test/2".into(),
                span: None,
                message: "warning".into(),
                evidence: None,
                fingerprint: "2".into(),
            },
            Diagnostic {
                rule_id: "test-3".into(),
                target: None,
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
