#[cfg(test)]
mod tests {
    use crate::commands::doctor::{Conflict, extract_managed_entries, validate_destinations};

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
}
