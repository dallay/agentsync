#[cfg(test)]
mod tests {
    use crate::commands::status::{
        StatusEntry, collect_status_entries, collect_status_hints, entry_is_problematic,
        render_status_entry,
    };
    use agentsync::{Linker, config::Config, linker::SyncOptions};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn load_linker(temp_dir: &TempDir, config_content: &str) -> (Linker, PathBuf) {
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        let config_path = agents_dir.join("agentsync.toml");
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        (Linker::new(config, config_path.clone()), config_path)
    }

    fn commands_symlink_contents_config(destination: &str) -> String {
        format!(
            r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.commands]
            source = "commands"
            destination = "{destination}"
            type = "symlink-contents"
        "#
        )
    }

    fn skills_target_config(destination: &str, sync_type: &str) -> String {
        format!(
            r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.skills]
            source = "skills"
            destination = "{destination}"
            type = "{sync_type}"
        "#
        )
    }

    fn single_symlink_config(source: &str, destination: &str) -> String {
        format!(
            r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.main]
            source = "{source}"
            destination = "{destination}"
            type = "symlink"
        "#
        )
    }

    #[test]
    fn test_missing_entry_is_problematic() {
        let e = StatusEntry {
            destination: "/tmp/dest".into(),
            sync_type: "symlink".into(),
            destination_kind: crate::commands::status::DestinationKind::Missing,
            exists: false,
            is_symlink: false,
            points_to: None,
            expected_source: None,
            issues: vec![crate::commands::status::StatusIssue {
                kind: crate::commands::status::StatusIssueKind::MissingDestination,
                path: "/tmp/dest".into(),
                expected: None,
                actual: None,
            }],
            managed_children: None,
        };
        assert!(entry_is_problematic(&e));
    }

    #[test]
    fn test_non_symlink_exists_problematic() {
        let e = StatusEntry {
            destination: "/tmp/dest".into(),
            sync_type: "symlink".into(),
            destination_kind: crate::commands::status::DestinationKind::File,
            exists: true,
            is_symlink: false,
            points_to: None,
            expected_source: None,
            issues: vec![crate::commands::status::StatusIssue {
                kind: crate::commands::status::StatusIssueKind::InvalidDestinationType,
                path: "/tmp/dest".into(),
                expected: Some("symlink".into()),
                actual: Some("file".into()),
            }],
            managed_children: None,
        };
        // Non-symlink destinations are considered problematic (drift)
        assert!(entry_is_problematic(&e));
    }

    #[test]
    fn test_symlink_points_equal_not_problematic() {
        let e = StatusEntry {
            destination: "/home/user/project/sub/dest".into(),
            sync_type: "symlink".into(),
            destination_kind: crate::commands::status::DestinationKind::Symlink,
            exists: true,
            is_symlink: true,
            points_to: Some("../src/lib".into()),
            expected_source: Some("/home/user/project/src/lib".into()),
            issues: Vec::new(),
            managed_children: None,
        };
        // fake canonicalize will resolve ../src/lib relative to parent of dest
        assert!(!entry_is_problematic(&e));
    }

    #[test]
    fn test_symlink_points_different_is_problematic() {
        let e = StatusEntry {
            destination: "/tmp/dest".into(),
            sync_type: "symlink".into(),
            destination_kind: crate::commands::status::DestinationKind::Symlink,
            exists: true,
            is_symlink: true,
            points_to: Some("/other/place".into()),
            expected_source: Some("/expected/place".into()),
            issues: vec![crate::commands::status::StatusIssue {
                kind: crate::commands::status::StatusIssueKind::IncorrectLinkTarget,
                path: "/tmp/dest".into(),
                expected: Some("/expected/place".into()),
                actual: Some("/other/place".into()),
            }],
            managed_children: None,
        };
        assert!(entry_is_problematic(&e));
    }

    #[test]
    #[cfg(unix)]
    fn test_collect_status_entries_expands_module_map_entries() {
        let temp_dir = TempDir::new().unwrap();
        let claude_dir = temp_dir.path().join(".agents/claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("api.md"), "# API").unwrap();
        fs::write(claude_dir.join("ui.md"), "# UI").unwrap();

        let config = r#"
            source_dir = "claude"

            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "placeholder"
            destination = "placeholder"
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "api.md"
            destination = "src/api"

            [[agents.claude.targets.modules.mappings]]
            source = "ui.md"
            destination = "src/ui"

            [[agents.claude.targets.modules.mappings]]
            source = "missing.md"
            destination = "src/missing"
        "#;

        let (linker, config_path) = load_linker(&temp_dir, config);
        linker.sync(&SyncOptions::default()).unwrap();

        let entries = collect_status_entries(&linker, &config_path).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().any(|entry| {
            entry.destination.ends_with("src/api/CLAUDE.md") && entry.exists && entry.is_symlink
        }));
        assert!(entries.iter().any(|entry| {
            entry.destination.ends_with("src/ui/CLAUDE.md") && entry.exists && entry.is_symlink
        }));
        assert!(entries.iter().any(|entry| {
            entry.destination.ends_with("src/missing/CLAUDE.md") && !entry.exists
        }));

        let json = serde_json::to_value(&entries).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 3);
    }

    #[test]
    #[cfg(unix)]
    fn test_collect_status_entries_reports_stale_module_map_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let claude_dir = temp_dir.path().join(".agents/claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("expected.md"), "# Expected").unwrap();
        fs::write(claude_dir.join("other.md"), "# Other").unwrap();
        fs::create_dir_all(temp_dir.path().join("src/api")).unwrap();

        let config = r#"
            source_dir = "claude"

            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "placeholder"
            destination = "placeholder"
            type = "module-map"

            [[agents.claude.targets.modules.mappings]]
            source = "expected.md"
            destination = "src/api"
        "#;

        let (linker, config_path) = load_linker(&temp_dir, config);
        let dest = temp_dir.path().join("src/api/CLAUDE.md");
        std::os::unix::fs::symlink("../../.agents/claude/other.md", &dest).unwrap();

        let entries = collect_status_entries(&linker, &config_path).unwrap();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert!(entry.is_symlink);
        assert!(entry.points_to.as_deref().unwrap().contains("other.md"));
        assert!(
            entry
                .expected_source
                .as_deref()
                .unwrap()
                .ends_with("expected.md")
        );
        assert!(entry_is_problematic(entry));
    }

    #[test]
    #[cfg(unix)]
    fn test_collect_status_hints_reports_recognized_mode_mismatch_without_problem() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path().join(".agents/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("SKILL.md"), "# skill").unwrap();
        fs::create_dir_all(temp_dir.path().join(".claude")).unwrap();
        unix_fs::symlink("../.agents/skills", temp_dir.path().join(".claude/skills")).unwrap();

        let config = &skills_target_config(".claude/skills", "symlink-contents");

        let (linker, config_path) = load_linker(&temp_dir, config);
        let entries = collect_status_entries(&linker, &config_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(!entry_is_problematic(&entries[0]));

        let hints = collect_status_hints(&linker, &config_path);
        let hint = hints
            .get(&temp_dir.path().join(".claude/skills").display().to_string())
            .unwrap();
        assert!(hint.contains("symlink-contents"));
        assert!(hint.contains("symlink"));
    }

    #[test]
    #[cfg(unix)]
    fn test_collect_status_hints_stays_quiet_for_matching_symlink_mode() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path().join(".agents/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::create_dir_all(temp_dir.path().join(".claude")).unwrap();
        unix_fs::symlink("../.agents/skills", temp_dir.path().join(".claude/skills")).unwrap();

        let config = &skills_target_config(".claude/skills", "symlink");

        let (linker, config_path) = load_linker(&temp_dir, config);
        let hints = collect_status_hints(&linker, &config_path);
        assert!(hints.is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn test_symlink_contents_empty_source_directory_is_valid_in_status_and_json() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join(".agents/commands")).unwrap();
        fs::create_dir_all(temp_dir.path().join(".claude/commands")).unwrap();

        let (linker, config_path) = load_linker(
            &temp_dir,
            &commands_symlink_contents_config(".claude/commands"),
        );

        let entries = collect_status_entries(&linker, &config_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(!entry_is_problematic(&entries[0]));

        let json = serde_json::to_value(&entries).unwrap();
        let entry = &json.as_array().unwrap()[0];
        assert_eq!(entry["sync_type"], "symlink-contents");
        assert_eq!(entry["destination_kind"], "directory");
        assert_eq!(entry["issues"], serde_json::json!([]));
        assert_eq!(entry["managed_children"], serde_json::json!([]));

        let rendered = render_status_entry(&entries[0]);
        assert_eq!(rendered.len(), 1);
        assert!(rendered[0].contains("OK:"));
        assert!(rendered[0].contains("symlink-contents container"));
        assert!(rendered[0].contains("0 managed entries expected"));
    }

    #[test]
    #[cfg(unix)]
    fn test_symlink_contents_missing_expected_child_is_problematic() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join(".agents/commands");
        let dest_dir = temp_dir.path().join(".claude/commands");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&dest_dir).unwrap();
        fs::write(source_dir.join("review.md"), "# Review").unwrap();
        fs::write(source_dir.join("ship.md"), "# Ship").unwrap();
        unix_fs::symlink(
            "../../.agents/commands/review.md",
            dest_dir.join("review.md"),
        )
        .unwrap();

        let (linker, config_path) = load_linker(
            &temp_dir,
            &commands_symlink_contents_config(".claude/commands"),
        );

        let entries = collect_status_entries(&linker, &config_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entry_is_problematic(&entries[0]));

        let json = serde_json::to_value(&entries).unwrap();
        let issues = json.as_array().unwrap()[0]["issues"].as_array().unwrap();
        assert!(issues.iter().any(|issue| {
            issue["kind"] == "missing-expected-child"
                && issue["path"].as_str().unwrap().ends_with("ship.md")
        }));
    }

    #[test]
    #[cfg(unix)]
    fn test_symlink_contents_wrong_child_type_and_invalid_destination_type_are_problematic() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join(".agents/commands");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("review.md"), "# Review").unwrap();

        let file_destination = temp_dir.path().join(".claude/commands");
        fs::create_dir_all(file_destination.parent().unwrap()).unwrap();
        fs::write(&file_destination, "not a directory").unwrap();

        let (file_linker, file_config_path) = load_linker(
            &temp_dir,
            &commands_symlink_contents_config(".claude/commands"),
        );
        let file_entries = collect_status_entries(&file_linker, &file_config_path).unwrap();
        assert!(entry_is_problematic(&file_entries[0]));

        let file_json = serde_json::to_value(&file_entries).unwrap();
        assert!(
            file_json.as_array().unwrap()[0]["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| issue["kind"] == "invalid-destination-type")
        );

        fs::remove_file(&file_destination).unwrap();
        fs::create_dir_all(&file_destination).unwrap();
        fs::write(file_destination.join("review.md"), "not a symlink").unwrap();

        let (child_linker, child_config_path) = load_linker(
            &temp_dir,
            &commands_symlink_contents_config(".claude/commands"),
        );
        let child_entries = collect_status_entries(&child_linker, &child_config_path).unwrap();
        assert!(entry_is_problematic(&child_entries[0]));

        let child_json = serde_json::to_value(&child_entries).unwrap();
        assert!(
            child_json.as_array().unwrap()[0]["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| issue["kind"] == "child-not-symlink")
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_symlink_contents_wrong_child_target_is_problematic_and_rendered() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join(".agents/commands");
        let dest_dir = temp_dir.path().join(".claude/commands");
        let wrong_target = temp_dir.path().join("wrong.md");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&dest_dir).unwrap();
        fs::write(source_dir.join("review.md"), "# Review").unwrap();
        fs::write(&wrong_target, "# Wrong").unwrap();
        unix_fs::symlink("../../wrong.md", dest_dir.join("review.md")).unwrap();

        let (linker, config_path) = load_linker(
            &temp_dir,
            &commands_symlink_contents_config(".claude/commands"),
        );

        let entries = collect_status_entries(&linker, &config_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entry_is_problematic(&entries[0]));

        let json = serde_json::to_value(&entries).unwrap();
        let issues = json.as_array().unwrap()[0]["issues"].as_array().unwrap();
        assert!(issues.iter().any(|issue| {
            issue["kind"] == "incorrect-link-target"
                && issue["path"].as_str().unwrap().ends_with("review.md")
                && issue["actual"].as_str().unwrap().contains("wrong.md")
        }));

        let rendered = render_status_entry(&entries[0]);
        assert_eq!(rendered.len(), 1);
        assert!(rendered[0].contains("Drift:"));
        assert!(rendered[0].contains("review.md points to"));
        assert!(rendered[0].contains("wrong.md"));
    }

    #[test]
    #[cfg(unix)]
    fn test_symlink_missing_destination_is_rendered_as_missing() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join(".agents/claude")).unwrap();
        fs::write(temp_dir.path().join(".agents/claude/CLAUDE.md"), "# Claude").unwrap();

        let (linker, config_path) = load_linker(
            &temp_dir,
            &single_symlink_config("claude/CLAUDE.md", ".claude/CLAUDE.md"),
        );

        let entries = collect_status_entries(&linker, &config_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entry_is_problematic(&entries[0]));
        assert_eq!(
            entries[0].issues[0].kind,
            crate::commands::status::StatusIssueKind::MissingDestination
        );

        let rendered = render_status_entry(&entries[0]);
        assert_eq!(
            rendered,
            vec![format!(
                "{} Missing: {}",
                "!",
                temp_dir.path().join(".claude/CLAUDE.md").display()
            )]
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_symlink_invalid_destination_type_is_rendered_as_non_symlink() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join(".agents/claude")).unwrap();
        fs::write(temp_dir.path().join(".agents/claude/CLAUDE.md"), "# Claude").unwrap();
        fs::create_dir_all(temp_dir.path().join(".claude")).unwrap();
        fs::write(temp_dir.path().join(".claude/CLAUDE.md"), "not a symlink").unwrap();

        let (linker, config_path) = load_linker(
            &temp_dir,
            &single_symlink_config("claude/CLAUDE.md", ".claude/CLAUDE.md"),
        );

        let entries = collect_status_entries(&linker, &config_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entry_is_problematic(&entries[0]));
        assert_eq!(
            entries[0].issues[0].kind,
            crate::commands::status::StatusIssueKind::InvalidDestinationType
        );

        let rendered = render_status_entry(&entries[0]);
        assert_eq!(
            rendered,
            vec![format!(
                "· Exists but not a symlink: {}",
                temp_dir.path().join(".claude/CLAUDE.md").display()
            )]
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_symlink_contents_missing_container_directory_is_rendered_as_drift() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join(".agents/commands");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("review.md"), "# Review").unwrap();

        let (linker, config_path) = load_linker(
            &temp_dir,
            &commands_symlink_contents_config(".claude/commands"),
        );

        let entries = collect_status_entries(&linker, &config_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entry_is_problematic(&entries[0]));
        assert_eq!(
            entries[0].issues[0].kind,
            crate::commands::status::StatusIssueKind::MissingDestination
        );

        let rendered = render_status_entry(&entries[0]);
        assert_eq!(rendered.len(), 1);
        assert!(rendered[0].contains("missing managed container directory"));
    }

    #[test]
    #[cfg(unix)]
    fn test_symlink_contents_missing_source_directory_is_rendered_as_missing_source() {
        let temp_dir = TempDir::new().unwrap();

        let (linker, config_path) = load_linker(
            &temp_dir,
            &commands_symlink_contents_config(".claude/commands"),
        );

        let entries = collect_status_entries(&linker, &config_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entry_is_problematic(&entries[0]));
        assert_eq!(
            entries[0].issues[0].kind,
            crate::commands::status::StatusIssueKind::MissingExpectedSource
        );

        let rendered = render_status_entry(&entries[0]);
        assert_eq!(rendered.len(), 1);
        assert!(rendered[0].contains("Link points to missing source"));
        assert!(
            rendered[0].contains(
                &temp_dir
                    .path()
                    .join(".claude/commands")
                    .display()
                    .to_string()
            )
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_collect_status_entries_reports_healthy_populated_symlink_contents_container() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join(".agents/commands");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(commands_dir.join("review.md"), "# Review").unwrap();
        fs::write(commands_dir.join("ship.md"), "# Ship").unwrap();

        let (linker, config_path) = load_linker(
            &temp_dir,
            &commands_symlink_contents_config(".claude/commands"),
        );
        linker.sync(&SyncOptions::default()).unwrap();

        let entries = collect_status_entries(&linker, &config_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(!entry_is_problematic(&entries[0]));

        let json = serde_json::to_value(&entries).unwrap();
        let entry = &json.as_array().unwrap()[0];
        assert_eq!(entry["sync_type"], "symlink-contents");
        assert_eq!(entry["destination_kind"], "directory");
        assert_eq!(entry["issues"], serde_json::json!([]));
        assert_eq!(entry["managed_children"].as_array().unwrap().len(), 2);
    }

    #[test]
    #[cfg(unix)]
    fn test_linker_sync_treats_existing_empty_symlink_contents_source_as_valid() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join(".agents/commands")).unwrap();

        let (linker, config_path) = load_linker(
            &temp_dir,
            &commands_symlink_contents_config(".claude/commands"),
        );

        let result = linker.sync(&SyncOptions::default()).unwrap();
        assert_eq!(result.created, 0);
        assert_eq!(result.updated, 0);
        assert_eq!(result.errors, 0);

        let dest_dir = temp_dir.path().join(".claude/commands");
        assert!(dest_dir.exists());
        assert!(dest_dir.is_dir());
        assert_eq!(fs::read_dir(&dest_dir).unwrap().count(), 0);

        let entries = collect_status_entries(&linker, &config_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(!entry_is_problematic(&entries[0]));
    }
}
