//! Symlink creation and mutation operations.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::TargetConfig;

use super::matches_pattern;
use super::{ExistingSymlinkAction, Linker, ResolvedSource, SyncOptions, SyncResult};

impl Linker {
    /// Creates one symlink from a resolved source path to a destination path.
    ///
    /// Missing sources are skipped. Existing correct symlinks are preserved; incorrect symlinks are
    /// updated, and existing files or directories are backed up before replacement.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let result = linker.create_symlink(&source, &destination, &options)?;
    /// assert_eq!(result.created, 1);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the destination cannot be prepared, the source path cannot be resolved
    /// relative to the destination, or the symlink cannot be created or updated.
    ///
    /// # Arguments
    ///
    /// * `source` - The resolved source entry to link.
    /// * `dest` - The destination path for the symlink.
    /// * `options` - Options controlling dry-run and synchronization behavior.
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

    /// Determines whether an existing symlink already points to the requested source or needs updating.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::path::Path;
    /// # let current_target = Path::new("../source");
    /// # let requested_target = Path::new("../source");
    /// assert_eq!(current_target, requested_target);
    /// ```
    ///
    /// Returns [`ExistingSymlinkAction::AlreadyCorrect`] for a matching target and
    /// [`ExistingSymlinkAction::Updated`] when the target differs. In a dry run,
    /// an incorrect symlink is reported without being removed.
    ///
    /// # Errors
    ///
    /// Returns an error if the existing symlink cannot be read or removed.
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

    /// Backs up an existing destination before it is replaced.
    ///
    /// In dry-run mode, reports the intended backup without modifying the filesystem.
    /// Otherwise, moves the destination to a `.bak` path, replacing any existing backup.
    ///
    /// # Arguments
    ///
    /// * `dest` - The existing file or directory to back up.
    /// * `options` - Synchronization options controlling whether changes are simulated.
    ///
    /// # Errors
    ///
    /// Returns an error if path validation, backup removal, or renaming fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn example(linker: &Linker, destination: &std::path::Path, options: &SyncOptions) {
    /// let _ = linker.backup_existing_destination(destination, options);
    /// # }
    /// ```
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

    /// Creates symlinks for matching entries in a source directory.
    ///
    /// Missing or invalid source directories are skipped. Existing destination
    /// symlinks that resolve to the source directory are also skipped to prevent
    /// circular links.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::path::Path;
    ///
    /// # let linker: Linker = todo!();
    /// # let target: TargetConfig = todo!();
    /// # let options: SyncOptions = todo!();
    /// let result = linker.create_symlinks_for_contents(
    ///     Path::new("source"),
    ///     Path::new("destination"),
    ///     Some("*.toml"),
    ///     &target,
    ///     &options,
    /// )?;
    /// # let _: SyncResult = result;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// `pattern` filters entries by name when provided.
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

/// Constructs the backup path for a destination by appending `.bak`.
///
/// # Examples
///
/// ```
/// use std::path::{Path, PathBuf};
///
/// let destination = Path::new("config");
/// assert_eq!(
///     backup_path_for_destination(destination),
///     PathBuf::from("config.bak")
/// );
/// ```
fn backup_path_for_destination(dest: &Path) -> PathBuf {
    // Performance: Use OsString::push to avoid string formatting and UTF-8 validation overhead.
    let mut os_string = dest.as_os_str().to_os_string();
    os_string.push(".bak");
    PathBuf::from(os_string)
}

/// Removes the path at `path`, including files, directories, and symbolic links.
/// Missing paths are treated as already removed.
///
/// # Examples
///
/// ```
/// use std::fs;
///
/// let path = std::env::temp_dir().join(format!(
///     "remove-existing-path-{}",
///     std::process::id()
/// ));
/// fs::write(&path, b"temporary file").unwrap();
///
/// remove_existing_path(&path).unwrap();
///
/// assert!(!path.exists());
/// ```
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

/// Removes a symbolic link on any supported platform.
///
/// Handles both file and directory symbolic links according to the platform's
/// filesystem requirements.
///
/// # Examples
///
/// ```
/// # use std::fs;
/// # use std::path::Path;
/// # let dir = std::env::temp_dir().join(format!("remove_symlink_{}", std::process::id()));
/// # fs::create_dir_all(&dir)?;
/// # let target = dir.join("target");
/// # let link = dir.join("link");
/// # fs::write(&target, b"content")?;
/// # #[cfg(unix)]
/// # std::os::unix::fs::symlink(&target, &link)?;
/// # #[cfg(windows)]
/// # std::os::windows::fs::symlink_file(&target, &link)?;
/// remove_symlink(Path::new(&link))?;
/// # fs::remove_file(target)?;
/// # fs::remove_dir(dir)?;
/// # Ok::<(), std::io::Error>(())
/// ```
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
