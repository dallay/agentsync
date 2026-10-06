//! Cleanup implementation for managed symlink targets.

use anyhow::Result;
use colored::Colorize;
#[cfg(test)]
use std::fs;
use std::path::Path;

use crate::config::SyncType;

use super::{Linker, SyncOptions, SyncResult, enumerate, quarantine};

impl Linker {
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
        if dest.is_symlink() {
            self.remove_managed_symlink(&dest, options.dry_run, result)?;
        }
        Ok(())
    }

    /// Remove a single managed symlink, emitting a per-path span
    /// (`operation="remove"`, `path`, `outcome`) around the decision.
    pub(super) fn remove_managed_symlink(
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
            let Some(name) = dest.file_name() else {
                result.errors += 1;
                span.record("outcome", "error");
                tracing::error!(path = %dest.display(), "Managed symlink destination has no final component");
                return Ok(());
            };
            let Some(parent_path) = dest.parent() else {
                result.errors += 1;
                span.record("outcome", "error");
                tracing::error!(path = %dest.display(), "Managed symlink destination has no parent");
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
        if !dest.is_dir() {
            return Ok(());
        }
        let contents = match self.open_contents_directory(&dest) {
            Ok(contents) => contents,
            Err(error) => {
                result.skipped += 1;
                if options.verbose {
                    println!(
                        "  {} Skipping unsafe or unreadable destination directory {}: {}",
                        "!".yellow(),
                        dest.display(),
                        error
                    );
                }
                tracing::warn!(
                    error = %error,
                    path = %dest.display(),
                    "Skipping unsafe or unreadable symlink-contents destination during clean"
                );
                return Ok(());
            }
        };
        let entries = match contents.entries() {
            Ok(entries) => entries,
            Err(error) => {
                result.skipped += 1;
                if options.verbose {
                    println!(
                        "  {} Skipping unsafe or unreadable destination directory {}: {}",
                        "!".yellow(),
                        dest.display(),
                        error
                    );
                }
                tracing::warn!(
                    error = %error,
                    path = %dest.display(),
                    "Skipping unsafe or unreadable symlink-contents destination during clean"
                );
                return Ok(());
            }
        };
        for entry in entries {
            let metadata = match contents.symlink_metadata(&entry.name) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    result.skipped += 1;
                    if options.verbose {
                        println!(
                            "  {} Skipping destination child that could not be inspected: {} ({})",
                            "!".yellow(),
                            entry.path.display(),
                            error
                        );
                    }
                    tracing::warn!(
                        error = %error,
                        path = %entry.path.display(),
                        "Skipping symlink-contents child that could not be inspected during clean"
                    );
                    continue;
                }
            };
            if metadata.file_type().is_symlink()
                && !enumerate::zcode_contents_child_filtered(agent_name, target_config, &entry.path)
            {
                match self.contents_child_matches_pattern(agent_name, target_config, &entry.path) {
                    Ok(true) => {
                        let identity = quarantine::EntryIdentity::capture(&metadata);
                        self.remove_managed_contents_symlink(
                            &contents,
                            &entry,
                            identity,
                            options.dry_run,
                            result,
                        );
                    }
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
        Ok(())
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

        if let enumerate::NestedGlobDiscoveryStatus::Incomplete {
            search_root,
            reason,
        } = &enumeration.discovery
        {
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
            if dest.is_symlink() {
                self.remove_managed_symlink(&dest, options.dry_run, result)?;
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

            if dest.is_symlink() {
                self.remove_managed_symlink(&dest, options.dry_run, result)?;
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
    fn clean_symlink_contents_target_counts_invalid_destination() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let target = make_target("source-dir", "/outside", SyncType::SymlinkContents);
        let linker = make_linker(project_root, true, target);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn clean_reports_missing_nested_glob_root_as_skipped() {
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
    #[cfg(unix)]
    fn clean_symlink_contents_preserves_children_excluded_by_pattern() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let source_dir = project_root.join(".agents/source");
        fs::create_dir(&source_dir).unwrap();
        fs::write(source_dir.join("managed.md"), "managed").unwrap();
        fs::write(source_dir.join("user.txt"), "user").unwrap();

        let dest = project_root.join("dest");
        fs::create_dir(&dest).unwrap();
        symlink(source_dir.join("managed.md"), dest.join("managed.md")).unwrap();
        symlink(source_dir.join("user.txt"), dest.join("user.txt")).unwrap();

        let mut target = make_target("source", "dest", SyncType::SymlinkContents);
        target.pattern = Some("*.md".to_string());
        let linker = make_linker(project_root, true, target);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 1);
        assert!(!dest.join("managed.md").exists());
        assert!(dest.join("user.txt").is_symlink());
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
    #[cfg(unix)]
    fn clean_does_not_remove_children_from_container_replaced_after_open() {
        use std::{cell::Cell, os::unix::fs::symlink, rc::Rc};

        fn run_case(replacement_is_external: bool) {
            let temp = TempDir::new().unwrap();
            let project_root = temp.path().join("project");
            fs::create_dir_all(project_root.join(".agents")).unwrap();
            let source = project_root.join(".agents/managed.md");
            fs::write(&source, "managed source").unwrap();

            let container = project_root.join("managed-container");
            fs::create_dir(&container).unwrap();
            let replacement = if replacement_is_external {
                temp.path().join("replacement")
            } else {
                project_root.join("replacement")
            };
            fs::create_dir(&replacement).unwrap();
            let replacement_child = replacement.join("managed.md");
            symlink(&source, &replacement_child).unwrap();

            let valid_destination = project_root.join("valid.md");
            symlink(&source, &valid_destination).unwrap();
            let mut linker = make_linker(
                &project_root,
                true,
                make_target("source-dir", "managed-container", SyncType::SymlinkContents),
            );
            linker
                .config
                .agents
                .get_mut("test")
                .unwrap()
                .targets
                .insert(
                    "valid".to_string(),
                    make_target("managed.md", "valid.md", SyncType::Symlink),
                );

            let hook_called = Rc::new(Cell::new(false));
            let hook_called_in_hook = Rc::clone(&hook_called);
            let hook_container = container.clone();
            let hook_replacement = replacement.clone();
            let moved_container = project_root.join("moved-container");
            *linker.read_contents_after_open_hook.borrow_mut() = Some(Rc::new(move |path| {
                assert_eq!(path, hook_container);
                fs::rename(path, &moved_container).unwrap();
                symlink(&hook_replacement, path).unwrap();
                hook_called_in_hook.set(true);
            }));

            let result = linker.clean(&SyncOptions::default()).unwrap();

            assert!(hook_called.get(), "container replacement hook must run");
            assert_eq!(result.errors, 0);
            assert_eq!(result.removed, 1, "only the independent target is cleaned");
            assert!(
                replacement_child.is_symlink(),
                "replacement child is preserved"
            );
            assert!(container.is_symlink(), "replacement container is preserved");
            assert!(
                !valid_destination.exists(),
                "the valid target is still cleaned"
            );
        }

        run_case(false);
        run_case(true);
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
            make_target("source", "dest", SyncType::SymlinkContents),
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
    fn clean_symlink_contents_preserves_zcode_source_pattern_filter() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let source_dir = project_root.join(".agents/commands");
        let destination_dir = project_root.join(".zcode/commands");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&destination_dir).unwrap();

        let selected_source = source_dir.join("selected.agent.md");
        let excluded_source = source_dir.join("excluded.md");
        fs::write(&selected_source, "selected command").unwrap();
        fs::write(&excluded_source, "excluded command").unwrap();
        let selected_destination = destination_dir.join("selected.md");
        let excluded_destination = destination_dir.join("excluded.md");
        symlink(&selected_source, &selected_destination).unwrap();
        symlink(&excluded_source, &excluded_destination).unwrap();

        let mut target = make_target("commands", ".zcode/commands", SyncType::SymlinkContents);
        target.pattern = Some("*.agent.md".to_string());
        let mut linker = make_linker(project_root, true, target);
        let test_agent = linker.config.agents.remove("test").unwrap();
        linker.config.agents.insert("zcode".to_string(), test_agent);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 1);
        assert!(!selected_destination.exists());
        assert!(excluded_destination.is_symlink());
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
    fn clean_preserves_replaced_empty_container_before_quarantine() {
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
        *linker.quarantine_before_container_move_hook.borrow_mut() =
            Some(std::rc::Rc::new(move |opened| {
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
    fn clean_reports_unknown_zcode_source_pattern_as_skipped() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        fs::write(project_root.join(".agents/commands"), "not a directory").unwrap();
        let source = project_root.join(".agents/source.agent.md");
        fs::write(&source, "managed source").unwrap();
        let destination = project_root.join(".zcode/commands");
        fs::create_dir_all(&destination).unwrap();
        symlink(&source, destination.join("source.md")).unwrap();

        let mut target = make_target("commands", ".zcode/commands", SyncType::SymlinkContents);
        target.pattern = Some("*.agent.md".to_string());
        let mut linker = make_linker(project_root, true, target);
        let test_agent = linker.config.agents.remove("test").unwrap();
        linker.config.agents.insert("zcode".to_string(), test_agent);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
        assert_eq!(result.skipped, 1);
        assert!(destination.join("source.md").is_symlink());
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
}
