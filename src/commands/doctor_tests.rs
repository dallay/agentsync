#[cfg(test)]
mod tests {
    use crate::commands::doctor::{
        Conflict, extract_managed_entries, normalize_path, validate_destinations,
    };
    use std::path::PathBuf;

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
    }
}
