//! Template generation for init command
//!
//! Provides default configuration templates for new projects and interactive wizard.

use anyhow::Result;
use std::fs;
use std::path::Path;

/// Default configuration template
pub const DEFAULT_CONFIG: &str = r#"# AgentSync Configuration
# This file defines how AI agent configurations are synchronized via symbolic links.
# 
# Source directory (relative to this config file)
source_dir = "."

# Optional: compress AGENTS.md and point symlinks to the compressed file
# compress_agents_md = false

# Gitignore management
[gitignore]
enabled = true
marker = "AI Agent Symlinks"
# Additional entries can be added here:
# entries = ["custom-file.md"]

# =============================================================================
# MCP (Model Context Protocol) Configuration
# =============================================================================
# Define MCP servers once here, and AgentSync will generate the appropriate
# config files for each AI agent (Claude, Copilot, Codex, Cursor, Gemini, VS Code, OpenCode).
#
# [mcp]
# enabled = true
# merge_strategy = "merge"  # "merge" (default) or "overwrite"
#
# [mcp_servers.filesystem]
# command = "npx"
# args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
#
# [mcp_servers.git]
# command = "npx"
# args = ["-y", "@modelcontextprotocol/server-git", "--repository", "."]

# =============================================================================
# Agent Configurations
# =============================================================================
# Each agent has:
#   - enabled: Whether to sync this agent (default: true)
#   - description: Human-readable description
#   - targets: Map of target configurations
#
# Each target has:
#   - source: Path relative to source_dir
#   - destination: Path relative to project root
#   - type: "symlink" or "symlink-contents"
#   - pattern: (optional) Glob pattern for symlink-contents

# -----------------------------------------------------------------------------
# Claude Code
# -----------------------------------------------------------------------------
[agents.claude]
enabled = true
description = "Claude Code - Anthropic's AI coding assistant"

[agents.claude.targets.instructions]
source = "AGENTS.md"
destination = "CLAUDE.md"
type = "symlink"

# -----------------------------------------------------------------------------
# GitHub Copilot
# -----------------------------------------------------------------------------
[agents.copilot]
enabled = true
description = "GitHub Copilot - VS Code and GitHub integrated AI"

[agents.copilot.targets.instructions]
source = "AGENTS.md"
destination = ".github/copilot-instructions.md"
type = "symlink"

# -----------------------------------------------------------------------------
# Cursor
# -----------------------------------------------------------------------------
[agents.cursor]
enabled = true
description = "Cursor - AI code editor and CLI"

# -----------------------------------------------------------------------------
# OpenAI Codex CLI
# -----------------------------------------------------------------------------
[agents.codex]
enabled = true
description = "OpenAI Codex CLI - OpenAI's AI coding agent"

[agents.codex.targets.skills]
source = "skills"
destination = ".codex/skills"
type = "symlink-contents"

# -----------------------------------------------------------------------------
# Root AGENTS.md
# -----------------------------------------------------------------------------
[agents.root]
enabled = true
description = "Root AGENTS.md for tools that look for it in repo root"

[agents.root.targets.agents]
source = "AGENTS.md"
destination = "AGENTS.md"
type = "symlink"
"#;

/// Default AGENTS.md template
pub const DEFAULT_AGENTS_MD: &str = r#"# AI Agent Instructions

> This file provides instructions for AI coding assistants working on this project.

## Project Overview

<!-- Describe your project here -->

## Code Style

<!-- Describe your coding conventions -->

## Architecture

<!-- Describe your project architecture -->

## Testing

<!-- Describe your testing approach -->
"#;

/// Initialize a new configuration in the given directory
pub fn init(project_root: &Path, force: bool) -> Result<()> {
    use colored::Colorize;

    let agents_dir = project_root.join(".agents");
    let config_path = agents_dir.join("agentsync.toml");
    let agents_md_path = agents_dir.join("AGENTS.md");

    // Create .agents directory
    if !agents_dir.exists() {
        fs::create_dir_all(&agents_dir)?;
        println!(
            "  {} Created directory: {}",
            "✔".green(),
            agents_dir.display()
        );
    }

    // Create skills directory
    let skills_dir = agents_dir.join("skills");
    if !skills_dir.exists() {
        fs::create_dir_all(&skills_dir)?;
        println!(
            "  {} Created directory: {}",
            "✔".green(),
            skills_dir.display()
        );
    }

    // Create config file
    if config_path.exists() && !force {
        println!(
            "  {} Config already exists: {} (use --force to overwrite)",
            "!".yellow(),
            config_path.display()
        );
    } else {
        fs::write(&config_path, DEFAULT_CONFIG)?;
        println!("  {} Created: {}", "✔".green(), config_path.display());
    }

    // Create AGENTS.md
    if agents_md_path.exists() && !force {
        println!(
            "  {} AGENTS.md already exists: {} (use --force to overwrite)",
            "!".yellow(),
            agents_md_path.display()
        );
    } else {
        fs::write(&agents_md_path, DEFAULT_AGENTS_MD)?;
        println!("  {} Created: {}", "✔".green(), agents_md_path.display());
    }

    Ok(())
}

/// Discovered agent-related file
#[derive(Debug, Clone)]
struct DiscoveredFile {
    /// Path to the file relative to project root
    path: std::path::PathBuf,
    /// Type of agent file
    file_type: AgentFileType,
    /// Display name for user selection
    display_name: String,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // Some variants like Other are reserved for future extensibility
enum AgentFileType {
    ClaudeInstructions,
    CursorDirectory,
    McpConfig,
    CopilotInstructions,
    RootAgentsFile,
    Other,
}

/// Scan project for existing agent-related files
fn scan_agent_files(project_root: &Path) -> Result<Vec<DiscoveredFile>> {
    let mut discovered = Vec::new();

    // Check for CLAUDE.md in root
    let claude_path = project_root.join("CLAUDE.md");
    if claude_path.exists() {
        discovered.push(DiscoveredFile {
            path: "CLAUDE.md".into(),
            file_type: AgentFileType::ClaudeInstructions,
            display_name: "CLAUDE.md (Claude instructions)".to_string(),
        });
    }

    // Check for .cursor/ directory
    let cursor_path = project_root.join(".cursor");
    if cursor_path.exists() && cursor_path.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".cursor".into(),
            file_type: AgentFileType::CursorDirectory,
            display_name: ".cursor/ (Cursor configuration directory)".to_string(),
        });
    }

    // Check for .mcp.json
    let mcp_path = project_root.join(".mcp.json");
    if mcp_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".mcp.json".into(),
            file_type: AgentFileType::McpConfig,
            display_name: ".mcp.json (MCP configuration)".to_string(),
        });
    }

    // Check for GitHub Copilot instructions
    let copilot_path = project_root.join(".github").join("copilot-instructions.md");
    if copilot_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".github/copilot-instructions.md".into(),
            file_type: AgentFileType::CopilotInstructions,
            display_name: ".github/copilot-instructions.md (Copilot instructions)".to_string(),
        });
    }

    // Check for AGENTS.md in root (not in .agents/)
    let agents_path = project_root.join("AGENTS.md");
    if agents_path.exists() {
        discovered.push(DiscoveredFile {
            path: "AGENTS.md".into(),
            file_type: AgentFileType::RootAgentsFile,
            display_name: "AGENTS.md (Root agent instructions)".to_string(),
        });
    }

    Ok(discovered)
}

/// Interactive wizard for initializing agentsync with file migration
pub fn init_wizard(project_root: &Path, force: bool) -> Result<()> {
    use colored::Colorize;
    use dialoguer::{Confirm, MultiSelect, theme::ColorfulTheme};

    // Scan for existing agent files
    println!("{}", "🔍 Scanning for existing agent files...".cyan());
    let discovered_files = scan_agent_files(project_root)?;

    if discovered_files.is_empty() {
        println!("{}", "  No existing agent files found.".dimmed());
        println!(
            "{}",
            "  Proceeding with standard initialization...".dimmed()
        );
        return init(project_root, force);
    }

    println!("  {} Found {} file(s)", "✔".green(), discovered_files.len());

    // Display found files
    println!("\n{}", "Detected files:".bold());
    for file in &discovered_files {
        println!("  • {}", file.display_name.yellow());
    }

    println!();

    // Ask if user wants to migrate files
    let should_migrate = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Would you like to migrate these files to the .agents/ directory?")
        .default(true)
        .interact()?;

    if !should_migrate {
        println!(
            "{}",
            "  Skipping migration. Creating standard configuration...".dimmed()
        );
        return init(project_root, force);
    }

    // Let user select which files to migrate
    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select files to migrate (use Space to select, Enter to confirm)")
        .items(
            &discovered_files
                .iter()
                .map(|f| f.display_name.as_str())
                .collect::<Vec<_>>(),
        )
        .defaults(&discovered_files.iter().map(|_| true).collect::<Vec<_>>())
        .interact()?;

    let files_to_migrate: Vec<_> = selections
        .iter()
        .map(|&idx| discovered_files[idx].clone())
        .collect();

    if files_to_migrate.is_empty() {
        println!("{}", "  No files selected for migration.".dimmed());
        println!(
            "{}",
            "  Proceeding with standard initialization...".dimmed()
        );
        return init(project_root, force);
    }

    // Create .agents directory structure
    println!("\n{}", "📦 Setting up .agents/ directory...".cyan());
    let agents_dir = project_root.join(".agents");
    if !agents_dir.exists() {
        fs::create_dir_all(&agents_dir)?;
        println!(
            "  {} Created directory: {}",
            "✔".green(),
            agents_dir.display()
        );
    }

    // Create skills directory
    let skills_dir = agents_dir.join("skills");
    if !skills_dir.exists() {
        fs::create_dir_all(&skills_dir)?;
        println!(
            "  {} Created directory: {}",
            "✔".green(),
            skills_dir.display()
        );
    }

    // Migrate selected files
    println!("\n{}", "🔄 Migrating files...".cyan());

    // Collect all instruction files first
    let instruction_files: Vec<_> = files_to_migrate
        .iter()
        .filter(|f| {
            matches!(
                f.file_type,
                AgentFileType::ClaudeInstructions
                    | AgentFileType::RootAgentsFile
                    | AgentFileType::CopilotInstructions
            )
        })
        .collect();

    // Determine how to handle instruction files
    let mut migrated_content: Option<String> = None;
    let mut instruction_files_merged = 0;

    if instruction_files.len() > 1 {
        // Multiple instruction files - merge them with section headings
        let mut merged = String::new();
        for file in &instruction_files {
            let src_path = project_root.join(&file.path);
            if let Ok(content) = fs::read_to_string(&src_path) {
                if !merged.is_empty() {
                    merged.push_str("\n\n---\n\n");
                }
                merged.push_str(&format!(
                    "# Instructions from {}\n\n{}",
                    file.path.display(),
                    content
                ));
                instruction_files_merged += 1;
            }
        }
        if !merged.is_empty() {
            migrated_content = Some(merged);
        }
    } else if instruction_files.len() == 1 {
        // Single instruction file - use its content directly
        let src_path = project_root.join(&instruction_files[0].path);
        if let Ok(content) = fs::read_to_string(&src_path) {
            migrated_content = Some(content);
            instruction_files_merged = 1;
        }
    }

    // Track migration counts
    let mut files_actually_migrated = 0;
    let mut files_skipped = 0;

    for file in &files_to_migrate {
        let src_path = project_root.join(&file.path);

        match file.file_type {
            AgentFileType::ClaudeInstructions
            | AgentFileType::RootAgentsFile
            | AgentFileType::CopilotInstructions => {
                // Already handled above - content merged into AGENTS.md
                continue;
            }
            AgentFileType::CursorDirectory => {
                // Copy .cursor directory to .agents/.cursor
                if src_path.exists() {
                    let dest_path = agents_dir.join(".cursor");
                    copy_dir_all(&src_path, &dest_path)?;
                    let dest_display = dest_path
                        .strip_prefix(project_root)
                        .unwrap_or(&dest_path)
                        .display();
                    println!(
                        "  {} Copied: {} → {}",
                        "✔".green(),
                        file.path.display(),
                        dest_display
                    );
                    files_actually_migrated += 1;
                }
            }
            AgentFileType::McpConfig => {
                // Note: MCP config will be handled by agentsync.toml
                println!(
                    "  {} Note: .mcp.json detected. You can configure MCP servers in agentsync.toml",
                    "ℹ".blue()
                );
                files_skipped += 1;
            }
            AgentFileType::Other => {
                files_skipped += 1;
            }
        }
    }

    // Create AGENTS.md with migrated content
    let agents_md_path = agents_dir.join("AGENTS.md");
    if let Some(content) = migrated_content {
        if agents_md_path.exists() && !force {
            println!(
                "  {} AGENTS.md already exists (use --force to overwrite)",
                "!".yellow()
            );
        } else {
            fs::write(&agents_md_path, &content)?;
            if instruction_files_merged > 1 {
                println!(
                    "  {} Created: {} (merged {} instruction files)",
                    "✔".green(),
                    agents_md_path.display(),
                    instruction_files_merged
                );
            } else {
                println!(
                    "  {} Created: {} (with migrated content)",
                    "✔".green(),
                    agents_md_path.display()
                );
            }
        }
    } else {
        // Use default template if no content migrated
        if !agents_md_path.exists() || force {
            fs::write(&agents_md_path, DEFAULT_AGENTS_MD)?;
            println!("  {} Created: {}", "✔".green(), agents_md_path.display());
        }
    }

    // Generate config file
    println!("\n{}", "⚙️  Generating configuration...".cyan());
    let config_path = agents_dir.join("agentsync.toml");

    if config_path.exists() && !force {
        println!(
            "  {} Config already exists: {} (use --force to overwrite)",
            "!".yellow(),
            config_path.display()
        );
    } else {
        fs::write(&config_path, DEFAULT_CONFIG)?;
        println!("  {} Created: {}", "✔".green(), config_path.display());
    }

    // Provide migration summary
    println!("\n{}", "📋 Migration Summary:".bold());
    if instruction_files_merged > 0 {
        println!(
            "  • Merged {} instruction file(s) into AGENTS.md",
            instruction_files_merged
        );
    }
    if files_actually_migrated > 0 {
        println!(
            "  • Migrated {} file(s) to .agents/",
            files_actually_migrated
        );
    }
    if files_skipped > 0 {
        println!(
            "  • Skipped {} file(s) (content noted in configuration)",
            files_skipped
        );
    }
    println!(
        "  • Configuration saved to {}",
        ".agents/agentsync.toml".cyan()
    );

    // Ask if user wants to back up original files
    let should_backup = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(
            "Would you like to back up the original files? (They will be moved to .agents/backup/)",
        )
        .default(true)
        .interact()?;

    if should_backup {
        let backup_dir = agents_dir.join("backup");
        fs::create_dir_all(&backup_dir)?;

        for file in &files_to_migrate {
            if file.file_type == AgentFileType::McpConfig {
                // Skip files that weren't actually migrated
                continue;
            }

            let src_path = project_root.join(&file.path);
            if !src_path.exists() {
                continue;
            }

            let backup_path = backup_dir.join(&file.path);
            if let Some(parent) = backup_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Try to move the file/directory first (rename)
            match fs::rename(&src_path, &backup_path) {
                Ok(_) => {
                    println!("  {} Moved: {}", "✔".green(), file.path.display());
                }
                Err(_) => {
                    // Cross-filesystem or other error - fall back to copy then delete
                    if src_path.is_dir() {
                        copy_dir_all(&src_path, &backup_path)?;
                        fs::remove_dir_all(&src_path)?;
                    } else {
                        fs::copy(&src_path, &backup_path)?;
                        fs::remove_file(&src_path)?;
                    }
                    println!("  {} Moved: {}", "✔".green(), file.path.display());
                }
            }
        }
    }

    Ok(())
}

/// Helper function to copy a directory recursively
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        // Use symlink_metadata to detect symlinks without following them
        let metadata = entry.path().symlink_metadata()?;

        if metadata.is_symlink() {
            // Handle symlinks: recreate them at destination
            #[cfg(unix)]
            {
                use std::os::unix::fs as unix_fs;
                let link_target = fs::read_link(&src_path)?;
                unix_fs::symlink(&link_target, &dst_path)?;
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs as windows_fs;
                let link_target = fs::read_link(&src_path)?;
                // Resolve target relative to source parent to check if it's a directory
                let resolved_target = src_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(&link_target);

                if resolved_target.is_dir() {
                    windows_fs::symlink_dir(&link_target, &dst_path)?;
                } else {
                    windows_fs::symlink_file(&link_target, &dst_path)?;
                }
            }
        } else if metadata.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ==========================================================================
    // INIT FUNCTION TESTS
    // ==========================================================================

    #[test]
    fn test_init_creates_agents_directory() {
        let temp_dir = TempDir::new().unwrap();

        init(temp_dir.path(), false).unwrap();

        let agents_dir = temp_dir.path().join(".agents");
        assert!(agents_dir.exists());
        assert!(agents_dir.is_dir());
    }

    #[test]
    fn test_init_creates_skills_directory() {
        let temp_dir = TempDir::new().unwrap();

        init(temp_dir.path(), false).unwrap();

        let skills_dir = temp_dir.path().join(".agents").join("skills");
        assert!(skills_dir.exists());
        assert!(skills_dir.is_dir());
    }

    #[test]
    fn test_init_creates_config_file() {
        let temp_dir = TempDir::new().unwrap();

        init(temp_dir.path(), false).unwrap();

        let config_path = temp_dir.path().join(".agents").join("agentsync.toml");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("[agents.claude]"));
        assert!(content.contains("[agents.copilot]"));
        assert!(content.contains("[agents.cursor]"));
        assert!(content.contains("[agents.codex]"));
    }

    #[test]
    fn test_init_creates_agents_md() {
        let temp_dir = TempDir::new().unwrap();

        init(temp_dir.path(), false).unwrap();

        let agents_md_path = temp_dir.path().join(".agents").join("AGENTS.md");
        assert!(agents_md_path.exists());

        let content = fs::read_to_string(&agents_md_path).unwrap();
        assert!(content.contains("# AI Agent Instructions"));
        assert!(content.contains("## Project Overview"));
    }

    #[test]
    fn test_init_does_not_overwrite_without_force() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        // Create existing files with custom content
        let config_path = agents_dir.join("agentsync.toml");
        let original_config = "# My custom config";
        fs::write(&config_path, original_config).unwrap();

        let agents_md_path = agents_dir.join("AGENTS.md");
        let original_agents = "# My custom agents";
        fs::write(&agents_md_path, original_agents).unwrap();

        // Init without force
        init(temp_dir.path(), false).unwrap();

        // Files should NOT be overwritten
        assert_eq!(fs::read_to_string(&config_path).unwrap(), original_config);
        assert_eq!(
            fs::read_to_string(&agents_md_path).unwrap(),
            original_agents
        );
    }

    #[test]
    fn test_init_overwrites_with_force() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        // Create existing files with custom content
        let config_path = agents_dir.join("agentsync.toml");
        fs::write(&config_path, "# Old config").unwrap();

        let agents_md_path = agents_dir.join("AGENTS.md");
        fs::write(&agents_md_path, "# Old agents").unwrap();

        // Init WITH force
        init(temp_dir.path(), true).unwrap();

        // Files SHOULD be overwritten with default content
        let config_content = fs::read_to_string(&config_path).unwrap();
        assert!(config_content.contains("[agents.claude]"));
        assert!(config_content.contains("[agents.cursor]"));
        assert!(config_content.contains("[agents.codex]"));

        let agents_content = fs::read_to_string(&agents_md_path).unwrap();
        assert!(agents_content.contains("# AI Agent Instructions"));
    }

    #[test]
    fn test_init_with_existing_agents_dir() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        // Init should work even if .agents exists
        let result = init(temp_dir.path(), false);
        assert!(result.is_ok());

        // Should still create files
        assert!(agents_dir.join("agentsync.toml").exists());
        assert!(agents_dir.join("AGENTS.md").exists());
    }

    #[test]
    fn test_init_creates_nested_structure() {
        let temp_dir = TempDir::new().unwrap();
        let nested_project = temp_dir.path().join("deep").join("nested").join("project");
        fs::create_dir_all(&nested_project).unwrap();

        init(&nested_project, false).unwrap();

        let agents_dir = nested_project.join(".agents");
        assert!(agents_dir.exists());
        assert!(agents_dir.join("agentsync.toml").exists());
        assert!(agents_dir.join("AGENTS.md").exists());
    }

    // ==========================================================================
    // DEFAULT TEMPLATE TESTS
    // ==========================================================================

    #[test]
    fn test_default_config_is_valid_toml() {
        // Ensure the default config template is valid TOML
        let result: Result<crate::config::Config, _> = toml::from_str(DEFAULT_CONFIG);
        assert!(result.is_ok(), "Default config should be valid TOML");
    }

    #[test]
    fn test_default_config_contains_expected_agents() {
        let config: crate::config::Config = toml::from_str(DEFAULT_CONFIG).unwrap();

        assert!(config.agents.contains_key("claude"));
        assert!(config.agents.contains_key("copilot"));
        assert!(config.agents.contains_key("cursor"));
        assert!(config.agents.contains_key("codex"));
        assert!(config.agents.contains_key("root"));
    }

    #[test]
    fn test_default_config_agents_are_enabled() {
        let config: crate::config::Config = toml::from_str(DEFAULT_CONFIG).unwrap();

        assert!(config.agents["claude"].enabled);
        assert!(config.agents["copilot"].enabled);
        assert!(config.agents["cursor"].enabled);
        assert!(config.agents["codex"].enabled);
        assert!(config.agents["root"].enabled);
    }

    #[test]
    fn test_default_config_gitignore_enabled() {
        let config: crate::config::Config = toml::from_str(DEFAULT_CONFIG).unwrap();

        assert!(config.gitignore.enabled);
        assert_eq!(config.gitignore.marker, "AI Agent Symlinks");
    }

    #[test]
    fn test_default_agents_md_contains_sections() {
        assert!(DEFAULT_AGENTS_MD.contains("# AI Agent Instructions"));
        assert!(DEFAULT_AGENTS_MD.contains("## Project Overview"));
        assert!(DEFAULT_AGENTS_MD.contains("## Code Style"));
        assert!(DEFAULT_AGENTS_MD.contains("## Architecture"));
        assert!(DEFAULT_AGENTS_MD.contains("## Testing"));
    }

    // ==========================================================================
    // WIZARD TESTS
    // ==========================================================================

    #[test]
    fn test_scan_agent_files_finds_claude_md() {
        let temp_dir = TempDir::new().unwrap();
        let claude_path = temp_dir.path().join("CLAUDE.md");
        fs::write(&claude_path, "# Claude Instructions").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::ClaudeInstructions);
        assert_eq!(discovered[0].path.to_str().unwrap(), "CLAUDE.md");
    }

    #[test]
    fn test_scan_agent_files_finds_cursor_dir() {
        let temp_dir = TempDir::new().unwrap();
        let cursor_dir = temp_dir.path().join(".cursor");
        fs::create_dir_all(&cursor_dir).unwrap();
        fs::write(cursor_dir.join("test.txt"), "test").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::CursorDirectory);
        assert_eq!(discovered[0].path.to_str().unwrap(), ".cursor");
    }

    #[test]
    fn test_scan_agent_files_finds_mcp_json() {
        let temp_dir = TempDir::new().unwrap();
        let mcp_path = temp_dir.path().join(".mcp.json");
        fs::write(&mcp_path, "{}").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::McpConfig);
        assert_eq!(discovered[0].path.to_str().unwrap(), ".mcp.json");
    }

    #[test]
    fn test_scan_agent_files_finds_copilot_instructions() {
        let temp_dir = TempDir::new().unwrap();
        let github_dir = temp_dir.path().join(".github");
        fs::create_dir_all(&github_dir).unwrap();
        let copilot_path = github_dir.join("copilot-instructions.md");
        fs::write(&copilot_path, "# Copilot Instructions").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::CopilotInstructions);
        assert_eq!(
            discovered[0].path.to_str().unwrap(),
            ".github/copilot-instructions.md"
        );
    }

    #[test]
    fn test_scan_agent_files_finds_root_agents_md() {
        let temp_dir = TempDir::new().unwrap();
        let agents_path = temp_dir.path().join("AGENTS.md");
        fs::write(&agents_path, "# Agent Instructions").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::RootAgentsFile);
        assert_eq!(discovered[0].path.to_str().unwrap(), "AGENTS.md");
    }

    #[test]
    fn test_scan_agent_files_finds_multiple() {
        let temp_dir = TempDir::new().unwrap();

        // Create multiple agent files
        fs::write(temp_dir.path().join("CLAUDE.md"), "claude").unwrap();
        fs::write(temp_dir.path().join(".mcp.json"), "{}").unwrap();
        fs::create_dir_all(temp_dir.path().join(".cursor")).unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 3);
    }

    #[test]
    fn test_scan_agent_files_empty_project() {
        let temp_dir = TempDir::new().unwrap();
        let discovered = scan_agent_files(temp_dir.path()).unwrap();
        assert_eq!(discovered.len(), 0);
    }

    #[test]
    fn test_copy_dir_all() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        let dst_dir = temp_dir.path().join("dst");

        // Create source structure
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("file1.txt"), "content1").unwrap();
        fs::create_dir_all(src_dir.join("subdir")).unwrap();
        fs::write(src_dir.join("subdir").join("file2.txt"), "content2").unwrap();

        // Copy
        copy_dir_all(&src_dir, &dst_dir).unwrap();

        // Verify
        assert!(dst_dir.exists());
        assert!(dst_dir.join("file1.txt").exists());
        assert!(dst_dir.join("subdir").exists());
        assert!(dst_dir.join("subdir").join("file2.txt").exists());
        assert_eq!(
            fs::read_to_string(dst_dir.join("file1.txt")).unwrap(),
            "content1"
        );
        assert_eq!(
            fs::read_to_string(dst_dir.join("subdir").join("file2.txt")).unwrap(),
            "content2"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_copy_dir_all_with_symlinks() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        let dst_dir = temp_dir.path().join("dst");

        // Create source structure with a symlink
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("real_file.txt"), "content").unwrap();

        // Create a symlink
        let link_path = src_dir.join("link_to_file.txt");
        unix_fs::symlink("real_file.txt", &link_path).unwrap();

        // Copy
        copy_dir_all(&src_dir, &dst_dir).unwrap();

        // Verify symlink was recreated (not followed)
        let dst_link = dst_dir.join("link_to_file.txt");
        assert!(dst_link.exists());

        let metadata = dst_link.symlink_metadata().unwrap();
        assert!(metadata.is_symlink());

        // Verify symlink target
        let link_target = fs::read_link(&dst_link).unwrap();
        assert_eq!(link_target.to_str().unwrap(), "real_file.txt");
    }

    #[test]
    fn test_merge_multiple_instruction_files() {
        let temp_dir = TempDir::new().unwrap();

        // Create multiple instruction files
        fs::write(
            temp_dir.path().join("CLAUDE.md"),
            "# Claude Instructions\n\nUse Claude.",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("AGENTS.md"),
            "# Agent Instructions\n\nGeneral agent info.",
        )
        .unwrap();

        let github_dir = temp_dir.path().join(".github");
        fs::create_dir_all(&github_dir).unwrap();
        fs::write(
            github_dir.join("copilot-instructions.md"),
            "# Copilot Instructions\n\nUse Copilot.",
        )
        .unwrap();

        // Scan files
        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        // Should find all three instruction files
        let instruction_files: Vec<_> = discovered
            .iter()
            .filter(|f| {
                matches!(
                    f.file_type,
                    AgentFileType::ClaudeInstructions
                        | AgentFileType::RootAgentsFile
                        | AgentFileType::CopilotInstructions
                )
            })
            .collect();

        assert_eq!(instruction_files.len(), 3);
    }

    #[test]
    fn test_backup_moves_files() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        // Create a test file that would be backed up
        let test_file = temp_dir.path().join("CLAUDE.md");
        fs::write(&test_file, "test content").unwrap();
        assert!(test_file.exists());

        // Simulate backup by moving the file
        let backup_dir = agents_dir.join("backup");
        fs::create_dir_all(&backup_dir).unwrap();
        let backup_path = backup_dir.join("CLAUDE.md");

        // Try rename (move)
        let result = fs::rename(&test_file, &backup_path);

        if result.is_ok() {
            // File should be moved (not exist at original location)
            assert!(!test_file.exists());
            assert!(backup_path.exists());
            assert_eq!(fs::read_to_string(&backup_path).unwrap(), "test content");
        } else {
            // If rename fails, test the fallback (copy + delete)
            fs::copy(&test_file, &backup_path).unwrap();
            fs::remove_file(&test_file).unwrap();

            assert!(!test_file.exists());
            assert!(backup_path.exists());
            assert_eq!(fs::read_to_string(&backup_path).unwrap(), "test content");
        }
    }

    #[test]
    fn test_backup_moves_directories() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        // Create a test directory
        let test_dir = temp_dir.path().join(".cursor");
        fs::create_dir_all(&test_dir).unwrap();
        fs::write(test_dir.join("config.txt"), "config content").unwrap();
        assert!(test_dir.exists());

        // Simulate backup by moving the directory
        let backup_dir = agents_dir.join("backup");
        fs::create_dir_all(&backup_dir).unwrap();
        let backup_path = backup_dir.join(".cursor");

        // Try rename (move)
        let result = fs::rename(&test_dir, &backup_path);

        if result.is_ok() {
            // Directory should be moved
            assert!(!test_dir.exists());
            assert!(backup_path.exists());
            assert!(backup_path.join("config.txt").exists());
        } else {
            // If rename fails, test the fallback (copy + delete)
            copy_dir_all(&test_dir, &backup_path).unwrap();
            fs::remove_dir_all(&test_dir).unwrap();

            assert!(!test_dir.exists());
            assert!(backup_path.exists());
            assert!(backup_path.join("config.txt").exists());
        }
    }
}
