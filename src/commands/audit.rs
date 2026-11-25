//! Static security audit CLI command.
//!
//! Implements `agentsync audit` as a read-only static analyzer that detects
//! credential exposure, risky shell patterns, file traversal issues, and
//! unexpected privilege escalation.

use anyhow::{Context, Result};
use clap::{ArgGroup, Parser, ValueEnum};
use colored::Colorize;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use agentsync::audit::{audit_all, audit_directory_for_target, audit_plugin_selection};
use agentsync::diagnostics::{
    DiagnosticReport, ScanCoverage, ScanError, ScanErrorCode, ScanStatus, Severity,
};

/// Output format for audit results.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum AuditFormat {
    /// Human-readable output with colors.
    #[default]
    Human,
    /// Machine-readable JSON output.
    Json,
}

/// Severity threshold for --fail-on.
#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum FailThreshold {
    /// Fail on any warning or error.
    Warning,
    /// Fail only on errors.
    Error,
}

/// Arguments for the audit command.
#[derive(Parser, Debug)]
#[command(group(ArgGroup::new("target").args(["all", "skill", "plugin"]).multiple(false)))]
pub struct AuditArgs {
    /// Audit all skills in the project.
    #[arg(long, conflicts_with_all = ["skill", "plugin"])]
    all: bool,

    /// Audit a specific skill by ID.
    #[arg(long, conflicts_with = "plugin")]
    skill: Option<String>,

    /// Audit a specific plugin by ID.
    #[arg(long)]
    plugin: Option<String>,

    /// Output format.
    #[arg(long, default_value = "human")]
    format: AuditFormat,

    /// Fail with non-zero exit code if findings reach this severity.
    #[arg(long)]
    fail_on: Option<FailThreshold>,
}

/// An audit result that must be reflected in the process exit status.
#[derive(Debug)]
pub struct AuditExitError {
    code: i32,
    message: &'static str,
}

impl AuditExitError {
    pub fn incomplete() -> Self {
        Self {
            code: 2,
            message: "audit scan incomplete",
        }
    }

    pub fn threshold_reached() -> Self {
        Self {
            code: 1,
            message: "audit findings met the configured threshold",
        }
    }

    pub fn operational() -> Self {
        Self {
            code: 2,
            message: "audit operation failed",
        }
    }

    pub fn exit_code(&self) -> i32 {
        self.code
    }
}

impl fmt::Display for AuditExitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for AuditExitError {}

/// Run the audit command.
pub fn run_audit(args: AuditArgs, project_root: PathBuf) -> Result<()> {
    // Determine what to audit
    let audit_target = if args.all {
        AuditTarget::All
    } else if let Some(skill_id) = &args.skill {
        AuditTarget::Skill(skill_id.clone())
    } else if let Some(plugin_id) = &args.plugin {
        AuditTarget::Plugin(plugin_id.clone())
    } else {
        // Default: audit the entire .agents/ directory
        AuditTarget::All
    };

    let report = match &audit_target {
        AuditTarget::All => audit_all(&project_root),
        AuditTarget::Skill(skill_id) if !valid_selector(skill_id) => {
            incomplete_report(ScanErrorCode::InvalidTarget, None)
        }
        AuditTarget::Skill(skill_id) => audit_skill(&project_root, skill_id),
        AuditTarget::Plugin(plugin_id) if !valid_plugin_selector(plugin_id) => {
            incomplete_report(ScanErrorCode::InvalidTarget, None)
        }
        AuditTarget::Plugin(plugin_id) => audit_plugin_selection(&project_root, plugin_id),
    };

    print_report(&report, args.format).map_err(|_| AuditExitError::operational())?;

    if report.status == Some(ScanStatus::Incomplete) {
        return Err(AuditExitError::incomplete().into());
    }

    let threshold_reached = match args.fail_on {
        Some(FailThreshold::Warning) => report.summary.warning > 0 || report.summary.error > 0,
        Some(FailThreshold::Error) => report.summary.error > 0,
        None => false,
    };
    if threshold_reached {
        return Err(AuditExitError::threshold_reached().into());
    }

    Ok(())
}

enum AuditTarget {
    All,
    Skill(String),
    Plugin(String),
}

/// Print the audit report in the specified format.
fn print_report(report: &DiagnosticReport, format: AuditFormat) -> Result<()> {
    match format {
        AuditFormat::Human => print_human_report(report),
        AuditFormat::Json => print_json_report(report),
    }
}

/// Print a human-readable report.
fn print_human_report(report: &DiagnosticReport) -> Result<()> {
    println!("\n{}", "=== Security Audit Report ===".bold());
    if let Some(status) = report.status {
        println!("Scan status: {status:?}");
    }
    println!("Total findings: {}", report.findings.len());
    if let Some(coverage) = &report.coverage {
        println!(
            "Coverage: {}/{} targets scanned; files discovered: {}, files scanned: {}, files skipped: {}, bytes scanned: {}",
            coverage.targets_scanned,
            coverage.targets_discovered,
            coverage.files_discovered,
            coverage.files_scanned,
            coverage.files_skipped,
            coverage.bytes_scanned
        );
    }

    if report.summary.error > 0 {
        println!("  {} errors", report.summary.error.to_string().red().bold());
    }
    if report.summary.warning > 0 {
        println!(
            "  {} warnings",
            report.summary.warning.to_string().yellow().bold()
        );
    }
    if report.summary.info > 0 {
        println!("  {} info", report.summary.info.to_string().blue());
    }

    match (report.status, report.findings.is_empty()) {
        (Some(ScanStatus::Complete), true) | (None, true) => {
            println!(
                "{} No findings detected by the enabled audit rules.\nThis does not guarantee that the audited content is safe.",
                "✓".green().bold()
            );
        }
        (Some(ScanStatus::Incomplete), true) => {
            println!("Scan incomplete; no findings were confirmed.");
        }
        _ => {}
    }

    println!();
    for (severity, heading) in [
        (Severity::Error, "--- Errors ---".red().bold()),
        (Severity::Warning, "--- Warnings ---".yellow().bold()),
        (Severity::Info, "--- Info ---".blue().bold()),
    ] {
        let findings = report
            .findings
            .iter()
            .filter(|finding| finding.severity == severity)
            .collect::<Vec<_>>();
        if !findings.is_empty() {
            println!("{heading}");
            for finding in findings {
                print_diagnostic(finding);
            }
            println!();
        }
    }

    if let Some(scan_errors) = report
        .scan_errors
        .as_ref()
        .filter(|errors| !errors.is_empty())
    {
        println!("{}", "--- Scan errors ---".red().bold());
        for error in scan_errors {
            let target = error
                .target
                .as_deref()
                .map(|target| format!(" [{target}]"))
                .unwrap_or_default();
            let location = error
                .source_path
                .as_deref()
                .map(|path| format!(" ({path})"))
                .unwrap_or_default();
            println!("  {}{target}{location}", error.code.message());
        }
    }

    Ok(())
}

/// Print a single diagnostic.
fn print_diagnostic(diag: &agentsync::diagnostics::Diagnostic) {
    let severity_icon = match diag.severity {
        Severity::Error => "✗".red(),
        Severity::Warning => "⚠".yellow(),
        Severity::Info => "ℹ".blue(),
    };

    let location = if let Some(span) = &diag.span {
        format!("{}:{}", diag.source_path, span.line)
    } else {
        diag.source_path.clone()
    };
    let target = diag
        .target
        .as_deref()
        .map(|target| format!(" [{target}]"))
        .unwrap_or_default();

    println!(
        "  {} [{}]{} {}",
        severity_icon,
        diag.rule_id.bold(),
        target,
        diag.message
    );
    println!("     {}", location.dimmed());

    if let Some(evidence) = &diag.evidence {
        println!("     {}", evidence.dimmed());
    }
}

/// Print a JSON report.
fn print_json_report(report: &DiagnosticReport) -> Result<()> {
    let json =
        serde_json::to_string_pretty(report).context("Failed to serialize audit report to JSON")?;
    println!("{}", json);
    Ok(())
}

fn audit_skill(project_root: &std::path::Path, skill_id: &str) -> DiagnosticReport {
    let agents_root = project_root.join(".agents");
    let skills_root = agents_root.join("skills");
    let skill_root = skills_root.join(skill_id);
    let target = format!("skill/{skill_id}");

    for (path, relative_path) in [
        (agents_root.as_path(), ".agents"),
        (skills_root.as_path(), ".agents/skills"),
        (skill_root.as_path(), ".agents/skills/<target>"),
    ] {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return incomplete_target_report(
                    ScanErrorCode::SymlinkSkipped,
                    Some(relative_path),
                    &target,
                );
            }
            Ok(metadata) if !metadata.is_dir() => {
                return incomplete_target_report(
                    ScanErrorCode::TargetNotFound,
                    Some(relative_path),
                    &target,
                );
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return incomplete_target_report(
                    ScanErrorCode::TargetNotFound,
                    Some(relative_path),
                    &target,
                );
            }
            Err(_) => {
                return incomplete_target_report(
                    ScanErrorCode::RootReadFailed,
                    Some(relative_path),
                    &target,
                );
            }
        }
    }

    audit_directory_for_target(&skill_root, &target)
}

fn valid_selector(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

fn valid_plugin_selector(value: &str) -> bool {
    let mut parts = value.split('/');
    let first = parts.next().unwrap_or_default();
    let Some(second) = parts.next() else {
        return false;
    };
    valid_selector(first) && valid_selector(second) && parts.next().is_none()
}

fn incomplete_report(code: ScanErrorCode, target: Option<&str>) -> DiagnosticReport {
    let error = ScanError::new(code, None);
    let error = target.map_or(error.clone(), |target| error.with_target(target));
    DiagnosticReport::with_scan(
        Vec::new(),
        ScanStatus::Incomplete,
        ScanCoverage::default(),
        vec![error],
    )
}

fn incomplete_target_report(
    code: ScanErrorCode,
    source_path: Option<&str>,
    target: &str,
) -> DiagnosticReport {
    let error = ScanError::new(code, source_path.map(str::to_string)).with_target(target);
    DiagnosticReport::with_scan(
        Vec::new(),
        ScanStatus::Incomplete,
        ScanCoverage {
            targets_discovered: 0,
            ..ScanCoverage::default()
        },
        vec![error],
    )
}
