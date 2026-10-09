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
        CapStdUnixQuarantineOps, EntryIdentity, EntryLocation, MoveOutcome, RemoveDirectoryOutcome,
        RemoveOutcome, UnixQuarantineOps, move_entry_no_replace, move_unix_entry_no_replace,
        remove_empty_directory_if_unchanged, remove_symlink_if_unchanged, remove_unix,
        remove_unix_empty_directory,
    };
    use cap_std::ambient_authority;
    use cap_std::fs::Dir;
    use std::cell::Cell;
    use std::ffi::OsStr;
    use std::fs;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    #[cfg(target_os = "linux")]
    fn identity_without_creation_time_is_not_considered_same_generation() {
        let directory = Dir::open_ambient_dir("/proc/self", ambient_authority()).unwrap();
        let metadata = directory.metadata(".").unwrap();
        assert!(!EntryIdentity::capture(&metadata).matches(&metadata));
    }

    struct FailRemoveFileOps;

    impl UnixQuarantineOps for FailRemoveFileOps {
        fn remove_file_or_symlink(&self, _parent: &Dir, _name: &OsStr) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected unlink failure",
            ))
        }
    }

    #[derive(Clone, Copy)]
    enum InjectedFailure {
        MetadataAt(usize, std::io::ErrorKind),
        RenameAt(usize, std::io::ErrorKind),
        RenameAlways(std::io::ErrorKind),
        ReplaceOnMetadataAt(usize),
        OpenDir(std::io::ErrorKind),
        DirectoryMetadata(std::io::ErrorKind),
        DirectoryEntries(std::io::ErrorKind),
        RemoveDir(std::io::ErrorKind),
    }

    struct FaultOps {
        failure: InjectedFailure,
        metadata_calls: Cell<usize>,
        rename_calls: Cell<usize>,
        replacement_paths: Option<(PathBuf, PathBuf)>,
    }

    impl FaultOps {
        fn new(failure: InjectedFailure) -> Self {
            Self {
                failure,
                metadata_calls: Cell::new(0),
                rename_calls: Cell::new(0),
                replacement_paths: None,
            }
        }

        fn replace_on_metadata(
            call: usize,
            replacement_path: PathBuf,
            destination_path: PathBuf,
        ) -> Self {
            Self {
                failure: InjectedFailure::ReplaceOnMetadataAt(call),
                metadata_calls: Cell::new(0),
                rename_calls: Cell::new(0),
                replacement_paths: Some((replacement_path, destination_path)),
            }
        }

        fn error(kind: std::io::ErrorKind) -> std::io::Error {
            std::io::Error::new(kind, "injected quarantine operation failure")
        }
    }

    impl UnixQuarantineOps for FaultOps {
        fn symlink_metadata(
            &self,
            parent: &Dir,
            name: &OsStr,
        ) -> std::io::Result<cap_std::fs::Metadata> {
            let call = self.metadata_calls.get() + 1;
            self.metadata_calls.set(call);
            if let InjectedFailure::MetadataAt(fail_at, kind) = self.failure
                && call == fail_at
            {
                return Err(Self::error(kind));
            }
            if let InjectedFailure::ReplaceOnMetadataAt(fail_at) = self.failure
                && call == fail_at
            {
                let (replacement_path, destination_path) = self.replacement_paths.as_ref().unwrap();
                fs::rename(replacement_path, destination_path)?;
            }
            CapStdUnixQuarantineOps.symlink_metadata(parent, name)
        }

        fn rename_no_replace(&self, parent: &Dir, from: &OsStr, to: &OsStr) -> std::io::Result<()> {
            let call = self.rename_calls.get() + 1;
            self.rename_calls.set(call);
            match self.failure {
                InjectedFailure::RenameAt(fail_at, kind) if call == fail_at => {
                    return Err(Self::error(kind));
                }
                InjectedFailure::RenameAlways(kind) => return Err(Self::error(kind)),
                _ => {}
            }
            CapStdUnixQuarantineOps.rename_no_replace(parent, from, to)
        }

        fn open_dir_nofollow(&self, parent: &Dir, name: &OsStr) -> std::io::Result<Dir> {
            if let InjectedFailure::OpenDir(kind) = self.failure {
                return Err(Self::error(kind));
            }
            CapStdUnixQuarantineOps.open_dir_nofollow(parent, name)
        }

        fn directory_metadata(&self, directory: &Dir) -> std::io::Result<cap_std::fs::Metadata> {
            if let InjectedFailure::DirectoryMetadata(kind) = self.failure {
                return Err(Self::error(kind));
            }
            CapStdUnixQuarantineOps.directory_metadata(directory)
        }

        fn directory_has_entries(&self, directory: &Dir) -> std::io::Result<bool> {
            if let InjectedFailure::DirectoryEntries(kind) = self.failure {
                return Err(Self::error(kind));
            }
            CapStdUnixQuarantineOps.directory_has_entries(directory)
        }

        fn remove_dir(&self, parent: &Dir, name: &OsStr) -> std::io::Result<()> {
            if let InjectedFailure::RemoveDir(kind) = self.failure {
                return Err(Self::error(kind));
            }
            CapStdUnixQuarantineOps.remove_dir(parent, name)
        }
    }

    struct DifferentDirectoryIdentityOps {
        replacement_path: PathBuf,
    }

    impl UnixQuarantineOps for DifferentDirectoryIdentityOps {
        fn directory_metadata(&self, _directory: &Dir) -> std::io::Result<cap_std::fs::Metadata> {
            Dir::open_ambient_dir(&self.replacement_path, ambient_authority())?.metadata(".")
        }
    }

    #[test]
    fn remove_unix_restores_managed_link_when_unlink_fails() {
        let temp = TempDir::new().unwrap();
        let target_path = temp.path().join("target.md");
        let link_path = temp.path().join("managed.md");
        fs::write(&target_path, b"target bytes").unwrap();
        symlink(&target_path, &link_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed.md");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let error = remove_unix(
            &FailRemoveFileOps,
            &parent,
            name,
            expected,
            &link_path,
            || {},
            |_| {},
        )
        .err()
        .expect("unlink failure must be reported after restoring the link");

        assert!(error.to_string().contains("the entry was restored"));
        assert_eq!(fs::read_link(&link_path).unwrap(), target_path);
        assert_eq!(fs::read(&target_path).unwrap(), b"target bytes");
    }

    #[test]
    fn remove_unix_reports_rename_failure_without_losing_link() {
        let temp = TempDir::new().unwrap();
        let target_path = temp.path().join("target.md");
        let link_path = temp.path().join("managed.md");
        fs::write(&target_path, b"target bytes").unwrap();
        symlink(&target_path, &link_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed.md");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let ops = FaultOps::new(InjectedFailure::RenameAt(
            1,
            std::io::ErrorKind::PermissionDenied,
        ));
        let error = remove_unix(&ops, &parent, name, expected, &link_path, || {}, |_| {})
            .err()
            .expect("rename failure should be reported");

        assert!(error.to_string().contains("failed to quarantine"));
        assert_eq!(fs::read_link(&link_path).unwrap(), target_path);
    }

    #[test]
    fn remove_unix_stops_after_sixteen_name_collisions() {
        let temp = TempDir::new().unwrap();
        let target_path = temp.path().join("target.md");
        let link_path = temp.path().join("managed.md");
        fs::write(&target_path, b"target bytes").unwrap();
        symlink(&target_path, &link_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed.md");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let ops = FaultOps::new(InjectedFailure::RenameAlways(
            std::io::ErrorKind::AlreadyExists,
        ));
        let error = remove_unix(&ops, &parent, name, expected, &link_path, || {}, |_| {})
            .err()
            .expect("quarantine must stop after the bounded retry count");

        assert!(
            error
                .to_string()
                .contains("could not reserve a unique quarantine name")
        );
        assert_eq!(ops.rename_calls.get(), 16);
        assert_eq!(fs::read_link(&link_path).unwrap(), target_path);
    }

    #[test]
    fn remove_unix_empty_directory_restores_after_open_failure() {
        let temp = TempDir::new().unwrap();
        let directory_path = temp.path().join("managed");
        fs::create_dir(&directory_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let error = remove_unix_empty_directory(
            &FaultOps::new(InjectedFailure::OpenDir(
                std::io::ErrorKind::PermissionDenied,
            )),
            &parent,
            name,
            expected,
            &directory_path,
            || {},
            |_| {},
        )
        .err()
        .expect("open failure should be reported after restoring the directory");

        assert!(error.to_string().contains("the entry was restored"));
        assert!(directory_path.is_dir());
        assert_eq!(fs::read_dir(&directory_path).unwrap().count(), 0);
    }

    #[test]
    fn remove_unix_empty_directory_restores_after_metadata_failure() {
        let temp = TempDir::new().unwrap();
        let directory_path = temp.path().join("managed");
        fs::create_dir(&directory_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let error = remove_unix_empty_directory(
            &FaultOps::new(InjectedFailure::DirectoryMetadata(
                std::io::ErrorKind::PermissionDenied,
            )),
            &parent,
            name,
            expected,
            &directory_path,
            || {},
            |_| {},
        )
        .err()
        .expect("directory metadata failure should restore the directory");

        assert!(error.to_string().contains("the entry was restored"));
        assert!(directory_path.is_dir());
        assert_eq!(fs::read_dir(&directory_path).unwrap().count(), 0);
    }

    #[test]
    fn remove_unix_empty_directory_restores_when_opened_identity_changes() {
        let temp = TempDir::new().unwrap();
        let directory_path = temp.path().join("managed");
        let replacement_path = temp.path().join("replacement");
        fs::create_dir(&directory_path).unwrap();
        fs::create_dir(&replacement_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let ops = DifferentDirectoryIdentityOps { replacement_path };
        let outcome = remove_unix_empty_directory(
            &ops,
            &parent,
            name,
            expected,
            &directory_path,
            || {},
            |_| {},
        )
        .unwrap();

        assert!(matches!(outcome, RemoveDirectoryOutcome::Changed));
        assert!(directory_path.is_dir());
        assert_eq!(fs::read_dir(&directory_path).unwrap().count(), 0);
    }

    #[test]
    fn remove_unix_empty_directory_restores_after_entries_failure() {
        let temp = TempDir::new().unwrap();
        let directory_path = temp.path().join("managed");
        fs::create_dir(&directory_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let error = remove_unix_empty_directory(
            &FaultOps::new(InjectedFailure::DirectoryEntries(
                std::io::ErrorKind::PermissionDenied,
            )),
            &parent,
            name,
            expected,
            &directory_path,
            || {},
            |_| {},
        )
        .err()
        .expect("directory enumeration failure should restore the directory");

        assert!(error.to_string().contains("the entry was restored"));
        assert!(directory_path.is_dir());
        assert_eq!(fs::read_dir(&directory_path).unwrap().count(), 0);
    }

    #[test]
    fn remove_unix_empty_directory_restores_after_remove_failure() {
        let temp = TempDir::new().unwrap();
        let directory_path = temp.path().join("managed");
        fs::create_dir(&directory_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let error = remove_unix_empty_directory(
            &FaultOps::new(InjectedFailure::RemoveDir(
                std::io::ErrorKind::PermissionDenied,
            )),
            &parent,
            name,
            expected,
            &directory_path,
            || {},
            |_| {},
        )
        .err()
        .expect("directory removal failure should restore the directory");

        assert!(error.to_string().contains("the entry was restored"));
        assert!(directory_path.is_dir());
        assert_eq!(fs::read_dir(&directory_path).unwrap().count(), 0);
    }

    #[test]
    fn remove_unix_empty_directory_stops_after_sixteen_name_collisions() {
        let temp = TempDir::new().unwrap();
        let directory_path = temp.path().join("managed");
        fs::create_dir(&directory_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let ops = FaultOps::new(InjectedFailure::RenameAlways(
            std::io::ErrorKind::AlreadyExists,
        ));
        let error = remove_unix_empty_directory(
            &ops,
            &parent,
            name,
            expected,
            &directory_path,
            || {},
            |_| {},
        )
        .err()
        .expect("directory quarantine should stop after bounded retries");

        assert!(
            error
                .to_string()
                .contains("could not reserve a unique quarantine name")
        );
        assert_eq!(ops.rename_calls.get(), 16);
        assert!(directory_path.is_dir());
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

    #[test]
    fn move_unix_restores_backup_after_quarantine_metadata_failure() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("backup.bak");
        let destination_path = temp.path().join("restored.md");
        fs::write(&source_path, b"backup bytes").unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let source_name = OsStr::new("backup.bak");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(source_name).unwrap());
        let ops = FaultOps::new(InjectedFailure::MetadataAt(2, std::io::ErrorKind::NotFound));
        let error = move_unix_entry_no_replace(
            &ops,
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
        .err()
        .expect("quarantine metadata failure should restore the backup");

        assert!(error.to_string().contains("the entry was restored"));
        assert_eq!(fs::read(&source_path).unwrap(), b"backup bytes");
        assert!(!destination_path.exists());
    }

    #[test]
    fn move_unix_reports_published_backup_metadata_failure() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("backup.bak");
        let destination_path = temp.path().join("restored.md");
        fs::write(&source_path, b"backup bytes").unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let source_name = OsStr::new("backup.bak");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(source_name).unwrap());
        let ops = FaultOps::new(InjectedFailure::MetadataAt(3, std::io::ErrorKind::NotFound));
        let error = move_unix_entry_no_replace(
            &ops,
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
        .err()
        .expect("published backup metadata failure should be reported");

        assert!(
            error
                .to_string()
                .contains("failed to verify restored backup")
        );
        assert!(!source_path.exists());
        assert_eq!(fs::read(&destination_path).unwrap(), b"backup bytes");
    }

    #[test]
    fn move_unix_preserves_replacement_at_published_destination() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("backup.bak");
        let destination_path = temp.path().join("restored.md");
        let replacement_path = temp.path().join("replacement.tmp");
        fs::write(&source_path, b"backup bytes").unwrap();
        fs::write(&replacement_path, b"concurrent user data").unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let source_name = OsStr::new("backup.bak");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(source_name).unwrap());
        let ops = FaultOps::replace_on_metadata(3, replacement_path, destination_path.clone());
        let outcome = move_unix_entry_no_replace(
            &ops,
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
        assert_eq!(fs::read(&source_path).unwrap(), b"concurrent user data");
        assert!(!destination_path.exists());
    }

    #[test]
    fn move_unix_stops_after_sixteen_name_collisions() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("backup.bak");
        let destination_path = temp.path().join("restored.md");
        fs::write(&source_path, b"backup bytes").unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let source_name = OsStr::new("backup.bak");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(source_name).unwrap());
        let ops = FaultOps::new(InjectedFailure::RenameAlways(
            std::io::ErrorKind::AlreadyExists,
        ));
        let error = move_unix_entry_no_replace(
            &ops,
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
        .err()
        .expect("backup movement should stop after bounded retries");

        assert!(
            error
                .to_string()
                .contains("could not reserve a unique quarantine name")
        );
        assert_eq!(ops.rename_calls.get(), 16);
        assert_eq!(fs::read(&source_path).unwrap(), b"backup bytes");
    }

    #[test]
    fn remove_symlink_quarantine_disappearing_reports_restore_failure() {
        let temp = TempDir::new().unwrap();
        let target_path = temp.path().join("target.md");
        let link_path = temp.path().join("managed.md");
        fs::write(&target_path, b"target bytes").unwrap();
        symlink(&target_path, &link_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed.md");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let after_move_root = temp.path().to_path_buf();
        let link_for_callback = link_path.clone();
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
            fs::remove_file(quarantined).unwrap();
            fs::write(&link_for_callback, b"replacement occupant").unwrap();
        };

        let error =
            remove_symlink_if_unchanged(&parent, name, expected, &link_path, || {}, after_move)
                .err()
                .expect("missing quarantined symlink should be reported");

        assert!(error.to_string().contains("recovery entry remains"));
        assert_eq!(fs::read(&link_path).unwrap(), b"replacement occupant");
        assert_eq!(fs::read(&target_path).unwrap(), b"target bytes");
    }

    #[test]
    fn remove_empty_directory_quarantine_disappearing_reports_restore_failure() {
        let temp = TempDir::new().unwrap();
        let directory_path = temp.path().join("managed");
        fs::create_dir(&directory_path).unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let name = OsStr::new("managed");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(name).unwrap());
        let after_move_root = temp.path().to_path_buf();
        let path_for_callback = directory_path.clone();
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
            fs::remove_dir(quarantined).unwrap();
            fs::write(&path_for_callback, b"replacement occupant").unwrap();
        };

        let error = remove_empty_directory_if_unchanged(
            &parent,
            name,
            expected,
            &directory_path,
            || {},
            after_move,
        )
        .err()
        .expect("missing quarantined directory should be reported");

        assert!(error.to_string().contains("recovery entry remains"));
        assert_eq!(fs::read(&directory_path).unwrap(), b"replacement occupant");
    }

    #[test]
    fn move_backup_quarantine_disappearing_reports_restore_failure() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("backup.bak");
        let destination_path = temp.path().join("restored.md");
        fs::write(&source_path, b"backup bytes").unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let source_name = OsStr::new("backup.bak");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(source_name).unwrap());
        let after_quarantine_root = temp.path().to_path_buf();
        let source_for_callback = source_path.clone();
        let after_quarantine = move |_: &Path| {
            let quarantined = fs::read_dir(&after_quarantine_root)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with(".agentsync-quarantine-")
                    })
                })
                .unwrap();
            fs::remove_file(quarantined).unwrap();
            fs::write(&source_for_callback, b"replacement occupant").unwrap();
        };

        let error = move_entry_no_replace(
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
            after_quarantine,
        )
        .err()
        .expect("missing quarantined backup should be reported");

        assert!(error.to_string().contains("recovery entry remains"));
        assert_eq!(fs::read(&source_path).unwrap(), b"replacement occupant");
        assert!(!destination_path.exists());
    }

    #[test]
    fn move_quarantined_symlink_backup_restores_changed_entry() {
        let temp = TempDir::new().unwrap();
        let target_path = temp.path().join("target.md");
        let source_path = temp.path().join("backup.bak");
        let destination_path = temp.path().join("restored.md");
        fs::write(&target_path, b"target bytes").unwrap();
        fs::write(&source_path, b"backup bytes").unwrap();

        let parent = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        let source_name = OsStr::new("backup.bak");
        let expected = EntryIdentity::capture(&parent.symlink_metadata(source_name).unwrap());
        let after_quarantine_root = temp.path().to_path_buf();
        let target_for_callback = target_path.clone();
        let after_quarantine = move |_: &Path| {
            let quarantined = fs::read_dir(&after_quarantine_root)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with(".agentsync-quarantine-")
                    })
                })
                .unwrap();
            fs::remove_file(&quarantined).unwrap();
            symlink(&target_for_callback, quarantined).unwrap();
        };

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
            after_quarantine,
        )
        .unwrap();

        assert!(matches!(outcome, MoveOutcome::Changed));
        assert_eq!(fs::read_link(&source_path).unwrap(), target_path);
        assert_eq!(fs::read(&target_path).unwrap(), b"target bytes");
        assert!(!destination_path.exists());
    }
}
