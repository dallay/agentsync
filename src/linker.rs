//! Symbolic link creation and management
//!
//! Handles creating, updating, and removing symbolic links
//! for AI agent configuration synchronization.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::config::{Config, SyncType, TargetConfig};

/// Options for the sync operation
#[derive(Debug, Clone)]
pub struct SyncOptions {
    /// Remove existing symlinks before creating new ones
    pub clean: bool,
    /// Show what would be done without making changes
    pub dry_run: bool,
    /// Show detailed output
    pub verbose: bool,
    /// Filter to specific agents
    pub agents: Option<Vec<String>>,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            clean: false,
            dry_run: false,
            verbose: false,
            agents: None,
        }
    }
}

/// Result of a sync operation
#[derive(Debug, Default)]
pub struct SyncResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub removed: usize,
    pub errors: usize,
}

/// Performs the synchronization of agent configurations
pub struct Linker {
    config: Config,
    #[allow(dead_code)]
    config_path: PathBuf,
    project_root: PathBuf,
    source_dir: PathBuf,
}

impl Linker {
    /// Create a new linker from a configuration
    pub fn new(config: Config, config_path: PathBuf) -> Self {
        let project_root = Config::project_root(&config_path);
        let source_dir = config.source_dir(&config_path);

        Self {
            config,
            config_path,
            project_root,
            source_dir,
        }
    }

    /// Get the project root path
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Get the config
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Perform the sync operation
    pub fn sync(&self, options: &SyncOptions) -> Result<SyncResult> {
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
            if let Some(ref filter) = options.agents {
                if !filter.iter().any(|f| agent_name.contains(f)) {
                    if options.verbose {
                        println!("  {} Skipping filtered agent: {}", "○".yellow(), agent_name);
                    }
                    continue;
                }
            }

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

                match self.process_target(target_config, options) {
                    Ok(target_result) => {
                        result.created += target_result.created;
                        result.updated += target_result.updated;
                        result.skipped += target_result.skipped;
                    }
                    Err(e) => {
                        eprintln!("  {} Error processing {}: {}", "✘".red(), target_name, e);
                        result.errors += 1;
                    }
                }
            }
        }

        Ok(result)
    }

    /// Process a single target configuration
    fn process_target(&self, target: &TargetConfig, options: &SyncOptions) -> Result<SyncResult> {
        let source = self.source_dir.join(&target.source);
        let dest = self.project_root.join(&target.destination);

        match target.sync_type {
            SyncType::Symlink => self.create_symlink(&source, &dest, options),
            SyncType::SymlinkContents => {
                self.create_symlinks_for_contents(&source, &dest, target.pattern.as_deref(), options)
            }
        }
    }

    /// Create a single symlink
    fn create_symlink(
        &self,
        source: &Path,
        dest: &Path,
        options: &SyncOptions,
    ) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        // Check if source exists
        if !source.exists() {
            println!("  {} Source does not exist: {}", "!".yellow(), source.display());
            result.skipped += 1;
            return Ok(result);
        }

        // Create parent directory if needed
        if let Some(parent) = dest.parent() {
            if !parent.exists() {
                if options.dry_run {
                    if options.verbose {
                        println!("  {} Would create directory: {}", "→".cyan(), parent.display());
                    }
                } else {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
                    if options.verbose {
                        println!("  {} Created directory: {}", "✔".green(), parent.display());
                    }
                }
            }
        }

        // Calculate relative path from dest to source
        let relative_source = self.relative_path(dest, source)?;

        // Handle existing destination
        if dest.is_symlink() {
            let current_target = fs::read_link(dest)?;
            if current_target == relative_source {
                if options.verbose {
                    println!("  {} Already linked: {}", "✔".green(), dest.display());
                }
                result.skipped += 1;
                return Ok(result);
            } else {
                // Wrong target, remove and recreate
                if options.dry_run {
                    println!(
                        "  {} Would update symlink: {} -> {}",
                        "→".cyan(),
                        dest.display(),
                        relative_source.display()
                    );
                } else {
                    fs::remove_file(dest)?;
                    if options.verbose {
                        println!(
                            "  {} Removed old symlink: {} (was -> {})",
                            "○".yellow(),
                            dest.display(),
                            current_target.display()
                        );
                    }
                }
                result.updated += 1;
            }
        } else if dest.exists() {
            // It's a regular file/directory - back it up
            if options.dry_run {
                println!(
                    "  {} Would backup and replace: {}",
                    "→".cyan(),
                    dest.display()
                );
            } else {
                let backup = PathBuf::from(format!(
                    "{}.bak.{}",
                    dest.display(),
                    chrono_lite_timestamp()
                ));
                fs::rename(dest, &backup)?;
                println!(
                    "  {} Backed up: {} -> {}",
                    "!".yellow(),
                    dest.display(),
                    backup.display()
                );
            }
            result.updated += 1;
        } else {
            result.created += 1;
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
            #[cfg(unix)]
            std::os::unix::fs::symlink(&relative_source, dest)
                .with_context(|| format!("Failed to create symlink: {}", dest.display()))?;

            #[cfg(windows)]
            {
                if source.is_dir() {
                    std::os::windows::fs::symlink_dir(&relative_source, dest)?;
                } else {
                    std::os::windows::fs::symlink_file(&relative_source, dest)?;
                }
            }

            println!(
                "  {} Linked: {} -> {}",
                "✔".green(),
                dest.display(),
                relative_source.display()
            );
        }

        Ok(result)
    }

    /// Create symlinks for all contents of a directory
    fn create_symlinks_for_contents(
        &self,
        source_dir: &Path,
        dest_dir: &Path,
        pattern: Option<&str>,
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

        // Create destination directory if needed
        if !dest_dir.exists() {
            if options.dry_run {
                if options.verbose {
                    println!(
                        "  {} Would create directory: {}",
                        "→".cyan(),
                        dest_dir.display()
                    );
                }
            } else {
                fs::create_dir_all(dest_dir)?;
                if options.verbose {
                    println!("  {} Created directory: {}", "✔".green(), dest_dir.display());
                }
            }
        }

        // Iterate through source directory contents
        for entry in WalkDir::new(source_dir).min_depth(1).max_depth(1) {
            let entry = entry?;
            let item_name = entry.file_name().to_string_lossy();

            // Apply pattern filter if specified
            if let Some(pat) = pattern {
                if !matches_pattern(&item_name, pat) {
                    continue;
                }
            }

            let source_path = entry.path();
            let dest_path = dest_dir.join(entry.file_name());

            let item_result = self.create_symlink(source_path, &dest_path, options)?;
            result.created += item_result.created;
            result.updated += item_result.updated;
            result.skipped += item_result.skipped;
        }

        Ok(result)
    }

    /// Calculate relative path from dest to source
    fn relative_path(&self, from: &Path, to: &Path) -> Result<PathBuf> {
        let from_dir = from.parent().unwrap_or(from);

        // Canonicalize paths for accurate relative calculation
        let from_abs = if from_dir.exists() {
            fs::canonicalize(from_dir)?
        } else {
            // If dest dir doesn't exist yet, use project root as base
            let relative = from_dir.strip_prefix(&self.project_root).unwrap_or(from_dir);
            self.project_root.join(relative)
        };

        let to_abs = fs::canonicalize(to)
            .with_context(|| format!("Source path does not exist: {}", to.display()))?;

        // Use pathdiff to calculate relative path
        pathdiff::diff_paths(&to_abs, &from_abs)
            .ok_or_else(|| anyhow::anyhow!("Cannot calculate relative path"))
    }

    /// Clean all symlinks managed by this configuration
    pub fn clean(&self, options: &SyncOptions) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        println!("{}", "Cleaning managed symlinks...".cyan());

        for (_agent_name, agent_config) in &self.config.agents {
            for (_target_name, target_config) in &agent_config.targets {
                let dest = self.project_root.join(&target_config.destination);

                if dest.is_symlink() {
                    if options.dry_run {
                        println!("  {} Would remove: {}", "→".cyan(), dest.display());
                    } else {
                        fs::remove_file(&dest)?;
                        println!("  {} Removed: {}", "✔".green(), dest.display());
                    }
                    result.removed += 1;
                } else if dest.is_dir() && target_config.sync_type == SyncType::SymlinkContents {
                    // For symlink-contents, remove symlinks inside the directory
                    for entry in WalkDir::new(&dest).min_depth(1).max_depth(1) {
                        let entry = entry?;
                        if entry.path().is_symlink() {
                            if options.dry_run {
                                println!("  {} Would remove: {}", "→".cyan(), entry.path().display());
                            } else {
                                fs::remove_file(entry.path())?;
                                println!("  {} Removed: {}", "✔".green(), entry.path().display());
                            }
                            result.removed += 1;
                        }
                    }
                    // Try to remove the directory if empty
                    if !options.dry_run {
                        let _ = fs::remove_dir(&dest);
                    }
                }
            }
        }

        Ok(result)
    }
}

/// Simple glob pattern matching (supports * and ?)
fn matches_pattern(name: &str, pattern: &str) -> bool {
    let mut name_chars = name.chars().peekable();
    let mut pattern_chars = pattern.chars().peekable();

    while let Some(p) = pattern_chars.next() {
        match p {
            '*' => {
                // Match zero or more characters
                if pattern_chars.peek().is_none() {
                    return true; // * at end matches everything
                }
                // Try to match the rest of the pattern
                while name_chars.peek().is_some() {
                    let remaining_name: String = name_chars.clone().collect();
                    let remaining_pattern: String =
                        std::iter::once(pattern_chars.clone()).flatten().collect();
                    if matches_pattern(&remaining_name, &remaining_pattern) {
                        return true;
                    }
                    name_chars.next();
                }
                return false;
            }
            '?' => {
                // Match exactly one character
                if name_chars.next().is_none() {
                    return false;
                }
            }
            c => {
                // Match literal character
                if name_chars.next() != Some(c) {
                    return false;
                }
            }
        }
    }

    name_chars.next().is_none()
}

/// Generate a simple timestamp without external dependencies
fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("test.md", "*.md"));
        assert!(matches_pattern("test.md", "test.*"));
        assert!(matches_pattern("test.md", "test.md"));
        assert!(matches_pattern("test.md", "????.md"));
        assert!(!matches_pattern("test.md", "*.txt"));
        assert!(!matches_pattern("test.md", "foo.*"));
        assert!(matches_pattern("a", "*"));
        assert!(matches_pattern("", "*"));
        assert!(!matches_pattern("", "?"));
    }
}
