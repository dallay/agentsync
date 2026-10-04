//! Revert implementation: undo `apply` by removing managed symlinks and
//! restoring `.bak` backups of pre-existing files.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::config::SyncType;

use super::{Linker, SyncOptions, SyncResult, symlinks};

impl Linker {
    /// Revert managed destinations to their pre-apply state.
    pub fn revert(&self, options: &SyncOptions) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        println!("{}", "Reverting managed symlinks...".cyan());

        for (agent_name, agent_config) in &self.config.agents {
            if !super::agent_selected(&self.config, agent_name, agent_config.enabled, options) {
                continue;
            }
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
        let dest = match self.ensure_safe_destination(&target_config.destination) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        self.revert_destination(&dest, options, result)
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
        let dest = match self.ensure_safe_destination(&target_config.destination) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        if !dest.is_dir() {
            return Ok(());
        }
        let is_zcode_commands = crate::agent_ids::canonical_any_agent_id(agent_name)
            == Some("zcode")
            && target_config.destination.ends_with(".zcode/commands");
        for entry in fs::read_dir(&dest)
            .with_context(|| format!("Failed to read destination directory: {}", dest.display()))?
        {
            let entry =
                entry.with_context(|| format!("Failed to read entry in: {}", dest.display()))?;
            let entry_path = entry.path();
            if entry_path.is_symlink() {
                if is_zcode_commands
                    && !entry_path
                        .file_name()
                        .and_then(OsStr::to_str)
                        .is_some_and(|name| name.ends_with(".md"))
                {
                    continue;
                }
                self.revert_destination(&entry_path, options, result)?;
            } else if entry_path
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| name.ends_with(".bak"))
            {
                let twin = entry_path.with_extension("");
                // Never overwrite a real user file that appeared after the
                // backup was taken; restore into an absent twin or over a
                // managed symlink twin.
                if !twin.exists() || twin.is_symlink() {
                    self.revert_destination(&twin, options, result)?;
                }
            }
        }
        // Try to remove the directory if empty
        if !options.dry_run {
            self.revalidate_unlink_path(&dest)?;
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
            self.revert_destination(&dest, options, result)?;
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

            self.revert_destination(&dest, options, result)?;
        }
        Ok(())
    }

    /// Shared per-path revert used by all four sync types.
    pub(super) fn revert_destination(
        &self,
        dest: &Path,
        options: &SyncOptions,
        result: &mut SyncResult,
    ) -> Result<()> {
        if dest.is_symlink() {
            self.remove_managed_symlink(dest, options.dry_run, result)?;
        }
        let backup = symlinks::backup_path_for_destination(dest);
        if backup.exists() {
            self.restore_backup(dest, &backup, options, result)?;
        }
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
        if options.dry_run {
            println!("  {} Would restore: {}", "→".cyan(), dest.display());
            return Ok(());
        }
        if let Err(e) = self.revalidate_unlink_path(dest) {
            result.errors += 1;
            tracing::error!(error = %e, path = %dest.display(), "Failed to revalidate revert destination");
            return Ok(());
        }
        if let Err(e) = self.revalidate_path(backup) {
            result.errors += 1;
            tracing::error!(error = %e, path = %backup.display(), "Failed to revalidate revert backup");
            return Ok(());
        }
        let op = if options.keep_backups {
            copy_backup_contents(backup, dest)
        } else {
            fs::rename(backup, dest).map_err(anyhow::Error::from)
        };
        if let Err(e) = op {
            result.errors += 1;
            tracing::error!(error = %e, path = %dest.display(), "Failed to restore backup");
            return Ok(());
        }
        self.invalidate_path(dest);
        self.invalidate_path(backup);
        self.invalidate_glob_cache();
        println!("  {} Restored: {}", "✔".green(), dest.display());
        result.restored += 1;
        Ok(())
    }
}

/// Copy a backup over `dest` without consuming it (for `--keep-backups`).
fn copy_backup_contents(backup: &Path, dest: &Path) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(backup)?;
    if metadata.is_dir() {
        copy_dir_all(backup, dest)
    } else {
        fs::copy(backup, dest)
            .map(|_| ())
            .map_err(anyhow::Error::from)
    }
}

/// Recursive directory copy (std has none). Skips nothing; caller guarantees
/// both paths are inside the project root via revalidation.
fn copy_dir_all(src: &Path, dst: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AgentConfig, Config, TargetConfig};
    use std::collections::BTreeMap;
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
    fn revert_symlink_contents_removes_children_and_restores_backups() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        fs::create_dir_all(project_root.join(".agents")).unwrap();
        let source_file = project_root.join(".agents/shared.md");
        fs::write(&source_file, "shared").unwrap();

        let dest_dir = project_root.join("dest");
        fs::create_dir_all(&dest_dir).unwrap();
        symlink(&source_file, dest_dir.join("linked.md")).unwrap();
        assert!(dest_dir.join("linked.md").is_symlink());
        fs::write(dest_dir.join("kept.md.bak"), "kept-original").unwrap();

        let target = make_target("unused", "dest", SyncType::SymlinkContents);
        let linker = make_linker(project_root, true, target);

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
        let source_file = project_root.join(".agents/shared-context.md");
        fs::write(&source_file, "ctx").unwrap();
        symlink(&source_file, &dest).unwrap();
        // Backup path computed exactly like production code does.
        let backup = crate::linker::symlinks::backup_path_for_destination(&dest);
        fs::write(&backup, "orig-mapped\n").unwrap();

        let mut target = make_target("unused", "unused", SyncType::ModuleMap);
        target.mappings = vec![mapping];
        let linker = make_linker(project_root, true, target);

        let result = linker.revert(&SyncOptions::default()).unwrap();

        assert!(!dest.is_symlink());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "orig-mapped\n");
        assert!(!backup.exists());
        assert_eq!(result.restored, 1);
    }
}
