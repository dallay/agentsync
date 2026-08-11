//! Symlink creation and mutation operations.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
#[cfg(windows)]
use std::os::windows::fs::FileTypeExt;
use std::path::{Path, PathBuf};

use crate::config::TargetConfig;

use super::matches_pattern;
use super::timing::SpanKind;
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
        let _span = self.timing_span(SpanKind::LinkCreation);

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

        // Handle existing destination with a SINGLE existence-class probe.
        // `symlink_metadata` does not follow the final component: a broken
        // symlink still lstat-succeeds with `is_symlink() == true`, so it
        // takes the symlink path (never the backup path) — the same semantics
        // the previous `dest.is_symlink()` + `dest.exists()` pair produced,
        // but with one syscall instead of two. `NotFound` means the
        // destination is fresh; any other probe error is propagated instead
        // of silently claiming the destination was created.
        match fs::symlink_metadata(dest) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
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
            }
            Ok(_) => {
                self.backup_existing_destination(dest, options)?;
                result.updated += 1;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                result.created += 1;
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("Failed to probe destination: {}", dest.display()));
            }
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
                    std::os::windows::fs::symlink_dir(&relative_source, dest).with_context(
                        || format!(
                            "Failed to create directory symlink: {}\nEnsure Windows Developer Mode is enabled or run as Administrator.\nSee https://dallay.github.io/agentsync/guides/windows-symlink-setup/ for details.",
                            dest.display()
                        ),
                    )?;
                } else {
                    std::os::windows::fs::symlink_file(&relative_source, dest).with_context(
                        || format!(
                            "Failed to create file symlink: {}\nEnsure Windows Developer Mode is enabled or run as Administrator.\nSee https://dallay.github.io/agentsync/guides/windows-symlink-setup/ for details.",
                            dest.display()
                        ),
                    )?;
                }
            }

            // Scoped invalidation: only the mutated dest's canonical identity
            // can have changed; sibling/ancestor entries stay cached.
            self.invalidate_path(dest);

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
            self.invalidate_path(dest);
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
            // The rename moves `dest` (now absent) to `backup` (now present);
            // both prefixes must be re-canonicalized on next use.
            self.invalidate_path(dest);
            self.invalidate_path(&backup);
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

        // Iterate through source directory contents in deterministic order.
        for entry in sorted_dir_entries(source_dir)? {
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

            // B3 (REQ: Reuse DirEntry Existence): `read_dir` already proved
            // the child dirent exists. For non-symlink children (the common
            // case) `source.exists()` is guaranteed true, so thread it as a
            // hint and drop the duplicate per-child stat. Symlink children
            // must still be probed: `exists()` follows the link, so a
            // broken-symlink child (dirent exists, target missing) keeps
            // reporting "Source does not exist" unchanged.
            let source_exists = match entry.file_type() {
                Ok(file_type) if !file_type.is_symlink() => Some(true),
                _ => None,
            };
            let resolved =
                self.resolve_source_path_with_hint(&source_path, target, options, source_exists)?;
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

/// Collect the entries of `dir`, returning them in deterministic sorted order.
/// Directory iteration order from the OS is unspecified (often hash/creation
/// order on APFS), so sorting by file name makes `symlink-contents` link
/// creation and printed output reproducible across runs and platforms.
fn sorted_dir_entries(dir: &Path) -> anyhow::Result<Vec<fs::DirEntry>> {
    let mut entries: Vec<fs::DirEntry> = fs::read_dir(dir)
        .with_context(|| format!("Failed to read source directory: {}", dir.display()))?
        .collect::<Result<_, _>>()
        .with_context(|| format!("Failed to read entry in: {}", dir.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    Ok(entries)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    // ==========================================================================
    // test helpers
    // ==========================================================================

    /// Build a `Linker` rooted at `project_root` with an empty (unused) config,
    /// mirroring the helper in `paths.rs` tests. `create_symlink` is called
    /// directly with a `ResolvedSource`, so the config contents are irrelevant.
    #[cfg(unix)]
    fn make_linker(project_root: &Path) -> Linker {
        let agents_dir = project_root.join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();
        let config_path = agents_dir.join("agentsync.toml");

        let config = Config {
            source_dir: ".".to_string(),
            compress_agents_md: false,
            default_agents: vec![],
            agents: BTreeMap::new(),
            gitignore: Default::default(),
            mcp: Default::default(),
            mcp_servers: Default::default(),
        };

        Linker::new(config, config_path)
    }

    /// Standard fixture: a real source file and a destination whose parent
    /// (the project root) already exists. Returns the `TempDir` FIRST so it
    /// stays alive (and the tree stays on disk) for the whole test scope.
    #[cfg(unix)]
    fn make_single_link_fixture() -> (TempDir, Linker, PathBuf, PathBuf) {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let linker = make_linker(root);

        let source = root.join(".agents").join("source.md");
        fs::write(&source, "# Source").unwrap();
        let dest = root.join("dest.md");

        (temp, linker, source, dest)
    }

    // ==========================================================================
    // B4 — deterministic directory iteration order (symlink-contents).
    // The source fixture creates entries in NON-alphabetical order (`z`, `a`,
    // `m`), so OS read_dir order (creation order on this filesystem) differs
    // from sorted order (`a`, `m`, `z`). The helper must return sorted names
    // so link creation order — and printed output — is reproducible.
    // ==========================================================================

    #[test]
    #[cfg(unix)]
    fn sorted_dir_entries_returns_sorted_file_names() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        for name in ["z.md", "a.md", "m.md"] {
            let file = root.join(name);
            fs::write(&file, format!("{name}\n")).unwrap();
        }

        let names: Vec<String> = sorted_dir_entries(root)
            .unwrap()
            .into_iter()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            names,
            vec!["a.md".to_string(), "m.md".to_string(), "z.md".to_string()],
            "source directory iteration order must be sorted by file name"
        );
    }

    // ==========================================================================
    // create_symlink — single existence probe (B2, REQ: No Redundant
    // Existence Probe). One `symlink_metadata` lstat must decide the branch;
    // behavior for broken symlinks (symlink path, never backup) is preserved.
    // ==========================================================================

    #[test]
    #[cfg(unix)]
    fn create_symlink_nonexistent_dest_creates() {
        let (_temp, linker, source, dest) = make_single_link_fixture();
        let resolved = ResolvedSource {
            path: source.clone(),
            exists: true,
        };

        let result = linker
            .create_symlink(&resolved, &dest, &SyncOptions::default())
            .unwrap();

        assert_eq!(result.created, 1);
        assert_eq!(result.updated, 0);
        assert_eq!(result.skipped, 0);
        assert!(dest.is_symlink());
        // The link must point at the resolved source.
        let expected = linker.relative_path(&dest, &source, false).unwrap();
        assert_eq!(fs::read_link(&dest).unwrap(), expected);
    }

    #[test]
    #[cfg(unix)]
    fn create_symlink_regular_file_dest_backs_up() {
        let (_temp, linker, source, dest) = make_single_link_fixture();
        fs::write(&dest, "old content").unwrap();
        let resolved = ResolvedSource {
            path: source.clone(),
            exists: true,
        };

        let result = linker
            .create_symlink(&resolved, &dest, &SyncOptions::default())
            .unwrap();

        assert_eq!(result.updated, 1, "regular file must be backed up");
        assert_eq!(result.created, 0);
        // The original content survives as `<dest>.bak`.
        let backup = backup_path_for_destination(&dest);
        assert_eq!(fs::read_to_string(&backup).unwrap(), "old content");
        // The destination is now a symlink to the source.
        assert!(dest.is_symlink());
        let expected = linker.relative_path(&dest, &source, false).unwrap();
        assert_eq!(fs::read_link(&dest).unwrap(), expected);
    }

    #[test]
    #[cfg(unix)]
    fn create_symlink_broken_symlink_no_backup() {
        use std::os::unix::fs::symlink;

        let (_temp, linker, source, dest) = make_single_link_fixture();
        // A symlink whose target does not exist: `exists()` is false but the
        // entry IS a symlink. It must take the symlink path — never the
        // backup path — and be replaced with the correct target.
        symlink("missing-target.md", &dest).unwrap();
        assert!(dest.is_symlink());
        assert!(!dest.exists());
        let resolved = ResolvedSource {
            path: source.clone(),
            exists: true,
        };

        let result = linker
            .create_symlink(&resolved, &dest, &SyncOptions::default())
            .unwrap();

        assert_eq!(result.updated, 1, "broken symlink must be updated");
        assert_eq!(result.created, 0);
        assert!(
            !backup_path_for_destination(&dest).exists(),
            "broken symlink must NOT be backed up"
        );
        assert!(dest.is_symlink());
        let expected = linker.relative_path(&dest, &source, false).unwrap();
        assert_eq!(fs::read_link(&dest).unwrap(), expected);
    }

    #[test]
    #[cfg(unix)]
    fn create_symlink_valid_symlink_already_correct_skips() {
        use std::os::unix::fs::symlink;

        let (_temp, linker, source, dest) = make_single_link_fixture();
        // Pre-link the destination to exactly the target create_symlink computes.
        let expected = linker.relative_path(&dest, &source, false).unwrap();
        symlink(&expected, &dest).unwrap();
        assert!(dest.is_symlink());
        assert!(dest.exists(), "target exists so this is a valid symlink");
        let resolved = ResolvedSource {
            path: source.clone(),
            exists: true,
        };

        let result = linker
            .create_symlink(&resolved, &dest, &SyncOptions::default())
            .unwrap();

        assert_eq!(result.skipped, 1);
        assert_eq!(result.created, 0);
        assert_eq!(result.updated, 0);
        assert_eq!(fs::read_link(&dest).unwrap(), expected, "link unchanged");
    }

    #[test]
    #[cfg(unix)]
    fn create_symlink_valid_symlink_wrong_target_updates() {
        use std::os::unix::fs::symlink;

        let (_temp, linker, source, dest) = make_single_link_fixture();
        // A valid symlink (target exists) pointing at the WRONG target.
        let stale = dest.parent().unwrap().join("stale-target.md");
        fs::write(&stale, "stale").unwrap();
        symlink("stale-target.md", &dest).unwrap();
        let resolved = ResolvedSource {
            path: source.clone(),
            exists: true,
        };

        let result = linker
            .create_symlink(&resolved, &dest, &SyncOptions::default())
            .unwrap();

        assert_eq!(result.updated, 1, "wrong-target symlink must be updated");
        assert_eq!(result.created, 0);
        assert!(
            !backup_path_for_destination(&dest).exists(),
            "symlink must never be backed up"
        );
        let expected = linker.relative_path(&dest, &source, false).unwrap();
        assert_eq!(fs::read_link(&dest).unwrap(), expected);
    }

    #[test]
    #[cfg(unix)]
    fn create_symlink_propagates_probe_error_on_loop_dest() {
        use std::os::unix::fs::symlink;

        let (_temp, linker, source, _) = make_single_link_fixture();
        let root = source.parent().unwrap().parent().unwrap().to_path_buf();

        // `root/loop` is a self-referential symlink, so any path through it
        // (including `root/loop/loop/dest.md`) fails `symlink_metadata` with
        // ELOOP. The single-probe implementation must propagate this error
        // instead of silently claiming the destination was created.
        let loop_link = root.join("loop");
        symlink("loop", &loop_link).unwrap();
        let dest = loop_link.join("loop").join("dest.md");
        let resolved = ResolvedSource {
            path: source,
            exists: true,
        };

        let result = linker.create_symlink(
            &resolved,
            &dest,
            &SyncOptions {
                dry_run: true,
                ..Default::default()
            },
        );

        assert!(
            result.is_err(),
            "unprobeable destination must fail, not report created"
        );
    }

    // ==========================================================================
    // create_symlinks_for_contents — DirEntry existence reuse (B3, REQ: Reuse
    // DirEntry Existence in Symlink-Contents). Characterization: these tests
    // pin the CURRENT behavior and must stay green after the optimization.
    // ==========================================================================

    /// Build a `symlink-contents` target. `create_symlinks_for_contents` takes
    /// source/dest paths directly, so only `sync_type` matters (compression is
    /// off via `make_linker`'s config).
    fn make_contents_target() -> TargetConfig {
        TargetConfig {
            source: "src".to_string(),
            destination: "links".to_string(),
            sync_type: crate::config::SyncType::SymlinkContents,
            pattern: None,
            exclude: vec![],
            mappings: vec![],
        }
    }

    #[test]
    fn contents_regular_children_all_created() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let linker = make_linker(root);

        let source_dir = root.join(".agents").join("src");
        fs::create_dir_all(&source_dir).unwrap();
        for i in 0..4 {
            fs::write(source_dir.join(format!("f{i:04}.md")), "FIXED\n").unwrap();
        }
        let dest_dir = root.join("links");

        let result = linker
            .create_symlinks_for_contents(
                &source_dir,
                &dest_dir,
                None,
                &make_contents_target(),
                &SyncOptions::default(),
            )
            .unwrap();

        assert_eq!(result.created, 4);
        assert_eq!(result.updated, 0);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.errors, 0);
        assert_eq!(fs::read_dir(&dest_dir).unwrap().count(), 4);
    }

    #[test]
    #[cfg(unix)]
    fn contents_broken_symlink_child_still_skipped() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let linker = make_linker(root);

        let source_dir = root.join(".agents").join("src");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("a.md"), "A\n").unwrap();
        fs::write(source_dir.join("b.md"), "B\n").unwrap();
        fs::create_dir(source_dir.join("sub")).unwrap();
        // A broken symlink child: read_dir yields the dirent (it exists) but
        // `exists()` follows the target and returns false. It must keep
        // reporting "Source does not exist" (skipped) — never error the run.
        symlink("missing-target.md", source_dir.join("broken.md")).unwrap();
        assert!(!source_dir.join("broken.md").exists());

        let dest_dir = root.join("links");
        let result = linker
            .create_symlinks_for_contents(
                &source_dir,
                &dest_dir,
                None,
                &make_contents_target(),
                &SyncOptions::default(),
            )
            .unwrap();

        assert_eq!(result.created, 3, "a.md + b.md + sub");
        assert_eq!(result.updated, 0);
        assert_eq!(
            result.skipped, 1,
            "broken symlink child must be skipped, not errored"
        );
        assert_eq!(
            result.errors, 0,
            "broken symlink child must not fail the run"
        );
        assert_eq!(fs::read_dir(&dest_dir).unwrap().count(), 3);
    }

    #[test]
    fn contents_missing_source_dir_skips_identically() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let linker = make_linker(root);

        let missing = root.join(".agents").join("does-not-exist");
        let dest_dir = root.join("links");

        let result = linker
            .create_symlinks_for_contents(
                &missing,
                &dest_dir,
                None,
                &make_contents_target(),
                &SyncOptions::default(),
            )
            .unwrap();

        assert_eq!(result.created, 0);
        assert_eq!(result.skipped, 1, "missing source dir must skip, not error");
        assert_eq!(result.errors, 0);
        assert!(
            !dest_dir.exists(),
            "no destination dir must be created for a missing source"
        );
    }

    // ==========================================================================
    // resolve_source_path_with_hint — B3 hint semantics (RED: these tests
    // drive the new API). The `Some(true)` contract proves the duplicate
    // per-child stat is skipped deterministically, without wall-clock timing.
    // ==========================================================================

    #[test]
    fn resolve_source_path_with_hint_some_true_skips_duplicate_stat() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let linker = make_linker(root);

        let source = root.join(".agents").join("source.md");
        fs::write(&source, "# S").unwrap();
        let target = make_contents_target();

        // A hint of Some(true) asserts existence without probing.
        let resolved = linker
            .resolve_source_path_with_hint(&source, &target, &SyncOptions::default(), Some(true))
            .unwrap();
        assert!(resolved.exists);

        // Deterministic proof the hint skips the stat: delete the file; the
        // hinted call must STILL report exists=true (it trusted the hint
        // instead of re-stat-ing), while the unhinted call re-stats and
        // reports false.
        fs::remove_file(&source).unwrap();

        let hinted = linker
            .resolve_source_path_with_hint(&source, &target, &SyncOptions::default(), Some(true))
            .unwrap();
        assert!(
            hinted.exists,
            "hinted resolution must not re-stat the source"
        );

        let plain = linker
            .resolve_source_path(&source, &target, &SyncOptions::default())
            .unwrap();
        assert!(
            !plain.exists,
            "unhinted resolution must still stat the source"
        );
    }

    #[test]
    fn resolve_source_path_with_hint_none_falls_back_to_stat() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let linker = make_linker(root);

        let source = root.join(".agents").join("source.md");
        fs::write(&source, "# S").unwrap();
        let target = make_contents_target();

        let resolved = linker
            .resolve_source_path_with_hint(&source, &target, &SyncOptions::default(), None)
            .unwrap();
        assert!(resolved.exists);

        fs::remove_file(&source).unwrap();

        let resolved = linker
            .resolve_source_path_with_hint(&source, &target, &SyncOptions::default(), None)
            .unwrap();
        assert!(
            !resolved.exists,
            "None hint must fall back to source.exists()"
        );
    }

    // ==========================================================================
    // backup_path_for_destination
    // ==========================================================================

    #[test]
    fn backup_path_for_destination_appends_bak_suffix() {
        let dest = Path::new("/tmp/project/AGENTS.md");

        assert_eq!(
            backup_path_for_destination(dest),
            PathBuf::from("/tmp/project/AGENTS.md.bak")
        );
    }

    #[test]
    fn backup_path_for_destination_preserves_extensionless_names() {
        let dest = Path::new("/tmp/project/README");

        assert_eq!(
            backup_path_for_destination(dest),
            PathBuf::from("/tmp/project/README.bak")
        );
    }

    #[test]
    fn backup_path_for_destination_is_relative_when_input_is_relative() {
        let dest = Path::new("nested/dir/file.md");

        assert_eq!(
            backup_path_for_destination(dest),
            PathBuf::from("nested/dir/file.md.bak")
        );
    }

    // ==========================================================================
    // remove_existing_path
    // ==========================================================================

    #[test]
    fn remove_existing_path_removes_regular_file() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("file.md");
        fs::write(&file, "hi").unwrap();

        remove_existing_path(&file).unwrap();

        assert!(!file.exists());
    }

    #[test]
    fn remove_existing_path_removes_nonempty_directory() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("dir");
        fs::create_dir_all(dir.join("nested")).unwrap();
        fs::write(dir.join("nested").join("file.md"), "hi").unwrap();

        remove_existing_path(&dir).unwrap();

        assert!(!dir.exists());
    }

    #[test]
    fn remove_existing_path_is_noop_for_missing_path() {
        let temp = TempDir::new().unwrap();
        let missing = temp.path().join("missing.md");

        assert!(remove_existing_path(&missing).is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn remove_existing_path_removes_symlink_without_following_target() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let target = temp.path().join("target.md");
        fs::write(&target, "hi").unwrap();
        let link = temp.path().join("link.md");
        symlink(&target, &link).unwrap();

        remove_existing_path(&link).unwrap();

        assert!(!link.exists());
        // Removing the symlink entry must not delete its target.
        assert!(target.exists());
    }

    // ==========================================================================
    // remove_symlink
    // ==========================================================================

    #[test]
    #[cfg(unix)]
    fn remove_symlink_removes_file_symlink() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let target = temp.path().join("target.md");
        fs::write(&target, "hi").unwrap();
        let link = temp.path().join("link.md");
        symlink(&target, &link).unwrap();

        remove_symlink(&link).unwrap();

        assert!(!link.exists());
        assert!(target.exists());
    }

    #[test]
    #[cfg(unix)]
    fn remove_symlink_errs_for_missing_path() {
        let temp = TempDir::new().unwrap();
        let missing = temp.path().join("missing-link.md");

        assert!(remove_symlink(&missing).is_err());
    }
}
