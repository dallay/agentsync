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

#[cfg(windows)]
struct HandleDacl {
    _descriptor: LocalSecurityDescriptor,
    acl: *mut windows_sys::Win32::Security::ACL,
    snapshot: DaclSnapshot,
}

#[cfg(windows)]
const PRIVATE_STAGING_DACL_SDDL: &str = "D:P(A;;FA;;;OW)";

impl Linker {
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
        let resolved = self.expected_source_path(&source_path, target_config)?;
        self.relative_path(dest, &resolved, false).ok()
    }

    /// Expected link content for one `symlink-contents` child: the relative
    /// path from the child to the source entry `apply` linked from (same
    /// destination-name transform as apply, including the zcode command
    /// mapping). `None` when no source entry maps to this child.
    fn symlink_contents_expected_target(
        &self,
        agent_name: &str,
        target_config: &crate::config::TargetConfig,
        dest_child: &Path,
    ) -> Option<PathBuf> {
        let child_name = dest_child.file_name()?.to_str()?;
        let source_dir = self.source_dir.join(&target_config.source);
        for entry in fs::read_dir(&source_dir).ok()?.flatten() {
            let item_name = entry.file_name();
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
                let resolved = self.expected_source_path(&entry.path(), target_config)?;
                return self.relative_path(dest_child, &resolved, false).ok();
            }
        }
        None
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
        if matches!(fs::symlink_metadata(&dest), Ok(m) if m.file_type().is_symlink()) {
            return self.revert_destination(&dest, None, options, result);
        }
        if !dest.is_dir() {
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
            if enumerate::contents_child_is_managed(agent_name, target_config, &entry_path) {
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
                if !visited.insert(twin.clone()) {
                    continue;
                }
                // Never overwrite a real user file that appeared after the
                // backup was taken; restore into an absent twin or over a
                // managed symlink twin.
                if !twin.exists() || twin.is_symlink() {
                    let expected =
                        self.symlink_contents_expected_target(agent_name, target_config, &twin);
                    self.revert_destination(&twin, expected, options, result)?;
                } else {
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
        let enumeration = self.enumerate_nested_glob(target_config, options)?;
        if let Err(s) = enumeration.template {
            result.errors += 1;
            tracing::warn!(destination = %s.dest, error = %s.error, "Skipping revert target with unsafe destination");
            return Ok(());
        }
        if let enumerate::NestedGlobDiscoveryStatus::Incomplete {
            search_root,
            reason,
        } = enumeration.discovery
        {
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
                "Nested-glob revert discovery incomplete; target is skipped"
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
        let (Some(name), Some(parent_path)) = (dest.file_name(), dest.parent()) else {
            result.errors += 1;
            span.record("outcome", "error");
            tracing::warn!(path = %dest.display(), "Skipping revert: destination lacks parent or final component");
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
        let dest_metadata = match parent.symlink_metadata(name) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                result.errors += 1;
                span.record("outcome", "error");
                tracing::warn!(error = %error, path = %dest.display(), "Skipping revert: failed to inspect destination relative to parent capability");
                return Ok(());
            }
        };
        if dest_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.file_type().is_symlink())
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
                    let identity =
                        quarantine::EntryIdentity::capture(dest_metadata.as_ref().ok_or_else(
                            || anyhow::anyhow!("verified symlink metadata disappeared"),
                        )?);
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
                        match parent.symlink_metadata(name) {
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
            tracing::warn!(path = %backup.display(), "Skipping revert: backup lacks a final component");
            return Ok(());
        };
        let backup_metadata = match parent.symlink_metadata(backup_name) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                result.errors += 1;
                span.record("outcome", "error");
                tracing::warn!(error = %error, path = %backup.display(), "Skipping revert: failed to inspect backup relative to parent capability");
                return Ok(());
            }
        };
        if backup_metadata.is_some() {
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
        match fs::symlink_metadata(dest) {
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
        let op: Result<usize> = if options.keep_backups {
            copy_backup_contents(backup, dest)
        } else {
            fs::rename(backup, dest)
                .with_context(|| {
                    format!(
                        "Failed to restore backup {} to {}",
                        backup.display(),
                        dest.display()
                    )
                })
                .map(|()| 0)
        };
        let skipped_copies = match op {
            Ok(skipped) => skipped,
            Err(e) => {
                result.errors += 1;
                span.record("outcome", "error");
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
                "Capability-relative directory --keep-backups copy is unsupported on Windows; backup remains at {}",
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
    let mut defaulted = 0;
    if unsafe {
        GetSecurityDescriptorDacl(
            descriptor.0,
            &mut present,
            &mut descriptor_acl,
            &mut defaulted,
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
    let mut defaulted = 0;
    if unsafe { GetSecurityDescriptorDacl(descriptor.0, &mut present, &mut acl, &mut defaulted) }
        == 0
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
    use cap_fs_ext::DirExt;
    use cap_std::fs::{OpenOptions, OpenOptionsExt};
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

/// Copy a backup over `dest` without consuming it (for `--keep-backups`).
/// Returns the number of special entries skipped along the way, so the caller
/// can refuse to report a partial copy as a successful restore.
#[cfg(test)]
fn copy_backup_contents(backup: &Path, dest: &Path) -> anyhow::Result<usize> {
    let metadata = fs::symlink_metadata(backup)
        .with_context(|| format!("Failed to stat backup for restore: {}", backup.display()))?;
    if metadata.is_dir() {
        copy_dir_all(backup, dest)
    } else if metadata.is_file() {
        fs::copy(backup, dest)
            .with_context(|| {
                format!(
                    "Failed to copy backup {} to {}",
                    backup.display(),
                    dest.display()
                )
            })
            .map(|_| 0)
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

/// Recursive directory copy (std has none). Skips non-regular entries and
/// returns how many were skipped; caller guarantees both paths are inside
/// the project root via revalidation.
#[cfg(test)]
fn copy_dir_all(src: &Path, dst: &Path) -> anyhow::Result<usize> {
    fs::create_dir_all(dst)
        .with_context(|| format!("Failed to create restore directory: {}", dst.display()))?;
    let mut skipped = 0usize;
    for entry in fs::read_dir(src)
        .with_context(|| format!("Failed to read backup directory: {}", src.display()))?
    {
        let entry =
            entry.with_context(|| format!("Failed to read entry in backup: {}", src.display()))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to stat backup entry: {}", src_path.display()))?;
        if file_type.is_dir() {
            skipped += copy_dir_all(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "Failed to copy backup entry {} to {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
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
    Ok(skipped)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{make_linker, make_target};
    use super::*;
    use tempfile::TempDir;

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
    fn revert_skips_missing_nested_glob_root_without_touching_destinations() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let target = make_target(
            "missing-source",
            "dest/{relative_path}/{file_name}",
            SyncType::NestedGlob,
        );
        let linker = make_linker(project_root, true, target);
        let destination = project_root.join("dest/retained.md");
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        let source = project_root.join("user-owned-source.md");
        fs::write(&source, "keep this link").unwrap();
        symlink(&source, &destination).unwrap();

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert_eq!(result.skipped, 1);
        assert_eq!(result.removed, 0);
        assert!(destination.is_symlink());
        assert_eq!(fs::read_link(destination).unwrap(), source);
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
    #[cfg(windows)]
    fn keep_backups_rejection_preserves_symlink_and_backup() {
        use std::os::windows::fs::symlink_file;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let source = project_root.join(".agents/source.md");
        fs::write(&source, "managed source").unwrap();

        let dest = project_root.join("dest.md");
        let target = make_target("source.md", "dest.md", SyncType::Symlink);
        let linker = make_linker(project_root, true, target);
        let expected = linker.relative_path(&dest, &source, false).unwrap();
        symlink_file(&expected, &dest).unwrap();
        let backup = project_root.join("dest.md.bak");
        fs::write(&backup, "original user data").unwrap();

        let options = SyncOptions {
            keep_backups: true,
            ..Default::default()
        };
        let error = match linker.revert(&options) {
            Err(error) => error.to_string(),
            Ok(result) => {
                panic!("Windows keep-backups should be rejected before mutation; result={result:?}")
            }
        };

        assert!(
            error.contains("--keep-backups is not supported on Windows"),
            "{error}"
        );
        assert!(
            dest.is_symlink(),
            "the managed symlink must remain in place"
        );
        assert_eq!(
            fs::read_to_string(&backup).unwrap(),
            "original user data",
            "the backup must remain untouched"
        );
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
    fn revert_does_not_follow_replaced_parent_during_destination_removal() {
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
        let backup = symlinks::backup_path_for_destination(&dest);
        let linker = make_linker(
            project_root,
            true,
            make_target("source.md", "nested/dest.md", SyncType::Symlink),
        );
        let expected = linker.relative_path(&dest, &source, false).unwrap();
        symlink(&expected, &dest).unwrap();
        fs::write(&backup, "original user content").unwrap();

        let hook_original = original_parent.clone();
        let hook_moved = moved_parent.clone();
        let hook_source = expected.clone();
        *linker.quarantine_before_move_hook.borrow_mut() = Some(std::rc::Rc::new(move |_| {
            fs::rename(&hook_original, &hook_moved).unwrap();
            fs::create_dir(&hook_original).unwrap();
            symlink(&hook_source, hook_original.join("dest.md")).unwrap();
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
}
