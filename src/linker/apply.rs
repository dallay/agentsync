//! Apply orchestration and source-resolution implementation.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::config::{SyncType, TargetConfig};

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
            // Skip disabled agents
            if !agent_config.enabled {
                if options.verbose {
                    println!("  {} Skipping disabled agent: {}", "○".yellow(), agent_name);
                }
                continue;
            }

            // Filter by agent name if specified
            // Priority: CLI --agents flag > default_agents config > all enabled agents
            if let Some(ref filter) = options.agents {
                if !filter
                    .iter()
                    .any(|f| crate::agent_ids::sync_filter_matches(agent_name, f))
                {
                    if options.verbose {
                        println!("  {} Skipping filtered agent: {}", "○".yellow(), agent_name);
                    }
                    continue;
                }
            } else if !self.config.default_agents.is_empty()
                && !self
                    .config
                    .default_agents
                    .iter()
                    .any(|f| crate::agent_ids::sync_filter_matches(agent_name, f))
            {
                if options.verbose {
                    println!(
                        "  {} Skipping agent (not in default_agents): {}",
                        "○".yellow(),
                        agent_name
                    );
                }
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

            // Process each target
            for (target_name, target_config) in &agent_config.targets {
                if options.verbose {
                    println!("  Processing target: {}", target_name.dimmed());
                }

                match self.process_target(agent_name, target_config, options) {
                    Ok(target_result) => {
                        result.created += target_result.created;
                        result.updated += target_result.updated;
                        result.skipped += target_result.skipped;
                    }
                    Err(e) => {
                        tracing::error!(target = %target_name, error = %e, "Error processing target");
                        result.errors += 1;
                    }
                }
            }
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
            exists: source.exists(),
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
        self.invalidate_path_cache();
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

            // Resolve destination filename
            let filename = crate::config::resolve_module_map_filename(mapping, agent_name);

            // SECURITY: Validate that the joined destination (dir + filename) is safe.
            let dest_str = format!("{}/{}", mapping.destination, filename);
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
        }

        Ok(result)
    }
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
