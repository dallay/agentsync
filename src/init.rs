//! Template generation for init command
//!
//! Provides default configuration templates for new projects.

use anyhow::Result;
use std::fs;
use std::path::Path;

/// Default configuration template
pub const DEFAULT_CONFIG: &str = r#"# AgentSync Configuration
# This file defines how AI agent configurations are synchronized via symbolic links.
# 
# Source directory (relative to this config file)
source_dir = "."

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
# config files for each AI agent (Claude, Copilot, Cursor, Gemini, VS Code, OpenCode).
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
}
