use agentsync::Linker;
use anyhow::Result;
use colored::Colorize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

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
            return Err(e);
        }
    };

    let config = match agentsync::config::Config::load(&config_path) {
        Ok(c) => {
            println!("  {} Config loaded successfully", "✔".green());
            c
        }
        Err(e) => {
            println!("  {} Failed to parse config: {}", "✗".red(), e);
            return Err(e);
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
    for (agent_name, agent) in &linker.config().agents {
        if !agent.enabled {
            continue;
        }
        for (target_name, target) in &agent.targets {
            let target_source = source_dir.join(&target.source);
            if !target_source.exists() {
                println!(
                    "  {} Missing source for agent {} (target {}): {}",
                    "✗".red(),
                    agent_name.bold(),
                    target_name.dimmed(),
                    target_source.display()
                );
                issues += 1;
            }
        }
    }

    // 4. Destination Path Conflict Check
    let mut destinations: Vec<(String, String, String)> = Vec::new(); // (path, agent, target)
    for (agent_name, agent) in &linker.config().agents {
        if !agent.enabled {
            continue;
        }
        for (target_name, target) in &agent.targets {
            destinations.push((
                target.destination.clone(),
                agent_name.clone(),
                target_name.clone(),
            ));
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
            }
        }
    }

    // 6. .gitignore Audit
    let gitignore_path = linker.project_root().join(".gitignore");
    if gitignore_path.exists() {
        match fs::read_to_string(&gitignore_path) {
            Ok(content) => {
                let marker = &linker.config().gitignore.marker;
                let start_marker = format!("# START {}", marker);
                let end_marker = format!("# END {}", marker);

                if !content.contains(&start_marker) || !content.contains(&end_marker) {
                    if linker.config().gitignore.enabled {
                        println!(
                            "  {} .gitignore managed section missing (Marker: {})",
                            "⚠".yellow(),
                            marker
                        );
                        issues += 1;
                    }
                } else {
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

    // Check for exact duplicates
    for (dest, _, _) in destinations {
        if !seen_dests.insert(dest) {
            // We only want to report each duplicate once
            if conflicts.iter().all(|c| match c {
                Conflict::Duplicate(d) => d != dest,
                _ => true,
            }) {
                conflicts.push(Conflict::Duplicate(dest.clone()));
            }
        }
    }

    // Check for overlapping paths (one is parent of another)
    for i in 0..destinations.len() {
        for j in 0..destinations.len() {
            if i == j {
                continue;
            }
            let (d1, a1, t1) = &destinations[i];
            let (d2, a2, t2) = &destinations[j];

            let p1 = Path::new(d1);
            let p2 = Path::new(d2);

            if p2.starts_with(p1) && p1 != p2 {
                conflicts.push(Conflict::Overlap(
                    d1.clone(),
                    d2.clone(),
                    format!("{}/{}", a1, t1),
                    format!("{}/{}", a2, t2),
                ));
            }
        }
    }

    conflicts
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
        if trimmed == end_marker {
            break;
        }
        if in_section && !trimmed.is_empty() && !trimmed.starts_with('#') {
            entries.push(trimmed.to_string());
        }
    }

    entries
}
