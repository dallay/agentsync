#[cfg(windows)]
mod windows_tests {
    use crate::linker::quarantine::{
        EntryIdentity, RemoveOutcome, remove_symlink_if_unchanged, rename_open_handle,
    };
    use cap_std::ambient_authority;
    use cap_std::fs::{Dir, OpenOptions, OpenOptionsExt};
    use std::ffi::OsStr;
    use std::fs;
    use std::os::windows::fs::symlink_file;
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ, FILE_SHARE_WRITE, SYNCHRONIZE,
    };

    #[test]
    fn rename_open_handle_moves_backup_relative_to_verified_parent() {
        let temp = tempfile::TempDir::new().unwrap();
        let source_path = temp.path().join("backup.bak");
        let destination_path = temp.path().join("restored.md");
        fs::write(&source_path, b"original backup").unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .access_mode(DELETE | FILE_READ_ATTRIBUTES | SYNCHRONIZE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
        let source = parent
            .open_with(OsStr::new("backup.bak"), &options)
            .unwrap();

        rename_open_handle(&source, &parent, OsStr::new("restored.md"), temp.path()).unwrap();

        drop(source);
        assert!(!source_path.exists());
        assert_eq!(fs::read(destination_path).unwrap(), b"original backup");

        let source = temp.path().join("second-backup.bak");
        let occupied = temp.path().join("occupied.md");
        fs::write(&source, b"second backup").unwrap();
        fs::write(&occupied, b"user data").unwrap();
        let source_handle = parent
            .open_with(OsStr::new("second-backup.bak"), &options)
            .unwrap();

        assert!(
            rename_open_handle(
                &source_handle,
                &parent,
                OsStr::new("occupied.md"),
                temp.path()
            )
            .is_err()
        );
        drop(source_handle);
        assert_eq!(fs::read(source).unwrap(), b"second backup");
        assert_eq!(fs::read(occupied).unwrap(), b"user data");
    }

    #[test]
    fn rename_open_handle_rejects_parent_path_with_different_identity() {
        let temp = tempfile::TempDir::new().unwrap();
        let original_parent = temp.path().join("original-parent");
        let replacement_parent = temp.path().join("replacement-parent");
        fs::create_dir(&original_parent).unwrap();
        fs::create_dir(&replacement_parent).unwrap();
        let source_path = original_parent.join("backup.bak");
        let destination_path = replacement_parent.join("restored.md");
        fs::write(&source_path, b"original backup").unwrap();

        let parent = Dir::open_ambient_dir(&original_parent, ambient_authority()).unwrap();
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .access_mode(DELETE | FILE_READ_ATTRIBUTES | SYNCHRONIZE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
        let source = parent
            .open_with(OsStr::new("backup.bak"), &options)
            .unwrap();

        let error = rename_open_handle(
            &source,
            &parent,
            OsStr::new("restored.md"),
            &replacement_parent,
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read(&source_path).unwrap(), b"original backup");
        assert!(!destination_path.exists());
    }

    #[test]
    fn open_reparse_handle_matches_symlink_metadata_identity_and_is_removed() {
        let temp = tempfile::TempDir::new().unwrap();
        let target = temp.path().join("target.md");
        let link = temp.path().join("managed.md");
        fs::write(&target, "target").unwrap();
        symlink_file(&target, &link).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed.md");
        let observed = parent.symlink_metadata(name).unwrap();
        assert!(
            observed.file_type().is_symlink(),
            "parent symlink_metadata must identify the link itself: {observed:?}"
        );
        let expected = EntryIdentity::capture(&observed);

        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .access_mode(DELETE | FILE_READ_ATTRIBUTES | SYNCHRONIZE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
        let opened = parent.open_with(name, &options).unwrap();
        let handle_metadata = opened.metadata().unwrap();
        assert!(
            handle_metadata.file_type().is_symlink(),
            "the no-reparse-point handle must identify the link itself: {handle_metadata:?}"
        );
        let actual = EntryIdentity::capture(&handle_metadata);
        assert_eq!(
            expected, actual,
            "symlink_metadata identity must match the opened reparse-point handle"
        );
        drop(opened);

        let outcome =
            remove_symlink_if_unchanged(&parent, name, expected, &link, || {}, |_| {}).unwrap();
        assert!(
            matches!(outcome, RemoveOutcome::Removed),
            "matching symlink should be quarantined and removed; target={}",
            target.display()
        );
        assert!(
            parent
                .symlink_metadata(name)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
            "removed link should no longer be present"
        );
    }
}
