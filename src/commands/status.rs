use crate::output::{HumanFormatter, LabelKind, OutputMode};
use agentsync::Linker;
use anyhow::Result;
use clap::Args;
use std::path::{Path, PathBuf};

pub use agentsync::status::{
    DestinationKind, StatusEntry, StatusIssue, StatusIssueKind, collect_status_entries,
    collect_status_hints, entry_is_problematic,
};

/// Arguments for the status command
#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Output machine-readable JSON
    #[arg(long)]
    pub json: bool,
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
        return if problems > 0 {
            Err(anyhow::anyhow!("status found {problems} problem(s)"))
        } else {
            Ok(())
        };
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
        return Err(anyhow::anyhow!("status found {problems} problem(s)"));
    }

    println!("\n{}", render_status_summary(0, &formatter));
    Ok(())
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
