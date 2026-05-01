use crate::output::{HumanFormatter, LabelKind, OutputMode};
use agentsync::config::{SyncType, TargetConfig};
use agentsync::skills_layout::detect_skills_mode_mismatch;
use agentsync::{Linker, linker::SymlinkContentsChildExpectation};
use anyhow::Result;
use clap::Args;
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

    let use_color = match crate::output::output_mode(json) {
        OutputMode::Json => false,
        OutputMode::Human { use_color } => use_color,
    };
    let formatter = HumanFormatter::new(use_color);

    for entry in &entries {
        for line in render_status_entry(entry, &formatter) {
            println!("{line}");
        }

        if !entry_is_problematic(entry)
            && let Some(hint) = hints.get(&entry.destination)
        {
            println!("  {}", render_status_hint(hint, &formatter));
        }
    }

    if problems > 0 {
        println!("\n{}", render_status_summary(problems, &formatter));
        std::process::exit(1);
    }

    println!("\n{}", render_status_summary(0, &formatter));
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

pub(crate) fn render_status_entry(entry: &StatusEntry, formatter: &HumanFormatter) -> Vec<String> {
    if entry.issues.is_empty() {
        return render_ok_line(entry, formatter);
    }

    entry
        .issues
        .iter()
        .flat_map(|issue| render_issue_line(entry, issue, formatter))
        .collect()
}

fn render_ok_line(entry: &StatusEntry, formatter: &HumanFormatter) -> Vec<String> {
    match entry.sync_type.as_str() {
        "symlink-contents" => {
            let managed_count = entry.managed_children.as_ref().map_or(0, Vec::len);
            if entry.destination_kind == DestinationKind::Directory {
                vec![
                    format!(
                        "{}: {}",
                        formatter.format_label("✔", "OK", LabelKind::Success),
                        entry.destination
                    ),
                    format!(
                        "  {}",
                        formatter.format_key_value("type", "symlink-contents container")
                    ),
                    format!(
                        "  {}",
                        formatter.format_key_value(
                            "managed entries expected",
                            &managed_count.to_string()
                        )
                    ),
                ]
            } else {
                vec![format!(
                    "{}: {}",
                    formatter.format_label("✔", "OK", LabelKind::Success),
                    entry.destination
                )]
            }
        }
        _ => vec![format!(
            "{}: {} -> {}",
            formatter.format_label("✔", "OK", LabelKind::Success),
            entry.destination,
            entry.points_to.as_deref().unwrap_or("<unknown>")
        )],
    }
}

fn render_issue_line(
    entry: &StatusEntry,
    issue: &StatusIssue,
    formatter: &HumanFormatter,
) -> Vec<String> {
    let mut lines = match issue.kind {
        StatusIssueKind::MissingDestination => match entry.sync_type.as_str() {
            "symlink-contents" => vec![format!(
                "{}: {} missing managed container directory",
                formatter.format_label("✗", "Drift", LabelKind::Failure),
                entry.destination
            )],
            _ => vec![format!(
                "{}: {}",
                formatter.format_label("!", "Missing", LabelKind::Warning),
                entry.destination
            )],
        },
        StatusIssueKind::InvalidDestinationType => {
            if entry.sync_type.as_str() == "symlink-contents" {
                vec![format!(
                    "{}: {} exists as {} but symlink-contents expects a directory container",
                    formatter.format_label("✗", "Drift", LabelKind::Failure),
                    entry.destination,
                    issue.actual.as_deref().unwrap_or("an invalid path type")
                )]
            } else {
                vec![format!(
                    "{}: {}",
                    formatter.format_label("·", "Exists but not a symlink", LabelKind::Muted),
                    entry.destination
                )]
            }
        }
        StatusIssueKind::MissingExpectedChild => vec![format!(
            "{}: {} missing managed child {}",
            formatter.format_label("✗", "Drift", LabelKind::Failure),
            entry.destination,
            Path::new(&issue.path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&issue.path)
        )],
        StatusIssueKind::ChildNotSymlink => vec![format!(
            "{}: {} exists but is not a symlink",
            formatter.format_label("✗", "Drift", LabelKind::Failure),
            issue.path
        )],
        StatusIssueKind::IncorrectLinkTarget => {
            if issue.path == entry.destination {
                vec![format!(
                    "{}: {}",
                    formatter.format_label("✗", "Incorrect link", LabelKind::Failure),
                    entry.destination
                )]
            } else {
                vec![format!(
                    "{}: {}",
                    formatter.format_label("✗", "Drift", LabelKind::Failure),
                    issue.path
                )]
            }
        }
        StatusIssueKind::MissingExpectedSource => {
            if entry.sync_type.as_str() == "symlink-contents" {
                vec![format!(
                    "{}: {}",
                    formatter.format_label(
                        "!",
                        "Missing source container directory",
                        LabelKind::Warning
                    ),
                    entry.destination
                )]
            } else {
                vec![format!(
                    "{}: {}",
                    formatter.format_label(
                        "!",
                        "Link points to missing source",
                        LabelKind::Warning
                    ),
                    issue.actual.as_deref().unwrap_or(&entry.destination)
                )]
            }
        }
    };

    lines.extend(issue_detail_lines(issue, formatter));
    lines
}

fn issue_detail_lines(issue: &StatusIssue, formatter: &HumanFormatter) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(actual) = issue.actual.as_deref() {
        lines.push(format!(
            "  {}",
            formatter.format_key_value("actual", actual)
        ));
    }
    if let Some(expected) = issue.expected.as_deref() {
        lines.push(format!(
            "  {}",
            formatter.format_key_value("expected", expected)
        ));
    }
    lines
}

pub(crate) fn render_status_hint(hint: &str, formatter: &HumanFormatter) -> String {
    formatter.format_hint(hint)
}

pub(crate) fn render_status_summary(problems: usize, formatter: &HumanFormatter) -> String {
    if problems == 0 {
        formatter.format_summary_line("Status", "All good", LabelKind::Success)
    } else {
        formatter.format_summary_line(
            "Status",
            &format!("{problems} problems found"),
            LabelKind::Failure,
        )
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
