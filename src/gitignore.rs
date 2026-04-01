//! Gitignore management
//!
//! Handles automatic updates to .gitignore to exclude
//! generated symlinks from version control.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Build the start/end marker pair used to delimit the managed section in `.gitignore`.
pub fn managed_markers(marker: &str) -> (String, String) {
    (format!("# START {}", marker), format!("# END {}", marker))
}

/// Update .gitignore with managed entries
pub fn update_gitignore(
    project_root: &Path,
    marker: &str,
    entries: &[String],
    dry_run: bool,
) -> Result<()> {
    let gitignore_path = project_root.join(".gitignore");
    let (start_marker, end_marker) = managed_markers(marker);

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

    // Optimization: skip write if content is unchanged to avoid unnecessary I/O
    if existing_content == new_content {
        println!(
            "  {} .gitignore is already up to date ({} entries)",
            "✔".green(),
            entries.len()
        );
        return Ok(());
    }

    // Write the file
    fs::write(&gitignore_path, &new_content)
        .with_context(|| format!("Failed to write .gitignore: {}", gitignore_path.display()))?;

    println!(
        "  {} Updated .gitignore with {} managed entries",
        "✔".green(),
        entries.len()
    );

    Ok(())
}

/// Remove the managed section from .gitignore when management is disabled.
pub fn cleanup_gitignore(project_root: &Path, marker: &str, dry_run: bool) -> Result<()> {
    let gitignore_path = project_root.join(".gitignore");
    if !gitignore_path.exists() {
        return Ok(());
    }

    let (start_marker, end_marker) = managed_markers(marker);
    let existing_content = fs::read_to_string(&gitignore_path)
        .with_context(|| format!("Failed to read .gitignore: {}", gitignore_path.display()))?;
    let cleaned_content = remove_managed_section(&existing_content, &start_marker, &end_marker);

    if existing_content == cleaned_content {
        return Ok(());
    }

    if dry_run {
        println!("  {} Would remove managed .gitignore section", "→".cyan(),);
        return Ok(());
    }

    fs::write(&gitignore_path, &cleaned_content)
        .with_context(|| format!("Failed to write .gitignore: {}", gitignore_path.display()))?;

    println!("  {} Removed managed .gitignore section", "✔".green(),);

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

    #[test]
    fn test_cleanup_gitignore_removes_matching_managed_block() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        fs::write(
            &gitignore_path,
            "node_modules/\n# START Marker\nmanaged\n# END Marker\ndist/\n",
        )
        .unwrap();

        cleanup_gitignore(temp_dir.path(), "Marker", false).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(content, "node_modules/\ndist/\n");
    }

    #[test]
    fn test_cleanup_gitignore_respects_custom_marker() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        fs::write(
            &gitignore_path,
            "# START Default Marker\nkeep\n# END Default Marker\n# START Custom Marker\nremove\n# END Custom Marker\n",
        )
        .unwrap();

        cleanup_gitignore(temp_dir.path(), "Custom Marker", false).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains("# START Default Marker"));
        assert!(content.contains("keep"));
        assert!(!content.contains("Custom Marker"));
        assert!(!content.contains("remove"));
    }

    #[test]
    fn test_cleanup_gitignore_dry_run_does_not_modify_existing() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        let original = "node_modules/\n# START Marker\nmanaged\n# END Marker\n";
        fs::write(&gitignore_path, original).unwrap();

        cleanup_gitignore(temp_dir.path(), "Marker", true).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn test_cleanup_gitignore_is_noop_when_matching_block_missing() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        let original = "node_modules/\n# START Other\nmanaged\n# END Other\n";
        fs::write(&gitignore_path, original).unwrap();

        cleanup_gitignore(temp_dir.path(), "Marker", false).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(content, original);
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

    #[test]
    fn test_update_gitignore_preserves_mtime_if_unchanged() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        let entries = vec!["test.md".to_string()];

        // Initial update
        update_gitignore(temp_dir.path(), "Marker", &entries, false).unwrap();
        let mtime1 = fs::metadata(&gitignore_path).unwrap().modified().unwrap();

        // Small sleep to ensure mtime would change if written
        std::thread::sleep(std::time::Duration::from_millis(20));

        // Second update with same content
        update_gitignore(temp_dir.path(), "Marker", &entries, false).unwrap();
        let mtime2 = fs::metadata(&gitignore_path).unwrap().modified().unwrap();

        assert_eq!(
            mtime1, mtime2,
            "Modification time should not change if content is identical"
        );
    }
}
