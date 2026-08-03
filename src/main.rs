//! AgentSync CLI
//!
//! Command-line interface for synchronizing AI agent configurations.

use anyhow::{Context, Result};
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
use output::{
    human_use_color, init_next_steps_lines, print_header, print_lines,
    render_apply_summary_with_color, render_clean_phase_with_color,
    render_clean_summary_with_color, render_dry_run_notice, render_gitignore_phase_with_color,
    render_mcp_phase, render_mcp_summary_with_color, render_sync_phase_with_color,
};

fn current_project_root<F>(path: Option<PathBuf>, current_dir: F) -> Result<PathBuf>
where
    F: FnOnce() -> Result<PathBuf>,
{
    path.map_or_else(
        || current_dir().context("failed to determine current project directory"),
        Ok,
    )
}

fn merge_clean_result_into_apply_result(result: &mut SyncResult, clean_result: &SyncResult) {
    result.updated += clean_result.updated;
    result.skipped += clean_result.skipped;
    result.removed += clean_result.removed;
    result.errors += clean_result.errors;
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
        #[arg(
            long,
            requires = "wizard",
            help = "Run the init wizard with an experimental full-screen TUI intro"
        )]
        experimental_tui: bool,
        #[arg(
            short = 't',
            long,
            help = "Path to a TOML config template to use instead of the built-in default"
        )]
        template: Option<PathBuf>,
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
            let root =
                current_project_root(project_root, || env::current_dir().map_err(Into::into))?;
            run_skill(cmd, root)?;
        }
        Commands::Status { args, project_root } => {
            let project_root =
                current_project_root(project_root, || env::current_dir().map_err(Into::into))?;
            run_status(args.json, project_root)?;
        }
        Commands::Doctor { project_root } => {
            let project_root =
                current_project_root(project_root, || env::current_dir().map_err(Into::into))?;
            run_doctor(project_root)?;
        }
        Commands::Init {
            path,
            force,
            wizard,
            experimental_tui,
            template,
        } => handle_init(path, force, wizard, experimental_tui, template)?,
        Commands::Apply {
            path,
            config,
            clean,
            dry_run,
            verbose,
            agents,
            no_gitignore,
        } => handle_apply(ApplyArgs {
            path,
            config,
            clean,
            dry_run,
            verbose,
            agents,
            no_gitignore,
        })?,
        Commands::Clean {
            path,
            config,
            dry_run,
            verbose,
        } => handle_clean(path, config, dry_run, verbose)?,
        Commands::DevInstall { skill_id, json } => {
            let project_root =
                current_project_root(None, || env::current_dir().map_err(Into::into))?;
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

fn handle_init(
    path: Option<PathBuf>,
    force: bool,
    wizard: bool,
    experimental_tui: bool,
    template: Option<PathBuf>,
) -> Result<()> {
    let project_root = current_project_root(path, || env::current_dir().map_err(Into::into))?;
    print_header();
    if wizard {
        println!(
            "{}",
            "Starting interactive configuration wizard...\n".cyan()
        );
        if experimental_tui {
            init::init_wizard_experimental_tui(&project_root, force, template.as_deref())?;
        } else {
            init::init_wizard(&project_root, force, template.as_deref())?;
        }
    } else {
        println!("{}", "Initializing agentsync configuration...\n".cyan());
        let (config_content, source) = init::resolve_config_template(template.as_deref())?;
        if let Some(notice) = source.notice() {
            use colored::Colorize;
            println!("  {} {notice}", "✔".green());
        }
        init::init(&project_root, force, &config_content)?;
    }
    println!("\n{}", "✨ Initialization complete!".green().bold());
    if let Some(lines) = init_next_steps_lines(wizard) {
        for line in lines {
            println!("{line}");
        }
    }
    Ok(())
}

struct ApplyArgs {
    path: Option<PathBuf>,
    config: Option<PathBuf>,
    clean: bool,
    dry_run: bool,
    verbose: bool,
    agents: Option<Vec<String>>,
    no_gitignore: bool,
}

fn handle_apply(args: ApplyArgs) -> Result<()> {
    let start_dir = current_project_root(args.path, || env::current_dir().map_err(Into::into))?;
    print_header();
    let config_path = match args.config {
        Some(p) => p,
        None => Config::find_config(&start_dir)?,
    };
    if args.verbose {
        println!(
            "Using config: {}\n",
            config_path.display().to_string().dimmed()
        );
    }
    let config = Config::load(&config_path)?;
    let linker = Linker::new(config, config_path);
    let use_color = human_use_color();
    if args.dry_run {
        print_lines(&render_dry_run_notice(use_color));
        println!();
    }
    let clean_result = if args.clean {
        print_lines(&render_clean_phase_with_color(args.dry_run, use_color));
        let clean_opts = SyncOptions {
            dry_run: args.dry_run,
            verbose: args.verbose,
            ..Default::default()
        };
        let clean_result = linker.clean(&clean_opts)?;
        println!();
        Some(clean_result)
    } else {
        None
    };
    print_lines(&render_sync_phase_with_color(
        args.dry_run,
        args.clean,
        use_color,
    ));
    let options = SyncOptions {
        clean: false,
        dry_run: args.dry_run,
        verbose: args.verbose,
        agents: args.agents,
    };
    let mut result = linker.sync(&options)?;
    if let Some(clean_result) = &clean_result {
        merge_clean_result_into_apply_result(&mut result, clean_result);
    }
    if !args.no_gitignore {
        handle_apply_gitignore(&linker, args.dry_run, use_color)?;
    }
    if linker.config().mcp.enabled && !linker.config().mcp_servers.is_empty() {
        handle_apply_mcp(
            &linker,
            options.dry_run,
            use_color,
            options.agents.as_ref(),
            &mut result,
        )?;
    }
    println!();
    print_lines(&render_apply_summary_with_color(
        options.dry_run,
        &result,
        use_color,
    ));
    Ok(())
}

fn handle_apply_gitignore(linker: &Linker, dry_run: bool, use_color: bool) -> Result<()> {
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
    Ok(())
}

fn handle_apply_mcp(
    linker: &Linker,
    dry_run: bool,
    use_color: bool,
    agents: Option<&Vec<String>>,
    result: &mut SyncResult,
) -> Result<()> {
    println!();
    print_lines(&render_mcp_phase(dry_run, use_color));
    match linker.sync_mcp(dry_run, agents) {
        Ok(mcp_result) => {
            if mcp_result.created > 0
                || mcp_result.updated > 0
                || mcp_result.skipped > 0
                || mcp_result.errors > 0
            {
                print_lines(&render_mcp_summary_with_color(&mcp_result, use_color));
            }
        }
        Err(e) => {
            tracing::error!(%e, "Error syncing MCP configs");
            result.errors += 1;
        }
    }
    Ok(())
}

fn handle_clean(
    path: Option<PathBuf>,
    config: Option<PathBuf>,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    let start_dir = current_project_root(path, || env::current_dir().map_err(Into::into))?;
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
        dry_run, &result, use_color,
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands, current_project_root};
    use crate::output::{
        init_next_steps_lines, render_apply_summary_with_color, render_clean_phase_with_color,
        render_clean_summary_with_color, render_gitignore_phase_with_color,
        render_mcp_summary_with_color, render_sync_phase_with_color,
    };
    use agentsync::{SyncResult, mcp::McpSyncResult};
    use clap::Parser;

    fn render_apply_summary(dry_run: bool, result: &SyncResult) -> Vec<String> {
        render_apply_summary_with_color(dry_run, result, false)
    }
    fn render_clean_phase(dry_run: bool) -> Vec<String> {
        render_clean_phase_with_color(dry_run, false)
    }
    fn render_clean_summary(dry_run: bool, result: &SyncResult) -> Vec<String> {
        render_clean_summary_with_color(dry_run, result, false)
    }
    fn render_dry_run_notice(use_color: bool) -> Vec<String> {
        super::output::render_dry_run_notice(use_color)
    }
    fn render_gitignore_phase(enabled: bool, dry_run: bool) -> Vec<String> {
        render_gitignore_phase_with_color(enabled, dry_run, false)
    }
    fn render_mcp_summary(result: &McpSyncResult) -> Vec<String> {
        render_mcp_summary_with_color(result, false)
    }
    fn render_sync_phase(dry_run: bool, clean_first: bool) -> Vec<String> {
        render_sync_phase_with_color(dry_run, clean_first, false)
    }

    #[test]
    fn current_project_root_reports_cwd_errors_with_context() {
        let error = current_project_root(None, || Err(anyhow::anyhow!("cwd unavailable")))
            .expect_err("cwd resolution should fail");

        assert_eq!(
            format!("{error:#}"),
            "failed to determine current project directory: cwd unavailable"
        );
    }

    #[test]
    fn current_project_root_uses_explicit_path_without_resolving_cwd() {
        let expected = std::path::PathBuf::from("/tmp/project");
        let actual = current_project_root(Some(expected.clone()), || {
            panic!("current directory should not be resolved when a path is provided")
        })
        .expect("explicit project path should be returned");

        assert_eq!(actual, expected);
    }

    #[test]
    fn current_project_root_returns_resolved_cwd() {
        let expected = std::path::PathBuf::from("/tmp/current-project");
        let actual = current_project_root(None, || Ok(expected.clone()))
            .expect("resolved current directory should be returned");

        assert_eq!(actual, expected);
    }

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
                "✗ Sync completed with errors".to_string(),
                "  Created: 2".to_string(),
                "  Updated: 1".to_string(),
                "  Skipped: 3".to_string(),
                "  Removed: 0".to_string(),
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
            render_clean_summary(
                false,
                &SyncResult {
                    removed: 3,
                    ..Default::default()
                }
            ),
            vec![
                "✔ Clean complete".to_string(),
                "  Removed: 3".to_string(),
                "  Errors: 0".to_string()
            ]
        );
        assert_eq!(
            render_clean_summary(
                true,
                &SyncResult {
                    removed: 3,
                    errors: 1,
                    ..Default::default()
                }
            ),
            vec![
                "✗ Clean dry run completed with errors".to_string(),
                "  Would remove: 3".to_string(),
                "  Errors: 1".to_string()
            ]
        );
    }

    #[test]
    fn test_render_mcp_summary_reports_all_counts() {
        let summary = render_mcp_summary(&McpSyncResult {
            created: 1,
            updated: 2,
            skipped: 3,
            errors: 4,
        });

        assert_eq!(
            summary,
            vec![
                "  Created: 1".to_string(),
                "  Updated: 2".to_string(),
                "  Skipped: 3".to_string(),
                "  Errors: 4".to_string(),
            ]
        );
    }

    #[test]
    fn test_merge_clean_result_into_apply_result_preserves_created_count() {
        let mut result = SyncResult {
            created: 3,
            updated: 5,
            skipped: 7,
            removed: 11,
            errors: 13,
        };
        let clean_result = SyncResult {
            created: 17,
            updated: 19,
            skipped: 23,
            removed: 29,
            errors: 31,
        };

        super::merge_clean_result_into_apply_result(&mut result, &clean_result);

        assert_eq!(result.created, 3);
        assert_eq!(result.updated, 24);
        assert_eq!(result.skipped, 30);
        assert_eq!(result.removed, 40);
        assert_eq!(result.errors, 44);
    }

    #[test]
    fn test_init_experimental_tui_requires_wizard_flag() {
        assert!(Cli::try_parse_from(["agentsync", "init", "--experimental-tui"]).is_err());
    }

    #[test]
    fn test_init_experimental_tui_parses_with_wizard_flag() {
        let cli = Cli::try_parse_from(["agentsync", "init", "--wizard", "--experimental-tui"])
            .expect("experimental TUI should parse when wizard is enabled");

        let Commands::Init {
            wizard,
            experimental_tui,
            ..
        } = cli.command
        else {
            panic!("expected init command");
        };

        assert!(wizard);
        assert!(experimental_tui);
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
