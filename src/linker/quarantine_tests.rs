#[cfg(windows)]
mod windows_tests {
    use crate::linker::quarantine::{
        EntryIdentity, EntryLocation, MoveOutcome, RemoveDirectoryOutcome, RemoveOutcome,
        move_entry_no_replace, remove_empty_directory_if_unchanged, remove_symlink_if_unchanged,
        rename_open_handle,
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

    #[test]
    fn remove_empty_directory_if_unchanged_removes_empty_directory() {
        let temp = tempfile::TempDir::new().unwrap();
        let directory_path = temp.path().join("managed");
        fs::create_dir(&directory_path).unwrap();
        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());

        let outcome = remove_empty_directory_if_unchanged(
            &parent,
            name,
            expected,
            &directory_path,
            || {},
            |_| {},
        )
        .unwrap();

        assert!(matches!(outcome, RemoveDirectoryOutcome::Removed));
        assert!(!directory_path.exists());
    }

    #[test]
    fn remove_nonempty_directory_if_unchanged_restores_contents() {
        let temp = tempfile::TempDir::new().unwrap();
        let directory_path = temp.path().join("managed");
        fs::create_dir(&directory_path).unwrap();
        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let temp_path = temp.path().to_path_buf();
        let after_move = move |_: &std::path::Path| {
            let quarantined = fs::read_dir(&temp_path)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with(".agentsync-quarantine-")
                    })
                })
                .unwrap();
            fs::write(quarantined.join("user-data.txt"), b"preserve me").unwrap();
        };

        let outcome = remove_empty_directory_if_unchanged(
            &parent,
            name,
            expected,
            &directory_path,
            || {},
            after_move,
        )
        .unwrap();

        assert!(matches!(outcome, RemoveDirectoryOutcome::NotEmpty));
        assert_eq!(
            fs::read(directory_path.join("user-data.txt")).unwrap(),
            b"preserve me"
        );
    }

    #[test]
    fn move_backup_no_replace_publishes_and_preserves_occupied_destination() {
        let temp = tempfile::TempDir::new().unwrap();
        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let source_path = temp.path().join("backup.bak");
        let destination_path = temp.path().join("restored.md");
        fs::write(&source_path, b"backup bytes").unwrap();
        let source_name = OsStr::new("backup.bak");
        let destination_name = OsStr::new("restored.md");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(source_name).unwrap());

        let outcome = move_entry_no_replace(
            EntryLocation {
                parent: &parent,
                name: source_name,
                path: &source_path,
            },
            EntryLocation {
                parent: &parent,
                name: destination_name,
                path: &destination_path,
            },
            expected,
            || {},
            |_| {},
        )
        .unwrap();
        assert!(matches!(outcome, MoveOutcome::Moved));
        assert_eq!(fs::read(&destination_path).unwrap(), b"backup bytes");
        assert!(!source_path.exists());

        let second_source_path = temp.path().join("second-backup.bak");
        let occupied_path = temp.path().join("occupied.md");
        fs::write(&second_source_path, b"second backup").unwrap();
        fs::write(&occupied_path, b"user data").unwrap();
        let second_source_name = OsStr::new("second-backup.bak");
        let second_expected =
            EntryIdentity::capture(&parent.symlink_metadata(second_source_name).unwrap());
        let error = move_entry_no_replace(
            EntryLocation {
                parent: &parent,
                name: second_source_name,
                path: &second_source_path,
            },
            EntryLocation {
                parent: &parent,
                name: OsStr::new("occupied.md"),
                path: &occupied_path,
            },
            second_expected,
            || {},
            |_| {},
        )
        .err()
        .expect("occupied destination must fail without overwriting user data");

        assert!(error.to_string().contains("failed to publish backup"));
        assert_eq!(fs::read(&second_source_path).unwrap(), b"second backup");
        assert_eq!(fs::read(&occupied_path).unwrap(), b"user data");
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_tests {
    use crate::linker::quarantine::{
        EntryIdentity, EntryLocation, MoveOutcome, RemoveDirectoryOutcome, RemoveOutcome,
        move_entry_no_replace, remove_empty_directory_if_unchanged, remove_symlink_if_unchanged,
    };
    use cap_std::ambient_authority;
    use cap_std::fs::Dir;
    use std::ffi::OsStr;
    use std::fs;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::symlink;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    #[cfg(target_os = "linux")]
    fn identity_without_creation_time_is_not_considered_same_generation() {
        let directory = Dir::open_ambient_dir("/proc/self", ambient_authority()).unwrap();
        let metadata = directory.metadata(".").unwrap();
        assert!(!EntryIdentity::capture(&metadata).matches(&metadata));
    }

    #[test]
    fn remove_matching_symlink_if_unchanged_removes_only_the_link() {
        let temp = TempDir::new().unwrap();
        let target_path = temp.path().join("target.md");
        let link_path = temp.path().join("managed.md");
        fs::write(&target_path, b"target bytes").unwrap();
        symlink(&target_path, &link_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed.md");
        let metadata = parent.symlink_metadata(name).unwrap();
        let expected = EntryIdentity::capture(&metadata);

        let outcome =
            remove_symlink_if_unchanged(&parent, name, expected, &link_path, || {}, |_| {})
                .unwrap();

        assert!(matches!(outcome, RemoveOutcome::Removed));
        assert!(
            parent
                .symlink_metadata(name)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        );
        assert_eq!(fs::read(&target_path).unwrap(), b"target bytes");
    }

    #[test]
    fn remove_replaced_symlink_if_unchanged_preserves_replacement() {
        let temp = TempDir::new().unwrap();
        let first_target = temp.path().join("first.md");
        let second_target = temp.path().join("second.md");
        let link_path = temp.path().join("managed.md");
        fs::write(&first_target, b"first target").unwrap();
        fs::write(&second_target, b"second target").unwrap();
        symlink(&first_target, &link_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed.md");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        fs::remove_file(&link_path).unwrap();
        symlink(&second_target, &link_path).unwrap();

        let outcome =
            remove_symlink_if_unchanged(&parent, name, expected, &link_path, || {}, |_| {})
                .unwrap();

        assert!(matches!(outcome, RemoveOutcome::Changed));
        assert_eq!(fs::read_link(&link_path).unwrap(), second_target);
        assert_eq!(fs::read(&first_target).unwrap(), b"first target");
        assert_eq!(fs::read(&second_target).unwrap(), b"second target");
    }

    #[test]
    fn remove_missing_symlink_if_unchanged_returns_changed() {
        let temp = TempDir::new().unwrap();
        let target_path = temp.path().join("target.md");
        let link_path = temp.path().join("managed.md");
        fs::write(&target_path, b"target bytes").unwrap();
        symlink(&target_path, &link_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed.md");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        fs::remove_file(&link_path).unwrap();

        let outcome =
            remove_symlink_if_unchanged(&parent, name, expected, &link_path, || {}, |_| {})
                .unwrap();

        assert!(matches!(outcome, RemoveOutcome::Changed));
        assert_eq!(fs::read(&target_path).unwrap(), b"target bytes");
    }

    #[test]
    fn remove_symlink_disappearing_before_quarantine_returns_changed() {
        let temp = TempDir::new().unwrap();
        let target_path = temp.path().join("target.md");
        let link_path = temp.path().join("managed.md");
        fs::write(&target_path, b"target bytes").unwrap();
        symlink(&target_path, &link_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed.md");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let link_before_move = link_path.clone();

        let outcome = remove_symlink_if_unchanged(
            &parent,
            name,
            expected,
            &link_path,
            move || fs::remove_file(link_before_move).unwrap(),
            |_| {},
        )
        .unwrap();

        assert!(matches!(outcome, RemoveOutcome::Changed));
        assert_eq!(fs::read(&target_path).unwrap(), b"target bytes");
    }

    #[test]
    fn remove_regular_file_if_unchanged_preserves_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("managed.md");
        fs::write(&file_path, b"user data").unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed.md");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());

        let outcome =
            remove_symlink_if_unchanged(&parent, name, expected, &file_path, || {}, |_| {})
                .unwrap();

        assert!(matches!(outcome, RemoveOutcome::Changed));
        assert_eq!(fs::read(&file_path).unwrap(), b"user data");
    }

    #[test]
    fn remove_replaced_quarantine_entry_restores_unexpected_file() {
        let temp = TempDir::new().unwrap();
        let target_path = temp.path().join("target.md");
        let link_path = temp.path().join("managed.md");
        fs::write(&target_path, b"target bytes").unwrap();
        symlink(&target_path, &link_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed.md");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let after_move_root = temp.path().to_path_buf();
        let after_move = move |_: &Path| {
            let quarantined = fs::read_dir(&after_move_root)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with(".agentsync-quarantine-")
                    })
                })
                .unwrap();
            fs::remove_file(&quarantined).unwrap();
            fs::write(quarantined, b"replacement data").unwrap();
        };

        let outcome =
            remove_symlink_if_unchanged(&parent, name, expected, &link_path, || {}, after_move)
                .unwrap();

        assert!(matches!(outcome, RemoveOutcome::Changed));
        assert_eq!(fs::read(&link_path).unwrap(), b"replacement data");
        assert_eq!(fs::read(&target_path).unwrap(), b"target bytes");
    }

    #[test]
    fn remove_matching_empty_directory_if_unchanged_removes_directory() {
        let temp = TempDir::new().unwrap();
        let directory_path = temp.path().join("managed");
        fs::create_dir(&directory_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());

        let outcome = remove_empty_directory_if_unchanged(
            &parent,
            name,
            expected,
            &directory_path,
            || {},
            |_| {},
        )
        .unwrap();

        assert!(matches!(outcome, RemoveDirectoryOutcome::Removed));
        assert!(!directory_path.exists());
    }

    #[test]
    fn remove_missing_empty_directory_returns_changed() {
        let temp = TempDir::new().unwrap();
        let directory_path = temp.path().join("managed");
        fs::create_dir(&directory_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        fs::remove_dir(&directory_path).unwrap();

        let outcome = remove_empty_directory_if_unchanged(
            &parent,
            name,
            expected,
            &directory_path,
            || {},
            |_| {},
        )
        .unwrap();

        assert!(matches!(outcome, RemoveDirectoryOutcome::Changed));
    }

    #[test]
    fn remove_replaced_empty_directory_if_unchanged_preserves_replacement() {
        let temp = TempDir::new().unwrap();
        let directory_path = temp.path().join("managed");
        let replacement_path = temp.path().join("replacement");
        fs::create_dir(&directory_path).unwrap();
        fs::create_dir(&replacement_path).unwrap();
        fs::write(replacement_path.join("user-data.txt"), b"preserve me").unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        fs::remove_dir(&directory_path).unwrap();
        fs::rename(&replacement_path, &directory_path).unwrap();

        let outcome = remove_empty_directory_if_unchanged(
            &parent,
            name,
            expected,
            &directory_path,
            || {},
            |_| {},
        )
        .unwrap();

        assert!(matches!(outcome, RemoveDirectoryOutcome::Changed));
        assert_eq!(
            fs::read(directory_path.join("user-data.txt")).unwrap(),
            b"preserve me"
        );
    }

    #[test]
    fn remove_regular_file_as_empty_directory_returns_changed() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("managed");
        fs::write(&file_path, b"user data").unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());

        let outcome =
            remove_empty_directory_if_unchanged(&parent, name, expected, &file_path, || {}, |_| {})
                .unwrap();

        assert!(matches!(outcome, RemoveDirectoryOutcome::Changed));
        assert_eq!(fs::read(&file_path).unwrap(), b"user data");
    }

    #[test]
    fn remove_empty_directory_disappearing_before_quarantine_returns_changed() {
        let temp = TempDir::new().unwrap();
        let directory_path = temp.path().join("managed");
        fs::create_dir(&directory_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let directory_before_move = directory_path.clone();

        let outcome = remove_empty_directory_if_unchanged(
            &parent,
            name,
            expected,
            &directory_path,
            move || fs::remove_dir(directory_before_move).unwrap(),
            |_| {},
        )
        .unwrap();

        assert!(matches!(outcome, RemoveDirectoryOutcome::Changed));
        assert!(!directory_path.exists());
    }

    #[test]
    fn remove_directory_if_changed_to_nonempty_restores_its_contents() {
        let temp = TempDir::new().unwrap();
        let directory_path = temp.path().join("managed");
        fs::create_dir(&directory_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let after_move_root = temp.path().to_path_buf();
        let after_move = move |_: &Path| {
            let quarantined = fs::read_dir(&after_move_root)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with(".agentsync-quarantine-")
                    })
                })
                .unwrap();
            fs::write(quarantined.join("user-data.txt"), b"preserve me").unwrap();
        };

        let outcome = remove_empty_directory_if_unchanged(
            &parent,
            name,
            expected,
            &directory_path,
            || {},
            after_move,
        )
        .unwrap();

        assert!(matches!(outcome, RemoveDirectoryOutcome::NotEmpty));
        assert_eq!(
            fs::read(directory_path.join("user-data.txt")).unwrap(),
            b"preserve me"
        );
    }

    #[test]
    fn remove_replaced_quarantine_directory_restores_symlink() {
        let temp = TempDir::new().unwrap();
        let target_path = temp.path().join("target");
        let directory_path = temp.path().join("managed");
        fs::create_dir(&target_path).unwrap();
        fs::create_dir(&directory_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let after_move_root = temp.path().to_path_buf();
        let target_for_callback = target_path.clone();
        let after_move = move |_: &Path| {
            let quarantined = fs::read_dir(&after_move_root)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with(".agentsync-quarantine-")
                    })
                })
                .unwrap();
            fs::remove_dir(&quarantined).unwrap();
            symlink(&target_for_callback, quarantined).unwrap();
        };

        let outcome = remove_empty_directory_if_unchanged(
            &parent,
            name,
            expected,
            &directory_path,
            || {},
            after_move,
        )
        .unwrap();

        assert!(matches!(outcome, RemoveDirectoryOutcome::Changed));
        assert_eq!(fs::read_link(&directory_path).unwrap(), target_path);
    }

    #[test]
    fn move_matching_backup_if_unchanged_publishes_at_destination() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("backup.bak");
        let destination_path = temp.path().join("restored.md");
        fs::write(&source_path, b"backup bytes").unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let source_name = OsStr::new("backup.bak");
        let destination_name = OsStr::new("restored.md");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(source_name).unwrap());
        let outcome = move_entry_no_replace(
            EntryLocation {
                parent: &parent,
                name: source_name,
                path: &source_path,
            },
            EntryLocation {
                parent: &parent,
                name: destination_name,
                path: &destination_path,
            },
            expected,
            || {},
            |_| {},
        )
        .unwrap();

        assert!(matches!(outcome, MoveOutcome::Moved));
        assert!(!source_path.exists());
        assert_eq!(fs::read(&destination_path).unwrap(), b"backup bytes");
    }

    #[test]
    fn move_backup_disappearing_before_quarantine_returns_changed() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("backup.bak");
        let destination_path = temp.path().join("restored.md");
        fs::write(&source_path, b"backup bytes").unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let source_name = OsStr::new("backup.bak");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(source_name).unwrap());
        let source_before_move = source_path.clone();
        let outcome = move_entry_no_replace(
            EntryLocation {
                parent: &parent,
                name: source_name,
                path: &source_path,
            },
            EntryLocation {
                parent: &parent,
                name: OsStr::new("restored.md"),
                path: &destination_path,
            },
            expected,
            move || fs::remove_file(source_before_move).unwrap(),
            |_| {},
        )
        .unwrap();

        assert!(matches!(outcome, MoveOutcome::Changed));
        assert!(!source_path.exists());
        assert!(!destination_path.exists());
    }

    #[test]
    fn move_symlink_backup_if_unchanged_preserves_source() {
        let temp = TempDir::new().unwrap();
        let target_path = temp.path().join("target.md");
        let source_path = temp.path().join("backup.bak");
        let destination_path = temp.path().join("restored.md");
        fs::write(&target_path, b"target bytes").unwrap();
        symlink(&target_path, &source_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let source_name = OsStr::new("backup.bak");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(source_name).unwrap());
        let outcome = move_entry_no_replace(
            EntryLocation {
                parent: &parent,
                name: source_name,
                path: &source_path,
            },
            EntryLocation {
                parent: &parent,
                name: OsStr::new("restored.md"),
                path: &destination_path,
            },
            expected,
            || {},
            |_| {},
        )
        .unwrap();

        assert!(matches!(outcome, MoveOutcome::Changed));
        assert_eq!(fs::read_link(&source_path).unwrap(), target_path);
        assert!(!destination_path.exists());
    }

    #[test]
    fn move_backup_rejects_distinct_parent_capabilities() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("backup.bak");
        let destination_path = temp.path().join("restored.md");
        fs::write(&source_path, b"backup bytes").unwrap();

        let source_parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let destination_parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let source_name = OsStr::new("backup.bak");
        let expected =
            EntryIdentity::capture(&source_parent.symlink_metadata(source_name).unwrap());
        let result = move_entry_no_replace(
            EntryLocation {
                parent: &source_parent,
                name: source_name,
                path: &source_path,
            },
            EntryLocation {
                parent: &destination_parent,
                name: OsStr::new("restored.md"),
                path: &destination_path,
            },
            expected,
            || {},
            |_| {},
        );

        assert!(result.is_err());
        assert_eq!(fs::read(&source_path).unwrap(), b"backup bytes");
        assert!(!destination_path.exists());
    }

    #[test]
    fn move_backup_if_destination_exists_restores_source_without_overwrite() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("backup.bak");
        let destination_path = temp.path().join("restored.md");
        fs::write(&source_path, b"backup bytes").unwrap();
        fs::write(&destination_path, b"user data").unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let source_name = OsStr::new("backup.bak");
        let destination_name = OsStr::new("restored.md");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(source_name).unwrap());
        let result = move_entry_no_replace(
            EntryLocation {
                parent: &parent,
                name: source_name,
                path: &source_path,
            },
            EntryLocation {
                parent: &parent,
                name: destination_name,
                path: &destination_path,
            },
            expected,
            || {},
            |_| {},
        );

        assert!(result.is_err());
        assert_eq!(fs::read(&source_path).unwrap(), b"backup bytes");
        assert_eq!(fs::read(&destination_path).unwrap(), b"user data");
    }

    #[test]
    fn remove_symlink_with_invalid_name_returns_io_error() {
        let temp = TempDir::new().unwrap();
        let placeholder = temp.path().join("placeholder");
        let display_path = temp.path().join("display");
        fs::write(&placeholder, b"placeholder").unwrap();
        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let expected = EntryIdentity::capture(&parent.symlink_metadata("placeholder").unwrap());
        let invalid_name = OsStr::from_bytes(b"invalid\0name");

        let error = remove_symlink_if_unchanged(
            &parent,
            invalid_name,
            expected,
            &display_path,
            || {},
            |_| {},
        )
        .err()
        .expect("invalid symlink name should fail before quarantine");

        assert!(error.to_string().contains("failed to inspect"));
    }

    #[test]
    fn remove_empty_directory_with_invalid_name_returns_io_error() {
        let temp = TempDir::new().unwrap();
        let placeholder = temp.path().join("placeholder");
        let display_path = temp.path().join("display");
        fs::write(&placeholder, b"placeholder").unwrap();
        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let expected = EntryIdentity::capture(&parent.symlink_metadata("placeholder").unwrap());
        let invalid_name = OsStr::from_bytes(b"invalid\0name");

        let error = remove_empty_directory_if_unchanged(
            &parent,
            invalid_name,
            expected,
            &display_path,
            || {},
            |_| {},
        )
        .err()
        .expect("invalid directory name should fail before quarantine");

        assert_eq!(
            error
                .root_cause()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn move_backup_with_invalid_name_returns_io_error() {
        let temp = TempDir::new().unwrap();
        let placeholder = temp.path().join("placeholder");
        let source_path = temp.path().join("source");
        let destination_path = temp.path().join("destination");
        fs::write(&placeholder, b"placeholder").unwrap();
        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let expected = EntryIdentity::capture(&parent.symlink_metadata("placeholder").unwrap());
        let invalid_name = OsStr::from_bytes(b"invalid\0name");

        let error = move_entry_no_replace(
            EntryLocation {
                parent: &parent,
                name: invalid_name,
                path: &source_path,
            },
            EntryLocation {
                parent: &parent,
                name: OsStr::new("destination"),
                path: &destination_path,
            },
            expected,
            || {},
            |_| {},
        )
        .err()
        .expect("invalid source name should fail before quarantine");

        assert_eq!(
            error
                .root_cause()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
    }
}
