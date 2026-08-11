//! Apply orchestration and source-resolution implementation.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::config::{ModuleMapping, SyncType, TargetConfig};

use super::timing::{SpanKind, sync_type_name};
use super::{Linker, ResolvedSource, SyncOptions, SyncResult};

impl Linker {
    pub(super) fn should_compress_agents_md(&self, source: &Path, target: &TargetConfig) -> bool {
        self.config.compress_agents_md
            && matches!(
                target.sync_type,
                SyncType::Symlink | SyncType::SymlinkContents
            )
            && is_agents_md_path(source)
    }

    pub(super) fn compressed_agents_md_path(path: &Path) -> PathBuf {
        path.with_file_name(super::COMPRESSED_AGENTS_MD_NAME)
    }

    /// Perform the sync operation.
    pub fn sync(&self, options: &SyncOptions) -> Result<SyncResult> {
        // Clear caches at the start of every sync run to prevent stale state.
        self.compression_cache.borrow_mut().clear();
        self.ensured_dirs.borrow_mut().clear();
        self.ensured_compressed.borrow_mut().clear();
        self.invalidate_path_cache();
        self.invalidate_glob_cache();
        self.canonical_project_root.borrow_mut().take();

        let mut result = SyncResult::default();

        if options.dry_run {
            println!("{}", "Running in dry-run mode\n".cyan());
        }

        for (agent_name, agent_config) in &self.config.agents {
            let agent_span = tracing::info_span!(
                "agentsync",
                operation = "sync",
                agent_id = %agent_name,
                outcome = tracing::field::Empty
            );
            let _agent_enter = agent_span.enter();

            // Skip disabled agents
            if !agent_config.enabled {
                tracing::debug!(reason = "disabled", "Skipping agent");
                agent_span.record("outcome", "skipped");
                continue;
            }

            // Filter by agent name if specified
            // Priority: CLI --agents flag > default_agents config > all enabled agents
            if let Some(ref filter) = options.agents {
                if !filter
                    .iter()
                    .any(|f| crate::agent_ids::sync_filter_matches(agent_name, f))
                {
                    tracing::debug!(reason = "filtered", "Skipping agent");
                    agent_span.record("outcome", "skipped");
                    continue;
                }
            } else if !self.config.default_agents.is_empty()
                && !self
                    .config
                    .default_agents
                    .iter()
                    .any(|f| crate::agent_ids::sync_filter_matches(agent_name, f))
            {
                tracing::debug!(reason = "not in default_agents", "Skipping agent");
                agent_span.record("outcome", "skipped");
                continue;
            }
            // If neither --agents nor default_agents, process all enabled agents

            // Print agent header
            let desc = if agent_config.description.is_empty() {
                String::new()
            } else {
                format!(" - {}", agent_config.description)
            };
            println!("\n{}{}", agent_name.bold(), desc.dimmed());

            let mut agent_errors = 0usize;
            let mut agent_created = 0usize;
            let mut agent_updated = 0usize;

            // Process each target
            for (target_name, target_config) in &agent_config.targets {
                let target_span = tracing::info_span!(
                    "agentsync",
                    operation = "sync",
                    agent_id = %agent_name,
                    target = %target_name,
                    path = %target_config.destination,
                    outcome = tracing::field::Empty
                );
                let _target_enter = target_span.enter();

                tracing::debug!(target = %target_name, "Processing target");

                match self.process_target(agent_name, target_config, options) {
                    Ok(target_result) => {
                        result.created += target_result.created;
                        result.updated += target_result.updated;
                        result.skipped += target_result.skipped;
                        result.errors += target_result.errors;
                        agent_errors += target_result.errors;
                        agent_created += target_result.created;
                        agent_updated += target_result.updated;
                        let outcome = if target_result.errors > 0 {
                            "error"
                        } else if target_result.created > 0 {
                            "created"
                        } else if target_result.updated > 0 {
                            "updated"
                        } else {
                            "skipped"
                        };
                        target_span.record("outcome", outcome);
                    }
                    Err(e) => {
                        tracing::error!(
                            agent_id = %agent_name,
                            target = %target_name,
                            path = %target_config.destination,
                            error = %e,
                            "Error processing target"
                        );
                        target_span.record("outcome", "error");
                        result.errors += 1;
                        agent_errors += 1;
                    }
                }
            }

            let agent_outcome = if agent_errors > 0 {
                "error"
            } else if agent_created + agent_updated > 0 {
                "ok"
            } else {
                "skipped"
            };
            agent_span.record("outcome", agent_outcome);
        }

        Ok(result)
    }

    /// Process a single target configuration.
    fn process_target(
        &self,
        agent_name: &str,
        target: &TargetConfig,
        options: &SyncOptions,
    ) -> Result<SyncResult> {
        let _span = self.timing_span(SpanKind::Target(sync_type_name(target.sync_type)));
        let source = self.source_dir.join(&target.source);

        match target.sync_type {
            SyncType::Symlink => {
                let dest = self.ensure_safe_destination(&target.destination)?;
                self.revalidate_path(&source)?; // SECURITY: Validate source
                let resolved = self.resolve_source_path(&source, target, options)?;
                self.create_symlink(&resolved, &dest, options)
            }
            SyncType::SymlinkContents => {
                let dest = self.ensure_safe_destination(&target.destination)?;
                self.revalidate_path(&source)?; // SECURITY: Validate source
                self.create_symlinks_for_contents(
                    &source,
                    &dest,
                    target.pattern.as_deref(),
                    target,
                    options,
                )
            }
            SyncType::NestedGlob => {
                let _ = self.ensure_safe_destination(&target.destination)?;
                let search_root = self.project_root.join(&target.source);
                self.revalidate_path(&search_root).with_context(|| {
                    format!(
                        "NestedGlob source resolves outside project root: {}",
                        target.source
                    )
                })?;

                self.process_nested_glob(
                    &search_root,
                    target.pattern.as_deref().unwrap_or("**/AGENTS.md"),
                    &target.exclude,
                    &target.destination,
                    options,
                )
            }
            SyncType::ModuleMap => self.process_module_map(agent_name, target, options),
        }
    }

    pub(super) fn resolve_source_path(
        &self,
        source: &Path,
        target: &TargetConfig,
        options: &SyncOptions,
    ) -> Result<ResolvedSource> {
        self.resolve_source_path_with_hint(source, target, options, None)
    }

    /// Resolve a source path for linking, with an optional existence hint.
    ///
    /// `source_exists` lets callers that already proved the source exists
    /// (e.g. a `symlink-contents` loop iterating `fs::read_dir` entries) skip
    /// the duplicate per-child `source.exists()` stat — REQ: Reuse DirEntry
    /// Existence in Symlink-Contents.
    ///
    /// * `Some(true)` asserts existence WITHOUT probing (sound only when the
    ///   caller knows the path resolves to a real file/dir — never for a
    ///   symlink, whose `exists()` follows the target and may be false).
    /// * `Some(false)` asserts absence without probing.
    /// * `None` (the default via [`Self::resolve_source_path`]) probes.
    ///
    /// The compression path always probes the source itself and is unchanged.
    pub(super) fn resolve_source_path_with_hint(
        &self,
        source: &Path,
        target: &TargetConfig,
        options: &SyncOptions,
        source_exists: Option<bool>,
    ) -> Result<ResolvedSource> {
        if self.should_compress_agents_md(source, target) {
            if !source.exists() {
                return Ok(ResolvedSource {
                    path: source.to_path_buf(),
                    exists: false,
                });
            }

            let compressed = Self::compressed_agents_md_path(source);

            if !options.dry_run {
                self.write_compressed_agents_md(source, &compressed, options)?;
            }

            let exists = options.dry_run || compressed.exists();
            return Ok(ResolvedSource {
                path: compressed,
                exists,
            });
        }

        Ok(ResolvedSource {
            path: source.to_path_buf(),
            exists: source_exists.unwrap_or_else(|| source.exists()),
        })
    }

    pub(super) fn write_compressed_agents_md(
        &self,
        source: &Path,
        dest: &Path,
        options: &SyncOptions,
    ) -> Result<()> {
        // Optimization: skip if this output file was already ensured in this run.
        if self.ensured_compressed.borrow().contains(dest) {
            return Ok(());
        }

        let compressed = {
            let mut cache = self.compression_cache.borrow_mut();
            if let Some(cached) = cache.get(source) {
                Rc::clone(cached)
            } else {
                let content = fs::read_to_string(source)
                    .with_context(|| format!("Failed to read AGENTS.md: {}", source.display()))?;
                // Optimization: cache compressed AGENTS.md as Rc<str> so every agent sharing the
                // same source reuses one allocation. Previously the cache returned cloned Strings,
                // copying the full compressed content once per target before the eventual write.
                let compressed: Rc<str> = Rc::from(compress_agents_md_content(&content));
                cache.insert(source.to_path_buf(), Rc::clone(&compressed));
                compressed
            }
        };

        // SECURITY: Validate destination path before any filesystem operation
        self.revalidate_path(dest)?;

        if let Ok(existing) = fs::read_to_string(dest)
            && existing.as_str() == compressed.as_ref()
        {
            self.ensured_compressed
                .borrow_mut()
                .insert(dest.to_path_buf());
            return Ok(());
        }

        if let Some(parent) = dest.parent() {
            self.ensure_directory(parent, options)?;
        }
        fs::write(dest, compressed.as_bytes())
            .with_context(|| format!("Failed to write compressed AGENTS.md: {}", dest.display()))?;
        self.invalidate_path(dest);
        self.invalidate_glob_cache();

        self.ensured_compressed
            .borrow_mut()
            .insert(dest.to_path_buf());
        Ok(())
    }

    /// Process a `module-map` target: iterate mappings and create a symlink
    /// for each one, resolving the destination filename from:
    /// 1. mapping.filename_override (explicit user choice)
    /// 2. agent_convention_filename (per-agent convention)
    /// 3. source file basename (fallback)
    fn process_module_map(
        &self,
        agent_name: &str,
        target: &TargetConfig,
        options: &SyncOptions,
    ) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        if target.mappings.is_empty() {
            if options.verbose {
                println!(
                    "  {} No mappings defined for module-map target",
                    "!".yellow()
                );
            }
            return Ok(result);
        }

        for mapping in &target.mappings {
            let source_path = self.source_dir.join(&mapping.source);
            self.revalidate_path(&source_path)?; // SECURITY: Validate source

            // SECURITY: Validate that the joined destination (dir + filename) is safe.
            let dest_str = module_map_destination(mapping, agent_name);
            let dest = match self.ensure_safe_destination(&dest_str) {
                Ok(d) => d,
                Err(e) => {
                    if options.verbose {
                        println!(
                            "  {} Skipping mapping {}: {}",
                            "!".yellow(),
                            mapping.source,
                            e
                        );
                    }
                    result.skipped += 1;
                    continue;
                }
            };

            let resolved = ResolvedSource {
                path: source_path.clone(),
                exists: source_path.exists(),
            };

            let item_result = self.create_symlink(&resolved, &dest, options)?;
            result.created += item_result.created;
            result.updated += item_result.updated;
            result.skipped += item_result.skipped;
            result.errors += item_result.errors;
        }

        Ok(result)
    }
}

pub(super) fn module_map_destination(mapping: &ModuleMapping, agent_name: &str) -> String {
    let filename = crate::config::resolve_module_map_filename(mapping, agent_name);
    format!("{}/{}", mapping.destination, filename)
}

fn is_agents_md_path(path: &Path) -> bool {
    path.file_name() == Some(std::ffi::OsStr::new("AGENTS.md"))
}

/// Detect a code fence delimiter (``` or ~~~) at the start of a trimmed line.
fn detect_fence_delimiter(trimmed_start: &str) -> Option<&str> {
    if trimmed_start.starts_with("```") {
        let len = trimmed_start
            .find(|c| c != '`')
            .unwrap_or(trimmed_start.len());
        Some(&trimmed_start[..len])
    } else if trimmed_start.starts_with("~~~") {
        let len = trimmed_start
            .find(|c| c != '~')
            .unwrap_or(trimmed_start.len());
        Some(&trimmed_start[..len])
    } else {
        None
    }
}

/// Toggle fence state: open a new fence, close a matching one, or leave unchanged.
fn toggle_fence<'a>(current: Option<&'a str>, delim: &'a str) -> Option<&'a str> {
    match current {
        None => Some(delim),
        Some(open) if open == delim => None,
        other => other,
    }
}

fn compress_agents_md_content(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut fence_delim: Option<&str> = None;
    let mut previous_blank = false;

    for line in input.lines() {
        let trimmed_end = line.trim_end_matches([' ', '\t']);
        let trimmed_start = trimmed_end.trim_start();

        if let Some(delim) = detect_fence_delimiter(trimmed_start) {
            fence_delim = toggle_fence(fence_delim, delim);
            out.push_str(trimmed_end);
            out.push('\n');
            previous_blank = false;
            continue;
        }

        if fence_delim.is_some() {
            out.push_str(trimmed_end);
            out.push('\n');
            previous_blank = false;
            continue;
        }

        if trimmed_end.trim().is_empty() {
            if !previous_blank {
                out.push('\n');
                previous_blank = true;
            }
            continue;
        }

        previous_blank = false;
        let (leading, rest) = split_leading_whitespace(trimmed_end);
        out.push_str(leading);
        normalize_inline_whitespace_to(rest, &mut out);
        out.push('\n');
    }

    out
}

fn split_leading_whitespace(line: &str) -> (&str, &str) {
    // Performance: Use byte operations to avoid UTF-8 decoding overhead
    // for common leading ASCII whitespace.
    let idx = line
        .as_bytes()
        .iter()
        .position(|&b| b != b' ' && b != b'\t')
        .unwrap_or(line.len());
    line.split_at(idx)
}

fn normalize_inline_whitespace_to(line: &str, out: &mut String) {
    let mut in_whitespace = false;

    for ch in line.chars() {
        if ch == ' ' || ch == '\t' {
            if !in_whitespace {
                out.push(' ');
                in_whitespace = true;
            }
        } else {
            in_whitespace = false;
            out.push(ch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // is_agents_md_path / compressed_agents_md_path
    // ==========================================================================

    #[test]
    fn is_agents_md_path_matches_exact_filename_only() {
        assert!(is_agents_md_path(Path::new("AGENTS.md")));
        assert!(is_agents_md_path(Path::new("/some/nested/AGENTS.md")));
        assert!(!is_agents_md_path(Path::new("agents.md")));
        assert!(!is_agents_md_path(Path::new("AGENTS.MD")));
        assert!(!is_agents_md_path(Path::new("OTHER.md")));
    }

    #[test]
    fn compressed_agents_md_path_replaces_file_name() {
        let path = Path::new("/project/.agents/AGENTS.md");

        assert_eq!(
            Linker::compressed_agents_md_path(path),
            PathBuf::from("/project/.agents/AGENTS.compact.md")
        );
    }

    // ==========================================================================
    // module_map_destination
    // ==========================================================================

    #[test]
    fn module_map_destination_joins_directory_and_resolved_filename() {
        let mapping = ModuleMapping {
            source: "shared/context.md".to_string(),
            destination: "src/api".to_string(),
            filename_override: None,
        };

        assert_eq!(
            module_map_destination(&mapping, "claude"),
            "src/api/CLAUDE.md"
        );
    }

    #[test]
    fn module_map_destination_honors_filename_override() {
        let mapping = ModuleMapping {
            source: "shared/context.md".to_string(),
            destination: "src/api".to_string(),
            filename_override: Some("CUSTOM.md".to_string()),
        };

        assert_eq!(
            module_map_destination(&mapping, "claude"),
            "src/api/CUSTOM.md"
        );
    }

    #[test]
    fn module_map_destination_falls_back_to_source_basename_for_unknown_agent() {
        let mapping = ModuleMapping {
            source: "shared/context.md".to_string(),
            destination: "src/api".to_string(),
            filename_override: None,
        };

        assert_eq!(
            module_map_destination(&mapping, "some-unknown-agent"),
            "src/api/context.md"
        );
    }

    // ==========================================================================
    // fence detection / toggling
    // ==========================================================================

    #[test]
    fn detect_fence_delimiter_recognizes_backtick_and_tilde_fences() {
        assert_eq!(detect_fence_delimiter("```"), Some("```"));
        assert_eq!(detect_fence_delimiter("```rust"), Some("```"));
        assert_eq!(detect_fence_delimiter("````"), Some("````"));
        assert_eq!(detect_fence_delimiter("~~~"), Some("~~~"));
        assert_eq!(detect_fence_delimiter("~~~~"), Some("~~~~"));
    }

    #[test]
    fn detect_fence_delimiter_ignores_non_fence_lines() {
        assert_eq!(detect_fence_delimiter("plain text"), None);
        assert_eq!(detect_fence_delimiter("``code``"), None);
        assert_eq!(detect_fence_delimiter(""), None);
    }

    #[test]
    fn toggle_fence_opens_and_closes_matching_delimiter() {
        assert_eq!(toggle_fence(None, "```"), Some("```"));
        assert_eq!(toggle_fence(Some("```"), "```"), None);
    }

    #[test]
    fn toggle_fence_ignores_mismatched_delimiter_while_open() {
        // A ~~~ fence encountered while a ``` fence is open must not close it.
        assert_eq!(toggle_fence(Some("```"), "~~~"), Some("```"));
    }

    // ==========================================================================
    // whitespace helpers
    // ==========================================================================

    #[test]
    fn split_leading_whitespace_separates_indentation_from_content() {
        assert_eq!(split_leading_whitespace("   foo"), ("   ", "foo"));
        assert_eq!(split_leading_whitespace("\t\tfoo"), ("\t\t", "foo"));
        assert_eq!(split_leading_whitespace("foo"), ("", "foo"));
        assert_eq!(split_leading_whitespace(""), ("", ""));
        assert_eq!(split_leading_whitespace("   "), ("   ", ""));
    }

    #[test]
    fn normalize_inline_whitespace_to_collapses_runs_of_whitespace() {
        let mut out = String::new();
        normalize_inline_whitespace_to("a   b\t\tc", &mut out);
        assert_eq!(out, "a b c");
    }

    #[test]
    fn normalize_inline_whitespace_to_appends_to_existing_content() {
        let mut out = String::from("prefix-");
        normalize_inline_whitespace_to("x  y", &mut out);
        assert_eq!(out, "prefix-x y");
    }

    // ==========================================================================
    // compress_agents_md_content
    // ==========================================================================

    #[test]
    fn compress_agents_md_content_collapses_blank_lines_and_whitespace() {
        let input = "# Title\n\n\n\nSome   text  with   spacing.\n\n\nMore.\n";

        let compressed = compress_agents_md_content(input);

        assert_eq!(compressed, "# Title\n\nSome text with spacing.\n\nMore.\n");
    }

    #[test]
    fn compress_agents_md_content_preserves_whitespace_inside_fences() {
        let input = "Text\n```\nfn  main() {\n    let  x = 1;\n}\n```\nAfter\n";

        let compressed = compress_agents_md_content(input);

        assert!(compressed.contains("fn  main() {\n    let  x = 1;\n}"));
        assert!(compressed.contains("Text\n```"));
        assert!(compressed.contains("```\nAfter"));
    }

    #[test]
    fn compress_agents_md_content_preserves_leading_indentation_outside_fences() {
        let input = "- item one\n  - nested   item\n";

        let compressed = compress_agents_md_content(input);

        assert_eq!(compressed, "- item one\n  - nested item\n");
    }

    #[test]
    fn compress_agents_md_content_trims_trailing_whitespace() {
        let input = "line one   \nline two\t\n";

        let compressed = compress_agents_md_content(input);

        assert_eq!(compressed, "line one\nline two\n");
    }

    #[test]
    fn compress_agents_md_content_handles_tilde_fences_like_backtick_fences() {
        let input = "~~~\nspaced   out\n~~~\n";

        let compressed = compress_agents_md_content(input);

        assert_eq!(compressed, "~~~\nspaced   out\n~~~\n");
    }
}
