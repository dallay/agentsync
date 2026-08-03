//! Cleanup implementation for managed symlink targets.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;

use crate::config::SyncType;

use super::{Linker, SyncOptions, SyncResult, symlinks};

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

    /// Clean a single symlink target.
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

    /// Clean symlink-contents: remove symlinks inside the destination directory.
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
            let entry_path = entry.path();
            if entry_path.is_symlink() {
                if options.dry_run {
                    println!("  {} Would remove: {}", "→".cyan(), entry_path.display());
                } else {
                    self.revalidate_unlink_path(&entry_path)?;
                    symlinks::remove_symlink(&entry_path)?;
                    self.invalidate_path_cache();
                    println!("  {} Removed: {}", "✔".green(), entry_path.display());
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

    /// Clean nested-glob targets: re-discover matched files and remove symlinks.
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
                    symlinks::remove_symlink(&dest)?;
                    self.invalidate_path_cache();
                    println!("  {} Removed: {}", "✔".green(), dest.display());
                }
                result.removed += 1;
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
        for mapping in &target_config.mappings {
            let dest_str = super::apply::module_map_destination(mapping, agent_name);
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
