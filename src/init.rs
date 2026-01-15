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
        println!("  {} Created directory: {}", "✔".green(), agents_dir.display());
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
