//! Gitignore management
//!
//! Handles automatic updates to .gitignore to exclude
//! generated symlinks from version control.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Update .gitignore with managed entries
pub fn update_gitignore(
    project_root: &Path,
    marker: &str,
    entries: &[String],
    dry_run: bool,
) -> Result<()> {
    let gitignore_path = project_root.join(".gitignore");
    let start_marker = format!("# START {}", marker);
    let end_marker = format!("# END {}", marker);

    // Read existing content or start fresh
    let existing_content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)
            .with_context(|| format!("Failed to read .gitignore: {}", gitignore_path.display()))?
    } else {
        String::new()
    };

    // Remove existing managed section if present
    let content_without_managed =
        remove_managed_section(&existing_content, &start_marker, &end_marker);

    // Build new managed section
    let mut managed_section = String::new();
    managed_section.push('\n');
    managed_section.push_str(&start_marker);
    managed_section.push('\n');
    for entry in entries {
        managed_section.push_str(entry);
        managed_section.push('\n');
    }
    managed_section.push_str(&end_marker);
    managed_section.push('\n');

    // Combine content
    let new_content = format!("{}{}", content_without_managed.trim_end(), managed_section);

    if dry_run {
        println!(
            "  {} Would update .gitignore with {} entries",
            "→".cyan(),
            entries.len()
        );
        return Ok(());
    }

    // Write the file
    fs::write(&gitignore_path, new_content)
        .with_context(|| format!("Failed to write .gitignore: {}", gitignore_path.display()))?;

    println!(
        "  {} Updated .gitignore with {} managed entries",
        "✔".green(),
        entries.len()
    );

    Ok(())
}

/// Remove the managed section from gitignore content
fn remove_managed_section(content: &str, start_marker: &str, end_marker: &str) -> String {
    let mut result = String::new();
    let mut in_managed_section = false;

    for line in content.lines() {
        if line.trim() == start_marker {
            in_managed_section = true;
            continue;
        }
        if line.trim() == end_marker {
            in_managed_section = false;
            continue;
        }
        if !in_managed_section {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_managed_section() {
        let content = r#"node_modules/
*.log

# START Test Marker
AGENTS.md
CLAUDE.md
# END Test Marker

dist/
"#;

        let result = remove_managed_section(content, "# START Test Marker", "# END Test Marker");

        assert!(result.contains("node_modules/"));
        assert!(result.contains("dist/"));
        assert!(!result.contains("AGENTS.md"));
        assert!(!result.contains("CLAUDE.md"));
        assert!(!result.contains("Test Marker"));
    }

    #[test]
    fn test_remove_managed_section_not_present() {
        let content = "node_modules/\n*.log\n";
        let result = remove_managed_section(content, "# START Marker", "# END Marker");
        assert_eq!(result, content);
    }
}
