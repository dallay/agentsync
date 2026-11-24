//! Safe import of native agent/MCP configuration.
//!
//! Parses configuration files from major agents and generates reviewable diffs
//! between imported and existing AgentSync configuration.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::config::{Config, McpServerConfig};

/// A parsed MCP server configuration from a native agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeMcpServer {
    /// Server name.
    pub name: String,
    /// Command to run the server.
    pub command: String,
    /// Arguments for the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// A parsed instruction file from a native agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeInstruction {
    /// Agent ID.
    pub agent: String,
    /// File path relative to project root.
    pub path: String,
    /// Content of the instruction file.
    pub content: String,
}

/// Result of parsing a native configuration.
#[derive(Debug, Clone, Serialize)]
pub struct NativeConfig {
    /// Agent ID.
    pub agent: String,
    /// MCP servers found.
    pub mcp_servers: Vec<NativeMcpServer>,
    /// Instruction files found.
    pub instructions: Vec<NativeInstruction>,
    /// Source files parsed.
    pub source_files: Vec<String>,
}

/// A diff between imported and existing configuration.
#[derive(Debug, Clone, Serialize)]
pub struct ImportDiff {
    /// Agent ID.
    pub agent: String,
    /// MCP servers that would be added.
    pub mcp_added: Vec<NativeMcpServer>,
    /// MCP servers that would be modified.
    pub mcp_modified: Vec<(String, NativeMcpServer, NativeMcpServer)>,
    /// Instructions that would be added.
    pub instructions_added: Vec<NativeInstruction>,
    /// Instructions that would be modified.
    pub instructions_modified: Vec<(String, NativeInstruction, NativeInstruction)>,
}

impl ImportDiff {
    /// Check if there are no changes.
    pub fn is_empty(&self) -> bool {
        self.mcp_added.is_empty()
            && self.mcp_modified.is_empty()
            && self.instructions_added.is_empty()
            && self.instructions_modified.is_empty()
    }

    /// Get the total number of changes.
    pub fn change_count(&self) -> usize {
        self.mcp_added.len()
            + self.mcp_modified.len()
            + self.instructions_added.len()
            + self.instructions_modified.len()
    }
}

/// Result of an import operation.
#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    /// Agent ID.
    pub agent: String,
    /// Configuration diff.
    pub diff: ImportDiff,
    /// Warnings during import.
    pub warnings: Vec<String>,
    /// Whether the import was successful.
    pub success: bool,
}

/// Parse MCP servers from Claude's .mcp.json.
pub fn parse_claud_mcp(project_root: &Path) -> Result<Vec<NativeMcpServer>> {
    let mcp_path = project_root.join(".mcp.json");
    if !mcp_path.exists() {
        return Ok(Vec::new());
    }

    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- mcp_path is constructed from project_root with fixed filename
    let content = fs::read_to_string(&mcp_path)
        .with_context(|| format!("Failed to read {}", mcp_path.display()))?;

    let mut servers = Vec::new();

    // Parse the JSON structure
    #[derive(Deserialize)]
    struct ClaudeMcpJson {
        #[serde(default, rename = "mcpServers")]
        mcp_servers: BTreeMap<String, ClaudeMcpEntry>,
    }

    #[derive(Deserialize)]
    struct ClaudeMcpEntry {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: Option<BTreeMap<String, String>>,
    }

    let mcp_json: ClaudeMcpJson = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", mcp_path.display()))?;

    for (name, entry) in mcp_json.mcp_servers {
        servers.push(NativeMcpServer {
            name,
            command: entry.command,
            args: entry.args,
            env: entry.env.unwrap_or_default(),
        });
    }

    Ok(servers)
}

/// Parse MCP servers from VS Code's .vscode/mcp.json.
pub fn parse_vscode_mcp(project_root: &Path) -> Result<Vec<NativeMcpServer>> {
    let mcp_path = project_root.join(".vscode/mcp.json");
    if !mcp_path.exists() {
        return Ok(Vec::new());
    }

    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- mcp_path is constructed from project_root with fixed filename
    let content = fs::read_to_string(&mcp_path)
        .with_context(|| format!("Failed to read {}", mcp_path.display()))?;

    let mut servers = Vec::new();

    #[derive(Deserialize)]
    struct VsCodeMcpJson {
        #[serde(default, rename = "mcpServers")]
        mcp_servers: BTreeMap<String, VsCodeMcpEntry>,
    }

    #[derive(Deserialize)]
    struct VsCodeMcpEntry {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: Option<BTreeMap<String, String>>,
    }

    let mcp_json: VsCodeMcpJson = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", mcp_path.display()))?;

    for (name, entry) in mcp_json.mcp_servers {
        servers.push(NativeMcpServer {
            name,
            command: entry.command,
            args: entry.args,
            env: entry.env.unwrap_or_default(),
        });
    }

    Ok(servers)
}

/// Parse MCP servers from Cursor's .cursor/mcp.json.
pub fn parse_cursor_mcp(project_root: &Path) -> Result<Vec<NativeMcpServer>> {
    let mcp_path = project_root.join(".cursor/mcp.json");
    if !mcp_path.exists() {
        return Ok(Vec::new());
    }

    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- mcp_path is constructed from project_root with fixed filename
    let content = fs::read_to_string(&mcp_path)
        .with_context(|| format!("Failed to read {}", mcp_path.display()))?;

    let mut servers = Vec::new();

    #[derive(Deserialize)]
    struct CursorMcpJson {
        #[serde(default, rename = "mcpServers")]
        mcp_servers: BTreeMap<String, CursorMcpEntry>,
    }

    #[derive(Deserialize)]
    struct CursorMcpEntry {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: Option<BTreeMap<String, String>>,
    }

    let mcp_json: CursorMcpJson = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", mcp_path.display()))?;

    for (name, entry) in mcp_json.mcp_servers {
        servers.push(NativeMcpServer {
            name,
            command: entry.command,
            args: entry.args,
            env: entry.env.unwrap_or_default(),
        });
    }

    Ok(servers)
}

/// Parse instruction files from an agent.
pub fn parse_instructions(project_root: &Path, agent: &str) -> Result<Vec<NativeInstruction>> {
    let mut instructions = Vec::new();

    // Determine the instruction file path based on agent
    let instruction_paths = match agent {
        "claude" => vec!["CLAUDE.md"],
        "copilot" => vec![".github/copilot-instructions.md"],
        "codex" | "opencode" | "zcode" | "minimax" => vec!["AGENTS.md"],
        "gemini" => vec!["GEMINI.md"],
        "cursor" => vec![".cursor/rules/agentsync.mdc"],
        "windsurf" => vec![".windsurfrules"],
        "crush" => vec!["CRUSH.md"],
        "warp" => vec!["WARP.md"],
        "amp" => vec!["AMPCODE.md"],
        _ => vec![],
    };

    for path_str in instruction_paths {
        let path = project_root.join(path_str);
        if path.exists() {
            // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- path is constructed from project_root with fixed filenames
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            instructions.push(NativeInstruction {
                agent: agent.to_string(),
                path: path_str.to_string(),
                content,
            });
        }
    }

    Ok(instructions)
}

/// Parse native configuration for an agent.
pub fn parse_native_config(project_root: &Path, agent: &str) -> Result<NativeConfig> {
    let mut mcp_servers = Vec::new();
    let mut source_files = Vec::new();

    // Parse MCP servers based on agent
    match agent {
        "claude" => {
            let servers = parse_claud_mcp(project_root)?;
            if !servers.is_empty() {
                source_files.push(".mcp.json".to_string());
            }
            mcp_servers.extend(servers);
        }
        "copilot" | "vscode" => {
            let servers = parse_vscode_mcp(project_root)?;
            if !servers.is_empty() {
                source_files.push(".vscode/mcp.json".to_string());
            }
            mcp_servers.extend(servers);
        }
        "cursor" => {
            let servers = parse_cursor_mcp(project_root)?;
            if !servers.is_empty() {
                source_files.push(".cursor/mcp.json".to_string());
            }
            mcp_servers.extend(servers);
        }
        _ => {}
    }

    // Parse instructions
    let instructions = parse_instructions(project_root, agent)?;
    for inst in &instructions {
        source_files.push(inst.path.clone());
    }

    Ok(NativeConfig {
        agent: agent.to_string(),
        mcp_servers,
        instructions,
        source_files,
    })
}

/// Generate a diff between imported and existing configuration.
pub fn generate_diff(
    project_root: &Path,
    agent: &str,
    existing_config: &Config,
) -> Result<ImportDiff> {
    let native = parse_native_config(project_root, agent)?;

    let mut diff = ImportDiff {
        agent: agent.to_string(),
        mcp_added: Vec::new(),
        mcp_modified: Vec::new(),
        instructions_added: Vec::new(),
        instructions_modified: Vec::new(),
    };

    // Compare MCP servers
    let existing_mcp: BTreeMap<String, &McpServerConfig> = existing_config
        .mcp_servers
        .iter()
        .map(|(k, v)| (k.clone(), v))
        .collect();

    for native_server in &native.mcp_servers {
        match existing_mcp.get(&native_server.name) {
            None => {
                diff.mcp_added.push(native_server.clone());
            }
            Some(existing) => {
                // Check if the server config has changed
                if server_changed(existing, native_server) {
                    diff.mcp_modified.push((
                        native_server.name.clone(),
                        native_server.clone(),
                        existing_to_native(existing),
                    ));
                }
            }
        }
    }

    // Compare instructions
    let existing_agent = existing_config.agents.get(agent);
    for native_inst in &native.instructions {
        if let Some(agent_config) = existing_agent {
            // Check if the instruction is already managed
            let is_managed = agent_config
                .targets
                .values()
                .any(|t| t.destination == native_inst.path);

            if !is_managed {
                diff.instructions_added.push(native_inst.clone());
            }
        } else {
            diff.instructions_added.push(native_inst.clone());
        }
    }

    Ok(diff)
}

/// Check if an MCP server configuration has changed.
fn server_changed(existing: &McpServerConfig, native: &NativeMcpServer) -> bool {
    existing.command.as_deref() != Some(native.command.as_str()) || existing.args != native.args
}

/// Convert an existing MCP server config to a native format for comparison.
fn existing_to_native(existing: &McpServerConfig) -> NativeMcpServer {
    NativeMcpServer {
        name: String::new(),
        command: existing.command.clone().unwrap_or_default(),
        args: existing.args.clone(),
        env: BTreeMap::new(),
    }
}

/// Import native configuration into AgentSync config.
pub fn import_native_config(project_root: &Path, agent: &str) -> Result<ImportResult> {
    let mut warnings = Vec::new();

    // Parse native config
    let _parsed_config = match parse_native_config(project_root, agent) {
        Ok(config) => config,
        Err(e) => {
            return Ok(ImportResult {
                agent: agent.to_string(),
                diff: ImportDiff {
                    agent: agent.to_string(),
                    mcp_added: Vec::new(),
                    mcp_modified: Vec::new(),
                    instructions_added: Vec::new(),
                    instructions_modified: Vec::new(),
                },
                warnings: vec![format!("Failed to parse native config: {}", e)],
                success: false,
            });
        }
    };

    // Load existing config
    let existing_config = match Config::find_config(project_root) {
        Ok(path) => Config::load(&path).unwrap_or_default(),
        Err(_) => Config::default(),
    };

    // Generate diff
    let diff = generate_diff(project_root, agent, &existing_config)?;

    let success = !diff.is_empty();

    if diff.is_empty() {
        warnings.push("No changes detected — configuration is already in sync".to_string());
    }

    Ok(ImportResult {
        agent: agent.to_string(),
        diff,
        warnings,
        success,
    })
}

/// List agents with native configuration files.
pub fn list_agents_with_config(project_root: &Path) -> Vec<String> {
    let mut agents = Vec::new();

    // Check for Claude
    if project_root.join(".mcp.json").exists() {
        agents.push("claude".to_string());
    }

    // Check for VS Code / Copilot
    if project_root.join(".vscode/mcp.json").exists() {
        agents.push("vscode".to_string());
    }

    // Check for Cursor
    if project_root.join(".cursor/mcp.json").exists() {
        agents.push("cursor".to_string());
    }

    // Check for instruction files
    let instruction_checks = vec![
        ("CLAUDE.md", "claude"),
        (".github/copilot-instructions.md", "copilot"),
        ("AGENTS.md", "codex"),
        ("GEMINI.md", "gemini"),
        (".cursor/rules/agentsync.mdc", "cursor"),
    ];

    for (file, agent) in instruction_checks {
        if project_root.join(file).exists() && !agents.contains(&agent.to_string()) {
            agents.push(agent.to_string());
        }
    }

    agents.sort();
    agents
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_project(temp_dir: &TempDir, files: &[(&str, &str)]) {
        for (path, content) in files {
            let full_path = temp_dir.path().join(path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&full_path, content).unwrap();
        }
    }

    #[test]
    fn test_parse_claud_mcp_empty() {
        let temp_dir = TempDir::new().unwrap();
        let servers = parse_claud_mcp(temp_dir.path()).unwrap();
        assert!(servers.is_empty());
    }

    #[test]
    fn test_parse_claud_mcp() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(
            &temp_dir,
            &[(
                ".mcp.json",
                r#"{
  "mcpServers": {
    "test-server": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-test"]
    }
  }
}"#,
            )],
        );
        let servers = parse_claud_mcp(temp_dir.path()).unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "test-server");
        assert_eq!(servers[0].command, "npx");
    }

    #[test]
    fn test_parse_vscode_mcp() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(
            &temp_dir,
            &[(
                ".vscode/mcp.json",
                r#"{
  "mcpServers": {
    "vscode-server": {
      "command": "node",
      "args": ["server.js"]
    }
  }
}"#,
            )],
        );
        let servers = parse_vscode_mcp(temp_dir.path()).unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "vscode-server");
    }

    #[test]
    fn test_parse_cursor_mcp() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(
            &temp_dir,
            &[(
                ".cursor/mcp.json",
                r#"{
  "mcpServers": {
    "cursor-server": {
      "command": "python",
      "args": ["-m", "http.server"]
    }
  }
}"#,
            )],
        );
        let servers = parse_cursor_mcp(temp_dir.path()).unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "cursor-server");
    }

    #[test]
    fn test_parse_instructions() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(
            &temp_dir,
            &[("CLAUDE.md", "# Instructions\n\nFollow these rules.")],
        );
        let instructions = parse_instructions(temp_dir.path(), "claude").unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].agent, "claude");
        assert_eq!(instructions[0].path, "CLAUDE.md");
    }

    #[test]
    fn test_parse_native_config() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(
            &temp_dir,
            &[
                (
                    ".mcp.json",
                    r#"{"mcpServers": {"test": {"command": "test"}}}"#,
                ),
                ("CLAUDE.md", "# Test"),
            ],
        );
        let config = parse_native_config(temp_dir.path(), "claude").unwrap();
        assert_eq!(config.agent, "claude");
        assert_eq!(config.mcp_servers.len(), 1);
        assert_eq!(config.instructions.len(), 1);
    }

    #[test]
    fn test_generate_diff() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(
            &temp_dir,
            &[
                (
                    ".mcp.json",
                    r#"{"mcpServers": {"new-server": {"command": "new"}}}"#,
                ),
                ("CLAUDE.md", "# New instructions"),
            ],
        );
        let config = Config::default();
        let diff = generate_diff(temp_dir.path(), "claude", &config).unwrap();
        assert!(!diff.is_empty());
        assert!(!diff.mcp_added.is_empty() || !diff.instructions_added.is_empty());
    }

    #[test]
    fn test_list_agents_with_config() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(
            &temp_dir,
            &[
                (
                    ".mcp.json",
                    r#"{"mcpServers": {"test": {"command": "test"}}}"#,
                ),
                ("CLAUDE.md", "# Test"),
            ],
        );
        let agents = list_agents_with_config(temp_dir.path());
        assert!(agents.contains(&"claude".to_string()));
    }

    #[test]
    fn test_import_result_serialization() {
        let result = ImportResult {
            agent: "test".to_string(),
            diff: ImportDiff {
                agent: "test".to_string(),
                mcp_added: Vec::new(),
                mcp_modified: Vec::new(),
                instructions_added: Vec::new(),
                instructions_modified: Vec::new(),
            },
            warnings: vec!["Test warning".to_string()],
            success: true,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("test"));
    }

    #[test]
    fn test_diff_change_count() {
        let mut diff = ImportDiff {
            agent: "test".to_string(),
            mcp_added: Vec::new(),
            mcp_modified: Vec::new(),
            instructions_added: Vec::new(),
            instructions_modified: Vec::new(),
        };
        assert_eq!(diff.change_count(), 0);

        diff.mcp_added.push(NativeMcpServer {
            name: "test".to_string(),
            command: "test".to_string(),
            args: Vec::new(),
            env: BTreeMap::new(),
        });
        assert_eq!(diff.change_count(), 1);
    }
}
