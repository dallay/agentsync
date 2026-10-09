//! Cleanup implementation for managed symlink targets.

use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::config::SyncType;

use super::{Linker, SyncOptions, SyncResult, enumerate, quarantine};

impl Linker {
    #[cfg(test)]
    fn set_clean_metadata_error_path_for_tests(&self, path: &Path) {
        *self.clean_metadata_error_path.borrow_mut() = Some(path.to_path_buf());
    }

    fn clean_symlink_metadata(&self, path: &Path) -> std::io::Result<fs::Metadata> {
        #[cfg(test)]
        if self.clean_metadata_error_path.borrow().as_deref() == Some(path) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected metadata inspection failure",
            ));
        }

        fs::symlink_metadata(path)
    }

    fn report_clean_metadata_error(
        path: &Path,
        error: &std::io::Error,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) {
        result.skipped += 1;
        if options.verbose {
            println!(
                "  {} Skipping destination that could not be inspected: {} ({})",
                "!".yellow(),
                path.display(),
                error
            );
        }
        tracing::warn!(
            error = %error,
            path = %path.display(),
            "Skipping clean destination: failed to inspect metadata"
        );
    }

    #[cfg(test)]
    fn set_clean_before_read_contents_hook_for_tests<F>(&self, hook: F)
    where
        F: Fn(&Path) + 'static,
    {
        *self.clean_before_read_contents_hook.borrow_mut() = Some(std::rc::Rc::new(hook));
    }

    /// Clean all symlinks managed by this configuration.
    pub fn clean(&self, options: &SyncOptions) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        println!("{}", "Cleaning managed symlinks...".cyan());

        // Clean deliberately processes every configured target. Apply filters do not affect
        // cleanup, so stale managed links cannot survive a filtered apply invocation.
        for (agent_name, agent_config) in &self.config.agents {
            for target_config in agent_config.targets.values() {
                match target_config.sync_type {
                    SyncType::NestedGlob => {
                        self.clean_nested_glob_target(target_config, options, &mut result)?;
                    }
                    SyncType::SymlinkContents => {
                        self.clean_symlink_contents_target(
                            agent_name,
                            target_config,
                            options,
                            &mut result,
                        )?;
                    }
                    SyncType::Symlink => {
                        self.clean_symlink_target(target_config, options, &mut result)?;
                    }
                    SyncType::ModuleMap => {
                        self.clean_module_map_target(
                            agent_name,
                            target_config,
                            options,
                            &mut result,
                        )?;
                    }
                }
            }
        }

        Ok(result)
    }

    /// Clean a single symlink target.
    fn clean_symlink_target(
        &self,
        target_config: &crate::config::TargetConfig,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        let dest = match self.resolve_destination(&target_config.destination) {
            Ok(d) => d,
            Err(skipped) => {
                result.skipped += 1;
                if options.verbose {
                    println!(
                        "  {} Skipping unsafe destination {}: {}",
                        "!".yellow(),
                        skipped.dest,
                        skipped.error
                    );
                }
                tracing::warn!(
                    destination = %skipped.dest,
                    error = %skipped.error,
                    "Skipping unsafe symlink destination during clean"
                );
                return Ok(());
            }
        };
        let metadata = match self.clean_symlink_metadata(&dest) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                Self::report_clean_metadata_error(&dest, &error, options, result);
                return Ok(());
            }
        };
        if metadata.file_type().is_symlink() {
            self.remove_managed_symlink_quarantined(&dest, options.dry_run, result)?;
        }
        Ok(())
    }

    /// Remove a single managed symlink, emitting a per-path span
    /// (`operation="remove"`, `path`, `outcome`) around the decision.
    fn remove_managed_symlink_quarantined(
        &self,
        dest: &Path,
        dry_run: bool,
        result: &mut SyncResult,
    ) -> Result<()> {
        let span = tracing::info_span!(
            "agentsync",
            operation = "remove",
            path = %dest.display(),
            outcome = tracing::field::Empty
        );
        let _enter = span.enter();
        if dry_run {
            println!("  {} Would remove: {}", "→".cyan(), dest.display());
            span.record("outcome", "would_remove");
        } else {
            let (Some(name), Some(parent_path)) = (dest.file_name(), dest.parent()) else {
                result.errors += 1;
                span.record("outcome", "error");
                tracing::error!(path = %dest.display(), "Managed symlink destination lacks a parent or final component");
                return Ok(());
            };
            let parent = match self.open_project_relative_directory(parent_path) {
                Ok(parent) => parent,
                Err(error) => {
                    result.skipped += 1;
                    span.record("outcome", "skipped");
                    tracing::warn!(error = %error, path = %dest.display(), "Skipping unsafe managed symlink parent");
                    return Ok(());
                }
            };
            let metadata = match parent.symlink_metadata(name) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    result.skipped += 1;
                    span.record("outcome", "skipped");
                    return Ok(());
                }
                Err(error) => {
                    result.errors += 1;
                    span.record("outcome", "error");
                    tracing::error!(error = %error, path = %dest.display(), "Failed to inspect managed symlink before quarantine");
                    return Ok(());
                }
            };
            if !metadata.file_type().is_symlink() {
                span.record("outcome", "unchanged");
                return Ok(());
            }
            let identity = quarantine::EntryIdentity::capture(&metadata);
            match quarantine::remove_symlink_if_unchanged(
                &parent,
                name,
                identity,
                dest,
                || {
                    #[cfg(test)]
                    if let Some(hook) = self.quarantine_before_move_hook.borrow_mut().take() {
                        hook(dest);
                    }
                },
                |_path| {
                    #[cfg(test)]
                    if let Some(hook) = self.quarantine_after_move_hook.borrow_mut().take() {
                        hook(_path);
                    }
                },
            ) {
                Ok(quarantine::RemoveOutcome::Removed) => {
                    self.invalidate_path(dest);
                    println!("  {} Removed: {}", "✔".green(), dest.display());
                    span.record("outcome", "removed");
                }
                Ok(quarantine::RemoveOutcome::Changed) => {
                    result.skipped += 1;
                    span.record("outcome", "skipped");
                    tracing::warn!(path = %dest.display(), "Managed symlink changed before quarantine; preserving it");
                    return Ok(());
                }
                Err(error) => {
                    result.errors += 1;
                    span.record("outcome", "error");
                    tracing::error!(error = %error, path = %dest.display(), "Failed to quarantine managed symlink");
                    return Ok(());
                }
            }
        }
        result.removed += 1;
        Ok(())
    }

    /// Clean symlink-contents: remove symlinks inside the destination directory.
    fn clean_symlink_contents_target(
        &self,
        agent_name: &str,
        target_config: &crate::config::TargetConfig,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        let dest = match self.resolve_destination(&target_config.destination) {
            Ok(d) => d,
            Err(skipped) => {
                result.skipped += 1;
                if options.verbose {
                    println!(
                        "  {} Skipping unsafe destination {}: {}",
                        "!".yellow(),
                        skipped.dest,
                        skipped.error
                    );
                }
                tracing::warn!(
                    destination = %skipped.dest,
                    error = %skipped.error,
                    "Skipping unsafe symlink-contents destination during clean"
                );
                return Ok(());
            }
        };
        let metadata = match self.clean_symlink_metadata(&dest) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                Self::report_clean_metadata_error(&dest, &error, options, result);
                return Ok(());
            }
        };
        if metadata.file_type().is_symlink() {
            if options.verbose {
                println!(
                    "  {} Skipping symlink-contents destination that is a symlink: {}",
                    "!".yellow(),
                    dest.display()
                );
            }
            result.skipped += 1;
            return Ok(());
        }
        if !metadata.is_dir() {
            return Ok(());
        }
        #[cfg(test)]
        if let Some(hook) = self.clean_before_read_contents_hook.borrow().as_ref() {
            hook(&dest);
        }
        let contents = match self.open_contents_directory(&dest) {
            Ok(contents) => contents,
            Err(error) => {
                result.skipped += 1;
                tracing::warn!(error = %error, path = %dest.display(), "Skipping clean target: failed to open destination directory without following links");
                return Ok(());
            }
        };
        let entries = match contents.entries() {
            Ok(entries) => entries,
            Err(error) => {
                result.skipped += 1;
                tracing::warn!(error = %error, path = %dest.display(), "Skipping clean target: failed to read destination directory");
                return Ok(());
            }
        };
        for entry in entries {
            let metadata = match self.clean_contents_symlink_metadata(&contents, &entry) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    Self::report_clean_metadata_error(&entry.path, &error, options, result);
                    continue;
                }
            };
            if metadata.file_type().is_symlink()
                && !enumerate::zcode_contents_child_filtered(agent_name, target_config, &entry.path)
            {
                match self.contents_child_matches_pattern(agent_name, target_config, &entry.path) {
                    Ok(true) => self.remove_managed_contents_symlink(
                        &contents,
                        &entry,
                        quarantine::EntryIdentity::capture(&metadata),
                        options.dry_run,
                        result,
                    ),
                    Ok(false) => {}
                    Err(error) => {
                        result.skipped += 1;
                        if options.verbose {
                            println!(
                                "  {} Skipping child whose source pattern could not be checked: {} ({})",
                                "!".yellow(),
                                entry.path.display(),
                                error
                            );
                        }
                        tracing::warn!(
                            error = %error,
                            path = %entry.path.display(),
                            "Skipping symlink-contents child with unknown source-pattern eligibility"
                        );
                    }
                }
            }
        }
        // Try to remove the directory if empty
        if !options.dry_run {
            let remaining_entries = match contents.entries() {
                Ok(entries) => entries,
                Err(error) => {
                    result.skipped += 1;
                    tracing::warn!(
                        error = %error,
                        path = %dest.display(),
                        "Skipping container cleanup after failing to recheck directory contents"
                    );
                    return Ok(());
                }
            };
            if remaining_entries.is_empty() {
                match contents.remove_empty_directory(
                    || {
                        #[cfg(test)]
                        if let Some(hook) = self
                            .quarantine_before_container_move_hook
                            .borrow_mut()
                            .take()
                        {
                            hook(&dest);
                        }
                    },
                    |_path| {
                        #[cfg(test)]
                        if let Some(hook) = self.quarantine_after_move_hook.borrow_mut().take() {
                            hook(_path);
                        }
                    },
                ) {
                    Ok(quarantine::RemoveDirectoryOutcome::Removed)
                    | Ok(quarantine::RemoveDirectoryOutcome::NotEmpty) => {}
                    Ok(quarantine::RemoveDirectoryOutcome::Changed) => {
                        result.skipped += 1;
                        if options.verbose {
                            println!(
                                "  {} Preserving destination directory changed during cleanup: {}",
                                "!".yellow(),
                                dest.display()
                            );
                        }
                        tracing::warn!(
                            path = %dest.display(),
                            "Destination directory changed before quarantine; preserving it"
                        );
                    }
                    Err(error) => {
                        result.errors += 1;
                        tracing::error!(
                            error = %error,
                            path = %dest.display(),
                            "Failed to quarantine empty symlink-contents destination directory"
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn clean_contents_symlink_metadata(
        &self,
        contents: &enumerate::ContentsDirectory,
        entry: &enumerate::ContentsEntry,
    ) -> std::io::Result<cap_std::fs::Metadata> {
        #[cfg(test)]
        if self.clean_metadata_error_path.borrow().as_deref() == Some(entry.path.as_path()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected metadata inspection failure",
            ));
        }

        contents.symlink_metadata(&entry.name)
    }

    fn remove_managed_contents_symlink(
        &self,
        contents: &enumerate::ContentsDirectory,
        entry: &enumerate::ContentsEntry,
        identity: quarantine::EntryIdentity,
        dry_run: bool,
        result: &mut SyncResult,
    ) {
        let span = tracing::info_span!(
            "agentsync",
            operation = "remove",
            path = %entry.path.display(),
            outcome = tracing::field::Empty
        );
        let _enter = span.enter();
        if dry_run {
            println!("  {} Would remove: {}", "→".cyan(), entry.path.display());
            span.record("outcome", "would_remove");
        } else {
            match contents.remove_symlink_if_unchanged(
                &entry.name,
                identity,
                &entry.path,
                || {
                    #[cfg(test)]
                    if let Some(hook) = self.quarantine_before_move_hook.borrow_mut().take() {
                        hook(&entry.path);
                    }
                },
                |_path| {
                    #[cfg(test)]
                    if let Some(hook) = self.quarantine_after_move_hook.borrow_mut().take() {
                        hook(_path);
                    }
                },
            ) {
                Ok(quarantine::RemoveOutcome::Removed) => {
                    self.invalidate_path(&entry.path);
                    println!("  {} Removed: {}", "✔".green(), entry.path.display());
                    span.record("outcome", "removed");
                }
                Ok(quarantine::RemoveOutcome::Changed) => {
                    result.skipped += 1;
                    span.record("outcome", "skipped");
                    tracing::warn!(path = %entry.path.display(), "Managed child changed before quarantine; preserving it");
                    return;
                }
                Err(error) => {
                    span.record("outcome", "error");
                    result.errors += 1;
                    tracing::error!(error = %error, path = %entry.path.display(), "Failed to quarantine managed symlink child");
                    return;
                }
            }
        }
        result.removed += 1;
    }

    /// Clean nested-glob targets: re-discover matched files and remove symlinks.
    fn clean_nested_glob_target(
        &self,
        target_config: &crate::config::TargetConfig,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        let enumeration = self.enumerate_nested_glob(target_config, options)?;
        if let Err(skipped) = enumeration.template {
            result.skipped += 1;
            if options.verbose {
                println!(
                    "  {} Skipping unsafe nested-glob destination {}: {}",
                    "!".yellow(),
                    skipped.dest,
                    skipped.error
                );
            }
            tracing::warn!(
                destination = %skipped.dest,
                error = %skipped.error,
                "Skipping unsafe nested-glob destination during clean"
            );
            return Ok(());
        }
        let incomplete_discovery = match &enumeration.discovery {
            enumerate::NestedGlobDiscoveryStatus::MissingRoot { search_root } => {
                Some((search_root, "Search root does not exist"))
            }
            enumerate::NestedGlobDiscoveryStatus::Incomplete {
                search_root,
                reason,
            } => Some((search_root, reason.as_str())),
            enumerate::NestedGlobDiscoveryStatus::IncompleteWalk { search_root } => Some((
                search_root,
                "WalkDir traversal encountered one or more entry errors",
            )),
            enumerate::NestedGlobDiscoveryStatus::NotAttempted
            | enumerate::NestedGlobDiscoveryStatus::Complete => None,
        };
        if let Some((search_root, reason)) = incomplete_discovery {
            result.skipped += 1;
            if options.verbose {
                println!(
                    "  {} Skipping incomplete nested-glob discovery under {}: {}",
                    "!".yellow(),
                    search_root.display(),
                    reason
                );
            }
            tracing::warn!(
                path = %search_root.display(),
                reason = %reason,
                "Nested-glob discovery incomplete during clean"
            );
        }

        for item in enumeration.entries {
            let dest = match item.dest {
                Ok(dest) => dest,
                Err(skipped) => {
                    result.skipped += 1;
                    if options.verbose {
                        println!(
                            "  {} Skipping unsafe nested-glob destination {}: {}",
                            "!".yellow(),
                            skipped.dest,
                            skipped.error
                        );
                    }
                    tracing::warn!(
                        destination = %skipped.dest,
                        error = %skipped.error,
                        "Skipping unsafe expanded nested-glob destination during clean"
                    );
                    continue;
                }
            };
            let metadata = match self.clean_symlink_metadata(&dest) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    Self::report_clean_metadata_error(&dest, &error, options, result);
                    continue;
                }
            };
            if metadata.file_type().is_symlink() {
                self.remove_managed_symlink_quarantined(&dest, options.dry_run, result)?;
            }
        }
        Ok(())
    }

    /// Clean module-map targets: remove symlinks for each mapping.
    fn clean_module_map_target(
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
                    result.skipped += 1;
                    if options.verbose {
                        println!("  {} Skipping mapping {}: {}", "!".yellow(), item.source, e);
                    }
                    tracing::warn!(
                        mapping = %item.source,
                        destination = %item.dest_str,
                        error = %e,
                        "Skipping unsafe module-map destination during clean"
                    );
                    continue;
                }
            };

            let metadata = match self.clean_symlink_metadata(&dest) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    Self::report_clean_metadata_error(&dest, &error, options, result);
                    continue;
                }
            };
            if metadata.file_type().is_symlink() {
                self.remove_managed_symlink_quarantined(&dest, options.dry_run, result)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{make_linker, make_target};
    use super::*;
    use tempfile::TempDir;

    #[test]
    #[cfg(unix)]
    fn clean_removes_symlinks_even_for_disabled_agents() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let source = project_root.join(".agents/source.md");
        fs::write(&source, "hi").unwrap();

        let target = make_target("source.md", "dest.md", SyncType::Symlink);
        // The agent is disabled; `clean` deliberately ignores agent/apply filters,
        // so the managed symlink must still be removed.
        let linker = make_linker(project_root, false, target);

        let dest = project_root.join("dest.md");
        symlink(&source, &dest).unwrap();
        assert!(dest.is_symlink());

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 1);
        assert!(!dest.exists());
    }

    #[test]
    fn clean_symlink_target_skips_invalid_destination_without_error() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();

        // An absolute destination fails `ensure_safe_destination`; clean must
        // count the incomplete target without propagating an error.
        let target = make_target("source.md", "/etc/passwd", SyncType::Symlink);
        let linker = make_linker(project_root, true, target);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    #[cfg(unix)]
    fn clean_counts_symlink_target_metadata_error_and_continues() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let bad_source = project_root.join(".agents/bad.md");
        let good_source = project_root.join(".agents/good.md");
        fs::write(&bad_source, "uninspectable source").unwrap();
        fs::write(&good_source, "managed source").unwrap();
        let bad_destination = project_root.join("bad.md");
        let good_destination = project_root.join("good.md");
        symlink(&bad_source, &bad_destination).unwrap();
        symlink(&good_source, &good_destination).unwrap();

        let mut linker = make_linker(
            project_root,
            true,
            make_target("bad.md", "bad.md", SyncType::Symlink),
        );
        let targets = &mut linker.config.agents.get_mut("test").unwrap().targets;
        targets.clear();
        targets.insert(
            "a-uninspectable".to_string(),
            make_target("bad.md", "bad.md", SyncType::Symlink),
        );
        targets.insert(
            "b-good".to_string(),
            make_target("good.md", "good.md", SyncType::Symlink),
        );
        linker.set_clean_metadata_error_path_for_tests(&bad_destination);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(
            result.skipped, 1,
            "the metadata failure is one skipped target"
        );
        assert_eq!(
            result.errors, 0,
            "inspection failures remain conservative skips"
        );
        assert_eq!(result.removed, 1, "the independent managed link is cleaned");
        assert!(
            bad_destination.is_symlink(),
            "the uninspected link is preserved"
        );
        assert!(!good_destination.exists());
    }

    #[test]
    fn clean_symlink_contents_target_leaves_regular_files_untouched() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();

        let dest_dir = project_root.join("dest");
        fs::create_dir_all(&dest_dir).unwrap();
        fs::write(dest_dir.join("regular.md"), "hi").unwrap();

        let target = make_target("source-dir", "dest", SyncType::SymlinkContents);
        let linker = make_linker(project_root, true, target);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
        assert!(dest_dir.join("regular.md").exists());
    }

    #[test]
    fn clean_symlink_contents_target_is_noop_for_missing_destination() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();

        let target = make_target(
            "source-dir",
            "dest-never-created",
            SyncType::SymlinkContents,
        );
        let linker = make_linker(project_root, true, target);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
    }

    #[test]
    fn clean_counts_unresolvable_symlink_contents_destination() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let linker = make_linker(
            project_root,
            true,
            make_target("source-dir", "/outside", SyncType::SymlinkContents),
        );

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.skipped, 1);
        assert_eq!(result.removed, 0);
    }

    #[test]
    #[cfg(unix)]
    fn clean_counts_symlink_contents_metadata_error_and_continues() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let good_source = project_root.join(".agents/good.md");
        fs::write(&good_source, "managed source").unwrap();
        let uninspectable = project_root.join("uninspectable-container");
        fs::create_dir(&uninspectable).unwrap();
        let good_destination = project_root.join("good.md");
        symlink(&good_source, &good_destination).unwrap();

        let mut linker = make_linker(
            project_root,
            true,
            make_target(
                "source-dir",
                "uninspectable-container",
                SyncType::SymlinkContents,
            ),
        );
        let targets = &mut linker.config.agents.get_mut("test").unwrap().targets;
        targets.clear();
        targets.insert(
            "a-uninspectable".to_string(),
            make_target(
                "source-dir",
                "uninspectable-container",
                SyncType::SymlinkContents,
            ),
        );
        targets.insert(
            "b-good".to_string(),
            make_target("good.md", "good.md", SyncType::Symlink),
        );
        linker.set_clean_metadata_error_path_for_tests(&uninspectable);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(
            result.skipped, 1,
            "the metadata failure is one skipped target"
        );
        assert_eq!(
            result.errors, 0,
            "inspection failures remain conservative skips"
        );
        assert_eq!(result.removed, 1, "the independent managed link is cleaned");
        assert!(
            uninspectable.is_dir(),
            "the uninspected container is preserved"
        );
        assert!(!good_destination.exists());
    }

    #[test]
    #[cfg(unix)]
    fn clean_counts_symlink_contents_child_metadata_error_and_continues() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let bad_source = project_root.join(".agents/bad.md");
        let good_source = project_root.join(".agents/good.md");
        fs::write(&bad_source, "uninspectable source").unwrap();
        fs::write(&good_source, "managed source").unwrap();
        let container = project_root.join("managed-contents");
        fs::create_dir(&container).unwrap();
        let bad_child = container.join("bad.md");
        symlink(&bad_source, &bad_child).unwrap();
        let good_destination = project_root.join("good.md");
        symlink(&good_source, &good_destination).unwrap();

        let mut linker = make_linker(
            project_root,
            true,
            make_target("source-dir", "managed-contents", SyncType::SymlinkContents),
        );
        let targets = &mut linker.config.agents.get_mut("test").unwrap().targets;
        targets.clear();
        targets.insert(
            "a-contents".to_string(),
            make_target("source-dir", "managed-contents", SyncType::SymlinkContents),
        );
        targets.insert(
            "b-good".to_string(),
            make_target("good.md", "good.md", SyncType::Symlink),
        );
        linker.set_clean_metadata_error_path_for_tests(&bad_child);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(
            result.skipped, 1,
            "the metadata failure is one skipped child"
        );
        assert_eq!(
            result.errors, 0,
            "inspection failures remain conservative skips"
        );
        assert_eq!(result.removed, 1, "the independent managed link is cleaned");
        assert!(
            bad_child.is_symlink(),
            "the uninspected child link is preserved"
        );
        assert!(!good_destination.exists());
    }

    #[test]
    #[cfg(unix)]
    fn clean_counts_nested_glob_child_metadata_error_and_continues() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let bad_source = project_root.join("source/bad/AGENTS.md");
        fs::create_dir_all(bad_source.parent().unwrap()).unwrap();
        fs::write(&bad_source, "nested source").unwrap();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let good_source = project_root.join(".agents/good.md");
        fs::write(&good_source, "managed source").unwrap();

        let bad_destination = project_root.join("dest/bad/AGENTS.md");
        fs::create_dir_all(bad_destination.parent().unwrap()).unwrap();
        symlink(&bad_source, &bad_destination).unwrap();
        let good_destination = project_root.join("good.md");
        symlink(&good_source, &good_destination).unwrap();

        let mut nested_target = make_target(
            "source",
            "dest/{relative_path}/{file_name}",
            SyncType::NestedGlob,
        );
        nested_target.pattern = Some("**/AGENTS.md".to_string());
        let mut linker = make_linker(project_root, true, nested_target.clone());
        let targets = &mut linker.config.agents.get_mut("test").unwrap().targets;
        targets.clear();
        targets.insert("a-nested".to_string(), nested_target);
        targets.insert(
            "b-good".to_string(),
            make_target("good.md", "good.md", SyncType::Symlink),
        );
        linker.set_clean_metadata_error_path_for_tests(&bad_destination);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(
            result.skipped, 1,
            "the child metadata failure is one skipped target"
        );
        assert_eq!(
            result.errors, 0,
            "inspection failures remain conservative skips"
        );
        assert_eq!(result.removed, 1, "the independent managed link is cleaned");
        assert!(
            bad_destination.is_symlink(),
            "the uninspected link is preserved"
        );
        assert!(!good_destination.exists());
    }

    #[test]
    #[cfg(unix)]
    fn clean_symlink_contents_skips_symlink_destination_container() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let other_dir = project_root.join("other");
        fs::create_dir_all(&other_dir).unwrap();
        let unrelated_source = project_root.join(".agents/unrelated.md");
        fs::write(&unrelated_source, "unrelated source").unwrap();
        let unrelated_child = other_dir.join("unrelated.md");
        symlink(&unrelated_source, &unrelated_child).unwrap();
        let container = project_root.join("managed-container");
        symlink(&other_dir, &container).unwrap();

        let target = make_target("source-dir", "managed-container", SyncType::SymlinkContents);
        let linker = make_linker(project_root, true, target);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
        assert_eq!(result.skipped, 1);
        assert!(
            container.is_symlink(),
            "the destination symlink is preserved"
        );
        assert!(
            unrelated_child.is_symlink(),
            "the unrelated child inside its target directory is preserved"
        );
        assert_eq!(
            fs::read_to_string(&unrelated_source).unwrap(),
            "unrelated source"
        );
    }

    #[test]
    #[cfg(unix)]
    fn clean_continues_after_read_failure_for_replaced_symlink_contents_container() {
        use std::os::unix::fs::symlink;
        use std::{cell::Cell, rc::Rc};

        let temp = TempDir::new().unwrap();
        let project_root = temp.path().join("project");
        let outside = temp.path().join("outside");
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("keep.md"), "outside file").unwrap();
        fs::write(project_root.join(".agents/good.md"), "managed source").unwrap();

        let mut linker = make_linker(
            &project_root,
            true,
            make_target("source-dir", "unsafe-container", SyncType::SymlinkContents),
        );
        let targets = &mut linker.config.agents.get_mut("test").unwrap().targets;
        targets.clear();
        targets.insert(
            "a-unsafe-container".to_string(),
            make_target("source-dir", "unsafe-container", SyncType::SymlinkContents),
        );
        targets.insert(
            "b-good-link".to_string(),
            make_target("good.md", "good.md", SyncType::Symlink),
        );

        let unsafe_container = project_root.join("unsafe-container");
        fs::create_dir(&unsafe_container).unwrap();
        symlink(
            project_root.join(".agents/good.md"),
            project_root.join("good.md"),
        )
        .unwrap();

        let hook_called = Rc::new(Cell::new(false));
        let hook_called_in_hook = Rc::clone(&hook_called);
        let outside_for_hook = outside.clone();
        let unsafe_container_for_hook = unsafe_container.clone();
        linker.set_clean_before_read_contents_hook_for_tests(move |dir| {
            if dir == unsafe_container_for_hook {
                fs::remove_dir(dir).unwrap();
                symlink(&outside_for_hook, dir).unwrap();
                hook_called_in_hook.set(true);
            }
        });

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert!(
            hook_called.get(),
            "container should be replaced after resolution"
        );
        assert_eq!(result.removed, 1);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.errors, 0);
        assert!(!project_root.join("good.md").exists());
        assert!(unsafe_container.is_symlink());
        assert_eq!(
            fs::read_to_string(outside.join("keep.md")).unwrap(),
            "outside file"
        );
    }

    #[test]
    #[cfg(unix)]
    fn clean_does_not_follow_replaced_project_root_after_capability_open() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path().join("project");
        let moved_project = temp.path().join("project-before-replacement");
        let outside_root = temp.path().join("outside");
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        fs::create_dir_all(project_root.join("dest")).unwrap();
        fs::create_dir_all(outside_root.join("dest")).unwrap();
        let outside_source = outside_root.join("attacker-source.md");
        fs::write(&outside_source, "outside source").unwrap();
        let outside_link = outside_root.join("dest/attacker.md");
        symlink(&outside_source, &outside_link).unwrap();

        let linker = make_linker(
            &project_root,
            true,
            make_target("source-dir", "dest", SyncType::SymlinkContents),
        );
        let hook_project = project_root.clone();
        let hook_moved = moved_project.clone();
        let hook_outside = outside_root.clone();
        let expected_canonical_root = project_root.canonicalize().unwrap();
        *linker.project_root_before_open_hook.borrow_mut() =
            Some(Box::new(move |canonical_root| {
                assert_eq!(canonical_root, expected_canonical_root);
                fs::rename(&hook_project, &hook_moved).unwrap();
                symlink(&hook_outside, &hook_project).unwrap();
            }));

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
        assert_eq!(result.skipped, 1);
        assert!(
            outside_link.is_symlink(),
            "replacement root must not be traversed"
        );
    }

    #[test]
    #[cfg(unix)]
    fn clean_rejects_replaced_project_root_directory_after_identity_snapshot() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path().join("project");
        let moved_project = temp.path().join("project-before-replacement");
        let outside_root = temp.path().join("outside");
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        fs::create_dir_all(project_root.join("dest")).unwrap();
        fs::create_dir_all(outside_root.join("dest")).unwrap();
        let outside_source = outside_root.join("attacker-source.md");
        fs::write(&outside_source, "outside source").unwrap();
        let outside_link = outside_root.join("dest/attacker.md");
        symlink(&outside_source, &outside_link).unwrap();

        let linker = make_linker(
            &project_root,
            true,
            make_target("source-dir", "dest", SyncType::SymlinkContents),
        );
        let hook_project = project_root.clone();
        let hook_moved = moved_project.clone();
        let hook_outside = outside_root.clone();
        let expected_canonical_root = project_root.canonicalize().unwrap();
        *linker.project_root_before_open_hook.borrow_mut() =
            Some(Box::new(move |canonical_root| {
                assert_eq!(canonical_root, expected_canonical_root);
                fs::rename(&hook_project, &hook_moved).unwrap();
                fs::rename(&hook_outside, &hook_project).unwrap();
            }));

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
        assert_eq!(result.skipped, 1);
        assert!(
            project_root.join("dest/attacker.md").is_symlink(),
            "a different real directory must not be accepted as the opened project root"
        );
    }

    #[test]
    #[cfg(unix)]
    fn clean_preserves_regular_file_replacing_managed_symlink_before_quarantine() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let source = project_root.join(".agents/source.md");
        fs::write(&source, "managed source").unwrap();
        let destination_dir = project_root.join("dest");
        fs::create_dir(&destination_dir).unwrap();
        let destination = destination_dir.join("source.md");
        symlink(&source, &destination).unwrap();
        let linker = make_linker(
            project_root,
            true,
            make_target("source-dir", "dest", SyncType::SymlinkContents),
        );
        *linker.quarantine_before_move_hook.borrow_mut() = Some(std::rc::Rc::new(move |path| {
            fs::remove_file(path).unwrap();
            fs::write(path, "keep-user-data").unwrap();
        }));

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
        assert_eq!(result.skipped, 1);
        assert_eq!(fs::read_to_string(&destination).unwrap(), "keep-user-data");
    }

    #[test]
    #[cfg(unix)]
    fn clean_preserves_quarantine_when_original_name_is_reoccupied() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let source = project_root.join(".agents/source.md");
        fs::write(&source, "managed source").unwrap();
        let destination_dir = project_root.join("dest");
        fs::create_dir(&destination_dir).unwrap();
        let destination = destination_dir.join("source.md");
        symlink(&source, &destination).unwrap();
        let linker = make_linker(
            project_root,
            true,
            make_target("source-dir", "dest", SyncType::SymlinkContents),
        );
        *linker.quarantine_before_move_hook.borrow_mut() = Some(std::rc::Rc::new(move |path| {
            fs::remove_file(path).unwrap();
            fs::write(path, "quarantined-user-data").unwrap();
        }));
        let destination_for_hook = destination.clone();
        *linker.quarantine_after_move_hook.borrow_mut() = Some(std::rc::Rc::new(move |_| {
            fs::write(&destination_for_hook, "new-name-occupant").unwrap();
        }));

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
        assert_eq!(result.errors, 1);
        assert_eq!(
            fs::read_to_string(&destination).unwrap(),
            "new-name-occupant"
        );
        let quarantined = fs::read_dir(&destination_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name().is_some_and(|name| {
                    name.to_string_lossy().starts_with(".agentsync-quarantine-")
                })
            })
            .expect("failed restoration should preserve the quarantined entry");
        assert_eq!(
            fs::read_to_string(quarantined).unwrap(),
            "quarantined-user-data"
        );
    }

    #[test]
    #[cfg(unix)]
    fn clean_preserves_replaced_empty_container_before_quarantine() {
        use std::rc::Rc;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path().join("project");
        let moved_container = temp.path().join("opened-container");
        fs::create_dir_all(project_root.join(".agents/source-dir")).unwrap();
        let destination = project_root.join("dest");
        fs::create_dir(&destination).unwrap();

        let linker = make_linker(
            &project_root,
            true,
            make_target("source-dir", "dest", SyncType::SymlinkContents),
        );
        let destination_for_hook = destination.clone();
        let moved_for_hook = moved_container.clone();
        *linker.quarantine_before_container_move_hook.borrow_mut() = Some(Rc::new(move |opened| {
            assert_eq!(opened, destination_for_hook);
            fs::rename(&destination_for_hook, &moved_for_hook).unwrap();
            fs::create_dir(&destination_for_hook).unwrap();
        }));

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert!(
            destination.is_dir(),
            "replacement container must be preserved"
        );
        assert_eq!(result.skipped, 1);
        assert_eq!(result.errors, 0);
    }

    #[test]
    #[cfg(unix)]
    fn clean_rejects_internal_symlink_replacement_before_reading_children() {
        use std::os::unix::fs::symlink;
        use std::{cell::Cell, rc::Rc};

        let temp = TempDir::new().unwrap();
        let project_root = temp.path().join("project");
        let unrelated_dir = project_root.join("unrelated");
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        fs::create_dir_all(&unrelated_dir).unwrap();
        let managed_source = project_root.join(".agents/managed.md");
        fs::write(&managed_source, "managed source").unwrap();
        let unrelated_child = unrelated_dir.join("managed.md");
        symlink(&managed_source, &unrelated_child).unwrap();

        let mut linker = make_linker(
            &project_root,
            true,
            make_target("source-dir", "managed-container", SyncType::SymlinkContents),
        );
        let targets = &mut linker.config.agents.get_mut("test").unwrap().targets;
        targets.clear();
        targets.insert(
            "a-container".to_string(),
            make_target("source-dir", "managed-container", SyncType::SymlinkContents),
        );
        targets.insert(
            "b-good-link".to_string(),
            make_target("managed.md", "good.md", SyncType::Symlink),
        );

        let container = project_root.join("managed-container");
        fs::create_dir(&container).unwrap();
        let good_destination = project_root.join("good.md");
        symlink(&managed_source, &good_destination).unwrap();

        let hook_called = Rc::new(Cell::new(false));
        let hook_called_in_hook = Rc::clone(&hook_called);
        let unrelated_dir_for_hook = unrelated_dir.clone();
        let container_for_hook = container.clone();
        linker.set_clean_before_read_contents_hook_for_tests(move |dir| {
            if dir == container_for_hook {
                fs::remove_dir(dir).unwrap();
                symlink(&unrelated_dir_for_hook, dir).unwrap();
                hook_called_in_hook.set(true);
            }
        });

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert!(
            hook_called.get(),
            "the checked container is replaced by the seam"
        );
        assert_eq!(result.skipped, 1, "the replaced container is skipped");
        assert_eq!(result.errors, 0);
        assert_eq!(result.removed, 1, "the independent valid target is cleaned");
        assert!(
            unrelated_child.is_symlink(),
            "the unrelated child is preserved"
        );
        assert!(
            container.is_symlink(),
            "the replacement symlink is preserved"
        );
        assert_eq!(
            fs::read_to_string(&managed_source).unwrap(),
            "managed source"
        );
        assert!(!good_destination.exists());
    }

    #[test]
    #[cfg(unix)]
    fn clean_does_not_quarantine_nonempty_symlink_contents_container() {
        use std::cell::Cell;
        use std::rc::Rc;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path().join("project");
        fs::create_dir_all(project_root.join(".agents/source-dir")).unwrap();
        fs::write(
            project_root.join(".agents/source-dir/managed.md"),
            "managed",
        )
        .unwrap();

        let destination = project_root.join("dest");
        fs::create_dir_all(&destination).unwrap();
        let user_file = destination.join("user.txt");
        fs::write(&user_file, "keep this file").unwrap();

        let linker = make_linker(
            &project_root,
            true,
            make_target("source-dir", "dest", SyncType::SymlinkContents),
        );
        let quarantine_attempted = Rc::new(Cell::new(false));
        let quarantine_attempted_in_hook = Rc::clone(&quarantine_attempted);
        *linker.quarantine_before_container_move_hook.borrow_mut() =
            Some(Rc::new(move |_| quarantine_attempted_in_hook.set(true)));

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
        assert!(
            !quarantine_attempted.get(),
            "a container with user files should not be moved out of its path"
        );
        assert_eq!(fs::read_to_string(user_file).unwrap(), "keep this file");
    }

    #[test]
    #[cfg(unix)]
    fn clean_does_not_unlink_children_from_directory_replaced_after_metadata() {
        use std::{cell::Cell, os::unix::fs::symlink, rc::Rc};

        let temp = TempDir::new().unwrap();
        let project_root = temp.path().join("project");
        let unrelated_dir = project_root.join("unrelated");
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        fs::create_dir_all(&unrelated_dir).unwrap();

        let managed_source = project_root.join(".agents/managed.md");
        fs::write(&managed_source, "managed source").unwrap();
        let unrelated_child = unrelated_dir.join("managed.md");
        symlink(&managed_source, &unrelated_child).unwrap();

        let target = make_target("source-dir", "managed-container", SyncType::SymlinkContents);
        let linker = make_linker(&project_root, true, target);
        let container = project_root.join("managed-container");
        fs::create_dir(&container).unwrap();

        let hook_called = Rc::new(Cell::new(false));
        let hook_called_in_hook = Rc::clone(&hook_called);
        let unrelated_dir_for_hook = unrelated_dir.clone();
        let container_for_hook = container.clone();
        *linker.read_contents_after_metadata_hook.borrow_mut() = Some(Rc::new(move |dir| {
            if dir == container_for_hook {
                fs::remove_dir(dir).unwrap();
                symlink(&unrelated_dir_for_hook, dir).unwrap();
                hook_called_in_hook.set(true);
            }
        }));

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert!(
            hook_called.get(),
            "the container should be replaced after metadata"
        );
        assert_eq!(result.errors, 0);
        assert!(
            unrelated_child.is_symlink(),
            "the unrelated child must be preserved"
        );
        assert!(
            container.is_symlink(),
            "the replacement symlink must be preserved"
        );
        assert_eq!(
            fs::read_to_string(&managed_source).unwrap(),
            "managed source"
        );
    }

    #[test]
    #[cfg(unix)]
    fn clean_symlink_contents_respects_target_pattern() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let source_dir = project_root.join(".agents/source");
        let destination_dir = project_root.join("managed-contents");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir(&destination_dir).unwrap();
        let selected_source = source_dir.join("selected.txt");
        let excluded_source = source_dir.join("excluded.md");
        fs::write(&selected_source, "selected").unwrap();
        fs::write(&excluded_source, "excluded").unwrap();
        let selected_destination = destination_dir.join("selected.txt");
        let excluded_destination = destination_dir.join("excluded.md");
        symlink(&selected_source, &selected_destination).unwrap();
        symlink(&excluded_source, &excluded_destination).unwrap();

        let mut target = make_target("source", "managed-contents", SyncType::SymlinkContents);
        target.pattern = Some("*.txt".to_string());
        let linker = make_linker(project_root, true, target);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 1, "only the matching source is managed");
        assert!(!selected_destination.is_symlink());
        assert!(
            excluded_destination.is_symlink(),
            "pattern-excluded child must remain"
        );
    }

    #[test]
    #[cfg(unix)]
    fn clean_zcode_pattern_does_not_infer_sources_after_source_read_error() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        fs::write(project_root.join(".agents/commands"), "not a directory").unwrap();
        let managed_source = project_root.join(".agents/managed.md");
        fs::write(&managed_source, "managed source").unwrap();
        let destination_dir = project_root.join(".zcode/commands");
        fs::create_dir_all(&destination_dir).unwrap();
        let unrelated_child = destination_dir.join("cmd.md");
        symlink(&managed_source, &unrelated_child).unwrap();

        let mut target = make_target("commands", ".zcode/commands", SyncType::SymlinkContents);
        target.pattern = Some("cmd.agent.md".to_string());
        let mut linker = make_linker(project_root, true, target);
        let agent = linker.config.agents.remove("test").unwrap();
        linker.config.agents.insert("zcode".to_string(), agent);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0, "unknown source scope must fail closed");
        assert_eq!(result.skipped, 1, "unknown source scope must be visible");
        assert!(unrelated_child.is_symlink());
    }

    #[test]
    fn clean_nested_glob_reports_missing_search_root_as_skipped() {
        let temp = TempDir::new().unwrap();
        let target = make_target(
            "missing-source",
            "dest/{relative_path}/{file_name}",
            SyncType::NestedGlob,
        );
        let linker = make_linker(temp.path(), true, target);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn clean_module_map_target_skips_mapping_with_invalid_destination() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();

        let mut target = make_target("unused", "unused", SyncType::ModuleMap);
        target.mappings = vec![crate::config::ModuleMapping {
            source: "shared/context.md".to_string(),
            destination: "/etc".to_string(),
            filename_override: None,
        }];
        let linker = make_linker(project_root, true, target);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
    }

    #[test]
    #[cfg(unix)]
    fn clean_counts_module_map_metadata_error_and_continues() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let bad_source = project_root.join(".agents/bad.md");
        let good_source = project_root.join(".agents/good.md");
        fs::write(&bad_source, "uninspectable source").unwrap();
        fs::write(&good_source, "managed source").unwrap();
        let bad_destination = project_root.join("modules/bad.md");
        fs::create_dir_all(bad_destination.parent().unwrap()).unwrap();
        symlink(&bad_source, &bad_destination).unwrap();
        let good_destination = project_root.join("good.md");
        symlink(&good_source, &good_destination).unwrap();

        let mut module_target = make_target("unused", "unused", SyncType::ModuleMap);
        module_target.mappings = vec![crate::config::ModuleMapping {
            source: "bad.md".to_string(),
            destination: "modules".to_string(),
            filename_override: Some("bad.md".to_string()),
        }];
        let mut linker = make_linker(project_root, true, module_target.clone());
        let targets = &mut linker.config.agents.get_mut("test").unwrap().targets;
        targets.clear();
        targets.insert("a-module-map".to_string(), module_target);
        targets.insert(
            "b-good".to_string(),
            make_target("good.md", "good.md", SyncType::Symlink),
        );
        linker.set_clean_metadata_error_path_for_tests(&bad_destination);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(
            result.skipped, 1,
            "the metadata failure is one skipped mapping"
        );
        assert_eq!(
            result.errors, 0,
            "inspection failures remain conservative skips"
        );
        assert_eq!(result.removed, 1, "the independent managed link is cleaned");
        assert!(
            bad_destination.is_symlink(),
            "the uninspected link is preserved"
        );
        assert!(!good_destination.exists());
    }
}
