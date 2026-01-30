use agentsync::Linker;
use anyhow::Result;
use clap::Args;
use colored::Colorize;
use serde::Serialize;
use std::path::PathBuf;

/// Arguments for the status command
#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Project root (defaults to CWD)
    #[arg(long)]
    pub project_root: Option<PathBuf>,

    /// Output machine-readable JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Serialize)]
struct StatusEntry {
    destination: String,
    exists: bool,
    is_symlink: bool,
    points_to: Option<String>,
    expected_source: Option<String>,
}

pub fn run_status(args: StatusArgs, project_root: PathBuf) -> Result<()> {
    // Find and load config
    let config_path = if let Some(p) = args.project_root.as_ref() {
        agentsync::config::Config::find_config(p)?
    } else {
        agentsync::config::Config::find_config(&project_root)?
    };

    let config = agentsync::config::Config::load(&config_path)?;
    let linker = Linker::new(config, config_path.clone());

    let mut entries: Vec<StatusEntry> = Vec::new();

    for agent in linker.config().agents.values() {
        if !agent.enabled {
            continue;
        }
        for target in agent.targets.values() {
            let dest = linker.project_root().join(&target.destination);
            // Access source dir via public API on Config
            let source = linker
                .config()
                .source_dir(&config_path)
                .join(&target.source);

            let exists = dest.exists();
            let is_symlink = dest.is_symlink();
            let mut points_to = None;
            if is_symlink && let Ok(link) = std::fs::read_link(&dest) {
                points_to = Some(link.display().to_string());
            }

            let expected = if source.exists() {
                Some(source.display().to_string())
            } else {
                None
            };

            entries.push(StatusEntry {
                destination: dest.display().to_string(),
                exists,
                is_symlink,
                points_to,
                expected_source: expected,
            });
        }
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        let mut problems = 0usize;
        for e in &entries {
            if !e.exists {
                println!("{} Missing: {}", "!".yellow(), e.destination);
                problems += 1;
            } else if e.is_symlink {
                if let Some(expected) = &e.expected_source {
                    if let Some(points) = &e.points_to {
                        if !points.contains(expected) {
                            println!(
                                "{} Incorrect link: {} -> {} (expected: {})",
                                "✗".red(),
                                e.destination,
                                points,
                                expected
                            );
                            problems += 1;
                        } else {
                            println!("{} OK: {} -> {}", "✔".green(), e.destination, points);
                        }
                    } else {
                        println!("{} Unknown link target: {}", "?".yellow(), e.destination);
                        problems += 1;
                    }
                } else {
                    println!(
                        "{} Link points to missing source: {} -> {}",
                        "!".yellow(),
                        e.destination,
                        e.points_to.as_deref().unwrap_or("<unknown>")
                    );
                    problems += 1;
                }
            } else {
                println!(
                    "{} Exists but not a symlink: {}",
                    "·".dimmed(),
                    e.destination
                );
            }
        }

        if problems > 0 {
            println!("\nStatus: {} problems found", problems);
            std::process::exit(1);
        } else {
            println!("\nStatus: All good");
        }
    }

    Ok(())
}
