#[cfg(test)]
mod tests {
    use crate::commands::status::StatusEntry;
    use crate::commands::status::entry_is_problematic;
    use std::path::PathBuf;

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
}
