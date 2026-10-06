//! Revert implementation: undo `apply` by removing managed symlinks and
//! restoring `.bak` backups of pre-existing files.

use anyhow::{Context, Result};
use cap_fs_ext::DirExt;
use colored::Colorize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::SyncType;

use super::{Linker, SyncOptions, SyncResult, enumerate, quarantine, symlinks};

#[cfg(all(test, target_os = "linux"))]
fn rename_exclusive(from: &Path, to: &Path) -> std::io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        from,
        rustix::fs::CWD,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)
}

#[cfg(all(test, not(target_os = "linux")))]
fn rename_exclusive(from: &Path, to: &Path) -> std::io::Result<()> {
    renamore::rename_exclusive(from, to)
}

#[cfg(windows)]
const PRIVATE_STAGING_DACL_SDDL: &str = "D:P(A;;FA;;;OW)";

#[cfg(windows)]
#[derive(Debug, PartialEq, Eq)]
struct DaclSnapshot {
    present: bool,
    acl: Option<Vec<u8>>,
    protected: bool,
}

#[cfg(windows)]
struct LocalSecurityDescriptor(windows_sys::Win32::Security::PSECURITY_DESCRIPTOR);

#[cfg(windows)]
impl Drop for LocalSecurityDescriptor {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::LocalFree(self.0.cast());
        }
    }
}

#[cfg(all(windows, test))]
struct NamedDacl {
    _descriptor: LocalSecurityDescriptor,
    acl: *mut windows_sys::Win32::Security::ACL,
    snapshot: DaclSnapshot,
}

#[cfg(windows)]
struct HandleDacl {
    _descriptor: LocalSecurityDescriptor,
    acl: *mut windows_sys::Win32::Security::ACL,
    snapshot: DaclSnapshot,
}

#[cfg(test)]
fn validate_keep_backup_dacl(protected: bool, path: &Path) -> anyhow::Result<()> {
    if protected {
        return Ok(());
    }
    anyhow::bail!(
        "Backup has an inherited DACL that cannot be safely reproduced by --keep-backups staging; use a normal consuming restore (without --keep-backups) to atomically move the original .bak: {}",
        path.display()
    )
}

#[cfg(test)]
fn validate_copyable_dacl(present: bool, acl_is_null: bool, path: &Path) -> anyhow::Result<()> {
    if !present {
        anyhow::bail!(
            "Backup has no Windows DACL; refusing to publish a restore: {}",
            path.display()
        );
    }
    if acl_is_null {
        anyhow::bail!(
            "Backup has a NULL DACL that grants full access; refusing to publish a restore: {}",
            path.display()
        );
    }
    Ok(())
}

#[cfg(all(windows, test))]
fn windows_wide_path(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

#[cfg(all(windows, test))]
fn get_named_dacl(path: &Path) -> anyhow::Result<NamedDacl> {
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        ACL, GetSecurityDescriptorControl, GetSecurityDescriptorDacl, SE_DACL_PROTECTED,
    };

    let wide_path = windows_wide_path(path);
    let mut acl = std::ptr::null_mut();
    let mut descriptor = std::ptr::null_mut();
    let status = unsafe {
        GetNamedSecurityInfoW(
            wide_path.as_ptr(),
            SE_FILE_OBJECT,
            windows_sys::Win32::Security::DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut acl,
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(std::io::Error::from_raw_os_error(status as i32))
            .with_context(|| format!("Failed to query Windows DACL for {}", path.display()));
    }
    let descriptor = LocalSecurityDescriptor(descriptor);
    let mut present = 0;
    let mut descriptor_acl: *mut ACL = std::ptr::null_mut();
    if unsafe {
        GetSecurityDescriptorDacl(
            descriptor.0,
            &mut present,
            &mut descriptor_acl,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("Failed to inspect Windows DACL for {}", path.display()));
    }
    let mut control = 0;
    let mut revision = 0;
    if unsafe { GetSecurityDescriptorControl(descriptor.0, &mut control, &mut revision) } == 0 {
        return Err(std::io::Error::last_os_error()).with_context(|| {
            format!(
                "Failed to inspect Windows DACL protection for {}",
                path.display()
            )
        });
    }
    let acl_bytes = if descriptor_acl.is_null() {
        None
    } else {
        let size = unsafe { (*descriptor_acl).AclSize as usize };
        Some(unsafe { std::slice::from_raw_parts(descriptor_acl.cast::<u8>(), size) }.to_vec())
    };

    Ok(NamedDacl {
        _descriptor: descriptor,
        acl: descriptor_acl,
        snapshot: DaclSnapshot {
            present: present != 0,
            acl: acl_bytes,
            protected: control & SE_DACL_PROTECTED != 0,
        },
    })
}

#[cfg(all(windows, test))]
fn read_path_dacl(path: &Path) -> anyhow::Result<DaclSnapshot> {
    Ok(get_named_dacl(path)?.snapshot)
}

#[cfg(all(windows, test))]
fn set_path_dacl_from_sddl(path: &Path, sddl: &str) -> anyhow::Result<()> {
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1, SE_FILE_OBJECT,
        SetNamedSecurityInfoW,
    };
    use windows_sys::Win32::Security::{ACL, GetSecurityDescriptorDacl};

    let wide_sddl: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
    let mut descriptor = std::ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide_sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("Failed to parse Windows security descriptor for {sddl}"));
    }
    let descriptor = LocalSecurityDescriptor(descriptor);
    let mut present = 0;
    let mut acl: *mut ACL = std::ptr::null_mut();
    if unsafe {
        GetSecurityDescriptorDacl(descriptor.0, &mut present, &mut acl, std::ptr::null_mut())
    } == 0
    {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("Failed to read Windows security descriptor for {sddl}"));
    }
    if present == 0 {
        anyhow::bail!("Windows security descriptor contains no DACL: {sddl}");
    }
    let acl_bytes = if acl.is_null() {
        None
    } else {
        let size = unsafe { (*acl).AclSize as usize };
        Some(unsafe { std::slice::from_raw_parts(acl.cast::<u8>(), size) }.to_vec())
    };

    let wide_path = windows_wide_path(path);
    let status = unsafe {
        SetNamedSecurityInfoW(
            wide_path.as_ptr(),
            SE_FILE_OBJECT,
            windows_sys::Win32::Security::DACL_SECURITY_INFORMATION
                | windows_sys::Win32::Security::PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            acl,
            std::ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(std::io::Error::from_raw_os_error(status as i32))
            .with_context(|| format!("Failed to set Windows DACL for {}", path.display()));
    }
    let actual = read_path_dacl(path)?;
    if actual
        != (DaclSnapshot {
            present: true,
            acl: acl_bytes,
            protected: true,
        })
    {
        anyhow::bail!("Windows DACL was not applied exactly to {}", path.display());
    }
    Ok(())
}

#[cfg(all(windows, test))]
fn copy_path_dacl(source: &Path, destination: &Path) -> anyhow::Result<()> {
    use windows_sys::Win32::Security::Authorization::{SE_FILE_OBJECT, SetNamedSecurityInfoW};

    let source_dacl = get_named_dacl(source)?;
    validate_copyable_dacl(
        source_dacl.snapshot.present,
        source_dacl.snapshot.acl.is_none(),
        source,
    )?;
    validate_keep_backup_dacl(source_dacl.snapshot.protected, source)?;

    let wide_destination = windows_wide_path(destination);
    let status = unsafe {
        SetNamedSecurityInfoW(
            wide_destination.as_ptr(),
            SE_FILE_OBJECT,
            windows_sys::Win32::Security::DACL_SECURITY_INFORMATION
                | windows_sys::Win32::Security::PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            source_dacl.acl,
            std::ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(std::io::Error::from_raw_os_error(status as i32)).with_context(|| {
            format!(
                "Failed to preserve backup Windows DACL on staged restore {}",
                destination.display()
            )
        });
    }

    let restored_dacl = read_path_dacl(destination)?;
    let expected_dacl = DaclSnapshot {
        protected: true,
        ..source_dacl.snapshot
    };
    if restored_dacl != expected_dacl {
        anyhow::bail!(
            "Restored Windows DACL does not match backup; refusing to publish: {}",
            destination.display()
        );
    }
    Ok(())
}

#[cfg(all(windows, test))]
fn protect_staging_directory(path: &Path) -> anyhow::Result<()> {
    set_path_dacl_from_sddl(path, PRIVATE_STAGING_DACL_SDDL).with_context(|| {
        format!(
            "Failed to restrict Windows restore staging directory before copying bytes: {}",
            path.display()
        )
    })
}

#[cfg(all(windows, test))]
fn copy_file_with_windows_security(source: &Path, destination: &Path) -> anyhow::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{COPY_FILE_FAIL_IF_EXISTS, CopyFileExW};

    let source_dacl = read_path_dacl(source)?;
    validate_keep_backup_dacl(source_dacl.protected, source)?;

    let wide_source = windows_wide_path(source);
    let wide_destination = windows_wide_path(destination);
    if unsafe {
        CopyFileExW(
            wide_source.as_ptr(),
            wide_destination.as_ptr(),
            None,
            std::ptr::null(),
            std::ptr::null_mut(),
            COPY_FILE_FAIL_IF_EXISTS,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).with_context(|| {
            format!(
                "Failed to copy backup file with Windows security properties: {}",
                source.display()
            )
        });
    }
    copy_path_dacl(source, destination)
}

#[cfg(all(test, unix))]
type StagedDirectoryPermissions = Vec<(PathBuf, fs::Permissions, fs::Permissions)>;
#[cfg(all(test, windows))]
type StagedDirectoryPermissions = Vec<(PathBuf, PathBuf)>;
#[cfg(all(test, not(any(unix, windows))))]
type StagedDirectoryPermissions = ();

impl Linker {
    fn revert_symlink_metadata(&self, path: &Path) -> std::io::Result<fs::Metadata> {
        #[cfg(test)]
        if self.revert_metadata_error_path.borrow().as_deref() == Some(path) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected revert metadata inspection failure",
            ));
        }

        fs::symlink_metadata(path)
    }

    fn revert_parent_symlink_metadata(
        &self,
        parent: &cap_std::fs::Dir,
        name: &std::ffi::OsStr,
        _display_path: &Path,
    ) -> std::io::Result<cap_std::fs::Metadata> {
        #[cfg(test)]
        if self.revert_metadata_error_path.borrow().as_deref() == Some(_display_path) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected revert metadata inspection failure",
            ));
        }

        parent.symlink_metadata(name)
    }

    /// Revert managed destinations to their pre-apply state.
    pub fn revert(&self, options: &SyncOptions) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        println!("{}", "Reverting managed symlinks...".cyan());

        for (agent_name, agent_config) in &self.config.agents {
            let agent_span = tracing::info_span!(
                "agentsync",
                operation = "revert",
                agent_id = %agent_name,
                outcome = tracing::field::Empty
            );
            let _agent_enter = agent_span.enter();
            if !super::revert_agent_selected(&self.config, agent_name, options) {
                tracing::debug!(reason = "filtered", "Skipping agent");
                agent_span.record("outcome", "skipped");
                continue;
            }
            let errors_before = result.errors;
            for target_config in agent_config.targets.values() {
                match target_config.sync_type {
                    SyncType::Symlink => {
                        self.revert_symlink_target(target_config, options, &mut result)?;
                    }
                    SyncType::SymlinkContents => {
                        self.revert_symlink_contents_target(
                            agent_name,
                            target_config,
                            options,
                            &mut result,
                        )?;
                    }
                    SyncType::NestedGlob => {
                        self.revert_nested_glob_target(target_config, options, &mut result)?;
                    }
                    SyncType::ModuleMap => {
                        self.revert_module_map_target(
                            agent_name,
                            target_config,
                            options,
                            &mut result,
                        )?;
                    }
                }
            }
            agent_span.record(
                "outcome",
                if result.errors > errors_before {
                    "error"
                } else {
                    "ok"
                },
            );
        }

        Ok(result)
    }

    /// Revert a single symlink target: remove the managed symlink, then
    /// restore the `.bak` backup if one exists.
    fn revert_symlink_target(
        &self,
        target_config: &crate::config::TargetConfig,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        let dest = match self.resolve_destination(&target_config.destination) {
            Ok(d) => d,
            Err(s) => {
                result.errors += 1;
                tracing::warn!(destination = %s.dest, error = %s.error, "Skipping revert target with unsafe destination");
                return Ok(());
            }
        };
        let expected = self.symlink_expected_target(&dest, target_config);
        self.revert_destination(&dest, expected, options, result)
    }

    /// Expected link content `apply` would have written for a `symlink`
    /// target: the relative path from `dest` to the resolved source (same
    /// `relative_path` helper `create_symlink` uses, including the compressed
    /// `AGENTS.md` mapping). `None` when the source vanished, in which case
    /// the caller skips with a warning instead of touching the link.
    fn symlink_expected_target(
        &self,
        dest: &Path,
        target_config: &crate::config::TargetConfig,
    ) -> Option<PathBuf> {
        let source_path = self.source_dir.join(&target_config.source);
        let resolved = self.expected_applied_source_path(&source_path, target_config)?;
        let allow_missing = self.should_compress_agents_md(&source_path, target_config);
        self.relative_path(dest, &resolved, allow_missing).ok()
    }

    /// Resolve the source path `apply` would have linked, including generated
    /// compressed output that may have been removed after apply. Unlike
    /// `expected_source_path`, this never falls back to the original
    /// `AGENTS.md` when compression applies; the original source must remain
    /// present so the expected applied path is still attributable.
    fn expected_applied_source_path(
        &self,
        source: &Path,
        target: &crate::config::TargetConfig,
    ) -> Option<PathBuf> {
        if self.should_compress_agents_md(source, target) {
            return source
                .exists()
                .then(|| Self::compressed_agents_md_path(source));
        }
        self.expected_source_path(source, target)
    }

    /// Expected link content for one `symlink-contents` child: the relative
    /// path from the child to the source entry `apply` linked from (same
    /// destination-name transform as apply, including the zcode command
    /// mapping). `None` when no source entry maps to this child or its winner
    /// is no longer available.
    fn symlink_contents_expected_target(
        &self,
        agent_name: &str,
        target_config: &crate::config::TargetConfig,
        dest_child: &Path,
    ) -> Option<PathBuf> {
        let child_name = dest_child.file_name()?.to_str()?;
        let source_dir = self.source_dir.join(&target_config.source);
        #[cfg(test)]
        let source_entries_override = self
            .symlink_contents_source_entries_override
            .borrow()
            .clone();
        #[cfg(test)]
        let source_entries: Vec<PathBuf> = if let Some(mut entries) = source_entries_override {
            // Keep the injected enumeration subject to the same ordering rule
            // as apply, independent of how the test supplied its entries.
            entries.sort_by_key(|entry| {
                entry
                    .file_name()
                    .map(std::ffi::OsStr::to_os_string)
                    .unwrap_or_default()
            });
            entries
        } else {
            symlinks::sorted_dir_entries(&source_dir)
                .ok()?
                .into_iter()
                .map(|entry| entry.path())
                .collect()
        };
        #[cfg(not(test))]
        let source_entries: Vec<PathBuf> = symlinks::sorted_dir_entries(&source_dir)
            .ok()?
            .into_iter()
            .map(|entry| entry.path())
            .collect();

        let mut winning_source = None;
        for source_path in source_entries {
            let item_name = source_path.file_name()?;
            let item_str = item_name.to_string_lossy();
            if let Some(pattern) = target_config.pattern.as_deref()
                && !super::matches_pattern(&item_str, pattern)
            {
                continue;
            }
            let destination_name = if crate::agent_ids::canonical_any_agent_id(agent_name)
                == Some("zcode")
                && target_config.destination.ends_with(".zcode/commands")
            {
                crate::zcode_command_destination(&item_str)
            } else {
                item_str.into_owned()
            };
            if destination_name == child_name {
                winning_source = Some(source_path);
            }
        }
        let winning_source = winning_source?;
        let resolved = self.expected_applied_source_path(&winning_source, target_config)?;
        let allow_missing = self.should_compress_agents_md(&winning_source, target_config);
        self.relative_path(dest_child, &resolved, allow_missing)
            .ok()
    }

    /// Revert a symlink-contents target: revert every managed child symlink
    /// and restore every orphaned `.bak` backup. Leave the directory in place
    /// because revert cannot prove whether it existed before apply.
    fn revert_symlink_contents_target(
        &self,
        agent_name: &str,
        target_config: &crate::config::TargetConfig,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        use std::ffi::OsStr;
        let dest = match self.resolve_destination(&target_config.destination) {
            Ok(d) => d,
            Err(s) => {
                result.errors += 1;
                tracing::warn!(destination = %s.dest, error = %s.error, "Skipping revert target with unsafe destination");
                return Ok(());
            }
        };
        // Never traverse a destination that is itself a symlink: `read_dir`
        // would follow the link out of the project root. Revert the link
        // itself instead. Apply never creates a bare symlink here (it makes a
        // real directory, or skips a pre-existing link), so there is no
        // applied source to verify against: an uncomputable expectation makes
        // `revert_destination` leave it alone with a warning.
        let metadata = match self.revert_symlink_metadata(&dest) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                result.errors += 1;
                tracing::warn!(error = %error, path = %dest.display(), "Skipping revert target: failed to inspect destination directory");
                return Ok(());
            }
        };
        if metadata.file_type().is_symlink() {
            return self.revert_destination(&dest, None, options, result);
        }
        if !metadata.is_dir() {
            return Ok(());
        }
        let entries = match self.read_contents_entries(&dest) {
            Ok(entries) => entries,
            Err(e) => {
                result.errors += 1;
                tracing::warn!(error = %e, path = %dest.display(), "Skipping revert target: failed to read destination directory");
                return Ok(());
            }
        };
        // Dry-run mutates nothing, so the symlink branch and the orphan
        // `.bak` branch below can visit the same twin twice in one pass and
        // count it twice. Track visited destinations and handle each once.
        // (The same guard also covers real runs, where the upfront listing
        // still contains a `.bak` consumed moments earlier by a restore.)
        let mut visited: HashSet<PathBuf> = HashSet::new();
        for entry_path in entries {
            let metadata = match self.revert_symlink_metadata(&entry_path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    result.errors += 1;
                    tracing::warn!(error = %error, path = %entry_path.display(), "Skipping revert entry: failed to inspect destination");
                    continue;
                }
            };
            if enumerate::contents_child_is_managed(
                agent_name,
                target_config,
                &entry_path,
                metadata.file_type().is_symlink(),
            ) {
                if !visited.insert(entry_path.clone()) {
                    continue;
                }
                let expected =
                    self.symlink_contents_expected_target(agent_name, target_config, &entry_path);
                self.revert_destination(&entry_path, expected, options, result)?;
            } else if entry_path
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| name.ends_with(".bak"))
            {
                let twin_name = entry_path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .and_then(|name| name.strip_suffix(".bak"))
                    .filter(|name| !name.is_empty());
                let Some(twin_name) = twin_name else {
                    continue;
                };
                let twin = entry_path.with_file_name(twin_name);
                // A managed symlink may already have restored this backup
                // through its twin branch. Do not count that same entry as an
                // excluded orphan based on the now-visited destination name.
                if visited.contains(&twin) {
                    continue;
                }
                let matches_pattern = match self.contents_child_matches_pattern(
                    agent_name,
                    target_config,
                    &twin,
                ) {
                    Ok(matches) => matches,
                    Err(error) => {
                        println!(
                            "  {} Skipping orphan backup with unknown source-pattern eligibility: {} ({})",
                            "!".yellow(),
                            entry_path.display(),
                            error
                        );
                        tracing::warn!(
                            error = %error,
                            path = %entry_path.display(),
                            twin = %twin.display(),
                            "Skipping orphan backup with unknown source-pattern eligibility"
                        );
                        result.skipped += 1;
                        continue;
                    }
                };
                if !matches_pattern
                    || enumerate::zcode_contents_child_filtered(agent_name, target_config, &twin)
                {
                    println!(
                        "  {} Skipping orphan backup outside target scope: {}",
                        "!".yellow(),
                        entry_path.display()
                    );
                    tracing::warn!(path = %entry_path.display(), twin = %twin.display(), "Skipping orphan backup: destination is excluded by target filters");
                    result.skipped += 1;
                    continue;
                }
                if !visited.insert(twin.clone()) {
                    continue;
                }
                // Never overwrite a real user file that appeared after the
                // backup was taken; restore into an absent twin or over a
                // managed symlink twin.
                match self.revert_symlink_metadata(&twin) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        let expected =
                            self.symlink_contents_expected_target(agent_name, target_config, &twin);
                        self.revert_destination(&twin, expected, options, result)?;
                    }
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        let expected =
                            self.symlink_contents_expected_target(agent_name, target_config, &twin);
                        self.revert_destination(&twin, expected, options, result)?;
                    }
                    Ok(_) => {
                        // Orphan backup whose twin is a real file: left in place,
                        // loudly (never silently).
                        println!(
                            "  {} Skipping restore over user file: {}",
                            "!".yellow(),
                            twin.display()
                        );
                        tracing::warn!(path = %twin.display(), "Skipping orphan backup restore: destination is a real file");
                        result.skipped += 1;
                    }
                    Err(error) => {
                        result.errors += 1;
                        tracing::warn!(error = %error, path = %twin.display(), "Skipping orphan backup restore: failed to inspect destination");
                    }
                }
            }
        }
        Ok(())
    }

    /// Revert nested-glob targets: re-discover matched files and revert each
    /// generated symlink, restoring backups where present.
    fn revert_nested_glob_target(
        &self,
        target_config: &crate::config::TargetConfig,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        let enumeration = match self.enumerate_nested_glob(target_config, options) {
            Ok(enumeration) => enumeration,
            Err(error) => {
                let search_root = self.project_root.join(&target_config.source);
                result.errors += 1;
                println!(
                    "  {} Nested-glob revert discovery failed for {}: {}",
                    "!".yellow(),
                    search_root.display(),
                    error
                );
                tracing::warn!(
                    search_root = %search_root.display(),
                    error = %error,
                    "Nested-glob revert discovery failed; target is incomplete"
                );
                return Ok(());
            }
        };
        if let Err(s) = enumeration.template {
            result.errors += 1;
            tracing::warn!(destination = %s.dest, error = %s.error, "Skipping revert target with unsafe destination");
            return Ok(());
        }
        let incomplete_discovery = match enumeration.discovery {
            enumerate::NestedGlobDiscoveryStatus::Incomplete {
                search_root,
                reason,
            } => Some((search_root, reason)),
            enumerate::NestedGlobDiscoveryStatus::IncompleteWalk { search_root } => Some((
                search_root,
                "WalkDir traversal encountered one or more entry errors".to_string(),
            )),
            enumerate::NestedGlobDiscoveryStatus::NotAttempted
            | enumerate::NestedGlobDiscoveryStatus::Complete => None,
        };
        if let Some((search_root, reason)) = incomplete_discovery {
            result.skipped += 1;
            println!(
                "  {} Revert skipped: nested-glob source discovery is incomplete for {}: {}",
                "!".yellow(),
                search_root.display(),
                reason
            );
            tracing::warn!(
                search_root = %search_root.display(),
                reason = %reason,
                "Skipping nested-glob revert because source discovery is incomplete"
            );
            return Ok(());
        }

        for item in enumeration.entries {
            let dest = match item.dest {
                Ok(dest) => dest,
                Err(s) => {
                    result.errors += 1;
                    tracing::warn!(destination = %s.dest, error = %s.error, "Skipping revert destination with unsafe path");
                    continue;
                }
            };
            // The match's source full path is exactly what apply linked from.
            let expected = self.relative_path(&dest, &item.source, false).ok();
            self.revert_destination(&dest, expected, options, result)?;
        }
        Ok(())
    }

    /// Revert module-map targets: revert each mapped symlink, restoring
    /// backups where present.
    fn revert_module_map_target(
        &self,
        agent_name: &str,
        target_config: &crate::config::TargetConfig,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        for item in self.enumerate_module_map(agent_name, target_config) {
            let dest = match item.dest {
                Ok(d) => d,
                Err(e) => {
                    result.errors += 1;
                    tracing::warn!(mapping = %item.source, destination = %item.dest_str, error = %e, "Skipping revert mapping with unsafe destination");
                    if options.verbose {
                        println!("  {} Skipping mapping {}: {}", "!".yellow(), item.source, e);
                    }
                    continue;
                }
            };

            // Mirror the apply-side source resolution (`process_module_map`
            // links `source_dir/<mapping.source>` with no compression): a
            // vanished source leaves the expectation uncomputable, so the
            // destination is skipped with a warning instead of touched.
            let expected_source = self.source_dir.join(&item.source);
            let expected = if expected_source.exists() {
                self.relative_path(&dest, &expected_source, false).ok()
            } else {
                None
            };
            self.revert_destination(&dest, expected, options, result)?;
        }
        Ok(())
    }

    /// Shared per-path revert used by all four sync types.
    ///
    /// `expected` is the exact link content `apply` would have written
    /// (computed by the caller with the same `relative_path` helper
    /// `create_symlink` uses). A symlink whose target differs was repointed by
    /// the user after apply: warn, count a skip, and touch neither the link
    /// nor its `.bak`. `None` means the applied source is uncomputable (the
    /// source vanished): warn and skip the same way.
    pub(super) fn revert_destination(
        &self,
        dest: &Path,
        expected: Option<PathBuf>,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        let span = tracing::info_span!(
            "agentsync",
            operation = "revert",
            path = %dest.display(),
            outcome = tracing::field::Empty
        );
        let _enter = span.enter();
        let Some(name) = dest.file_name() else {
            result.errors += 1;
            span.record("outcome", "error");
            tracing::warn!(path = %dest.display(), "Skipping revert: destination has no final component");
            return Ok(());
        };
        let Some(parent_path) = dest.parent() else {
            result.errors += 1;
            span.record("outcome", "error");
            tracing::warn!(path = %dest.display(), "Skipping revert: destination has no parent");
            return Ok(());
        };
        let parent = match self.open_project_relative_directory(parent_path) {
            Ok(parent) => parent,
            Err(error) => {
                result.errors += 1;
                span.record("outcome", "error");
                tracing::warn!(error = %error, path = %dest.display(), "Skipping revert: failed to open destination parent capability");
                return Ok(());
            }
        };
        let dest_metadata = match self.revert_parent_symlink_metadata(&parent, name, dest) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                result.errors += 1;
                span.record("outcome", "error");
                tracing::warn!(error = %error, path = %dest.display(), "Skipping revert: failed to inspect destination");
                return Ok(());
            }
        };
        if let Some(metadata) = dest_metadata
            .as_ref()
            .filter(|metadata| metadata.file_type().is_symlink())
        {
            match &expected {
                Some(want) => {
                    let actual = match parent.read_link(name) {
                        Ok(target) => target,
                        Err(e) => {
                            result.errors += 1;
                            span.record("outcome", "error");
                            tracing::warn!(error = %e, path = %dest.display(), "Skipping revert: failed to read symlink target");
                            return Ok(());
                        }
                    };
                    if actual != *want {
                        println!(
                            "  {} Skipping unmanaged symlink (target differs from applied source): {}",
                            "!".yellow(),
                            dest.display()
                        );
                        tracing::warn!(path = %dest.display(), actual = %actual.display(), expected = %want.display(), "Skipping revert: symlink target differs from applied source");
                        result.skipped += 1;
                        span.record("outcome", "skipped");
                        return Ok(());
                    }
                    let identity = quarantine::EntryIdentity::capture(metadata);
                    if options.dry_run {
                        println!("  {} Would remove: {}", "→".cyan(), dest.display());
                        result.removed += 1;
                    } else {
                        match quarantine::remove_symlink_if_unchanged(
                            &parent,
                            name,
                            identity,
                            dest,
                            || {
                                #[cfg(test)]
                                if let Some(hook) =
                                    self.quarantine_before_move_hook.borrow_mut().take()
                                {
                                    hook(dest);
                                }
                            },
                            |_path| {},
                        ) {
                            Ok(quarantine::RemoveOutcome::Removed) => {
                                self.invalidate_path(dest);
                                println!("  {} Removed: {}", "✔".green(), dest.display());
                                result.removed += 1;
                            }
                            Ok(quarantine::RemoveOutcome::Changed) => {
                                result.skipped += 1;
                                span.record("outcome", "skipped");
                                tracing::warn!(path = %dest.display(), "Destination changed before revert quarantine; preserving it and its backup");
                                return Ok(());
                            }
                            Err(error) => {
                                result.errors += 1;
                                span.record("outcome", "error");
                                tracing::error!(error = %error, path = %dest.display(), "Failed to quarantine managed symlink during revert");
                                return Ok(());
                            }
                        }
                    }
                    if !options.dry_run {
                        match self.revert_parent_symlink_metadata(&parent, name, dest) {
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                            Ok(_) => {
                                result.errors += 1;
                                println!(
                                    "  {} Refusing to restore while destination remains: {}",
                                    "!".yellow(),
                                    dest.display()
                                );
                                tracing::warn!(path = %dest.display(), "Refusing to restore backup because managed symlink destination remains after removal");
                                span.record("outcome", "error");
                                return Ok(());
                            }
                            Err(e) => {
                                result.errors += 1;
                                println!(
                                    "  {} Refusing to restore because destination could not be inspected: {}",
                                    "!".yellow(),
                                    dest.display()
                                );
                                tracing::warn!(error = %e, path = %dest.display(), "Refusing to restore backup because destination inspection failed after symlink removal");
                                span.record("outcome", "error");
                                return Ok(());
                            }
                        }
                    }
                }
                None => {
                    println!(
                        "  {} Skipping revert: cannot determine applied source for: {}",
                        "!".yellow(),
                        dest.display()
                    );
                    tracing::warn!(path = %dest.display(), "Skipping revert: expected symlink target is uncomputable (source vanished or destination not managed by apply)");
                    result.skipped += 1;
                    span.record("outcome", "skipped");
                    return Ok(());
                }
            }
        }
        let backup = symlinks::backup_path_for_destination(dest);
        let Some(backup_name) = backup.file_name() else {
            result.errors += 1;
            span.record("outcome", "error");
            tracing::warn!(path = %backup.display(), "Skipping revert: backup has no final component");
            return Ok(());
        };
        match self.revert_parent_symlink_metadata(&parent, backup_name, &backup) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                span.record("outcome", "ok");
                return Ok(());
            }
            Err(error) => {
                result.errors += 1;
                span.record("outcome", "error");
                tracing::warn!(error = %error, path = %backup.display(), "Skipping revert: failed to inspect backup");
                return Ok(());
            }
        }
        {
            // Never restore over a real user file that appeared after apply,
            // including during dry-run. A verified managed symlink remains in
            // place only in dry-run, so it is the sole existing entry allowed.
            match self.revert_parent_symlink_metadata(&parent, name, dest) {
                Ok(metadata) if !metadata.file_type().is_symlink() || !options.dry_run => {
                    result.errors += 1;
                    span.record("outcome", "error");
                    println!(
                        "  {} Refusing to restore over existing destination: {}",
                        "!".yellow(),
                        dest.display()
                    );
                    tracing::warn!(path = %dest.display(), "Refusing to restore backup over existing destination");
                    return Ok(());
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    result.errors += 1;
                    span.record("outcome", "error");
                    tracing::warn!(error = %e, path = %dest.display(), "Refusing to restore backup because destination inspection failed");
                    return Ok(());
                }
            }
            self.restore_backup_with_parent(
                &parent,
                name,
                backup_name,
                dest,
                &backup,
                options,
                result,
            )?;
        }
        span.record("outcome", "ok");
        Ok(())
    }

    /// Move (or, with keep_backups, copy) a `.bak` backup back to `dest`.
    #[allow(clippy::too_many_arguments)]
    fn restore_backup_with_parent(
        &self,
        parent: &cap_std::fs::Dir,
        dest_name: &std::ffi::OsStr,
        backup_name: &std::ffi::OsStr,
        dest: &Path,
        backup: &Path,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        let backup_metadata = match parent.symlink_metadata(backup_name) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                result.errors += 1;
                tracing::error!(error = %error, path = %backup.display(), "Failed to inspect backup relative to opened parent");
                return Ok(());
            }
        };
        if backup_metadata.file_type().is_symlink()
            || (!backup_metadata.is_file() && !backup_metadata.is_dir())
        {
            result.errors += 1;
            println!(
                "  {} Skipping special backup entry: {}",
                "!".yellow(),
                backup.display()
            );
            tracing::warn!(path = %backup.display(), "Skipping backup restore: unsupported entry type");
            return Ok(());
        }

        match parent.symlink_metadata(dest_name) {
            Ok(metadata) if options.dry_run && metadata.file_type().is_symlink() => {}
            Ok(_) => {
                result.errors += 1;
                println!(
                    "  {} Refusing to restore over existing destination: {}",
                    "!".yellow(),
                    dest.display()
                );
                tracing::warn!(path = %dest.display(), "Refusing to restore backup over existing destination");
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                result.errors += 1;
                tracing::warn!(error = %error, path = %dest.display(), "Refusing to restore because destination inspection failed");
                return Ok(());
            }
        }

        if options.dry_run {
            println!("  {} Would restore: {}", "→".cyan(), dest.display());
            result.restored += 1;
            return Ok(());
        }

        if options.keep_backups {
            let skipped = match copy_backup_contents_capability(
                parent,
                backup_name,
                dest_name,
                backup,
                dest,
                &backup_metadata,
            ) {
                Ok(skipped) => skipped,
                Err(error) => {
                    result.errors += 1;
                    tracing::error!(error = %error, path = %dest.display(), "Failed to copy backup through parent capability");
                    return Ok(());
                }
            };
            if skipped > 0 {
                result.errors += 1;
                tracing::warn!(path = %backup.display(), skipped, "Backup copy skipped special entries");
                return Ok(());
            }
        } else {
            let identity = quarantine::EntryIdentity::capture(&backup_metadata);
            match quarantine::move_entry_no_replace(
                quarantine::EntryLocation {
                    parent,
                    name: backup_name,
                    path: backup,
                },
                quarantine::EntryLocation {
                    parent,
                    name: dest_name,
                    path: dest,
                },
                identity,
                || {},
                |_path| {},
            ) {
                Ok(quarantine::MoveOutcome::Moved) => {}
                Ok(quarantine::MoveOutcome::Changed) => {
                    result.skipped += 1;
                    tracing::warn!(path = %backup.display(), "Backup changed before capability-relative restore; preserving it");
                    return Ok(());
                }
                Err(error) => {
                    result.errors += 1;
                    tracing::error!(error = %error, path = %dest.display(), "Failed to restore backup relative to opened parent");
                    return Ok(());
                }
            }
        }

        self.invalidate_path(dest);
        self.invalidate_path(backup);
        self.invalidate_glob_cache();
        println!("  {} Restored: {}", "✔".green(), dest.display());
        result.restored += 1;
        Ok(())
    }

    /// Move (or, with keep_backups, copy) a `.bak` backup back to `dest`.
    #[cfg(test)]
    fn restore_backup(
        &self,
        dest: &Path,
        backup: &Path,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        self.restore_backup_with_hook(dest, backup, options, result, || Ok(()))
    }

    #[cfg(test)]
    fn restore_backup_with_hook<F>(
        &self,
        dest: &Path,
        backup: &Path,
        options: &SyncOptions,
        result: &mut SyncResult,
        before_restore: F,
    ) -> Result<()>
    where
        F: FnOnce() -> Result<()>,
    {
        let span = tracing::info_span!(
            "agentsync",
            operation = "restore",
            path = %dest.display(),
            outcome = tracing::field::Empty
        );
        let _enter = span.enter();
        if options.dry_run {
            println!("  {} Would restore: {}", "→".cyan(), dest.display());
            span.record("outcome", "would_restore");
            result.restored += 1;
            return Ok(());
        }
        match self.revert_symlink_metadata(dest) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                result.errors += 1;
                span.record("outcome", "error");
                println!(
                    "  {} Refusing to restore through symlink destination: {}",
                    "!".yellow(),
                    dest.display()
                );
                tracing::warn!(path = %dest.display(), "Refusing to restore backup through existing symlink destination");
                return Ok(());
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                result.errors += 1;
                span.record("outcome", "error");
                tracing::error!(error = %e, path = %dest.display(), "Failed to inspect revert destination");
                return Ok(());
            }
        }
        if let Err(e) = self.revalidate_unlink_path(dest) {
            result.errors += 1;
            span.record("outcome", "error");
            tracing::error!(error = %e, path = %dest.display(), "Failed to revalidate revert destination");
            return Ok(());
        }
        if let Err(e) = self.revalidate_path(backup) {
            result.errors += 1;
            span.record("outcome", "error");
            tracing::error!(error = %e, path = %backup.display(), "Failed to revalidate revert backup");
            return Ok(());
        }
        if let Err(e) = before_restore() {
            result.errors += 1;
            span.record("outcome", "error");
            tracing::error!(error = %e, path = %dest.display(), "Failed before restoring backup");
            return Ok(());
        }
        let op: Result<usize> = if options.keep_backups {
            copy_backup_contents(backup, dest)
        } else {
            move_backup_contents(backup, dest)
        };
        let skipped_copies = match op {
            Ok(skipped) => skipped,
            Err(e) => {
                result.errors += 1;
                span.record("outcome", "error");
                println!(
                    "  {} Failed to restore backup: {} ({e})",
                    "!".yellow(),
                    dest.display()
                );
                tracing::error!(error = %e, path = %dest.display(), "Failed to restore backup");
                return Ok(());
            }
        };
        // A partial copy (special entries skipped) must not be reported as a
        // restore: warn loudly and count the error instead.
        if skipped_copies > 0 {
            result.errors += 1;
            span.record("outcome", "error");
            println!(
                "  {} Restored with {} skipped special entries (see warnings): {}",
                "!".yellow(),
                skipped_copies,
                dest.display()
            );
            tracing::warn!(path = %dest.display(), skipped = skipped_copies, "Backup restore skipped special entries; not counted as restored");
            return Ok(());
        }
        self.invalidate_path(dest);
        self.invalidate_path(backup);
        self.invalidate_glob_cache();
        println!("  {} Restored: {}", "✔".green(), dest.display());
        span.record("outcome", "restored");
        result.restored += 1;
        Ok(())
    }
}

fn copy_backup_contents_capability(
    parent: &cap_std::fs::Dir,
    backup_name: &std::ffi::OsStr,
    destination_name: &std::ffi::OsStr,
    backup_path: &Path,
    destination_path: &Path,
    expected: &cap_std::fs::Metadata,
) -> anyhow::Result<usize> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if expected.is_file() {
            copy_backup_file_capability(
                parent,
                backup_name,
                destination_name,
                backup_path,
                destination_path,
                quarantine::EntryIdentity::capture(expected),
            )?;
            return Ok(0);
        }
        if expected.is_dir() && !expected.file_type().is_symlink() {
            return copy_backup_directory_capability(
                parent,
                backup_name,
                destination_name,
                backup_path,
                destination_path,
                quarantine::EntryIdentity::capture(expected),
            );
        }
        Ok(1)
    }

    #[cfg(windows)]
    {
        if expected.is_file() {
            copy_backup_file_capability_windows(
                parent,
                backup_name,
                destination_name,
                backup_path,
                destination_path,
                quarantine::EntryIdentity::capture(expected),
            )?;
            Ok(0)
        } else {
            anyhow::bail!(
                "Capability-relative keep-backups directory copy is not yet supported on Windows; original backup is preserved at {}",
                backup_path.display()
            )
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = (parent, backup_name, destination_name, expected);
        anyhow::bail!(
            "Capability-relative --keep-backups copy is unsupported on this platform; backup remains at {}",
            backup_path.display()
        )
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn copy_backup_file_capability(
    parent: &cap_std::fs::Dir,
    backup_name: &std::ffi::OsStr,
    destination_name: &std::ffi::OsStr,
    backup_path: &Path,
    destination_path: &Path,
    expected: quarantine::EntryIdentity,
) -> anyhow::Result<()> {
    let mut source = open_backup_file_capability(parent, backup_name, backup_path)?;
    let source_metadata = source.metadata()?;
    anyhow::ensure!(
        source_metadata.is_file() && expected.matches(&source_metadata),
        "Backup changed before capability-relative copy: {}",
        backup_path.display()
    );
    let (staging_name, staging, staging_identity) =
        create_restore_staging_directory(parent, destination_path)?;
    let mut destination =
        create_restore_file_capability(&staging, std::ffi::OsStr::new("file"), destination_path)?;
    std::io::copy(&mut source, &mut destination).with_context(|| {
        format!(
            "Failed to copy backup bytes for {}",
            destination_path.display()
        )
    })?;
    destination.sync_all()?;
    destination
        .set_permissions(source_metadata.permissions())
        .with_context(|| {
            format!(
                "Failed to preserve backup permissions for {}",
                destination_path.display()
            )
        })?;
    let staged_identity = quarantine::EntryIdentity::capture(&destination.metadata()?);
    drop(destination);

    if let Err(error) = quarantine::rename_between_no_replace(
        &staging,
        std::ffi::OsStr::new("file"),
        parent,
        destination_name,
    ) {
        return Err(error).with_context(|| {
            format!(
                "Failed to publish copied backup without replacement; staging recovery directory remains at {}",
                restore_staging_path(destination_path, &staging_name).display()
            )
        });
    }
    let published = parent.symlink_metadata(destination_name).with_context(|| {
        format!(
            "Failed to verify restored file: {}",
            destination_path.display()
        )
    })?;
    anyhow::ensure!(
        published.is_file() && staged_identity.matches(&published),
        "Restored file changed identity before verification: {}",
        destination_path.display()
    );
    cleanup_empty_restore_staging_directory(
        parent,
        &staging_name,
        staging_identity,
        destination_path,
    )?;
    Ok(())
}

#[cfg(windows)]
fn get_handle_dacl(
    handle: windows_sys::Win32::Foundation::HANDLE,
    path: &Path,
) -> anyhow::Result<HandleDacl> {
    use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorControl, GetSecurityDescriptorDacl,
        SE_DACL_PROTECTED,
    };

    let mut acl = std::ptr::null_mut();
    let mut descriptor = std::ptr::null_mut();
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut acl,
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(std::io::Error::from_raw_os_error(status as i32))
            .with_context(|| format!("Failed to query backup DACL by handle: {}", path.display()));
    }
    let descriptor = LocalSecurityDescriptor(descriptor);
    let mut present = 0;
    let mut descriptor_acl: *mut ACL = std::ptr::null_mut();
    if unsafe {
        GetSecurityDescriptorDacl(
            descriptor.0,
            &mut present,
            &mut descriptor_acl,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("Failed to inspect backup DACL: {}", path.display()));
    }
    let mut control = 0;
    let mut revision = 0;
    if unsafe { GetSecurityDescriptorControl(descriptor.0, &mut control, &mut revision) } == 0 {
        return Err(std::io::Error::last_os_error()).with_context(|| {
            format!(
                "Failed to inspect backup DACL protection: {}",
                path.display()
            )
        });
    }
    let acl_bytes = if descriptor_acl.is_null() {
        None
    } else {
        let size = unsafe { (*descriptor_acl).AclSize as usize };
        Some(unsafe { std::slice::from_raw_parts(descriptor_acl.cast::<u8>(), size) }.to_vec())
    };
    Ok(HandleDacl {
        _descriptor: descriptor,
        acl: descriptor_acl,
        snapshot: DaclSnapshot {
            present: present != 0,
            acl: acl_bytes,
            protected: control & SE_DACL_PROTECTED != 0,
        },
    })
}

#[cfg(windows)]
fn copy_open_handle_dacl<S, D>(
    source: &S,
    destination: &D,
    source_path: &Path,
    destination_path: &Path,
) -> anyhow::Result<()>
where
    S: std::os::windows::io::AsRawHandle,
    D: std::os::windows::io::AsRawHandle,
{
    use windows_sys::Win32::Security::Authorization::{SE_FILE_OBJECT, SetSecurityInfo};
    use windows_sys::Win32::Security::{
        DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
    };

    let source_dacl = get_handle_dacl(source.as_raw_handle() as _, source_path)?;
    anyhow::ensure!(
        source_dacl.snapshot.present && source_dacl.snapshot.acl.is_some(),
        "Backup DACL is absent or NULL; refusing capability-relative keep-backups copy: {}",
        source_path.display()
    );
    anyhow::ensure!(
        source_dacl.snapshot.protected,
        "Backup has an inherited DACL that cannot be safely reproduced by --keep-backups: {}",
        source_path.display()
    );
    let status = unsafe {
        SetSecurityInfo(
            destination.as_raw_handle() as _,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            source_dacl.acl,
            std::ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(std::io::Error::from_raw_os_error(status as i32)).with_context(|| {
            format!(
                "Failed to preserve backup DACL on staged handle: {}",
                destination_path.display()
            )
        });
    }
    let actual = get_handle_dacl(destination.as_raw_handle() as _, destination_path)?;
    let expected = DaclSnapshot {
        protected: true,
        ..source_dacl.snapshot
    };
    anyhow::ensure!(
        actual.snapshot == expected,
        "Staged backup DACL differs from source: {}",
        destination_path.display()
    );
    Ok(())
}

#[cfg(windows)]
fn protect_staging_directory_capability(
    parent: &cap_std::fs::Dir,
    name: &std::ffi::OsStr,
    display_path: &Path,
) -> anyhow::Result<()> {
    use cap_std::fs::{OpenOptions, OpenOptionsExt};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_LIST_DIRECTORY,
        FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, READ_CONTROL,
        SYNCHRONIZE, WRITE_DAC,
    };

    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .access_mode(
            WRITE_DAC | READ_CONTROL | FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY | SYNCHRONIZE,
        )
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    let handle = parent.open_with(name, &options).with_context(|| {
        format!(
            "Failed to open staging directory security handle: {}",
            display_path.display()
        )
    })?;
    set_handle_dacl_from_sddl(handle.as_raw_handle() as _, display_path)
}

#[cfg(windows)]
fn set_handle_dacl_from_sddl(
    handle: windows_sys::Win32::Foundation::HANDLE,
    display_path: &Path,
) -> anyhow::Result<()> {
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1, SE_FILE_OBJECT,
        SetSecurityInfo,
    };
    use windows_sys::Win32::Security::{
        ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl,
        PROTECTED_DACL_SECURITY_INFORMATION,
    };

    let wide_sddl = PRIVATE_STAGING_DACL_SDDL
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut raw_descriptor = std::ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide_sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut raw_descriptor,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).with_context(|| {
            format!(
                "Failed to create private staging DACL: {}",
                display_path.display()
            )
        });
    }
    let descriptor = LocalSecurityDescriptor(raw_descriptor);
    let mut present = 0;
    let mut acl: *mut ACL = std::ptr::null_mut();
    if unsafe {
        GetSecurityDescriptorDacl(descriptor.0, &mut present, &mut acl, std::ptr::null_mut())
    } == 0
        || present == 0
        || acl.is_null()
    {
        anyhow::bail!(
            "Private staging DACL descriptor is invalid for {}",
            display_path.display()
        );
    }
    let status = unsafe {
        SetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            acl,
            std::ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(std::io::Error::from_raw_os_error(status as i32)).with_context(|| {
            format!(
                "Failed to restrict restore staging DACL: {}",
                display_path.display()
            )
        });
    }
    let actual = get_handle_dacl(handle, display_path)?;
    anyhow::ensure!(
        actual.snapshot.protected && actual.snapshot.present && actual.snapshot.acl.is_some(),
        "Restore staging DACL is not protected: {}",
        display_path.display()
    );
    Ok(())
}

#[cfg(windows)]
fn create_restore_staging_directory(
    parent: &cap_std::fs::Dir,
    destination: &Path,
) -> anyhow::Result<(
    std::ffi::OsString,
    cap_std::fs::Dir,
    quarantine::EntryIdentity,
)> {
    for _ in 0..16 {
        let name = std::ffi::OsString::from(format!(
            ".agentsync-restore-{:032x}",
            rand::random::<u128>()
        ));
        match parent.create_dir(&name) {
            Ok(()) => {
                let directory = parent.open_dir_nofollow(&name)?;
                protect_staging_directory_capability(parent, &name, destination)?;
                let identity = quarantine::EntryIdentity::capture(&directory.metadata(".")?);
                return Ok((name, directory, identity));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to create capability-relative restore staging directory for {}",
                        destination.display()
                    )
                });
            }
        }
    }
    anyhow::bail!(
        "Unable to reserve restore staging directory for {}",
        destination.display()
    )
}

#[cfg(windows)]
fn cleanup_empty_restore_staging_directory_windows(
    parent: &cap_std::fs::Dir,
    name: &std::ffi::OsStr,
    identity: quarantine::EntryIdentity,
    destination: &Path,
) -> anyhow::Result<()> {
    match quarantine::remove_empty_directory_if_unchanged(
        parent,
        name,
        identity,
        destination,
        || {},
        |_path| {},
    )? {
        quarantine::RemoveDirectoryOutcome::Removed => Ok(()),
        quarantine::RemoveDirectoryOutcome::Changed
        | quarantine::RemoveDirectoryOutcome::NotEmpty => {
            anyhow::bail!(
                "Restore staging directory changed or remained non-empty beside {}",
                destination.display()
            )
        }
    }
}

#[cfg(windows)]
fn copy_backup_file_capability_windows(
    parent: &cap_std::fs::Dir,
    backup_name: &std::ffi::OsStr,
    destination_name: &std::ffi::OsStr,
    backup_path: &Path,
    destination_path: &Path,
    expected: quarantine::EntryIdentity,
) -> anyhow::Result<()> {
    use cap_std::fs::{DirExt, OpenOptions, OpenOptionsExt};
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
        FILE_READ_DATA, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_DATA,
        READ_CONTROL, SYNCHRONIZE, WRITE_DAC,
    };

    let mut source_options = OpenOptions::new();
    source_options
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .access_mode(FILE_READ_DATA | FILE_READ_ATTRIBUTES | READ_CONTROL | SYNCHRONIZE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
    let mut source = parent
        .open_with(backup_name, &source_options)
        .with_context(|| {
            format!(
                "Failed to open backup by parent capability: {}",
                backup_path.display()
            )
        })?;
    let source_metadata = source.metadata()?;
    anyhow::ensure!(
        source_metadata.is_file()
            && !source_metadata.file_type().is_symlink()
            && expected.matches(&source_metadata),
        "Backup changed before capability-relative copy: {}",
        backup_path.display()
    );

    let mut staging_name = None;
    let mut staging = None;
    for _ in 0..16 {
        let name = std::ffi::OsString::from(format!(
            ".agentsync-restore-{:032x}",
            rand::random::<u128>()
        ));
        match parent.create_dir(&name) {
            Ok(()) => {
                let directory = parent.open_dir_nofollow(&name)?;
                protect_staging_directory_capability(parent, &name, destination_path)?;
                staging_name = Some(name);
                staging = Some(directory);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to create capability-relative staging directory for {}",
                        destination_path.display()
                    )
                });
            }
        }
    }
    let staging_name = staging_name.ok_or_else(|| {
        anyhow::anyhow!(
            "Unable to reserve restore staging directory for {}",
            destination_path.display()
        )
    })?;
    let staging = staging
        .ok_or_else(|| anyhow::anyhow!("Restore staging directory handle was not created"))?;

    let mut destination_options = OpenOptions::new();
    destination_options
        .read(true)
        .write(true)
        .create_new(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .access_mode(
            DELETE
                | FILE_READ_DATA
                | FILE_WRITE_DATA
                | FILE_READ_ATTRIBUTES
                | READ_CONTROL
                | WRITE_DAC
                | SYNCHRONIZE,
        )
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    let mut destination = staging
        .open_with(std::ffi::OsStr::new("file"), &destination_options)
        .with_context(|| {
            format!(
                "Failed to create staged backup file for {}",
                destination_path.display()
            )
        })?;
    std::io::copy(&mut source, &mut destination).with_context(|| {
        format!(
            "Failed to copy backup bytes for {}",
            destination_path.display()
        )
    })?;
    destination.sync_all()?;
    destination.set_permissions(source_metadata.permissions())?;
    copy_open_handle_dacl(&source, &destination, backup_path, destination_path)?;
    let staged_identity = quarantine::EntryIdentity::capture(&destination.metadata()?);

    quarantine::rename_open_handle(&destination, parent, destination_name).with_context(|| {
        format!(
            "Failed to publish copied backup without replacement; staging recovery directory remains at {}",
            restore_staging_path(destination_path, &staging_name).display()
        )
    })?;
    let published = parent.symlink_metadata(destination_name)?;
    anyhow::ensure!(
        published.is_file() && staged_identity.matches(&published),
        "Restored file changed identity before verification: {}",
        destination_path.display()
    );
    let staging_identity = quarantine::EntryIdentity::capture(&staging.metadata(".")?);
    cleanup_empty_restore_staging_directory_windows(
        parent,
        &staging_name,
        staging_identity,
        destination_path,
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn copy_backup_directory_capability(
    parent: &cap_std::fs::Dir,
    backup_name: &std::ffi::OsStr,
    destination_name: &std::ffi::OsStr,
    backup_path: &Path,
    destination_path: &Path,
    expected: quarantine::EntryIdentity,
) -> anyhow::Result<usize> {
    let source_root = parent.open_dir_nofollow(backup_name).with_context(|| {
        format!(
            "Failed to open backup directory without following links: {}",
            backup_path.display()
        )
    })?;
    let source_metadata = source_root.metadata(".")?;
    anyhow::ensure!(
        expected.matches(&source_metadata),
        "Backup directory changed before capability-relative copy: {}",
        backup_path.display()
    );
    let (staging_name, staging, staging_identity) =
        create_restore_staging_directory(parent, destination_path)?;
    let tree_name = std::ffi::OsStr::new("tree");
    staging.create_dir(tree_name).with_context(|| {
        format!(
            "Failed to create staged restore tree beside {}",
            destination_path.display()
        )
    })?;
    let staged_root = staging.open_dir_nofollow(tree_name)?;
    let staged_root_identity = quarantine::EntryIdentity::capture(&staged_root.metadata(".")?);
    let mut directories = vec![(
        Vec::<std::ffi::OsString>::new(),
        source_metadata.permissions(),
    )];
    let skipped = copy_directory_contents_capability(
        &source_root,
        &staged_root,
        backup_path,
        destination_path,
        &mut directories,
    )?;
    if skipped > 0 {
        staging.remove_dir_all(tree_name).with_context(|| {
            format!(
                "Failed to discard partial restore tree for {}",
                destination_path.display()
            )
        })?;
        cleanup_empty_restore_staging_directory(
            parent,
            &staging_name,
            staging_identity,
            destination_path,
        )?;
        return Ok(skipped);
    }

    for (relative, permissions) in directories.iter().rev() {
        let directory = open_relative_directory(&staged_root, relative)?;
        directory
            .set_permissions(".", permissions.clone())
            .with_context(|| {
                format!(
                    "Failed to preserve restored directory permissions for {}",
                    destination_path.display()
                )
            })?;
    }
    if let Err(error) =
        quarantine::rename_between_no_replace(&staging, tree_name, parent, destination_name)
    {
        return Err(error).with_context(|| {
            format!(
                "Failed to publish restored directory without replacement; staging recovery directory remains at {}",
                restore_staging_path(destination_path, &staging_name).display()
            )
        });
    }
    let published = parent.symlink_metadata(destination_name).with_context(|| {
        format!(
            "Failed to verify restored directory: {}",
            destination_path.display()
        )
    })?;
    anyhow::ensure!(
        published.is_dir()
            && !published.file_type().is_symlink()
            && staged_root_identity.matches(&published),
        "Restored directory changed identity before verification: {}",
        destination_path.display()
    );
    cleanup_empty_restore_staging_directory(
        parent,
        &staging_name,
        staging_identity,
        destination_path,
    )?;
    Ok(0)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn copy_directory_contents_capability(
    source_root: &cap_std::fs::Dir,
    destination_root: &cap_std::fs::Dir,
    source_display: &Path,
    destination_display: &Path,
    directories: &mut Vec<(Vec<std::ffi::OsString>, cap_std::fs::Permissions)>,
) -> anyhow::Result<usize> {
    let mut pending = vec![Vec::<std::ffi::OsString>::new()];
    let mut skipped = 0usize;
    while let Some(relative) = pending.pop() {
        let source_directory = open_relative_directory(source_root, &relative)?;
        let destination_directory = open_relative_directory(destination_root, &relative)?;
        let entries = source_directory
            .entries()
            .with_context(|| {
                format!(
                    "Failed to read backup directory: {}",
                    source_display.display()
                )
            })?
            .map(|entry| {
                let entry = entry.with_context(|| {
                    format!("Failed to read backup entry: {}", source_display.display())
                })?;
                let name = entry.file_name();
                let metadata = source_directory.symlink_metadata(&name).with_context(|| {
                    format!(
                        "Failed to inspect backup entry: {}",
                        append_relative(source_display, &relative)
                            .join(&name)
                            .display()
                    )
                })?;
                Ok((name, metadata))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        for (name, metadata) in entries {
            let mut child_relative = relative.clone();
            child_relative.push(name.clone());
            let source_path = append_relative(source_display, &child_relative);
            let destination_path = append_relative(destination_display, &child_relative);
            if metadata.file_type().is_symlink() {
                skipped += 1;
            } else if metadata.is_dir() {
                let child = source_directory.open_dir_nofollow(&name)?;
                let child_metadata = child.metadata(".")?;
                anyhow::ensure!(
                    quarantine::EntryIdentity::capture(&child_metadata)
                        == quarantine::EntryIdentity::capture(&metadata),
                    "Backup directory changed while staging: {}",
                    source_path.display()
                );
                destination_directory.create_dir(&name).with_context(|| {
                    format!(
                        "Failed to create staged directory: {}",
                        destination_path.display()
                    )
                })?;
                directories.push((child_relative.clone(), metadata.permissions()));
                pending.push(child_relative);
            } else if metadata.is_file() {
                let mut source_file =
                    open_backup_file_capability(&source_directory, &name, &source_path)?;
                let source_file_metadata = source_file.metadata()?;
                anyhow::ensure!(
                    source_file_metadata.is_file()
                        && quarantine::EntryIdentity::capture(&source_file_metadata)
                            == quarantine::EntryIdentity::capture(&metadata),
                    "Backup file changed while staging: {}",
                    source_path.display()
                );
                let mut destination_file = create_restore_file_capability(
                    &destination_directory,
                    &name,
                    &destination_path,
                )?;
                std::io::copy(&mut source_file, &mut destination_file).with_context(|| {
                    format!("Failed to copy backup file: {}", source_path.display())
                })?;
                destination_file.sync_all()?;
                destination_file.set_permissions(metadata.permissions())?;
            } else {
                skipped += 1;
                tracing::warn!(path = %source_path.display(), "Skipping special backup entry during keep-backups copy");
            }
        }
    }
    Ok(skipped)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_relative_directory(
    root: &cap_std::fs::Dir,
    relative: &[std::ffi::OsString],
) -> anyhow::Result<cap_std::fs::Dir> {
    let mut current = root.try_clone()?;
    for component in relative {
        current = current.open_dir_nofollow(component)?;
    }
    Ok(current)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn append_relative(root: &Path, relative: &[std::ffi::OsString]) -> std::path::PathBuf {
    relative
        .iter()
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_backup_file_capability(
    parent: &cap_std::fs::Dir,
    name: &std::ffi::OsStr,
    display: &Path,
) -> anyhow::Result<cap_std::fs::File> {
    use cap_std::fs::OpenOptionsExt;

    let mut options = cap_std::fs::OpenOptions::new();
    options
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    parent.open_with(name, &options).with_context(|| {
        format!(
            "Failed to open backup without following links: {}",
            display.display()
        )
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn create_restore_file_capability(
    parent: &cap_std::fs::Dir,
    name: &std::ffi::OsStr,
    display: &Path,
) -> anyhow::Result<cap_std::fs::File> {
    use cap_std::fs::OpenOptionsExt;

    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true).write(true).create_new(true).mode(0o600);
    parent.open_with(name, &options).with_context(|| {
        format!(
            "Failed to create staged restore file for {}",
            display.display()
        )
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn create_restore_staging_directory(
    parent: &cap_std::fs::Dir,
    destination: &Path,
) -> anyhow::Result<(
    std::ffi::OsString,
    cap_std::fs::Dir,
    quarantine::EntryIdentity,
)> {
    use cap_std::fs::DirBuilderExt;

    for _ in 0..16 {
        let name = std::ffi::OsString::from(format!(
            ".agentsync-restore-{:032x}",
            rand::random::<u128>()
        ));
        let mut builder = cap_std::fs::DirBuilder::new();
        builder.mode(0o700);
        match parent.create_dir_with(&name, &builder) {
            Ok(()) => {
                let directory = parent.open_dir_nofollow(&name)?;
                let identity = quarantine::EntryIdentity::capture(&directory.metadata(".")?);
                return Ok((name, directory, identity));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to create restore staging directory for {}",
                        destination.display()
                    )
                });
            }
        }
    }
    anyhow::bail!(
        "Unable to reserve restore staging directory for {}",
        destination.display()
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn cleanup_empty_restore_staging_directory(
    parent: &cap_std::fs::Dir,
    staging_name: &std::ffi::OsStr,
    identity: quarantine::EntryIdentity,
    destination: &Path,
) -> anyhow::Result<()> {
    match quarantine::remove_empty_directory_if_unchanged(
        parent,
        staging_name,
        identity,
        destination,
        || {},
        |_path| {},
    )? {
        quarantine::RemoveDirectoryOutcome::Removed => Ok(()),
        quarantine::RemoveDirectoryOutcome::Changed
        | quarantine::RemoveDirectoryOutcome::NotEmpty => {
            anyhow::bail!(
                "Restore staging directory changed or remained non-empty beside {}",
                destination.display()
            )
        }
    }
}

fn restore_staging_path(destination: &Path, staging_name: &std::ffi::OsStr) -> std::path::PathBuf {
    destination
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(staging_name)
}

/// Copy a backup to an absent `dest` without consuming it (for `--keep-backups`).
/// Returns the number of special entries skipped along the way, so the caller
/// can refuse to report a partial copy as a successful restore.
#[cfg(test)]
fn copy_backup_contents(backup: &Path, dest: &Path) -> anyhow::Result<usize> {
    copy_backup_contents_with_file_publish_hook(backup, dest, || Ok(()))
}

#[cfg(test)]
fn copy_backup_contents_with_file_publish_hook<F>(
    backup: &Path,
    dest: &Path,
    before_file_publish: F,
) -> anyhow::Result<usize>
where
    F: FnOnce() -> anyhow::Result<()>,
{
    let metadata = fs::symlink_metadata(backup)
        .with_context(|| format!("Failed to stat backup for restore: {}", backup.display()))?;
    if metadata.is_dir() {
        let parent = dest.parent().unwrap_or_else(|| Path::new("."));
        let staging = tempfile::Builder::new()
            .prefix(".agentsync-restore-")
            .tempdir_in(parent)
            .with_context(|| {
                format!(
                    "Failed to create restore staging directory beside {}",
                    dest.display()
                )
            })?;
        #[cfg(windows)]
        protect_staging_directory(staging.path())?;
        let staged_tree = staging.path().join("tree");
        let mut directory_permissions = StagedDirectoryPermissions::default();
        let skipped = copy_dir_all(backup, &staged_tree, &mut directory_permissions)?;
        if skipped != 0 {
            return Ok(skipped);
        }
        #[cfg(unix)]
        apply_staged_directory_permissions(&directory_permissions)?;
        #[cfg(windows)]
        apply_staged_directory_acls(&directory_permissions)?;
        if let Err(error) = rename_exclusive(&staged_tree, dest) {
            #[cfg(unix)]
            reset_staged_directory_permissions(&directory_permissions).with_context(|| {
                format!(
                    "Failed to reset restore staging permissions after publish failure: {}",
                    staged_tree.display()
                )
            })?;
            #[cfg(windows)]
            reset_staged_directory_acls(&directory_permissions).with_context(|| {
                format!(
                    "Failed to reset restore staging ACLs after publish failure: {}",
                    staged_tree.display()
                )
            })?;
            return Err(error).with_context(|| {
                format!(
                    "Failed to publish restored directory exclusively: {}",
                    dest.display()
                )
            });
        }
        Ok(0)
    } else if metadata.is_file() {
        copy_backup_file_staged_exclusive(backup, dest, before_file_publish).map(|()| 0)
    } else {
        // Never materialize symlinks, fifos, sockets, or other special files
        // from a backup into the project tree.
        println!(
            "  {} Skipping special backup file (not a regular file or directory): {}",
            "!".yellow(),
            backup.display()
        );
        tracing::warn!(path = %backup.display(), "Skipping backup restore: not a regular file or directory");
        Ok(1)
    }
}

/// Restore a regular-file or directory backup to an absent destination by
/// atomically moving the original filesystem entry without replacement.
#[cfg(test)]
fn move_backup_contents(backup: &Path, dest: &Path) -> anyhow::Result<usize> {
    move_backup_contents_with_publish_hook(backup, dest, || Ok(()))
}

#[cfg(test)]
fn move_backup_contents_with_publish_hook<F>(
    backup: &Path,
    dest: &Path,
    before_publish: F,
) -> anyhow::Result<usize>
where
    F: FnOnce() -> anyhow::Result<()>,
{
    let metadata = fs::symlink_metadata(backup)
        .with_context(|| format!("Failed to stat backup for restore: {}", backup.display()))?;
    if !metadata.is_file() && !metadata.is_dir() {
        // Never move a root symlink, fifo, socket, or other special backup
        // entry into the project tree.
        println!(
            "  {} Skipping special backup file (not a regular file or directory): {}",
            "!".yellow(),
            backup.display()
        );
        tracing::warn!(path = %backup.display(), "Skipping backup restore: not a regular file or directory");
        return Ok(1);
    }

    before_publish().context("Failed before publishing restored backup")?;
    rename_exclusive(backup, dest).with_context(|| {
        format!(
            "Failed to publish restored backup without replacement: {}",
            dest.display()
        )
    })?;
    Ok(0)
}

/// Copy one regular file exclusively. Used for files inside a staged directory
/// restore, whose enclosing tree is already private and published atomically.
#[cfg(test)]
fn copy_file_exclusive(src: &Path, dst: &Path) -> anyhow::Result<()> {
    let source = fs::File::open(src)
        .with_context(|| format!("Failed to open backup file: {}", src.display()))?;
    let metadata = source
        .metadata()
        .with_context(|| format!("Failed to stat open backup file: {}", src.display()))?;
    if !metadata.is_file() {
        anyhow::bail!(
            "Backup entry is no longer a regular file: {}",
            src.display()
        );
    }
    #[cfg(windows)]
    {
        copy_file_with_windows_security(src, dst)?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let mut source = source;
        let mut destination = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dst)
            .with_context(|| {
                format!(
                    "Failed to create restore file exclusively: {}",
                    dst.display()
                )
            })?;
        std::io::copy(&mut source, &mut destination).with_context(|| {
            format!(
                "Failed to copy backup {} to {}",
                src.display(),
                dst.display()
            )
        })?;
        destination
            .set_permissions(metadata.permissions())
            .with_context(|| {
                format!("Failed to preserve backup permissions on {}", dst.display())
            })?;
        Ok(())
    }
}

/// Stage one regular backup file privately beside its destination, then
/// publish it without replacing an existing destination.
#[cfg(test)]
fn copy_backup_file_staged_exclusive<F>(
    src: &Path,
    dst: &Path,
    before_publish: F,
) -> anyhow::Result<()>
where
    F: FnOnce() -> anyhow::Result<()>,
{
    let source = fs::File::open(src)
        .with_context(|| format!("Failed to open backup file: {}", src.display()))?;
    let metadata = source
        .metadata()
        .with_context(|| format!("Failed to stat open backup file: {}", src.display()))?;
    if !metadata.is_file() {
        anyhow::bail!(
            "Backup entry is no longer a regular file: {}",
            src.display()
        );
    }
    let parent = dst.parent().unwrap_or_else(|| Path::new("."));
    let staging = tempfile::Builder::new()
        .prefix(".agentsync-restore-")
        .tempdir_in(parent)
        .with_context(|| {
            format!(
                "Failed to create restore staging directory beside {}",
                dst.display()
            )
        })?;
    #[cfg(windows)]
    protect_staging_directory(staging.path())?;

    let staged_path;
    #[cfg(windows)]
    {
        let staged_file = tempfile::Builder::new()
            .prefix("file-")
            .tempfile_in(staging.path())
            .with_context(|| {
                format!(
                    "Failed to reserve staged restore file for {}",
                    dst.display()
                )
            })?;
        staged_path = staged_file.path().to_path_buf();
        staged_file.close().with_context(|| {
            format!(
                "Failed to prepare staged restore path for {}",
                dst.display()
            )
        })?;
        copy_file_exclusive(src, &staged_path)?;
    }
    #[cfg(not(windows))]
    {
        let mut source = source;
        let staged_file = tempfile::Builder::new()
            .prefix("file-")
            .tempfile_in(staging.path())
            .with_context(|| {
                format!("Failed to create staged restore file for {}", dst.display())
            })?;
        let (mut destination, path) = staged_file
            .keep()
            .map_err(|error| error.error)
            .with_context(|| {
                format!("Failed to retain staged restore file for {}", dst.display())
            })?;
        std::io::copy(&mut source, &mut destination).with_context(|| {
            format!(
                "Failed to copy backup {} to {}",
                src.display(),
                dst.display()
            )
        })?;
        destination
            .sync_all()
            .with_context(|| format!("Failed to sync staged restore file for {}", dst.display()))?;
        destination
            .set_permissions(metadata.permissions())
            .with_context(|| {
                format!(
                    "Failed to preserve backup permissions on staged file for {}",
                    dst.display()
                )
            })?;
        drop(destination);
        staged_path = path;
    }
    before_publish().with_context(|| {
        format!(
            "Failed before publishing staged restore file: {}",
            dst.display()
        )
    })?;
    rename_exclusive(&staged_path, dst).with_context(|| {
        format!(
            "Failed to publish restored file exclusively: {}",
            dst.display()
        )
    })?;
    Ok(())
}

/// Recursive directory copy (std has none). Skips non-regular entries and
/// returns how many were skipped; caller guarantees both paths are inside
/// the project root via revalidation. Every directory and file is created
/// exclusively, so existing entries are never overwritten or followed.
/// On Unix, directory permission updates are deferred until the full tree is
/// copied and then applied to paths inside the private staging tree. On Windows,
/// source DACLs are copied to the staged tree before publication.
#[cfg(test)]
fn copy_dir_all(
    src: &Path,
    dst: &Path,
    directory_permissions: &mut StagedDirectoryPermissions,
) -> anyhow::Result<usize> {
    fs::create_dir(dst)
        .with_context(|| format!("Failed to create restore directory: {}", dst.display()))?;
    let mut skipped = 0usize;
    // Materialize this directory's entries before descending. Keeping `ReadDir`
    // alive through recursive calls retains one descriptor per depth and can
    // exhaust the process limit on deep backups.
    let entries = fs::read_dir(src)
        .with_context(|| format!("Failed to read backup directory: {}", src.display()))?
        .map(|entry| {
            let entry = entry
                .with_context(|| format!("Failed to read entry in backup: {}", src.display()))?;
            let src_path = entry.path();
            let name = entry.file_name();
            let file_type = entry
                .file_type()
                .with_context(|| format!("Failed to stat backup entry: {}", src_path.display()))?;
            Ok((src_path, name, file_type.is_dir(), file_type.is_file()))
        })
        .collect::<anyhow::Result<Vec<_>>>()
        .with_context(|| format!("Failed to read entries in backup: {}", src.display()))?;
    for (src_path, name, is_dir, is_file) in entries {
        let dst_path = dst.join(name);
        if is_dir {
            skipped += copy_dir_all(&src_path, &dst_path, directory_permissions)?;
        } else if is_file {
            copy_file_exclusive(&src_path, &dst_path)?;
        } else {
            // Never materialize symlinks, fifos, sockets, or other special
            // files from a backup into the project tree.
            println!(
                "  {} Skipping special backup entry: {}",
                "!".yellow(),
                src_path.display()
            );
            tracing::warn!(path = %src_path.display(), "Skipping backup entry: not a regular file or directory");
            skipped += 1;
        }
    }

    #[cfg(unix)]
    {
        let source_permissions = fs::symlink_metadata(src)
            .with_context(|| format!("Failed to stat backup directory: {}", src.display()))?
            .permissions();
        let staged_permissions = fs::symlink_metadata(dst)
            .with_context(|| format!("Failed to stat staged directory: {}", dst.display()))?
            .permissions();
        directory_permissions.push((dst.to_path_buf(), source_permissions, staged_permissions));
    }
    #[cfg(windows)]
    directory_permissions.push((src.to_path_buf(), dst.to_path_buf()));
    Ok(skipped)
}

#[cfg(all(test, unix))]
fn apply_staged_directory_permissions(
    directories: &StagedDirectoryPermissions,
) -> anyhow::Result<()> {
    for (directory, source_permissions, _) in directories {
        if let Err(error) = fs::set_permissions(directory, source_permissions.clone()) {
            let reset_result = reset_staged_directory_permissions(directories);
            return Err(error).with_context(|| {
                match reset_result {
                    Ok(()) => "Failed to preserve restored directory permissions".to_string(),
                    Err(reset_error) => format!(
                        "Failed to preserve restored directory permissions; staging reset also failed: {reset_error}"
                    ),
                }
            });
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
fn reset_staged_directory_permissions(
    directories: &StagedDirectoryPermissions,
) -> std::io::Result<()> {
    let mut first_error = None;
    // `copy_dir_all` records directories in post-order. Restore the root's
    // traversable staging mode before resetting nested paths.
    for (directory, _, staged_permissions) in directories.iter().rev() {
        if let Err(error) = fs::set_permissions(directory, staged_permissions.clone())
            && first_error.is_none()
        {
            first_error = Some(error);
        }
    }
    first_error.map_or(Ok(()), Err)
}

#[cfg(all(windows, test))]
fn apply_staged_directory_acls(directories: &StagedDirectoryPermissions) -> anyhow::Result<()> {
    for (source, staged) in directories {
        if let Err(error) = copy_path_dacl(source, staged) {
            let reset_result = reset_staged_directory_acls(directories);
            return Err(error).with_context(|| match reset_result {
                Ok(()) => "Failed to preserve restored Windows directory DACL".to_string(),
                Err(reset_error) => format!(
                    "Failed to preserve restored Windows directory DACL; staging ACL reset also failed: {reset_error}"
                ),
            });
        }
    }
    Ok(())
}

#[cfg(all(windows, test))]
fn reset_staged_directory_acls(directories: &StagedDirectoryPermissions) -> anyhow::Result<()> {
    let mut first_error = None;
    // copy_dir_all records directories in post-order, so reset the root first
    // to regain traversal before resetting any nested directory ACLs.
    for (_, staged) in directories.iter().rev() {
        if let Err(error) = set_path_dacl_from_sddl(staged, PRIVATE_STAGING_DACL_SDDL)
            && first_error.is_none()
        {
            first_error = Some(error);
        }
    }
    first_error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{make_linker, make_target};
    use super::*;
    use tempfile::TempDir;

    fn make_linker_for_agent(
        project_root: &Path,
        agent_name: &str,
        target: crate::config::TargetConfig,
        compress_agents_md: bool,
    ) -> Linker {
        let agent_config = crate::config::AgentConfig {
            enabled: true,
            description: String::new(),
            targets: std::collections::BTreeMap::from([("target".to_string(), target)]),
        };
        let config = crate::config::Config {
            source_dir: ".agents".to_string(),
            compress_agents_md,
            default_agents: vec![],
            agents: std::collections::BTreeMap::from([(agent_name.to_string(), agent_config)]),
            gitignore: Default::default(),
            mcp: Default::default(),
            mcp_servers: Default::default(),
            plugins: Default::default(),
        };
        Linker::new(config, project_root.join("agentsync.toml"))
    }

    #[test]
    fn restore_backup_dacl_policy_accepts_protected_and_rejects_inherited() {
        let backup = Path::new("backup.bak");

        assert!(validate_keep_backup_dacl(true, backup).is_ok());

        let error = validate_keep_backup_dacl(false, backup).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("inherited DACL"));
        assert!(message.contains("--keep-backups"));
        assert!(message.contains("normal consuming restore"));
    }

    #[test]
    fn restore_backup_dacl_policy_rejects_null_dacl() {
        let backup = Path::new("backup.bak");

        let error = validate_copyable_dacl(true, true, backup).unwrap_err();
        assert!(error.to_string().contains("NULL DACL"));
        assert!(validate_copyable_dacl(true, false, backup).is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn restoring_large_directory_tree_respects_low_file_descriptor_limit() {
        const CHILD_MARKER: &str = "AGENTSYNC_TEST_RESTORE_FD_LIMIT_CHILD";
        const TREE_ROOT: &str = "AGENTSYNC_TEST_RESTORE_FD_LIMIT_TREE";

        if std::env::var_os(CHILD_MARKER).is_some() {
            let tree_root = PathBuf::from(std::env::var_os(TREE_ROOT).unwrap());
            let mut directories = StagedDirectoryPermissions::new();
            let skipped = copy_dir_all(
                &tree_root.join("source"),
                &tree_root.join("staged"),
                &mut directories,
            )
            .unwrap();
            assert_eq!(skipped, 0);
            apply_staged_directory_permissions(&directories).unwrap();
            assert_eq!(directories.len(), 209);
            assert!(tree_root.join("staged/deep-000").exists());
            return;
        }

        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        for index in 0..128 {
            let child = source.join(format!("directory-{index:03}"));
            fs::create_dir(&child).unwrap();
            fs::write(child.join("file.txt"), b"restore me").unwrap();
        }
        let mut deep = source.clone();
        for index in 0..80 {
            deep.push(format!("deep-{index:03}"));
            fs::create_dir(&deep).unwrap();
        }
        fs::write(deep.join("deep.txt"), b"deep restore me").unwrap();

        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg("ulimit -n 64; exec \"$@\"")
            .arg("sh")
            .arg(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("linker::revert::tests::restoring_large_directory_tree_respects_low_file_descriptor_limit")
            .arg("--nocapture")
            .env(CHILD_MARKER, "1")
            .env(TREE_ROOT, temp.path())
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "low-fd restore subprocess failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    #[cfg(unix)]
    fn resetting_staged_directory_permissions_restores_parent_before_child() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().unwrap();
        let root = temp.path().join("staging");
        let parent = root.join("parent");
        let child = parent.join("child");
        fs::create_dir_all(&child).unwrap();
        let staging_permissions = fs::Permissions::from_mode(0o700);
        fs::set_permissions(&child, fs::Permissions::from_mode(0o000)).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o000)).unwrap();
        let directories = vec![
            (
                child,
                fs::Permissions::from_mode(0o755),
                staging_permissions.clone(),
            ),
            (
                parent,
                fs::Permissions::from_mode(0o755),
                staging_permissions.clone(),
            ),
            (root, fs::Permissions::from_mode(0o755), staging_permissions),
        ];

        reset_staged_directory_permissions(&directories).unwrap();

        assert_eq!(
            fs::metadata(temp.path().join("staging/parent/child"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }

    #[test]
    #[cfg(unix)]
    fn revert_symlink_contents_removes_children_and_restores_backups() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let source_dir = project_root.join(".agents/src");
        fs::create_dir_all(&source_dir).unwrap();
        let source_file = source_dir.join("linked.md");
        fs::write(&source_file, "shared").unwrap();

        let dest_dir = project_root.join("dest");
        fs::create_dir_all(&dest_dir).unwrap();
        let dest_child = dest_dir.join("linked.md");
        // Link exactly what apply would have written, or revert verification
        // skips the repointed-looking symlink.
        let target = make_target("src", "dest", SyncType::SymlinkContents);
        let linker = make_linker(project_root, true, target);
        let expected = linker
            .relative_path(&dest_child, &source_file, false)
            .unwrap();
        symlink(&expected, &dest_child).unwrap();
        assert!(dest_dir.join("linked.md").is_symlink());
        fs::write(dest_dir.join("kept.md.bak"), "kept-original").unwrap();

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert!(!dest_dir.join("linked.md").exists());
        assert_eq!(
            fs::read_to_string(dest_dir.join("kept.md")).unwrap(),
            "kept-original"
        );
        assert!(!dest_dir.join("kept.md.bak").exists());
        assert!(result.removed >= 1);
        assert_eq!(result.restored, 1);
    }

    #[test]
    #[cfg(unix)]
    fn revert_reports_destination_metadata_errors_instead_of_treating_them_as_absence() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let source_dir = project_root.join(".agents/src");
        let dest_dir = project_root.join("dest");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir(&dest_dir).unwrap();
        let source_file = source_dir.join("managed.md");
        let dest_child = dest_dir.join("managed.md");
        fs::write(&source_file, "managed source").unwrap();
        symlink("../.agents/src/managed.md", &dest_child).unwrap();

        let linker = make_linker(
            project_root,
            true,
            make_target("src", "dest", SyncType::SymlinkContents),
        );
        *linker.revert_metadata_error_path.borrow_mut() = Some(dest_dir.clone());

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert_eq!(
            result.errors, 1,
            "failed destination inspection is an error"
        );
        assert_eq!(
            result.removed, 0,
            "no child is changed after failed inspection"
        );
        assert!(dest_child.is_symlink(), "the uninspected child must remain");
    }

    #[test]
    fn revert_reports_backup_metadata_errors_and_preserves_backup() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let dest = project_root.join("dest.md");
        let backup = symlinks::backup_path_for_destination(&dest);
        fs::write(&backup, "original").unwrap();

        let linker = make_linker(
            project_root,
            true,
            make_target("source.md", "dest.md", SyncType::Symlink),
        );
        *linker.revert_metadata_error_path.borrow_mut() = Some(backup.clone());
        let mut result = SyncResult::default();

        linker
            .revert_destination(&dest, None, &SyncOptions::default(), &mut result)
            .unwrap();

        assert_eq!(result.errors, 1);
        assert_eq!(result.restored, 0);
        assert!(backup.exists(), "the uninspected backup must remain");
        assert!(
            !dest.exists(),
            "restore must not proceed after inspection fails"
        );
    }

    #[test]
    #[cfg(unix)]
    fn revert_nested_glob_restores_matched_destinations() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let module_dir = project_root.join("mods/a");
        fs::create_dir_all(&module_dir).unwrap();
        let module_source = module_dir.join("AGENTS.md");
        fs::write(&module_source, "# module\n").unwrap();
        symlink("AGENTS.md", module_dir.join("CLAUDE.md")).unwrap();
        fs::write(module_dir.join("CLAUDE.md.bak"), "orig-a\n").unwrap();

        let mut target = make_target(".", "{relative_path}/CLAUDE.md", SyncType::NestedGlob);
        target.pattern = Some("**/AGENTS.md".to_string());
        target.exclude = vec![".agents/**".to_string()];
        let linker = make_linker(project_root, true, target);

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert!(!module_dir.join("CLAUDE.md").is_symlink());
        assert_eq!(
            fs::read_to_string(module_dir.join("CLAUDE.md")).unwrap(),
            "orig-a\n"
        );
        assert!(!module_dir.join("CLAUDE.md.bak").exists());
        assert_eq!(result.restored, 1);
    }

    #[test]
    #[cfg(unix)]
    fn revert_module_map_restores_mapped_destinations() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();

        let mapping = crate::config::ModuleMapping {
            source: "shared/context.md".to_string(),
            destination: "mods/core".to_string(),
            filename_override: None,
        };
        let dest_str = crate::linker::apply::module_map_destination(&mapping, "test");
        let dest = project_root.join(&dest_str);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        // The mapped source must really exist at the apply-side location, or
        // revert verification skips the destination as uncomputable.
        let source_file = project_root.join(".agents/shared/context.md");
        if let Some(parent) = source_file.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&source_file, "ctx").unwrap();
        let mut target = make_target("unused", "unused", SyncType::ModuleMap);
        target.mappings = vec![mapping];
        let linker = make_linker(project_root, true, target);
        // Link exactly what apply would have written.
        let expected = linker.relative_path(&dest, &source_file, false).unwrap();
        symlink(&expected, &dest).unwrap();
        // Backup path computed exactly like production code does.
        let backup = crate::linker::symlinks::backup_path_for_destination(&dest);
        fs::write(&backup, "orig-mapped\n").unwrap();

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert!(!dest.is_symlink());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "orig-mapped\n");
        assert!(!backup.exists());
        assert_eq!(result.restored, 1);
    }

    #[test]
    #[cfg(unix)]
    fn revert_symlink_contents_dry_run_counts_pair_once() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let source_dir = project_root.join(".agents/src");
        fs::create_dir_all(&source_dir).unwrap();
        let source_file = source_dir.join("pair.md");
        fs::write(&source_file, "pair").unwrap();

        let dest_dir = project_root.join("dest");
        fs::create_dir_all(&dest_dir).unwrap();
        let dest_child = dest_dir.join("pair.md");
        let target = make_target("src", "dest", SyncType::SymlinkContents);
        let linker = make_linker(project_root, true, target);
        let expected = linker
            .relative_path(&dest_child, &source_file, false)
            .unwrap();
        symlink(&expected, &dest_child).unwrap();
        // Symlink + orphan `.bak` for the same twin: dry-run mutates nothing,
        // so without visited-tracking both branches would count this pair
        // twice.
        fs::write(dest_dir.join("pair.md.bak"), "pair-original").unwrap();

        let options = SyncOptions {
            dry_run: true,
            ..Default::default()
        };
        let result = linker.revert(&options).unwrap();

        assert_eq!(result.removed, 1, "managed symlink counted exactly once");
        assert_eq!(result.restored, 1, "orphan backup counted exactly once");
        // Dry-run changes nothing on disk.
        assert!(dest_child.is_symlink());
        assert!(dest_dir.join("pair.md.bak").exists());
    }

    #[test]
    #[cfg(unix)]
    fn revert_processes_disabled_agent_symlink_and_backup() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let source_file = project_root.join(".agents/AGENTS.md");
        fs::write(&source_file, "# hello\n").unwrap();

        let dest = project_root.join("CLAUDE.md");
        let target = make_target("AGENTS.md", "CLAUDE.md", SyncType::Symlink);
        // The agent is disabled after apply; revert must still process it.
        let linker = make_linker(project_root, false, target);
        let expected = linker.relative_path(&dest, &source_file, false).unwrap();
        symlink(&expected, &dest).unwrap();
        fs::write(project_root.join("CLAUDE.md.bak"), "original\n").unwrap();

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert!(!dest.is_symlink());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "original\n");
        assert!(!project_root.join("CLAUDE.md.bak").exists());
        assert_eq!(result.restored, 1);
    }

    #[test]
    #[cfg(unix)]
    fn revert_honors_agents_filter_for_disabled_agent() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let source_file = project_root.join(".agents/AGENTS.md");
        fs::write(&source_file, "# hello\n").unwrap();

        let dest = project_root.join("CLAUDE.md");
        let target = make_target("AGENTS.md", "CLAUDE.md", SyncType::Symlink);
        let linker = make_linker(project_root, false, target);
        let expected = linker.relative_path(&dest, &source_file, false).unwrap();
        symlink(&expected, &dest).unwrap();
        fs::write(project_root.join("CLAUDE.md.bak"), "original\n").unwrap();

        // An explicit `--agents` filter for another agent still excludes it.
        let options = SyncOptions {
            agents: Some(vec!["other".to_string()]),
            ..Default::default()
        };
        let result = linker.revert(&options).unwrap();

        assert_eq!(result.removed, 0);
        assert_eq!(result.restored, 0);
        assert!(dest.is_symlink());
        assert!(project_root.join("CLAUDE.md.bak").exists());
    }

    #[test]
    #[cfg(unix)]
    fn revert_leaves_repointed_symlink_and_backup_untouched() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let source_file = project_root.join(".agents/AGENTS.md");
        fs::write(&source_file, "# hello\n").unwrap();

        // User repointed the managed symlink elsewhere after apply.
        let dest = project_root.join("CLAUDE.md");
        fs::write(project_root.join("ELSEWHERE.md"), "elsewhere\n").unwrap();
        symlink("ELSEWHERE.md", &dest).unwrap();
        fs::write(project_root.join("CLAUDE.md.bak"), "original\n").unwrap();

        let target = make_target("AGENTS.md", "CLAUDE.md", SyncType::Symlink);
        let linker = make_linker(project_root, true, target);

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert_eq!(result.skipped, 1);
        assert_eq!(result.removed, 0);
        assert_eq!(result.restored, 0);
        assert!(dest.is_symlink());
        assert_eq!(fs::read_link(&dest).unwrap(), PathBuf::from("ELSEWHERE.md"));
        assert_eq!(
            fs::read_to_string(project_root.join("CLAUDE.md.bak")).unwrap(),
            "original\n"
        );
    }

    #[test]
    fn revert_symlink_target_counts_invalid_destination_as_error() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();

        // An absolute destination fails `ensure_safe_destination`; revert must
        // count it as an error (mirroring the invalid-destination coverage for
        // the other commands) rather than aborting or silently skipping.
        let target = make_target("source.md", "/etc/passwd", SyncType::Symlink);
        let linker = make_linker(project_root, true, target);

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert_eq!(result.errors, 1);
        assert_eq!(result.removed, 0);
    }

    #[test]
    #[cfg(unix)]
    fn revert_keep_backups_leaves_directory_backup_in_place() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let source_file = project_root.join(".agents/shared.md");
        fs::write(&source_file, "shared").unwrap();

        // Destination symlink plus a directory backup (the pre-apply regular
        // directory survived apply as `<dest>.bak`).
        let dest = project_root.join("DATA");
        let target = make_target("shared.md", "DATA", SyncType::Symlink);
        let linker = make_linker(project_root, true, target);
        let expected = linker.relative_path(&dest, &source_file, false).unwrap();
        symlink(&expected, &dest).unwrap();
        let backup = project_root.join("DATA.bak");
        fs::create_dir_all(&backup).unwrap();
        fs::write(backup.join("inner.txt"), "orig-inner\n").unwrap();

        let options = SyncOptions {
            keep_backups: true,
            ..Default::default()
        };
        let result = linker.revert(&options).unwrap();

        // The copy path restores the tree while leaving the `.bak` in place.
        assert!(dest.is_dir());
        assert!(!dest.is_symlink());
        assert_eq!(
            fs::read_to_string(dest.join("inner.txt")).unwrap(),
            "orig-inner\n"
        );
        assert_eq!(
            fs::read_to_string(backup.join("inner.txt")).unwrap(),
            "orig-inner\n"
        );
        assert_eq!(result.restored, 1);
    }

    #[test]
    #[cfg(unix)]
    fn restore_backup_refuses_existing_symlink_destination_with_keep_backups() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path().join("project");
        let external_file = temp.path().join("external.txt");
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        fs::write(&external_file, "external-original\n").unwrap();

        let target = make_target("source.md", "dest.txt", SyncType::Symlink);
        let linker = make_linker(&project_root, true, target);
        let dest = project_root.join("dest.txt");
        symlink(&external_file, &dest).unwrap();
        let backup = project_root.join("dest.txt.bak");
        fs::write(&backup, "backup-content\n").unwrap();
        let options = SyncOptions {
            keep_backups: true,
            ..Default::default()
        };
        let mut result = SyncResult::default();

        linker
            .restore_backup(&dest, &backup, &options, &mut result)
            .unwrap();

        assert!(dest.is_symlink());
        assert_eq!(
            fs::read_to_string(&external_file).unwrap(),
            "external-original\n"
        );
        assert_eq!(fs::read_to_string(&backup).unwrap(), "backup-content\n");
        assert_eq!(result.errors, 1);
        assert_eq!(result.restored, 0);
    }

    #[test]
    fn revert_refuses_regular_destination_conflict_in_dry_run_and_real_run() {
        for dry_run in [false, true] {
            let temp = TempDir::new().unwrap();
            let project_root = temp.path();
            fs::create_dir_all(project_root.join(".agents")).unwrap();
            let target = make_target("source.md", "dest.txt", SyncType::Symlink);
            let linker = make_linker(project_root, true, target);
            let dest = project_root.join("dest.txt");
            let backup = project_root.join("dest.txt.bak");
            fs::write(&dest, "user-edit\n").unwrap();
            fs::write(&backup, "backup-content\n").unwrap();
            let options = SyncOptions {
                dry_run,
                ..Default::default()
            };
            let mut result = SyncResult::default();

            linker
                .revert_destination(&dest, None, &options, &mut result)
                .unwrap();

            assert_eq!(fs::read_to_string(&dest).unwrap(), "user-edit\n");
            assert_eq!(fs::read_to_string(&backup).unwrap(), "backup-content\n");
            assert_eq!(result.errors, 1, "dry_run={dry_run}");
            assert_eq!(result.restored, 0, "dry_run={dry_run}");
        }
    }

    #[test]
    #[cfg(unix)]
    fn revert_preserves_regular_file_replacing_managed_symlink_before_quarantine() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let source = project_root.join(".agents/source.md");
        fs::write(&source, "managed source").unwrap();
        let dest = project_root.join("dest.md");
        let backup = symlinks::backup_path_for_destination(&dest);
        symlink(".agents/source.md", &dest).unwrap();
        fs::write(&backup, "pre-apply contents").unwrap();

        let linker = make_linker(
            project_root,
            true,
            make_target("source.md", "dest.md", SyncType::Symlink),
        );
        let expected = linker.relative_path(&dest, &source, false).unwrap();
        *linker.quarantine_before_move_hook.borrow_mut() = Some(std::rc::Rc::new(move |path| {
            fs::remove_file(path).unwrap();
            fs::write(path, "keep-user-data").unwrap();
        }));
        let mut result = SyncResult::default();

        linker
            .revert_destination(&dest, Some(expected), &SyncOptions::default(), &mut result)
            .unwrap();

        assert_eq!(fs::read_to_string(&dest).unwrap(), "keep-user-data");
        assert_eq!(fs::read_to_string(&backup).unwrap(), "pre-apply contents");
        assert_eq!(result.removed, 0);
        assert_eq!(result.restored, 0);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    #[cfg(unix)]
    fn revert_restores_backup_through_original_parent_after_parent_swap() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let source = project_root.join(".agents/source.md");
        fs::write(&source, "managed source").unwrap();
        let original_parent = project_root.join("nested");
        let moved_parent = project_root.join("nested-before-replacement");
        fs::create_dir(&original_parent).unwrap();
        let dest = original_parent.join("dest.md");
        let linker = make_linker(
            project_root,
            true,
            make_target("source.md", "nested/dest.md", SyncType::Symlink),
        );
        let expected = linker.relative_path(&dest, &source, false).unwrap();
        symlink(&expected, &dest).unwrap();
        let backup = symlinks::backup_path_for_destination(&dest);
        fs::write(&backup, "original user content").unwrap();

        let hook_original = original_parent.clone();
        let hook_moved = moved_parent.clone();
        let hook_expected = expected.clone();
        *linker.quarantine_before_move_hook.borrow_mut() = Some(std::rc::Rc::new(move |_| {
            fs::rename(&hook_original, &hook_moved).unwrap();
            fs::create_dir(&hook_original).unwrap();
            symlink(&hook_expected, hook_original.join("dest.md")).unwrap();
        }));
        let mut result = SyncResult::default();

        linker
            .revert_destination(&dest, Some(expected), &SyncOptions::default(), &mut result)
            .unwrap();

        assert_eq!(
            fs::read_to_string(moved_parent.join("dest.md")).unwrap(),
            "original user content"
        );
        assert!(original_parent.join("dest.md").is_symlink());
        assert!(!moved_parent.join("dest.md.bak").exists());
        assert_eq!(result.removed, 1);
        assert_eq!(result.restored, 1);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.errors, 0);
    }

    #[test]
    #[cfg(unix)]
    fn revert_keep_backups_copies_through_original_parent_after_parent_swap() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let source = project_root.join(".agents/source.md");
        fs::write(&source, "managed source").unwrap();
        let original_parent = project_root.join("nested");
        let moved_parent = project_root.join("nested-before-replacement");
        fs::create_dir(&original_parent).unwrap();
        let dest = original_parent.join("dest.md");
        let linker = make_linker(
            project_root,
            true,
            make_target("source.md", "nested/dest.md", SyncType::Symlink),
        );
        let expected = linker.relative_path(&dest, &source, false).unwrap();
        symlink(&expected, &dest).unwrap();
        let backup = symlinks::backup_path_for_destination(&dest);
        fs::write(&backup, "original user content").unwrap();

        let hook_original = original_parent.clone();
        let hook_moved = moved_parent.clone();
        let hook_expected = expected.clone();
        *linker.quarantine_before_move_hook.borrow_mut() = Some(std::rc::Rc::new(move |_| {
            fs::rename(&hook_original, &hook_moved).unwrap();
            fs::create_dir(&hook_original).unwrap();
            symlink(&hook_expected, hook_original.join("dest.md")).unwrap();
        }));
        let options = SyncOptions {
            keep_backups: true,
            ..Default::default()
        };
        let mut result = SyncResult::default();

        linker
            .revert_destination(&dest, Some(expected), &options, &mut result)
            .unwrap();

        assert_eq!(
            fs::read_to_string(moved_parent.join("dest.md")).unwrap(),
            "original user content"
        );
        assert_eq!(
            fs::read_to_string(moved_parent.join("dest.md.bak")).unwrap(),
            "original user content"
        );
        assert!(original_parent.join("dest.md").is_symlink());
        assert_eq!(result.restored, 1);
        assert_eq!(result.errors, 0);
    }

    #[test]
    #[cfg(unix)]
    fn revert_preserves_symlink_contents_child_excluded_by_pattern() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let source_dir = project_root.join(".agents/src");
        fs::create_dir_all(&source_dir).unwrap();
        let source_file = source_dir.join("ignored.md");
        fs::write(&source_file, "not selected by apply\n").unwrap();

        let dest_dir = project_root.join("dest");
        fs::create_dir_all(&dest_dir).unwrap();
        let dest_child = dest_dir.join("ignored.md");
        let mut target = make_target("src", "dest", SyncType::SymlinkContents);
        target.pattern = Some("*.txt".to_string());
        let linker = make_linker(project_root, true, target);
        let expected = linker
            .relative_path(&dest_child, &source_file, false)
            .unwrap();
        symlink(&expected, &dest_child).unwrap();

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert!(dest_child.is_symlink());
        assert_eq!(fs::read_link(&dest_child).unwrap(), expected);
        assert_eq!(result.removed, 0);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn revert_symlink_contents_orphan_respects_pattern() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents/src")).unwrap();

        let dest_dir = project_root.join("dest");
        fs::create_dir_all(&dest_dir).unwrap();
        let backup = dest_dir.join("ignored.md.bak");
        fs::write(&backup, "original").unwrap();

        let mut target = make_target("src", "dest", SyncType::SymlinkContents);
        target.pattern = Some("*.txt".to_string());
        let linker = make_linker(project_root, true, target);

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert!(!dest_dir.join("ignored.md").exists());
        assert!(backup.exists());
        assert_eq!(result.skipped, 1);
    }

    #[test]
    #[cfg(unix)]
    fn revert_zcode_orphan_pattern_fails_closed_when_source_enumeration_errors() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let agents_dir = project_root.join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(agents_dir.join("commands"), "not a source directory").unwrap();
        let destination_dir = project_root.join(".zcode/commands");
        fs::create_dir_all(&destination_dir).unwrap();
        let backup = destination_dir.join("cmd.md.bak");
        fs::write(&backup, "preserve orphan backup").unwrap();

        let mut target = make_target("commands", ".zcode/commands", SyncType::SymlinkContents);
        target.pattern = Some("cmd.agent.md".to_string());
        let linker = make_linker_for_agent(project_root, "zcode", target, false);

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert!(
            backup.exists(),
            "unknown source scope must not consume the backup"
        );
        assert!(!destination_dir.join("cmd.md").exists());
        assert_eq!(result.restored, 0);
    }

    #[test]
    fn revert_symlink_contents_orphan_respects_zcode_command_filter() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents/src")).unwrap();

        let dest_dir = project_root.join(".zcode/commands");
        fs::create_dir_all(&dest_dir).unwrap();
        let backup = dest_dir.join("ignored.txt.bak");
        fs::write(&backup, "original").unwrap();

        let target = make_target("src", ".zcode/commands", SyncType::SymlinkContents);
        let agent_config = crate::config::AgentConfig {
            enabled: true,
            description: String::new(),
            targets: std::collections::BTreeMap::from([("commands".to_string(), target)]),
        };
        let config = crate::config::Config {
            source_dir: ".agents".to_string(),
            compress_agents_md: false,
            default_agents: vec![],
            agents: std::collections::BTreeMap::from([("zcode".to_string(), agent_config)]),
            gitignore: Default::default(),
            mcp: Default::default(),
            mcp_servers: Default::default(),
            plugins: Default::default(),
        };
        let linker = Linker::new(config, project_root.join("agentsync.toml"));

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert!(!dest_dir.join("ignored.txt").exists());
        assert!(backup.exists());
        assert_eq!(result.skipped, 1);
    }

    #[test]
    #[cfg(unix)]
    fn zcode_apply_clean_and_revert_skip_non_markdown_contents_without_pattern() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let source_dir = project_root.join(".agents/commands");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("cmd.md"), "command").unwrap();
        fs::write(source_dir.join("notes.txt"), "not a command").unwrap();

        let target = make_target("commands", ".zcode/commands", SyncType::SymlinkContents);
        let linker = make_linker_for_agent(project_root, "zcode", target, false);
        let destination_dir = project_root.join(".zcode/commands");

        let applied = linker.sync(&SyncOptions::default()).unwrap();

        assert_eq!(applied.created, 1);
        assert_eq!(applied.skipped, 1);
        assert!(destination_dir.join("cmd.md").is_symlink());
        assert!(
            !destination_dir.join("notes.txt").exists(),
            "apply must not create a non-Markdown Z-Code command link"
        );

        let cleaned = linker.clean(&SyncOptions::default()).unwrap();
        assert_eq!(cleaned.removed, 1);
        assert!(!destination_dir.join("cmd.md").exists());
        assert!(!destination_dir.join("notes.txt").exists());

        linker.sync(&SyncOptions::default()).unwrap();
        let reverted = linker.revert(&SyncOptions::default()).unwrap();
        assert_eq!(reverted.removed, 1);
        assert!(!destination_dir.join("cmd.md").exists());
        assert!(!destination_dir.join("notes.txt").exists());
    }

    #[test]
    #[cfg(unix)]
    fn revert_restores_single_symlink_backup_when_compressed_source_is_missing() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let source = project_root.join(".agents/AGENTS.md");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, "# Instructions\n").unwrap();

        let dest = project_root.join("CLAUDE.md");
        let original = b"pre-existing \0 bytes\n";
        fs::write(&dest, original).unwrap();
        let target = make_target("AGENTS.md", "CLAUDE.md", SyncType::Symlink);
        let linker = make_linker_for_agent(project_root, "test", target, true);

        linker.sync(&SyncOptions::default()).unwrap();
        let compressed = source.with_file_name("AGENTS.compact.md");
        assert!(dest.is_symlink());
        assert!(compressed.exists());
        fs::remove_file(&compressed).unwrap();

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 1);
        assert_eq!(result.restored, 1);
        assert_eq!(fs::read(&dest).unwrap(), original);
        assert!(!dest.is_symlink());
        assert!(!dest.with_file_name("CLAUDE.md.bak").exists());
    }

    #[test]
    #[cfg(unix)]
    fn revert_symlink_contents_restores_backup_when_compressed_source_is_missing() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let source_dir = project_root.join(".agents/instructions");
        fs::create_dir_all(&source_dir).unwrap();
        let source = source_dir.join("AGENTS.md");
        fs::write(&source, "# Instructions\n").unwrap();

        let dest_dir = project_root.join("output");
        fs::create_dir_all(&dest_dir).unwrap();
        let dest = dest_dir.join("AGENTS.md");
        let original = b"pre-existing child bytes\0\n";
        fs::write(&dest, original).unwrap();
        let target = make_target("instructions", "output", SyncType::SymlinkContents);
        let linker = make_linker_for_agent(project_root, "test", target, true);

        linker.sync(&SyncOptions::default()).unwrap();
        let compressed = source.with_file_name("AGENTS.compact.md");
        assert!(dest.is_symlink());
        assert!(compressed.exists());
        fs::remove_file(&compressed).unwrap();

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 1);
        assert_eq!(result.restored, 1);
        assert_eq!(fs::read(&dest).unwrap(), original);
        assert!(!dest.is_symlink());
        assert!(!dest.with_file_name("AGENTS.md.bak").exists());
    }

    #[test]
    fn revert_zcode_orphan_matches_pattern_against_source_name() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let source_dir = project_root.join(".agents/commands");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("foo.agent.md"), "command source").unwrap();

        let dest_dir = project_root.join(".zcode/commands");
        fs::create_dir_all(&dest_dir).unwrap();
        let backup = dest_dir.join("foo.md.bak");
        fs::write(&backup, "original command").unwrap();

        let mut target = make_target("commands", ".zcode/commands", SyncType::SymlinkContents);
        target.pattern = Some("*.agent.md".to_string());
        let agent_config = crate::config::AgentConfig {
            enabled: true,
            description: String::new(),
            targets: std::collections::BTreeMap::from([("commands".to_string(), target)]),
        };
        let config = crate::config::Config {
            source_dir: ".agents".to_string(),
            compress_agents_md: false,
            default_agents: vec![],
            agents: std::collections::BTreeMap::from([("zcode".to_string(), agent_config)]),
            gitignore: Default::default(),
            mcp: Default::default(),
            mcp_servers: Default::default(),
            plugins: Default::default(),
        };
        let linker = Linker::new(config, project_root.join("agentsync.toml"));

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert_eq!(
            fs::read(dest_dir.join("foo.md")).unwrap(),
            b"original command"
        );
        assert!(!backup.exists());
        assert_eq!(result.restored, 1);
        assert_eq!(result.skipped, 0);
    }

    #[test]
    fn revert_zcode_orphan_matches_plain_source_name_after_source_removed() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let source_dir = project_root.join(".agents/commands");
        fs::create_dir_all(&source_dir).unwrap();
        let source = source_dir.join("foo.md");
        fs::write(&source, "command source").unwrap();

        let dest_dir = project_root.join(".zcode/commands");
        fs::create_dir_all(&dest_dir).unwrap();
        let backup = dest_dir.join("foo.md.bak");
        fs::write(&backup, "original command").unwrap();

        let mut target = make_target("commands", ".zcode/commands", SyncType::SymlinkContents);
        target.pattern = Some("foo.md".to_string());
        let agent_config = crate::config::AgentConfig {
            enabled: true,
            description: String::new(),
            targets: std::collections::BTreeMap::from([("commands".to_string(), target)]),
        };
        let config = crate::config::Config {
            source_dir: ".agents".to_string(),
            compress_agents_md: false,
            default_agents: vec![],
            agents: std::collections::BTreeMap::from([("zcode".to_string(), agent_config)]),
            gitignore: Default::default(),
            mcp: Default::default(),
            mcp_servers: Default::default(),
            plugins: Default::default(),
        };
        let linker = Linker::new(config, project_root.join("agentsync.toml"));
        fs::remove_file(source).unwrap();

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert_eq!(
            fs::read(dest_dir.join("foo.md")).unwrap(),
            b"original command"
        );
        assert!(!backup.exists());
        assert_eq!(result.restored, 1);
        assert_eq!(result.skipped, 0);
    }

    #[test]
    #[cfg(unix)]
    fn revert_zcode_collision_uses_the_same_sorted_winner_as_apply() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let source_dir = project_root.join(".agents/commands");
        fs::create_dir_all(&source_dir).unwrap();
        let agent_source = source_dir.join("foo.agent.md");
        let plain_source = source_dir.join("foo.md");
        fs::write(&agent_source, "agent-suffixed command").unwrap();
        fs::write(&plain_source, "plain command").unwrap();

        let dest_dir = project_root.join(".zcode/commands");
        fs::create_dir_all(&dest_dir).unwrap();
        let destination = dest_dir.join("foo.md");
        fs::write(&destination, b"original destination bytes\0\n").unwrap();

        let target = make_target("commands", ".zcode/commands", SyncType::SymlinkContents);
        let agent_config = crate::config::AgentConfig {
            enabled: true,
            description: String::new(),
            targets: std::collections::BTreeMap::from([("commands".to_string(), target)]),
        };
        let config = crate::config::Config {
            source_dir: ".agents".to_string(),
            compress_agents_md: false,
            default_agents: vec![],
            agents: std::collections::BTreeMap::from([("zcode".to_string(), agent_config)]),
            gitignore: Default::default(),
            mcp: Default::default(),
            mcp_servers: Default::default(),
            plugins: Default::default(),
        };
        let linker = Linker::new(config, project_root.join("agentsync.toml"));

        linker.sync(&SyncOptions::default()).unwrap();

        // Apply sorts names: `foo.agent.md` is linked first and `foo.md` is
        // the final source mapped to this destination. Verify that winner from
        // the actual symlink rather than assuming filesystem enumeration order.
        let applied_target = linker
            .relative_path(&destination, &plain_source, false)
            .unwrap();
        assert_eq!(fs::read_link(&destination).unwrap(), applied_target);
        let backup = dest_dir.join("foo.md.bak");
        assert_eq!(
            fs::read(&backup).unwrap(),
            b"original destination bytes\0\n"
        );

        // Force the revert enumerator to see the source entries in sorted
        // order. Apply's last match is `foo.md`, while the old revert logic's
        // first match is `foo.agent.md`. This models a deterministic read_dir
        // order without relying on the host filesystem's iteration order.
        *linker.symlink_contents_source_entries_override.borrow_mut() =
            Some(vec![agent_source, plain_source]);

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert!(!destination.is_symlink(), "managed symlink must be removed");
        assert_eq!(
            fs::read(&destination).unwrap(),
            b"original destination bytes\0\n"
        );
        assert!(!backup.exists(), "restored backup must be consumed");
        assert_eq!(result.restored, 1);
    }

    #[test]
    #[cfg(unix)]
    fn revert_zcode_paired_link_does_not_skip_backup_after_twin_was_visited() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let source_dir = project_root.join(".agents/commands");
        fs::create_dir_all(&source_dir).unwrap();
        let source = source_dir.join("foo.agent.md");
        fs::write(&source, "command source").unwrap();

        let dest_dir = project_root.join(".zcode/commands");
        fs::create_dir_all(&dest_dir).unwrap();
        let dest = dest_dir.join("foo.md");
        let mut target = make_target("commands", ".zcode/commands", SyncType::SymlinkContents);
        target.pattern = Some("*.agent.md".to_string());
        let agent_config = crate::config::AgentConfig {
            enabled: true,
            description: String::new(),
            targets: std::collections::BTreeMap::from([("commands".to_string(), target)]),
        };
        let config = crate::config::Config {
            source_dir: ".agents".to_string(),
            compress_agents_md: false,
            default_agents: vec![],
            agents: std::collections::BTreeMap::from([("zcode".to_string(), agent_config)]),
            gitignore: Default::default(),
            mcp: Default::default(),
            mcp_servers: Default::default(),
            plugins: Default::default(),
        };
        let linker = Linker::new(config, project_root.join("agentsync.toml"));
        let expected = linker.relative_path(&dest, &source, false).unwrap();
        symlink(expected, &dest).unwrap();
        let backup = dest_dir.join("foo.md.bak");
        fs::write(&backup, "original command").unwrap();

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert_eq!(fs::read(&dest).unwrap(), b"original command");
        assert!(!backup.exists());
        assert_eq!(result.removed, 1);
        assert_eq!(result.restored, 1);
        assert_eq!(result.skipped, 0);
    }

    #[test]
    fn revert_keeps_empty_symlink_contents_container() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents/src")).unwrap();
        let dest_dir = project_root.join("dest");
        fs::create_dir_all(&dest_dir).unwrap();
        let target = make_target("src", "dest", SyncType::SymlinkContents);
        let linker = make_linker(project_root, true, target);

        linker.revert(&SyncOptions::default()).unwrap();

        assert!(
            dest_dir.is_dir(),
            "revert must preserve the empty container"
        );
    }

    #[test]
    #[cfg(unix)]
    fn copy_backup_contents_refuses_file_destination_symlink() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let backup = temp.path().join("backup.txt");
        let external = temp.path().join("external.txt");
        let dest = temp.path().join("dest.txt");
        fs::write(&backup, "backup bytes").unwrap();
        fs::write(&external, "external bytes").unwrap();
        symlink(&external, &dest).unwrap();

        let result = copy_backup_contents(&backup, &dest);

        assert!(
            result.is_err(),
            "copy through a destination symlink must fail"
        );
        assert_eq!(fs::read(&external).unwrap(), b"external bytes");
        assert_eq!(fs::read(&backup).unwrap(), b"backup bytes");
    }

    #[test]
    #[cfg(unix)]
    fn copy_backup_contents_refuses_directory_destination_symlink() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let backup = temp.path().join("backup");
        let external = temp.path().join("external");
        let dest = temp.path().join("dest");
        fs::create_dir(&backup).unwrap();
        fs::create_dir(&external).unwrap();
        fs::write(backup.join("existing.txt"), "backup replacement").unwrap();
        fs::write(backup.join("new.txt"), "new backup bytes").unwrap();
        fs::write(external.join("existing.txt"), "external bytes").unwrap();
        symlink(&external, &dest).unwrap();

        let result = copy_backup_contents(&backup, &dest);

        assert!(
            result.is_err(),
            "copy through a destination symlink must fail"
        );
        assert_eq!(
            fs::read(external.join("existing.txt")).unwrap(),
            b"external bytes"
        );
        assert!(!external.join("new.txt").exists());
        assert_eq!(
            fs::read(backup.join("existing.txt")).unwrap(),
            b"backup replacement"
        );
    }

    #[test]
    fn copy_backup_contents_refuses_existing_regular_file_without_modifying_it() {
        let temp = TempDir::new().unwrap();
        let backup = temp.path().join("backup.txt");
        let dest = temp.path().join("dest.txt");
        fs::write(&backup, "backup bytes").unwrap();
        fs::write(&dest, "user bytes").unwrap();

        let result = copy_backup_contents(&backup, &dest);

        assert!(
            result.is_err(),
            "copy must not replace an existing destination"
        );
        assert_eq!(fs::read(&dest).unwrap(), b"user bytes");
        assert_eq!(fs::read(&backup).unwrap(), b"backup bytes");
    }

    #[test]
    #[cfg(windows)]
    fn restore_file_backup_preserves_custom_dacl() {
        const PRIVATE_DACL: &str = "D:P(A;;FA;;;OW)";
        const BROAD_PARENT_DACL: &str = "D:P(A;;FA;;;WD)";

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let backup_parent = project_root.join("backup-parent");
        let destination_parent = project_root.join("destination-parent");
        fs::create_dir_all(&backup_parent).unwrap();
        fs::create_dir_all(&destination_parent).unwrap();
        set_path_dacl_from_sddl(&destination_parent, BROAD_PARENT_DACL).unwrap();

        let backup = backup_parent.join("file.bak");
        let dest = destination_parent.join("file");
        fs::write(&backup, b"private backup bytes").unwrap();
        set_path_dacl_from_sddl(&backup, PRIVATE_DACL).unwrap();
        let source_dacl = read_path_dacl(&backup).unwrap();
        assert_ne!(source_dacl, read_path_dacl(&destination_parent).unwrap());

        let linker = make_linker(
            project_root,
            true,
            make_target("source.md", "destination", SyncType::Symlink),
        );
        let options = SyncOptions {
            keep_backups: true,
            ..Default::default()
        };
        let mut result = SyncResult::default();
        linker
            .restore_backup(&dest, &backup, &options, &mut result)
            .unwrap();

        assert_eq!(fs::read(&dest).unwrap(), b"private backup bytes");
        assert_eq!(read_path_dacl(&dest).unwrap(), source_dacl);
        assert!(backup.exists(), "--keep-backups must preserve the source");
        assert_eq!(result.restored, 1);
    }

    #[test]
    #[cfg(windows)]
    fn restore_file_backup_rejects_inherited_dacl_with_keep_backups() {
        const INHERITABLE_PARENT_DACL: &str = "D:P(A;OICI;FA;;;OW)";

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let backup_parent = project_root.join("backup-parent");
        let destination_parent = project_root.join("destination-parent");
        fs::create_dir_all(&backup_parent).unwrap();
        fs::create_dir_all(&destination_parent).unwrap();
        set_path_dacl_from_sddl(&backup_parent, INHERITABLE_PARENT_DACL).unwrap();

        let backup = backup_parent.join("file.bak");
        let dest = destination_parent.join("file");
        fs::write(&backup, b"inherited backup bytes").unwrap();
        let backup_dacl = read_path_dacl(&backup).unwrap();
        assert!(!backup_dacl.protected, "backup must inherit its DACL");

        let linker = make_linker(
            project_root,
            true,
            make_target("source.md", "destination", SyncType::Symlink),
        );
        let options = SyncOptions {
            keep_backups: true,
            ..Default::default()
        };
        let mut result = SyncResult::default();
        linker
            .restore_backup(&dest, &backup, &options, &mut result)
            .unwrap();

        assert_eq!(result.errors, 1);
        assert_eq!(result.restored, 0);
        assert!(!dest.exists(), "rejected restore must not publish a file");
        assert_eq!(fs::read(&backup).unwrap(), b"inherited backup bytes");
        assert_eq!(read_path_dacl(&backup).unwrap(), backup_dacl);
    }

    #[test]
    #[cfg(windows)]
    fn restore_mcp_snapshot_restricts_dacl_under_broad_parent() {
        const BROAD_PARENT_DACL: &str = "D:P(A;OICI;FA;;;WD)";

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        set_path_dacl_from_sddl(project_root, BROAD_PARENT_DACL).unwrap();
        let parent_dacl = read_path_dacl(project_root).unwrap();
        let config_path = project_root.join(".mcp.json");
        let applied = b"applied MCP config";
        let snapshot = b"snapshot MCP credentials";
        fs::write(&config_path, applied).unwrap();

        let result = super::super::restore_mcp_config_bytes(
            crate::mcp::McpAgent::ClaudeCode,
            project_root,
            &config_path,
            snapshot,
            Some(applied),
        )
        .unwrap();

        assert_eq!(result, super::super::RestoreMcpConfigOutcome::Restored);
        assert_eq!(fs::read(&config_path).unwrap(), snapshot);
        let restored_dacl = read_path_dacl(&config_path).unwrap();
        assert!(restored_dacl.protected);
        assert_ne!(restored_dacl, parent_dacl);
    }

    #[test]
    #[cfg(windows)]
    fn restore_directory_backup_preserves_custom_dacls() {
        const PRIVATE_DACL: &str = "D:P(A;;FA;;;OW)";
        const BROAD_PARENT_DACL: &str = "D:P(A;;FA;;;WD)";

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let backup_parent = project_root.join("backup-parent");
        let destination_parent = project_root.join("destination-parent");
        fs::create_dir_all(&backup_parent).unwrap();
        fs::create_dir_all(&destination_parent).unwrap();
        set_path_dacl_from_sddl(&destination_parent, BROAD_PARENT_DACL).unwrap();

        let backup = backup_parent.join("directory.bak");
        let nested_backup = backup.join("nested");
        fs::create_dir_all(&nested_backup).unwrap();
        let backup_file = nested_backup.join("file.txt");
        fs::write(&backup_file, b"private directory bytes").unwrap();
        for path in [&backup, &nested_backup, &backup_file] {
            set_path_dacl_from_sddl(path, PRIVATE_DACL).unwrap();
        }
        let root_dacl = read_path_dacl(&backup).unwrap();
        let nested_dacl = read_path_dacl(&nested_backup).unwrap();
        let file_dacl = read_path_dacl(&backup_file).unwrap();
        assert_ne!(root_dacl, read_path_dacl(&destination_parent).unwrap());
        assert_ne!(nested_dacl, read_path_dacl(&destination_parent).unwrap());
        assert_ne!(file_dacl, read_path_dacl(&destination_parent).unwrap());

        let dest = destination_parent.join("directory");
        let linker = make_linker(
            project_root,
            true,
            make_target("source.md", "destination", SyncType::Symlink),
        );
        let options = SyncOptions {
            keep_backups: true,
            ..Default::default()
        };
        let mut result = SyncResult::default();
        linker
            .restore_backup(&dest, &backup, &options, &mut result)
            .unwrap();

        assert_eq!(read_path_dacl(&dest).unwrap(), root_dacl);
        assert_eq!(read_path_dacl(&dest.join("nested")).unwrap(), nested_dacl);
        assert_eq!(
            read_path_dacl(&dest.join("nested/file.txt")).unwrap(),
            file_dacl
        );
        assert_eq!(
            fs::read(dest.join("nested/file.txt")).unwrap(),
            b"private directory bytes"
        );
        assert!(
            backup.is_dir(),
            "--keep-backups must preserve the source tree"
        );
        assert_eq!(result.restored, 1);
    }

    #[test]
    #[cfg(windows)]
    fn restore_directory_backup_rejects_inherited_dacl_with_keep_backups() {
        const INHERITABLE_PARENT_DACL: &str = "D:P(A;OICI;FA;;;OW)";

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let backup_parent = project_root.join("backup-parent");
        let destination_parent = project_root.join("destination-parent");
        fs::create_dir_all(&backup_parent).unwrap();
        fs::create_dir_all(&destination_parent).unwrap();
        set_path_dacl_from_sddl(&backup_parent, INHERITABLE_PARENT_DACL).unwrap();

        let backup = backup_parent.join("directory.bak");
        let dest = destination_parent.join("directory");
        fs::create_dir(&backup).unwrap();
        let backup_dacl = read_path_dacl(&backup).unwrap();
        assert!(
            !backup_dacl.protected,
            "backup directory must inherit its DACL"
        );

        let linker = make_linker(
            project_root,
            true,
            make_target("source.md", "destination", SyncType::Symlink),
        );
        let options = SyncOptions {
            keep_backups: true,
            ..Default::default()
        };
        let mut result = SyncResult::default();
        linker
            .restore_backup(&dest, &backup, &options, &mut result)
            .unwrap();

        assert_eq!(result.errors, 1);
        assert_eq!(result.restored, 0);
        assert!(
            !dest.exists(),
            "rejected restore must not publish a directory"
        );
        assert!(backup.is_dir());
        assert_eq!(read_path_dacl(&backup).unwrap(), backup_dacl);
    }

    #[test]
    #[cfg(windows)]
    fn restore_file_backup_keeps_backup_when_exclusive_publish_fails() {
        const PRIVATE_DACL: &str = "D:P(A;;FA;;;OW)";
        const BROAD_PARENT_DACL: &str = "D:P(A;;FA;;;WD)";

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let backup_parent = project_root.join("backup-parent");
        let destination_parent = project_root.join("destination-parent");
        fs::create_dir_all(&backup_parent).unwrap();
        fs::create_dir_all(&destination_parent).unwrap();
        set_path_dacl_from_sddl(&destination_parent, BROAD_PARENT_DACL).unwrap();
        let backup = backup_parent.join("file.bak");
        let dest = destination_parent.join("file");
        fs::write(&backup, b"private backup bytes").unwrap();
        set_path_dacl_from_sddl(&backup, PRIVATE_DACL).unwrap();
        let backup_dacl = read_path_dacl(&backup).unwrap();

        let linker = make_linker(
            project_root,
            true,
            make_target("source.md", "destination", SyncType::Symlink),
        );
        let mut result = SyncResult::default();
        linker
            .restore_backup_with_hook(&dest, &backup, &SyncOptions::default(), &mut result, || {
                fs::write(&dest, b"concurrent user bytes")?;
                Ok(())
            })
            .unwrap();

        assert_eq!(fs::read(&dest).unwrap(), b"concurrent user bytes");
        assert_eq!(read_path_dacl(&backup).unwrap(), backup_dacl);
        assert!(backup.exists(), "failed restore must retain the backup");
        assert_eq!(result.errors, 1);
        assert_eq!(result.restored, 0);
    }

    #[test]
    fn restore_staged_publication_uses_no_replace_for_files_and_directories() {
        let temp = TempDir::new().unwrap();
        let file_backup = temp.path().join("file.bak");
        let file_dest = temp.path().join("file");
        fs::write(&file_backup, "file backup bytes").unwrap();

        assert_eq!(copy_backup_contents(&file_backup, &file_dest).unwrap(), 0);
        assert_eq!(fs::read(&file_dest).unwrap(), b"file backup bytes");
        assert_eq!(fs::read(&file_backup).unwrap(), b"file backup bytes");

        fs::write(&file_dest, "concurrent file bytes").unwrap();
        assert!(copy_backup_contents(&file_backup, &file_dest).is_err());
        assert_eq!(fs::read(&file_dest).unwrap(), b"concurrent file bytes");
        assert_eq!(fs::read(&file_backup).unwrap(), b"file backup bytes");

        let directory_backup = temp.path().join("directory.bak");
        let directory_dest = temp.path().join("directory");
        fs::create_dir(&directory_backup).unwrap();
        fs::write(
            directory_backup.join("original.txt"),
            "directory backup bytes",
        )
        .unwrap();

        assert_eq!(
            copy_backup_contents(&directory_backup, &directory_dest).unwrap(),
            0
        );
        assert_eq!(
            fs::read(directory_dest.join("original.txt")).unwrap(),
            b"directory backup bytes"
        );
        assert_eq!(
            fs::read(directory_backup.join("original.txt")).unwrap(),
            b"directory backup bytes"
        );

        fs::write(
            directory_dest.join("original.txt"),
            "concurrent directory bytes",
        )
        .unwrap();
        assert!(copy_backup_contents(&directory_backup, &directory_dest).is_err());
        assert_eq!(
            fs::read(directory_dest.join("original.txt")).unwrap(),
            b"concurrent directory bytes"
        );
        assert_eq!(
            fs::read(directory_backup.join("original.txt")).unwrap(),
            b"directory backup bytes"
        );
    }

    #[test]
    #[cfg(unix)]
    fn move_backup_contents_preserves_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().unwrap();
        let backup = temp.path().join("destination.txt.bak");
        let destination = temp.path().join("destination.txt");
        fs::write(&backup, "original contents").unwrap();
        fs::set_permissions(&backup, fs::Permissions::from_mode(0o640)).unwrap();

        move_backup_contents(&backup, &destination).unwrap();

        assert_eq!(
            fs::symlink_metadata(&destination)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o640
        );
    }

    #[test]
    #[cfg(unix)]
    fn restore_backup_consumes_unreadable_file_preserving_mode_and_mtime() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::{Duration, UNIX_EPOCH};

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let linker = make_linker(
            project_root,
            true,
            make_target("source.md", "destination", SyncType::Symlink),
        );
        let backup = project_root.join("destination.bak");
        let dest = project_root.join("destination");
        fs::write(&backup, b"unreadable original contents").unwrap();
        let expected_mtime = UNIX_EPOCH + Duration::from_secs(1_234_567_890);
        fs::File::open(&backup)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(expected_mtime))
            .unwrap();
        fs::set_permissions(&backup, fs::Permissions::from_mode(0o000)).unwrap();

        let mut result = SyncResult::default();
        linker
            .restore_backup(&dest, &backup, &SyncOptions::default(), &mut result)
            .unwrap();
        assert_eq!(result.errors, 0);
        assert_eq!(result.restored, 1);
        let metadata = fs::symlink_metadata(&dest).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o000);
        assert_eq!(metadata.modified().unwrap(), expected_mtime);
        assert!(!backup.exists(), "consuming restore removes the backup");
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"unreadable original contents");
    }

    #[test]
    #[cfg(unix)]
    fn restore_backup_consumes_directory_preserving_metadata_and_symlinks() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        use std::time::{Duration, UNIX_EPOCH};

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let linker = make_linker(
            project_root,
            true,
            make_target("source.md", "destination", SyncType::Symlink),
        );
        let backup = project_root.join("directory.bak");
        let nested_backup = backup.join("nested");
        let external_target = project_root.join("external.txt");
        let restored = project_root.join("directory");
        fs::create_dir_all(&nested_backup).unwrap();
        fs::write(nested_backup.join("original.txt"), b"original bytes").unwrap();
        fs::write(&external_target, b"external target bytes").unwrap();
        symlink(&external_target, backup.join("external-link")).unwrap();

        let root_mtime = UNIX_EPOCH + Duration::from_secs(1_234_567_890);
        let nested_mtime = UNIX_EPOCH + Duration::from_secs(1_234_567_891);
        fs::File::open(&backup)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(root_mtime))
            .unwrap();
        fs::File::open(&nested_backup)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(nested_mtime))
            .unwrap();
        fs::set_permissions(&backup, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&nested_backup, fs::Permissions::from_mode(0o711)).unwrap();

        let mut result = SyncResult::default();
        linker
            .restore_backup(&restored, &backup, &SyncOptions::default(), &mut result)
            .unwrap();
        assert_eq!(result.errors, 0);
        assert_eq!(result.restored, 1);

        let restored_root = fs::symlink_metadata(&restored).unwrap();
        assert_eq!(restored_root.permissions().mode() & 0o777, 0o700);
        assert_eq!(restored_root.modified().unwrap(), root_mtime);
        let restored_nested = fs::symlink_metadata(restored.join("nested")).unwrap();
        assert_eq!(restored_nested.permissions().mode() & 0o777, 0o711);
        assert_eq!(restored_nested.modified().unwrap(), nested_mtime);
        let restored_link = fs::symlink_metadata(restored.join("external-link")).unwrap();
        assert!(restored_link.file_type().is_symlink());
        assert_eq!(
            fs::read_link(restored.join("external-link")).unwrap(),
            external_target
        );
        assert_eq!(
            fs::read(&external_target).unwrap(),
            b"external target bytes"
        );
        assert!(
            !backup.exists(),
            "consuming restore removes the backup tree"
        );
    }

    #[test]
    fn move_backup_contents_preserves_destination_when_publish_fails() {
        let temp = TempDir::new().unwrap();
        let backup = temp.path().join("destination.txt.bak");
        let destination = temp.path().join("destination.txt");
        fs::write(&backup, "original contents").unwrap();

        let result = move_backup_contents_with_publish_hook(&backup, &destination, || {
            fs::write(&destination, "concurrent user contents")?;
            Ok(())
        });

        assert!(
            result.is_err(),
            "exclusive publish must refuse the new file"
        );
        assert_eq!(fs::read(&destination).unwrap(), b"concurrent user contents");
        assert_eq!(fs::read(&backup).unwrap(), b"original contents");
    }

    #[test]
    #[cfg(unix)]
    fn restore_backup_discards_partial_directory_copy_with_special_entry() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let linker = make_linker(
            project_root,
            true,
            make_target("source.md", "destination", SyncType::Symlink),
        );
        let backup = project_root.join("destination.bak");
        let dest = project_root.join("destination");
        fs::create_dir(&backup).unwrap();
        fs::write(backup.join("regular.txt"), "backup content").unwrap();
        symlink("regular.txt", backup.join("unsupported-link")).unwrap();
        let options = SyncOptions {
            keep_backups: true,
            ..Default::default()
        };
        let mut result = SyncResult::default();

        linker
            .restore_backup(&dest, &backup, &options, &mut result)
            .unwrap();

        assert!(
            fs::symlink_metadata(&dest).is_err(),
            "an incomplete directory copy must not publish its destination"
        );
        assert_eq!(
            fs::read(backup.join("regular.txt")).unwrap(),
            b"backup content"
        );
        assert!(backup.join("unsupported-link").is_symlink());
        assert_eq!(result.errors, 1);
        assert_eq!(result.restored, 0);
    }

    #[test]
    #[cfg(unix)]
    fn restore_backup_preserves_directory_modes_and_keeps_backup() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let linker = make_linker(
            project_root,
            true,
            make_target("source.md", "destination", SyncType::Symlink),
        );
        let backup = project_root.join("destination.bak");
        let nested_backup = backup.join("nested");
        fs::create_dir_all(&nested_backup).unwrap();
        fs::write(nested_backup.join("file.txt"), "backup content").unwrap();
        fs::set_permissions(&backup, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&nested_backup, fs::Permissions::from_mode(0o750)).unwrap();
        let dest = project_root.join("destination");
        let options = SyncOptions {
            keep_backups: true,
            ..Default::default()
        };
        let mut result = SyncResult::default();

        linker
            .restore_backup(&dest, &backup, &options, &mut result)
            .unwrap();

        assert_eq!(
            fs::symlink_metadata(&dest).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::symlink_metadata(dest.join("nested"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o750
        );
        assert!(
            backup.is_dir(),
            "--keep-backups must preserve the source tree"
        );
        assert_eq!(
            fs::read(backup.join("nested/file.txt")).unwrap(),
            b"backup content"
        );
        assert_eq!(result.restored, 1);
    }

    #[test]
    fn restore_backup_does_not_replace_destination_created_at_restore_boundary() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let target = make_target("source.md", "dest.txt", SyncType::Symlink);
        let linker = make_linker(&project_root, true, target);
        let dest = project_root.join("dest.txt");
        let backup = project_root.join("dest.txt.bak");
        fs::write(&backup, "backup bytes").unwrap();
        let options = SyncOptions::default();
        let mut result = SyncResult::default();

        linker
            .restore_backup_with_hook(&dest, &backup, &options, &mut result, || {
                fs::write(&dest, "injected user bytes")?;
                Ok(())
            })
            .unwrap();

        assert_eq!(fs::read(&dest).unwrap(), b"injected user bytes");
        assert_eq!(fs::read(&backup).unwrap(), b"backup bytes");
        assert_eq!(result.errors, 1);
        assert_eq!(result.restored, 0);
    }
}
