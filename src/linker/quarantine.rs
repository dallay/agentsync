//! Conditional removal of managed filesystem entries.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;

use anyhow::Context;
use cap_fs_ext::{DirExt, MetadataExt};
use cap_std::fs::Dir as CapabilityDir;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EntryIdentity {
    device: u64,
    file: u64,
}

pub(super) struct EntryLocation<'a> {
    pub parent: &'a CapabilityDir,
    pub name: &'a OsStr,
    pub path: &'a Path,
}

impl EntryIdentity {
    pub(super) fn capture(metadata: &cap_std::fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            file: metadata.ino(),
        }
    }

    pub(super) fn matches(self, metadata: &cap_std::fs::Metadata) -> bool {
        Self::capture(metadata) == self
    }
}

pub(super) enum RemoveOutcome {
    Removed,
    Changed,
}

pub(super) enum RemoveDirectoryOutcome {
    Removed,
    Changed,
    NotEmpty,
}

pub(super) enum MoveOutcome {
    Moved,
    Changed,
}

/// Remove a symlink only if the object at `name` is still the object observed
/// during enumeration. The temporary sibling name is random and is published
/// with an atomic no-replace rename before the identity is checked again.
pub(super) fn remove_symlink_if_unchanged<F, G>(
    parent: &CapabilityDir,
    name: &OsStr,
    expected: EntryIdentity,
    display_path: &Path,
    before_move: F,
    after_move: G,
) -> anyhow::Result<RemoveOutcome>
where
    F: FnOnce(),
    G: FnOnce(&Path),
{
    #[cfg(windows)]
    {
        return remove_windows(
            parent,
            name,
            expected,
            display_path,
            before_move,
            after_move,
        );
    }

    #[cfg(not(windows))]
    {
        remove_unix(
            parent,
            name,
            expected,
            display_path,
            before_move,
            after_move,
        )
    }
}

pub(super) fn remove_empty_directory_if_unchanged<F, G>(
    parent: &CapabilityDir,
    name: &OsStr,
    expected: EntryIdentity,
    display_path: &Path,
    before_move: F,
    after_move: G,
) -> anyhow::Result<RemoveDirectoryOutcome>
where
    F: FnOnce(),
    G: FnOnce(&Path),
{
    #[cfg(windows)]
    {
        return remove_windows_empty_directory(
            parent,
            name,
            expected,
            display_path,
            before_move,
            after_move,
        );
    }

    #[cfg(not(windows))]
    {
        remove_unix_empty_directory(
            parent,
            name,
            expected,
            display_path,
            before_move,
            after_move,
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn move_entry_no_replace<F, G>(
    source: EntryLocation<'_>,
    destination: EntryLocation<'_>,
    expected: EntryIdentity,
    before_move: F,
    after_quarantine: G,
) -> anyhow::Result<MoveOutcome>
where
    F: FnOnce(),
    G: FnOnce(&Path),
{
    #[cfg(windows)]
    {
        return move_windows_entry_no_replace(
            source,
            destination,
            expected,
            before_move,
            after_quarantine,
        );
    }

    #[cfg(not(windows))]
    {
        move_unix_entry_no_replace(source, destination, expected, before_move, after_quarantine)
    }
}

#[cfg(not(windows))]
fn remove_unix<F, G>(
    parent: &CapabilityDir,
    name: &OsStr,
    expected: EntryIdentity,
    display_path: &Path,
    before_move: F,
    after_move: G,
) -> anyhow::Result<RemoveOutcome>
where
    F: FnOnce(),
    G: FnOnce(&Path),
{
    let before = match parent.symlink_metadata(name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RemoveOutcome::Changed);
        }
        Err(error) => {
            return Err(anyhow::Error::from(error).context(format!(
                "failed to inspect {} before quarantine",
                display_path.display()
            )));
        }
    };
    if !before.file_type().is_symlink() || !expected.matches(&before) {
        return Ok(RemoveOutcome::Changed);
    }
    before_move();

    for _ in 0..16 {
        let quarantine_name = random_quarantine_name();
        match rename_no_replace(parent, name, &quarantine_name) {
            Ok(()) => {
                after_move(display_path);
                let quarantined = match parent.symlink_metadata(&quarantine_name) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        return Err(restore_or_report(
                            parent,
                            &quarantine_name,
                            name,
                            display_path,
                            error,
                        ));
                    }
                };
                if !quarantined.file_type().is_symlink() || !expected.matches(&quarantined) {
                    restore_quarantined(parent, &quarantine_name, name, display_path)?;
                    return Ok(RemoveOutcome::Changed);
                }

                if let Err(error) = parent.remove_file_or_symlink(&quarantine_name) {
                    return Err(restore_or_report(
                        parent,
                        &quarantine_name,
                        name,
                        display_path,
                        error,
                    ));
                }
                return Ok(RemoveOutcome::Removed);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(RemoveOutcome::Changed);
            }
            Err(error) => {
                return Err(anyhow::Error::from(error)
                    .context(format!("failed to quarantine {}", display_path.display())));
            }
        }
    }

    anyhow::bail!(
        "could not reserve a unique quarantine name for {}",
        display_path.display()
    )
}

#[cfg(not(windows))]
fn remove_unix_empty_directory<F, G>(
    parent: &CapabilityDir,
    name: &OsStr,
    expected: EntryIdentity,
    display_path: &Path,
    before_move: F,
    after_move: G,
) -> anyhow::Result<RemoveDirectoryOutcome>
where
    F: FnOnce(),
    G: FnOnce(&Path),
{
    let before = match parent.symlink_metadata(name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RemoveDirectoryOutcome::Changed);
        }
        Err(error) => return Err(error.into()),
    };
    if before.file_type().is_symlink() || !before.is_dir() || !expected.matches(&before) {
        return Ok(RemoveDirectoryOutcome::Changed);
    }
    before_move();

    for _ in 0..16 {
        let quarantine_name = random_quarantine_name();
        match rename_no_replace(parent, name, &quarantine_name) {
            Ok(()) => {
                after_move(display_path);
                let quarantined = match parent.symlink_metadata(&quarantine_name) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        return Err(restore_or_report(
                            parent,
                            &quarantine_name,
                            name,
                            display_path,
                            error,
                        ));
                    }
                };
                if quarantined.file_type().is_symlink()
                    || !quarantined.is_dir()
                    || !expected.matches(&quarantined)
                {
                    restore_quarantined(parent, &quarantine_name, name, display_path)?;
                    return Ok(RemoveDirectoryOutcome::Changed);
                }
                let directory = match parent.open_dir_nofollow(&quarantine_name) {
                    Ok(directory) => directory,
                    Err(error) => {
                        return Err(restore_or_report(
                            parent,
                            &quarantine_name,
                            name,
                            display_path,
                            error,
                        ));
                    }
                };
                let directory_metadata = match directory.metadata(".") {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        return Err(restore_or_report(
                            parent,
                            &quarantine_name,
                            name,
                            display_path,
                            error,
                        ));
                    }
                };
                let directory_identity = EntryIdentity::capture(&directory_metadata);
                if directory_identity != expected {
                    restore_quarantined(parent, &quarantine_name, name, display_path)?;
                    return Ok(RemoveDirectoryOutcome::Changed);
                }
                let mut entries = match directory.entries() {
                    Ok(entries) => entries,
                    Err(error) => {
                        return Err(restore_or_report(
                            parent,
                            &quarantine_name,
                            name,
                            display_path,
                            error,
                        ));
                    }
                };
                match entries.next() {
                    None => {}
                    Some(Ok(_)) => {
                        restore_quarantined(parent, &quarantine_name, name, display_path)?;
                        return Ok(RemoveDirectoryOutcome::NotEmpty);
                    }
                    Some(Err(error)) => {
                        return Err(restore_or_report(
                            parent,
                            &quarantine_name,
                            name,
                            display_path,
                            error,
                        ));
                    }
                }
                drop(entries);
                drop(directory);
                if let Err(error) = parent.remove_dir(&quarantine_name) {
                    return Err(restore_or_report(
                        parent,
                        &quarantine_name,
                        name,
                        display_path,
                        error,
                    ));
                }
                return Ok(RemoveDirectoryOutcome::Removed);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(RemoveDirectoryOutcome::Changed);
            }
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!(
        "could not reserve a unique quarantine name for {}",
        display_path.display()
    )
}

#[cfg(not(windows))]
#[allow(clippy::too_many_arguments)]
fn move_unix_entry_no_replace<F, G>(
    source: EntryLocation<'_>,
    destination: EntryLocation<'_>,
    expected: EntryIdentity,
    before_move: F,
    after_quarantine: G,
) -> anyhow::Result<MoveOutcome>
where
    F: FnOnce(),
    G: FnOnce(&Path),
{
    anyhow::ensure!(
        std::ptr::eq(source.parent, destination.parent),
        "backup source and destination must share a parent capability"
    );
    let parent = source.parent;
    let source_name = source.name;
    let destination_name = destination.name;
    let source_path = source.path;
    let destination_path = destination.path;
    let before = match parent.symlink_metadata(source_name) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(MoveOutcome::Changed);
        }
        Err(error) => return Err(error.into()),
    };
    if before.file_type().is_symlink()
        || (!before.is_file() && !before.is_dir())
        || !expected.matches(&before)
    {
        return Ok(MoveOutcome::Changed);
    }
    before_move();

    for _ in 0..16 {
        let quarantine_name = random_quarantine_name();
        match rename_no_replace(parent, source_name, &quarantine_name) {
            Ok(()) => {
                after_quarantine(source_path);
                let quarantined = match parent.symlink_metadata(&quarantine_name) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        return Err(restore_or_report(
                            parent,
                            &quarantine_name,
                            source_name,
                            source_path,
                            error,
                        ));
                    }
                };
                if quarantined.file_type().is_symlink()
                    || (!quarantined.is_file() && !quarantined.is_dir())
                    || !expected.matches(&quarantined)
                {
                    restore_quarantined(parent, &quarantine_name, source_name, source_path)?;
                    return Ok(MoveOutcome::Changed);
                }

                if let Err(error) = rename_no_replace(parent, &quarantine_name, destination_name) {
                    return Err(restore_or_report(
                        parent,
                        &quarantine_name,
                        source_name,
                        source_path,
                        error,
                    ))
                    .with_context(|| {
                        format!("failed to publish backup at {}", destination_path.display())
                    });
                }
                let published = match parent.symlink_metadata(destination_name) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        return Err(anyhow::Error::from(error).context(format!(
                            "failed to verify restored backup at {}",
                            destination_path.display()
                        )));
                    }
                };
                if published.file_type().is_symlink()
                    || (!published.is_file() && !published.is_dir())
                    || !expected.matches(&published)
                {
                    let quarantine_name = random_quarantine_name();
                    rename_no_replace(parent, destination_name, &quarantine_name).map_err(|error| {
                        anyhow::anyhow!(
                            "restored backup at {} changed identity and could not be quarantined: {}; preserve the current destination",
                            destination_path.display(),
                            error
                        )
                    })?;
                    restore_quarantined(parent, &quarantine_name, source_name, source_path)?;
                    return Ok(MoveOutcome::Changed);
                }
                return Ok(MoveOutcome::Moved);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(MoveOutcome::Changed);
            }
            Err(error) => return Err(error.into()),
        }
    }

    anyhow::bail!(
        "could not reserve a unique quarantine name for {}",
        source_path.display()
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn rename_no_replace(parent: &CapabilityDir, from: &OsStr, to: &OsStr) -> io::Result<()> {
    rustix::fs::renameat_with(parent, from, parent, to, rustix::fs::RenameFlags::NOREPLACE)
        .map_err(io::Error::from)
}

pub(super) fn rename_between_no_replace(
    source_parent: &CapabilityDir,
    from: &OsStr,
    destination_parent: &CapabilityDir,
    to: &OsStr,
) -> io::Result<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        rustix::fs::renameat_with(
            source_parent,
            from,
            destination_parent,
            to,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(io::Error::from)
    }

    #[cfg(windows)]
    {
        use cap_std::fs::{OpenOptions, OpenOptionsExt};
        use windows_sys::Win32::Storage::FileSystem::{
            DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ, FILE_SHARE_WRITE, SYNCHRONIZE,
        };
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .access_mode(DELETE | FILE_READ_ATTRIBUTES | SYNCHRONIZE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
        let file = source_parent.open_with(from, &options)?;
        return rename_open_handle(&file, destination_parent, to);
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = (source_parent, from, destination_parent, to);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "handle-relative no-replace rename is unsupported on this platform",
        ))
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn rename_no_replace(_parent: &CapabilityDir, _from: &OsStr, _to: &OsStr) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "handle-relative no-replace rename is unsupported on this platform",
    ))
}

#[cfg(not(windows))]
fn restore_quarantined(
    parent: &CapabilityDir,
    quarantine_name: &OsStr,
    original_name: &OsStr,
    display_path: &Path,
) -> anyhow::Result<()> {
    rename_no_replace(parent, quarantine_name, original_name).map_err(|error| {
        anyhow::anyhow!(
            "could not restore changed entry for {}: {}; recovery entry remains at {}",
            display_path.display(),
            error,
            quarantine_path(display_path, quarantine_name).display()
        )
    })
}

#[cfg(not(windows))]
fn restore_or_report(
    parent: &CapabilityDir,
    quarantine_name: &OsStr,
    original_name: &OsStr,
    display_path: &Path,
    cause: io::Error,
) -> anyhow::Error {
    match restore_quarantined(parent, quarantine_name, original_name, display_path) {
        Ok(()) => anyhow::Error::from(cause).context(format!(
            "failed to remove quarantined symlink for {} (the entry was restored)",
            display_path.display()
        )),
        Err(restore_error) => anyhow::anyhow!(
            "{}; {}; recovery entry remains at {}",
            cause,
            restore_error,
            quarantine_path(display_path, quarantine_name).display()
        ),
    }
}

#[cfg(not(windows))]
fn random_quarantine_name() -> OsString {
    OsString::from(format!(
        ".agentsync-quarantine-{:032x}",
        rand::random::<u128>()
    ))
}

#[cfg(not(windows))]
fn quarantine_path(display_path: &Path, quarantine_name: &OsStr) -> std::path::PathBuf {
    display_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(quarantine_name)
}

#[cfg(windows)]
fn remove_windows<F, G>(
    parent: &CapabilityDir,
    name: &OsStr,
    expected: EntryIdentity,
    display_path: &Path,
    before_move: F,
    after_move: G,
) -> anyhow::Result<RemoveOutcome>
where
    F: FnOnce(),
    G: FnOnce(&Path),
{
    use cap_std::fs::{OpenOptions, OpenOptionsExt};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_DISPOSITION_FLAG_DELETE, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
        FileDispositionInfoEx, SYNCHRONIZE, SetFileInformationByHandle,
    };

    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .access_mode(DELETE | FILE_READ_ATTRIBUTES | SYNCHRONIZE)
        // Deliberately omit FILE_SHARE_DELETE while holding this handle: a
        // concurrent process cannot rename/unlink the inspected entry before
        // our handle-relative quarantine rename completes.
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
    let file = match parent.open_with(name, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RemoveOutcome::Changed);
        }
        Err(error) => return Err(error).map_err(anyhow::Error::from),
    };
    let metadata = file.metadata()?;
    if !metadata.file_type().is_symlink() || !expected.matches(&metadata) {
        return Ok(RemoveOutcome::Changed);
    }
    before_move();

    for _ in 0..16 {
        let quarantine_name = random_quarantine_name();
        match rename_open_handle(&file, parent, &quarantine_name) {
            Ok(()) => {
                after_move(display_path);
                let disposition =
                    windows_sys::Win32::Storage::FileSystem::FILE_DISPOSITION_INFO_EX {
                        Flags: FILE_DISPOSITION_FLAG_DELETE,
                    };
                // SAFETY: `file` is a valid handle opened with DELETE access;
                // `disposition` is the documented input structure for
                // FileDispositionInfoEx.
                let deleted = unsafe {
                    SetFileInformationByHandle(
                        file.as_raw_handle() as HANDLE,
                        FileDispositionInfoEx,
                        (&disposition as *const _).cast(),
                        std::mem::size_of_val(&disposition) as u32,
                    )
                };
                if deleted == 0 {
                    let cause = io::Error::last_os_error();
                    return Err(anyhow::anyhow!(
                        "failed to delete quarantined symlink for {}: {}; recovery entry remains at {}",
                        display_path.display(),
                        cause,
                        quarantine_path(display_path, &quarantine_name).display()
                    ));
                }
                return Ok(RemoveOutcome::Removed);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).map_err(anyhow::Error::from),
        }
    }
    anyhow::bail!(
        "could not reserve a unique quarantine name for {}",
        display_path.display()
    )
}

#[cfg(windows)]
fn remove_windows_empty_directory<F, G>(
    parent: &CapabilityDir,
    name: &OsStr,
    expected: EntryIdentity,
    display_path: &Path,
    before_move: F,
    after_move: G,
) -> anyhow::Result<RemoveDirectoryOutcome>
where
    F: FnOnce(),
    G: FnOnce(&Path),
{
    use cap_std::fs::{OpenOptions, OpenOptionsExt};
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ, FILE_SHARE_WRITE, SYNCHRONIZE,
    };

    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .access_mode(DELETE | FILE_READ_ATTRIBUTES | SYNCHRONIZE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
    let file = match parent.open_with(name, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RemoveDirectoryOutcome::Changed);
        }
        Err(error) => return Err(error).map_err(anyhow::Error::from),
    };
    let metadata = file.metadata()?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || !expected.matches(&metadata) {
        return Ok(RemoveDirectoryOutcome::Changed);
    }
    before_move();
    let directory = CapabilityDir::reopen_dir(&file)?;

    for _ in 0..16 {
        let quarantine_name = random_quarantine_name();
        match rename_open_handle(&file, parent, &quarantine_name) {
            Ok(()) => {
                after_move(display_path);
                let mut entries = match directory.entries() {
                    Ok(entries) => entries,
                    Err(error) => {
                        return Err(restore_windows_or_report(
                            &file,
                            parent,
                            name,
                            display_path,
                            &quarantine_name,
                            error,
                        ));
                    }
                };
                match entries.next() {
                    None => {}
                    Some(Ok(_)) => {
                        drop(entries);
                        rename_open_handle(&file, parent, name).map_err(|error| {
                            anyhow::anyhow!(
                                "could not restore non-empty container for {}: {}; recovery entry remains at {}",
                                display_path.display(),
                                error,
                                quarantine_path(display_path, &quarantine_name).display()
                            )
                        })?;
                        return Ok(RemoveDirectoryOutcome::NotEmpty);
                    }
                    Some(Err(error)) => {
                        drop(entries);
                        return Err(restore_windows_or_report(
                            &file,
                            parent,
                            name,
                            display_path,
                            &quarantine_name,
                            error,
                        ));
                    }
                }
                drop(entries);
                if let Err(error) = delete_open_handle(&file) {
                    return Err(restore_windows_or_report(
                        &file,
                        parent,
                        name,
                        display_path,
                        &quarantine_name,
                        error,
                    ));
                }
                return Ok(RemoveDirectoryOutcome::Removed);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).map_err(anyhow::Error::from),
        }
    }
    anyhow::bail!(
        "could not reserve a unique quarantine name for {}",
        display_path.display()
    )
}

#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
fn move_windows_entry_no_replace<F, G>(
    source: EntryLocation<'_>,
    destination: EntryLocation<'_>,
    expected: EntryIdentity,
    before_move: F,
    after_quarantine: G,
) -> anyhow::Result<MoveOutcome>
where
    F: FnOnce(),
    G: FnOnce(&Path),
{
    anyhow::ensure!(
        std::ptr::eq(source.parent, destination.parent),
        "backup source and destination must share a parent capability"
    );
    let parent = source.parent;
    let source_name = source.name;
    let destination_name = destination.name;
    let source_path = source.path;
    let destination_path = destination.path;
    use cap_std::fs::{OpenOptions, OpenOptionsExt};
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ, FILE_SHARE_WRITE, SYNCHRONIZE,
    };

    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .access_mode(DELETE | FILE_READ_ATTRIBUTES | SYNCHRONIZE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
    let file = match parent.open_with(source_name, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(MoveOutcome::Changed);
        }
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    if metadata.file_type().is_symlink()
        || (!metadata.is_file() && !metadata.is_dir())
        || !expected.matches(&metadata)
    {
        return Ok(MoveOutcome::Changed);
    }
    before_move();

    for _ in 0..16 {
        let quarantine_name = random_quarantine_name();
        match rename_open_handle(&file, parent, &quarantine_name) {
            Ok(()) => {
                after_quarantine(source_path);
                if let Err(error) = rename_open_handle(&file, parent, destination_name) {
                    return Err(restore_windows_or_report(
                        &file,
                        parent,
                        source_name,
                        source_path,
                        &quarantine_name,
                        error,
                    ))
                    .with_context(|| {
                        format!("failed to publish backup at {}", destination_path.display())
                    });
                }
                return Ok(MoveOutcome::Moved);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(MoveOutcome::Changed);
            }
            Err(error) => return Err(error.into()),
        }
    }

    anyhow::bail!(
        "could not reserve a unique quarantine name for {}",
        source_path.display()
    )
}

#[cfg(windows)]
fn delete_open_handle(file: &cap_std::fs::File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_DISPOSITION_FLAG_DELETE, FILE_DISPOSITION_INFO_EX, FileDispositionInfoEx,
        SetFileInformationByHandle,
    };

    let disposition = FILE_DISPOSITION_INFO_EX {
        Flags: FILE_DISPOSITION_FLAG_DELETE,
    };
    // SAFETY: `file` is a valid handle opened with DELETE access, and
    // `disposition` is the documented input structure for FileDispositionInfoEx.
    let deleted = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle() as HANDLE,
            FileDispositionInfoEx,
            (&disposition as *const _).cast(),
            std::mem::size_of_val(&disposition) as u32,
        )
    };
    if deleted == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn restore_windows_or_report(
    file: &cap_std::fs::File,
    parent: &CapabilityDir,
    original_name: &OsStr,
    display_path: &Path,
    quarantine_name: &OsStr,
    cause: io::Error,
) -> anyhow::Error {
    match rename_open_handle(file, parent, original_name) {
        Ok(()) => anyhow::Error::from(cause).context(format!(
            "operation failed for quarantined entry at {} (the entry was restored)",
            display_path.display()
        )),
        Err(restore_error) => anyhow::anyhow!(
            "{}; could not restore the entry: {}; recovery entry remains at {}",
            cause,
            restore_error,
            quarantine_path(display_path, quarantine_name).display()
        ),
    }
}

#[cfg(windows)]
pub(super) fn rename_open_handle(
    file: &cap_std::fs::File,
    parent: &CapabilityDir,
    name: &OsStr,
) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_RENAME_INFO, FileRenameInfoEx, SetFileInformationByHandle,
    };

    let wide_name = name.encode_wide().collect::<Vec<_>>();
    let file_name_length = u32::try_from(wide_name.len() * std::mem::size_of::<u16>())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "quarantine name too long"))?;
    let file_name_offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let buffer_length = file_name_offset + wide_name.len() * std::mem::size_of::<u16>();
    let word_count = buffer_length.div_ceil(std::mem::size_of::<u64>());
    let mut buffer = vec![0u64; word_count];
    let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    // SAFETY: `buffer` is suitably aligned and sized for FILE_RENAME_INFO's
    // fixed header plus the UTF-16 filename tail; the API consumes it before
    // this stack-owned buffer is dropped. Flags=0 means do not replace an
    // existing destination, and RootDirectory makes FileName relative to the
    // already-open parent directory handle.
    unsafe {
        (*info).Anonymous.Flags = 0;
        (*info).RootDirectory = parent.as_raw_handle() as HANDLE;
        (*info).FileNameLength = file_name_length;
        std::ptr::copy_nonoverlapping(
            wide_name.as_ptr(),
            (*info).FileName.as_mut_ptr(),
            wide_name.len(),
        );
        let renamed = SetFileInformationByHandle(
            file.as_raw_handle() as HANDLE,
            FileRenameInfoEx,
            info.cast(),
            buffer_length as u32,
        );
        if renamed == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(windows)]
fn random_quarantine_name() -> OsString {
    OsString::from(format!(
        ".agentsync-quarantine-{:032x}",
        rand::random::<u128>()
    ))
}

#[cfg(windows)]
fn quarantine_path(display_path: &Path, quarantine_name: &OsStr) -> std::path::PathBuf {
    display_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(quarantine_name)
}
