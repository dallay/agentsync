use agentsync::Linker;
use anyhow::Result;
use clap::Args;
use colored::Colorize;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Arguments for the status command
#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Output machine-readable JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Serialize, Debug, Clone)]
pub struct StatusEntry {
    pub destination: String,
    pub exists: bool,
    pub is_symlink: bool,
    pub points_to: Option<String>,
    pub expected_source: Option<String>,
}

pub fn run_status(json: bool, project_root: PathBuf) -> Result<()> {
    // Find and load config
    // project_root is already resolved by the caller
    let config_path = agentsync::config::Config::find_config(&project_root)?;

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

            let metadata = std::fs::symlink_metadata(&dest);
            let exists = metadata.is_ok();
            let is_symlink = metadata
                .as_ref()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);
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

    // helper to canonicalize with a sensible fallback
    // exposed at module level for unit testing
    fn canonicalize_fallback_local(p: &Path, base: Option<&Path>) -> Option<PathBuf> {
        let candidate = if p.is_absolute() {
            p.to_path_buf()
        } else if let Some(b) = base {
            b.join(p)
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(p)
        } else {
            p.to_path_buf()
        };

        // Prefer canonicalized path; if that fails, fall back to the candidate path
        Some(std::fs::canonicalize(&candidate).unwrap_or(candidate))
    }

    // predicate to determine whether an entry represents a problem
    let is_problematic =
        |e: &StatusEntry| -> bool { entry_is_problematic(e, canonicalize_fallback_local) };

    if json {
        let has_problems = entries.iter().any(is_problematic);
        println!("{}", serde_json::to_string_pretty(&entries)?);
        if has_problems {
            // exit with non-zero to indicate problems when JSON output is requested
            std::process::exit(1);
        }
    } else {
        let mut problems = 0usize;
        for e in &entries {
            if !e.exists {
                println!("{} Missing: {}", "!".yellow(), e.destination);
                problems += 1;
            } else if e.is_symlink {
                if let Some(expected) = &e.expected_source {
                    if let Some(points) = &e.points_to {
                        // canonicalize both sides and compare for equality
                        let dest_path = PathBuf::from(&e.destination);
                        let expected_pb = PathBuf::from(expected);
                        let points_pb = PathBuf::from(points);
                        let expected_canon = canonicalize_fallback_local(&expected_pb, None);
                        let points_canon =
                            canonicalize_fallback_local(&points_pb, dest_path.parent());

                        let equal = match (expected_canon, points_canon) {
                            (Some(a), Some(b)) => a == b,
                            _ => false,
                        };

                        if !equal {
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
                problems += 1;
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

// Extracted logic for testability: determines whether an entry is problematic
pub fn entry_is_problematic<F>(e: &StatusEntry, canonicalize: F) -> bool
where
    F: Fn(&Path, Option<&Path>) -> Option<PathBuf>,
{
    if !e.exists {
        return true;
    }
    if e.is_symlink {
        if let Some(expected) = &e.expected_source {
            if let Some(points) = &e.points_to {
                // canonicalize both sides and compare equality
                let dest_path = PathBuf::from(&e.destination);
                let expected_pb = PathBuf::from(expected);
                let points_pb = PathBuf::from(points);
                let expected_canon = canonicalize(&expected_pb, None);
                let points_canon = canonicalize(&points_pb, dest_path.parent());
                match (expected_canon, points_canon) {
                    (Some(a), Some(b)) => return a != b,
                    // if we couldn't canonicalize, treat as problematic
                    _ => return true,
                }
            } else {
                return true; // unknown link target
            }
        } else {
            return true; // link points to missing source
        }
    } else {
        return true; // exists but not a symlink
    }
    // unreachable
    #[allow(unreachable_code)]
    false
}
