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
    use tempfile::TempDir;

    // ==========================================================================
    // REMOVE MANAGED SECTION TESTS
    // ==========================================================================

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

    #[test]
    fn test_remove_managed_section_at_start() {
        let content = r#"# START Marker
managed1
managed2
# END Marker
other_content
"#;

        let result = remove_managed_section(content, "# START Marker", "# END Marker");

        assert!(!result.contains("managed1"));
        assert!(!result.contains("managed2"));
        assert!(result.contains("other_content"));
    }

    #[test]
    fn test_remove_managed_section_at_end() {
        let content = r#"other_content
# START Marker
managed1
managed2
# END Marker
"#;

        let result = remove_managed_section(content, "# START Marker", "# END Marker");

        assert!(result.contains("other_content"));
        assert!(!result.contains("managed1"));
        assert!(!result.contains("managed2"));
    }

    #[test]
    fn test_remove_managed_section_empty_managed() {
        let content = r#"before
# START Marker
# END Marker
after
"#;

        let result = remove_managed_section(content, "# START Marker", "# END Marker");

        assert!(result.contains("before"));
        assert!(result.contains("after"));
        assert!(!result.contains("START Marker"));
        assert!(!result.contains("END Marker"));
    }

    #[test]
    fn test_remove_managed_section_preserves_whitespace() {
        let content = "line1\n\n\nline2\n";
        let result = remove_managed_section(content, "# START", "# END");

        // Should preserve the original content including blank lines
        assert!(result.contains("line1"));
        assert!(result.contains("line2"));
    }

    // ==========================================================================
    // UPDATE GITIGNORE TESTS
    // ==========================================================================

    #[test]
    fn test_update_gitignore_creates_new_file() {
        let temp_dir = TempDir::new().unwrap();

        let entries = vec!["CLAUDE.md".to_string(), "AGENTS.md".to_string()];
        update_gitignore(temp_dir.path(), "AI Agent Symlinks", &entries, false).unwrap();

        let gitignore_path = temp_dir.path().join(".gitignore");
        assert!(gitignore_path.exists());

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains("# START AI Agent Symlinks"));
        assert!(content.contains("# END AI Agent Symlinks"));
        assert!(content.contains("CLAUDE.md"));
        assert!(content.contains("AGENTS.md"));
    }

    #[test]
    fn test_update_gitignore_appends_to_existing() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");

        // Create existing gitignore
        fs::write(&gitignore_path, "node_modules/\n*.log\n").unwrap();

        let entries = vec!["CLAUDE.md".to_string()];
        update_gitignore(temp_dir.path(), "AI Agent Symlinks", &entries, false).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();

        // Original content preserved
        assert!(content.contains("node_modules/"));
        assert!(content.contains("*.log"));

        // New content added
        assert!(content.contains("# START AI Agent Symlinks"));
        assert!(content.contains("CLAUDE.md"));
        assert!(content.contains("# END AI Agent Symlinks"));
    }

    #[test]
    fn test_update_gitignore_replaces_existing_section() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");

        // Create gitignore with existing managed section
        let initial_content = r#"node_modules/

# START Test Marker
OLD_ENTRY.md
# END Test Marker

dist/
"#;
        fs::write(&gitignore_path, initial_content).unwrap();

        let entries = vec!["NEW_ENTRY.md".to_string()];
        update_gitignore(temp_dir.path(), "Test Marker", &entries, false).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();

        // Original unmanaged content preserved
        assert!(content.contains("node_modules/"));
        assert!(content.contains("dist/"));

        // Old managed content removed
        assert!(!content.contains("OLD_ENTRY.md"));

        // New managed content added
        assert!(content.contains("NEW_ENTRY.md"));
    }

    #[test]
    fn test_update_gitignore_dry_run() {
        let temp_dir = TempDir::new().unwrap();

        let entries = vec!["CLAUDE.md".to_string()];
        update_gitignore(temp_dir.path(), "AI Agent Symlinks", &entries, true).unwrap();

        // File should NOT be created in dry-run mode
        let gitignore_path = temp_dir.path().join(".gitignore");
        assert!(!gitignore_path.exists());
    }

    #[test]
    fn test_update_gitignore_dry_run_does_not_modify_existing() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");

        let original_content = "node_modules/\n";
        fs::write(&gitignore_path, original_content).unwrap();

        let entries = vec!["CLAUDE.md".to_string()];
        update_gitignore(temp_dir.path(), "AI Agent Symlinks", &entries, true).unwrap();

        // Content should NOT be modified
        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(content, original_content);
    }

    #[test]
    fn test_update_gitignore_empty_entries() {
        let temp_dir = TempDir::new().unwrap();

        let entries: Vec<String> = vec![];
        update_gitignore(temp_dir.path(), "AI Agent Symlinks", &entries, false).unwrap();

        let gitignore_path = temp_dir.path().join(".gitignore");
        let content = fs::read_to_string(&gitignore_path).unwrap();

        // Section should still be created, just empty
        assert!(content.contains("# START AI Agent Symlinks"));
        assert!(content.contains("# END AI Agent Symlinks"));
    }

    #[test]
    fn test_update_gitignore_preserves_trailing_content() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");

        let initial_content = r#"# START Marker
old_entry
# END Marker
trailing_content
"#;
        fs::write(&gitignore_path, initial_content).unwrap();

        let entries = vec!["new_entry".to_string()];
        update_gitignore(temp_dir.path(), "Marker", &entries, false).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();

        assert!(content.contains("trailing_content"));
        assert!(content.contains("new_entry"));
        assert!(!content.contains("old_entry"));
    }

    #[test]
    fn test_update_gitignore_multiple_entries() {
        let temp_dir = TempDir::new().unwrap();

        let entries = vec![
            "entry1.md".to_string(),
            "entry2.md".to_string(),
            "entry3.md".to_string(),
            ".github/copilot-instructions.md".to_string(),
        ];
        update_gitignore(temp_dir.path(), "AI Agent Symlinks", &entries, false).unwrap();

        let gitignore_path = temp_dir.path().join(".gitignore");
        let content = fs::read_to_string(&gitignore_path).unwrap();

        for entry in &entries {
            assert!(content.contains(entry), "Should contain entry: {}", entry);
        }
    }

    #[test]
    fn test_update_gitignore_custom_marker() {
        let temp_dir = TempDir::new().unwrap();

        let entries = vec!["test.md".to_string()];
        update_gitignore(temp_dir.path(), "My Custom Marker", &entries, false).unwrap();

        let gitignore_path = temp_dir.path().join(".gitignore");
        let content = fs::read_to_string(&gitignore_path).unwrap();

        assert!(content.contains("# START My Custom Marker"));
        assert!(content.contains("# END My Custom Marker"));
    }

    // ==========================================================================
    // EDGE CASE TESTS
    // ==========================================================================

    #[test]
    fn test_update_gitignore_with_special_characters_in_entries() {
        let temp_dir = TempDir::new().unwrap();

        let entries = vec![
            ".github/copilot-instructions.md".to_string(),
            "path/with spaces/file.md".to_string(),
            "*.md".to_string(),
        ];
        update_gitignore(temp_dir.path(), "Marker", &entries, false).unwrap();

        let gitignore_path = temp_dir.path().join(".gitignore");
        let content = fs::read_to_string(&gitignore_path).unwrap();

        for entry in &entries {
            assert!(content.contains(entry));
        }
    }

    #[test]
    fn test_update_gitignore_idempotent() {
        let temp_dir = TempDir::new().unwrap();

        let entries = vec!["test.md".to_string()];

        // Apply twice
        update_gitignore(temp_dir.path(), "Marker", &entries, false).unwrap();
        update_gitignore(temp_dir.path(), "Marker", &entries, false).unwrap();

        let gitignore_path = temp_dir.path().join(".gitignore");
        let content = fs::read_to_string(&gitignore_path).unwrap();

        // Should only have one managed section
        let start_count = content.matches("# START Marker").count();
        let end_count = content.matches("# END Marker").count();

        assert_eq!(start_count, 1, "Should have exactly one START marker");
        assert_eq!(end_count, 1, "Should have exactly one END marker");
    }
}
