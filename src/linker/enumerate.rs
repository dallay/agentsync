//! Shared destination enumeration for `clean` and `revert`.
//!
//! Both commands walk the same four sync-type shapes; only the per-destination
//! action differs (drop the symlink vs drop it and restore the `.bak`
//! backup). These helpers yield candidate destination `PathBuf`s so the
//! traversal logic lives in one place. Skip/error reporting stays with the
//! callers: `clean` silently skips unsafe destinations while `revert` counts
//! them as errors.

use anyhow::{Context, Result};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

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
    pub entries: Vec<NestedGlobEntry>,
}

/// One module-map destination: either validated or skipped, in mapping order.
pub(super) struct ModuleMapEntry {
    pub source: String,
    pub dest_str: String,
    pub dest: Result<PathBuf, String>,
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
        for entry in fs::read_dir(dir)
            .with_context(|| format!("Failed to read destination directory: {}", dir.display()))?
        {
            let entry =
                entry.with_context(|| format!("Failed to read entry in: {}", dir.display()))?;
            entries.push(entry.path());
        }
        Ok(entries)
    }

    /// Discover nested-glob destinations: validate the search root, expand the
    /// template per match, and resolve each expansion. Empty expansions are
    /// dropped silently (both callers agree); the search-root checks fail
    /// silently to an empty listing (both callers agree).
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
                entries: Vec::new(),
            });
        }

        let search_root = self.project_root.join(&target.source);
        if self.revalidate_path(&search_root).is_err() {
            return Ok(NestedGlobEnumeration {
                template,
                entries: Vec::new(),
            });
        }
        if !search_root.exists() || !search_root.is_dir() {
            return Ok(NestedGlobEnumeration {
                template,
                entries: Vec::new(),
            });
        }
        let glob_pattern = target.pattern.as_deref().unwrap_or("**/AGENTS.md");
        let dest_template = &target.destination;
        let excludes = &target.exclude;

        let matches =
            self.get_nested_glob_matches(&search_root, glob_pattern, excludes, options)?;

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
        Ok(NestedGlobEnumeration { template, entries })
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
}

/// Whether a `symlink-contents` directory entry is a managed child: a symlink
/// that survives the zcode `.md` filter (zcode command directories only manage
/// `.md` files; every other agent manages every symlink child).
pub(super) fn contents_child_is_managed(
    agent_name: &str,
    target: &TargetConfig,
    entry_path: &Path,
) -> bool {
    if !entry_path.is_symlink() {
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
    use tempfile::TempDir;

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
}
