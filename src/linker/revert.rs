//! Revert implementation: undo `apply` by removing managed symlinks and
//! restoring `.bak` backups of pre-existing files.

use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::SyncType;

use super::{Linker, SyncOptions, SyncResult, enumerate, symlinks};

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
    /// and restore every orphaned `.bak` backup, then drop the directory if
    /// it is left empty.
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
        // Try to remove the directory if empty
        if !options.dry_run {
            if let Err(e) = self.revalidate_unlink_path(&dest) {
                result.errors += 1;
                tracing::warn!(error = %e, path = %dest.display(), "Skipping revert directory removal: unsafe destination");
                return Ok(());
            }
            let _ = fs::remove_dir(&dest);
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
        if dest.is_symlink() {
            match &expected {
                Some(want) => {
                    let actual = match fs::read_link(dest) {
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
                    self.remove_managed_symlink(dest, options.dry_run, result)?;
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
        if backup.exists() {
            // Never restore a backup over a real user file that appeared after
            // apply: refusing avoids data loss in both the `rename` and the
            // `--keep-backups` copy paths, and leaves the `.bak` in place.
            // Dry-run never touches the backup, so only real runs can refuse.
            if !options.dry_run && dest.exists() && !dest.is_symlink() {
                result.errors += 1;
                span.record("outcome", "error");
                println!(
                    "  {} Refusing to restore over user file: {}",
                    "!".yellow(),
                    dest.display()
                );
                tracing::warn!(path = %dest.display(), "Refusing to restore backup over existing non-symlink destination");
                return Ok(());
            }
            self.restore_backup(dest, &backup, options, result)?;
        }
        span.record("outcome", "ok");
        Ok(())
    }

    /// Move (or, with keep_backups, copy) a `.bak` backup back to `dest`.
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

/// Copy a backup over `dest` without consuming it (for `--keep-backups`).
/// Returns the number of special entries skipped along the way, so the caller
/// can refuse to report a partial copy as a successful restore.
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
}
