//! Gitignore management
//!
//! Handles automatic updates to .gitignore to exclude
//! generated symlinks from version control.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Build the start/end marker pair used to delimit the managed section in `.gitignore`.
pub fn managed_markers(marker: &str) -> (String, String) {
    (format!("# START {}", marker), format!("# END {}", marker))
}

/// Update .gitignore or .git/info/exclude with managed entries
pub fn update_gitignore(
    project_root: &Path,
    marker: &str,
    entries: &[String],
    dry_run: bool,
    local: bool,
) -> Result<()> {
    // SECURITY: Reject control characters that would break the line-oriented
    // managed section. A `destination` like "a\nEVIL" comes from operator
    // config but is written verbatim; without this guard it injects extra
    // gitignore rules. Enforced here at the last trusted decision point.
    if marker.contains(['\n', '\r']) {
        anyhow::bail!("gitignore marker must not contain newline: {:?}", marker);
    }
    if let Some(evil) = entries.iter().find(|e| e.contains(['\n', '\r'])) {
        anyhow::bail!("gitignore entry must not contain newline: {:?}", evil);
    }

    // Determine target path based on local mode
    let (target_path, display_name, opposite_path) = if local {
        // Verify Git repository exists before using local mode
        let git_dir = project_root.join(".git");
        if !git_dir.exists() {
            anyhow::bail!(
                "Cannot use local gitignore mode: no Git repository found at {}",
                project_root.display()
            );
        }

        // Resolve Git common directory for worktrees and submodules
        let git_common_dir = if git_dir.is_file() {
            // .git file in worktrees/submodules points to actual git dir
            let content = fs::read_to_string(&git_dir)
                .with_context(|| format!("Failed to read .git file: {}", git_dir.display()))?;

            if let Some(line) = content.lines().next() {
                if let Some(path_str) = line.strip_prefix("gitdir: ") {
                    let gitdir_path = if Path::new(path_str).is_absolute() {
                        Path::new(path_str).to_path_buf()
                    } else {
                        project_root.join(path_str)
                    };

                    // For worktrees, navigate to main .git directory
                    if gitdir_path
                        .components()
                        .any(|c| c.as_os_str() == "worktrees")
                    {
                        if let Some(parent) = gitdir_path.parent() {
                            if let Some(grandparent) = parent.parent() {
                                grandparent.to_path_buf()
                            } else {
                                git_dir
                            }
                        } else {
                            git_dir
                        }
                    } else {
                        gitdir_path
                    }
                } else {
                    git_dir
                }
            } else {
                git_dir
            }
        } else {
            git_dir
        };

        let git_info_exclude = git_common_dir.join("info").join("exclude");
        let opposite = project_root.join(".gitignore");
        (git_info_exclude, ".git/info/exclude", Some(opposite))
    } else {
        let gitignore = project_root.join(".gitignore");
        let opposite = project_root.join(".git").join("info").join("exclude");
        (gitignore, ".gitignore", Some(opposite))
    };

    let existing_permissions = reject_gitignore_symlink(&target_path)?;
    let (start_marker, end_marker) = managed_markers(marker);

    // Read existing content or start fresh
    let existing_content = if target_path.exists() {
        fs::read_to_string(&target_path).with_context(|| {
            format!("Failed to read {}: {}", display_name, target_path.display())
        })?
    } else {
        String::new()
    };

    // Remove existing managed section if present
    let content_without_managed =
        remove_managed_section(&existing_content, &start_marker, &end_marker);

    // Build new managed section
    let mut managed_section = String::new();
    managed_section.push('\n');
    managed_section.push_str(&start_marker);
    managed_section.push('\n');
    for entry in entries {
        managed_section.push_str(entry);
        managed_section.push('\n');
    }
    managed_section.push_str(&end_marker);
    managed_section.push('\n');

    // Combine content
    let new_content = format!("{}{}", content_without_managed.trim_end(), managed_section);

    if dry_run {
        let mode_suffix = if local { " (local mode)" } else { "" };
        println!(
            "  {} Would update {} with {} entries{}",
            "→".cyan(),
            display_name,
            entries.len(),
            mode_suffix
        );
        return Ok(());
    }

    // Optimization: skip write if content is unchanged to avoid unnecessary I/O
    if existing_content == new_content {
        let mode_suffix = if local { " (local mode)" } else { "" };
        println!(
            "  {} {} is already up to date ({} entries){}",
            "✔".green(),
            display_name,
            entries.len(),
            mode_suffix
        );
        return Ok(());
    }

    // Ensure .git/info directory exists in local mode
    if local && let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    write_gitignore_atomically(&target_path, &new_content, existing_permissions)?;

    let mode_suffix = if local { " (local mode)" } else { "" };
    println!(
        "  {} Updated {} with {} managed entries{}",
        "✔".green(),
        display_name,
        entries.len(),
        mode_suffix
    );

    // Clean up the opposite file if it has managed entries (mode switch)
    if let Some(opposite) = opposite_path
        && opposite.exists()
    {
        let opposite_permissions = reject_gitignore_symlink(&opposite)?;
        let opposite_content = fs::read_to_string(&opposite).ok().unwrap_or_default();
        let cleaned = remove_managed_section(&opposite_content, &start_marker, &end_marker);

        if opposite_content != cleaned && !cleaned.trim().is_empty() {
            // Only write if we actually removed something
            if !dry_run {
                write_gitignore_atomically(&opposite, &cleaned, opposite_permissions)?;
                let opposite_name = if local {
                    ".gitignore"
                } else {
                    ".git/info/exclude"
                };
                println!(
                    "  {} Cleaned managed entries from {}",
                    "✔".green(),
                    opposite_name
                );
            }
        }
    }

    Ok(())
}

/// Remove the managed section from .gitignore or .git/info/exclude when management is disabled.
/// Also cleans the opposite file if it contains managed entries (handles mode switches).
pub fn cleanup_gitignore(
    project_root: &Path,
    marker: &str,
    dry_run: bool,
    local: bool,
) -> Result<()> {
    // SECURITY: Same newline guard as update_gitignore so a malicious marker
    // cannot split the marker line and orphan managed content.
    if marker.contains(['\n', '\r']) {
        anyhow::bail!("gitignore marker must not contain newline: {:?}", marker);
    }

    // Determine target path based on local mode
    let (target_path, display_name) = if local {
        let git_info_exclude = project_root.join(".git").join("info").join("exclude");
        (git_info_exclude, ".git/info/exclude")
    } else {
        (project_root.join(".gitignore"), ".gitignore")
    };

    let opposite_path = if local {
        project_root.join(".gitignore")
    } else {
        project_root.join(".git").join("info").join("exclude")
    };

    let (start_marker, end_marker) = managed_markers(marker);

    // Clean the primary target file
    if target_path.exists() {
        let existing_permissions = reject_gitignore_symlink(&target_path)?;
        let existing_content = fs::read_to_string(&target_path).with_context(|| {
            format!("Failed to read {}: {}", display_name, target_path.display())
        })?;
        let cleaned_content = remove_managed_section(&existing_content, &start_marker, &end_marker);

        if existing_content != cleaned_content {
            if dry_run {
                let mode_suffix = if local { " (local mode)" } else { "" };
                println!(
                    "  {} Would remove managed {} section{}",
                    "→".cyan(),
                    display_name,
                    mode_suffix
                );
            } else {
                write_gitignore_atomically(&target_path, &cleaned_content, existing_permissions)?;
                let mode_suffix = if local { " (local mode)" } else { "" };
                println!(
                    "  {} Removed managed {} section{}",
                    "✔".green(),
                    display_name,
                    mode_suffix
                );
            }
        }
    }

    // Also clean the opposite file if it contains managed entries (handles mode switches)
    if opposite_path.exists() {
        let opposite_permissions = reject_gitignore_symlink(&opposite_path)?;
        let opposite_content = fs::read_to_string(&opposite_path).with_context(|| {
            format!("Failed to read opposite file: {}", opposite_path.display())
        })?;
        let cleaned_opposite =
            remove_managed_section(&opposite_content, &start_marker, &end_marker);

        if opposite_content != cleaned_opposite {
            let opposite_name = if local {
                ".gitignore"
            } else {
                ".git/info/exclude"
            };
            if dry_run {
                println!(
                    "  {} Would remove orphaned managed entries from {}",
                    "→".cyan(),
                    opposite_name
                );
            } else {
                write_gitignore_atomically(
                    &opposite_path,
                    &cleaned_opposite,
                    opposite_permissions,
                )?;
                println!(
                    "  {} Removed orphaned managed entries from {}",
                    "✔".green(),
                    opposite_name
                );
            }
        }
    }

    Ok(())
}

fn reject_gitignore_symlink(gitignore_path: &Path) -> Result<Option<fs::Permissions>> {
    match fs::symlink_metadata(gitignore_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!(
                "Refusing to read or write symlinked .gitignore: {}",
                gitignore_path.display()
            );
        }
        Ok(metadata) if metadata.file_type().is_file() => Ok(Some(metadata.permissions())),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error)
            .with_context(|| format!("Failed to inspect .gitignore: {}", gitignore_path.display())),
    }
}

fn write_gitignore_atomically(
    gitignore_path: &Path,
    content: &str,
    existing_permissions: Option<fs::Permissions>,
) -> Result<()> {
    write_gitignore_atomically_with_hook(gitignore_path, content, existing_permissions, || Ok(()))
}

fn write_gitignore_atomically_with_hook<F>(
    gitignore_path: &Path,
    content: &str,
    existing_permissions: Option<fs::Permissions>,
    before_persist: F,
) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    let parent = gitignore_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut builder = tempfile::Builder::new();

    #[cfg(unix)]
    if existing_permissions.is_none() {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o666));
    }

    let mut temporary_file = builder.tempfile_in(parent).with_context(|| {
        format!(
            "Failed to create temporary .gitignore in {}",
            parent.display()
        )
    })?;
    temporary_file
        .write_all(content.as_bytes())
        .with_context(|| {
            format!(
                "Failed to write temporary .gitignore for {}",
                gitignore_path.display()
            )
        })?;
    if let Some(permissions) = existing_permissions {
        preserve_native_owner_group(gitignore_path, temporary_file.path())?;
        temporary_file
            .as_file()
            .set_permissions(permissions)
            .with_context(|| {
                format!(
                    "Failed to preserve .gitignore permissions for {}",
                    gitignore_path.display()
                )
            })?;
        preserve_native_acl(gitignore_path, temporary_file.path())?;
    }
    temporary_file.as_file().sync_all().with_context(|| {
        format!(
            "Failed to sync temporary .gitignore for {}",
            gitignore_path.display()
        )
    })?;

    before_persist()?;

    temporary_file.persist(gitignore_path).with_context(|| {
        format!(
            "Failed to atomically replace .gitignore: {}",
            gitignore_path.display()
        )
    })?;
    Ok(())
}

#[cfg(unix)]
pub(crate) fn preserve_native_owner_group(source: &Path, destination: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::symlink_metadata(source).with_context(|| {
        format!(
            "Failed to inspect .gitignore ownership: {}",
            source.display()
        )
    })?;
    if !metadata.file_type().is_file() {
        anyhow::bail!(
            "Refusing to preserve metadata from non-regular .gitignore: {}",
            source.display()
        );
    }
    let staged = fs::symlink_metadata(destination).with_context(|| {
        format!(
            "Failed to inspect staged .gitignore ownership: {}",
            destination.display()
        )
    })?;
    if !staged.file_type().is_file() {
        anyhow::bail!(
            "Refusing to preserve ownership onto non-regular staged .gitignore: {}",
            destination.display()
        );
    }
    if !owner_group_change_needed(metadata.uid(), metadata.gid(), staged.uid(), staged.gid()) {
        return Ok(());
    }

    // If ownership differs and the process cannot preserve it, fail closed.
    // An in-place fallback would give up atomic replacement and mutate every
    // hard link to the original file.
    std::os::unix::fs::chown(destination, Some(metadata.uid()), Some(metadata.gid()))
        .with_context(|| {
            format!(
                "Failed to preserve .gitignore owner/group on staged file: {}",
                destination.display()
            )
        })?;
    let staged = fs::metadata(destination).with_context(|| {
        format!(
            "Failed to verify staged .gitignore ownership: {}",
            destination.display()
        )
    })?;
    anyhow::ensure!(
        staged.uid() == metadata.uid() && staged.gid() == metadata.gid(),
        "Staged .gitignore owner/group does not match the original: {}",
        destination.display()
    );
    Ok(())
}

#[cfg(unix)]
fn owner_group_change_needed(
    source_uid: u32,
    source_gid: u32,
    destination_uid: u32,
    destination_gid: u32,
) -> bool {
    source_uid != destination_uid || source_gid != destination_gid
}

#[cfg(windows)]
pub(crate) fn preserve_native_owner_group(source: &Path, destination: &Path) -> Result<()> {
    crate::mcp::preserve_windows_file_owner_sid(source, destination).with_context(|| {
        format!(
            "Failed to preserve .gitignore owner SID from {} onto {}",
            source.display(),
            destination.display()
        )
    })
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn preserve_native_owner_group(_source: &Path, _destination: &Path) -> Result<()> {
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn preserve_native_acl(source: &Path, destination: &Path) -> Result<()> {
    const POSIX_ACCESS_ACL: &str = "system.posix_acl_access";

    let expected_acl = match xattr::get(source, POSIX_ACCESS_ACL) {
        Ok(acl) => acl,
        Err(error) if is_posix_acl_capability_error(&error) => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!("Failed to read .gitignore POSIX ACL: {}", source.display())
            });
        }
    };
    match &expected_acl {
        Some(acl) => xattr::set(destination, POSIX_ACCESS_ACL, acl).with_context(|| {
            format!(
                "Failed to preserve .gitignore POSIX ACL on staged file: {}",
                destination.display()
            )
        })?,
        None => match xattr::remove(destination, POSIX_ACCESS_ACL) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) if is_posix_acl_capability_error(&error) => return Ok(()),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to remove inherited POSIX ACL from staged .gitignore: {}",
                        destination.display()
                    )
                });
            }
        },
    }
    let actual_acl = match xattr::get(destination, POSIX_ACCESS_ACL) {
        Ok(acl) => acl,
        Err(error) if expected_acl.is_none() && is_posix_acl_capability_error(&error) => {
            return Ok(());
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "Failed to verify staged .gitignore POSIX ACL: {}",
                    destination.display()
                )
            });
        }
    };
    anyhow::ensure!(
        actual_acl == expected_acl,
        "Staged .gitignore POSIX ACL does not match the original: {}",
        destination.display()
    );
    Ok(())
}

#[cfg(target_os = "linux")]
fn is_posix_acl_capability_error(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::Unsupported
        || error.raw_os_error().is_some_and(|code| {
            code == rustix::io::Errno::NOTSUP.raw_os_error()
                || code == rustix::io::Errno::OPNOTSUPP.raw_os_error()
        })
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
pub(crate) fn preserve_native_acl(source: &Path, destination: &Path) -> Result<()> {
    let expected_acl = exacl::getfacl(source, None)
        .with_context(|| format!("Failed to read .gitignore ACL: {}", source.display()))?;
    exacl::setfacl(&[destination], &expected_acl, None).with_context(|| {
        format!(
            "Failed to preserve .gitignore ACL on staged file: {}",
            destination.display()
        )
    })?;
    let actual_acl = exacl::getfacl(destination, None).with_context(|| {
        format!(
            "Failed to verify staged .gitignore ACL: {}",
            destination.display()
        )
    })?;
    anyhow::ensure!(
        actual_acl == expected_acl,
        "Staged .gitignore ACL does not match the original: {}",
        destination.display()
    );
    Ok(())
}

#[cfg(windows)]
pub(crate) fn preserve_native_acl(source: &Path, destination: &Path) -> Result<()> {
    crate::mcp::preserve_windows_file_dacl(source, destination)
        .with_context(|| format!("Failed to preserve .gitignore DACL: {}", source.display()))
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    windows
)))]
pub(crate) fn preserve_native_acl(source: &Path, _destination: &Path) -> Result<()> {
    anyhow::bail!(
        "Native ACL preservation is unsupported for this target; refusing to replace .gitignore: {}",
        source.display()
    )
}

/// Remove the managed section from gitignore content
fn remove_managed_section(content: &str, start_marker: &str, end_marker: &str) -> String {
    let mut result = String::new();
    let mut in_managed_section = false;

    for line in content.lines() {
        if line.trim() == start_marker {
            in_managed_section = true;
            continue;
        }
        if line.trim() == end_marker {
            in_managed_section = false;
            continue;
        }
        if !in_managed_section {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    #[cfg(unix)]
    fn owner_group_change_is_skipped_when_staged_metadata_matches() {
        assert!(!owner_group_change_needed(1000, 100, 1000, 100));
        assert!(owner_group_change_needed(1000, 100, 1001, 100));
        assert!(owner_group_change_needed(1000, 100, 1000, 101));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn posix_acl_capability_errors_are_distinguished_from_other_errors() {
        let unsupported =
            std::io::Error::from_raw_os_error(rustix::io::Errno::NOTSUP.raw_os_error());
        let invalid = std::io::Error::from_raw_os_error(22);

        assert!(is_posix_acl_capability_error(&unsupported));
        assert!(!is_posix_acl_capability_error(&invalid));
    }

    // ==========================================================================
    // REMOVE MANAGED SECTION TESTS
    // ==========================================================================

    #[test]
    fn test_remove_managed_section() {
        let content = r#"node_modules/
*.log

# START Test Marker
AGENTS.md
CLAUDE.md
# END Test Marker

dist/
"#;

        let result = remove_managed_section(content, "# START Test Marker", "# END Test Marker");

        assert!(result.contains("node_modules/"));
        assert!(result.contains("dist/"));
        assert!(!result.contains("AGENTS.md"));
        assert!(!result.contains("CLAUDE.md"));
        assert!(!result.contains("Test Marker"));
    }

    #[test]
    fn test_remove_managed_section_not_present() {
        let content = "node_modules/\n*.log\n";
        let result = remove_managed_section(content, "# START Marker", "# END Marker");
        assert_eq!(result, content);
    }

    #[test]
    fn test_remove_managed_section_at_start() {
        let content = r#"# START Marker
managed1
managed2
# END Marker
other_content
"#;

        let result = remove_managed_section(content, "# START Marker", "# END Marker");

        assert!(!result.contains("managed1"));
        assert!(!result.contains("managed2"));
        assert!(result.contains("other_content"));
    }

    #[test]
    fn test_remove_managed_section_at_end() {
        let content = r#"other_content
# START Marker
managed1
managed2
# END Marker
"#;

        let result = remove_managed_section(content, "# START Marker", "# END Marker");

        assert!(result.contains("other_content"));
        assert!(!result.contains("managed1"));
        assert!(!result.contains("managed2"));
    }

    #[test]
    fn test_remove_managed_section_empty_managed() {
        let content = r#"before
# START Marker
# END Marker
after
"#;

        let result = remove_managed_section(content, "# START Marker", "# END Marker");

        assert!(result.contains("before"));
        assert!(result.contains("after"));
        assert!(!result.contains("START Marker"));
        assert!(!result.contains("END Marker"));
    }

    #[test]
    fn test_remove_managed_section_preserves_whitespace() {
        let content = "line1\n\n\nline2\n";
        let result = remove_managed_section(content, "# START", "# END");

        // Should preserve the original content including blank lines
        assert!(result.contains("line1"));
        assert!(result.contains("line2"));
    }

    // ==========================================================================
    // UPDATE GITIGNORE TESTS
    // ==========================================================================

    #[test]
    fn test_update_gitignore_creates_new_file() {
        let temp_dir = TempDir::new().unwrap();

        let entries = vec!["CLAUDE.md".to_string(), "AGENTS.md".to_string()];
        update_gitignore(temp_dir.path(), "AI Agent Symlinks", &entries, false, false).unwrap();

        let gitignore_path = temp_dir.path().join(".gitignore");
        assert!(gitignore_path.exists());

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains("# START AI Agent Symlinks"));
        assert!(content.contains("# END AI Agent Symlinks"));
        assert!(content.contains("CLAUDE.md"));
        assert!(content.contains("AGENTS.md"));
    }

    #[test]
    fn test_update_gitignore_appends_to_existing() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");

        // Create existing gitignore
        fs::write(&gitignore_path, "node_modules/\n*.log\n").unwrap();

        let entries = vec!["CLAUDE.md".to_string()];
        update_gitignore(temp_dir.path(), "AI Agent Symlinks", &entries, false, false).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();

        // Original content preserved
        assert!(content.contains("node_modules/"));
        assert!(content.contains("*.log"));

        // New content added
        assert!(content.contains("# START AI Agent Symlinks"));
        assert!(content.contains("CLAUDE.md"));
        assert!(content.contains("# END AI Agent Symlinks"));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn update_gitignore_preserves_existing_posix_acl() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new_in(".").unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        fs::write(&gitignore_path, "existing-rule\n").unwrap();
        fs::set_permissions(&gitignore_path, fs::Permissions::from_mode(0o640)).unwrap();
        let mut posix_acl = 2_u32.to_le_bytes().to_vec();
        for (tag, permissions, id) in [
            (0x01_u16, 0x06_u16, u32::MAX), // ACL_USER_OBJ
            (0x02, 0x04, 123_456),          // ACL_USER
            (0x04, 0x04, u32::MAX),         // ACL_GROUP_OBJ
            (0x10, 0x04, u32::MAX),         // ACL_MASK
            (0x20, 0x00, u32::MAX),         // ACL_OTHER
        ] {
            posix_acl.extend_from_slice(&tag.to_le_bytes());
            posix_acl.extend_from_slice(&permissions.to_le_bytes());
            posix_acl.extend_from_slice(&id.to_le_bytes());
        }
        if let Err(error) = xattr::set(&gitignore_path, "system.posix_acl_access", &posix_acl) {
            if matches!(
                error.kind(),
                std::io::ErrorKind::Unsupported | std::io::ErrorKind::InvalidInput
            ) || is_posix_acl_capability_error(&error)
            {
                eprintln!("Skipping POSIX ACL test: filesystem does not support ACLs");
                return;
            }
            panic!("failed to establish test ACL: {error}");
        }
        let original_acl = xattr::get(&gitignore_path, "system.posix_acl_access")
            .unwrap()
            .expect("test ACL must be present");

        update_gitignore(
            temp_dir.path(),
            "AgentSync",
            &["generated.md".to_string()],
            false,
            false,
        )
        .unwrap();

        assert_eq!(
            xattr::get(&gitignore_path, "system.posix_acl_access")
                .unwrap()
                .expect("the ACL must remain present"),
            original_acl,
            "atomic replacement must preserve the original extended ACL"
        );
    }

    #[test]
    fn test_update_gitignore_replaces_existing_section() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");

        // Create gitignore with existing managed section
        let initial_content = r#"node_modules/

# START Test Marker
OLD_ENTRY.md
# END Test Marker

dist/
"#;
        fs::write(&gitignore_path, initial_content).unwrap();

        let entries = vec!["NEW_ENTRY.md".to_string()];
        update_gitignore(temp_dir.path(), "Test Marker", &entries, false, false).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();

        // Original unmanaged content preserved
        assert!(content.contains("node_modules/"));
        assert!(content.contains("dist/"));

        // Old managed content removed
        assert!(!content.contains("OLD_ENTRY.md"));

        // New managed content added
        assert!(content.contains("NEW_ENTRY.md"));
    }

    #[test]
    fn test_update_gitignore_dry_run() {
        let temp_dir = TempDir::new().unwrap();

        let entries = vec!["CLAUDE.md".to_string()];
        update_gitignore(temp_dir.path(), "AI Agent Symlinks", &entries, true, false).unwrap();

        // File should NOT be created in dry-run mode
        let gitignore_path = temp_dir.path().join(".gitignore");
        assert!(!gitignore_path.exists());
    }

    #[test]
    fn test_update_gitignore_dry_run_does_not_modify_existing() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");

        let original_content = "node_modules/\n";
        fs::write(&gitignore_path, original_content).unwrap();

        let entries = vec!["CLAUDE.md".to_string()];
        update_gitignore(temp_dir.path(), "AI Agent Symlinks", &entries, true, false).unwrap();

        // Content should NOT be modified
        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(content, original_content);
    }

    #[test]
    fn test_update_gitignore_empty_entries() {
        let temp_dir = TempDir::new().unwrap();

        let entries: Vec<String> = vec![];
        update_gitignore(temp_dir.path(), "AI Agent Symlinks", &entries, false, false).unwrap();

        let gitignore_path = temp_dir.path().join(".gitignore");
        let content = fs::read_to_string(&gitignore_path).unwrap();

        // Section should still be created, just empty
        assert!(content.contains("# START AI Agent Symlinks"));
        assert!(content.contains("# END AI Agent Symlinks"));
    }

    #[test]
    fn test_update_gitignore_preserves_trailing_content() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");

        let initial_content = r#"# START Marker
old_entry
# END Marker
trailing_content
"#;
        fs::write(&gitignore_path, initial_content).unwrap();

        let entries = vec!["new_entry".to_string()];
        update_gitignore(temp_dir.path(), "Marker", &entries, false, false).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();

        assert!(content.contains("trailing_content"));
        assert!(content.contains("new_entry"));
        assert!(!content.contains("old_entry"));
    }

    #[test]
    fn test_update_gitignore_multiple_entries() {
        let temp_dir = TempDir::new().unwrap();

        let entries = vec![
            "entry1.md".to_string(),
            "entry2.md".to_string(),
            "entry3.md".to_string(),
            ".github/copilot-instructions.md".to_string(),
        ];
        update_gitignore(temp_dir.path(), "AI Agent Symlinks", &entries, false, false).unwrap();

        let gitignore_path = temp_dir.path().join(".gitignore");
        let content = fs::read_to_string(&gitignore_path).unwrap();

        for entry in &entries {
            assert!(content.contains(entry), "Should contain entry: {}", entry);
        }
    }

    #[test]
    fn test_update_gitignore_custom_marker() {
        let temp_dir = TempDir::new().unwrap();

        let entries = vec!["test.md".to_string()];
        update_gitignore(temp_dir.path(), "My Custom Marker", &entries, false, false).unwrap();

        let gitignore_path = temp_dir.path().join(".gitignore");
        let content = fs::read_to_string(&gitignore_path).unwrap();

        assert!(content.contains("# START My Custom Marker"));
        assert!(content.contains("# END My Custom Marker"));
    }

    #[test]
    fn test_cleanup_gitignore_removes_matching_managed_block() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        fs::write(
            &gitignore_path,
            "node_modules/\n# START Marker\nmanaged\n# END Marker\ndist/\n",
        )
        .unwrap();

        cleanup_gitignore(temp_dir.path(), "Marker", false, false).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(content, "node_modules/\ndist/\n");
    }

    #[test]
    fn test_cleanup_gitignore_respects_custom_marker() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        fs::write(
            &gitignore_path,
            "# START Default Marker\nkeep\n# END Default Marker\n# START Custom Marker\nremove\n# END Custom Marker\n",
        )
        .unwrap();

        cleanup_gitignore(temp_dir.path(), "Custom Marker", false, false).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains("# START Default Marker"));
        assert!(content.contains("keep"));
        assert!(!content.contains("Custom Marker"));
        assert!(!content.contains("remove"));
    }

    #[test]
    fn test_cleanup_gitignore_dry_run_does_not_modify_existing() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        let original = "node_modules/\n# START Marker\nmanaged\n# END Marker\n";
        fs::write(&gitignore_path, original).unwrap();

        cleanup_gitignore(temp_dir.path(), "Marker", true, false).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn test_cleanup_gitignore_is_noop_when_matching_block_missing() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        let original = "node_modules/\n# START Other\nmanaged\n# END Other\n";
        fs::write(&gitignore_path, original).unwrap();

        cleanup_gitignore(temp_dir.path(), "Marker", false, false).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(content, original);
    }

    // ==========================================================================
    // EDGE CASE TESTS
    // ==========================================================================

    #[test]
    fn test_update_gitignore_with_special_characters_in_entries() {
        let temp_dir = TempDir::new().unwrap();

        let entries = vec![
            ".github/copilot-instructions.md".to_string(),
            "path/with spaces/file.md".to_string(),
            "*.md".to_string(),
        ];
        update_gitignore(temp_dir.path(), "Marker", &entries, false, false).unwrap();

        let gitignore_path = temp_dir.path().join(".gitignore");
        let content = fs::read_to_string(&gitignore_path).unwrap();

        for entry in &entries {
            assert!(content.contains(entry));
        }
    }

    #[test]
    fn test_update_gitignore_idempotent() {
        let temp_dir = TempDir::new().unwrap();

        let entries = vec!["test.md".to_string()];

        // Apply twice
        update_gitignore(temp_dir.path(), "Marker", &entries, false, false).unwrap();
        update_gitignore(temp_dir.path(), "Marker", &entries, false, false).unwrap();

        let gitignore_path = temp_dir.path().join(".gitignore");
        let content = fs::read_to_string(&gitignore_path).unwrap();

        // Should only have one managed section
        let start_count = content.matches("# START Marker").count();
        let end_count = content.matches("# END Marker").count();

        assert_eq!(start_count, 1, "Should have exactly one START marker");
        assert_eq!(end_count, 1, "Should have exactly one END marker");
    }

    #[test]
    fn test_update_gitignore_preserves_mtime_if_unchanged() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        let entries = vec!["test.md".to_string()];

        // Initial update
        update_gitignore(temp_dir.path(), "Marker", &entries, false, false).unwrap();
        let mtime1 = fs::metadata(&gitignore_path).unwrap().modified().unwrap();

        // Small sleep to ensure mtime would change if written
        std::thread::sleep(std::time::Duration::from_millis(20));

        // Second update with same content
        update_gitignore(temp_dir.path(), "Marker", &entries, false, false).unwrap();
        let mtime2 = fs::metadata(&gitignore_path).unwrap().modified().unwrap();

        assert_eq!(
            mtime1, mtime2,
            "Modification time should not change if content is identical"
        );
    }

    #[test]
    fn test_update_gitignore_rejects_newline_in_entries() {
        let temp_dir = TempDir::new().unwrap();
        let entries = vec!["good.md".to_string(), "evil\nINJECTED".to_string()];
        let result = update_gitignore(temp_dir.path(), "Marker", &entries, false, false);
        assert!(
            result.is_err(),
            "newline in gitignore entry must be rejected, got Ok"
        );
    }

    #[test]
    fn test_update_gitignore_rejects_newline_in_marker() {
        let temp_dir = TempDir::new().unwrap();
        let entries = vec!["good.md".to_string()];
        let result = update_gitignore(temp_dir.path(), "Bad\nMarker", &entries, false, false);
        assert!(
            result.is_err(),
            "newline in marker must be rejected, got Ok"
        );
    }

    // =============================================================================
    // Local mode tests (.git/info/exclude)
    // =============================================================================

    /// Helper to create a Git repository for local mode tests
    fn create_git_repo(project_root: &Path) {
        let git_dir = project_root.join(".git");
        fs::create_dir_all(&git_dir).unwrap();
        // Create a minimal .git directory structure
        fs::create_dir_all(git_dir.join("info")).unwrap();
        fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    }

    #[test]
    fn test_update_gitignore_local_creates_exclude_file() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Create Git repository first
        create_git_repo(project_root);

        let entries = vec!["CLAUDE.md".to_string(), "AGENTS.md".to_string()];

        // Call with local = true
        update_gitignore(project_root, "AI Agent Symlinks", &entries, false, true).unwrap();

        // Should create .git/info/exclude
        let exclude_path = project_root.join(".git").join("info").join("exclude");
        assert!(
            exclude_path.exists(),
            ".git/info/exclude should be created in local mode"
        );

        let content = fs::read_to_string(&exclude_path).unwrap();
        assert!(content.contains("# START AI Agent Symlinks"));
        assert!(content.contains("# END AI Agent Symlinks"));
        assert!(content.contains("CLAUDE.md"));
        assert!(content.contains("AGENTS.md"));

        // Should NOT create .gitignore (separate file in project root)
        let gitignore_path = project_root.join(".gitignore");
        assert!(
            !gitignore_path.exists(),
            ".gitignore should not be created in local mode"
        );
    }

    #[test]
    fn test_update_gitignore_local_appends_to_existing_exclude() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Create Git repository first
        create_git_repo(project_root);

        // Create existing .git/info/exclude with content
        let exclude_path = project_root.join(".git").join("info").join("exclude");
        fs::write(&exclude_path, "node_modules/\n*.log\n").unwrap();

        let entries = vec!["CLAUDE.md".to_string()];
        update_gitignore(project_root, "AI Agent Symlinks", &entries, false, true).unwrap();

        let content = fs::read_to_string(&exclude_path).unwrap();

        // Original content preserved
        assert!(content.contains("node_modules/"));
        assert!(content.contains("*.log"));

        // New content added
        assert!(content.contains("# START AI Agent Symlinks"));
        assert!(content.contains("CLAUDE.md"));
        assert!(content.contains("# END AI Agent Symlinks"));
    }

    #[test]
    fn test_update_gitignore_local_does_not_modify_gitignore() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Create Git repository first
        create_git_repo(project_root);

        // Create existing .gitignore
        let gitignore_path = project_root.join(".gitignore");
        fs::write(&gitignore_path, "node_modules/\n").unwrap();

        let entries = vec!["CLAUDE.md".to_string()];
        update_gitignore(project_root, "AI Agent Symlinks", &entries, false, true).unwrap();

        // .gitignore should remain unchanged
        let gitignore_content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(gitignore_content.contains("node_modules/"));
        // Note: update now cleans opposite file, so these assertions change
        assert!(!gitignore_content.contains("CLAUDE.md"));
        assert!(!gitignore_content.contains("AI Agent Symlinks"));
    }

    #[test]
    fn test_cleanup_gitignore_local_cleans_exclude_file() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Create Git repository first
        create_git_repo(project_root);

        // Create managed content in .git/info/exclude
        let exclude_path = project_root.join(".git").join("info").join("exclude");
        let original =
            "# Existing entry\n# START AI Agent Symlinks\nCLAUDE.md\n# END AI Agent Symlinks\n";
        fs::write(&exclude_path, original).unwrap();

        cleanup_gitignore(project_root, "AI Agent Symlinks", false, true).unwrap();

        let content = fs::read_to_string(&exclude_path).unwrap();

        // Should remove managed section
        assert!(!content.contains("CLAUDE.md"));
        assert!(!content.contains("# START AI Agent Symlinks"));

        // Should preserve original entry
        assert!(content.contains("# Existing entry"));
    }

    #[test]
    fn test_update_gitignore_local_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Create Git repository first
        create_git_repo(project_root);

        let entries = vec!["CLAUDE.md".to_string()];

        // First call
        update_gitignore(project_root, "AI Agent Symlinks", &entries, false, true).unwrap();

        // Second call should be idempotent
        update_gitignore(project_root, "AI Agent Symlinks", &entries, false, true).unwrap();

        let exclude_path = project_root.join(".git").join("info").join("exclude");
        let content = fs::read_to_string(&exclude_path).unwrap();

        // Should have exactly one managed block
        let start_count = content.matches("# START AI Agent Symlinks").count();
        let end_count = content.matches("# END AI Agent Symlinks").count();
        assert_eq!(start_count, 1);
        assert_eq!(end_count, 1);
    }

    #[test]
    fn test_cleanup_gitignore_local_does_not_modify_gitignore() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Create Git repository first
        create_git_repo(project_root);

        // Create .gitignore with managed content (opposite file)
        let gitignore_path = project_root.join(".gitignore");
        let gitignore_content = "# START AI Agent Symlinks\nCLAUDE.md\n# END AI Agent Symlinks\n";
        fs::write(&gitignore_path, gitignore_content).unwrap();

        // Cleanup in local mode should clean .git/info/exclude AND the opposite .gitignore
        cleanup_gitignore(project_root, "AI Agent Symlinks", false, true).unwrap();

        // .gitignore should have the managed section removed (cleanup now cleans opposite file)
        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(
            !content.contains("# START AI Agent Symlinks"),
            "Opposite file should have managed section removed"
        );
    }

    #[test]
    #[cfg(unix)]
    fn update_gitignore_rejects_symlink_without_modifying_target() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        fs::create_dir_all(&project_root).unwrap();
        let external = temp_dir.path().join("external.gitignore");
        let original = "# START AgentSync\nmanaged-entry\n# END AgentSync\n";
        fs::write(&external, original).unwrap();
        symlink(&external, project_root.join(".gitignore")).unwrap();

        let error = update_gitignore(
            &project_root,
            "AgentSync",
            &["new-entry".to_string()],
            false,
            false,
        )
        .expect_err("symlinked .gitignore must be rejected");
        assert!(format!("{error:#}").contains(".gitignore"));
        assert_eq!(fs::read_to_string(&external).unwrap(), original);
    }

    #[test]
    #[cfg(unix)]
    fn cleanup_gitignore_rejects_symlink_without_modifying_target() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        fs::create_dir_all(&project_root).unwrap();
        let external = temp_dir.path().join("external.gitignore");
        let original = "# START AgentSync\nmanaged-entry\n# END AgentSync\n";
        fs::write(&external, original).unwrap();
        symlink(&external, project_root.join(".gitignore")).unwrap();

        let error = cleanup_gitignore(&project_root, "AgentSync", false, false)
            .expect_err("symlinked .gitignore must be rejected");
        assert!(format!("{error:#}").contains(".gitignore"));
        assert_eq!(fs::read_to_string(&external).unwrap(), original);
    }

    #[test]
    #[cfg(unix)]
    fn atomic_gitignore_write_replaces_symlink_at_persist_boundary() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        let external = temp_dir.path().join("external.gitignore");
        let external_original = "external content must remain unchanged\n";
        let intended_content = "new managed gitignore content\n";
        fs::write(&gitignore_path, "old .gitignore content\n").unwrap();
        fs::write(&external, external_original).unwrap();
        let existing_permissions = fs::symlink_metadata(&gitignore_path).unwrap().permissions();

        write_gitignore_atomically_with_hook(
            &gitignore_path,
            intended_content,
            Some(existing_permissions),
            || {
                fs::remove_file(&gitignore_path)
                    .unwrap_or_else(|error| panic!("failed to remove old .gitignore: {error}"));
                symlink(&external, &gitignore_path).unwrap_or_else(|error| {
                    panic!("failed to arrange final-boundary symlink: {error}")
                });
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(fs::read_to_string(&external).unwrap(), external_original);
        assert_eq!(
            fs::read_to_string(&gitignore_path).unwrap(),
            intended_content
        );
        assert!(
            !fs::symlink_metadata(&gitignore_path)
                .unwrap()
                .file_type()
                .is_symlink(),
            "atomic replacement should replace the swapped-in symlink path"
        );
    }

    #[test]
    #[cfg(unix)]
    fn update_and_cleanup_gitignore_preserve_regular_file_permissions() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        fs::write(&gitignore_path, "# START Marker\nold-entry\n# END Marker\n").unwrap();
        fs::set_permissions(&gitignore_path, fs::Permissions::from_mode(0o640)).unwrap();
        let original_metadata = fs::metadata(&gitignore_path).unwrap();
        let original_uid = original_metadata.uid();
        let original_gid = original_metadata.gid();

        update_gitignore(
            temp_dir.path(),
            "Marker",
            &["new-entry".to_string()],
            false,
            false,
        )
        .unwrap();
        let metadata = fs::metadata(&gitignore_path).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o640);
        assert_eq!(metadata.uid(), original_uid);
        assert_eq!(metadata.gid(), original_gid);

        cleanup_gitignore(temp_dir.path(), "Marker", false, false).unwrap();
        let metadata = fs::metadata(&gitignore_path).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o640);
        assert_eq!(metadata.uid(), original_uid);
        assert_eq!(metadata.gid(), original_gid);
    }

    #[test]
    #[cfg(windows)]
    fn update_gitignore_preserves_restricted_windows_dacl() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        fs::write(&gitignore_path, "old-entry\n").unwrap();
        crate::mcp::set_restricted_permissions(&gitignore_path).unwrap();

        update_gitignore(
            temp_dir.path(),
            "AgentSync",
            &["new-entry".to_string()],
            false,
            false,
        )
        .unwrap();

        crate::mcp::verify_restricted_permissions(&gitignore_path).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn update_gitignore_preserves_existing_mode_under_restrictive_umask() {
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command;

        const CHILD_PATH_ENV: &str = "AGENTSYNC_GITIGNORE_UMASK_TEST_PATH";

        if let Some(path) = std::env::var_os(CHILD_PATH_ENV) {
            // SAFETY: This subprocess runs only this test, so changing its process umask
            // cannot affect other test threads or processes.
            unsafe {
                umask(0o077);
            }

            let project_root = Path::new(&path);
            update_gitignore(
                project_root,
                "Marker",
                &["new-entry".to_string()],
                false,
                false,
            )
            .unwrap();
            let gitignore_path = project_root.join(".gitignore");
            assert_eq!(
                fs::metadata(&gitignore_path).unwrap().permissions().mode() & 0o777,
                0o644,
                ".gitignore mode should be restored exactly despite umask 077"
            );

            let new_project_root = project_root.join("new-file-project");
            fs::create_dir(&new_project_root).unwrap();
            update_gitignore(
                &new_project_root,
                "Marker",
                &["new-entry".to_string()],
                false,
                false,
            )
            .unwrap();
            assert_eq!(
                fs::metadata(new_project_root.join(".gitignore"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600,
                "new .gitignore should retain the default 0666 filtered by umask 077"
            );
            return;
        }

        let temp_dir = TempDir::new().unwrap();
        let gitignore_path = temp_dir.path().join(".gitignore");
        fs::write(&gitignore_path, "# START Marker\nold-entry\n# END Marker\n").unwrap();
        fs::set_permissions(&gitignore_path, fs::Permissions::from_mode(0o644)).unwrap();

        let test_name = std::thread::current().name().unwrap().to_owned();
        let output = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(test_name)
            .arg("--nocapture")
            .env(CHILD_PATH_ENV, temp_dir.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "isolated umask test subprocess failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    unsafe extern "C" {
        fn umask(mask: u32) -> u32;
    }
}
