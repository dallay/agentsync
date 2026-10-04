//! AgentSync CLI
//!
//! Command-line interface for synchronizing AI agent configurations.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::env;
use std::path::PathBuf;

use agentsync::logging::LogFormat;
use agentsync::{
    Linker, PluginApplyMode, PluginManager, SyncOptions, SyncResult, config::Config, gitignore,
    init, plugins::PluginApplyResult,
};
use tracing_subscriber::filter::LevelFilter;
mod commands;
mod output;
use commands::dev_bench::{DevBenchArgs, run_dev_bench};
use commands::doctor::run_doctor;
use commands::plugin::{PluginCommand, run_plugin};
use commands::skill::{SkillCommand, run_skill};
use commands::status::{StatusArgs, run_status};
use output::{
    human_use_color, init_next_steps_lines, print_header, print_lines,
    render_apply_summary_with_color, render_clean_phase_with_color,
    render_clean_summary_with_color, render_dry_run_notice, render_gitignore_phase_with_color,
    render_mcp_phase, render_mcp_summary_with_color, render_revert_phase_with_color,
    render_revert_summary_with_color, render_sync_phase_with_color,
};

fn should_spawn_update_check(command: &Commands) -> bool {
    !matches!(command, Commands::Apply { offline: true, .. })
}

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
    result.restored += clean_result.restored;
    result.errors += clean_result.errors;
}

fn merge_plugin_result_into_apply_result(
    result: &mut SyncResult,
    plugin_result: &PluginApplyResult,
) {
    result.created += plugin_result.created;
    result.updated += plugin_result.updated;
    result.skipped += plugin_result.skipped;
    result.removed += plugin_result.removed;
    result.errors += plugin_result.errors;
}

// Logging is initialized in main via agentsync::logging::init_logging (stderr, human/json).

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

    /// Log event format: human (default) or json (newline-delimited JSON to stderr)
    #[arg(long, global = true, value_parser = parse_log_format, default_value = "human")]
    log_format: LogFormat,

    /// Minimum log level: trace, debug, info, warn, error, off (defaults to RUST_LOG or info)
    #[arg(long, global = true, value_parser = parse_log_level)]
    log_level: Option<LevelFilter>,
}

fn parse_log_format(s: &str) -> Result<LogFormat, String> {
    s.parse()
}

fn parse_log_level(s: &str) -> Result<LevelFilter, String> {
    s.parse().map_err(|_| {
        format!(
            "invalid log level '{s}' (expected 'trace', 'debug', 'info', 'warn', 'error', or 'off')"
        )
    })
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
    /// Manage repository-owned, vendor-neutral plugins.
    Plugin {
        #[command(subcommand)]
        cmd: PluginCommand,
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
        /// Do not use the network. Missing Git plugin snapshots fail instead of restoring.
        #[arg(long)]
        offline: bool,
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
    /// Restore pre-apply state: remove managed symlinks and restore .bak backups
    Revert {
        #[arg(short, long, alias = "project-root")]
        path: Option<PathBuf>,
        #[arg(short, long)]
        config: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
        #[arg(short, long)]
        verbose: bool,
        #[arg(short, long, value_delimiter = ',')]
        agents: Option<Vec<String>>,
        #[arg(long)]
        keep_backups: bool,
    },
    /// Developer-only: install a sample skill (dev)
    #[command(hide = true)]
    DevInstall {
        #[arg(help = "skill id to install")]
        skill_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Developer-only: run the linker performance benchmark (hidden)
    #[command(hide = true)]
    DevBench(DevBenchArgs),
}

fn main() {
    if run().is_err() {
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    // Initialize tracing subscriber for structured logging. Respects RUST_LOG env var.
    let cli = Cli::parse();
    agentsync::logging::init_logging(cli.log_format, cli.log_level);
    if should_spawn_update_check(&cli.command) {
        agentsync::update_check::spawn();
    }

    let result = match cli.command {
        Commands::Skill { cmd, project_root } => run_in_root_span("skill", || {
            let root =
                current_project_root(project_root, || env::current_dir().map_err(Into::into))?;
            run_skill(cmd, root)?;
            Ok(())
        }),
        Commands::Plugin { cmd, project_root } => run_in_root_span("plugin", || {
            let root =
                current_project_root(project_root, || env::current_dir().map_err(Into::into))?;
            let runtime = tokio::runtime::Runtime::new()
                .context("failed to create runtime for plugin command")?;
            runtime.block_on(run_plugin(cmd, root))
        }),
        Commands::Status { args, project_root } => run_in_root_span("status", || {
            let project_root =
                current_project_root(project_root, || env::current_dir().map_err(Into::into))?;
            run_status(args.json, project_root)?;
            Ok(())
        }),
        Commands::Doctor { project_root } => run_in_root_span("doctor", || {
            let project_root =
                current_project_root(project_root, || env::current_dir().map_err(Into::into))?;
            run_doctor(project_root)?;
            Ok(())
        }),
        Commands::Init {
            path,
            force,
            wizard,
            experimental_tui,
            template,
        } => run_in_root_span("init", || {
            handle_init(path, force, wizard, experimental_tui, template)?;
            Ok(())
        }),
        Commands::Apply {
            path,
            config,
            clean,
            dry_run,
            verbose,
            agents,
            no_gitignore,
            offline,
        } => run_in_root_span("apply", || {
            handle_apply(ApplyArgs {
                path,
                config,
                clean,
                dry_run,
                verbose,
                agents,
                no_gitignore,
                offline,
            })?;
            Ok(())
        }),
        Commands::Clean {
            path,
            config,
            dry_run,
            verbose,
        } => run_in_root_span("clean", || {
            handle_clean(path, config, dry_run, verbose)?;
            Ok(())
        }),
        Commands::Revert {
            path,
            config,
            dry_run,
            verbose,
            agents,
            keep_backups,
        } => run_in_root_span("revert", || {
            handle_revert(path, config, dry_run, verbose, agents, keep_backups)?;
            Ok(())
        }),
        Commands::DevInstall { skill_id, json } => run_in_root_span("skill", || {
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
            Ok(())
        }),
        Commands::DevBench(args) => run_in_root_span("dev-bench", || {
            run_dev_bench(args)?;
            Ok(())
        }),
    };
    if let Err(error) = &result {
        tracing::error!(error = %error, "Command failed");
    }
    result
}

/// Run `f` inside a root span named `agentsync` that records `outcome`
/// (ok/error) once it completes, so span-close JSON events expose the result.
fn run_in_root_span<F>(operation: &'static str, f: F) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    let span = tracing::info_span!("agentsync", operation, outcome = tracing::field::Empty);
    let result = span.in_scope(f);
    span.record("outcome", if result.is_ok() { "ok" } else { "error" });
    result
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
    offline: bool,
}

fn handle_apply(args: ApplyArgs) -> Result<()> {
    let start_dir = current_project_root(args.path, || env::current_dir().map_err(Into::into))?;
    print_header();
    let config_path = match args.config {
        Some(p) => p,
        None => Config::find_config(&start_dir)?,
    };
    if args.verbose {
        tracing::debug!(config_path = %config_path.display(), "Using config");
    }
    let config = Config::load(&config_path)?;
    let plugin_manager = PluginManager::new(
        Config::project_root(&config_path),
        config_path.clone(),
        config.plugins.clone(),
    );
    let plugin_result = plugin_manager.apply_with(PluginApplyMode {
        dry_run: args.dry_run,
        offline: args.offline,
    })?;
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
        keep_backups: false,
    };
    let mut result = linker.sync(&options)?;
    merge_plugin_result_into_apply_result(&mut result, &plugin_result);
    if let Some(clean_result) = &clean_result {
        merge_clean_result_into_apply_result(&mut result, clean_result);
    }
    if !args.no_gitignore {
        handle_apply_gitignore(&linker, args.dry_run, use_color)?;
    }
    if linker.config().mcp.enabled
        && (!linker.config().mcp_servers.is_empty() || !plugin_result.mcp_servers.is_empty())
    {
        handle_apply_mcp(
            &linker,
            options.dry_run,
            use_color,
            options.agents.as_ref(),
            &plugin_result.mcp_servers,
            &mut result,
        )?;
    }
    println!();
    print_lines(&render_apply_summary_with_color(
        options.dry_run,
        &result,
        use_color,
    ));
    if result.errors > 0 {
        return Err(anyhow::anyhow!(
            "apply completed with {} error(s)",
            result.errors
        ));
    }
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
    plugin_servers: &std::collections::BTreeMap<String, agentsync::config::McpServerConfig>,
    result: &mut SyncResult,
) -> Result<()> {
    println!();
    print_lines(&render_mcp_phase(dry_run, use_color));
    match linker.sync_mcp_with_servers(dry_run, agents, plugin_servers) {
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
            tracing::error!(
                config_path = %linker.config_path().display(),
                error = %e,
                "Error syncing MCP configs"
            );
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

/// Revert drops the managed gitignore block only on unfiltered runs: it is
/// filtered whenever `--agents` is passed OR `default_agents` is non-empty
/// and does not cover every configured agent (revert processes disabled agents
/// too, so their entries may still be needed).
fn revert_should_cleanup_gitignore(
    config: &Config,
    agents: &Option<Vec<String>>,
    result: &SyncResult,
) -> bool {
    if result.errors > 0 || result.skipped > 0 {
        return false;
    }
    if agents.is_some() {
        return false;
    }
    if config.default_agents.is_empty() {
        return true;
    }
    // Revert includes disabled agents, unlike apply; use its selector so this
    // gate accounts for every agent that the revert loop will process.
    let defaults_as_filter = SyncOptions {
        agents: Some(config.default_agents.clone()),
        ..Default::default()
    };
    config.agents.iter().all(|(name, _)| {
        agentsync::linker::revert_agent_selected(config, name, &defaults_as_filter)
    })
}

fn handle_revert(
    path: Option<PathBuf>,
    config: Option<PathBuf>,
    dry_run: bool,
    verbose: bool,
    agents: Option<Vec<String>>,
    keep_backups: bool,
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
    print_lines(&render_revert_phase_with_color(dry_run, use_color));
    let options = SyncOptions {
        dry_run,
        verbose,
        agents,
        keep_backups,
        ..Default::default()
    };
    let result = linker.revert(&options)?;
    // Clean up only after a complete, unfiltered revert: with --agents (or a
    // narrowing default_agents), other agents may still need their entries.
    if revert_should_cleanup_gitignore(linker.config(), &options.agents, &result) {
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
    println!();
    print_lines(&render_revert_summary_with_color(
        dry_run, &result, use_color,
    ));
    if result.errors > 0 {
        return Err(anyhow::anyhow!(
            "revert completed with {} error(s)",
            result.errors
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands, LogFormat, current_project_root, should_spawn_update_check};
    use crate::output::{
        init_next_steps_lines, render_apply_summary_with_color, render_clean_phase_with_color,
        render_clean_summary_with_color, render_gitignore_phase_with_color,
        render_mcp_summary_with_color, render_revert_phase_with_color,
        render_revert_summary_with_color, render_sync_phase_with_color,
    };
    use agentsync::{SyncResult, mcp::McpSyncResult};
    use clap::Parser;
    use tracing_subscriber::filter::LevelFilter;

    fn render_apply_summary(dry_run: bool, result: &SyncResult) -> Vec<String> {
        render_apply_summary_with_color(dry_run, result, false)
    }
    fn render_clean_phase(dry_run: bool) -> Vec<String> {
        render_clean_phase_with_color(dry_run, false)
    }
    fn render_revert_phase(dry_run: bool) -> Vec<String> {
        render_revert_phase_with_color(dry_run, false)
    }
    fn render_revert_summary(dry_run: bool, result: &SyncResult) -> Vec<String> {
        render_revert_summary_with_color(dry_run, result, false)
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
                restored: 0,
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
    fn test_render_revert_phase_and_summary_make_dry_run_clear() {
        assert_eq!(
            render_revert_phase(true),
            vec![
                "➤ Revert".to_string(),
                "  Previewing managed symlink restores".to_string()
            ]
        );
        assert_eq!(
            render_revert_summary(
                false,
                &SyncResult {
                    removed: 3,
                    restored: 2,
                    ..Default::default()
                },
            ),
            vec![
                "✔ Revert complete".to_string(),
                "  Removed: 3".to_string(),
                "  Restored: 2".to_string(),
                "  Errors: 0".to_string()
            ]
        );
        assert_eq!(
            render_revert_summary(
                true,
                &SyncResult {
                    removed: 3,
                    restored: 2,
                    errors: 1,
                    ..Default::default()
                },
            ),
            vec![
                "✗ Revert dry run completed with errors".to_string(),
                "  Would remove: 3".to_string(),
                "  Would restore: 2".to_string(),
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
            restored: 13,
            errors: 17,
        };
        let clean_result = SyncResult {
            created: 19,
            updated: 23,
            skipped: 29,
            removed: 31,
            restored: 37,
            errors: 41,
        };

        super::merge_clean_result_into_apply_result(&mut result, &clean_result);

        assert_eq!(result.created, 3);
        assert_eq!(result.updated, 28);
        assert_eq!(result.skipped, 36);
        assert_eq!(result.removed, 42);
        assert_eq!(result.restored, 50);
        assert_eq!(result.errors, 58);
    }

    #[test]
    fn test_merge_plugin_result_into_apply_result_preserves_all_counts() {
        let mut result = SyncResult {
            created: 3,
            updated: 5,
            skipped: 7,
            removed: 11,
            restored: 13,
            errors: 17,
        };
        let plugin_result = agentsync::plugins::PluginApplyResult {
            created: 17,
            updated: 19,
            skipped: 23,
            removed: 29,
            errors: 31,
            mcp_servers: Default::default(),
        };

        super::merge_plugin_result_into_apply_result(&mut result, &plugin_result);

        assert_eq!(result.created, 20);
        assert_eq!(result.updated, 24);
        assert_eq!(result.skipped, 30);
        assert_eq!(result.removed, 40);
        // Plugin results carry no restores: the apply-side value is preserved.
        assert_eq!(result.restored, 13);
        assert_eq!(result.errors, 48);
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

    #[test]
    fn cli_log_format_parses_json_after_subcommand() {
        let cli = Cli::try_parse_from(["agentsync", "apply", "--log-format", "json"])
            .expect("global --log-format should parse after the subcommand");
        assert_eq!(cli.log_format, LogFormat::Json);
    }

    #[test]
    fn cli_log_format_parses_json_before_subcommand() {
        let cli = Cli::try_parse_from(["agentsync", "--log-format", "json", "status"])
            .expect("global --log-format should parse before the subcommand");
        assert_eq!(cli.log_format, LogFormat::Json);
    }

    #[test]
    fn cli_log_format_defaults_to_human() {
        let cli = Cli::try_parse_from(["agentsync", "status"])
            .expect("status should parse without --log-format");
        assert_eq!(cli.log_format, LogFormat::Human);
    }

    #[test]
    fn cli_log_level_parses() {
        let cli = Cli::try_parse_from(["agentsync", "apply", "--log-level", "debug"])
            .expect("--log-level debug should parse");
        assert_eq!(cli.log_level, Some(LevelFilter::DEBUG));
    }

    #[test]
    fn cli_log_level_defaults_to_none() {
        let cli = Cli::try_parse_from(["agentsync", "status"])
            .expect("status should parse without --log-level");
        assert_eq!(cli.log_level, None);
    }

    #[test]
    fn cli_rejects_invalid_log_format() {
        assert!(Cli::try_parse_from(["agentsync", "status", "--log-format", "xml"]).is_err());
    }

    #[test]
    fn apply_offline_skips_update_checker() {
        let cli = Cli::try_parse_from(["agentsync", "apply", "--offline"])
            .expect("apply --offline should parse");
        assert!(!should_spawn_update_check(&cli.command));
        let online = Cli::try_parse_from(["agentsync", "apply"]).expect("apply should parse");
        assert!(should_spawn_update_check(&online.command));
        let dry_run = Cli::try_parse_from(["agentsync", "apply", "--dry-run"])
            .expect("apply --dry-run should parse");
        assert!(should_spawn_update_check(&dry_run.command));
    }

    fn make_revert_gate_config(
        default_agents: Vec<&str>,
        agents: Vec<(&str, bool)>,
    ) -> agentsync::config::Config {
        use agentsync::config::{AgentConfig, Config};
        use std::collections::BTreeMap;
        let mut map = BTreeMap::new();
        for (name, enabled) in agents {
            map.insert(
                name.to_string(),
                AgentConfig {
                    enabled,
                    description: String::new(),
                    targets: BTreeMap::new(),
                },
            );
        }
        Config {
            source_dir: ".".to_string(),
            compress_agents_md: false,
            default_agents: default_agents.into_iter().map(str::to_string).collect(),
            agents: map,
            gitignore: Default::default(),
            mcp: Default::default(),
            mcp_servers: Default::default(),
            plugins: Default::default(),
        }
    }

    #[test]
    fn revert_gitignore_cleanup_runs_when_unfiltered() {
        let config = make_revert_gate_config(vec![], vec![("claude", true), ("copilot", true)]);
        assert!(super::revert_should_cleanup_gitignore(
            &config,
            &None,
            &SyncResult::default()
        ));
    }

    #[test]
    fn revert_gitignore_cleanup_skips_with_agents_filter() {
        let config = make_revert_gate_config(vec![], vec![("claude", true), ("copilot", true)]);
        assert!(!super::revert_should_cleanup_gitignore(
            &config,
            &Some(vec!["claude".to_string()]),
            &SyncResult::default()
        ));
    }

    #[test]
    fn revert_gitignore_cleanup_skips_with_narrowing_default_agents() {
        let config =
            make_revert_gate_config(vec!["claude"], vec![("claude", true), ("copilot", true)]);
        assert!(!super::revert_should_cleanup_gitignore(
            &config,
            &None,
            &SyncResult::default()
        ));
    }

    #[test]
    fn revert_gitignore_cleanup_runs_when_default_agents_cover_all_configured_agents() {
        let config = make_revert_gate_config(
            vec!["claude", "copilot"],
            vec![("claude", true), ("copilot", true)],
        );
        assert!(super::revert_should_cleanup_gitignore(
            &config,
            &None,
            &SyncResult::default()
        ));
    }

    #[test]
    fn revert_gitignore_cleanup_respects_disabled_agent_selection() {
        // Revert still processes disabled `copilot`, so selecting only
        // `claude` narrows the actual revert scope and must keep gitignore.
        let config =
            make_revert_gate_config(vec!["claude"], vec![("claude", true), ("copilot", false)]);
        assert!(!super::revert_should_cleanup_gitignore(
            &config,
            &None,
            &SyncResult::default()
        ));
    }

    #[test]
    fn revert_gitignore_cleanup_skips_when_revert_has_errors() {
        let config = make_revert_gate_config(vec![], vec![("claude", true)]);
        let result = SyncResult {
            errors: 1,
            ..Default::default()
        };

        assert!(!super::revert_should_cleanup_gitignore(
            &config, &None, &result
        ));
    }

    #[test]
    fn revert_gitignore_cleanup_skips_when_revert_has_skips() {
        let config = make_revert_gate_config(vec![], vec![("claude", true)]);
        let result = SyncResult {
            skipped: 1,
            ..Default::default()
        };

        assert!(!super::revert_should_cleanup_gitignore(
            &config, &None, &result
        ));
    }
}
