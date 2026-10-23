//! Read-only inspection of materialized AgentSync destinations.
//!
//! This module owns the filesystem probes that back both `agentsync status` and
//! `agentsync inspect`. It lives in the library so that other commands (and future
//! stories such as profiles and capability adapters) can reuse the same reader
//! without duplicating drift detection.
//!
//! The types and functions here describe what AgentSync **materialized on disk**.
//! They never describe what a provider's runtime resolves to.

use crate::Linker;
use crate::config::{SyncType, TargetConfig};
use crate::linker::SymlinkContentsChildExpectation;
use crate::skills_layout::detect_skills_mode_mismatch;
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

/// A status entry together with the agent and target that declared it.
///
/// `collect_status_entries` flattens this shape for the `status` command. The
/// inspector needs the provenance keys, so the reader keeps them addressable.
#[derive(Debug, Clone)]
pub struct AgentStatusEntry {
    pub agent: String,
    pub target: String,
    pub entry: StatusEntry,
}

/// Collect the materialized destination entries for every enabled agent.
///
/// The order is deterministic: agents and targets are visited in `BTreeMap`
/// order, so the output never depends on hashing or filesystem order.
pub fn collect_status_entries(linker: &Linker, config_path: &Path) -> Result<Vec<StatusEntry>> {
    Ok(collect_agent_status_entries(linker, config_path)?
        .into_iter()
        .map(|entry| entry.entry)
        .collect())
}

/// Collect the materialized destination entries keyed by declaring agent and target.
pub fn collect_agent_status_entries(
    linker: &Linker,
    config_path: &Path,
) -> Result<Vec<AgentStatusEntry>> {
    let mut entries = Vec::new();
    let source_dir = linker.config().source_dir(config_path);

    for (agent_name, agent) in &linker.config().agents {
        if !agent.enabled {
            continue;
        }

        for (target_name, target) in &agent.targets {
            if target.sync_type == SyncType::ModuleMap {
                for mapping in &target.mappings {
                    let filename = crate::config::resolve_module_map_filename(mapping, agent_name);
                    let destination = linker
                        .project_root()
                        .join(&mapping.destination)
                        .join(&filename);
                    let source_path = source_dir.join(&mapping.source);
                    let expected_source = linker.expected_source_path(&source_path, target);

                    entries.push(AgentStatusEntry {
                        agent: agent_name.clone(),
                        target: target_name.clone(),
                        entry: validate_symlink_entry(
                            destination,
                            expected_source,
                            SyncType::ModuleMap,
                        ),
                    });
                }
                continue;
            }

            let destination = linker.project_root().join(&target.destination);
            // NestedGlob resolves `source` relative to the project root (matching apply and
            // doctor); all other sync types resolve it relative to source_dir.
            let source_path = if target.sync_type == SyncType::NestedGlob {
                linker.project_root().join(&target.source)
            } else {
                source_dir.join(&target.source)
            };

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
                    agent_name,
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

            entries.push(AgentStatusEntry {
                agent: agent_name.clone(),
                target: target_name.clone(),
                entry,
            });
        }
    }

    Ok(entries)
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

pub fn entry_is_problematic(entry: &StatusEntry) -> bool {
    !entry.issues.is_empty()
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

fn validate_symlink_contents_child(
    destination: &Path,
    child: SymlinkContentsChildExpectation,
) -> StatusChildEntry {
    let child_path = destination.join(&child.name);
    let metadata = std::fs::symlink_metadata(&child_path).ok();
    let kind = destination_kind(metadata.as_ref());
    let exists = metadata.is_some();
    let is_symlink = kind == DestinationKind::Symlink;
    let points_to = read_link_target(&child_path, is_symlink);

    StatusChildEntry {
        path: display_path(&child_path),
        exists,
        is_symlink,
        points_to,
        expected_source: display_path(&child.expected_source_path),
    }
}

fn collect_child_issues(child_status: &StatusChildEntry, issues: &mut Vec<StatusIssue>) {
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
}

fn validate_children_directory(
    destination: &Path,
    destination_kind: DestinationKind,
    exists: bool,
    children: Vec<SymlinkContentsChildExpectation>,
    issues: &mut Vec<StatusIssue>,
    managed_children: &mut Vec<StatusChildEntry>,
) {
    if !exists {
        issues.push(StatusIssue {
            kind: StatusIssueKind::MissingDestination,
            path: display_path(destination),
            expected: Some("directory".to_string()),
            actual: None,
        });
    } else if destination_kind != DestinationKind::Directory {
        issues.push(StatusIssue {
            kind: StatusIssueKind::InvalidDestinationType,
            path: display_path(destination),
            expected: Some("directory".to_string()),
            actual: Some(destination_kind_label(destination_kind).to_string()),
        });
    } else {
        for child in children {
            let child_status = validate_symlink_contents_child(destination, child);
            collect_child_issues(&child_status, issues);
            managed_children.push(child_status);
        }
    }
}

fn validate_symlink_contents_entry(
    linker: &Linker,
    destination: PathBuf,
    source_path: &Path,
    target: &TargetConfig,
    agent_name: &str,
    allow_skills_symlink_hint: bool,
) -> Result<StatusEntry> {
    let metadata = std::fs::symlink_metadata(&destination).ok();
    let destination_kind = destination_kind(metadata.as_ref());
    let exists = metadata.is_some();
    let is_symlink = destination_kind == DestinationKind::Symlink;
    let points_to = read_link_target(&destination, is_symlink);
    let child_expectations =
        linker.symlink_contents_expected_children(source_path, target, agent_name)?;
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

        validate_children_directory(
            &destination,
            destination_kind,
            exists,
            children,
            &mut issues,
            &mut managed_children,
        );
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

fn destination_kind(metadata: Option<&std::fs::Metadata>) -> DestinationKind {
    match metadata {
        None => DestinationKind::Missing,
        Some(meta) => {
            let file_type = meta.file_type();
            if file_type.is_symlink() {
                DestinationKind::Symlink
            } else if file_type.is_dir() {
                DestinationKind::Directory
            } else if file_type.is_file() {
                DestinationKind::File
            } else {
                DestinationKind::Other
            }
        }
    }
}

fn read_link_target(path: &Path, is_symlink: bool) -> Option<String> {
    if !is_symlink {
        return None;
    }
    std::fs::read_link(path)
        .ok()
        .map(|target| display_path(&target))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
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

fn sync_type_label(sync_type: SyncType) -> &'static str {
    match sync_type {
        SyncType::Symlink => "symlink",
        SyncType::SymlinkContents => "symlink-contents",
        SyncType::NestedGlob => "nested-glob",
        SyncType::ModuleMap => "module-map",
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
