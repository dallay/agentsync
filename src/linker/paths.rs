//! Path canonicalization and safety implementation.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

use super::Linker;

impl Linker {
    /// Drop path canonicalization cache after filesystem mutations that can affect
    /// destination safety checks in the same run.
    // `pub(super)` is required by the root façade and future mutation siblings.
    /// Clears cached canonical paths so subsequent path operations reflect filesystem mutations.
    ///
    /// # Examples
    ///
    /// ```
    /// # let linker = /* an initialized Linker */ todo!();
    /// linker.invalidate_path_cache();
    /// ```
    pub(super) fn invalidate_path_cache(&self) {
        self.path_cache.borrow_mut().clear();
    }

    /// Canonicalizes a path without consulting or updating the path cache.
    ///
    /// Adds the path to errors produced when canonicalization fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let canonical = linker.canonicalize_uncached(Path::new("."))?;
    /// assert!(canonical.is_absolute());
    /// ```
    fn canonicalize_uncached(&self, path: &Path) -> Result<PathBuf> {
        fs::canonicalize(path)
            .with_context(|| format!("Failed to canonicalize path: {}", path.display()))
    }

    /// Resolves and caches the canonical project root path.
    ///
    /// Returns an error if the project root cannot be canonicalized.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let canonical_root = linker.get_canonical_project_root()?;
    /// assert!(canonical_root.is_absolute());
    /// ```
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
    /// Validates a destination path and resolves it relative to the project root.
    ///
    /// # Errors
    ///
    /// Returns an error if the destination is absolute, empty, contains invalid
    /// components, or resolves outside the project root.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let destination = linker.ensure_safe_destination("assets/logo.svg")?;
    /// assert!(destination.ends_with("assets/logo.svg"));
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Returns
    ///
    /// The validated destination path joined to the project root.
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
    /// Revalidates a destination path before a filesystem mutation.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn example(linker: &Linker, destination: &Path) -> Result<()> {
    /// linker.revalidate_path(destination)?;
    /// # Ok(())
    /// # }
    /// ```
    pub(super) fn revalidate_path(&self, dest: &Path) -> Result<()> {
        self.ensure_safe_path(dest, &dest.display()).map(|_| ())
    }

    /// Re-validate a path before unlinking (remove_file/remove_dir).
    /// Unlike revalidate_path, this does NOT canonicalize the final component,
    /// allowing safe removal of symlinks that point outside project_root.
    /// The symlink entry itself must be within project_root, but its target can be anywhere.
    // `pub(super)` is required by the root façade and future symlink/clean siblings.
    /// Revalidates a path before it is removed from the filesystem.
    ///
    /// Absolute paths are checked for project-root containment. Relative paths must not
    /// contain parent-directory components and must have a safe existing parent.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn check(linker: &Linker) {
    /// let result = linker.revalidate_unlink_path(Path::new("../outside"));
    /// assert!(result.is_err());
    /// # }
    /// ```
    pub(super) fn revalidate_unlink_path(&self, path: &Path) -> Result<()> {
        let display_path = path.display().to_string();

        // SECURITY: Reject absolute paths
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

    /// Validates an absolute path before unlinking it.
    ///
    /// The path must be within the project root, and its canonicalized parent must
    /// also remain within the canonical project root.
    ///
    /// # Errors
    ///
    /// Returns an error if the path or its canonicalized parent is outside the
    /// project root, or if the parent cannot be canonicalized.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// linker.validate_absolute_unlink_path(path, "/project/file")?;
    /// ```
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

    /// Validates that an existing parent of a relative path resolves within the project root.
    ///
    /// # Arguments
    ///
    /// * `display_path` - Path text included in errors when the parent resolves outside the project root.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// linker.validate_relative_unlink_parent(Path::new("build/output"), "build/output")?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
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

    /// Canonicalizes a path and reuses cached results for repeated lookups.
    ///
    /// Filesystem errors are propagated to the caller.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use std::path::Path;
    ///
    /// let canonical = linker.canonicalize_cached(Path::new("src/lib.rs"))?;
    /// assert_eq!(canonical, linker.canonicalize_cached(Path::new("src/lib.rs"))?);
    /// ```
    fn canonicalize_cached(&self, path: &Path) -> Result<Rc<PathBuf>> {
        let mut cache = self.path_cache.borrow_mut();
        if let Some(cached) = cache.get(path) {
            return Ok(Rc::clone(cached));
        }

        let canonical = Rc::new(fs::canonicalize(path)?);
        cache.insert(path.to_path_buf(), Rc::clone(&canonical));
        Ok(canonical)
    }

    /// Calculate relative path from dest to source
    // `pub(super)` is required by the root façade and future symlink sibling.
    /// Calculates the path to `to` relative to the directory containing `from`.
    ///
    /// Missing destination paths can be resolved when `allow_missing` is `true`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let relative = linker.relative_path(
    ///     std::path::Path::new("src/main.rs"),
    ///     std::path::Path::new("src/lib.rs"),
    ///     false,
    /// )?;
    /// assert_eq!(relative, std::path::PathBuf::from("lib.rs"));
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if either path cannot be resolved when missing paths are
    /// not allowed, or if no relative path can be calculated.
    pub(super) fn relative_path(
        &self,
        from: &Path,
        to: &Path,
        allow_missing: bool,
    ) -> Result<PathBuf> {
        let from_dir = from.parent().unwrap_or(from);

        // Canonicalize paths for accurate relative calculation
        let from_abs = if from_dir.exists() {
            self.canonicalize_cached(from_dir)?
        } else {
            // If dest dir doesn't exist yet, use project root as base
            let relative = from_dir
                .strip_prefix(&self.project_root)
                .unwrap_or(from_dir);
            Rc::new(self.project_root.join(relative))
        };

        let to_abs = match self.canonicalize_cached(to) {
            Ok(path) => path,
            Err(_) if allow_missing => {
                if to.is_absolute() {
                    Rc::new(to.to_path_buf())
                } else {
                    Rc::new(self.project_root.join(to))
                }
            }
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
