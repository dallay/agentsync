//! Cleanup implementation for managed symlink targets.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;

use crate::config::SyncType;

use super::{Linker, SyncOptions, SyncResult, symlinks};

impl Linker {
    /// Removes all symlinks managed by the configuration, regardless of active filters.
    ///
    /// Cleanup processes every configured target and returns a summary of the removals.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let result = linker.clean(&options)?;
    /// println!("Removed {} symlinks", result.removed);
    /// ```
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
                        self.clean_symlink_contents_target(target_config, options, &mut result)?;
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

    /// Removes the managed symlink for a target when its destination is safe.
    ///
    /// In dry-run mode, reports the removal without changing the filesystem. Unsafe
    /// destinations are skipped.
    ///
    /// # Errors
    ///
    /// Returns an error if path revalidation or symlink removal fails.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// linker.clean_symlink_target(&target_config, &options, &mut result)?;
    /// ```
    ///
    /// # Parameters
    ///
    /// * `target_config` - Configuration containing the symlink destination.
    /// * `options` - Synchronization options, including dry-run behavior.
    /// * `result` - Synchronization result updated with the removal count.
    ///
    /// # Returns
    ///
    /// `Ok(())` after processing the target.
    fn clean_symlink_target(
        &self,
        target_config: &crate::config::TargetConfig,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        let dest = match self.ensure_safe_destination(&target_config.destination) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        if dest.is_symlink() {
            if options.dry_run {
                println!("  {} Would remove: {}", "→".cyan(), dest.display());
            } else {
                self.revalidate_unlink_path(&dest)?;
                symlinks::remove_symlink(&dest)?;
                self.invalidate_path_cache();
                println!("  {} Removed: {}", "✔".green(), dest.display());
            }
            result.removed += 1;
        }
        Ok(())
    }

    /// Removes symlinks directly inside the configured destination directory and removes the
    /// destination directory when it becomes empty.
    ///
    /// # Parameters
    ///
    /// * `target_config` — Target configuration containing the destination directory.
    /// * `options` — Synchronization options, including whether to perform a dry run.
    /// * `result` — Synchronization result updated with the number of removed symlinks.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let mut result = SyncResult::default();
    /// linker.clean_symlink_contents_target(&target_config, &options, &mut result)?;
    /// assert!(result.removed >= 0);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    fn clean_symlink_contents_target(
        &self,
        target_config: &crate::config::TargetConfig,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        let dest = match self.ensure_safe_destination(&target_config.destination) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        if !dest.is_dir() {
            return Ok(());
        }
        for entry in fs::read_dir(&dest)
            .with_context(|| format!("Failed to read destination directory: {}", dest.display()))?
        {
            let entry =
                entry.with_context(|| format!("Failed to read entry in: {}", dest.display()))?;
            if entry.path().is_symlink() {
                if options.dry_run {
                    println!("  {} Would remove: {}", "→".cyan(), entry.path().display());
                } else {
                    self.revalidate_unlink_path(&entry.path())?;
                    symlinks::remove_symlink(&entry.path())?;
                    self.invalidate_path_cache();
                    println!("  {} Removed: {}", "✔".green(), entry.path().display());
                }
                result.removed += 1;
            }
        }
        // Try to remove the directory if empty
        if !options.dry_run {
            self.revalidate_unlink_path(&dest)?;
            let _ = fs::remove_dir(&dest);
        }
        Ok(())
    }

    /// Removes symlinks for files currently matching a nested-glob target.
    ///
    /// Matching files are rediscovered using the target's pattern and exclusions. Unsafe or
    /// unavailable paths are skipped, and no files are removed in dry-run mode.
    ///
    /// # Errors
    ///
    /// Returns an error if matching files cannot be discovered or a path cannot be
    /// revalidated or removed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// linker.clean_nested_glob_target(&target_config, &options, &mut result)?;
    /// ```
    fn clean_nested_glob_target(
        &self,
        target_config: &crate::config::TargetConfig,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        if self
            .ensure_safe_destination(&target_config.destination)
            .is_err()
        {
            return Ok(());
        }

        let search_root = self.project_root.join(&target_config.source);
        if self.revalidate_path(&search_root).is_err() {
            return Ok(());
        }
        if !search_root.exists() || !search_root.is_dir() {
            return Ok(());
        }
        let glob_pattern = target_config.pattern.as_deref().unwrap_or("**/AGENTS.md");
        let dest_template = &target_config.destination;
        let excludes = &target_config.exclude;

        let matches =
            self.get_nested_glob_matches(&search_root, glob_pattern, excludes, options)?;

        for (_, rel_path) in matches.iter() {
            let dest_str = Self::expand_destination_template(dest_template, rel_path);
            if dest_str.is_empty() {
                continue;
            }

            let dest = match self.ensure_safe_destination(&dest_str) {
                Ok(dest) => dest,
                Err(_) => continue,
            };
            if dest.is_symlink() {
                if options.dry_run {
                    println!("  {} Would remove: {}", "→".cyan(), dest.display());
                } else {
                    self.revalidate_unlink_path(&dest)?;
                    fs::remove_file(&dest)?;
                    self.invalidate_path_cache();
                    println!("  {} Removed: {}", "✔".green(), dest.display());
                }
                result.removed += 1;
            }
        }
        Ok(())
    }

    /// Removes symlinks described by module mappings for an agent.
    ///
    /// Unsafe destinations are skipped, and dry-run mode reports removals without changing the filesystem.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Invoke through the linker's cleanup operation.
    /// # let _ = ();
    /// ```
    ///
    /// # Arguments
    ///
    /// * `agent_name` - Agent whose module-map filenames are resolved.
    /// * `target_config` - Module mappings whose destination symlinks are cleaned.
    /// * `options` - Controls dry-run and verbose behavior.
    /// * `result` - Accumulates the number of removed symlinks.
    fn clean_module_map_target(
        &self,
        agent_name: &str,
        target_config: &crate::config::TargetConfig,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        for mapping in &target_config.mappings {
            let filename = crate::config::resolve_module_map_filename(mapping, agent_name);

            let dest_str = format!("{}/{}", mapping.destination, filename);
            let dest = match self.ensure_safe_destination(&dest_str) {
                Ok(d) => d,
                Err(e) => {
                    if options.verbose {
                        println!(
                            "  {} Skipping mapping {}: {}",
                            "!".yellow(),
                            mapping.source,
                            e
                        );
                    }
                    continue;
                }
            };

            if dest.is_symlink() {
                if options.dry_run {
                    println!("  {} Would remove: {}", "→".cyan(), dest.display());
                } else {
                    self.revalidate_unlink_path(&dest)?;
                    symlinks::remove_symlink(&dest)?;
                    self.invalidate_path_cache();
                    println!("  {} Removed: {}", "✔".green(), dest.display());
                }
                result.removed += 1;
            }
        }
        Ok(())
    }
}
