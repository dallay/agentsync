//! Policy validation CLI command.
//!
//! Implements `agentsync policy validate` to evaluate policy packs.

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use std::path::PathBuf;

use agentsync::config::Config;
use agentsync::policy::{evaluate_all_policies, list_policies, policies_dir_for_project};

/// Arguments for the policy command.
#[derive(Parser, Debug)]
pub struct PolicyArgs {
    /// Project root (defaults to CWD).
    #[arg(short, long)]
    project_root: Option<PathBuf>,
}

/// Run the policy validate command.
pub fn run_policy(_args: PolicyArgs, project_root: PathBuf) -> Result<()> {
    let policies_dir = policies_dir_for_project(&project_root);

    if !policies_dir.exists() {
        println!(
            "No policies directory found. Create one at: {}",
            policies_dir.display()
        );
        return Ok(());
    }

    let policy_names = list_policies(&policies_dir)?;
    if policy_names.is_empty() {
        println!("No policies found in {}", policies_dir.display());
        return Ok(());
    }

    let config_path = Config::find_config(&project_root)?;
    let config = Config::load(&config_path)?;

    let reports = evaluate_all_policies(&project_root, &config)?;

    let mut all_passed = true;
    for report in &reports {
        if !report.passed() {
            all_passed = false;
        }
    }

    // Print results
    for report in &reports {
        let status = if report.passed() {
            format!("{}", "✓".green())
        } else {
            format!("{}", "✗".red())
        };

        println!(
            "{} Policy '{}': {} findings",
            status,
            report.policy_name.bold(),
            report.total
        );

        for diag in &report.diagnostics {
            let icon = match diag.severity {
                agentsync::diagnostics::Severity::Error => "✗".red(),
                agentsync::diagnostics::Severity::Warning => "⚠".yellow(),
                agentsync::diagnostics::Severity::Info => "ℹ".blue(),
            };
            println!("  {} [{}] {}", icon, diag.rule_id.bold(), diag.message);
            if let Some(evidence) = &diag.evidence {
                println!("     {}", evidence.dimmed());
            }
        }
    }

    if !all_passed {
        println!("\n{} Policy validation failed", "✗".red().bold());
        std::process::exit(1);
    } else {
        println!("\n{} All policies passed", "✓".green().bold());
    }

    Ok(())
}
