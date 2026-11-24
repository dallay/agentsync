//! Static security audit CLI command.
//!
//! Implements `agentsync audit` as a read-only static analyzer that detects
//! credential exposure, risky shell patterns, file traversal issues, and
//! unexpected privilege escalation.

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use colored::Colorize;
use std::path::PathBuf;

use agentsync::audit::{AuditReport, audit_directory};
use agentsync::diagnostics::Severity;

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
pub struct AuditArgs {
    /// Audit all skills in the project.
    #[arg(long)]
    all: bool,

    /// Audit a specific skill by ID.
    #[arg(long)]
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

    /// Project root (defaults to CWD).
    #[arg(short, long)]
    project_root: Option<PathBuf>,
}

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

    // Determine the directory to audit
    let audit_dir = match &audit_target {
        AuditTarget::All => project_root.join(".agents"),
        AuditTarget::Skill(skill_id) => project_root.join(".agents").join("skills").join(skill_id),
        AuditTarget::Plugin(plugin_id) => {
            project_root.join(".agents").join("plugins").join(plugin_id)
        }
    };

    if !audit_dir.exists() {
        eprintln!(
            "{} Audit target does not exist: {}",
            "warning:".yellow().bold(),
            audit_dir.display()
        );

        // Return empty report
        let report = AuditReport::from_diagnostics(vec![]);
        return print_report(&report, args.format, args.fail_on);
    }

    // Run audit
    let report = audit_directory(&audit_dir);

    // Print results
    print_report(&report, args.format, args.fail_on)?;

    // Check fail threshold
    if let Some(threshold) = args.fail_on {
        let should_fail = match threshold {
            FailThreshold::Warning => {
                *report.counts.get(&Severity::Warning).unwrap_or(&0) > 0
                    || *report.counts.get(&Severity::Error).unwrap_or(&0) > 0
            }
            FailThreshold::Error => *report.counts.get(&Severity::Error).unwrap_or(&0) > 0,
        };

        if should_fail {
            std::process::exit(1);
        }
    }

    Ok(())
}

enum AuditTarget {
    All,
    Skill(String),
    Plugin(String),
}

/// Print the audit report in the specified format.
fn print_report(
    report: &AuditReport,
    format: AuditFormat,
    _fail_on: Option<FailThreshold>,
) -> Result<()> {
    match format {
        AuditFormat::Human => print_human_report(report),
        AuditFormat::Json => print_json_report(report),
    }
}

/// Print a human-readable report.
fn print_human_report(report: &AuditReport) -> Result<()> {
    if report.diagnostics.is_empty() {
        println!("{} No security issues found.", "✓".green().bold());
        return Ok(());
    }

    // Print summary
    println!("\n{}", "=== Security Audit Report ===".bold());
    println!("Total findings: {}", report.total);

    let error_count = report.counts.get(&Severity::Error).unwrap_or(&0);
    let warning_count = report.counts.get(&Severity::Warning).unwrap_or(&0);
    let info_count = report.counts.get(&Severity::Info).unwrap_or(&0);

    if *error_count > 0 {
        println!("  {} errors", format!("{}", error_count).red().bold());
    }
    if *warning_count > 0 {
        println!(
            "  {} warnings",
            format!("{}", warning_count).yellow().bold()
        );
    }
    if *info_count > 0 {
        println!("  {} info", format!("{}", info_count).blue());
    }

    // Print diagnostics grouped by severity
    println!();

    // Errors first
    let errors: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();

    if !errors.is_empty() {
        println!("{}", "--- Errors ---".red().bold());
        for diag in &errors {
            print_diagnostic(diag);
        }
        println!();
    }

    // Warnings
    let warnings: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .collect();

    if !warnings.is_empty() {
        println!("{}", "--- Warnings ---".yellow().bold());
        for diag in &warnings {
            print_diagnostic(diag);
        }
        println!();
    }

    // Info
    let infos: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Info)
        .collect();

    if !infos.is_empty() {
        println!("{}", "--- Info ---".blue().bold());
        for diag in &infos {
            print_diagnostic(diag);
        }
        println!();
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

    println!(
        "  {} [{}] {}",
        severity_icon,
        diag.rule_id.bold(),
        diag.message
    );
    println!("     {}", location.dimmed());

    if let Some(evidence) = &diag.evidence {
        println!("     {}", evidence.dimmed());
    }
}

/// Print a JSON report.
fn print_json_report(report: &AuditReport) -> Result<()> {
    let json =
        serde_json::to_string_pretty(report).context("Failed to serialize audit report to JSON")?;
    println!("{}", json);
    Ok(())
}
