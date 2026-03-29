#[cfg(test)]
mod tests {
    use crate::commands::status::{
        StatusEntry, collect_status_entries, collect_status_hints, entry_is_problematic,
    };
    use agentsync::{Linker, config::Config, linker::SyncOptions};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // simple canonicalize that just joins relative paths to base and returns Some
    fn normalize_path(p: PathBuf) -> PathBuf {
        use std::path::Component;
        let mut out = if p.is_absolute() {
            PathBuf::from("/")
        } else {
            PathBuf::new()
        };
        for c in p.components() {
            match c {
                Component::ParentDir => {
                    out.pop();
                }
                Component::CurDir => {}
                Component::RootDir => out = PathBuf::from("/"),
                Component::Normal(s) => out.push(s),
                Component::Prefix(pfx) => out.push(pfx.as_os_str()),
            }
        }
        out
    }

    fn fake_canonicalize(p: &std::path::Path, base: Option<&std::path::Path>) -> Option<PathBuf> {
        let joined = if p.is_absolute() {
            p.to_path_buf()
        } else if let Some(b) = base {
            b.join(p)
        } else {
            std::env::current_dir().unwrap().join(p)
        };
        Some(normalize_path(joined))
    }

    fn load_linker(temp_dir: &TempDir, config_content: &str) -> (Linker, PathBuf) {
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        let config_path = agents_dir.join("agentsync.toml");
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();
        (Linker::new(config, config_path.clone()), config_path)
    }

    #[test]
    fn test_missing_entry_is_problematic() {
        let e = StatusEntry {
            destination: "/tmp/dest".into(),
            exists: false,
            is_symlink: false,
            points_to: None,
            expected_source: None,
        };
        assert!(entry_is_problematic(&e, fake_canonicalize));
    }

    #[test]
    fn test_non_symlink_exists_problematic() {
        let e = StatusEntry {
            destination: "/tmp/dest".into(),
            exists: true,
            is_symlink: false,
            points_to: None,
            expected_source: None,
        };
        // Non-symlink destinations are considered problematic (drift)
        assert!(entry_is_problematic(&e, fake_canonicalize));
    }

    #[test]
    fn test_symlink_points_equal_not_problematic() {
        let e = StatusEntry {
            destination: "/home/user/project/sub/dest".into(),
            exists: true,
            is_symlink: true,
            points_to: Some("../src/lib".into()),
            expected_source: Some("/home/user/project/src/lib".into()),
        };
        // fake canonicalize will resolve ../src/lib relative to parent of dest
        assert!(!entry_is_problematic(&e, fake_canonicalize));
    }

    #[test]
    fn test_symlink_points_different_is_problematic() {
        let e = StatusEntry {
            destination: "/tmp/dest".into(),
            exists: true,
            is_symlink: true,
            points_to: Some("/other/place".into()),
            expected_source: Some("/expected/place".into()),
        };
        assert!(entry_is_problematic(&e, fake_canonicalize));
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

        let entries = collect_status_entries(&linker, &config_path);
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

        let entries = collect_status_entries(&linker, &config_path);
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
        assert!(entry_is_problematic(entry, fake_canonicalize));
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

        let config = r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.skills]
            source = "skills"
            destination = ".claude/skills"
            type = "symlink-contents"
        "#;

        let (linker, config_path) = load_linker(&temp_dir, config);
        let entries = collect_status_entries(&linker, &config_path);
        assert_eq!(entries.len(), 1);
        assert!(!entry_is_problematic(&entries[0], fake_canonicalize));

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

        let config = r#"
            [agents.claude]
            enabled = true

            [agents.claude.targets.skills]
            source = "skills"
            destination = ".claude/skills"
            type = "symlink"
        "#;

        let (linker, config_path) = load_linker(&temp_dir, config);
        let hints = collect_status_hints(&linker, &config_path);
        assert!(hints.is_empty());
    }
}
