//! AgentSync CLI
//!
//! Command-line interface for synchronizing AI agent configurations.

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::env;
use std::path::PathBuf;

use agentsync::{Linker, SyncOptions, SyncResult, config::Config, gitignore, init};
mod commands;
mod output;
use commands::doctor::run_doctor;
use commands::skill::{SkillCommand, run_skill};
use commands::status::{StatusArgs, run_status};
use output::{HumanFormatter, LabelKind, output_mode};

fn human_use_color() -> bool {
    match output_mode(false) {
        output::OutputMode::Human { use_color } => use_color,
        output::OutputMode::Json => false,
    }
}

fn render_phase(title: &str, detail: &str, use_color: bool) -> Vec<String> {
    let formatter = HumanFormatter::new(use_color);
    vec![
        formatter.format_heading(&format!("➤ {title}")),
        format!("  {detail}"),
    ]
}

fn render_dry_run_notice(use_color: bool) -> Vec<String> {
    let formatter = HumanFormatter::new(use_color);
    vec![
        formatter.format_label("!", "Dry run", LabelKind::Warning),
        "  No filesystem changes will be made.".to_string(),
    ]
}

#[cfg(test)]
fn render_clean_phase(dry_run: bool) -> Vec<String> {
    render_clean_phase_with_color(dry_run, false)
}

fn render_clean_phase_with_color(dry_run: bool, use_color: bool) -> Vec<String> {
    render_phase(
        "Clean",
        if dry_run {
            "Previewing managed symlink removals"
        } else {
            "Removing managed symlinks"
        },
        use_color,
    )
}

#[cfg(test)]
fn render_sync_phase(dry_run: bool, clean_first: bool) -> Vec<String> {
    render_sync_phase_with_color(dry_run, clean_first, false)
}

fn render_sync_phase_with_color(dry_run: bool, clean_first: bool, use_color: bool) -> Vec<String> {
    let detail = match (dry_run, clean_first) {
        (true, true) => "Previewing clean and sync changes",
        (true, false) => "Previewing agent configuration changes",
        (false, true) => "Cleaning existing symlinks before syncing",
        (false, false) => "Syncing agent configurations",
    };
    render_phase("Sync", detail, use_color)
}

#[cfg(test)]
fn render_gitignore_phase(enabled: bool, dry_run: bool) -> Vec<String> {
    render_gitignore_phase_with_color(enabled, dry_run, false)
}

fn render_gitignore_phase_with_color(enabled: bool, dry_run: bool, use_color: bool) -> Vec<String> {
    let detail = match (enabled, dry_run) {
        (true, true) => "Previewing .gitignore update",
        (true, false) => "Updating .gitignore",
        (false, true) => "Previewing .gitignore cleanup",
        (false, false) => "Cleaning .gitignore",
    };
    render_phase("Gitignore", detail, use_color)
}

fn render_mcp_phase(dry_run: bool, use_color: bool) -> Vec<String> {
    render_phase(
        "MCP",
        if dry_run {
            "Previewing MCP configuration changes"
        } else {
            "Syncing MCP configurations"
        },
        use_color,
    )
}

fn render_count(label: &str, value: usize, kind: LabelKind, use_color: bool) -> String {
    let formatter = HumanFormatter::new(use_color);
    format!(
        "  {}",
        formatter.format_summary_line(label, &value.to_string(), kind)
    )
}

#[cfg(test)]
fn render_apply_summary(dry_run: bool, result: &SyncResult) -> Vec<String> {
    render_apply_summary_with_color(dry_run, result, false)
}

fn render_apply_summary_with_color(
    dry_run: bool,
    result: &SyncResult,
    use_color: bool,
) -> Vec<String> {
    let formatter = HumanFormatter::new(use_color);
    let mut lines = vec![formatter.format_label(
        "✔",
        if dry_run {
            "Sync dry run complete"
        } else {
            "Sync complete"
        },
        LabelKind::Success,
    )];
    lines.push(render_count(
        "Created",
        result.created,
        LabelKind::Success,
        use_color,
    ));
    lines.push(render_count(
        "Updated",
        result.updated,
        LabelKind::Warning,
        use_color,
    ));
    lines.push(render_count(
        "Skipped",
        result.skipped,
        LabelKind::Muted,
        use_color,
    ));
    lines.push(render_count(
        "Errors",
        result.errors,
        if result.errors > 0 {
            LabelKind::Failure
        } else {
            LabelKind::Muted
        },
        use_color,
    ));
    lines
}

#[cfg(test)]
fn render_clean_summary(dry_run: bool, removed: usize) -> Vec<String> {
    render_clean_summary_with_color(dry_run, removed, false)
}

fn render_clean_summary_with_color(dry_run: bool, removed: usize, use_color: bool) -> Vec<String> {
    let formatter = HumanFormatter::new(use_color);
    vec![
        formatter.format_label(
            "✔",
            if dry_run {
                "Clean dry run complete"
            } else {
                "Clean complete"
            },
            LabelKind::Success,
        ),
        render_count(
            if dry_run { "Would remove" } else { "Removed" },
            removed,
            LabelKind::Success,
            use_color,
        ),
    ]
}

#[cfg(test)]
fn render_mcp_summary(result: &agentsync::mcp::McpSyncResult) -> Vec<String> {
    render_mcp_summary_with_color(result, false)
}

fn render_mcp_summary_with_color(
    result: &agentsync::mcp::McpSyncResult,
    use_color: bool,
) -> Vec<String> {
    vec![
        render_count("Created", result.created, LabelKind::Success, use_color),
        render_count("Updated", result.updated, LabelKind::Warning, use_color),
    ]
}

fn print_lines(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}

fn init_next_steps_lines(wizard: bool) -> Option<Vec<String>> {
    if wizard {
        return None;
    }

    Some(vec![
        "Next steps:".to_string(),
        "  1. Edit .agents/AGENTS.md with your project instructions".to_string(),
        "  2. Run agentsync apply to create symlinks".to_string(),
    ])
}

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
        #[arg(short, long)]
        project_root: Option<PathBuf>,
    },
    /// Run diagnostic and health check
    Doctor {
        /// Project root (defaults to CWD)
        #[arg(short, long)]
        project_root: Option<PathBuf>,
    },
    /// Show status of managed symlinks
    Status {
        #[command(flatten)]
        args: StatusArgs,
        /// Project root (defaults to CWD)
        #[arg(short, long)]
        project_root: Option<PathBuf>,
    },
    /// Initialize a new agentsync configuration in the current or specified directory.
    Init {
        #[arg(
            short,
            long,
            help = "Project root directory (defaults to current dir)",
            alias = "project-root"
        )]
        path: Option<PathBuf>,
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
        #[arg(short, long, alias = "project-root")]
        path: Option<PathBuf>,
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
        #[arg(short, long, alias = "project-root")]
        path: Option<PathBuf>,
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
    agentsync::update_check::spawn();
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
            path,
            force,
            wizard,
        } => {
            let project_root = path.unwrap_or_else(|| env::current_dir().unwrap());
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
            if let Some(lines) = init_next_steps_lines(wizard) {
                for line in lines {
                    println!("{line}");
                }
            }
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
            let use_color = human_use_color();
            if dry_run {
                print_lines(&render_dry_run_notice(use_color));
                println!();
            }
            if clean {
                print_lines(&render_clean_phase_with_color(dry_run, use_color));
                let clean_opts = SyncOptions {
                    dry_run,
                    verbose,
                    ..Default::default()
                };
                linker.clean(&clean_opts)?;
                println!();
            }
            print_lines(&render_sync_phase_with_color(dry_run, clean, use_color));
            let options = SyncOptions {
                clean: false,
                dry_run,
                verbose,
                agents,
            };
            let mut result = linker.sync(&options)?;
            if !no_gitignore {
                if linker.config().gitignore.enabled {
                    println!();
                    print_lines(&render_gitignore_phase_with_color(true, dry_run, use_color));
                    let entries = linker.config().all_gitignore_entries();
                    gitignore::update_gitignore(
                        linker.project_root(),
                        &linker.config().gitignore.marker,
                        &entries,
                        dry_run,
                    )?;
                } else {
                    println!();
                    print_lines(&render_gitignore_phase_with_color(
                        false, dry_run, use_color,
                    ));
                    gitignore::cleanup_gitignore(
                        linker.project_root(),
                        &linker.config().gitignore.marker,
                        dry_run,
                    )?;
                }
            }
            if linker.config().mcp.enabled && !linker.config().mcp_servers.is_empty() {
                println!();
                print_lines(&render_mcp_phase(dry_run, use_color));
                match linker.sync_mcp(dry_run, options.agents.as_ref()) {
                    Ok(mcp_result) => {
                        if mcp_result.created > 0 || mcp_result.updated > 0 {
                            print_lines(&render_mcp_summary_with_color(&mcp_result, use_color));
                        }
                    }
                    Err(e) => {
                        tracing::error!(%e, "Error syncing MCP configs");
                        result.errors += 1;
                    }
                }
            }
            println!();
            print_lines(&render_apply_summary_with_color(
                dry_run, &result, use_color,
            ));
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
            let use_color = human_use_color();
            if dry_run {
                print_lines(&render_dry_run_notice(use_color));
                println!();
            }
            print_lines(&render_clean_phase_with_color(dry_run, use_color));
            let options = SyncOptions {
                dry_run,
                verbose,
                ..Default::default()
            };
            let result = linker.clean(&options)?;
            println!();
            print_lines(&render_clean_summary_with_color(
                dry_run,
                result.removed,
                use_color,
            ));
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

#[cfg(test)]
mod tests {
    use super::{
        init_next_steps_lines, render_apply_summary, render_clean_phase, render_clean_summary,
        render_dry_run_notice, render_gitignore_phase, render_mcp_summary, render_sync_phase,
    };
    use agentsync::{SyncResult, mcp::McpSyncResult};

    #[test]
    fn test_render_dry_run_notice_is_explicit() {
        assert_eq!(
            render_dry_run_notice(false),
            vec![
                "! Dry run".to_string(),
                "  No filesystem changes will be made.".to_string()
            ]
        );
    }

    #[test]
    fn test_render_sync_phase_names_dry_run_preview() {
        assert_eq!(
            render_sync_phase(true, false),
            vec![
                "➤ Sync".to_string(),
                "  Previewing agent configuration changes".to_string()
            ]
        );
    }

    #[test]
    fn test_render_gitignore_phase_distinguishes_update_and_clean() {
        assert_eq!(
            render_gitignore_phase(true, false),
            vec![
                "➤ Gitignore".to_string(),
                "  Updating .gitignore".to_string()
            ]
        );
        assert_eq!(
            render_gitignore_phase(false, true),
            vec![
                "➤ Gitignore".to_string(),
                "  Previewing .gitignore cleanup".to_string()
            ]
        );
    }

    #[test]
    fn test_render_apply_summary_uses_consistent_counts() {
        let summary = render_apply_summary(
            false,
            &SyncResult {
                created: 2,
                updated: 1,
                skipped: 3,
                removed: 0,
                errors: 1,
            },
        );

        assert_eq!(
            summary,
            vec![
                "✔ Sync complete".to_string(),
                "  Created: 2".to_string(),
                "  Updated: 1".to_string(),
                "  Skipped: 3".to_string(),
                "  Errors: 1".to_string(),
            ]
        );
    }

    #[test]
    fn test_render_clean_phase_and_summary_make_dry_run_clear() {
        assert_eq!(
            render_clean_phase(true),
            vec![
                "➤ Clean".to_string(),
                "  Previewing managed symlink removals".to_string()
            ]
        );
        assert_eq!(
            render_clean_summary(false, 3),
            vec!["✔ Clean complete".to_string(), "  Removed: 3".to_string()]
        );
        assert_eq!(
            render_clean_summary(true, 3),
            vec![
                "✔ Clean dry run complete".to_string(),
                "  Would remove: 3".to_string()
            ]
        );
    }

    #[test]
    fn test_render_mcp_summary_reports_created_and_updated() {
        let summary = render_mcp_summary(&McpSyncResult {
            created: 1,
            updated: 2,
            skipped: 0,
            errors: 0,
        });

        assert_eq!(
            summary,
            vec!["  Created: 1".to_string(), "  Updated: 2".to_string()]
        );
    }

    #[test]
    fn test_init_next_steps_lines_suppresses_generic_footer_for_wizard_runs() {
        assert!(init_next_steps_lines(true).is_none());

        let standard = init_next_steps_lines(false).expect("standard init should keep next steps");
        let rendered = standard.join("\n");
        assert!(rendered.contains("Edit .agents/AGENTS.md"));
        assert!(rendered.contains("Run agentsync apply"));
    }
}
