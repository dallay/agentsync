//! AgentSync CLI
//!
//! Command-line interface for synchronizing AI agent configurations.

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::env;
use std::path::PathBuf;

use agentsync::{Linker, SyncOptions, config::Config, gitignore, init};

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
    /// Initialize a new agentsync configuration
    Init {
        /// Project root directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Overwrite existing configuration
        #[arg(short, long)]
        force: bool,
    },

    /// Apply configuration and create symlinks
    Apply {
        /// Project root directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Path to configuration file
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Remove existing symlinks before creating new ones
        #[arg(long)]
        clean: bool,

        /// Show what would be done without making changes
        #[arg(long)]
        dry_run: bool,

        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,

        /// Filter to specific agents (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        agents: Option<Vec<String>>,

        /// Disable gitignore updates
        #[arg(long)]
        no_gitignore: bool,
    },

    /// Remove all managed symlinks
    Clean {
        /// Project root directory (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Path to configuration file
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Show what would be done without making changes
        #[arg(long)]
        dry_run: bool,

        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { path, force } => {
            let project_root = path.unwrap_or_else(|| env::current_dir().unwrap());

            print_header();
            println!("{}", "Initializing agentsync configuration...\n".cyan());

            init::init(&project_root, force)?;

            println!("\n{}", "✨ Initialization complete!".green().bold());
            println!(
                "\nNext steps:\n  1. Edit {} with your project instructions\n  2. Run {} to create symlinks",
                ".agents/AGENTS.md".cyan(),
                "agentsync apply".cyan()
            );
        }

        Commands::Apply {
            path,
            config,
            clean,
            dry_run,
            verbose,
            agents,
            no_gitignore,
        } => {
            let start_dir = path.unwrap_or_else(|| env::current_dir().unwrap());

            print_header();

            // Find or use specified config
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

            // Load configuration
            let config = Config::load(&config_path)?;
            let linker = Linker::new(config, config_path);

            // Clean first if requested
            if clean {
                println!("{}", "➤ Cleaning existing symlinks".cyan().bold());
                let clean_opts = SyncOptions {
                    dry_run,
                    verbose,
                    ..Default::default()
                };
                linker.clean(&clean_opts)?;
            }

            // Sync
            println!("{}", "➤ Syncing agent configurations".cyan().bold());
            let options = SyncOptions {
                clean: false,
                dry_run,
                verbose,
                agents,
            };

            let mut result = linker.sync(&options)?;

            // Update gitignore
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

            // Sync MCP configurations
            if linker.config().mcp.enabled && !linker.config().mcp_servers.is_empty() {
                println!("\n{}", "➤ Syncing MCP configurations".cyan().bold());
                match linker.sync_mcp(dry_run) {
                    Ok(mcp_result) => {
                        if mcp_result.created > 0 || mcp_result.updated > 0 {
                            println!(
                                "  MCP configs: Created {}, Updated {}",
                                mcp_result.created.to_string().green(),
                                mcp_result.updated.to_string().yellow()
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("  {} Error syncing MCP configs: {}", "✘".red(), e);
                        result.errors += 1;
                    }
                }
            }

            // Summary
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
            path,
            config,
            dry_run,
            verbose,
        } => {
            let start_dir = path.unwrap_or_else(|| env::current_dir().unwrap());

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
    }

    Ok(())
}

fn print_header() {
    println!(
        "{}",
        r#"
╔═══════════════════════════════════════════════════════════════════╗
║                         AgentSync                                 ║
║           AI Agent Configuration Synchronization                  ║
╚═══════════════════════════════════════════════════════════════════╝
"#
        .cyan()
        .bold()
    );
}
