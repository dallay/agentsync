//! Path canonicalization and safety implementation.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

use super::Linker;
use super::timing::SpanKind;

impl Linker {
    /// Drop every path canonicalization cache entry. Called at the start of
    /// each `sync()` so filesystem changes between runs are always reflected
    /// (REQ-023: caches never survive across runs).
    // `pub(super)` is required by the root façade and future mutation siblings.
    pub(super) fn invalidate_path_cache(&self) {
        self.path_cache.borrow_mut().clear();
    }

    /// Drop the path canonicalization cache entries whose canonical identity
    /// may have changed because of a filesystem mutation at `path`: the exact
    /// key and every key at or below it (prefix-based, REQ-023 scoped
    /// invalidation).
    ///
    /// Unlike [`Self::invalidate_path_cache`], unrelated paths stay cached —
    /// e.g. a sibling destination's parent directory — so `canonicalize_cached`
    /// keeps hitting for unchanged paths within the same run (SC-023b), while
    /// any path mutated by create/update/backup/remove is re-canonicalized on
    /// next use (SC-023c). `ensure_safe_path`/`revalidate_path` are unaffected:
    /// they use `canonicalize_uncached`, which always bypasses the cache.
    ///
    /// The cache is a `BTreeMap`, so keys sharing `path` as a prefix sort
    /// contiguously: a `range(path..)` scan with a `starts_with` filter removes
    /// only the affected entries in O(log n + m) (m = matches, usually 0 for
    /// leaf destinations) instead of scanning the whole map per mutation.
    // `pub(super)` is required by the root façade and mutation siblings.
    pub(super) fn invalidate_path(&self, path: &Path) {
        let mut cache = self.path_cache.borrow_mut();
        let to_remove: Vec<PathBuf> = cache
            .range(path.to_path_buf()..)
            .take_while(|(key, _)| key.starts_with(path))
            .map(|(key, _)| key.clone())
            .collect();
        for key in to_remove {
            cache.remove(&key);
        }
    }

    fn canonicalize_uncached(&self, path: &Path) -> Result<PathBuf> {
        fs::canonicalize(path)
            .with_context(|| format!("Failed to canonicalize path: {}", path.display()))
    }

    /// Get or compute the canonical project root path, using cache.
    fn get_canonical_project_root(&self) -> Result<Rc<PathBuf>> {
        let mut root_cache = self.canonical_project_root.borrow_mut();
        if let Some(ref root) = *root_cache {
            return Ok(Rc::clone(root));
        }

        let root = Rc::new(
            self.canonicalize_uncached(&self.project_root)
                .with_context(|| {
                    format!(
                        "Failed to canonicalize project root: {}",
                        self.project_root.display()
                    )
                })?,
        );
        *root_cache = Some(Rc::clone(&root));
        Ok(root)
    }

    /// Validate that a path resolves within the project root and contains no traversal.
    fn ensure_safe_path(
        &self,
        joined: &Path,
        display_path: &dyn std::fmt::Display,
    ) -> Result<PathBuf> {
        // SECURITY: Check for traversal components before canonicalization to prevent bypasses.
        if joined
            .components()
            .any(|c| matches!(c, Component::ParentDir))
        {
            anyhow::bail!("Path resolves outside project root: {}", display_path);
        }

        let existing_ancestor = joined.ancestors().find(|a| a.exists()).with_context(|| {
            format!(
                "Failed to resolve path within project root: {}",
                joined.display()
            )
        })?;

        let canonical_project_root = self.get_canonical_project_root()?;

        let canonical_ancestor =
            self.canonicalize_uncached(existing_ancestor)
                .with_context(|| {
                    format!(
                        "Failed to canonicalize path ancestor: {}",
                        existing_ancestor.display()
                    )
                })?;

        if !canonical_ancestor.starts_with(&*canonical_project_root) {
            anyhow::bail!("Path resolves outside project root: {}", display_path);
        }

        Ok(joined.to_path_buf())
    }

    /// Validate that a destination path is safe (relative and no traversal).
    /// Returns the resolved path within project_root if safe.
    // `pub(super)` is required by the root façade and future apply/clean siblings.
    pub(super) fn ensure_safe_destination(&self, dest_path: &str) -> Result<PathBuf> {
        let path = Path::new(dest_path);

        // SECURITY: Reject absolute paths to prevent writing to arbitrary locations.
        if path.is_absolute() {
            anyhow::bail!("Destination path must be relative: {}", dest_path);
        }

        // SECURITY: Reject empty/traversal/root/prefix components and ensure at least one Normal component.
        // Optimization: Use a single pass over components to validate safety.
        let mut has_normal = false;
        for component in path.components() {
            match component {
                Component::Normal(_) => has_normal = true,
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    anyhow::bail!(
                        "Destination path contains invalid path components: {}",
                        dest_path
                    );
                }
                Component::CurDir => {}
            }
        }

        if !has_normal {
            anyhow::bail!("Destination path must not be empty: {}", dest_path);
        }

        let joined = self.project_root.join(path);
        self.ensure_safe_path(&joined, &dest_path)
    }

    /// Re-validate a previously joined path immediately before filesystem mutation.
    // `pub(super)` is required by the root façade and future apply/symlink siblings.
    pub(super) fn revalidate_path(&self, dest: &Path) -> Result<()> {
        self.ensure_safe_path(dest, &dest.display()).map(|_| ())
    }

    /// Re-validate a path before unlinking (remove_file/remove_dir).
    /// Unlike revalidate_path, this does NOT canonicalize the final component,
    /// allowing safe removal of symlinks that point outside project_root.
    /// The symlink entry itself must be within project_root, but its target can be anywhere.
    // `pub(super)` is required by the root façade and future symlink/clean siblings.
    pub(super) fn revalidate_unlink_path(&self, path: &Path) -> Result<()> {
        let display_path = path.display().to_string();

        // SECURITY: Validate absolute paths against project_root.
        if path.is_absolute() {
            return self.validate_absolute_unlink_path(path, &display_path);
        }

        // SECURITY: Reject paths with ParentDir components before resolution
        for component in path.components() {
            if matches!(component, Component::ParentDir) {
                anyhow::bail!(
                    "Path contains parent directory (..) component: {}",
                    display_path
                );
            }
        }

        self.validate_relative_unlink_parent(path, &display_path)
    }

    /// Validate an absolute path for unlinking: must be under project_root with valid parent.
    fn validate_absolute_unlink_path(&self, path: &Path, display_path: &str) -> Result<()> {
        if !path.starts_with(&self.project_root) {
            anyhow::bail!("Path is outside project root: {}", display_path);
        }
        if let Some(parent) = path.parent() {
            let canonical_parent = self
                .canonicalize_uncached(parent)
                .with_context(|| format!("Failed to canonicalize parent: {}", parent.display()))?;

            let canonical_root = self.get_canonical_project_root()?;

            if !canonical_parent.starts_with(&*canonical_root) {
                anyhow::bail!(
                    "Path parent resolves outside project root: {}",
                    display_path
                );
            }
        }
        Ok(())
    }

    /// Validate that the parent of a relative path is within project_root.
    fn validate_relative_unlink_parent(&self, path: &Path, display_path: &str) -> Result<()> {
        let Some(parent) = path.parent() else {
            return Ok(());
        };
        if parent.as_os_str().is_empty() {
            return Ok(());
        }

        let parent_absolute = if parent.is_absolute() {
            parent.to_path_buf()
        } else {
            self.project_root.join(parent)
        };

        if !parent_absolute.exists() {
            return Ok(());
        }

        let canonical_parent = self
            .canonicalize_uncached(&parent_absolute)
            .with_context(|| {
                format!(
                    "Failed to canonicalize parent: {}",
                    parent_absolute.display()
                )
            })?;

        let canonical_root = self.get_canonical_project_root()?;

        if !canonical_parent.starts_with(&*canonical_root) {
            anyhow::bail!(
                "Path parent resolves outside project root: {}",
                display_path
            );
        }

        Ok(())
    }

    /// Get the canonical path for a given path, using a cache to avoid
    /// redundant I/O operations.
    fn canonicalize_cached(&self, path: &Path) -> Result<Rc<PathBuf>> {
        let mut cache = self.path_cache.borrow_mut();
        if let Some(cached) = cache.get(path) {
            return Ok(Rc::clone(cached));
        }

        let canonical = Rc::new(fs::canonicalize(path)?);
        cache.insert(path.to_path_buf(), Rc::clone(&canonical));
        Ok(canonical)
    }

    fn canonical_fallback_path(&self, path: &Path) -> Result<PathBuf> {
        let canonical_root = self.get_canonical_project_root()?;

        if path.is_absolute() {
            if let Ok(relative) = path.strip_prefix(&self.project_root) {
                return Ok(canonical_root.join(relative));
            }
            return Ok(path.to_path_buf());
        }

        Ok(canonical_root.join(path))
    }

    /// Calculate relative path from dest to source
    // `pub(super)` is required by the root façade and future symlink sibling.
    pub(super) fn relative_path(
        &self,
        from: &Path,
        to: &Path,
        allow_missing: bool,
    ) -> Result<PathBuf> {
        let _span = self.timing_span(SpanKind::Canonicalize);
        let from_dir = from.parent().unwrap_or(from);

        // Canonicalize paths for accurate relative calculation
        let from_abs = if from_dir.exists() {
            self.canonicalize_cached(from_dir)?
        } else {
            // If dest dir doesn't exist yet, use project root as base
            let relative = from_dir
                .strip_prefix(&self.project_root)
                .unwrap_or(from_dir);
            Rc::new(self.canonical_fallback_path(relative)?)
        };

        let to_abs = match self.canonicalize_cached(to) {
            Ok(path) => path,
            Err(_) if allow_missing => Rc::new(self.canonical_fallback_path(to)?),
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("Source path does not exist: {}", to.display()));
            }
        };

        // Use pathdiff to calculate relative path
        pathdiff::diff_paths(&*to_abs, &*from_abs)
            .ok_or_else(|| anyhow::anyhow!("Cannot calculate relative path"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::collections::BTreeMap;
    use tempfile::TempDir;

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
            plugins: Default::default(),
        };

        Linker::new(config, config_path)
    }

    // ==========================================================================
    // revalidate_unlink_path
    // ==========================================================================

    #[test]
    fn revalidate_unlink_path_accepts_absolute_path_inside_root() {
        let temp = TempDir::new().unwrap();
        let linker = make_linker(temp.path());

        let target = temp.path().join("link.md");
        fs::write(&target, "x").unwrap();

        assert!(linker.revalidate_unlink_path(&target).is_ok());
    }

    #[test]
    fn revalidate_unlink_path_rejects_absolute_path_outside_root() {
        let temp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let linker = make_linker(temp.path());

        let target = outside.path().join("evil.md");

        assert!(linker.revalidate_unlink_path(&target).is_err());
    }

    #[test]
    fn revalidate_unlink_path_rejects_relative_parent_dir_component() {
        let temp = TempDir::new().unwrap();
        let linker = make_linker(temp.path());

        let target = Path::new("subdir/../escape.md");

        assert!(linker.revalidate_unlink_path(target).is_err());
    }

    #[test]
    fn revalidate_unlink_path_accepts_relative_path_with_nonexistent_parent() {
        let temp = TempDir::new().unwrap();
        let linker = make_linker(temp.path());

        // The parent directory does not exist, so validation must short-circuit
        // to Ok without attempting canonicalization.
        let target = Path::new("does/not/exist/file.md");

        assert!(linker.revalidate_unlink_path(target).is_ok());
    }

    #[test]
    fn revalidate_unlink_path_accepts_relative_top_level_path() {
        let temp = TempDir::new().unwrap();
        let linker = make_linker(temp.path());

        // A top-level relative path has an empty parent component.
        let target = Path::new("file.md");

        assert!(linker.revalidate_unlink_path(target).is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn revalidate_unlink_path_rejects_relative_path_whose_parent_escapes_root() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let outside = TempDir::new_in(temp.path().parent().unwrap()).unwrap();
        let linker = make_linker(temp.path());

        let link_path = temp.path().join("escape-link");
        symlink(outside.path(), &link_path).unwrap();

        let target = Path::new("escape-link/file.md");

        assert!(linker.revalidate_unlink_path(target).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn revalidate_unlink_path_accepts_absolute_symlink_target_outside_root() {
        // The managed symlink entry itself is inside project_root even though its
        // target (read via fs::read_link elsewhere) may point outside; this method
        // validates the *entry* path, not the destination it points to.
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let linker = make_linker(temp.path());

        let target_file = outside.path().join("real.md");
        fs::write(&target_file, "hi").unwrap();
        let link = temp.path().join("link.md");
        symlink(&target_file, &link).unwrap();

        assert!(linker.revalidate_unlink_path(&link).is_ok());
    }

    // ==========================================================================
    // revalidate_path / ensure_safe_path
    // ==========================================================================

    #[test]
    fn revalidate_path_accepts_path_with_nonexistent_ancestors() {
        let temp = TempDir::new().unwrap();
        let linker = make_linker(temp.path());

        let deep = temp.path().join("a/b/c/file.md");

        assert!(linker.revalidate_path(&deep).is_ok());
    }

    #[test]
    fn revalidate_path_rejects_parent_dir_component() {
        let temp = TempDir::new().unwrap();
        let linker = make_linker(temp.path());

        let bad = temp.path().join("a/../../escape.md");

        assert!(linker.revalidate_path(&bad).is_err());
    }

    // ==========================================================================
    // relative_path
    // ==========================================================================

    #[test]
    fn relative_path_computes_correct_relative_target() {
        let temp = TempDir::new().unwrap();
        let linker = make_linker(temp.path());

        let source = temp.path().join("source.md");
        fs::write(&source, "hi").unwrap();
        let dest_dir = temp.path().join("nested");
        fs::create_dir_all(&dest_dir).unwrap();
        let dest = dest_dir.join("link.md");

        let rel = linker.relative_path(&dest, &source, false).unwrap();

        assert_eq!(rel, PathBuf::from("../source.md"));
    }

    #[test]
    fn relative_path_errs_when_source_missing_and_not_allowed() {
        let temp = TempDir::new().unwrap();
        let linker = make_linker(temp.path());

        let dest = temp.path().join("link.md");
        let missing_source = temp.path().join("missing.md");

        assert!(linker.relative_path(&dest, &missing_source, false).is_err());
    }

    #[test]
    fn relative_path_falls_back_when_source_missing_and_allowed() {
        let temp = TempDir::new().unwrap();
        let linker = make_linker(temp.path());

        let dest = temp.path().join("link.md");
        let missing_source = temp.path().join("missing.md");

        let rel = linker.relative_path(&dest, &missing_source, true).unwrap();

        assert_eq!(rel, PathBuf::from("missing.md"));
    }

    #[test]
    fn relative_path_uses_project_root_fallback_when_dest_dir_missing() {
        let temp = TempDir::new().unwrap();
        let linker = make_linker(temp.path());

        let source = temp.path().join("source.md");
        fs::write(&source, "hi").unwrap();
        // The destination's parent directory has not been created yet.
        let dest = temp.path().join("not-yet-created").join("link.md");

        let rel = linker.relative_path(&dest, &source, false).unwrap();

        assert_eq!(rel, PathBuf::from("../source.md"));
    }

    #[test]
    fn canonical_fallback_path_canonicalizes_absolute_path_inside_project_root() {
        let temp = TempDir::new().unwrap();
        let linker = make_linker(temp.path());
        let missing = temp.path().join("missing.md");

        let fallback = linker.canonical_fallback_path(&missing).unwrap();

        assert_eq!(
            fallback,
            fs::canonicalize(temp.path()).unwrap().join("missing.md")
        );
    }

    #[test]
    fn canonical_fallback_path_preserves_absolute_path_outside_project_root() {
        let temp = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let linker = make_linker(temp.path());
        let missing = outside.path().join("missing.md");

        let fallback = linker.canonical_fallback_path(&missing).unwrap();

        assert_eq!(fallback, missing);
    }

    // ==========================================================================
    // canonicalize_cached / invalidate_path_cache
    // ==========================================================================

    #[test]
    #[cfg(unix)]
    fn canonicalize_cache_returns_stale_value_until_invalidated() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let linker = make_linker(temp.path());

        let dest = temp.path().join("link.md");
        let real_a = temp.path().join("real-a.md");
        let real_b = temp.path().join("real-b.md");
        fs::write(&real_a, "a").unwrap();
        fs::write(&real_b, "b").unwrap();

        let source_link = temp.path().join("source-link.md");
        symlink(&real_a, &source_link).unwrap();

        let rel_a = linker.relative_path(&dest, &source_link, false).unwrap();
        assert_eq!(rel_a, PathBuf::from("real-a.md"));

        fs::remove_file(&source_link).unwrap();
        symlink(&real_b, &source_link).unwrap();

        // Cache still holds the canonical target resolved on the first call.
        let rel_stale = linker.relative_path(&dest, &source_link, false).unwrap();
        assert_eq!(rel_stale, PathBuf::from("real-a.md"));

        linker.invalidate_path_cache();

        let rel_fresh = linker.relative_path(&dest, &source_link, false).unwrap();
        assert_eq!(rel_fresh, PathBuf::from("real-b.md"));
    }

    // ==========================================================================
    // scoped invalidate_path (REQ-023, work unit B1)
    // ==========================================================================

    /// Build a linker with a `symlink-contents` target linking
    /// `.agents/flat/` → `links/` (4 fixed children), mirroring the benchmark
    /// flat fixture. Returns `(linker, links_dir)`.
    fn make_contents_linker(project_root: &Path) -> (Linker, PathBuf) {
        use crate::config::{AgentConfig, SyncType, TargetConfig};

        let flat = project_root.join(".agents").join("flat");
        fs::create_dir_all(&flat).unwrap();
        for i in 0..4 {
            fs::write(flat.join(format!("f{i:04}.md")), "FIXED\n").unwrap();
        }

        let mut targets = BTreeMap::new();
        targets.insert(
            "target".to_string(),
            TargetConfig {
                source: "flat".to_string(),
                destination: "links".to_string(),
                sync_type: SyncType::SymlinkContents,
                pattern: None,
                exclude: vec![],
                mappings: vec![],
            },
        );
        let agent_config = AgentConfig {
            enabled: true,
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
        fs::write(&config_path, "").unwrap();
        let linker = Linker::new(config, config_path);
        (linker, project_root.join("links"))
    }

    #[test]
    fn invalidate_path_drops_exact_and_descendant_keys_only() {
        let temp = TempDir::new().unwrap();
        let linker = make_linker(temp.path());
        let root = temp.path();

        let dir = root.join("dir");
        let dir_child = dir.join("child.md");
        let sibling = root.join("sibling.md");

        {
            let mut cache = linker.path_cache.borrow_mut();
            cache.insert(dir.clone(), Rc::new(dir.clone()));
            cache.insert(dir_child.clone(), Rc::new(dir_child.clone()));
            cache.insert(sibling.clone(), Rc::new(sibling.clone()));
        }

        linker.invalidate_path(&dir);

        let cache = linker.path_cache.borrow();
        assert!(!cache.contains_key(&dir), "exact key must be dropped");
        assert!(
            !cache.contains_key(&dir_child),
            "descendant keys must be dropped"
        );
        assert!(
            cache.contains_key(&sibling),
            "unrelated keys must survive scoped invalidation"
        );
    }

    #[test]
    fn scoped_invalidation_keeps_sibling_from_dir_cached() {
        let temp = TempDir::new().unwrap();
        let linker = make_linker(temp.path());

        let source = temp.path().join("source.md");
        fs::write(&source, "x").unwrap();
        let dest_dir = temp.path().join("links");
        fs::create_dir_all(&dest_dir).unwrap();
        let dest_a = dest_dir.join("a.md");
        let dest_b = dest_dir.join("b.md");

        // First link populates the cache for `links/` (from_dir) and `source.md`.
        let rel_a = linker.relative_path(&dest_a, &source, false).unwrap();
        assert_eq!(rel_a, PathBuf::from("../source.md"));

        // Post-mutation scoped invalidation — exactly what create_symlink now does.
        linker.invalidate_path(&dest_a);

        // The unchanged from_dir and source entries MUST survive.
        assert!(
            linker.path_cache.borrow().contains_key(&dest_dir),
            "from_dir must stay cached after mutating a child dest"
        );
        assert!(
            linker.path_cache.borrow().contains_key(&source),
            "source must stay cached after mutating a child dest"
        );

        // Second sibling reuses the surviving from_dir entry → same result.
        let rel_b = linker.relative_path(&dest_b, &source, false).unwrap();
        assert_eq!(rel_b, PathBuf::from("../source.md"));
    }

    #[test]
    #[cfg(unix)]
    fn sync_retains_path_cache_entries_within_run() {
        let temp = TempDir::new().unwrap();
        let (linker, links_dir) = make_contents_linker(temp.path());

        let result = linker.sync(&super::super::SyncOptions::default()).unwrap();
        assert_eq!(result.created, 4);

        // The last create_symlink only invalidates its own dest key; the
        // from_dir and source entries computed for it must survive the run.
        let cache = linker.path_cache.borrow();
        assert!(
            !cache.is_empty(),
            "scoped invalidation must leave cached entries within a run"
        );
        assert!(
            cache.contains_key(&links_dir),
            "from_dir must remain cached after the run's last mutation"
        );
    }

    #[test]
    #[cfg(unix)]
    fn sync_clears_path_cache_between_runs() {
        let temp = TempDir::new().unwrap();
        let (linker, links_dir) = make_contents_linker(temp.path());

        let first_source = temp.path().join(".agents/flat/f0000.md");
        let first_dest = links_dir.join("f0000.md");
        let expected_target = PathBuf::from("../.agents/flat/f0000.md");

        let result1 = linker.sync(&super::super::SyncOptions::default()).unwrap();
        assert_eq!(result1.created, 4);
        assert_eq!(fs::read_link(&first_dest).unwrap(), expected_target);

        // Poison the cache with wrong canonical identities, simulating stale
        // entries a previous run could have left behind.
        {
            let mut cache = linker.path_cache.borrow_mut();
            cache.insert(
                first_source.clone(),
                Rc::new(PathBuf::from("/wrong/canonical/source")),
            );
            cache.insert(
                links_dir.clone(),
                Rc::new(PathBuf::from("/wrong/canonical/dir")),
            );
        }

        // Run 2 must clear the cache at sync() start: the poison entries must
        // never be served, so the links keep their correct targets.
        let result2 = linker.sync(&super::super::SyncOptions::default()).unwrap();
        assert_eq!(result2.created, 0, "second run must skip correct links");
        assert_eq!(
            fs::read_link(&first_dest).unwrap(),
            expected_target,
            "run 2 must not serve run 1's cache entries"
        );
    }
}
