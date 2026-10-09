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

use super::{Linker, SyncOptions, quarantine};

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
    IncompleteWalk {
        search_root: PathBuf,
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
    identity: quarantine::EntryIdentity,
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

    pub fn remove_symlink_if_unchanged<F, G>(
        &self,
        name: &OsStr,
        expected: quarantine::EntryIdentity,
        display_path: &Path,
        before_move: F,
        after_move: G,
    ) -> anyhow::Result<quarantine::RemoveOutcome>
    where
        F: FnOnce(),
        G: FnOnce(&Path),
    {
        quarantine::remove_symlink_if_unchanged(
            &self.directory,
            name,
            expected,
            display_path,
            before_move,
            after_move,
        )
    }

    pub fn remove_empty_directory<F, G>(
        &self,
        before_move: F,
        after_move: G,
    ) -> anyhow::Result<quarantine::RemoveDirectoryOutcome>
    where
        F: FnOnce(),
        G: FnOnce(&Path),
    {
        quarantine::remove_empty_directory_if_unchanged(
            &self.parent,
            &self.name,
            self.identity,
            &self.path,
            before_move,
            after_move,
        )
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
        let contents = self
            .open_contents_directory(dir)
            .with_context(|| format!("Unsafe destination directory: {}", dir.display()))?;
        contents
            .entries()
            .map(|entries| entries.into_iter().map(|entry| entry.path).collect())
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
        let identity = quarantine::EntryIdentity::capture(&directory.metadata(".")?);
        let opened = ContentsDirectory {
            parent,
            directory,
            name: name.to_os_string(),
            path: dir.to_path_buf(),
            identity,
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
        let mut current = self.open_project_root_capability()?;
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

    fn open_project_root_capability(&self) -> Result<CapabilityDir> {
        if let Some(root) = self.project_root_capability.borrow().as_ref() {
            return root
                .try_clone()
                .context("failed to clone open project-root capability");
        }
        let canonical_root = fs::canonicalize(&self.project_root).with_context(|| {
            format!(
                "Failed to canonicalize project root: {}",
                self.project_root.display()
            )
        })?;
        let root_snapshot = CapabilityDir::open_ambient_dir(&canonical_root, ambient_authority())
            .with_context(|| {
            format!(
                "Failed to snapshot project root: {}",
                canonical_root.display()
            )
        })?;
        let expected_identity = quarantine::EntryIdentity::capture(&root_snapshot.metadata(".")?);
        #[cfg(test)]
        if let Some(hook) = self.project_root_before_open_hook.borrow_mut().take() {
            hook(&canonical_root);
        }
        let opened = Self::open_absolute_directory_nofollow(&canonical_root)?;
        anyhow::ensure!(
            expected_identity.matches(&opened.metadata(".")?),
            "Project root changed while opening its no-follow capability: {}",
            canonical_root.display()
        );
        let mut cache = self.project_root_capability.borrow_mut();
        if cache.is_none() {
            *cache = Some(
                opened
                    .try_clone()
                    .context("failed to cache project-root capability")?,
            );
        }
        cache
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("project-root capability was not initialized"))?
            .try_clone()
            .context("failed to clone open project-root capability")
    }

    #[cfg(unix)]
    fn open_absolute_directory_nofollow(path: &Path) -> Result<CapabilityDir> {
        let mut components = path.components();
        if !matches!(components.next(), Some(Component::RootDir)) {
            anyhow::bail!("Project root is not absolute: {}", path.display());
        }
        let mut current = CapabilityDir::open_ambient_dir(Path::new("/"), ambient_authority())
            .context("failed to open filesystem root capability")?;
        for component in components {
            let Component::Normal(name) = component else {
                anyhow::bail!(
                    "Invalid component in canonical project root: {}",
                    path.display()
                );
            };
            current = current.open_dir_nofollow(name).with_context(|| {
                format!("Unsafe project-root component: {}", name.to_string_lossy())
            })?;
        }
        Ok(current)
    }

    #[cfg(windows)]
    fn open_absolute_directory_nofollow(path: &Path) -> Result<CapabilityDir> {
        let mut components = path.components();
        let prefix = match components.next() {
            Some(Component::Prefix(prefix)) => prefix,
            _ => anyhow::bail!(
                "Project root has no Windows volume prefix: {}",
                path.display()
            ),
        };
        let root = match components.next() {
            Some(Component::RootDir) => Path::new(std::path::MAIN_SEPARATOR_STR),
            _ => anyhow::bail!("Project root is not rooted: {}", path.display()),
        };
        let mut anchor = PathBuf::from(prefix.as_os_str());
        anchor.push(root);
        let mut current = CapabilityDir::open_ambient_dir(&anchor, ambient_authority())
            .with_context(|| {
                format!(
                    "failed to open volume root capability: {}",
                    anchor.display()
                )
            })?;
        for component in components {
            let Component::Normal(name) = component else {
                anyhow::bail!(
                    "Invalid component in canonical project root: {}",
                    path.display()
                );
            };
            current = current.open_dir_nofollow(name).with_context(|| {
                format!("Unsafe project-root component: {}", name.to_string_lossy())
            })?;
        }
        Ok(current)
    }

    #[cfg(not(any(unix, windows)))]
    fn open_absolute_directory_nofollow(path: &Path) -> Result<CapabilityDir> {
        anyhow::bail!(
            "No-follow project-root capability opening is unsupported on this platform: {}",
            path.display()
        )
    }

    /// Discover nested-glob destinations: validate the search root, expand the
    /// template per match, and resolve each expansion. Empty expansions are
    /// dropped silently (both callers agree). Missing or unsafe search roots
    /// are marked incomplete, and both callers report the skipped target.
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
                NestedGlobDiscoveryStatus::IncompleteWalk { search_root }
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
    ) -> Result<bool> {
        let Some(pattern) = target.pattern.as_deref() else {
            return Ok(true);
        };
        let Some(destination_name) = entry_path.file_name().and_then(OsStr::to_str) else {
            return Ok(false);
        };

        let is_zcode_commands = crate::agent_ids::canonical_any_agent_id(agent_name)
            == Some("zcode")
            && target.destination.ends_with(".zcode/commands");
        if !is_zcode_commands {
            return Ok(super::matches_pattern(destination_name, pattern));
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
                    return Ok(destination_has_matching_source);
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
                return Err(error).with_context(|| {
                    format!(
                        "Cannot determine whether a Z-Code child matches the source pattern under {}",
                        source_dir.display()
                    )
                });
            }
        }

        Ok(super::matches_pattern(destination_name, pattern)
            || destination_name
                .strip_suffix(".md")
                .is_some_and(|stem| super::matches_pattern(&format!("{stem}.agent.md"), pattern)))
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
    #[cfg(unix)]
    fn read_contents_entries_stays_anchored_when_container_is_replaced() {
        use std::cell::Cell;
        use std::os::unix::fs::symlink;
        use std::rc::Rc;

        let temp = TempDir::new().unwrap();
        let project_root = temp.path().join("project");
        let outside = temp.path().join("outside");
        let dest = project_root.join("dest");
        std::fs::create_dir_all(project_root.join(".agents")).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(dest.join("original.txt"), "original directory entry").unwrap();
        std::fs::write(outside.join("attacker.txt"), "outside entry").unwrap();

        let linker = make_linker(
            &project_root,
            true,
            make_target("source", "dest", SyncType::SymlinkContents),
        );
        let hook_ran = Rc::new(Cell::new(false));
        let hook_ran_in_hook = Rc::clone(&hook_ran);
        let hook_dest = dest.clone();
        let hook_outside = outside.clone();
        let moved_destination = project_root.join("moved-destination");
        *linker.read_contents_after_metadata_hook.borrow_mut() = Some(Rc::new(move |path| {
            assert_eq!(path, hook_dest);
            std::fs::rename(path, &moved_destination).unwrap();
            symlink(&hook_outside, path).unwrap();
            hook_ran_in_hook.set(true);
        }));

        let entries = linker.read_contents_entries(&dest).unwrap();

        assert!(
            hook_ran.get(),
            "enumeration must use the opened directory handle"
        );
        assert_eq!(
            entries
                .iter()
                .filter_map(|entry| entry.file_name())
                .collect::<Vec<_>>(),
            vec![std::ffi::OsStr::new("original.txt")]
        );
        assert!(outside.join("attacker.txt").exists());
        assert!(dest.is_symlink());
    }

    #[test]
    fn contents_child_matches_zcode_orphan_pattern_against_source_and_recreated_name() {
        let project = TempDir::new().unwrap();
        let source_dir = project.path().join(".agents/commands");
        std::fs::create_dir_all(&source_dir).unwrap();
        let source = source_dir.join("foo.agent.md");
        std::fs::write(&source, "command source").unwrap();
        let mut target = make_target("commands", ".zcode/commands", SyncType::SymlinkContents);
        target.pattern = Some("*.agent.md".to_string());
        let mut linker = make_linker(project.path(), true, target.clone());
        let test_agent = linker.config.agents.remove("test").unwrap();
        linker.config.agents.insert("zcode".to_string(), test_agent);
        let orphan_destination = project.path().join(".zcode/commands/foo.md");

        assert!(
            linker
                .contents_child_matches_pattern("zcode", &target, &orphan_destination)
                .unwrap()
        );

        std::fs::remove_file(source).unwrap();
        std::fs::remove_dir(&source_dir).unwrap();
        assert!(
            linker
                .contents_child_matches_pattern("zcode", &target, &orphan_destination)
                .unwrap()
        );
        assert!(
            !linker
                .contents_child_matches_pattern(
                    "zcode",
                    &target,
                    &project.path().join(".zcode/commands/notes.txt")
                )
                .unwrap()
        );
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
            NestedGlobDiscoveryStatus::IncompleteWalk { search_root: root }
                if root == search_root
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
