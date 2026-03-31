use agentsync::Linker;
use anyhow::Result;
use colored::Colorize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use agentsync::config::{SyncType, TargetConfig};
use agentsync::skills_layout::{SkillsModeMismatch, detect_skills_mode_mismatch};

#[derive(Debug, PartialEq, Eq)]
pub struct MissingSourceIssue {
    pub agent: String,
    pub target: String,
    pub mapping: Option<String>,
    pub path: PathBuf,
}

pub fn run_doctor(project_root: PathBuf) -> Result<()> {
    println!("{}", "🩺 Running AgentSync Diagnostic...".bold().cyan());

    let mut issues = 0;

    // 1. Config Loading & Validation
    let config_path = match agentsync::config::Config::find_config(&project_root) {
        Ok(path) => {
            println!(
                "  {} Found config: {}",
                "✔".green(),
                path.display().to_string().dimmed()
            );
            path
        }
        Err(e) => {
            println!("  {} Could not find config: {}", "✗".red(), e);
            return Ok(());
        }
    };

    let config = match agentsync::config::Config::load(&config_path) {
        Ok(c) => {
            println!("  {} Config loaded successfully", "✔".green());
            c
        }
        Err(e) => {
            println!("  {} Failed to parse config: {}", "✗".red(), e);
            return Ok(());
        }
    };

    let linker = Linker::new(config, config_path.clone());
    let source_dir = linker.config().source_dir(&config_path);

    // 2. Source Directory Check
    if !source_dir.exists() {
        println!(
            "  {} Source directory does not exist: {}",
            "✗".red(),
            source_dir.display()
        );
        issues += 1;
    } else {
        println!(
            "  {} Source directory exists: {}",
            "✔".green(),
            source_dir.display().to_string().dimmed()
        );
    }

    // 3. Target Source Existence Check
    let mut missing_targets = 0;
    for (agent_name, agent) in &linker.config().agents {
        if !agent.enabled {
            continue;
        }
        for (target_name, target) in &agent.targets {
            for warning in target_configuration_warnings(target) {
                println!(
                    "  {} {} for agent {} (target {})",
                    "⚠".yellow(),
                    warning,
                    agent_name.bold(),
                    target_name.dimmed()
                );
                issues += 1;
            }

            for missing in collect_missing_sources(
                &source_dir,
                linker.project_root(),
                agent_name,
                target_name,
                target,
            ) {
                if let Some(mapping) = &missing.mapping {
                    println!(
                        "  {} Missing mapping source for agent {} (target {}, mapping {}): {}",
                        "✗".red(),
                        missing.agent.bold(),
                        missing.target.dimmed(),
                        mapping.dimmed(),
                        missing.path.display()
                    );
                } else {
                    println!(
                        "  {} Missing source for agent {} (target {}): {}",
                        "✗".red(),
                        missing.agent.bold(),
                        missing.target.dimmed(),
                        missing.path.display()
                    );
                }
                issues += 1;
                missing_targets += 1;
            }

            if let Some(mismatch) = collect_skills_mode_mismatch(
                linker.project_root(),
                &source_dir,
                agent_name,
                target_name,
                target,
            ) {
                println!("  {} {}", "⚠".yellow(), mismatch.doctor_warning());
                issues += 1;
            }
        }
    }
    if missing_targets == 0 {
        println!("  {} All target sources exist", "✔".green());
    }

    // 4. Destination Path Conflict Check
    let mut destinations: Vec<(String, String, String)> = Vec::new(); // (path, agent, target)
    for (agent_name, agent) in &linker.config().agents {
        if !agent.enabled {
            continue;
        }
        for (target_name, target) in &agent.targets {
            destinations.extend(expand_target_destinations(agent_name, target_name, target));
        }
    }

    let conflict_results = validate_destinations(&destinations);
    for conflict in conflict_results {
        match conflict {
            Conflict::Duplicate(dest) => {
                println!(
                    "  {} Duplicate destination: {} (used by multiple targets)",
                    "✗".red(),
                    dest.bold()
                );
                issues += 1;
            }
            Conflict::Overlap(parent, child, p_info, c_info) => {
                println!(
                    "  {} Overlapping destinations: {} ({}) is parent of {} ({})",
                    "⚠".yellow(),
                    parent.bold(),
                    p_info,
                    child.bold(),
                    c_info
                );
                issues += 1;
            }
        }
    }

    // 5. MCP Server Audit
    if linker.config().mcp.enabled {
        for (name, server) in &linker.config().mcp_servers {
            if server.disabled {
                continue;
            }
            if let Some(cmd) = &server.command {
                if !command_exists(cmd) {
                    println!(
                        "  {} MCP server {} command not found in PATH: {}",
                        "✗".red(),
                        name.bold(),
                        cmd.bold()
                    );
                    issues += 1;
                } else {
                    println!(
                        "  {} MCP server {} command executable: {}",
                        "✔".green(),
                        name.bold(),
                        cmd.dimmed()
                    );
                }
            } else {
                println!(
                    "  {} MCP server {} has no command configured (not audited)",
                    "ℹ".blue(),
                    name.bold()
                );
            }
        }
    }

    // 6. .gitignore Audit
    let gitignore_path = linker.project_root().join(".gitignore");
    if gitignore_path.exists() {
        match fs::read_to_string(&gitignore_path) {
            Ok(content) => {
                let marker = &linker.config().gitignore.marker;
                let (start_marker, end_marker) = agentsync::gitignore::managed_markers(marker);
                let has_start_marker = content
                    .lines()
                    .any(|line| line.trim_end_matches('\r') == start_marker);
                let has_end_marker = content
                    .lines()
                    .any(|line| line.trim_end_matches('\r') == end_marker);
                let has_managed_section = has_start_marker && has_end_marker;

                if gitignore_missing_section_is_issue(
                    linker.config().gitignore.enabled,
                    &content,
                    &start_marker,
                    &end_marker,
                ) {
                    println!(
                        "  {} .gitignore managed section missing (Marker: {})",
                        "⚠".yellow(),
                        marker
                    );
                    issues += 1;
                } else if has_managed_section {
                    // Audit entries
                    let managed_entries =
                        extract_managed_entries(&content, &start_marker, &end_marker);
                    let required_entries: HashSet<String> = linker
                        .config()
                        .all_gitignore_entries()
                        .into_iter()
                        .collect();
                    let actual_entries: HashSet<String> = managed_entries.into_iter().collect();

                    let missing: Vec<_> = required_entries.difference(&actual_entries).collect();
                    let extra: Vec<_> = actual_entries.difference(&required_entries).collect();

                    if !missing.is_empty() {
                        println!(
                            "  {} .gitignore missing {} managed entries",
                            "✗".red(),
                            missing.len()
                        );
                        for m in &missing {
                            println!("    - {}", m);
                        }
                        issues += 1;
                    }
                    if !extra.is_empty() {
                        println!(
                            "  {} .gitignore has {} extra entries in managed section",
                            "⚠".yellow(),
                            extra.len()
                        );
                        issues += 1;
                    }

                    if missing.is_empty() && extra.is_empty() {
                        println!("  {} .gitignore managed section is up to date", "✔".green());
                    }
                }
            }
            Err(e) => {
                println!("  {} Failed to read .gitignore: {}", "✗".red(), e);
                issues += 1;
            }
        }
    } else if linker.config().gitignore.enabled {
        println!("  {} .gitignore file not found", "⚠".yellow());
        issues += 1;
    }

    // 7. Unmanaged Claude Skills Check
    if let Some(warning) = check_unmanaged_claude_skills(linker.project_root(), linker.config()) {
        println!("  {} {}", "⚠".yellow(), warning);
        issues += 1;
    }

    if issues == 0 {
        println!("\n{}", "✨ All systems go! No issues found.".green().bold());
    } else {
        println!(
            "\n{}",
            format!(
                "✖ Found {} issues. Run 'agentsync apply' to fix some of them.",
                issues
            )
            .red()
            .bold()
        );
    }

    Ok(())
}

pub fn collect_skills_mode_mismatch(
    project_root: &Path,
    source_dir: &Path,
    agent_name: &str,
    target_name: &str,
    target: &TargetConfig,
) -> Option<SkillsModeMismatch> {
    let expected_source = source_dir.join(&target.source);
    detect_skills_mode_mismatch(
        project_root,
        &expected_source,
        agent_name,
        target_name,
        target,
    )
}

pub(crate) fn gitignore_missing_section_is_issue(
    gitignore_enabled: bool,
    content: &str,
    start_marker: &str,
    end_marker: &str,
) -> bool {
    let has_start_marker = content
        .lines()
        .any(|line| line.trim_end_matches('\r') == start_marker);
    let has_end_marker = content
        .lines()
        .any(|line| line.trim_end_matches('\r') == end_marker);

    match (has_start_marker, has_end_marker) {
        (true, true) => false,
        (false, false) => gitignore_enabled,
        _ => true,
    }
}

fn command_exists(cmd: &str) -> bool {
    // If it's a path, check if it exists and is executable
    let path = Path::new(cmd);
    if path.is_absolute() || cmd.contains('/') || cmd.contains('\\') {
        return is_executable(path);
    }

    // Search in PATH
    if let Ok(path_var) = std::env::var("PATH") {
        for p in std::env::split_paths(&path_var) {
            let full_path = p.join(cmd);
            if is_executable(&full_path) {
                return true;
            }
            #[cfg(windows)]
            {
                for ext in &["exe", "cmd", "bat", "com"] {
                    if is_executable(&full_path.with_extension(ext)) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn is_executable(path: &Path) -> bool {
    if !path.exists() || !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = path.metadata() {
            return metadata.permissions().mode() & 0o111 != 0;
        }
        false
    }

    #[cfg(windows)]
    {
        return true; // on Windows, if it exists and has an executable extension, we assume it's executable
    }

    #[cfg(not(any(unix, windows)))]
    {
        return true;
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Conflict {
    Duplicate(String),
    Overlap(String, String, String, String), // parent, child, parent_info, child_info
}

pub fn validate_destinations(destinations: &[(String, String, String)]) -> Vec<Conflict> {
    let mut conflicts = Vec::new();
    let mut seen_dests = HashSet::new();
    let mut reported_dups = HashSet::new();

    // Check for exact duplicates
    for (dest, _, _) in destinations {
        if !seen_dests.insert(dest) && reported_dups.insert(dest.clone()) {
            conflicts.push(Conflict::Duplicate(dest.clone()));
        }
    }

    // Check for overlapping paths (one is parent of another)
    // To avoid repeated overlaps and handle duplicates gracefully:
    // 1. Get a deduplicated list of unique destinations for overlap checking
    let mut unique_dests_info = std::collections::BTreeMap::new();
    for (dest, agent, target) in destinations {
        unique_dests_info
            .entry(dest)
            .or_insert_with(|| format!("{}/{}", agent, target));
    }
    let unique_dests: Vec<_> = unique_dests_info.keys().cloned().collect();

    let mut seen_overlaps = HashSet::new();
    for (i, d1) in unique_dests.iter().enumerate() {
        let info1 = &unique_dests_info[d1];

        for (j, d2) in unique_dests.iter().enumerate() {
            if i == j {
                continue;
            }
            let info2 = &unique_dests_info[d2];

            let p1 = Path::new(d1);
            let p2 = Path::new(d2);

            if p2.starts_with(p1)
                && p1 != p2
                && seen_overlaps.insert(((*d1).clone(), (*d2).clone()))
            {
                conflicts.push(Conflict::Overlap(
                    (*d1).clone(),
                    (*d2).clone(),
                    info1.clone(),
                    info2.clone(),
                ));
            }
        }
    }

    conflicts
}

pub fn target_configuration_warnings(target: &TargetConfig) -> Vec<&'static str> {
    let mut warnings = Vec::new();

    match target.sync_type {
        SyncType::ModuleMap if target.mappings.is_empty() => {
            warnings.push("module-map target has no mappings configured")
        }
        SyncType::ModuleMap => {}
        _ if !target.mappings.is_empty() => {
            warnings.push("mappings is only used by module-map targets")
        }
        _ => {}
    }

    warnings
}

pub fn collect_missing_sources(
    source_dir: &Path,
    project_root: &Path,
    agent_name: &str,
    target_name: &str,
    target: &TargetConfig,
) -> Vec<MissingSourceIssue> {
    match target.sync_type {
        SyncType::ModuleMap => target
            .mappings
            .iter()
            .filter_map(|mapping| {
                let path = source_dir.join(&mapping.source);
                (!path.exists()).then(|| MissingSourceIssue {
                    agent: agent_name.to_string(),
                    target: target_name.to_string(),
                    mapping: Some(mapping.source.clone()),
                    path,
                })
            })
            .collect(),
        SyncType::NestedGlob => {
            // For NestedGlob, source is relative to project_root, not source_dir
            let search_root = project_root.join(&target.source);
            if search_root.exists() {
                Vec::new()
            } else {
                vec![MissingSourceIssue {
                    agent: agent_name.to_string(),
                    target: target_name.to_string(),
                    mapping: None,
                    path: search_root,
                }]
            }
        }
        _ => {
            let path = source_dir.join(&target.source);
            if path.exists() {
                Vec::new()
            } else {
                vec![MissingSourceIssue {
                    agent: agent_name.to_string(),
                    target: target_name.to_string(),
                    mapping: None,
                    path,
                }]
            }
        }
    }
}

pub fn expand_target_destinations(
    agent_name: &str,
    target_name: &str,
    target: &TargetConfig,
) -> Vec<(String, String, String)> {
    match target.sync_type {
        SyncType::ModuleMap => target
            .mappings
            .iter()
            .map(|mapping| {
                let filename = agentsync::config::resolve_module_map_filename(mapping, agent_name);
                let dest_path = format!("{}/{}", mapping.destination, filename);
                (
                    normalize_path(&dest_path),
                    agent_name.to_string(),
                    target_name.to_string(),
                )
            })
            .collect(),
        _ => vec![(
            normalize_path(&target.destination),
            agent_name.to_string(),
            target_name.to_string(),
        )],
    }
}

pub fn normalize_path(path_str: &str) -> String {
    let mut components = Vec::new();
    for component in Path::new(path_str).components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if components.is_empty() {
                    components.push(component.as_os_str());
                } else {
                    components.pop();
                }
            }
            std::path::Component::Normal(c) => {
                components.push(c);
            }
            _ => {
                components.push(component.as_os_str());
            }
        }
    }
    let mut res = PathBuf::new();
    for c in components {
        res.push(c);
    }
    res.to_string_lossy().to_string()
}

/// Check if .claude/skills/ has content but is not managed by any target.
pub fn check_unmanaged_claude_skills(
    project_root: &Path,
    config: &agentsync::config::Config,
) -> Option<String> {
    let claude_skills = project_root.join(".claude").join("skills");
    if !claude_skills.exists() || !claude_skills.is_dir() {
        return None;
    }

    // Check if directory contains at least one valid skill (a subdirectory with SKILL.md)
    let has_skills = fs::read_dir(&claude_skills)
        .ok()
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|entry| {
                entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                    && entry.path().join("SKILL.md").exists()
            })
        })
        .unwrap_or(false);
    if !has_skills {
        return None;
    }

    // Check if any enabled target manages .claude/skills
    // Normalize destination by stripping leading "./" and trailing "/"
    let is_managed = config.agents.values().any(|agent| {
        agent.enabled
            && agent.targets.values().any(|target| {
                let dest = target
                    .destination
                    .strip_prefix("./")
                    .unwrap_or(&target.destination)
                    .trim_end_matches('/');
                dest == ".claude/skills"
            })
    });

    if is_managed {
        None
    } else {
        Some(
            ".claude/skills/ has content but is not managed by any target. \
             Run 'agentsync init --wizard' to adopt."
                .to_string(),
        )
    }
}

pub fn extract_managed_entries(content: &str, start_marker: &str, end_marker: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut in_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == start_marker {
            in_section = true;
            continue;
        }
        if in_section && trimmed == end_marker {
            break;
        }
        if in_section && !trimmed.is_empty() && !trimmed.starts_with('#') {
            entries.push(trimmed.to_string());
        }
    }

    entries
}
