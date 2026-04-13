use agentsync::config::{SyncType, TargetConfig};
use agentsync::skills_layout::detect_skills_mode_mismatch;
use agentsync::{Linker, linker::SymlinkContentsChildExpectation};
use anyhow::Result;
use clap::Args;
use colored::Colorize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Arguments for the status command
#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Output machine-readable JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DestinationKind {
    Missing,
    Symlink,
    Directory,
    File,
    Other,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StatusIssueKind {
    MissingDestination,
    InvalidDestinationType,
    MissingExpectedChild,
    ChildNotSymlink,
    IncorrectLinkTarget,
    MissingExpectedSource,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct StatusIssue {
    pub kind: StatusIssueKind,
    pub path: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct StatusChildEntry {
    pub path: String,
    pub exists: bool,
    pub is_symlink: bool,
    pub points_to: Option<String>,
    pub expected_source: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct StatusEntry {
    pub destination: String,
    pub sync_type: String,
    pub destination_kind: DestinationKind,
    pub exists: bool,
    pub is_symlink: bool,
    pub points_to: Option<String>,
    pub expected_source: Option<String>,
    pub issues: Vec<StatusIssue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub managed_children: Option<Vec<StatusChildEntry>>,
}

pub fn collect_status_entries(linker: &Linker, config_path: &Path) -> Result<Vec<StatusEntry>> {
    let mut entries = Vec::new();
    let source_dir = linker.config().source_dir(config_path);

    for (agent_name, agent) in &linker.config().agents {
        if !agent.enabled {
            continue;
        }

        for (target_name, target) in &agent.targets {
            if target.sync_type == SyncType::ModuleMap {
                for mapping in &target.mappings {
                    let filename =
                        agentsync::config::resolve_module_map_filename(mapping, agent_name);
                    let destination = linker
                        .project_root()
                        .join(&mapping.destination)
                        .join(&filename);
                    let source_path = source_dir.join(&mapping.source);
                    let expected_source = linker.expected_source_path(&source_path, target);

                    entries.push(validate_symlink_entry(
                        destination,
                        expected_source,
                        SyncType::ModuleMap,
                    ));
                }
                continue;
            }

            let destination = linker.project_root().join(&target.destination);
            let source_path = source_dir.join(&target.source);

            let entry = match target.sync_type {
                SyncType::Symlink => validate_symlink_entry(
                    destination,
                    linker.expected_source_path(&source_path, target),
                    SyncType::Symlink,
                ),
                SyncType::SymlinkContents => validate_symlink_contents_entry(
                    linker,
                    destination,
                    &source_path,
                    target,
                    detect_skills_mode_mismatch(
                        linker.project_root(),
                        &source_path,
                        agent_name,
                        target_name,
                        target,
                    )
                    .is_some(),
                )?,
                SyncType::NestedGlob => validate_symlink_entry(
                    destination,
                    linker.expected_source_path(&source_path, target),
                    SyncType::NestedGlob,
                ),
                SyncType::ModuleMap => unreachable!(),
            };

            entries.push(entry);
        }
    }

    Ok(entries)
}

pub fn run_status(json: bool, project_root: PathBuf) -> Result<()> {
    let config_path = agentsync::config::Config::find_config(&project_root)?;
    let config = agentsync::config::Config::load(&config_path)?;
    let linker = Linker::new(config, config_path.clone());

    let entries = collect_status_entries(&linker, &config_path)?;
    let hints = collect_status_hints(&linker, &config_path);
    let problems = entries
        .iter()
        .filter(|entry| entry_is_problematic(entry))
        .count();

    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        if problems > 0 {
            std::process::exit(1);
        }
        return Ok(());
    }

    for entry in &entries {
        for line in render_status_entry(entry) {
            println!("{line}");
        }

        if !entry_is_problematic(entry)
            && let Some(hint) = hints.get(&entry.destination)
        {
            println!("  {} {}", "↳".blue(), hint);
        }
    }

    if problems > 0 {
        println!("\nStatus: {} problems found", problems);
        std::process::exit(1);
    }

    println!("\nStatus: All good");
    Ok(())
}

pub fn collect_status_hints(linker: &Linker, config_path: &Path) -> HashMap<String, String> {
    let mut hints = HashMap::new();
    let source_dir = linker.config().source_dir(config_path);

    for (agent_name, agent) in &linker.config().agents {
        if !agent.enabled {
            continue;
        }

        for (target_name, target) in &agent.targets {
            let expected_source = source_dir.join(&target.source);
            if let Some(mismatch) = detect_skills_mode_mismatch(
                linker.project_root(),
                &expected_source,
                agent_name,
                target_name,
                target,
            ) {
                hints.insert(
                    linker
                        .project_root()
                        .join(&target.destination)
                        .display()
                        .to_string(),
                    mismatch.status_hint(),
                );
            }
        }
    }

    hints
}

pub fn entry_is_problematic(e: &StatusEntry) -> bool {
    !e.issues.is_empty()
}

pub(crate) fn render_status_entry(entry: &StatusEntry) -> Vec<String> {
    if entry.issues.is_empty() {
        return vec![render_ok_line(entry)];
    }

    entry
        .issues
        .iter()
        .map(|issue| render_issue_line(entry, issue))
        .collect()
}

fn render_ok_line(entry: &StatusEntry) -> String {
    match entry.sync_type.as_str() {
        "symlink-contents" => {
            let managed_count = entry.managed_children.as_ref().map_or(0, Vec::len);
            if entry.destination_kind == DestinationKind::Directory {
                format!(
                    "{} OK: {} (symlink-contents container, {} managed entries expected)",
                    "✔".green(),
                    entry.destination,
                    managed_count
                )
            } else {
                format!("{} OK: {}", "✔".green(), entry.destination)
            }
        }
        _ => format!(
            "{} OK: {} -> {}",
            "✔".green(),
            entry.destination,
            entry.points_to.as_deref().unwrap_or("<unknown>")
        ),
    }
}

fn render_issue_line(entry: &StatusEntry, issue: &StatusIssue) -> String {
    match issue.kind {
        StatusIssueKind::MissingDestination => match entry.sync_type.as_str() {
            "symlink-contents" => format!(
                "{} Drift: {} missing managed container directory",
                "✗".red(),
                entry.destination
            ),
            _ => format!("{} Missing: {}", "!".yellow(), entry.destination),
        },
        StatusIssueKind::InvalidDestinationType => {
            if entry.sync_type == "symlink-contents" {
                format!(
                    "{} Drift: {} exists as {} but symlink-contents expects a directory container",
                    "✗".red(),
                    entry.destination,
                    issue.actual.as_deref().unwrap_or("an invalid path type")
                )
            } else {
                format!(
                    "{} Exists but not a symlink: {}",
                    "·".dimmed(),
                    entry.destination
                )
            }
        }
        StatusIssueKind::MissingExpectedChild => format!(
            "{} Drift: {} missing managed child {}",
            "✗".red(),
            entry.destination,
            Path::new(&issue.path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&issue.path)
        ),
        StatusIssueKind::ChildNotSymlink => format!(
            "{} Drift: {} exists but is not a symlink",
            "✗".red(),
            issue.path
        ),
        StatusIssueKind::IncorrectLinkTarget => {
            if issue.path == entry.destination {
                format!(
                    "{} Incorrect link: {} -> {} (expected: {})",
                    "✗".red(),
                    entry.destination,
                    issue.actual.as_deref().unwrap_or("<unknown>"),
                    issue.expected.as_deref().unwrap_or("<unknown>")
                )
            } else {
                format!(
                    "{} Drift: {} points to {} (expected: {})",
                    "✗".red(),
                    issue.path,
                    issue.actual.as_deref().unwrap_or("<unknown>"),
                    issue.expected.as_deref().unwrap_or("<unknown>")
                )
            }
        }
        StatusIssueKind::MissingExpectedSource => format!(
            "{} Link points to missing source: {}",
            "!".yellow(),
            issue.actual.as_deref().unwrap_or(&entry.destination)
        ),
    }
}

fn validate_symlink_entry(
    destination: PathBuf,
    expected_source: Option<PathBuf>,
    sync_type: SyncType,
) -> StatusEntry {
    let metadata = std::fs::symlink_metadata(&destination).ok();
    let destination_kind = destination_kind(metadata.as_ref());
    let exists = metadata.is_some();
    let is_symlink = destination_kind == DestinationKind::Symlink;
    let points_to = read_link_target(&destination, is_symlink);
    let expected_source_string = expected_source.as_ref().map(|path| display_path(path));
    let mut issues = Vec::new();

    match expected_source.as_ref() {
        Some(expected) => {
            if !exists {
                issues.push(StatusIssue {
                    kind: StatusIssueKind::MissingDestination,
                    path: display_path(&destination),
                    expected: Some(display_path(expected)),
                    actual: None,
                });
            } else if !is_symlink {
                issues.push(StatusIssue {
                    kind: StatusIssueKind::InvalidDestinationType,
                    path: display_path(&destination),
                    expected: Some("symlink".to_string()),
                    actual: Some(destination_kind_label(destination_kind).to_string()),
                });
            } else if let Some(actual) = points_to.as_ref() {
                if !paths_match(&destination, Path::new(actual), expected) {
                    issues.push(StatusIssue {
                        kind: StatusIssueKind::IncorrectLinkTarget,
                        path: display_path(&destination),
                        expected: Some(display_path(expected)),
                        actual: Some(actual.clone()),
                    });
                }
            } else {
                issues.push(StatusIssue {
                    kind: StatusIssueKind::IncorrectLinkTarget,
                    path: display_path(&destination),
                    expected: Some(display_path(expected)),
                    actual: None,
                });
            }
        }
        None => {
            issues.push(StatusIssue {
                kind: StatusIssueKind::MissingExpectedSource,
                path: display_path(&destination),
                expected: None,
                actual: points_to.clone(),
            });
        }
    }

    StatusEntry {
        destination: display_path(&destination),
        sync_type: sync_type_label(sync_type).to_string(),
        destination_kind,
        exists,
        is_symlink,
        points_to,
        expected_source: expected_source_string,
        issues,
        managed_children: None,
    }
}

fn validate_symlink_contents_entry(
    linker: &Linker,
    destination: PathBuf,
    source_path: &Path,
    target: &TargetConfig,
    allow_skills_symlink_hint: bool,
) -> Result<StatusEntry> {
    let metadata = std::fs::symlink_metadata(&destination).ok();
    let destination_kind = destination_kind(metadata.as_ref());
    let exists = metadata.is_some();
    let is_symlink = destination_kind == DestinationKind::Symlink;
    let points_to = read_link_target(&destination, is_symlink);
    let child_expectations = linker.symlink_contents_expected_children(source_path, target)?;
    let mut issues = Vec::new();
    let mut managed_children = Vec::new();

    if let Some(children) = child_expectations {
        if allow_skills_symlink_hint && is_symlink {
            return Ok(StatusEntry {
                destination: display_path(&destination),
                sync_type: sync_type_label(SyncType::SymlinkContents).to_string(),
                destination_kind,
                exists,
                is_symlink,
                points_to,
                expected_source: None,
                issues,
                managed_children: Some(managed_children),
            });
        }

        if !exists {
            issues.push(StatusIssue {
                kind: StatusIssueKind::MissingDestination,
                path: display_path(&destination),
                expected: Some("directory".to_string()),
                actual: None,
            });
        } else if destination_kind != DestinationKind::Directory {
            issues.push(StatusIssue {
                kind: StatusIssueKind::InvalidDestinationType,
                path: display_path(&destination),
                expected: Some("directory".to_string()),
                actual: Some(destination_kind_label(destination_kind).to_string()),
            });
        } else {
            for child in children {
                let child_status = validate_symlink_contents_child(&destination, child);
                if !child_status.exists {
                    issues.push(StatusIssue {
                        kind: StatusIssueKind::MissingExpectedChild,
                        path: child_status.path.clone(),
                        expected: Some(child_status.expected_source.clone()),
                        actual: None,
                    });
                } else if !child_status.is_symlink {
                    issues.push(StatusIssue {
                        kind: StatusIssueKind::ChildNotSymlink,
                        path: child_status.path.clone(),
                        expected: Some(child_status.expected_source.clone()),
                        actual: None,
                    });
                } else if let Some(actual) = child_status.points_to.as_ref()
                    && !paths_match(
                        Path::new(&child_status.path),
                        Path::new(actual),
                        Path::new(&child_status.expected_source),
                    )
                {
                    issues.push(StatusIssue {
                        kind: StatusIssueKind::IncorrectLinkTarget,
                        path: child_status.path.clone(),
                        expected: Some(child_status.expected_source.clone()),
                        actual: Some(actual.clone()),
                    });
                }

                managed_children.push(child_status);
            }
        }
    } else {
        issues.push(StatusIssue {
            kind: StatusIssueKind::MissingExpectedSource,
            path: display_path(source_path),
            expected: None,
            actual: Some(display_path(&destination)),
        });
    }

    Ok(StatusEntry {
        destination: display_path(&destination),
        sync_type: sync_type_label(SyncType::SymlinkContents).to_string(),
        destination_kind,
        exists,
        is_symlink,
        points_to,
        expected_source: None,
        issues,
        managed_children: Some(managed_children),
    })
}

fn validate_symlink_contents_child(
    destination: &Path,
    expectation: SymlinkContentsChildExpectation,
) -> StatusChildEntry {
    let child_path = destination.join(&expectation.name);
    let metadata = std::fs::symlink_metadata(&child_path).ok();
    let exists = metadata.is_some();
    let is_symlink = metadata
        .as_ref()
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false);

    StatusChildEntry {
        path: display_path(&child_path),
        exists,
        is_symlink,
        points_to: read_link_target(&child_path, is_symlink),
        expected_source: display_path(&expectation.expected_source_path),
    }
}

fn read_link_target(path: &Path, is_symlink: bool) -> Option<String> {
    if !is_symlink {
        return None;
    }

    std::fs::read_link(path)
        .ok()
        .map(|link| link.display().to_string())
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn destination_kind(metadata: Option<&std::fs::Metadata>) -> DestinationKind {
    match metadata {
        None => DestinationKind::Missing,
        Some(metadata) if metadata.file_type().is_symlink() => DestinationKind::Symlink,
        Some(metadata) if metadata.is_dir() => DestinationKind::Directory,
        Some(metadata) if metadata.is_file() => DestinationKind::File,
        Some(_) => DestinationKind::Other,
    }
}

fn destination_kind_label(kind: DestinationKind) -> &'static str {
    match kind {
        DestinationKind::Missing => "missing",
        DestinationKind::Symlink => "symlink",
        DestinationKind::Directory => "directory",
        DestinationKind::File => "file",
        DestinationKind::Other => "other",
    }
}

fn sync_type_label(sync_type: SyncType) -> &'static str {
    match sync_type {
        SyncType::Symlink => "symlink",
        SyncType::SymlinkContents => "symlink-contents",
        SyncType::NestedGlob => "nested-glob",
        SyncType::ModuleMap => "module-map",
    }
}

fn paths_match(destination: &Path, points_to: &Path, expected: &Path) -> bool {
    let actual_path = if points_to.is_absolute() {
        points_to.to_path_buf()
    } else {
        destination
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(points_to)
    };

    let expected_canon = std::fs::canonicalize(expected).unwrap_or_else(|_| expected.to_path_buf());
    let actual_canon = std::fs::canonicalize(&actual_path).unwrap_or(actual_path);
    expected_canon == actual_canon
}
