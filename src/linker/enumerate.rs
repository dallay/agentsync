//! Shared destination enumeration for `clean` and `revert`.
//!
//! Both commands walk the same four sync-type shapes; only the per-destination
//! action differs (drop the symlink vs drop it and restore the `.bak`
//! backup). These helpers yield candidate destination `PathBuf`s so the
//! traversal logic lives in one place. Skip/error reporting stays with the
//! callers: `clean` silently skips unsafe destinations while `revert` counts
//! them as errors.

use anyhow::{Context, Result};
use cap_fs_ext::DirExt;
use cap_std::ambient_authority;
use cap_std::fs::Dir as CapabilityDir;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::config::TargetConfig;

use super::{Linker, SyncOptions};

/// A destination string that failed `ensure_safe_destination`, kept with its
/// display strings so callers can warn (revert) or stay silent (clean).
pub(super) struct SkippedDestination {
    pub dest: String,
    pub error: String,
}

/// One expanded nested-glob destination: either validated or skipped,
/// plus the discovered source file the destination was expanded from (used
/// by revert to verify the symlink target before removing it).
pub(super) struct NestedGlobEntry {
    pub source: PathBuf,
    pub dest: Result<PathBuf, SkippedDestination>,
}

/// Nested-glob discovery result: the raw template probe plus one entry per
/// non-empty expansion, in discovery order.
pub(super) struct NestedGlobEnumeration {
    pub template: Result<(), SkippedDestination>,
    pub discovery: NestedGlobDiscoveryStatus,
    pub entries: Vec<NestedGlobEntry>,
}

/// Whether nested-glob source discovery was attempted and completed.
/// An available directory with no matches is still a complete discovery.
pub(super) enum NestedGlobDiscoveryStatus {
    NotAttempted,
    Complete,
    Incomplete {
        search_root: PathBuf,
        reason: String,
    },
}

/// One module-map destination: either validated or skipped, in mapping order.
pub(super) struct ModuleMapEntry {
    pub source: String,
    pub dest_str: String,
    pub dest: Result<PathBuf, String>,
}

/// A symlink-contents directory opened component-by-component without following
/// symlinks. Enumeration and child removal stay relative to these open handles
/// even if an attacker replaces a path component after it was opened.
pub(super) struct ContentsDirectory {
    parent: CapabilityDir,
    directory: CapabilityDir,
    name: OsString,
    path: PathBuf,
}

pub(super) struct ContentsEntry {
    pub name: OsString,
    pub path: PathBuf,
}

impl ContentsDirectory {
    pub fn entries(&self) -> Result<Vec<ContentsEntry>> {
        self.directory
            .entries()
            .with_context(|| {
                format!(
                    "Failed to read destination directory: {}",
                    self.path.display()
                )
            })?
            .map(|entry| {
                let entry = entry
                    .with_context(|| format!("Failed to read entry in: {}", self.path.display()))?;
                let name = entry.file_name();
                Ok(ContentsEntry {
                    path: self.path.join(&name),
                    name,
                })
            })
            .collect()
    }

    pub fn symlink_metadata(&self, name: &OsStr) -> std::io::Result<cap_std::fs::Metadata> {
        self.directory.symlink_metadata(name)
    }

    pub fn remove_file_or_symlink(&self, name: &OsStr) -> std::io::Result<()> {
        self.directory.remove_file_or_symlink(name)
    }

    pub fn remove_empty_directory(&self) -> std::io::Result<()> {
        self.parent.remove_dir(&self.name)
    }
}

impl Linker {
    /// Resolve a single destination string, capturing the failure display
    /// strings instead of discarding them.
    pub(super) fn resolve_destination(
        &self,
        dest_str: &str,
    ) -> Result<PathBuf, SkippedDestination> {
        self.ensure_safe_destination(dest_str)
            .map_err(|e| SkippedDestination {
                dest: dest_str.to_string(),
                error: e.to_string(),
            })
    }

    /// Read the raw entry paths of a `symlink-contents` destination directory.
    pub(super) fn read_contents_entries(&self, dir: &Path) -> Result<Vec<PathBuf>> {
        self.revalidate_path(dir)
            .with_context(|| format!("Unsafe destination directory: {}", dir.display()))?;
        let mut entries = Vec::new();
        let metadata = fs::symlink_metadata(dir).with_context(|| {
            format!("Failed to inspect destination directory: {}", dir.display())
        })?;
        if metadata.file_type().is_symlink() {
            anyhow::bail!(
                "Unsafe destination directory is a symlink: {}",
                dir.display()
            );
        }
        for entry in fs::read_dir(dir)
            .with_context(|| format!("Failed to read destination directory: {}", dir.display()))?
        {
            let entry =
                entry.with_context(|| format!("Failed to read entry in: {}", dir.display()))?;
            entries.push(entry.path());
        }
        Ok(entries)
    }

    /// Open a `symlink-contents` destination by walking one component at a time
    /// from an already-open project root. `open_dir_nofollow` rejects symlink
    /// or reparse-point components, and the returned handles remain anchored
    /// if the path is renamed or replaced later.
    pub(super) fn open_contents_directory(&self, dir: &Path) -> Result<ContentsDirectory> {
        let name = dir
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Destination directory has no final component"))?;
        let parent_path = dir
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Destination directory has no parent"))?;
        let parent = self.open_project_relative_directory(parent_path)?;
        let directory = parent
            .open_dir_nofollow(name)
            .with_context(|| format!("Unsafe destination directory: {}", dir.display()))?;
        let opened = ContentsDirectory {
            parent,
            directory,
            name: name.to_os_string(),
            path: dir.to_path_buf(),
        };

        #[cfg(test)]
        if let Some(hook) = self.read_contents_after_metadata_hook.borrow().as_ref() {
            hook(dir);
        }

        Ok(opened)
    }

    /// Open a project-relative directory by traversing each component from a
    /// capability rooted at the canonical project directory.
    pub(super) fn open_project_relative_directory(&self, dir: &Path) -> Result<CapabilityDir> {
        let relative = dir
            .strip_prefix(&self.project_root)
            .with_context(|| format!("Directory is outside project root: {}", dir.display()))?;
        let canonical_root = fs::canonicalize(&self.project_root).with_context(|| {
            format!(
                "Failed to canonicalize project root: {}",
                self.project_root.display()
            )
        })?;
        let mut current = CapabilityDir::open_ambient_dir(&canonical_root, ambient_authority())
            .with_context(|| {
                format!("Failed to open project root: {}", canonical_root.display())
            })?;
        for component in relative.components() {
            let Component::Normal(name) = component else {
                anyhow::bail!(
                    "Invalid project-relative directory component: {}",
                    dir.display()
                );
            };
            current = current.open_dir_nofollow(name).with_context(|| {
                format!("Unsafe directory component: {}", name.to_string_lossy())
            })?;
        }
        Ok(current)
    }

    /// Discover nested-glob destinations: validate the search root, expand the
    /// template per match, and resolve each expansion. Empty expansions are
    /// dropped silently (both callers agree). Missing or unsafe search roots
    /// are marked incomplete; `clean` keeps its historical silent skip while
    /// `revert` reports it.
    pub(super) fn enumerate_nested_glob(
        &self,
        target: &TargetConfig,
        options: &SyncOptions,
    ) -> Result<NestedGlobEnumeration> {
        let template = match self.ensure_safe_destination(&target.destination) {
            Ok(_) => Ok(()),
            Err(e) => Err(SkippedDestination {
                dest: target.destination.clone(),
                error: e.to_string(),
            }),
        };
        // Short-circuit the walk when the template itself is unsafe, mirroring
        // the historical early return in both callers.
        if template.is_err() {
            return Ok(NestedGlobEnumeration {
                template,
                discovery: NestedGlobDiscoveryStatus::NotAttempted,
                entries: Vec::new(),
            });
        }

        let search_root = self.project_root.join(&target.source);
        if let Err(error) = self.revalidate_path(&search_root) {
            return Ok(NestedGlobEnumeration {
                template,
                discovery: NestedGlobDiscoveryStatus::Incomplete {
                    search_root,
                    reason: format!("Unsafe search root: {error}"),
                },
                entries: Vec::new(),
            });
        }
        if !search_root.exists() || !search_root.is_dir() {
            return Ok(NestedGlobEnumeration {
                template,
                discovery: NestedGlobDiscoveryStatus::Incomplete {
                    search_root,
                    reason: "Search root does not exist or is not a directory".to_string(),
                },
                entries: Vec::new(),
            });
        }
        let glob_pattern = target.pattern.as_deref().unwrap_or("**/AGENTS.md");
        let dest_template = &target.destination;
        let excludes = &target.exclude;

        let (matches, discovery_complete) = self.get_nested_glob_matches_with_status(
            &search_root,
            glob_pattern,
            excludes,
            options,
        )?;

        let mut entries = Vec::with_capacity(matches.len());
        for (full_path, rel_path) in matches.iter() {
            let dest_str = Self::expand_destination_template(dest_template, rel_path);
            if dest_str.is_empty() {
                continue;
            }
            entries.push(NestedGlobEntry {
                source: full_path.clone(),
                dest: self.resolve_destination(&dest_str),
            });
        }
        Ok(NestedGlobEnumeration {
            template,
            discovery: if discovery_complete {
                NestedGlobDiscoveryStatus::Complete
            } else {
                NestedGlobDiscoveryStatus::Incomplete {
                    search_root,
                    reason: "WalkDir traversal encountered one or more entry errors".to_string(),
                }
            },
            entries,
        })
    }

    /// Resolve every module-map mapping destination, preserving mapping order.
    pub(super) fn enumerate_module_map(
        &self,
        agent_name: &str,
        target: &TargetConfig,
    ) -> Vec<ModuleMapEntry> {
        target
            .mappings
            .iter()
            .map(|mapping| {
                let dest_str = super::apply::module_map_destination(mapping, agent_name);
                let dest = self
                    .ensure_safe_destination(&dest_str)
                    .map_err(|e| e.to_string());
                ModuleMapEntry {
                    source: mapping.source.clone(),
                    dest_str,
                    dest,
                }
            })
            .collect()
    }

    /// Return whether a symlink-contents child is within the source-name
    /// pattern that apply uses. Z-Code can rename `<name>.agent.md` to
    /// `<name>.md`, so inspect source entries when available and use the
    /// reversible filename convention when the source has disappeared.
    pub(super) fn contents_child_matches_pattern(
        &self,
        agent_name: &str,
        target: &TargetConfig,
        entry_path: &Path,
    ) -> bool {
        let Some(pattern) = target.pattern.as_deref() else {
            return true;
        };
        let Some(destination_name) = entry_path.file_name().and_then(OsStr::to_str) else {
            return false;
        };

        let is_zcode_commands = crate::agent_ids::canonical_any_agent_id(agent_name)
            == Some("zcode")
            && target.destination.ends_with(".zcode/commands");
        if !is_zcode_commands {
            return super::matches_pattern(destination_name, pattern);
        }

        let source_dir = self.source_dir.join(&target.source);
        match super::symlinks::sorted_dir_entries(&source_dir) {
            Ok(source_entries) => {
                let mut destination_has_source = false;
                let mut destination_has_matching_source = false;
                for source_entry in source_entries {
                    let source_name = source_entry.file_name().to_string_lossy().into_owned();
                    if crate::zcode_command_destination(&source_name) == destination_name {
                        destination_has_source = true;
                        destination_has_matching_source |=
                            super::matches_pattern(&source_name, pattern);
                    }
                }
                if destination_has_source {
                    return destination_has_matching_source;
                }
            }
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
            {
                // The source may have been removed after apply. Reconstruct the
                // canonical `.agent.md` mapping from the destination name below.
            }
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    path = %source_dir.display(),
                    "Cannot determine whether a Z-Code child matches the source pattern"
                );
                return false;
            }
        }

        super::matches_pattern(destination_name, pattern)
            || destination_name
                .strip_suffix(".md")
                .is_some_and(|stem| super::matches_pattern(&format!("{stem}.agent.md"), pattern))
    }
}

/// Whether a `symlink-contents` directory entry is a managed child: a symlink
/// that survives the zcode `.md` filter (zcode command directories only manage
/// `.md` files; every other agent manages every symlink child).
pub(super) fn contents_child_is_managed(
    agent_name: &str,
    target: &TargetConfig,
    entry_path: &Path,
    is_symlink: bool,
) -> bool {
    if !is_symlink {
        return false;
    }
    !zcode_contents_child_filtered(agent_name, target, entry_path)
}

/// Whether the zcode `.md` filter excludes this entry (regardless of whether
/// the entry is a symlink): true only inside `.zcode/commands` targets for
/// names that do not end in `.md`.
pub(super) fn zcode_contents_child_filtered(
    agent_name: &str,
    target: &TargetConfig,
    entry_path: &Path,
) -> bool {
    crate::agent_ids::canonical_any_agent_id(agent_name) == Some("zcode")
        && target.destination.ends_with(".zcode/commands")
        && !entry_path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.ends_with(".md"))
}

#[cfg(test)]
mod tests {
    use crate::config::SyncType;
    use crate::linker::test_support::{make_linker, make_target};
    use std::collections::VecDeque;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use walkdir::DirEntry;

    use super::super::discovery::NestedGlobWalkError;
    use super::NestedGlobDiscoveryStatus;

    struct InjectedWalk(VecDeque<std::result::Result<DirEntry, NestedGlobWalkError>>);

    impl super::super::discovery::NestedGlobWalkIterator for InjectedWalk {
        fn next_entry(&mut self) -> Option<std::result::Result<DirEntry, NestedGlobWalkError>> {
            self.0.pop_front()
        }

        fn skip_current_dir(&mut self) {}
    }

    fn walk_entries_with_error(
        root: &std::path::Path,
        unreadable_dir: &std::path::Path,
    ) -> InjectedWalk {
        let mut entries = walkdir::WalkDir::new(root)
            .sort_by_file_name()
            .into_iter()
            .map(|entry| entry.map_err(NestedGlobWalkError::from))
            .filter(|entry| {
                entry.as_ref().map_or(true, |entry| {
                    !entry.path().starts_with(unreadable_dir.join("AGENTS.md"))
                })
            })
            .collect::<Vec<_>>();
        let error_index = entries
            .iter()
            .position(|entry| {
                entry
                    .as_ref()
                    .is_ok_and(|entry| entry.path() == unreadable_dir)
            })
            .expect("the unreadable directory is present in the otherwise valid walk");
        entries[error_index] = Err(NestedGlobWalkError {
            path: Some(PathBuf::from(unreadable_dir)),
            message: "injected WalkDir entry error".to_string(),
        });
        InjectedWalk(entries.into())
    }

    #[test]
    fn read_contents_entries_rejects_directory_outside_project_root() {
        let project = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let linker = make_linker(
            project.path(),
            true,
            make_target("source", "dest", SyncType::SymlinkContents),
        );

        let error = linker
            .read_contents_entries(outside.path())
            .expect_err("directory outside project root must be rejected");

        assert!(format!("{error:#}").contains("outside project root"));
    }

    #[test]
    fn nested_glob_enumeration_marks_existing_empty_root_complete() {
        let project = TempDir::new().unwrap();
        std::fs::create_dir_all(project.path().join("source")).unwrap();
        let target = make_target(
            "source",
            "dest/{relative_path}/{file_name}",
            SyncType::NestedGlob,
        );
        let linker = make_linker(project.path(), true, target.clone());

        let enumeration = linker
            .enumerate_nested_glob(&target, &super::super::SyncOptions::default())
            .unwrap();

        assert!(matches!(
            enumeration.discovery,
            NestedGlobDiscoveryStatus::Complete
        ));
        assert!(enumeration.entries.is_empty());
    }

    #[test]
    fn nested_glob_enumeration_marks_walk_entry_error_incomplete() {
        let project = TempDir::new().unwrap();
        let search_root = project.path().join("source");
        let good_file = search_root.join("good/AGENTS.md");
        let unreadable_dir = search_root.join("unreadable");
        std::fs::create_dir_all(good_file.parent().unwrap()).unwrap();
        std::fs::write(&good_file, "valid match").unwrap();
        std::fs::create_dir_all(&unreadable_dir).unwrap();
        std::fs::write(unreadable_dir.join("AGENTS.md"), "omitted subtree").unwrap();

        let target = make_target(
            "source",
            "dest/{relative_path}/{file_name}",
            SyncType::NestedGlob,
        );
        let linker = make_linker(project.path(), true, target.clone());
        linker.set_nested_glob_walk_override_for_tests(Box::new(walk_entries_with_error(
            &search_root,
            &unreadable_dir,
        )));

        let enumeration = linker
            .enumerate_nested_glob(&target, &super::super::SyncOptions::default())
            .unwrap();

        assert_eq!(
            enumeration.entries.len(),
            1,
            "the valid sibling match was found"
        );
        assert!(matches!(
            enumeration.discovery,
            NestedGlobDiscoveryStatus::Incomplete { search_root: root, reason }
                if root == search_root && reason.contains("WalkDir")
        ));
    }

    #[test]
    #[cfg(unix)]
    fn nested_glob_revert_skips_partial_discovery_without_removing_links() {
        use std::os::unix::fs::symlink;

        let project = TempDir::new().unwrap();
        let search_root = project.path().join("source");
        let good_file = search_root.join("good/AGENTS.md");
        let unreadable_dir = search_root.join("unreadable");
        std::fs::create_dir_all(good_file.parent().unwrap()).unwrap();
        std::fs::write(&good_file, "valid match").unwrap();
        std::fs::create_dir_all(&unreadable_dir).unwrap();
        std::fs::write(unreadable_dir.join("AGENTS.md"), "omitted subtree").unwrap();

        let mut target = make_target(
            "source",
            "dest/{relative_path}/{file_name}",
            SyncType::NestedGlob,
        );
        target.pattern = Some("**/AGENTS.md".to_string());
        let linker = make_linker(project.path(), true, target);
        let good_dest = project.path().join("dest/good/AGENTS.md");
        let unknown_dest = project.path().join("dest/unreadable/AGENTS.md");
        std::fs::create_dir_all(good_dest.parent().unwrap()).unwrap();
        std::fs::create_dir_all(unknown_dest.parent().unwrap()).unwrap();
        symlink(
            linker.relative_path(&good_dest, &good_file, false).unwrap(),
            &good_dest,
        )
        .unwrap();
        symlink(
            linker
                .relative_path(&unknown_dest, &unreadable_dir.join("AGENTS.md"), false)
                .unwrap(),
            &unknown_dest,
        )
        .unwrap();
        linker.set_nested_glob_walk_override_for_tests(Box::new(walk_entries_with_error(
            &search_root,
            &unreadable_dir,
        )));

        let result = linker
            .revert(&super::super::SyncOptions::default())
            .unwrap();

        assert_eq!(result.skipped, 1);
        assert_eq!(result.errors, 0);
        assert!(
            good_dest.is_symlink(),
            "partial matches must not be reverted"
        );
        assert!(
            unknown_dest.is_symlink(),
            "unknown orphan paths must not be inferred"
        );
    }

    #[test]
    #[cfg(unix)]
    fn clean_reports_partial_nested_glob_discovery_and_continues_other_targets() {
        use std::os::unix::fs::symlink;

        let project = TempDir::new().unwrap();
        let search_root = project.path().join("source");
        let good_file = search_root.join("good/AGENTS.md");
        let unreadable_dir = search_root.join("unreadable");
        std::fs::create_dir_all(good_file.parent().unwrap()).unwrap();
        std::fs::write(&good_file, "valid match").unwrap();
        std::fs::create_dir_all(&unreadable_dir).unwrap();
        std::fs::write(unreadable_dir.join("AGENTS.md"), "omitted subtree").unwrap();
        std::fs::create_dir_all(project.path().join("empty-source")).unwrap();
        let independent_source = project.path().join(".agents/independent.md");
        std::fs::create_dir_all(independent_source.parent().unwrap()).unwrap();
        std::fs::write(&independent_source, "independent").unwrap();

        let mut partial_target = make_target(
            "source",
            "dest/{relative_path}/{file_name}",
            SyncType::NestedGlob,
        );
        partial_target.pattern = Some("**/AGENTS.md".to_string());
        let mut linker = make_linker(project.path(), true, partial_target.clone());
        let targets = &mut linker.config.agents.get_mut("test").unwrap().targets;
        targets.clear();
        targets.insert("a-partial".to_string(), partial_target);
        targets.insert(
            "b-empty".to_string(),
            make_target(
                "empty-source",
                "empty-dest/{relative_path}/{file_name}",
                SyncType::NestedGlob,
            ),
        );
        targets.insert(
            "z-independent".to_string(),
            make_target("independent.md", "independent.md", SyncType::Symlink),
        );

        let known_destination = project.path().join("dest/good/AGENTS.md");
        std::fs::create_dir_all(known_destination.parent().unwrap()).unwrap();
        symlink(&good_file, &known_destination).unwrap();
        let independent_destination = project.path().join("independent.md");
        symlink(&independent_source, &independent_destination).unwrap();
        linker.set_nested_glob_walk_override_for_tests(Box::new(walk_entries_with_error(
            &search_root,
            &unreadable_dir,
        )));

        let result = linker.clean(&super::super::SyncOptions::default()).unwrap();

        assert_eq!(result.removed, 2, "known and independent links are cleaned");
        assert_eq!(
            result.skipped, 1,
            "the incomplete walk is reported; the valid empty root adds no skip"
        );
        assert_eq!(result.errors, 0);
        assert!(!known_destination.exists());
        assert!(!independent_destination.exists());
    }
}
