//! Cleanup implementation for managed symlink targets.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

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
            self.remove_managed_symlink(&dest, options.dry_run, result)?;
        }
        Ok(())
    }

    /// Remove a single managed symlink, emitting a per-path span
    /// (`operation="remove"`, `path`, `outcome`) around the decision.
    fn remove_managed_symlink(
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
            if let Err(e) = self.revalidate_unlink_path(dest) {
                span.record("outcome", "error");
                result.errors += 1;
                tracing::error!(error = %e, path = %dest.display(), "Failed to revalidate managed symlink removal");
                return Ok(());
            }
            if let Err(e) = symlinks::remove_symlink(dest) {
                span.record("outcome", "error");
                result.errors += 1;
                tracing::error!(error = %e, path = %dest.display(), "Failed to remove managed symlink");
                return Ok(());
            }
            self.invalidate_path(dest);
            println!("  {} Removed: {}", "✔".green(), dest.display());
            span.record("outcome", "removed");
        }
        result.removed += 1;
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
                self.remove_managed_symlink(&entry_path, options.dry_run, result)?;
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
                self.remove_managed_symlink(&dest, options.dry_run, result)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AgentConfig, Config, TargetConfig};
    use std::collections::BTreeMap;
    use std::path::Path;
    use tempfile::TempDir;

    fn make_target(source: &str, destination: &str, sync_type: SyncType) -> TargetConfig {
        TargetConfig {
            source: source.to_string(),
            destination: destination.to_string(),
            sync_type,
            pattern: None,
            exclude: vec![],
            mappings: vec![],
        }
    }

    fn make_linker(project_root: &Path, agent_enabled: bool, target: TargetConfig) -> Linker {
        let mut targets = BTreeMap::new();
        targets.insert("target".to_string(), target);

        let agent_config = AgentConfig {
            enabled: agent_enabled,
            description: String::new(),
            targets,
        };

        let mut agents = BTreeMap::new();
        agents.insert("test".to_string(), agent_config);

        let config = Config {
            source_dir: ".agents".to_string(),
            compress_agents_md: false,
            default_agents: vec![],
            agents,
            gitignore: Default::default(),
            mcp: Default::default(),
            mcp_servers: Default::default(),
            plugins: Default::default(),
        };

        let config_path = project_root.join("agentsync.toml");
        Linker::new(config, config_path)
    }

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

        // An absolute destination fails `ensure_safe_destination`; clean must skip
        // it silently rather than propagating an error.
        let target = make_target("source.md", "/etc/passwd", SyncType::Symlink);
        let linker = make_linker(project_root, true, target);

        let result = linker.clean(&SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 0);
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
