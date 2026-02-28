//! AgentSync CLI
//!
//! Command-line interface for synchronizing AI agent configurations.

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::env;
use std::path::PathBuf;

use agentsync::{Linker, SyncOptions, config::Config, gitignore, init};
mod commands;
use commands::doctor::run_doctor;
use commands::skill::{SkillCommand, run_skill};
use commands::status::{StatusArgs, run_status};

// tracing_subscriber is used to initialize logging in main

#[derive(Parser)]
#[command(name = "agentsync")]
#[command(
    author,
    version,
    about = "Sync AI agent configurations using symbolic links"
)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage installable AI agent skills from skills.sh/other providers
    Skill {
        #[command(subcommand)]
        cmd: SkillCommand,
        /// Root of the project (defaults to CWD)
        #[arg(short = 'p', long)]
        project_root: Option<PathBuf>,
    },
    /// Run diagnostic and health check
    Doctor {
        /// Project root (defaults to CWD)
        #[arg(short = 'p', long)]
        project_root: Option<PathBuf>,
    },
    /// Show status of managed symlinks
    Status {
        #[command(flatten)]
        args: StatusArgs,
        /// Project root (defaults to CWD)
        #[arg(short = 'p', long)]
        project_root: Option<PathBuf>,
    },
    /// Initialize a new agentsync configuration in the current or specified directory.
    Init {
        #[arg(
            short = 'p',
            long = "project-root",
            help = "Project root directory (defaults to current dir)"
        )]
        project_root: Option<PathBuf>,
        #[arg(
            short,
            long,
            help = "Overwrite existing configuration without prompting"
        )]
        force: bool,
        #[arg(
            short,
            long,
            help = "Run interactive configuration wizard to migrate existing files"
        )]
        wizard: bool,
    },
    /// Apply the configuration from agentsync.toml
    Apply {
        #[arg(short = 'p', long = "project-root")]
        project_root: Option<PathBuf>,
        #[arg(short, long)]
        config: Option<PathBuf>,
        #[arg(long)]
        clean: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(short, long)]
        verbose: bool,
        #[arg(short, long, value_delimiter = ',')]
        agents: Option<Vec<String>>,
        #[arg(long)]
        no_gitignore: bool,
    },
    /// Remove all symlinks created by agentsync
    Clean {
        #[arg(short = 'p', long = "project-root")]
        project_root: Option<PathBuf>,
        #[arg(short, long)]
        config: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
        #[arg(short, long)]
        verbose: bool,
    },
    /// Developer-only: install a sample skill (dev)
    #[command(hide = true)]
    DevInstall {
        #[arg(help = "skill id to install")]
        skill_id: String,
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    // Initialize tracing subscriber for structured logging. Respects RUST_LOG env var.
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Skill { cmd, project_root } => {
            let root = project_root.unwrap_or_else(|| env::current_dir().unwrap());
            run_skill(cmd, root)?;
        }
        Commands::Status { args, project_root } => {
            let project_root = project_root.unwrap_or_else(|| env::current_dir().unwrap());
            run_status(args.json, project_root)?;
        }
        Commands::Doctor { project_root } => {
            let project_root = project_root.unwrap_or_else(|| env::current_dir().unwrap());
            run_doctor(project_root)?;
        }
        Commands::Init {
            project_root,
            force,
            wizard,
        } => {
            let project_root = project_root.unwrap_or_else(|| env::current_dir().unwrap());
            print_header();
            if wizard {
                println!(
                    "{}",
                    "Starting interactive configuration wizard...\n".cyan()
                );
                init::init_wizard(&project_root, force)?;
            } else {
                println!("{}", "Initializing agentsync configuration...\n".cyan());
                init::init(&project_root, force)?;
            }
            println!("\n{}", "✨ Initialization complete!".green().bold());
            println!(
                "\nNext steps:\n  1. Edit {} with your project instructions\n  2. Run {} to create symlinks",
                ".agents/AGENTS.md".cyan(),
                "agentsync apply".cyan()
            );
        }
        Commands::Apply {
            project_root,
            config,
            clean,
            dry_run,
            verbose,
            agents,
            no_gitignore,
        } => {
            let start_dir = project_root.unwrap_or_else(|| env::current_dir().unwrap());
            print_header();
            let config_path = match config {
                Some(p) => p,
                None => Config::find_config(&start_dir)?,
            };
            if verbose {
                println!(
                    "Using config: {}\n",
                    config_path.display().to_string().dimmed()
                );
            }
            let config = Config::load(&config_path)?;
            let linker = Linker::new(config, config_path);
            if clean {
                println!("{}", "➤ Cleaning existing symlinks".cyan().bold());
                let clean_opts = SyncOptions {
                    dry_run,
                    verbose,
                    ..Default::default()
                };
                linker.clean(&clean_opts)?;
            }
            println!("{}", "➤ Syncing agent configurations".cyan().bold());
            let options = SyncOptions {
                clean: false,
                dry_run,
                verbose,
                agents,
            };
            let mut result = linker.sync(&options)?;
            if !no_gitignore && linker.config().gitignore.enabled {
                println!("\n{}", "➤ Updating .gitignore".cyan().bold());
                let entries = linker.config().all_gitignore_entries();
                gitignore::update_gitignore(
                    linker.project_root(),
                    &linker.config().gitignore.marker,
                    &entries,
                    dry_run,
                )?;
            }
            if linker.config().mcp.enabled && !linker.config().mcp_servers.is_empty() {
                println!("\n{}", "➤ Syncing MCP configurations".cyan().bold());
                match linker.sync_mcp(dry_run, options.agents.as_ref()) {
                    Ok(mcp_result) => {
                        if mcp_result.created > 0 || mcp_result.updated > 0 {
                            println!(
                                "  MCP configs: Created {}, Updated {}",
                                mcp_result.created.to_string().green(),
                                mcp_result.updated.to_string().yellow(),
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!(%e, "Error syncing MCP configs");
                        result.errors += 1;
                    }
                }
            }
            println!("\n{}", "✨ Sync complete!".green().bold());
            println!(
                "  Created: {}, Updated: {}, Skipped: {}, Errors: {}",
                result.created.to_string().green(),
                result.updated.to_string().yellow(),
                result.skipped.to_string().dimmed(),
                if result.errors > 0 {
                    result.errors.to_string().red()
                } else {
                    result.errors.to_string().dimmed()
                }
            );
        }
        Commands::Clean {
            project_root,
            config,
            dry_run,
            verbose,
        } => {
            let start_dir = project_root.unwrap_or_else(|| env::current_dir().unwrap());
            print_header();
            let config_path = match config {
                Some(p) => p,
                None => Config::find_config(&start_dir)?,
            };
            let config = Config::load(&config_path)?;
            let linker = Linker::new(config, config_path);
            let options = SyncOptions {
                dry_run,
                verbose,
                ..Default::default()
            };
            let result = linker.clean(&options)?;
            println!("\n{}", "✨ Clean complete!".green().bold());
            println!("  Removed: {} symlinks", result.removed.to_string().green());
        }
        Commands::DevInstall { skill_id, json } => {
            let project_root = env::current_dir().unwrap();
            use commands::skill::SkillInstallArgs;
            use commands::skill::run_install;
            let args = SkillInstallArgs {
                skill_id,
                source: None,
                json,
            };
            run_install(args, project_root)?;
        }
    }
    Ok(())
}

fn print_header() {
    let banner = include_str!("banner.txt");
    println!("{}", banner.cyan().bold());
}
