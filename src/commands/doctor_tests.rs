#[cfg(test)]
mod tests {
    use crate::commands::doctor::{
        Conflict, collect_missing_sources, expand_target_destinations, extract_managed_entries,
        normalize_path, target_configuration_warnings, validate_destinations,
    };
    use agentsync::config::{Config, SyncType};
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn module_map_config(toml_suffix: &str) -> Config {
        let toml = format!(
            r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.modules]
            source = "placeholder"
            destination = "placeholder"
            type = "module-map"
            {toml_suffix}
            "#
        );

        toml::from_str(&toml).unwrap()
    }

    #[test]
    fn test_extract_managed_entries() {
        let content = r#"# some comment
line1
# START Marker
entry1
entry2
# some other comment inside
# END Marker
line2
"#;
        let entries = extract_managed_entries(content, "# START Marker", "# END Marker");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], "entry1");
        assert_eq!(entries[1], "entry2");
    }

    #[test]
    fn test_extract_managed_entries_empty() {
        let content = r#"# START Marker
# END Marker
"#;
        let entries = extract_managed_entries(content, "# START Marker", "# END Marker");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_extract_managed_entries_no_markers() {
        let content = "some content\n";
        let entries = extract_managed_entries(content, "# START", "# END");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_extract_managed_entries_out_of_order() {
        // (3) an extract_managed_entries(...) case where the end marker appears before the start marker
        // to verify the function handles that gracefully
        let content = r#"# END Marker
# START Marker
entry1
# END Marker
"#;
        let entries = extract_managed_entries(content, "# START Marker", "# END Marker");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], "entry1");
    }

    #[test]
    fn test_validate_destinations_no_conflicts() {
        let dests = vec![
            (
                "a/b".to_string(),
                "agent1".to_string(),
                "target1".to_string(),
            ),
            (
                "x/y".to_string(),
                "agent2".to_string(),
                "target2".to_string(),
            ),
        ];
        let conflicts = validate_destinations(&dests);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_validate_destinations_duplicates() {
        let dests = vec![
            (
                "a/b".to_string(),
                "agent1".to_string(),
                "target1".to_string(),
            ),
            (
                "a/b".to_string(),
                "agent2".to_string(),
                "target2".to_string(),
            ),
        ];
        let conflicts = validate_destinations(&dests);
        assert_eq!(conflicts.len(), 1);
        assert!(matches!(conflicts[0], Conflict::Duplicate(_)));
    }

    #[test]
    fn test_validate_destinations_multiple_duplicates() {
        // (1) a case where the same path appears 3+ times to ensure
        // validate_destinations(...) still returns a single Conflict::Duplicate
        let dests = vec![
            (
                "a/b".to_string(),
                "agent1".to_string(),
                "target1".to_string(),
            ),
            (
                "a/b".to_string(),
                "agent2".to_string(),
                "target2".to_string(),
            ),
            (
                "a/b".to_string(),
                "agent3".to_string(),
                "target3".to_string(),
            ),
        ];
        let conflicts = validate_destinations(&dests);
        assert_eq!(conflicts.len(), 1);
        assert!(matches!(conflicts[0], Conflict::Duplicate(_)));
    }

    #[test]
    fn test_validate_destinations_overlaps() {
        let dests = vec![
            (
                "a/b".to_string(),
                "agent1".to_string(),
                "target1".to_string(),
            ),
            (
                "a/b/c".to_string(),
                "agent2".to_string(),
                "target2".to_string(),
            ),
        ];
        let conflicts = validate_destinations(&dests);
        assert_eq!(conflicts.len(), 1);
        match &conflicts[0] {
            Conflict::Overlap(parent, child, _, _) => {
                assert_eq!(parent, "a/b");
                assert_eq!(child, "a/b/c");
            }
            _ => panic!("Expected Overlap"),
        }
    }

    #[test]
    fn test_validate_destinations_combined() {
        // (2) a combined scenario like vec!["a/b","a/b","a/b/c"]
        // to ensure validate_destinations(...) produces both duplicate and overlap conflicts
        let dests = vec![
            (
                "a/b".to_string(),
                "agent1".to_string(),
                "target1".to_string(),
            ),
            (
                "a/b".to_string(),
                "agent2".to_string(),
                "target2".to_string(),
            ),
            (
                "a/b/c".to_string(),
                "agent3".to_string(),
                "target3".to_string(),
            ),
        ];
        let conflicts = validate_destinations(&dests);
        // Expecting one Duplicate and one Overlap
        assert_eq!(conflicts.len(), 2);
        let has_duplicate = conflicts
            .iter()
            .any(|c| matches!(c, Conflict::Duplicate(d) if d == "a/b"));
        let has_overlap = conflicts
            .iter()
            .any(|c| matches!(c, Conflict::Overlap(p, ch, _, _) if p == "a/b" && ch == "a/b/c"));
        assert!(has_duplicate, "Missing expected duplicate conflict");
        assert!(has_overlap, "Missing expected overlap conflict");
    }

    #[test]
    fn test_normalize_path() {
        let expected = |parts: &[&str]| {
            let mut p = PathBuf::new();
            for part in parts {
                p.push(part);
            }
            p.to_string_lossy().to_string()
        };

        assert_eq!(normalize_path("a/b/c"), expected(&["a", "b", "c"]));
        assert_eq!(normalize_path("./a/b"), expected(&["a", "b"]));
        assert_eq!(normalize_path("a/./b"), expected(&["a", "b"]));
        assert_eq!(normalize_path("a/b/"), expected(&["a", "b"]));
        assert_eq!(normalize_path("a//b"), expected(&["a", "b"]));
        #[cfg(unix)]
        assert_eq!(normalize_path("/a/b"), "/a/b");

        // ParentDir handling
        assert_eq!(normalize_path("a/../b"), expected(&["b"]));
        assert_eq!(normalize_path("../a"), expected(&["..", "a"]));
    }

    #[test]
    fn test_collect_missing_sources_ignores_module_map_placeholder_target_source() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("api-context.md"), "# API").unwrap();

        let config = module_map_config(
            r#"
            [[agents.claude.targets.modules.mappings]]
            source = "api-context.md"
            destination = "src/api"
            "#,
        );
        let target = &config.agents["claude"].targets["modules"];

        let issues = collect_missing_sources(
            temp_dir.path(),
            temp_dir.path(),
            "claude",
            "modules",
            target,
        );
        assert!(issues.is_empty());
    }

    #[test]
    fn test_collect_missing_sources_reports_missing_module_map_mapping_source() {
        let temp_dir = TempDir::new().unwrap();
        let config = module_map_config(
            r#"
            [[agents.claude.targets.modules.mappings]]
            source = "missing.md"
            destination = "src/api"
            "#,
        );
        let target = &config.agents["claude"].targets["modules"];

        let issues = collect_missing_sources(
            temp_dir.path(),
            temp_dir.path(),
            "claude",
            "modules",
            target,
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].mapping.as_deref(), Some("missing.md"));
        assert!(issues[0].path.ends_with("missing.md"));
    }

    #[test]
    fn test_expand_target_destinations_expands_module_map_destinations() {
        let config = module_map_config(
            r#"
            [[agents.claude.targets.modules.mappings]]
            source = "api.md"
            destination = "src/api"

            [[agents.claude.targets.modules.mappings]]
            source = "ui.md"
            destination = "src/ui"
            "#,
        );
        let target = &config.agents["claude"].targets["modules"];

        let destinations = expand_target_destinations("claude", "modules", target);
        assert_eq!(destinations.len(), 2);
        // Use ends_with for cross-platform path comparison (Windows uses \ separator)
        assert!(destinations.iter().any(|(dest, agent, target)| {
            Path::new(dest).ends_with("src/api/CLAUDE.md")
                && *agent == "claude"
                && *target == "modules"
        }));
        assert!(
            destinations
                .iter()
                .any(|(dest, _, _)| { Path::new(dest).ends_with("src/ui/CLAUDE.md") })
        );
    }

    #[test]
    fn test_validate_destinations_detects_duplicate_between_module_map_and_regular_target() {
        let module_map = module_map_config(
            r#"
            [[agents.claude.targets.modules.mappings]]
            source = "api.md"
            destination = "src/api"
            "#,
        );
        let module_target = &module_map.agents["claude"].targets["modules"];

        let regular_toml = r#"
            [agents.codex]
            enabled = true

            [agents.codex.targets.main]
            source = "AGENTS.md"
            destination = "src/api/CLAUDE.md"
            type = "symlink"
        "#;
        let regular: Config = toml::from_str(regular_toml).unwrap();
        let regular_target = &regular.agents["codex"].targets["main"];

        let mut destinations = expand_target_destinations("claude", "modules", module_target);
        destinations.extend(expand_target_destinations("codex", "main", regular_target));

        let conflicts = validate_destinations(&destinations);
        // Use Path for cross-platform comparison (Windows uses \ separator)
        assert!(conflicts.iter().any(|conflict| {
            matches!(conflict, Conflict::Duplicate(dest) if Path::new(dest).ends_with("src/api/CLAUDE.md"))
        }));
    }

    #[test]
    fn test_target_configuration_warnings_for_module_map_edge_cases() {
        let empty_module_map = module_map_config("");
        let empty_target = &empty_module_map.agents["claude"].targets["modules"];
        assert_eq!(
            target_configuration_warnings(empty_target),
            vec!["module-map target has no mappings configured"]
        );

        let wrong_type_toml = r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.main]
            source = "source.md"
            destination = "dest.md"
            type = "symlink"

            [[agents.claude.targets.main.mappings]]
            source = "api.md"
            destination = "src/api"
        "#;
        let wrong_type: Config = toml::from_str(wrong_type_toml).unwrap();
        let wrong_type_target = &wrong_type.agents["claude"].targets["main"];
        assert_eq!(wrong_type_target.sync_type, SyncType::Symlink);
        assert_eq!(
            target_configuration_warnings(wrong_type_target),
            vec!["mappings is only used by module-map targets"]
        );
    }
}