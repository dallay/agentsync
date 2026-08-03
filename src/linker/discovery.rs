//! Nested-glob discovery and destination-template implementation.

use anyhow::Result;
use colored::Colorize;
use std::path::Path;
use std::rc::Rc;
use walkdir::WalkDir;

use super::{Linker, NestedGlobKey, NestedGlobMatches, ResolvedSource, SyncOptions, SyncResult};

impl Linker {
    /// Expands destination placeholders using a discovered file's path relative to the search root.
    ///
    /// Supported placeholders are `{relative_path}`, `{file_name}`, `{stem}`, and `{ext}`.
    /// Files directly inside the search root use `.` for `{relative_path}`. Unknown
    /// placeholders and unmatched opening braces remain unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let destination = expand_destination_template(
    ///     "{relative_path}/{stem}.{ext}",
    ///     Path::new("clients/agent-runtime/AGENTS.md"),
    /// );
    ///
    /// assert_eq!(destination, "clients/agent-runtime/AGENTS.md");
    /// ```
    pub(super) fn expand_destination_template(
        template: &str,
        rel_path: &Path, // path of the discovered file relative to search root
    ) -> String {
        // Optimization: use Cow<str> from to_string_lossy() to avoid heap allocations
        // for valid UTF-8 paths.
        let file_name = rel_path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();

        let stem = rel_path
            .file_stem()
            .map(|s| s.to_string_lossy())
            .unwrap_or_default();

        let ext = rel_path
            .extension()
            .map(|e| e.to_string_lossy())
            .unwrap_or_default();

        // Use "." for files directly inside the search root so that
        // templates like "{relative_path}/CLAUDE.md" produce a valid relative
        // path ("./CLAUDE.md") rather than an absolute path ("/CLAUDE.md").
        let dir = rel_path
            .parent()
            .map(|p| {
                let s = p.to_string_lossy();
                if s.is_empty() {
                    std::borrow::Cow::Borrowed(".")
                } else {
                    s
                }
            })
            .unwrap_or(std::borrow::Cow::Borrowed("."));

        // Performance Optimization: single-pass expansion with String::with_capacity
        // to eliminate O(N) heap allocations from multiple .replace() calls.
        let mut result = String::with_capacity(template.len() + dir.len() + file_name.len());
        let mut remaining = template;

        while let Some(start_idx) = remaining.find('{') {
            result.push_str(&remaining[..start_idx]);
            let after_brace = &remaining[start_idx..];

            if let Some(end_idx) = after_brace.find('}') {
                let placeholder = &after_brace[..end_idx + 1];
                match placeholder {
                    "{relative_path}" => result.push_str(&dir),
                    "{file_name}" => result.push_str(&file_name),
                    "{stem}" => result.push_str(&stem),
                    "{ext}" => result.push_str(&ext),
                    _ => result.push_str(placeholder),
                }
                remaining = &after_brace[end_idx + 1..];
            } else {
                result.push('{');
                remaining = &after_brace[1..];
            }
        }
        result.push_str(remaining);
        result
    }

    /// Discovers files matching a nested glob and creates symlinks at their expanded destinations.
    ///
    /// Invalid search roots and individual matches with empty or unsafe destinations are skipped.
    /// Discovery and symlink errors are returned.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let result = linker.process_nested_glob(
    ///     Path::new("assets"),
    ///     "**/*.png",
    ///     &[],
    ///     "public/{relative_path}",
    ///     &options,
    /// )?;
    /// assert!(result.created + result.updated + result.skipped > 0);
    /// # Ok::<(), YourError>(())
    /// ```
    ///
    /// `search_root` is the directory in which to search. `glob_pattern` selects matching files,
    /// `excludes` omits matching paths, and `dest_template` determines each symlink destination.
    /// `options` controls synchronization behaviour and output.
    pub(super) fn process_nested_glob
    pub(super) fn process_nested_glob(
        &self,
        search_root: &Path,
        glob_pattern: &str,
        excludes: &[String],
        dest_template: &str,
        options: &SyncOptions,
    ) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        if !search_root.exists() || !search_root.is_dir() {
            println!(
                "  {} Search root does not exist: {}",
                "!".yellow(),
                search_root.display()
            );
            result.skipped += 1;
            return Ok(result);
        }

        let matches = self.get_nested_glob_matches(search_root, glob_pattern, excludes, options)?;

        for (full_path, rel_path) in matches.iter() {
            let dest_str = Self::expand_destination_template(dest_template, rel_path);
            if dest_str.is_empty() {
                if options.verbose {
                    println!(
                        "  {} Destination template produced empty path for: {}",
                        "!".yellow(),
                        full_path.display()
                    );
                }
                result.skipped += 1;
                continue;
            }

            let dest = match self.ensure_safe_destination(&dest_str) {
                Ok(dest) => dest,
                Err(err) => {
                    if options.verbose {
                        println!(
                            "  {} Skipping nested-glob destination {}: {}",
                            "!".yellow(),
                            dest_str,
                            err
                        );
                    }
                    result.skipped += 1;
                    continue;
                }
            };

            let resolved = ResolvedSource {
                path: full_path.to_path_buf(),
                exists: true,
            };

            let item_result = self.create_symlink(&resolved, &dest, options)?;
            result.created += item_result.created;
            result.updated += item_result.updated;
            result.skipped += item_result.skipped;
        }

        Ok(result)
    }

    /// Retrieves nested-glob discovery results, reusing results cached for the same search root, pattern, and exclusions.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let matches = linker.get_nested_glob_matches(
    ///     search_root,
    ///     glob_pattern,
    ///     excludes,
    ///     options,
    /// )?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Returns
    ///
    /// The discovered matching paths and their relative paths.
    ///
    /// # Errors
    ///
    /// Returns an error if nested-glob discovery fails.
    pub(super) fn get_nested_glob_matches(
        &self,
        search_root: &Path,
        glob_pattern: &str,
        excludes: &[String],
        options: &SyncOptions,
    ) -> Result<NestedGlobMatches> {
        let key: NestedGlobKey = (
            search_root.to_path_buf(),
            glob_pattern.to_string(),
            excludes.to_vec(),
        );

        let mut cache = self.glob_cache.borrow_mut();
        if let Some(cached) = cache.get(&key) {
            return Ok(Rc::clone(cached));
        }

        let mut found = Vec::new();
        self.for_each_nested_glob_match(
            search_root,
            glob_pattern,
            excludes,
            options,
            |full_path, rel_path| {
                found.push((full_path.to_path_buf(), rel_path.to_path_buf()));
                Ok(())
            },
        )?;
        let rc_found = Rc::new(found);
        cache.insert(key, Rc::clone(&rc_found));
        Ok(rc_found)
    }

    /// Visits each file under `search_root` that matches the nested glob pattern.
    ///
    /// Excluded paths are skipped, matching directories are not traversed, and the
    /// callback receives each matching file's full and root-relative paths.
    ///
    /// # Errors
    ///
    /// Returns an error if the callback returns an error.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// linker.for_each_nested_glob_match(
    ///     Path::new("templates"),
    ///     "**/*.hbs",
    ///     &[],
    ///     &options,
    ///     |full_path, relative_path| {
    ///         println!("{} -> {}", relative_path.display(), full_path.display());
    ///         Ok(())
    ///     },
    /// )?;
    /// ```
    fn for_each_nested_glob_match<F>(
        &self,
        search_root: &Path,
        glob_pattern: &str,
        excludes: &[String],
        options: &SyncOptions,
        mut on_match: F,
    ) -> Result<()>
    where
        F: FnMut(&Path, &Path) -> Result<()>,
    {
        // Optimization: Pre-split patterns once to avoid redundant allocations in the walk loop.
        let split_pattern: Vec<&str> = glob_pattern.split('/').collect();
        let split_excludes: Vec<Vec<&str>> =
            excludes.iter().map(|e| e.split('/').collect()).collect();

        let mut it = WalkDir::new(search_root).follow_links(false).into_iter();

        while let Some(entry) = it.next() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    if options.verbose {
                        let path = err
                            .path()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "<unknown path>".to_string());
                        println!(
                            "  {} WalkDir error while traversing {}: {}",
                            "!".yellow(),
                            path,
                            err
                        );
                    }
                    tracing::debug!(error = %err, path = ?err.path(), "WalkDir entry skipped during nested-glob traversal");
                    continue;
                }
            };

            let full_path = entry.path();
            let rel_path = match full_path.strip_prefix(search_root) {
                Ok(path) => path,
                Err(_) => continue,
            };

            // Performance: generating a platform-agnostic relative path for glob matching.
            // On Unix, this is zero-allocation (Cow::Borrowed).
            // On Windows, it performs exactly one allocation for the backslash replacement.
            let rel_os_str = rel_path.to_string_lossy();
            let rel_str = if std::path::MAIN_SEPARATOR == '/' {
                rel_os_str
            } else {
                std::borrow::Cow::Owned(rel_os_str.replace(std::path::MAIN_SEPARATOR, "/"))
            };

            if rel_str.is_empty() {
                continue;
            }

            let path_it = rel_str.split('/');
            if let Some(idx) = split_excludes
                .iter()
                .position(|exclude_parts| path_glob_match_iter(path_it.clone(), exclude_parts))
            {
                let matched_exclude = &excludes[idx];
                if options.verbose {
                    println!(
                        "  {} Excluded by '{}': {}",
                        "○".yellow(),
                        matched_exclude,
                        full_path.display()
                    );
                }

                if entry.file_type().is_dir() {
                    it.skip_current_dir();
                }
                continue;
            }

            if !entry.file_type().is_file() {
                continue;
            }

            if !path_glob_match_iter(path_it, &split_pattern) {
                continue;
            }

            on_match(full_path, rel_path)?;
        }

        Ok(())
    }
}

/// Matches a path segment against a glob pattern using `*` for zero or more
/// characters and `?` for exactly one character.
///
/// # Examples
///
/// ```
/// assert!(matches_pattern("file.txt", "*.txt"));
/// assert!(matches_pattern("a1", "a?"));
/// assert!(!matches_pattern("file.rs", "*.txt"));
/// ```
pub(super) fn matches_pattern(name: &str, pattern: &str) -> bool {
    let mut name_it = name.chars();
    let mut pattern_it = pattern.chars();

    let mut star_p_it = None;
    let mut star_n_it = None;

    loop {
        let s_char = name_it.clone().next();
        let p_char = pattern_it.clone().next();

        match (s_char, p_char) {
            (Some(s), Some(p)) if p == s || p == '?' => {
                name_it.next();
                pattern_it.next();
            }
            (_, Some('*')) => {
                pattern_it.next();
                star_p_it = Some(pattern_it.clone());
                star_n_it = Some(name_it.clone());
            }
            (Some(_), _) => {
                if let (Some(star_p), Some(star_n)) = (star_p_it.as_mut(), star_n_it.as_mut()) {
                    if star_n.next().is_none() {
                        return false;
                    }
                    name_it = star_n.clone();
                    pattern_it = star_p.clone();
                } else {
                    return false;
                }
            }
            (None, _) => return pattern_it.all(|c| c == '*'),
        }
    }
}

/// Matches a slash-separated path against a glob pattern with `**` support.
///
/// The path and pattern use `/` as the separator. The `*` and `?` wildcards
/// match within a single path segment, while `**` matches zero or more segments.
///
/// # Examples
///
/// ```
/// assert!(matches_path_glob("src/lib.rs", "src/*.rs"));
/// assert!(matches_path_glob("src/nested/lib.rs", "src/**/*.rs"));
/// assert!(!matches_path_glob("tests/lib.rs", "src/**/*.rs"));
/// ```
///
/// Returns `true` if the path matches the pattern, `false` otherwise.
#[cfg(test)]
pub(super) fn matches_path_glob(path: &str, pattern: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    path_glob_match_iter(path.split('/'), &pattern_parts)
}

/// Matches path segments against a glob pattern, supporting `*`, `?`, and `**`.
///
/// # Examples
///
/// ```
/// let path = ["src", "nested", "main.rs"];
/// let pattern = ["src", "**", "*.rs"];
///
/// assert!(path_glob_match_iter(path.into_iter(), &pattern));
/// ```
pub(super) fn path_glob_match_iter<'a, I>(mut path_it: I) -> bool
pub(super) fn path_glob_match_iter<'a, I>(mut path_it: I, pattern: &[&str]) -> bool
where
    I: Iterator<Item = &'a str> + Clone,
{
    let mut pat_idx = 0;
    let mut backtrack_path_it: Option<I> = None;
    let mut backtrack_pat_idx = None;

    loop {
        let mut path_it_peek = path_it.clone();
        match path_it_peek.next() {
            Some(s) => {
                if !try_match_segment(
                    s,
                    &mut path_it,
                    pattern,
                    &mut pat_idx,
                    &mut backtrack_path_it,
                    &mut backtrack_pat_idx,
                ) {
                    return false;
                }
            }
            None => {
                while pat_idx < pattern.len() && pattern[pat_idx] == "**" {
                    pat_idx += 1;
                }
                return pat_idx == pattern.len();
            }
        }
    }
}

/// Advances path-pattern matching for one path segment, including `**`
/// backtracking state.
///
/// # Arguments
///
/// * `segment` - The path segment to match.
/// * `path_it` - The remaining path segments.
/// * `pattern` - The remaining path-pattern segments.
/// * `pat_idx` - The current pattern position.
/// * `backtrack_path_it` - Saved path position for `**` backtracking.
/// * `backtrack_pat_idx` - Saved pattern position for `**` backtracking.
///
/// # Returns
///
/// `true` if the segment can be matched and the matching state is advanced,
/// `false` if no valid match exists.
///
/// # Examples
///
/// ```
/// let mut path_it = "file.txt".split('/');
/// let pattern = ["*.txt"];
/// let mut pat_idx = 0;
/// let mut backtrack_path_it = None;
/// let mut backtrack_pat_idx = None;
///
/// assert!(try_match_segment(
///     "file.txt",
///     &mut path_it,
///     &pattern,
///     &mut pat_idx,
///     &mut backtrack_path_it,
///     &mut backtrack_pat_idx,
/// ));
/// assert_eq!(pat_idx, 1);
/// ```
fn try_match_segment<'a, I>(
    segment: &str,
    path_it: &mut I,
    pattern: &[&str],
    pat_idx: &mut usize,
    backtrack_path_it: &mut Option<I>,
    backtrack_pat_idx: &mut Option<usize>,
) -> bool
where
    I: Iterator<Item = &'a str> + Clone,
{
    if *pat_idx < pattern.len() && pattern[*pat_idx] == "**" {
        // ** matches zero or more segments
        *backtrack_pat_idx = Some(*pat_idx);
        *backtrack_path_it = Some(path_it.clone());
        *pat_idx += 1;
    } else if *pat_idx < pattern.len() && matches_pattern(segment, pattern[*pat_idx]) {
        path_it.next(); // Consume segment
        *pat_idx += 1;
    } else if let (Some(b_pat_idx), Some(b_path_it)) =
        (*backtrack_pat_idx, backtrack_path_it.as_mut())
    {
        // Backtrack: last ** matches one more segment
        if b_path_it.next().is_none() {
            return false;
        }
        *path_it = b_path_it.clone();
        *pat_idx = b_pat_idx + 1;
    } else {
        return false;
    }
    true
}
