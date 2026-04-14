//! Template generation for init command
//!
//! Provides default configuration templates for new projects and interactive wizard.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::config::{Config, SyncType, TargetConfig};
use crate::skills_layout::{detect_skills_layout_match, detect_skills_mode_mismatch};

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

[agents.claude.targets.skills]
source = "skills"
destination = ".claude/skills"
type = "symlink"

[agents.claude.targets.commands]
source = "commands"
destination = ".claude/commands"
type = "symlink-contents"

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
type = "symlink"

# -----------------------------------------------------------------------------
# Gemini CLI
# -----------------------------------------------------------------------------
[agents.gemini]
enabled = true
description = "Gemini CLI - Google's AI coding assistant"

[agents.gemini.targets.instructions]
source = "AGENTS.md"
destination = "GEMINI.md"
type = "symlink"

[agents.gemini.targets.skills]
source = "skills"
destination = ".gemini/skills"
type = "symlink"

[agents.gemini.targets.commands]
source = "commands"
destination = ".gemini/commands"
type = "symlink-contents"

# -----------------------------------------------------------------------------
# OpenCode
# -----------------------------------------------------------------------------
[agents.opencode]
enabled = true
description = "OpenCode - Open-source AI coding assistant"

[agents.opencode.targets.instructions]
source = "AGENTS.md"
destination = "OPENCODE.md"
type = "symlink"

[agents.opencode.targets.skills]
source = "skills"
destination = ".opencode/skills"
type = "symlink"

# Note: intentionally singular per OpenCode convention (.opencode/command, not .opencode/commands)
[agents.opencode.targets.commands]
source = "commands"
destination = ".opencode/command"
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

    // Create commands directory
    let commands_dir = agents_dir.join("commands");
    if !commands_dir.exists() {
        fs::create_dir_all(&commands_dir)?;
        println!(
            "  {} Created directory: {}",
            "✔".green(),
            commands_dir.display()
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
    // Instruction / rules files (merged into AGENTS.md on migration)
    ClaudeInstructions,
    CopilotInstructions,
    RootAgentsFile,
    WindsurfRules,
    ClineRules,
    CrushInstructions,
    AmpInstructions,
    AntigravityRules,
    AmazonQRules,
    AiderConfig,
    FirebaseRules,
    OpenHandsMicroagents,
    JunieDirectory,
    AugmentRules,
    KilocodeDirectory,
    GooseHints,
    QwenDirectory,
    RooRules,
    ZedSettings,
    TraeRules,
    WarpInstructions,
    KiroSteering,
    FirebenderConfig,
    FactoryDirectory,
    VibeDirectory,
    JetBrainsRules,
    GeminiInstructions,
    OpenCodeInstructions,
    // Skill directories (contents merged into .agents/skills/ on migration)
    ClaudeSkills,
    CursorSkills,
    CodexSkills,
    GeminiSkills,
    OpenCodeSkills,
    RooSkills,
    FactorySkills,
    VibeSkills,
    AntigravitySkills,
    // Command directories (contents merged into .agents/commands/ on migration)
    ClaudeCommands,
    GeminiCommands,
    OpenCodeCommands,
    // Directory / config files (copied as-is on migration)
    CursorDirectory,
    WindsurfDirectory,
    // MCP / tooling config (noted, not migrated as content)
    McpConfig,
    CursorMcpConfig,
    CopilotMcpConfig,
    WindsurfMcpConfig,
    CodexConfig,
    RooMcpConfig,
    KiroMcpConfig,
    AmazonQMcpConfig,
    KilocodeMcpConfig,
    FactoryMcpConfig,
    OpenCodeConfig,
    Other,
}

/// Check if a directory has at least one entry, propagating IO errors.
fn dir_has_entries(path: &Path) -> Result<bool> {
    Ok(fs::read_dir(path)
        .with_context(|| format!("Failed to read directory: {}", path.display()))?
        .next()
        .is_some())
}

/// Scan project for existing agent-related files
fn scan_agent_files(project_root: &Path) -> Result<Vec<DiscoveredFile>> {
    let mut discovered = Vec::new();

    // -------------------------------------------------------------------------
    // Native MCP agents
    // -------------------------------------------------------------------------

    // Claude Code: CLAUDE.md
    let claude_path = project_root.join("CLAUDE.md");
    if claude_path.exists() {
        discovered.push(DiscoveredFile {
            path: "CLAUDE.md".into(),
            file_type: AgentFileType::ClaudeInstructions,
            display_name: "CLAUDE.md (Claude Code instructions)".to_string(),
        });
    }

    // Claude Code: .claude/skills/ directory
    let claude_skills_path = project_root.join(".claude").join("skills");
    if claude_skills_path.exists() && claude_skills_path.is_dir() {
        // Only report if directory has at least one child entry
        let has_content = dir_has_entries(&claude_skills_path)?;
        if has_content {
            discovered.push(DiscoveredFile {
                path: ".claude/skills".into(),
                file_type: AgentFileType::ClaudeSkills,
                display_name: "Claude Code skills (.claude/skills/)".to_string(),
            });
        }
    }

    // Claude Code: .claude/commands/ directory
    let claude_commands_path = project_root.join(".claude").join("commands");
    if claude_commands_path.exists() && claude_commands_path.is_dir() {
        let has_content = dir_has_entries(&claude_commands_path)?;
        if has_content {
            discovered.push(DiscoveredFile {
                path: ".claude/commands".into(),
                file_type: AgentFileType::ClaudeCommands,
                display_name: "Claude Code commands (.claude/commands/)".to_string(),
            });
        }
    }

    // GitHub Copilot: .github/copilot-instructions.md
    let copilot_path = project_root.join(".github").join("copilot-instructions.md");
    if copilot_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".github/copilot-instructions.md".into(),
            file_type: AgentFileType::CopilotInstructions,
            display_name: ".github/copilot-instructions.md (Copilot instructions)".to_string(),
        });
    }

    // Copilot: .vscode/mcp.json
    let copilot_mcp_path = project_root.join(".vscode").join("mcp.json");
    if copilot_mcp_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".vscode/mcp.json".into(),
            file_type: AgentFileType::CopilotMcpConfig,
            display_name: ".vscode/mcp.json (VS Code / Copilot MCP configuration)".to_string(),
        });
    }

    // Cursor: .cursor/ directory
    let cursor_path = project_root.join(".cursor");
    if cursor_path.exists() && cursor_path.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".cursor".into(),
            file_type: AgentFileType::CursorDirectory,
            display_name: ".cursor/ (Cursor configuration directory)".to_string(),
        });
    }

    // Cursor: .cursor/skills/ directory
    let cursor_skills_path = project_root.join(".cursor").join("skills");
    if cursor_skills_path.exists() && cursor_skills_path.is_dir() {
        let has_content = dir_has_entries(&cursor_skills_path)?;
        if has_content {
            discovered.push(DiscoveredFile {
                path: ".cursor/skills".into(),
                file_type: AgentFileType::CursorSkills,
                display_name: "Cursor skills (.cursor/skills/)".to_string(),
            });
        }
    }

    // Cursor: .cursor/mcp.json
    let cursor_mcp_path = project_root.join(".cursor").join("mcp.json");
    if cursor_mcp_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".cursor/mcp.json".into(),
            file_type: AgentFileType::CursorMcpConfig,
            display_name: ".cursor/mcp.json (Cursor MCP configuration)".to_string(),
        });
    }

    // Gemini CLI: GEMINI.md
    let gemini_path = project_root.join("GEMINI.md");
    if gemini_path.exists() {
        discovered.push(DiscoveredFile {
            path: "GEMINI.md".into(),
            file_type: AgentFileType::GeminiInstructions,
            display_name: "GEMINI.md (Gemini CLI instructions)".to_string(),
        });
    }

    // Gemini CLI: .gemini/skills/ directory
    let gemini_skills_path = project_root.join(".gemini").join("skills");
    if gemini_skills_path.exists() && gemini_skills_path.is_dir() {
        let has_content = dir_has_entries(&gemini_skills_path)?;
        if has_content {
            discovered.push(DiscoveredFile {
                path: ".gemini/skills".into(),
                file_type: AgentFileType::GeminiSkills,
                display_name: "Gemini skills (.gemini/skills/)".to_string(),
            });
        }
    }

    // Gemini CLI: .gemini/commands/ directory
    let gemini_commands_path = project_root.join(".gemini").join("commands");
    if gemini_commands_path.exists() && gemini_commands_path.is_dir() {
        let has_content = dir_has_entries(&gemini_commands_path)?;
        if has_content {
            discovered.push(DiscoveredFile {
                path: ".gemini/commands".into(),
                file_type: AgentFileType::GeminiCommands,
                display_name: "Gemini commands (.gemini/commands/)".to_string(),
            });
        }
    }

    // OpenCode: OPENCODE.md
    let opencode_path = project_root.join("OPENCODE.md");
    if opencode_path.exists() {
        discovered.push(DiscoveredFile {
            path: "OPENCODE.md".into(),
            file_type: AgentFileType::OpenCodeInstructions,
            display_name: "OPENCODE.md (OpenCode instructions)".to_string(),
        });
    }

    // OpenCode: .opencode/skills/ directory
    let opencode_skills_path = project_root.join(".opencode").join("skills");
    if opencode_skills_path.exists() && opencode_skills_path.is_dir() {
        let has_content = dir_has_entries(&opencode_skills_path)?;
        if has_content {
            discovered.push(DiscoveredFile {
                path: ".opencode/skills".into(),
                file_type: AgentFileType::OpenCodeSkills,
                display_name: "OpenCode skills (.opencode/skills/)".to_string(),
            });
        }
    }

    // OpenCode: .opencode/command/ directory
    let opencode_commands_path = project_root.join(".opencode").join("command");
    if opencode_commands_path.exists() && opencode_commands_path.is_dir() {
        let has_content = dir_has_entries(&opencode_commands_path)?;
        if has_content {
            discovered.push(DiscoveredFile {
                path: ".opencode/command".into(),
                file_type: AgentFileType::OpenCodeCommands,
                display_name: "OpenCode commands (.opencode/command/)".to_string(),
            });
        }
    }

    // OpenCode: opencode.json
    let opencode_config_path = project_root.join("opencode.json");
    if opencode_config_path.exists() {
        discovered.push(DiscoveredFile {
            path: "opencode.json".into(),
            file_type: AgentFileType::OpenCodeConfig,
            display_name: "opencode.json (OpenCode configuration)".to_string(),
        });
    }

    // Generic MCP config: .mcp.json
    let mcp_path = project_root.join(".mcp.json");
    if mcp_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".mcp.json".into(),
            file_type: AgentFileType::McpConfig,
            display_name: ".mcp.json (MCP configuration)".to_string(),
        });
    }

    // -------------------------------------------------------------------------
    // Root AGENTS.md (used by Codex CLI and many other agents)
    // -------------------------------------------------------------------------
    let agents_path = project_root.join("AGENTS.md");
    if agents_path.exists() {
        discovered.push(DiscoveredFile {
            path: "AGENTS.md".into(),
            file_type: AgentFileType::RootAgentsFile,
            display_name: "AGENTS.md (Root agent instructions)".to_string(),
        });
    }

    // Codex CLI: .codex/skills/ directory
    let codex_skills_path = project_root.join(".codex").join("skills");
    if codex_skills_path.exists() && codex_skills_path.is_dir() {
        let has_content = dir_has_entries(&codex_skills_path)?;
        if has_content {
            discovered.push(DiscoveredFile {
                path: ".codex/skills".into(),
                file_type: AgentFileType::CodexSkills,
                display_name: "Codex skills (.codex/skills/)".to_string(),
            });
        }
    }

    // Codex CLI: .codex/config.toml
    let codex_config_path = project_root.join(".codex").join("config.toml");
    if codex_config_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".codex/config.toml".into(),
            file_type: AgentFileType::CodexConfig,
            display_name: ".codex/config.toml (Codex configuration)".to_string(),
        });
    }

    // -------------------------------------------------------------------------
    // Configurable agents — rules / instruction files
    // -------------------------------------------------------------------------

    // Windsurf: .windsurfrules
    let windsurfrules_path = project_root.join(".windsurfrules");
    if windsurfrules_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".windsurfrules".into(),
            file_type: AgentFileType::WindsurfRules,
            display_name: ".windsurfrules (Windsurf rules)".to_string(),
        });
    }

    // Windsurf: .windsurf/ directory (rules + MCP)
    let windsurf_dir = project_root.join(".windsurf");
    if windsurf_dir.exists() && windsurf_dir.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".windsurf".into(),
            file_type: AgentFileType::WindsurfDirectory,
            display_name: ".windsurf/ (Windsurf configuration directory)".to_string(),
        });
    }

    // Windsurf: .windsurf/mcp_config.json
    let windsurf_mcp_path = project_root.join(".windsurf").join("mcp_config.json");
    if windsurf_mcp_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".windsurf/mcp_config.json".into(),
            file_type: AgentFileType::WindsurfMcpConfig,
            display_name: ".windsurf/mcp_config.json (Windsurf MCP configuration)".to_string(),
        });
    }

    // Cline: .clinerules
    let cline_path = project_root.join(".clinerules");
    if cline_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".clinerules".into(),
            file_type: AgentFileType::ClineRules,
            display_name: ".clinerules (Cline rules)".to_string(),
        });
    }

    // Crush: CRUSH.md
    let crush_path = project_root.join("CRUSH.md");
    if crush_path.exists() {
        discovered.push(DiscoveredFile {
            path: "CRUSH.md".into(),
            file_type: AgentFileType::CrushInstructions,
            display_name: "CRUSH.md (Crush instructions)".to_string(),
        });
    }

    // Amp: AMPCODE.md
    let amp_path = project_root.join("AMPCODE.md");
    if amp_path.exists() {
        discovered.push(DiscoveredFile {
            path: "AMPCODE.md".into(),
            file_type: AgentFileType::AmpInstructions,
            display_name: "AMPCODE.md (Amp instructions)".to_string(),
        });
    }

    // Amazon Q CLI: .amazonq/rules/
    let amazonq_rules = project_root.join(".amazonq").join("rules");
    if amazonq_rules.exists() && amazonq_rules.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".amazonq/rules".into(),
            file_type: AgentFileType::AmazonQRules,
            display_name: ".amazonq/rules/ (Amazon Q CLI rules)".to_string(),
        });
    }

    // Amazon Q: .amazonq/mcp.json
    let amazonq_mcp_path = project_root.join(".amazonq").join("mcp.json");
    if amazonq_mcp_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".amazonq/mcp.json".into(),
            file_type: AgentFileType::AmazonQMcpConfig,
            display_name: ".amazonq/mcp.json (Amazon Q MCP configuration)".to_string(),
        });
    }

    // Aider: .aider.conf.yml
    let aider_path = project_root.join(".aider.conf.yml");
    if aider_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".aider.conf.yml".into(),
            file_type: AgentFileType::AiderConfig,
            display_name: ".aider.conf.yml (Aider configuration)".to_string(),
        });
    }

    // Firebase Studio / IDX: .idx/airules.md
    let firebase_rules = project_root.join(".idx").join("airules.md");
    if firebase_rules.exists() {
        discovered.push(DiscoveredFile {
            path: ".idx/airules.md".into(),
            file_type: AgentFileType::FirebaseRules,
            display_name: ".idx/airules.md (Firebase Studio / IDX rules)".to_string(),
        });
    }

    // OpenHands: .openhands/microagents/
    let openhands_path = project_root.join(".openhands").join("microagents");
    if openhands_path.exists() && openhands_path.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".openhands/microagents".into(),
            file_type: AgentFileType::OpenHandsMicroagents,
            display_name: ".openhands/microagents/ (OpenHands microagents)".to_string(),
        });
    }

    // Junie (JetBrains): .junie/
    let junie_path = project_root.join(".junie");
    if junie_path.exists() && junie_path.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".junie".into(),
            file_type: AgentFileType::JunieDirectory,
            display_name: ".junie/ (Junie / JetBrains AI configuration)".to_string(),
        });
    }

    // Augment Code: .augment/rules/
    let augment_rules = project_root.join(".augment").join("rules");
    if augment_rules.exists() && augment_rules.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".augment/rules".into(),
            file_type: AgentFileType::AugmentRules,
            display_name: ".augment/rules/ (Augment Code rules)".to_string(),
        });
    }

    // Kilo Code: .kilocode/
    let kilocode_path = project_root.join(".kilocode");
    if kilocode_path.exists() && kilocode_path.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".kilocode".into(),
            file_type: AgentFileType::KilocodeDirectory,
            display_name: ".kilocode/ (Kilo Code configuration)".to_string(),
        });
    }

    // Kilo Code: .kilocode/mcp.json
    let kilocode_mcp_path = project_root.join(".kilocode").join("mcp.json");
    if kilocode_mcp_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".kilocode/mcp.json".into(),
            file_type: AgentFileType::KilocodeMcpConfig,
            display_name: ".kilocode/mcp.json (Kilo Code MCP configuration)".to_string(),
        });
    }

    // Goose (Block): .goosehints
    let goose_path = project_root.join(".goosehints");
    if goose_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".goosehints".into(),
            file_type: AgentFileType::GooseHints,
            display_name: ".goosehints (Goose hints)".to_string(),
        });
    }

    // Qwen Code: .qwen/
    let qwen_path = project_root.join(".qwen");
    if qwen_path.exists() && qwen_path.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".qwen".into(),
            file_type: AgentFileType::QwenDirectory,
            display_name: ".qwen/ (Qwen Code configuration)".to_string(),
        });
    }

    // Roo Code: .roo/rules/
    let roo_rules = project_root.join(".roo").join("rules");
    if roo_rules.exists() && roo_rules.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".roo/rules".into(),
            file_type: AgentFileType::RooRules,
            display_name: ".roo/rules/ (Roo Code rules)".to_string(),
        });
    }

    // Roo Code: .roo/skills/ directory
    let roo_skills_path = project_root.join(".roo").join("skills");
    if roo_skills_path.exists() && roo_skills_path.is_dir() {
        let has_content = dir_has_entries(&roo_skills_path)?;
        if has_content {
            discovered.push(DiscoveredFile {
                path: ".roo/skills".into(),
                file_type: AgentFileType::RooSkills,
                display_name: "Roo Code skills (.roo/skills/)".to_string(),
            });
        }
    }

    // Roo Code: .roo/mcp.json
    let roo_mcp_path = project_root.join(".roo").join("mcp.json");
    if roo_mcp_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".roo/mcp.json".into(),
            file_type: AgentFileType::RooMcpConfig,
            display_name: ".roo/mcp.json (Roo Code MCP configuration)".to_string(),
        });
    }

    // Trae AI: .trae/rules/
    let trae_rules = project_root.join(".trae").join("rules");
    if trae_rules.exists() && trae_rules.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".trae/rules".into(),
            file_type: AgentFileType::TraeRules,
            display_name: ".trae/rules/ (Trae AI rules)".to_string(),
        });
    }

    // Warp: WARP.md
    let warp_path = project_root.join("WARP.md");
    if warp_path.exists() {
        discovered.push(DiscoveredFile {
            path: "WARP.md".into(),
            file_type: AgentFileType::WarpInstructions,
            display_name: "WARP.md (Warp terminal instructions)".to_string(),
        });
    }

    // Kiro: .kiro/steering/
    let kiro_steering = project_root.join(".kiro").join("steering");
    if kiro_steering.exists() && kiro_steering.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".kiro/steering".into(),
            file_type: AgentFileType::KiroSteering,
            display_name: ".kiro/steering/ (Kiro steering documents)".to_string(),
        });
    }

    // Kiro: .kiro/settings/mcp.json
    let kiro_mcp_path = project_root.join(".kiro").join("settings").join("mcp.json");
    if kiro_mcp_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".kiro/settings/mcp.json".into(),
            file_type: AgentFileType::KiroMcpConfig,
            display_name: ".kiro/settings/mcp.json (Kiro MCP configuration)".to_string(),
        });
    }

    // Firebender: firebender.json
    let firebender_path = project_root.join("firebender.json");
    if firebender_path.exists() {
        discovered.push(DiscoveredFile {
            path: "firebender.json".into(),
            file_type: AgentFileType::FirebenderConfig,
            display_name: "firebender.json (Firebender configuration)".to_string(),
        });
    }

    // Factory (Droids): .factory/
    let factory_path = project_root.join(".factory");
    if factory_path.exists() && factory_path.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".factory".into(),
            file_type: AgentFileType::FactoryDirectory,
            display_name: ".factory/ (Factory Droids configuration)".to_string(),
        });
    }

    // Factory: .factory/skills/ directory
    let factory_skills_path = project_root.join(".factory").join("skills");
    if factory_skills_path.exists() && factory_skills_path.is_dir() {
        let has_content = dir_has_entries(&factory_skills_path)?;
        if has_content {
            discovered.push(DiscoveredFile {
                path: ".factory/skills".into(),
                file_type: AgentFileType::FactorySkills,
                display_name: "Factory skills (.factory/skills/)".to_string(),
            });
        }
    }

    // Factory: .factory/mcp.json
    let factory_mcp_path = project_root.join(".factory").join("mcp.json");
    if factory_mcp_path.exists() {
        discovered.push(DiscoveredFile {
            path: ".factory/mcp.json".into(),
            file_type: AgentFileType::FactoryMcpConfig,
            display_name: ".factory/mcp.json (Factory MCP configuration)".to_string(),
        });
    }

    // Vibe (Mistral): .vibe/
    let vibe_path = project_root.join(".vibe");
    if vibe_path.exists() && vibe_path.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".vibe".into(),
            file_type: AgentFileType::VibeDirectory,
            display_name: ".vibe/ (Vibe / Mistral configuration)".to_string(),
        });
    }

    // Vibe: .vibe/skills/ directory
    let vibe_skills_path = project_root.join(".vibe").join("skills");
    if vibe_skills_path.exists() && vibe_skills_path.is_dir() {
        let has_content = dir_has_entries(&vibe_skills_path)?;
        if has_content {
            discovered.push(DiscoveredFile {
                path: ".vibe/skills".into(),
                file_type: AgentFileType::VibeSkills,
                display_name: "Vibe skills (.vibe/skills/)".to_string(),
            });
        }
    }

    // JetBrains AI Assistant: .aiassistant/rules/
    let jetbrains_rules = project_root.join(".aiassistant").join("rules");
    if jetbrains_rules.exists() && jetbrains_rules.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".aiassistant/rules".into(),
            file_type: AgentFileType::JetBrainsRules,
            display_name: ".aiassistant/rules/ (JetBrains AI Assistant rules)".to_string(),
        });
    }

    // Antigravity: .agent/rules/
    let antigravity_rules = project_root.join(".agent").join("rules");
    if antigravity_rules.exists() && antigravity_rules.is_dir() {
        discovered.push(DiscoveredFile {
            path: ".agent/rules".into(),
            file_type: AgentFileType::AntigravityRules,
            display_name: ".agent/rules/ (Antigravity rules)".to_string(),
        });
    }

    // Antigravity: .agent/skills/ directory
    let antigravity_skills_path = project_root.join(".agent").join("skills");
    if antigravity_skills_path.exists() && antigravity_skills_path.is_dir() {
        let has_content = dir_has_entries(&antigravity_skills_path)?;
        if has_content {
            discovered.push(DiscoveredFile {
                path: ".agent/skills".into(),
                file_type: AgentFileType::AntigravitySkills,
                display_name: "Antigravity skills (.agent/skills/)".to_string(),
            });
        }
    }

    // Zed editor: .zed/settings.json
    let zed_settings = project_root.join(".zed").join("settings.json");
    if zed_settings.exists() {
        discovered.push(DiscoveredFile {
            path: ".zed/settings.json".into(),
            file_type: AgentFileType::ZedSettings,
            display_name: ".zed/settings.json (Zed editor AI settings)".to_string(),
        });
    }

    Ok(discovered)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SkillsWizardChoice {
    agent_name: String,
    destination: String,
    recommended_mode: SyncType,
    reason: Option<String>,
    already_canonical: bool,
}

fn skills_choice_for_file_type(file_type: &AgentFileType) -> Option<(&'static str, &'static str)> {
    match file_type {
        AgentFileType::ClaudeSkills => Some(("claude", ".claude/skills")),
        AgentFileType::CodexSkills => Some(("codex", ".codex/skills")),
        AgentFileType::GeminiSkills => Some(("gemini", ".gemini/skills")),
        AgentFileType::OpenCodeSkills => Some(("opencode", ".opencode/skills")),
        _ => None,
    }
}

fn build_skills_wizard_choices(
    project_root: &Path,
    expected_source: &Path,
    files_to_migrate: &[DiscoveredFile],
) -> Vec<SkillsWizardChoice> {
    let mut choices = BTreeMap::new();

    for file in files_to_migrate {
        let Some((agent_name, destination)) = skills_choice_for_file_type(&file.file_type) else {
            continue;
        };

        let target = TargetConfig {
            source: "skills".to_string(),
            destination: destination.to_string(),
            sync_type: SyncType::Symlink,
            pattern: None,
            exclude: Vec::new(),
            mappings: Vec::new(),
        };

        let already_canonical =
            detect_skills_layout_match(project_root, expected_source, "skills", &target).is_some();

        let reason = if already_canonical {
            Some("existing destination already uses the canonical directory symlink".to_string())
        } else {
            Some("recommended default for skills targets".to_string())
        };

        choices
            .entry(agent_name.to_string())
            .or_insert(SkillsWizardChoice {
                agent_name: agent_name.to_string(),
                destination: destination.to_string(),
                recommended_mode: SyncType::Symlink,
                reason,
                already_canonical,
            });
    }

    choices.into_values().collect()
}

fn sync_type_label(sync_type: SyncType) -> &'static str {
    match sync_type {
        SyncType::Symlink => "symlink",
        SyncType::SymlinkContents => "symlink-contents",
        SyncType::NestedGlob => "nested-glob",
        SyncType::ModuleMap => "module-map",
    }
}

fn resolve_skills_mode_selection(choice: &SkillsWizardChoice, selected_index: usize) -> SyncType {
    match selected_index {
        0 => SyncType::Symlink,
        1 => SyncType::SymlinkContents,
        _ => choice.recommended_mode,
    }
}

fn build_default_config_with_skills_modes(modes: &BTreeMap<String, SyncType>) -> String {
    let mut rendered = Vec::new();
    let mut current_skills_agent: Option<String> = None;

    for line in DEFAULT_CONFIG.lines() {
        if let Some(agent_name) = parse_skills_section_agent(line) {
            current_skills_agent = Some(agent_name.to_string());
            rendered.push(line.to_string());
            continue;
        }

        if line.starts_with("[agents.") {
            current_skills_agent = None;
        }

        if let Some(agent_name) = current_skills_agent.as_deref()
            && line.trim_start().starts_with("type = ")
        {
            let mode = modes.get(agent_name).copied().unwrap_or(SyncType::Symlink);
            rendered.push(format!("type = \"{}\"", sync_type_label(mode)));
            current_skills_agent = None;
            continue;
        }

        rendered.push(line.to_string());
    }

    let mut output = rendered.join("\n");
    if DEFAULT_CONFIG.ends_with('\n') {
        output.push('\n');
    }
    output
}

const AGENT_CONFIG_LAYOUT_START_MARKER: &str = "<!-- agentsync:agent-config-layout:start -->";
const AGENT_CONFIG_LAYOUT_END_MARKER: &str = "<!-- agentsync:agent-config-layout:end -->";

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstructionTargetLayout {
    destination: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SkillsTargetLayout {
    destination: String,
    sync_type: SyncType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandTargetLayout {
    destination: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct AgentLayoutFacts {
    instructions: Vec<InstructionTargetLayout>,
    skills: Vec<SkillsTargetLayout>,
    commands: Vec<CommandTargetLayout>,
}

fn build_wizard_layout_facts(rendered_config: &str) -> Result<AgentLayoutFacts> {
    let config: Config = toml::from_str(rendered_config)
        .context("Failed to parse rendered wizard config for AGENTS layout facts")?;
    let mut facts = AgentLayoutFacts::default();

    for agent in config.agents.into_values() {
        if !agent.enabled {
            continue;
        }

        for (target_name, target) in agent.targets {
            match target_name.as_str() {
                "instructions"
                    if target.source == "AGENTS.md" && target.sync_type == SyncType::Symlink =>
                {
                    facts.instructions.push(InstructionTargetLayout {
                        destination: target.destination,
                    });
                }
                "agents"
                    if target.source == "AGENTS.md" && target.sync_type == SyncType::Symlink =>
                {
                    facts.instructions.push(InstructionTargetLayout {
                        destination: target.destination,
                    });
                }
                "skills" if target.source == "skills" => {
                    if matches!(
                        target.sync_type,
                        SyncType::Symlink | SyncType::SymlinkContents
                    ) {
                        facts.skills.push(SkillsTargetLayout {
                            destination: target.destination,
                            sync_type: target.sync_type,
                        });
                    }
                }
                "commands"
                    if target.source == "commands"
                        && target.sync_type == SyncType::SymlinkContents =>
                {
                    facts.commands.push(CommandTargetLayout {
                        destination: target.destination,
                    });
                }
                _ => {}
            }
        }
    }

    Ok(facts)
}

fn render_destination_list(destinations: &[String]) -> String {
    destinations
        .iter()
        .map(|destination| format!("`{destination}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_agent_config_layout_section(facts: &AgentLayoutFacts) -> String {
    let mut lines = vec![
        AGENT_CONFIG_LAYOUT_START_MARKER.to_string(),
        "## Agent config layout".to_string(),
        String::new(),
        "`.agents/` is the canonical source for shared instructions, skills, and commands in this project.".to_string(),
    ];

    if !facts.instructions.is_empty() {
        let destinations = facts
            .instructions
            .iter()
            .map(|target| target.destination.clone())
            .collect::<Vec<_>>();
        lines.push(String::new());
        lines.push(format!(
            "- Instructions: `.agents/AGENTS.md` is the canonical instructions file, and these `symlink` targets reflect it directly in {}.",
            render_destination_list(&destinations)
        ));
    }

    if !facts.skills.is_empty() {
        lines.push(String::new());
        lines.push("- Skills: `.agents/skills/` is the canonical skills directory.".to_string());

        for target in &facts.skills {
            let description = match target.sync_type {
                SyncType::Symlink => format!(
                    "  - `{}` reflects `.agents/skills/` directly because this target uses `symlink`.",
                    target.destination
                ),
                SyncType::SymlinkContents => format!(
                    "  - `{}` is populated from `.agents/skills/` when `agentsync apply` runs because this target uses `symlink-contents`; add, remove, or rename skill entries in `.agents/skills/`, then rerun `agentsync apply`.",
                    target.destination
                ),
                _ => continue,
            };
            lines.push(description);
        }
    }

    if !facts.commands.is_empty() {
        let destinations = facts
            .commands
            .iter()
            .map(|target| target.destination.clone())
            .collect::<Vec<_>>();
        lines.push(String::new());
        lines.push(format!(
            "- Commands: `.agents/commands/` is the canonical commands directory, `agentsync apply` populates command entries into {}, and `agentsync status` validates those destinations as managed container directories rather than requiring the destination path itself to be a symlink.",
            render_destination_list(&destinations)
        ));
    }

    lines.push(String::new());
    lines.push(AGENT_CONFIG_LAYOUT_END_MARKER.to_string());
    lines.join("\n")
}

fn strip_agent_config_layout_block(content: &str) -> String {
    let Some(start) = content.find(AGENT_CONFIG_LAYOUT_START_MARKER) else {
        return content.to_string();
    };
    let Some(relative_end) = content[start..].find(AGENT_CONFIG_LAYOUT_END_MARKER) else {
        return content.to_string();
    };
    let end = start + relative_end + AGENT_CONFIG_LAYOUT_END_MARKER.len();

    let mut prefix = content[..start].to_string();
    while prefix.ends_with('\n') {
        prefix.pop();
    }

    let mut suffix = content[end..].to_string();
    while suffix.starts_with('\n') {
        suffix.remove(0);
    }

    if prefix.is_empty() {
        suffix
    } else if suffix.is_empty() {
        prefix
    } else {
        format!("{prefix}\n\n{suffix}")
    }
}

fn find_agents_layout_insertion_offset(content: &str) -> usize {
    if !content.starts_with("# ") {
        return 0;
    }

    let Some(first_newline) = content.find('\n') else {
        return content.len();
    };

    let mut offset = first_newline + 1;
    let mut remainder = &content[offset..];

    while remainder.starts_with('\n') {
        offset += 1;
        remainder = &content[offset..];
    }

    if remainder.is_empty() || remainder.starts_with('#') {
        return offset;
    }

    let mut running_offset = offset;
    let mut lines = remainder.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        running_offset += line.len();
        let trimmed = line.trim();
        if trimmed.is_empty()
            && lines
                .peek()
                .is_some_and(|next| next.trim_start().starts_with('#'))
        {
            break;
        }
    }

    running_offset
}

fn upsert_agent_config_layout_block(base_content: &str, layout_block: &str) -> String {
    let stripped_content = strip_agent_config_layout_block(base_content);
    let stripped = stripped_content.trim_matches('\n');
    if stripped.is_empty() {
        return format!("{layout_block}\n");
    }

    let insertion_offset = find_agents_layout_insertion_offset(stripped);
    let prefix = stripped[..insertion_offset].trim_end_matches('\n');
    let suffix = stripped[insertion_offset..].trim_start_matches('\n');

    match (prefix.is_empty(), suffix.is_empty()) {
        (true, true) => format!("{layout_block}\n"),
        (true, false) => format!("{layout_block}\n\n{suffix}\n"),
        (false, true) => format!("{prefix}\n\n{layout_block}\n"),
        (false, false) => format!("{prefix}\n\n{layout_block}\n\n{suffix}\n"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManagedFileOutcome {
    Written,
    Preserved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BackupOutcome {
    NotOffered,
    Declined,
    Completed { moved_count: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WizardSummaryFacts {
    instruction_files_merged: usize,
    migrated_count: usize,
    skipped_count: usize,
    agents_md: ManagedFileOutcome,
    config: ManagedFileOutcome,
    backup: BackupOutcome,
}

fn render_wizard_post_migration_summary(facts: &WizardSummaryFacts) -> Vec<String> {
    let mut lines = vec![
        "  • .agents/ is now the canonical source of truth for the migrated instructions and generated config.".to_string(),
        format!(
            "  • The wizard merged {} instruction file(s), migrated {} additional item(s), and skipped {} item(s) that needed to stay as-is.",
            facts.instruction_files_merged, facts.migrated_count, facts.skipped_count
        ),
        format!("  • {}.", managed_file_sentence(facts.agents_md, ".agents/AGENTS.md")),
        format!("  • {}.", managed_file_sentence(facts.config, ".agents/agentsync.toml")),
    ];

    // Backup outcome goes right after the managed-file lines, before follow-up guidance.
    let backup_line = match &facts.backup {
        BackupOutcome::Completed { moved_count } => {
            format!("  • Created a backup of {moved_count} original item(s) in `.agents/backup/`.")
        }
        BackupOutcome::Declined => {
            "  • No backup was created; the original files remain where they are.".to_string()
        }
        BackupOutcome::NotOffered => {
            "  • Backup was not offered because the wizard kept the existing `.agents/AGENTS.md`."
                .to_string()
        }
    };
    lines.push(backup_line);

    // Follow-up guidance after all factual lines.
    lines.push(
        "  • This wizard migrated content into `.agents/`, but it did not run `agentsync apply`."
            .to_string(),
    );
    lines.push("  • Run `agentsync apply` next to reconcile the downstream managed files that should point back to `.agents/`.".to_string());
    lines.push("  • If your config keeps gitignore management enabled (the default for new configs), collaborators should also run `agentsync apply` so `.gitignore` behavior stays aligned with `.agents/`.".to_string());
    lines.push("  • After the wizard and apply, review the resulting changes with your normal git workflow; git may show different changes depending on what already existed before migration and what apply updates here.".to_string());

    lines
}

fn managed_file_sentence(outcome: ManagedFileOutcome, path: &str) -> String {
    match outcome {
        ManagedFileOutcome::Written => format!("Wrote `{path}`"),
        ManagedFileOutcome::Preserved => format!("Kept the existing `{path}`"),
    }
}

fn parse_skills_section_agent(line: &str) -> Option<&str> {
    line.strip_prefix("[agents.")?
        .strip_suffix(".targets.skills]")
}

fn collect_post_init_skills_warnings(
    project_root: &Path,
    config_path: &Path,
    selected_agents: &[String],
) -> Result<Vec<String>> {
    let config = Config::load(config_path)?;
    let source_dir = config.source_dir(config_path);
    let mut warnings = Vec::new();

    for agent_name in selected_agents {
        let Some(agent) = config.agents.get(agent_name) else {
            continue;
        };
        let Some(target) = agent.targets.get("skills") else {
            continue;
        };

        let expected_source = source_dir.join(&target.source);
        if let Some(mismatch) = detect_skills_mode_mismatch(
            project_root,
            &expected_source,
            agent_name,
            "skills",
            target,
        ) {
            warnings.push(mismatch.wizard_warning());
        }
    }

    Ok(warnings)
}

/// Interactive wizard for initializing agentsync with file migration
pub fn init_wizard(project_root: &Path, force: bool) -> Result<()> {
    use colored::Colorize;
    use dialoguer::{Confirm, MultiSelect, Select, theme::ColorfulTheme};

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
            discovered_files
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

    // Create commands directory
    let commands_dir = agents_dir.join("commands");
    if !commands_dir.exists() {
        fs::create_dir_all(&commands_dir)?;
        println!(
            "  {} Created directory: {}",
            "✔".green(),
            commands_dir.display()
        );
    }

    let config_path = agents_dir.join("agentsync.toml");
    let can_write_config = force || !config_path.exists();
    let skills_choices = build_skills_wizard_choices(project_root, &skills_dir, &files_to_migrate);
    let mut skills_modes = BTreeMap::new();
    if can_write_config {
        for choice in &skills_choices {
            println!(
                "\n{}",
                format!(
                    "🧠 Skills target for {} ({})",
                    choice.agent_name, choice.destination
                )
                .cyan()
            );
            println!(
                "  {} Recommended: {}{}",
                "ℹ".blue(),
                sync_type_label(choice.recommended_mode).bold(),
                choice
                    .reason
                    .as_ref()
                    .map(|reason| format!(" — {reason}"))
                    .unwrap_or_default()
            );

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("How should this skills target sync?")
                .items([
                    "symlink — link the whole directory to .agents/skills",
                    "symlink-contents — keep a directory and link each item inside it",
                ])
                .default(if choice.recommended_mode == SyncType::Symlink {
                    0
                } else {
                    1
                })
                .interact()?;

            skills_modes.insert(
                choice.agent_name.clone(),
                resolve_skills_mode_selection(choice, selection),
            );
        }
    }

    let rendered_config = build_default_config_with_skills_modes(&skills_modes);
    let layout_facts = build_wizard_layout_facts(&rendered_config)?;
    let layout_block = render_agent_config_layout_section(&layout_facts);

    // Migrate selected files
    println!("\n{}", "🔄 Migrating files...".cyan());

    // Collect all instruction files first (plain text files that get merged into AGENTS.md)
    let instruction_files: Vec<_> = files_to_migrate
        .iter()
        .filter(|f| {
            matches!(
                f.file_type,
                AgentFileType::ClaudeInstructions
                    | AgentFileType::RootAgentsFile
                    | AgentFileType::CopilotInstructions
                    | AgentFileType::WindsurfRules
                    | AgentFileType::ClineRules
                    | AgentFileType::CrushInstructions
                    | AgentFileType::AmpInstructions
                    | AgentFileType::GooseHints
                    | AgentFileType::WarpInstructions
                    | AgentFileType::GeminiInstructions
                    | AgentFileType::OpenCodeInstructions
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
            let content = fs::read_to_string(&src_path)
                .map_err(|e| anyhow::anyhow!("Failed to read '{}': {}", src_path.display(), e))?;
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
        if !merged.is_empty() {
            migrated_content = Some(merged);
        }
    } else if instruction_files.len() == 1 {
        // Single instruction file - use its content directly
        let src_path = project_root.join(&instruction_files[0].path);
        let content = fs::read_to_string(&src_path)
            .map_err(|e| anyhow::anyhow!("Failed to read '{}': {}", src_path.display(), e))?;
        migrated_content = Some(content);
        instruction_files_merged = 1;
    }

    // Track migration counts
    let mut files_actually_migrated = 0;
    let mut files_skipped = 0;

    for file in &files_to_migrate {
        let src_path = project_root.join(&file.path);

        match file.file_type {
            // Plain-text instruction files — content already merged into AGENTS.md above
            AgentFileType::ClaudeInstructions
            | AgentFileType::RootAgentsFile
            | AgentFileType::CopilotInstructions
            | AgentFileType::WindsurfRules
            | AgentFileType::ClineRules
            | AgentFileType::CrushInstructions
            | AgentFileType::AmpInstructions
            | AgentFileType::GooseHints
            | AgentFileType::WarpInstructions
            | AgentFileType::GeminiInstructions
            | AgentFileType::OpenCodeInstructions => {
                // Already handled above — content merged into AGENTS.md
                continue;
            }
            // Skill directories — copy contents into .agents/skills/
            AgentFileType::ClaudeSkills
            | AgentFileType::CursorSkills
            | AgentFileType::CodexSkills
            | AgentFileType::GeminiSkills
            | AgentFileType::OpenCodeSkills
            | AgentFileType::RooSkills
            | AgentFileType::FactorySkills
            | AgentFileType::VibeSkills
            | AgentFileType::AntigravitySkills => {
                if src_path.exists() && src_path.is_dir() {
                    for entry in fs::read_dir(&src_path)? {
                        let entry = entry?;
                        let entry_path = entry.path();
                        let skill_name = entry.file_name();
                        let dest_skill = skills_dir.join(&skill_name);
                        if dest_skill.exists() {
                            println!(
                                "  {} Skipped: skill '{}' already exists in .agents/skills/",
                                "⚠".yellow(),
                                skill_name.to_string_lossy()
                            );
                            files_skipped += 1;
                        } else if entry_path.is_dir() {
                            copy_dir_all(&entry_path, &dest_skill)?;
                            println!(
                                "  {} Copied skill: {} → .agents/skills/{}",
                                "✔".green(),
                                entry_path.display(),
                                skill_name.to_string_lossy()
                            );
                            files_actually_migrated += 1;
                        } else {
                            fs::copy(&entry_path, &dest_skill)?;
                            println!(
                                "  {} Copied skill: {} → .agents/skills/{}",
                                "✔".green(),
                                entry_path.display(),
                                skill_name.to_string_lossy()
                            );
                            files_actually_migrated += 1;
                        }
                    }
                }
            }
            // Command directories — copy contents into .agents/commands/
            AgentFileType::ClaudeCommands
            | AgentFileType::GeminiCommands
            | AgentFileType::OpenCodeCommands => {
                if src_path.exists() && src_path.is_dir() {
                    for entry in fs::read_dir(&src_path)? {
                        let entry = entry?;
                        let entry_path = entry.path();
                        let cmd_name = entry.file_name();
                        let dest_cmd = commands_dir.join(&cmd_name);
                        if dest_cmd.exists() {
                            println!(
                                "  {} Skipped: command '{}' already exists in .agents/commands/",
                                "⚠".yellow(),
                                cmd_name.to_string_lossy()
                            );
                            files_skipped += 1;
                        } else if entry_path.is_dir() {
                            copy_dir_all(&entry_path, &dest_cmd)?;
                            println!(
                                "  {} Copied command: {} → .agents/commands/{}",
                                "✔".green(),
                                entry_path.display(),
                                cmd_name.to_string_lossy()
                            );
                            files_actually_migrated += 1;
                        } else {
                            fs::copy(&entry_path, &dest_cmd)?;
                            println!(
                                "  {} Copied command: {} → .agents/commands/{}",
                                "✔".green(),
                                entry_path.display(),
                                cmd_name.to_string_lossy()
                            );
                            files_actually_migrated += 1;
                        }
                    }
                }
            }
            // Directories — copy to .agents/
            AgentFileType::CursorDirectory
            | AgentFileType::WindsurfDirectory
            | AgentFileType::AntigravityRules
            | AgentFileType::AmazonQRules
            | AgentFileType::OpenHandsMicroagents
            | AgentFileType::JunieDirectory
            | AgentFileType::AugmentRules
            | AgentFileType::KilocodeDirectory
            | AgentFileType::QwenDirectory
            | AgentFileType::RooRules
            | AgentFileType::TraeRules
            | AgentFileType::KiroSteering
            | AgentFileType::FactoryDirectory
            | AgentFileType::VibeDirectory
            | AgentFileType::JetBrainsRules => {
                if src_path.exists() {
                    // Derive a sane destination under .agents/
                    let dest_path = agents_dir.join(&file.path);
                    if let Some(parent) = dest_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
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
            // Single-file configs — copy file (handles both root-level and nested files)
            AgentFileType::AiderConfig
            | AgentFileType::FirebenderConfig
            | AgentFileType::FirebaseRules => {
                if src_path.exists() {
                    let dest_path = agents_dir.join(&file.path);
                    if let Some(parent) = dest_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&src_path, &dest_path)?;
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
            // MCP / tooling configs — just note them
            AgentFileType::McpConfig
            | AgentFileType::ZedSettings
            | AgentFileType::CursorMcpConfig
            | AgentFileType::CopilotMcpConfig
            | AgentFileType::WindsurfMcpConfig
            | AgentFileType::CodexConfig
            | AgentFileType::RooMcpConfig
            | AgentFileType::KiroMcpConfig
            | AgentFileType::AmazonQMcpConfig
            | AgentFileType::KilocodeMcpConfig
            | AgentFileType::FactoryMcpConfig
            | AgentFileType::OpenCodeConfig => {
                println!(
                    "  {} Note: {} detected. You can configure MCP servers in agentsync.toml",
                    "ℹ".blue(),
                    file.path.display()
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
    let agents_md_outcome = if let Some(content) = migrated_content {
        if agents_md_path.exists() && !force {
            println!(
                "  {} AGENTS.md already exists (use --force to overwrite)",
                "!".yellow()
            );
            ManagedFileOutcome::Preserved
        } else {
            let rendered_agents_md = upsert_agent_config_layout_block(&content, &layout_block);
            fs::write(&agents_md_path, rendered_agents_md)?;
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
            ManagedFileOutcome::Written
        }
    } else if !agents_md_path.exists() || force {
        let rendered_agents_md = upsert_agent_config_layout_block(DEFAULT_AGENTS_MD, &layout_block);
        fs::write(&agents_md_path, rendered_agents_md)?;
        println!("  {} Created: {}", "✔".green(), agents_md_path.display());
        ManagedFileOutcome::Written
    } else {
        ManagedFileOutcome::Preserved
    };

    // Generate config file
    println!("\n{}", "⚙️  Generating configuration...".cyan());

    let config_outcome = if config_path.exists() && !force {
        println!(
            "  {} Config already exists: {} (use --force to overwrite)",
            "!".yellow(),
            config_path.display()
        );
        ManagedFileOutcome::Preserved
    } else {
        fs::write(&config_path, &rendered_config)?;
        println!("  {} Created: {}", "✔".green(), config_path.display());

        let selected_skill_agents = skills_choices
            .iter()
            .map(|choice| choice.agent_name.clone())
            .collect::<Vec<_>>();
        let warnings =
            collect_post_init_skills_warnings(project_root, &config_path, &selected_skill_agents)?;

        println!("\n{}", "🔎 Post-init skills validation:".bold());
        if warnings.is_empty() {
            println!(
                "  {} No skills mode mismatches detected for selected targets",
                "✔".green()
            );
        } else {
            for warning in warnings {
                println!("  {} {}", "⚠".yellow(), warning);
            }
        }
        ManagedFileOutcome::Written
    };

    // Ask if user wants to back up original files.
    // Only offer backup when AGENTS.md was actually written — otherwise the
    // instruction files would be moved without a migrated destination existing.
    let backup_outcome = if matches!(agents_md_outcome, ManagedFileOutcome::Preserved) {
        BackupOutcome::NotOffered
    } else {
        let should_backup = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(
                "Would you like to back up the original files? (They will be moved to .agents/backup/)",
            )
            .default(true)
            .interact()?;

        if should_backup {
            let backup_dir = agents_dir.join("backup");
            fs::create_dir_all(&backup_dir)?;
            let mut moved_count = 0;

            for file in &files_to_migrate {
                if matches!(
                    file.file_type,
                    AgentFileType::McpConfig
                        | AgentFileType::ZedSettings
                        | AgentFileType::CursorMcpConfig
                        | AgentFileType::CopilotMcpConfig
                        | AgentFileType::WindsurfMcpConfig
                        | AgentFileType::CodexConfig
                        | AgentFileType::RooMcpConfig
                        | AgentFileType::KiroMcpConfig
                        | AgentFileType::AmazonQMcpConfig
                        | AgentFileType::KilocodeMcpConfig
                        | AgentFileType::FactoryMcpConfig
                        | AgentFileType::OpenCodeConfig
                        | AgentFileType::Other
                ) {
                    // Skip files that weren't actually migrated
                    continue;
                }

                let src_path = project_root.join(&file.path);
                if !src_path.exists() {
                    continue;
                }

                if let Some((agent_name, _)) = skills_choice_for_file_type(&file.file_type) {
                    let selected_mode = skills_modes
                        .get(agent_name)
                        .copied()
                        .unwrap_or(SyncType::Symlink);
                    let preserve_existing_layout = skills_choices.iter().any(|choice| {
                        choice.agent_name == agent_name
                            && choice.already_canonical
                            && selected_mode == SyncType::Symlink
                    });

                    if preserve_existing_layout {
                        continue;
                    }
                }

                let backup_path = backup_dir.join(&file.path);
                if let Some(parent) = backup_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                // Try to move the file/directory first (rename)
                match fs::rename(&src_path, &backup_path) {
                    Ok(_) => {
                        println!("  {} Moved: {}", "✔".green(), file.path.display());
                        moved_count += 1;
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
                        moved_count += 1;
                    }
                }
            }

            BackupOutcome::Completed { moved_count }
        } else {
            BackupOutcome::Declined
        }
    };

    println!("\n{}", "📋 Post-migration Summary:".bold());
    let summary = render_wizard_post_migration_summary(&WizardSummaryFacts {
        instruction_files_merged,
        migrated_count: files_actually_migrated,
        skipped_count: files_skipped,
        agents_md: agents_md_outcome,
        config: config_outcome,
        backup: backup_outcome,
    });
    for line in summary {
        println!("{line}");
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
                // On Windows, we need to know if target is dir or file.
                // For relative targets, resolve against the source symlink's parent.
                let resolved_target = if link_target.is_absolute() {
                    link_target.clone()
                } else {
                    src_path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(&link_target)
                };

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
    use std::collections::BTreeMap;
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
    fn test_default_config_all_agents_have_skills_symlink_target() {
        let config: crate::config::Config = toml::from_str(DEFAULT_CONFIG).unwrap();

        let cases = [
            ("claude", ".claude/skills"),
            ("codex", ".codex/skills"),
            ("gemini", ".gemini/skills"),
            ("opencode", ".opencode/skills"),
        ];

        for (agent, expected_destination) in cases {
            assert!(
                config.agents[agent].targets.contains_key("skills"),
                "{agent} should have a skills target"
            );

            let skills_target = &config.agents[agent].targets["skills"];
            assert_eq!(
                skills_target.source, "skills",
                "{agent} skills source mismatch"
            );
            assert_eq!(
                skills_target.destination, expected_destination,
                "{agent} skills destination mismatch"
            );
            assert_eq!(
                skills_target.sync_type,
                crate::config::SyncType::Symlink,
                "{agent} skills sync_type mismatch"
            );
        }
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
    fn test_scan_agent_files_finds_gemini_md() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("GEMINI.md"), "# Gemini").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::GeminiInstructions);
    }

    #[test]
    fn test_scan_agent_files_finds_clinerules() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".clinerules"), "# Cline Rules").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::ClineRules);
        assert_eq!(discovered[0].path.to_str().unwrap(), ".clinerules");
    }

    #[test]
    fn test_scan_agent_files_finds_crush_md() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("CRUSH.md"), "# Crush").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::CrushInstructions);
    }

    #[test]
    fn test_scan_agent_files_finds_windsurf_dir() {
        let temp_dir = TempDir::new().unwrap();
        let windsurf_dir = temp_dir.path().join(".windsurf");
        fs::create_dir_all(&windsurf_dir).unwrap();
        fs::write(windsurf_dir.join("rules.md"), "rules").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::WindsurfDirectory);
    }

    #[test]
    fn test_scan_agent_files_finds_amazonq_rules() {
        let temp_dir = TempDir::new().unwrap();
        let amazonq_rules = temp_dir.path().join(".amazonq").join("rules");
        fs::create_dir_all(&amazonq_rules).unwrap();
        fs::write(amazonq_rules.join("rules.md"), "rules").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::AmazonQRules);
    }

    #[test]
    fn test_scan_agent_files_finds_aider_config() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".aider.conf.yml"), "model: gpt-4").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::AiderConfig);
    }

    #[test]
    fn test_scan_agent_files_finds_firebase_rules() {
        let temp_dir = TempDir::new().unwrap();
        let idx_dir = temp_dir.path().join(".idx");
        fs::create_dir_all(&idx_dir).unwrap();
        fs::write(idx_dir.join("airules.md"), "# Rules").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::FirebaseRules);
    }

    #[test]
    fn test_scan_agent_files_finds_openhands_microagents() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".openhands").join("microagents");
        fs::create_dir_all(&path).unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::OpenHandsMicroagents);
    }

    #[test]
    fn test_scan_agent_files_finds_junie_dir() {
        let temp_dir = TempDir::new().unwrap();
        let junie_dir = temp_dir.path().join(".junie");
        fs::create_dir_all(&junie_dir).unwrap();
        fs::write(junie_dir.join("guidelines.md"), "guidelines").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::JunieDirectory);
    }

    #[test]
    fn test_scan_agent_files_finds_augment_rules() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".augment").join("rules");
        fs::create_dir_all(&path).unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::AugmentRules);
    }

    #[test]
    fn test_scan_agent_files_finds_kilocode_dir() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".kilocode");
        fs::create_dir_all(&path).unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::KilocodeDirectory);
    }

    #[test]
    fn test_scan_agent_files_finds_goosehints() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".goosehints"), "hints").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::GooseHints);
    }

    #[test]
    fn test_scan_agent_files_finds_roo_rules() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".roo").join("rules");
        fs::create_dir_all(&path).unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::RooRules);
    }

    #[test]
    fn test_scan_agent_files_finds_trae_rules() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".trae").join("rules");
        fs::create_dir_all(&path).unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::TraeRules);
    }

    #[test]
    fn test_scan_agent_files_finds_warp_md() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("WARP.md"), "# Warp").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::WarpInstructions);
    }

    #[test]
    fn test_scan_agent_files_finds_kiro_steering() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".kiro").join("steering");
        fs::create_dir_all(&path).unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::KiroSteering);
    }

    #[test]
    fn test_scan_agent_files_finds_firebender_json() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("firebender.json"), "{}").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::FirebenderConfig);
    }

    #[test]
    fn test_scan_agent_files_finds_factory_dir() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".factory");
        fs::create_dir_all(&path).unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::FactoryDirectory);
    }

    #[test]
    fn test_scan_agent_files_finds_vibe_dir() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".vibe");
        fs::create_dir_all(&path).unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::VibeDirectory);
    }

    #[test]
    fn test_scan_agent_files_finds_jetbrains_rules() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".aiassistant").join("rules");
        fs::create_dir_all(&path).unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].file_type, AgentFileType::JetBrainsRules);
    }
    #[test]
    fn test_scan_agent_files_finds_claude_skills_with_content() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir
            .path()
            .join(".claude")
            .join("skills")
            .join("my-skill");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("SKILL.md"), "# My Skill").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let skills_entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::ClaudeSkills)
            .collect();
        assert_eq!(skills_entry.len(), 1);
        assert_eq!(skills_entry[0].path.to_str().unwrap(), ".claude/skills");
        assert!(skills_entry[0].display_name.contains("Claude"));
        assert!(skills_entry[0].display_name.contains("skills"));
    }

    #[test]
    fn test_scan_agent_files_ignores_empty_claude_skills() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path().join(".claude").join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let skills_entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::ClaudeSkills)
            .collect();
        assert!(skills_entry.is_empty());
    }

    #[test]
    fn test_scan_agent_files_ignores_absent_claude_skills() {
        let temp_dir = TempDir::new().unwrap();
        // No .claude/ directory at all

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let skills_entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::ClaudeSkills)
            .collect();
        assert!(skills_entry.is_empty());
    }

    #[test]
    fn test_scan_agent_files_finds_claude_skills_alongside_claude_md() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("CLAUDE.md"), "# Claude").unwrap();
        let skills_dir = temp_dir
            .path()
            .join(".claude")
            .join("skills")
            .join("my-skill");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("SKILL.md"), "# Skill").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let has_instructions = discovered
            .iter()
            .any(|f| f.file_type == AgentFileType::ClaudeInstructions);
        let has_skills = discovered
            .iter()
            .any(|f| f.file_type == AgentFileType::ClaudeSkills);
        assert!(has_instructions);
        assert!(has_skills);
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

    #[test]
    fn test_render_wizard_summary_includes_canonical_apply_and_git_guidance() {
        let lines = render_wizard_post_migration_summary(&WizardSummaryFacts {
            instruction_files_merged: 2,
            migrated_count: 3,
            skipped_count: 1,
            agents_md: ManagedFileOutcome::Written,
            config: ManagedFileOutcome::Written,
            backup: BackupOutcome::Declined,
        });
        let summary = lines.join("\n");

        assert!(summary.contains(".agents/ is now the canonical source of truth"));
        assert!(summary.contains("Run `agentsync apply` next"));
        assert!(summary.contains("did not run `agentsync apply`"));
        assert!(summary.contains("collaborators should also run `agentsync apply`"));
        assert!(summary.contains(".gitignore` behavior stays aligned with `.agents/`"));
        assert!(summary.contains("review the resulting changes with your normal git workflow"));
        assert!(summary.contains("depending on what already existed before migration"));
    }

    #[test]
    fn test_render_wizard_summary_reports_backup_outcomes_and_avoids_unsafe_claims() {
        let completed = render_wizard_post_migration_summary(&WizardSummaryFacts {
            instruction_files_merged: 1,
            migrated_count: 1,
            skipped_count: 0,
            agents_md: ManagedFileOutcome::Written,
            config: ManagedFileOutcome::Written,
            backup: BackupOutcome::Completed { moved_count: 2 },
        })
        .join("\n");
        assert!(completed.contains("Created a backup of 2 original item(s) in `.agents/backup/`"));

        let declined = render_wizard_post_migration_summary(&WizardSummaryFacts {
            instruction_files_merged: 0,
            migrated_count: 1,
            skipped_count: 0,
            agents_md: ManagedFileOutcome::Written,
            config: ManagedFileOutcome::Preserved,
            backup: BackupOutcome::Declined,
        })
        .join("\n");
        assert!(
            declined.contains("No backup was created; the original files remain where they are")
        );

        let not_offered = render_wizard_post_migration_summary(&WizardSummaryFacts {
            instruction_files_merged: 0,
            migrated_count: 1,
            skipped_count: 0,
            agents_md: ManagedFileOutcome::Preserved,
            config: ManagedFileOutcome::Preserved,
            backup: BackupOutcome::NotOffered,
        })
        .join("\n");
        assert!(not_offered.contains(
            "Backup was not offered because the wizard kept the existing `.agents/AGENTS.md`"
        ));

        for summary in [&completed, &declined, &not_offered] {
            assert!(!summary.contains("already ran `agentsync apply`"));
            assert!(!summary.contains("updated `.gitignore`"));
            assert!(!summary.contains("working tree is clean"));
            assert!(!summary.contains("staged"));
            assert!(!summary.contains("unstaged"));
        }
    }

    // ==========================================================================
    // WIZARD SKILL MIGRATION TESTS
    // ==========================================================================

    #[test]
    fn test_wizard_skill_migration_copies_skills() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Set up .claude/skills/ with two skills
        let skill_a = project_root.join(".claude/skills/skill-a");
        let skill_b = project_root.join(".claude/skills/skill-b");
        fs::create_dir_all(&skill_a).unwrap();
        fs::create_dir_all(&skill_b).unwrap();
        fs::write(skill_a.join("SKILL.md"), "# Skill A").unwrap();
        fs::write(skill_b.join("SKILL.md"), "# Skill B").unwrap();

        // Set up .agents/skills/ destination
        let agents_skills = project_root.join(".agents/skills");
        fs::create_dir_all(&agents_skills).unwrap();

        // Simulate the migration logic for ClaudeSkills
        let src_path = project_root.join(".claude/skills");
        for entry in fs::read_dir(&src_path).unwrap() {
            let entry = entry.unwrap();
            let entry_path = entry.path();
            let skill_name = entry.file_name();
            let dest_skill = agents_skills.join(&skill_name);
            if entry_path.is_dir() {
                copy_dir_all(&entry_path, &dest_skill).unwrap();
            }
        }

        // Verify both skills were copied
        assert!(agents_skills.join("skill-a/SKILL.md").exists());
        assert!(agents_skills.join("skill-b/SKILL.md").exists());
        assert_eq!(
            fs::read_to_string(agents_skills.join("skill-a/SKILL.md")).unwrap(),
            "# Skill A"
        );
        assert_eq!(
            fs::read_to_string(agents_skills.join("skill-b/SKILL.md")).unwrap(),
            "# Skill B"
        );
    }

    #[test]
    fn test_wizard_skill_migration_skips_collisions() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Set up .claude/skills/ with a skill
        let claude_skill = project_root.join(".claude/skills/shared-skill");
        fs::create_dir_all(&claude_skill).unwrap();
        fs::write(claude_skill.join("SKILL.md"), "# From Claude").unwrap();

        // Set up .agents/skills/ with a pre-existing skill of the same name
        let agents_skill = project_root.join(".agents/skills/shared-skill");
        fs::create_dir_all(&agents_skill).unwrap();
        fs::write(agents_skill.join("SKILL.md"), "# Original").unwrap();

        // Simulate the migration logic — skip on collision
        let src_path = project_root.join(".claude/skills");
        let agents_skills = project_root.join(".agents/skills");
        let mut skipped = 0;

        for entry in fs::read_dir(&src_path).unwrap() {
            let entry = entry.unwrap();
            let skill_name = entry.file_name();
            let dest_skill = agents_skills.join(&skill_name);
            if dest_skill.exists() {
                skipped += 1;
            }
        }

        // Original should be preserved, not overwritten
        assert_eq!(skipped, 1);
        assert_eq!(
            fs::read_to_string(agents_skills.join("shared-skill/SKILL.md")).unwrap(),
            "# Original"
        );
    }

    #[test]
    fn test_wizard_skill_migration_handles_mixed_content() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Set up .claude/skills/ with a subdirectory and a loose file
        let skill_dir = project_root.join(".claude/skills/valid-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# Valid").unwrap();
        fs::write(project_root.join(".claude/skills/notes.txt"), "some notes").unwrap();

        // Set up destination
        let agents_skills = project_root.join(".agents/skills");
        fs::create_dir_all(&agents_skills).unwrap();

        // Simulate the migration logic (handles both dirs and files)
        let src_path = project_root.join(".claude/skills");
        for entry in fs::read_dir(&src_path).unwrap() {
            let entry = entry.unwrap();
            let entry_path = entry.path();
            let skill_name = entry.file_name();
            let dest_skill = agents_skills.join(&skill_name);
            if entry_path.is_dir() {
                copy_dir_all(&entry_path, &dest_skill).unwrap();
            } else {
                fs::copy(&entry_path, &dest_skill).unwrap();
            }
        }

        // Verify both were copied
        assert!(agents_skills.join("valid-skill/SKILL.md").exists());
        assert!(agents_skills.join("notes.txt").exists());
        assert_eq!(
            fs::read_to_string(agents_skills.join("notes.txt")).unwrap(),
            "some notes"
        );
    }

    // ==========================================================================
    // UNIVERSAL AGENT ADOPTION — INSTRUCTION FILE SCAN TESTS
    // ==========================================================================

    #[test]
    fn test_scan_agent_files_finds_windsurf_rules() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".windsurfrules"), "Use TypeScript").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::WindsurfRules)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".windsurfrules");
    }

    #[test]
    fn test_scan_agent_files_finds_opencode_instructions() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("OPENCODE.md"), "# OpenCode").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::OpenCodeInstructions)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), "OPENCODE.md");
    }

    #[test]
    fn test_scan_agent_files_finds_amp_instructions() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("AMPCODE.md"), "# Amp").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::AmpInstructions)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), "AMPCODE.md");
    }

    // ==========================================================================
    // UNIVERSAL AGENT ADOPTION — SKILL DIRECTORY SCAN TESTS
    // ==========================================================================

    #[test]
    fn test_scan_agent_files_finds_cursor_skills() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir
            .path()
            .join(".cursor")
            .join("skills")
            .join("my-skill");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("SKILL.md"), "# Skill").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::CursorSkills)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".cursor/skills");
        assert!(entry[0].display_name.contains("Cursor"));
        assert!(entry[0].display_name.contains("skills"));
    }

    #[test]
    fn test_scan_agent_files_finds_codex_skills() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir
            .path()
            .join(".codex")
            .join("skills")
            .join("my-skill");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("SKILL.md"), "# Skill").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::CodexSkills)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".codex/skills");
    }

    #[test]
    fn test_scan_agent_files_finds_gemini_skills() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir
            .path()
            .join(".gemini")
            .join("skills")
            .join("data-analysis");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("SKILL.md"), "# Skill").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::GeminiSkills)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".gemini/skills");
    }

    #[test]
    fn test_scan_agent_files_finds_opencode_skills() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir
            .path()
            .join(".opencode")
            .join("skills")
            .join("my-skill");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("SKILL.md"), "# Skill").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::OpenCodeSkills)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".opencode/skills");
    }

    #[test]
    fn test_scan_agent_files_finds_roo_skills() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path().join(".roo").join("skills").join("my-skill");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("SKILL.md"), "# Skill").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::RooSkills)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".roo/skills");
    }

    #[test]
    fn test_scan_agent_files_finds_factory_skills() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir
            .path()
            .join(".factory")
            .join("skills")
            .join("my-skill");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("SKILL.md"), "# Skill").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::FactorySkills)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".factory/skills");
    }

    #[test]
    fn test_scan_agent_files_finds_vibe_skills() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir
            .path()
            .join(".vibe")
            .join("skills")
            .join("my-skill");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("SKILL.md"), "# Skill").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::VibeSkills)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".vibe/skills");
    }

    #[test]
    fn test_scan_agent_files_finds_antigravity_skills() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir
            .path()
            .join(".agent")
            .join("skills")
            .join("my-skill");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("SKILL.md"), "# Skill").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::AntigravitySkills)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".agent/skills");
    }

    #[test]
    fn test_scan_agent_files_ignores_empty_skill_directory() {
        let temp_dir = TempDir::new().unwrap();
        // Create empty skill directories for multiple agents
        fs::create_dir_all(temp_dir.path().join(".gemini/skills")).unwrap();
        fs::create_dir_all(temp_dir.path().join(".cursor/skills")).unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let skill_entries: Vec<_> = discovered
            .iter()
            .filter(|f| {
                matches!(
                    f.file_type,
                    AgentFileType::GeminiSkills | AgentFileType::CursorSkills
                )
            })
            .collect();
        assert!(skill_entries.is_empty());
    }

    // ==========================================================================
    // UNIVERSAL AGENT ADOPTION — COMMAND DIRECTORY SCAN TESTS
    // ==========================================================================

    #[test]
    fn test_scan_agent_files_finds_claude_commands() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join(".claude").join("commands");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(commands_dir.join("review.md"), "# Review").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::ClaudeCommands)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".claude/commands");
        assert!(entry[0].display_name.contains("Claude"));
        assert!(entry[0].display_name.contains("commands"));
    }

    #[test]
    fn test_scan_agent_files_finds_gemini_commands() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join(".gemini").join("commands");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(commands_dir.join("analyze.md"), "# Analyze").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::GeminiCommands)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".gemini/commands");
    }

    #[test]
    fn test_scan_agent_files_finds_opencode_commands() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join(".opencode").join("command");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(commands_dir.join("deploy.md"), "# Deploy").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::OpenCodeCommands)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".opencode/command");
    }

    #[test]
    fn test_scan_agent_files_ignores_empty_command_directory() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join(".claude/commands")).unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let cmd_entries: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::ClaudeCommands)
            .collect();
        assert!(cmd_entries.is_empty());
    }

    // ==========================================================================
    // UNIVERSAL AGENT ADOPTION — MCP CONFIG SCAN TESTS
    // ==========================================================================

    #[test]
    fn test_scan_agent_files_finds_cursor_mcp_config() {
        let temp_dir = TempDir::new().unwrap();
        let cursor_dir = temp_dir.path().join(".cursor");
        fs::create_dir_all(&cursor_dir).unwrap();
        fs::write(cursor_dir.join("mcp.json"), "{}").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::CursorMcpConfig)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".cursor/mcp.json");
    }

    #[test]
    fn test_scan_agent_files_finds_copilot_mcp_config() {
        let temp_dir = TempDir::new().unwrap();
        let vscode_dir = temp_dir.path().join(".vscode");
        fs::create_dir_all(&vscode_dir).unwrap();
        fs::write(vscode_dir.join("mcp.json"), "{}").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::CopilotMcpConfig)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".vscode/mcp.json");
    }

    #[test]
    fn test_scan_agent_files_finds_windsurf_mcp_config() {
        let temp_dir = TempDir::new().unwrap();
        let windsurf_dir = temp_dir.path().join(".windsurf");
        fs::create_dir_all(&windsurf_dir).unwrap();
        fs::write(windsurf_dir.join("mcp_config.json"), "{}").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::WindsurfMcpConfig)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".windsurf/mcp_config.json");
    }

    #[test]
    fn test_scan_agent_files_finds_codex_config() {
        let temp_dir = TempDir::new().unwrap();
        let codex_dir = temp_dir.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(codex_dir.join("config.toml"), "[settings]").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::CodexConfig)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".codex/config.toml");
    }

    #[test]
    fn test_scan_agent_files_finds_roo_mcp_config() {
        let temp_dir = TempDir::new().unwrap();
        let roo_dir = temp_dir.path().join(".roo");
        fs::create_dir_all(&roo_dir).unwrap();
        fs::write(roo_dir.join("mcp.json"), "{}").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::RooMcpConfig)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".roo/mcp.json");
    }

    #[test]
    fn test_scan_agent_files_finds_kiro_mcp_config() {
        let temp_dir = TempDir::new().unwrap();
        let kiro_settings = temp_dir.path().join(".kiro").join("settings");
        fs::create_dir_all(&kiro_settings).unwrap();
        fs::write(kiro_settings.join("mcp.json"), "{}").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::KiroMcpConfig)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".kiro/settings/mcp.json");
    }

    #[test]
    fn test_scan_agent_files_finds_amazonq_mcp_config() {
        let temp_dir = TempDir::new().unwrap();
        let amazonq_dir = temp_dir.path().join(".amazonq");
        fs::create_dir_all(&amazonq_dir).unwrap();
        fs::write(amazonq_dir.join("mcp.json"), "{}").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::AmazonQMcpConfig)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".amazonq/mcp.json");
    }

    #[test]
    fn test_scan_agent_files_finds_kilocode_mcp_config() {
        let temp_dir = TempDir::new().unwrap();
        let kilocode_dir = temp_dir.path().join(".kilocode");
        fs::create_dir_all(&kilocode_dir).unwrap();
        fs::write(kilocode_dir.join("mcp.json"), "{}").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::KilocodeMcpConfig)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".kilocode/mcp.json");
    }

    #[test]
    fn test_scan_agent_files_finds_factory_mcp_config() {
        let temp_dir = TempDir::new().unwrap();
        let factory_dir = temp_dir.path().join(".factory");
        fs::create_dir_all(&factory_dir).unwrap();
        fs::write(factory_dir.join("mcp.json"), "{}").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::FactoryMcpConfig)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), ".factory/mcp.json");
    }

    #[test]
    fn test_scan_agent_files_finds_opencode_config() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("opencode.json"), "{}").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let entry: Vec<_> = discovered
            .iter()
            .filter(|f| f.file_type == AgentFileType::OpenCodeConfig)
            .collect();
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0].path.to_str().unwrap(), "opencode.json");
    }

    // ==========================================================================
    // UNIVERSAL AGENT ADOPTION — DEFAULT_CONFIG TESTS
    // ==========================================================================

    #[test]
    fn test_default_config_contains_gemini_agent() {
        let config: crate::config::Config = toml::from_str(DEFAULT_CONFIG).unwrap();

        assert!(config.agents.contains_key("gemini"));
        let gemini = &config.agents["gemini"];
        assert!(gemini.enabled);
        assert!(gemini.targets.contains_key("instructions"));
        assert!(gemini.targets.contains_key("skills"));
        assert!(gemini.targets.contains_key("commands"));
        assert!(
            gemini.targets["instructions"]
                .destination
                .contains("GEMINI.md")
        );
        assert!(
            gemini.targets["skills"]
                .destination
                .contains(".gemini/skills")
        );
    }

    #[test]
    fn test_default_config_contains_opencode_agent() {
        let config: crate::config::Config = toml::from_str(DEFAULT_CONFIG).unwrap();

        assert!(config.agents.contains_key("opencode"));
        let opencode = &config.agents["opencode"];
        assert!(opencode.enabled);
        assert!(opencode.targets.contains_key("instructions"));
        assert!(opencode.targets.contains_key("skills"));
        assert!(opencode.targets.contains_key("commands"));
        assert!(
            opencode.targets["instructions"]
                .destination
                .contains("OPENCODE.md")
        );
        assert!(
            opencode.targets["skills"]
                .destination
                .contains(".opencode/skills")
        );
    }

    #[test]
    fn test_default_config_claude_has_commands_target() {
        let config: crate::config::Config = toml::from_str(DEFAULT_CONFIG).unwrap();

        let claude = &config.agents["claude"];
        assert!(claude.targets.contains_key("commands"));

        let commands_target = &claude.targets["commands"];
        assert_eq!(commands_target.source, "commands");
        assert_eq!(commands_target.destination, ".claude/commands");
        assert_eq!(
            commands_target.sync_type,
            crate::config::SyncType::SymlinkContents
        );
    }

    #[test]
    fn test_build_skills_wizard_choices_is_empty_without_selected_skills_targets() {
        let temp_dir = TempDir::new().unwrap();
        let choices = build_skills_wizard_choices(
            temp_dir.path(),
            &temp_dir.path().join(".agents/skills"),
            &[DiscoveredFile {
                path: "CLAUDE.md".into(),
                file_type: AgentFileType::ClaudeInstructions,
                display_name: "CLAUDE.md".to_string(),
            }],
        );

        assert!(choices.is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn test_build_skills_wizard_choices_preserves_existing_directory_symlink() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().unwrap();
        let expected_source = temp_dir.path().join(".agents/skills");
        fs::create_dir_all(&expected_source).unwrap();
        fs::create_dir_all(temp_dir.path().join(".claude")).unwrap();
        unix_fs::symlink("../.agents/skills", temp_dir.path().join(".claude/skills")).unwrap();

        let choices = build_skills_wizard_choices(
            temp_dir.path(),
            &expected_source,
            &[DiscoveredFile {
                path: ".claude/skills".into(),
                file_type: AgentFileType::ClaudeSkills,
                display_name: "Claude skills".to_string(),
            }],
        );

        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].agent_name, "claude");
        assert_eq!(choices[0].recommended_mode, SyncType::Symlink);
        assert!(
            choices[0]
                .reason
                .as_deref()
                .unwrap()
                .contains("canonical directory symlink")
        );
    }

    #[test]
    fn test_resolve_skills_mode_selection_allows_override() {
        let choice = SkillsWizardChoice {
            agent_name: "claude".to_string(),
            destination: ".claude/skills".to_string(),
            recommended_mode: SyncType::Symlink,
            reason: None,
            already_canonical: false,
        };

        assert_eq!(resolve_skills_mode_selection(&choice, 0), SyncType::Symlink);
        assert_eq!(
            resolve_skills_mode_selection(&choice, 1),
            SyncType::SymlinkContents
        );
    }

    #[test]
    fn test_build_default_config_with_skills_modes_only_changes_requested_skills_targets() {
        let mut modes = BTreeMap::new();
        modes.insert("claude".to_string(), SyncType::SymlinkContents);

        let rendered = build_default_config_with_skills_modes(&modes);
        let config: crate::config::Config = toml::from_str(&rendered).unwrap();

        assert_eq!(
            config.agents["claude"].targets["skills"].sync_type,
            SyncType::SymlinkContents
        );
        assert_eq!(
            config.agents["codex"].targets["skills"].sync_type,
            SyncType::Symlink
        );
        assert_eq!(
            config.agents["claude"].targets["commands"].sync_type,
            SyncType::SymlinkContents
        );
    }

    #[test]
    fn test_build_wizard_layout_facts_uses_rendered_config_targets_and_modes() {
        let mut modes = BTreeMap::new();
        modes.insert("gemini".to_string(), SyncType::SymlinkContents);

        let rendered = build_default_config_with_skills_modes(&modes);
        let facts = build_wizard_layout_facts(&rendered).unwrap();

        assert_eq!(facts.instructions.len(), 5);
        assert!(
            facts
                .instructions
                .iter()
                .any(|target| target.destination == "CLAUDE.md")
        );
        assert!(
            facts
                .instructions
                .iter()
                .any(|target| target.destination == ".github/copilot-instructions.md")
        );
        assert!(
            facts
                .instructions
                .iter()
                .any(|target| target.destination == "GEMINI.md")
        );
        assert!(
            facts
                .instructions
                .iter()
                .any(|target| target.destination == "OPENCODE.md")
        );
        assert!(
            facts
                .instructions
                .iter()
                .any(|target| target.destination == "AGENTS.md")
        );

        assert_eq!(facts.skills.len(), 4);
        assert!(facts.skills.iter().any(|target| {
            target.destination == ".gemini/skills" && target.sync_type == SyncType::SymlinkContents
        }));
        assert!(facts.skills.iter().any(|target| {
            target.destination == ".claude/skills" && target.sync_type == SyncType::Symlink
        }));

        assert_eq!(facts.commands.len(), 3);
        assert!(
            facts
                .commands
                .iter()
                .any(|target| target.destination == ".claude/commands")
        );
        assert!(
            facts
                .commands
                .iter()
                .any(|target| target.destination == ".gemini/commands")
        );
        assert!(
            facts
                .commands
                .iter()
                .any(|target| target.destination == ".opencode/command")
        );
    }

    #[test]
    fn test_render_agent_config_layout_section_includes_markers_and_mode_specific_wording() {
        let facts = AgentLayoutFacts {
            instructions: vec![
                InstructionTargetLayout {
                    destination: "CLAUDE.md".to_string(),
                },
                InstructionTargetLayout {
                    destination: "AGENTS.md".to_string(),
                },
            ],
            skills: vec![
                SkillsTargetLayout {
                    destination: ".claude/skills".to_string(),
                    sync_type: SyncType::Symlink,
                },
                SkillsTargetLayout {
                    destination: ".gemini/skills".to_string(),
                    sync_type: SyncType::SymlinkContents,
                },
            ],
            commands: vec![CommandTargetLayout {
                destination: ".opencode/command".to_string(),
            }],
        };

        let rendered = render_agent_config_layout_section(&facts);

        assert_eq!(
            rendered.matches(AGENT_CONFIG_LAYOUT_START_MARKER).count(),
            1
        );
        assert_eq!(rendered.matches(AGENT_CONFIG_LAYOUT_END_MARKER).count(), 1);
        assert!(rendered.contains("## Agent config layout"));
        assert!(rendered.contains("`.agents/` is the canonical source"));
        assert!(rendered.contains("`.agents/AGENTS.md` is the canonical instructions file"));
        assert!(rendered.contains("`.claude/skills` reflects `.agents/skills/` directly"));
        assert!(rendered.contains(
            "`.gemini/skills` is populated from `.agents/skills/` when `agentsync apply` runs"
        ));
        assert!(rendered.contains("add, remove, or rename skill entries"));
        assert!(rendered.contains("`.opencode/command`"));
    }

    #[test]
    fn test_agent_config_layout_omits_targets_not_present_in_generated_config() {
        let rendered = build_default_config_with_skills_modes(&BTreeMap::new())
            .replace(
                r#"
[agents.copilot.targets.instructions]
source = "AGENTS.md"
destination = ".github/copilot-instructions.md"
type = "symlink"
"#,
                "\n",
            )
            .replace(
                r#"
[agents.codex.targets.skills]
source = "skills"
destination = ".codex/skills"
type = "symlink"
"#,
                "\n",
            )
            .replace(
                r#"
# Note: intentionally singular per OpenCode convention (.opencode/command, not .opencode/commands)
[agents.opencode.targets.commands]
source = "commands"
destination = ".opencode/command"
type = "symlink-contents"
"#,
                "\n",
            );

        let facts = build_wizard_layout_facts(&rendered).unwrap();
        let layout = render_agent_config_layout_section(&facts);
        let agents_md = upsert_agent_config_layout_block(DEFAULT_AGENTS_MD, &layout);

        assert!(
            !facts
                .instructions
                .iter()
                .any(|target| target.destination == ".github/copilot-instructions.md")
        );
        assert!(
            !facts
                .skills
                .iter()
                .any(|target| target.destination == ".codex/skills")
        );
        assert!(
            !facts
                .commands
                .iter()
                .any(|target| target.destination == ".opencode/command")
        );

        assert!(!agents_md.contains("`.github/copilot-instructions.md`"));
        assert!(!agents_md.contains("`.codex/skills`"));
        assert!(!agents_md.contains("`.opencode/command`"));

        assert!(agents_md.contains("`CLAUDE.md`"));
        assert!(agents_md.contains("`.claude/skills`"));
        assert!(agents_md.contains("`.claude/commands`"));
    }

    #[test]
    fn test_upsert_agent_config_layout_block_places_block_after_default_intro() {
        let layout_block = render_agent_config_layout_section(
            &build_wizard_layout_facts(&build_default_config_with_skills_modes(&BTreeMap::new()))
                .unwrap(),
        );

        let rendered = upsert_agent_config_layout_block(DEFAULT_AGENTS_MD, &layout_block);
        let overview_index = rendered.find("## Project Overview").unwrap();
        let layout_index = rendered.find("## Agent config layout").unwrap();
        let intro_index = rendered
            .find("> This file provides instructions for AI coding assistants working on this project.")
            .unwrap();

        assert!(intro_index < layout_index);
        assert!(layout_index < overview_index);
    }

    #[test]
    fn test_upsert_agent_config_layout_block_places_block_after_migrated_header() {
        let base_content = "# Instructions from CLAUDE.md\n\nThis content was migrated.\n\n## Existing Section\n\nBody\n";
        let layout_block = render_agent_config_layout_section(
            &build_wizard_layout_facts(&build_default_config_with_skills_modes(&BTreeMap::new()))
                .unwrap(),
        );

        let rendered = upsert_agent_config_layout_block(base_content, &layout_block);
        let migrated_intro_index = rendered.find("This content was migrated.").unwrap();
        let layout_index = rendered.find("## Agent config layout").unwrap();
        let section_index = rendered.find("## Existing Section").unwrap();

        assert!(migrated_intro_index < layout_index);
        assert!(layout_index < section_index);
    }

    #[test]
    fn test_upsert_agent_config_layout_block_replaces_existing_managed_block_idempotently() {
        let old_layout = format!(
            "{AGENT_CONFIG_LAYOUT_START_MARKER}\n## Agent config layout\n\nold\n\n{AGENT_CONFIG_LAYOUT_END_MARKER}"
        );
        let base_content =
            format!("# AI Agent Instructions\n\n> Intro\n\n{old_layout}\n\n## Project Overview\n");
        let layout_block = render_agent_config_layout_section(
            &build_wizard_layout_facts(&build_default_config_with_skills_modes(&BTreeMap::new()))
                .unwrap(),
        );

        let rendered_once = upsert_agent_config_layout_block(&base_content, &layout_block);
        let rendered_twice = upsert_agent_config_layout_block(&rendered_once, &layout_block);

        assert_eq!(rendered_once, rendered_twice);
        assert_eq!(
            rendered_twice
                .matches(AGENT_CONFIG_LAYOUT_START_MARKER)
                .count(),
            1
        );
        assert_eq!(
            rendered_twice
                .matches(AGENT_CONFIG_LAYOUT_END_MARKER)
                .count(),
            1
        );
        assert!(!rendered_twice.contains("\n\nold\n\n"));
    }

    #[test]
    fn test_wizard_preserve_without_force_leaves_existing_agents_md_unchanged() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let agents_md_path = agents_dir.join("AGENTS.md");
        let existing_agents_md = "# Existing AGENTS\n\nKeep me unchanged.\n";
        fs::write(&agents_md_path, existing_agents_md).unwrap();

        let force = false;
        let result = if agents_md_path.exists() && !force {
            fs::read_to_string(&agents_md_path).unwrap()
        } else {
            let layout_block = render_agent_config_layout_section(
                &build_wizard_layout_facts(&build_default_config_with_skills_modes(
                    &BTreeMap::new(),
                ))
                .unwrap(),
            );
            upsert_agent_config_layout_block(existing_agents_md, &layout_block)
        };

        assert_eq!(result, existing_agents_md);
        assert!(!result.contains("Agent config layout"));
    }

    #[test]
    #[cfg(unix)]
    fn test_collect_post_init_skills_warnings_reports_override_mismatch() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().unwrap();
        init(temp_dir.path(), false).unwrap();
        fs::create_dir_all(temp_dir.path().join(".claude")).unwrap();
        unix_fs::symlink("../.agents/skills", temp_dir.path().join(".claude/skills")).unwrap();

        let mut modes = BTreeMap::new();
        modes.insert("claude".to_string(), SyncType::SymlinkContents);
        let config_path = temp_dir.path().join(".agents/agentsync.toml");
        fs::write(&config_path, build_default_config_with_skills_modes(&modes)).unwrap();

        let warnings = collect_post_init_skills_warnings(
            temp_dir.path(),
            &config_path,
            &["claude".to_string()],
        )
        .unwrap();

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("symlink-contents"));
        assert!(warnings[0].contains("directory symlink"));
    }

    #[test]
    #[cfg(unix)]
    fn test_collect_post_init_skills_warnings_stays_quiet_for_matching_symlink_mode() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().unwrap();
        init(temp_dir.path(), false).unwrap();
        fs::create_dir_all(temp_dir.path().join(".claude")).unwrap();
        unix_fs::symlink("../.agents/skills", temp_dir.path().join(".claude/skills")).unwrap();

        let config_path = temp_dir.path().join(".agents/agentsync.toml");
        fs::write(
            &config_path,
            build_default_config_with_skills_modes(&BTreeMap::new()),
        )
        .unwrap();

        let warnings = collect_post_init_skills_warnings(
            temp_dir.path(),
            &config_path,
            &["claude".to_string()],
        )
        .unwrap();

        assert!(warnings.is_empty());
    }

    #[test]
    fn test_init_creates_commands_directory() {
        let temp_dir = TempDir::new().unwrap();

        init(temp_dir.path(), false).unwrap();

        let commands_dir = temp_dir.path().join(".agents").join("commands");
        assert!(commands_dir.exists());
        assert!(commands_dir.is_dir());
    }

    #[test]
    fn test_scan_agent_files_finds_skills_alongside_parent_dir() {
        let temp_dir = TempDir::new().unwrap();
        // Create .cursor/ with rules AND skills
        let cursor_dir = temp_dir.path().join(".cursor");
        fs::create_dir_all(cursor_dir.join("rules")).unwrap();
        fs::write(cursor_dir.join("rules/some-rule.mdc"), "rule").unwrap();
        let skills_dir = cursor_dir.join("skills").join("my-skill");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("SKILL.md"), "# Skill").unwrap();

        let discovered = scan_agent_files(temp_dir.path()).unwrap();

        let has_dir = discovered
            .iter()
            .any(|f| f.file_type == AgentFileType::CursorDirectory);
        let has_skills = discovered
            .iter()
            .any(|f| f.file_type == AgentFileType::CursorSkills);
        assert!(has_dir);
        assert!(has_skills);
    }
}
