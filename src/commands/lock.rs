//! Environment lock CLI command.
//!
//! Implements `agentsync lock` subcommands: generate and verify.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

use agentsync::config::Config;
use agentsync::lock::{EnvLock, VerifyResult, generate_lock, verify_lock};

/// Lock management subcommands.
#[derive(Subcommand, Debug)]
pub enum LockCommand {
    /// Generate a new lockfile from the current project state.
    Generate {
        /// Project root (defaults to CWD).
        #[arg(short, long)]
        project_root: Option<PathBuf>,
    },
    /// Verify the current environment against the lockfile.
    Verify {
        /// Fail with non-zero exit code if drift is detected.
        #[arg(long)]
        fail_on_drift: bool,
        /// Project root (defaults to CWD).
        #[arg(short, long)]
        project_root: Option<PathBuf>,
    },
}

/// Arguments for the lock command.
#[derive(Parser, Debug)]
pub struct LockArgs {
    #[command(subcommand)]
    pub cmd: LockCommand,
}

/// Run the lock command.
pub fn run_lock(args: LockArgs, project_root: &std::path::Path) -> Result<()> {
    match args.cmd {
        LockCommand::Generate { .. } => run_generate(project_root),
        LockCommand::Verify { fail_on_drift, .. } => run_verify(project_root, fail_on_drift),
    }
}

/// Generate a new lockfile.
fn run_generate(project_root: &std::path::Path) -> Result<()> {
    let config_path = Config::find_config(project_root)?;
    let config = Config::load(&config_path)?;

    let lock = generate_lock(project_root, &config).context("Failed to generate lockfile")?;

    lock.save(project_root).context("Failed to save lockfile")?;

    let lock_path = EnvLock::lockfile_path(project_root);
    println!(
        "{} Lockfile generated: {} ({} entries)",
        "✓".green().bold(),
        lock_path.display(),
        lock.total
    );

    // Print summary by category
    for (category, entries) in &lock.entries {
        if !entries.is_empty() {
            println!(
                "  {}: {} file(s)",
                format!("{:?}", category).to_lowercase(),
                entries.len()
            );
        }
    }

    Ok(())
}

/// Verify the current environment against the lockfile.
fn run_verify(project_root: &std::path::Path, fail_on_drift: bool) -> Result<()> {
    let lock = EnvLock::load(project_root)?;
    let config_path = Config::find_config(project_root)?;
    let config = Config::load(&config_path)?;

    let result = verify_lock(project_root, &config, &lock)?;

    print_verify_result(&result)?;

    if fail_on_drift && !result.matches {
        std::process::exit(1);
    }

    Ok(())
}

/// Print the verification result.
fn print_verify_result(result: &VerifyResult) -> Result<()> {
    if result.matches {
        println!(
            "{} Environment matches lockfile — no drift detected",
            "✓".green().bold()
        );
        return Ok(());
    }

    println!("{} Environment drift detected!", "✗".red().bold());

    if !result.added.is_empty() {
        println!(
            "\n  {} ({} new file(s))",
            "Added:".blue().bold(),
            result.added.len()
        );
        for path in &result.added {
            println!("    + {}", path.dimmed());
        }
    }

    if !result.modified.is_empty() {
        println!(
            "\n  {} ({} modified file(s))",
            "Modified:".yellow().bold(),
            result.modified.len()
        );
        for path in &result.modified {
            println!("    ~ {}", path.dimmed());
        }
    }

    if !result.removed.is_empty() {
        println!(
            "\n  {} ({} removed file(s))",
            "Removed:".red().bold(),
            result.removed.len()
        );
        for path in &result.removed {
            println!("    - {}", path.dimmed());
        }
    }

    Ok(())
}
