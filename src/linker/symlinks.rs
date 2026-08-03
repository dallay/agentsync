//! Symlink creation and mutation operations.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::TargetConfig;

use super::matches_pattern;
use super::{ExistingSymlinkAction, Linker, ResolvedSource, SyncOptions, SyncResult};

impl Linker {
    /// Create a single symlink
    pub(super) fn create_symlink(
        &self,
        source: &ResolvedSource,
        dest: &Path,
        options: &SyncOptions,
    ) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        // Check if source exists
        if !source.exists {
            println!(
                "  {} Source does not exist: {}",
                "!".yellow(),
                source.path.display()
            );
            result.skipped += 1;
            return Ok(result);
        }

        // Create parent directory if needed
        if let Some(parent) = dest.parent() {
            self.ensure_directory(parent, options)?;
        }

        // Calculate relative path from dest to source
        let allow_missing = options.dry_run && !source.path.exists();
        let relative_source = self.relative_path(dest, &source.path, allow_missing)?;

        // Handle existing destination
        if dest.is_symlink() {
            let action = self.handle_existing_symlink(dest, &relative_source, options)?;
            match action {
                ExistingSymlinkAction::AlreadyCorrect => {
                    result.skipped += 1;
                    return Ok(result);
                }
                ExistingSymlinkAction::Updated => {
                    result.updated += 1;
                }
            }
        } else if dest.exists() {
            self.backup_existing_destination(dest, options)?;
            result.updated += 1;
        } else {
            result.created += 1;
        }

        // Create the symlink
        if options.dry_run {
            if result.created > 0 {
                println!(
                    "  {} Would link: {} -> {}",
                    "→".cyan(),
                    dest.display(),
                    relative_source.display()
                );
            }
        } else {
            self.revalidate_path(dest)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&relative_source, dest)
                .with_context(|| format!("Failed to create symlink: {}", dest.display()))?;

            #[cfg(windows)]
            {
                if source.path.is_dir() {
                    std::os::windows::fs::symlink_dir(&relative_source, dest)?;
                } else {
                    std::os::windows::fs::symlink_file(&relative_source, dest)?;
                }
            }

            self.invalidate_path_cache();

            println!(
                "  {} Linked: {} -> {}",
                "✔".green(),
                dest.display(),
                relative_source.display()
            );
        }

        Ok(result)
    }

    /// Handle an existing symlink at the destination. Returns the action taken.
    fn handle_existing_symlink(
        &self,
        dest: &Path,
        relative_source: &Path,
        options: &SyncOptions,
    ) -> Result<ExistingSymlinkAction> {
        let current_target = fs::read_link(dest)?;
        if current_target == relative_source {
            if options.verbose {
                println!("  {} Already linked: {}", "✔".green(), dest.display());
            }
            return Ok(ExistingSymlinkAction::AlreadyCorrect);
        }

        // Wrong target, remove and recreate
        if options.dry_run {
            println!(
                "  {} Would update symlink: {} -> {}",
                "→".cyan(),
                dest.display(),
                relative_source.display()
            );
        } else {
            self.revalidate_unlink_path(dest)?;
            remove_symlink(dest)?;
            self.invalidate_path_cache();
            if options.verbose {
                println!(
                    "  {} Removed old symlink: {} (was -> {})",
                    "○".yellow(),
                    dest.display(),
                    current_target.display()
                );
            }
        }
        Ok(ExistingSymlinkAction::Updated)
    }

    /// Back up an existing regular file/directory at the destination.
    fn backup_existing_destination(&self, dest: &Path, options: &SyncOptions) -> Result<()> {
        if options.dry_run {
            println!(
                "  {} Would backup and replace: {}",
                "→".cyan(),
                dest.display()
            );
        } else {
            let backup = backup_path_for_destination(dest);
            self.revalidate_path(dest)?;
            self.revalidate_path(&backup)?;
            remove_existing_path(&backup)?;
            fs::rename(dest, &backup)?;
            self.invalidate_path_cache();
            self.invalidate_glob_cache();
            println!(
                "  {} Backed up: {} -> {}",
                "!".yellow(),
                dest.display(),
                backup.display()
            );
        }
        Ok(())
    }

    /// Create symlinks for all contents of a directory
    pub(super) fn create_symlinks_for_contents(
        &self,
        source_dir: &Path,
        dest_dir: &Path,
        pattern: Option<&str>,
        target: &TargetConfig,
        options: &SyncOptions,
    ) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        if !source_dir.exists() || !source_dir.is_dir() {
            println!(
                "  {} Source directory does not exist: {}",
                "!".yellow(),
                source_dir.display()
            );
            result.skipped += 1;
            return Ok(result);
        }

        // SECURITY: Detect if dest_dir is a symlink pointing to source_dir BEFORE ensure_directory
        // This prevents creating circular symlinks inside the source directory
        if dest_dir.exists()
            && dest_dir.is_symlink()
            && let Ok(dest_target) = fs::read_link(dest_dir)
        {
            let dest_canonical = if dest_target.is_absolute() {
                dest_target
            } else {
                dest_dir.parent().unwrap_or(dest_dir).join(&dest_target)
            };

            // Canonicalize both paths to compare them
            let source_canonical = fs::canonicalize(source_dir).ok();
            let dest_resolved = fs::canonicalize(&dest_canonical).ok();

            if source_canonical.is_some()
                && dest_resolved.is_some()
                && source_canonical == dest_resolved
            {
                println!(
                    "  {} Destination is a symlink to source: {} -> {}",
                    "!".yellow(),
                    dest_dir.display(),
                    source_dir.display()
                );
                println!(
                    "  {} Skipping to prevent circular symlinks in source directory",
                    "!".yellow()
                );
                result.skipped += 1;
                return Ok(result);
            }
        }

        // Create destination directory if needed
        self.ensure_directory(dest_dir, options)?;

        // Iterate through source directory contents
        for entry in fs::read_dir(source_dir)
            .with_context(|| format!("Failed to read source directory: {}", source_dir.display()))?
        {
            let entry = entry
                .with_context(|| format!("Failed to read entry in: {}", source_dir.display()))?;
            let file_name = entry.file_name();
            let item_name = file_name.to_string_lossy();

            // Apply pattern filter if specified
            if let Some(pat) = pattern
                && !matches_pattern(&item_name, pat)
            {
                continue;
            }

            let source_path = entry.path();
            let dest_path = dest_dir.join(entry.file_name());

            let resolved = self.resolve_source_path(&source_path, target, options)?;
            // SECURITY: Validate each child source entry before creating symlink
            self.revalidate_path(&resolved.path)?;
            let item_result = self.create_symlink(&resolved, &dest_path, options)?;
            result.created += item_result.created;
            result.updated += item_result.updated;
            result.skipped += item_result.skipped;
        }

        Ok(result)
    }
}

fn backup_path_for_destination(dest: &Path) -> PathBuf {
    // Performance: Use OsString::push to avoid string formatting and UTF-8 validation overhead.
    let mut os_string = dest.as_os_str().to_os_string();
    os_string.push(".bak");
    PathBuf::from(os_string)
}

fn remove_existing_path(path: &Path) -> std::io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };

    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        remove_symlink(path)
    } else if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Remove a symlink, handling both file and directory symlinks cross-platform.
/// On Windows, directory symlinks require `fs::remove_dir()` instead of `fs::remove_file()`.
pub(super) fn remove_symlink(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let meta = fs::symlink_metadata(path)?;
        if meta.file_type().is_symlink_dir() {
            return fs::remove_dir(path);
        }
    }
    fs::remove_file(path)
}
