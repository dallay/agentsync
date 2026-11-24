//! Profile management CLI command.
//!
//! Implements `agentsync profile` subcommands: list, show, and use.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

use agentsync::profile::{
    ProfileInfo, get_profile_info, list_profiles, profiles_dir_for_project, validate_profile_chain,
};

/// Profile management subcommands.
#[derive(Subcommand, Debug)]
pub enum ProfileCommand {
    /// List all available profiles.
    List {
        /// Project root (defaults to CWD).
        #[arg(short, long)]
        project_root: Option<PathBuf>,
    },
    /// Show details about a profile.
    Show {
        /// Profile name.
        name: String,
        /// Project root (defaults to CWD).
        #[arg(short, long)]
        project_root: Option<PathBuf>,
    },
    /// Validate a profile chain (check for cycles and missing profiles).
    Validate {
        /// Profile name.
        name: String,
        /// Project root (defaults to CWD).
        #[arg(short, long)]
        project_root: Option<PathBuf>,
    },
}

/// Arguments for the profile command.
#[derive(Parser, Debug)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub cmd: ProfileCommand,
}

/// Run the profile command.
pub fn run_profile(args: ProfileArgs, project_root: &std::path::Path) -> Result<()> {
    match args.cmd {
        ProfileCommand::List { .. } => run_list(project_root),
        ProfileCommand::Show { name, .. } => run_show(project_root, &name),
        ProfileCommand::Validate { name, .. } => run_validate(project_root, &name),
    }
}

/// List all available profiles.
fn run_list(project_root: &std::path::Path) -> Result<()> {
    let profiles_dir = profiles_dir_for_project(project_root);

    if !profiles_dir.exists() {
        println!(
            "No profiles directory found. Create one at: {}",
            profiles_dir.display()
        );
        return Ok(());
    }

    let profiles = list_profiles(&profiles_dir)?;

    if profiles.is_empty() {
        println!("No profiles found in {}", profiles_dir.display());
        return Ok(());
    }

    println!("Available profiles:");
    println!();

    for name in &profiles {
        let info = match get_profile_info(&profiles_dir, name) {
            Ok(info) => info,
            Err(e) => {
                println!(
                    "  {} {} {}",
                    name.red().bold(),
                    "(".red(),
                    e.to_string().dimmed()
                );
                continue;
            }
        };
        print_profile_info(&info);
    }

    Ok(())
}

/// Print profile information.
fn print_profile_info(info: &ProfileInfo) {
    let extends_str = match &info.extends {
        Some(parent) => format!(" (extends {})", parent),
        None => String::new(),
    };

    println!("  {}{}", info.name.bold(), extends_str.dimmed());

    if !info.description.is_empty() {
        println!("    {}", info.description.dimmed());
    }

    let mut items = Vec::new();
    if info.agent_count > 0 {
        items.push(format!("{} agent(s)", info.agent_count));
    }
    if info.mcp_server_count > 0 {
        items.push(format!("{} MCP server(s)", info.mcp_server_count));
    }
    if !items.is_empty() {
        println!("    {}", items.join(", ").dimmed());
    }
    println!();
}

/// Show details about a specific profile.
fn run_show(project_root: &std::path::Path, name: &str) -> Result<()> {
    let profiles_dir = profiles_dir_for_project(project_root);

    let info = get_profile_info(&profiles_dir, name)
        .with_context(|| format!("Profile '{}' not found", name))?;

    println!("{}", format!("Profile: {}", name).bold());
    println!();

    if !info.description.is_empty() {
        println!("  Description: {}", info.description);
    }

    match &info.extends {
        Some(parent) => println!("  Extends:     {}", parent),
        None => println!("  Extends:     (none)"),
    }

    println!("  Agents:      {}", info.agent_count);
    println!("  MCP Servers: {}", info.mcp_server_count);
    println!();

    // Validate the chain
    if let Err(e) = validate_profile_chain(&profiles_dir, name) {
        println!(
            "  {} {}",
            "Warning:".yellow().bold(),
            e.to_string().dimmed()
        );
    } else {
        println!("  {} Chain is valid", "✓".green());
    }

    Ok(())
}

/// Validate a profile chain.
fn run_validate(project_root: &std::path::Path, name: &str) -> Result<()> {
    let profiles_dir = profiles_dir_for_project(project_root);

    // First check if the profile exists
    let _info = get_profile_info(&profiles_dir, name)
        .with_context(|| format!("Profile '{}' not found", name))?;

    match validate_profile_chain(&profiles_dir, name) {
        Ok(()) => {
            println!("{} Profile '{}' chain is valid", "✓".green(), name);
            Ok(())
        }
        Err(e) => {
            println!("{} Profile '{}' has issues: {}", "✗".red(), name, e);
            Err(e)
        }
    }
}
