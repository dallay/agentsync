//! MCP (Model Context Protocol) configuration generation
//!
//! This module handles generating MCP configuration files for different
//! AI agents. Each agent may have different file formats and locations.

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{McpMergeStrategy, McpServerConfig};

// =============================================================================
// MCP Output Format
// =============================================================================

/// The standard JSON structure for MCP servers
#[derive(Debug, Clone)]
pub struct McpOutput {
    /// The mcpServers object
    pub servers: HashMap<String, McpServerConfig>,
}

impl McpOutput {
    pub fn new(servers: HashMap<String, McpServerConfig>) -> Self {
        Self { servers }
    }

    /// Filter out disabled servers
    pub fn enabled_servers(&self) -> HashMap<String, &McpServerConfig> {
        self.servers
            .iter()
            .filter(|(_, config)| !config.disabled)
            .map(|(name, config)| (name.clone(), config))
            .collect()
    }
}

// =============================================================================
// Agent Definition
// =============================================================================

/// Known MCP-compatible agent identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum McpAgent {
    /// Claude Code (.mcp.json)
    ClaudeCode,
    /// GitHub Copilot (.copilot/mcp-config.json)
    GithubCopilot,
    /// Gemini CLI (.gemini/settings.json)
    GeminiCli,
    /// VS Code (.vscode/mcp.json)
    VsCode,
    /// OpenCode (opencode.json)
    OpenCode,
}

impl McpAgent {
    /// Get all supported agents
    pub fn all() -> &'static [McpAgent] {
        &[
            McpAgent::ClaudeCode,
            McpAgent::GithubCopilot,
            McpAgent::GeminiCli,
            McpAgent::VsCode,
            McpAgent::OpenCode,
        ]
    }

    /// Get the agent identifier string (used in config)
    pub fn id(&self) -> &'static str {
        match self {
            McpAgent::ClaudeCode => "claude",
            McpAgent::GithubCopilot => "copilot",
            McpAgent::GeminiCli => "gemini",
            McpAgent::VsCode => "vscode",
            McpAgent::OpenCode => "opencode",
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            McpAgent::ClaudeCode => "Claude Code",
            McpAgent::GithubCopilot => "GitHub Copilot",
            McpAgent::GeminiCli => "Gemini CLI",
            McpAgent::VsCode => "VS Code",
            McpAgent::OpenCode => "OpenCode",
        }
    }

    /// Get the project-level config file path (relative to project root)
    pub fn config_path(&self) -> &'static str {
        match self {
            McpAgent::ClaudeCode => ".mcp.json",
            McpAgent::GithubCopilot => ".copilot/mcp-config.json",
            McpAgent::GeminiCli => ".gemini/settings.json",
            McpAgent::VsCode => ".vscode/mcp.json",
            McpAgent::OpenCode => "opencode.json",
        }
    }

    /// Get the formatter for this agent
    pub fn formatter(&self) -> Box<dyn McpFormatter> {
        match self {
            McpAgent::ClaudeCode => Box::new(ClaudeCodeFormatter),
            McpAgent::GithubCopilot => Box::new(GithubCopilotFormatter),
            McpAgent::GeminiCli => Box::new(GeminiCliFormatter),
            McpAgent::VsCode => Box::new(VsCodeFormatter),
            McpAgent::OpenCode => Box::new(OpenCodeFormatter),
        }
    }

    /// Parse agent from string identifier
    pub fn from_id(id: &str) -> Option<McpAgent> {
        match id.to_lowercase().as_str() {
            "claude" | "claude-code" | "claude_code" => Some(McpAgent::ClaudeCode),
            "copilot" | "github-copilot" | "github_copilot" => Some(McpAgent::GithubCopilot),
            "gemini" | "gemini-cli" | "gemini_cli" => Some(McpAgent::GeminiCli),
            "vscode" | "vs-code" | "vs_code" => Some(McpAgent::VsCode),
            "opencode" | "open-code" | "open_code" => Some(McpAgent::OpenCode),
            _ => None,
        }
    }
}

// =============================================================================
// MCP Formatter Trait
// =============================================================================

/// Trait for formatting MCP configuration for different agents
pub trait McpFormatter: Send + Sync {
    /// Format MCP servers into agent-specific JSON structure
    fn format(&self, servers: &HashMap<String, &McpServerConfig>) -> Value;

    /// Parse existing configuration file to extract mcpServers
    fn parse_existing(&self, content: &str) -> Result<HashMap<String, Value>>;

    /// Merge new servers with existing configuration
    fn merge(
        &self,
        existing_content: &str,
        new_servers: &HashMap<String, &McpServerConfig>,
    ) -> Result<String>;

    /// Remove servers that are no longer in the configuration
    /// Default implementation just calls merge, but specific formatters can override
    fn cleanup_removed_servers(
        &self,
        existing_content: &str,
        new_servers: &HashMap<String, McpServerConfig>,
    ) -> Result<String> {
        // Default implementation - adapt owned map to refs and call merge
        let refs: HashMap<String, &McpServerConfig> =
            new_servers.iter().map(|(k, v)| (k.clone(), v)).collect();
        self.merge(existing_content, &refs)
    }

    /// Whether this formatter wraps mcpServers in another key
    fn wrapper_key(&self) -> Option<&'static str> {
        None
    }

    /// Whether the formatter should preserve unrelated top-level settings when
    /// running in Overwrite mode. Some agents (Gemini, OpenCode) keep other
    /// settings in their config files and we shouldn't clobber them.
    fn preserve_on_overwrite(&self) -> bool {
        false
    }
}

// =============================================================================
// Standard MCP Helper Functions
// =============================================================================

/// Format servers into standard { "mcpServers": { ... } } structure
fn format_standard_mcp(servers: &HashMap<String, &McpServerConfig>) -> Value {
    let mut mcp_servers = Map::new();

    for (name, config) in servers {
        mcp_servers.insert(name.clone(), server_to_json(config));
    }

    json!({
        "mcpServers": mcp_servers
    })
}

/// Parse standard MCP config structure
fn parse_standard_mcp(content: &str, context_msg: &str) -> Result<HashMap<String, Value>> {
    let parsed: Value = serde_json::from_str(content).context(context_msg.to_string())?;

    let servers = parsed
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    Ok(servers)
}

/// Merge new servers into standard MCP config structure
fn merge_standard_mcp(
    existing_content: &str,
    new_servers: &HashMap<String, &McpServerConfig>,
    context_msg: &str,
) -> Result<String> {
    let mut existing = parse_standard_mcp(existing_content, context_msg)?;

    // New servers override existing ones with same name
    for (name, config) in new_servers {
        existing.insert(name.clone(), server_to_json(config));
    }

    let output = json!({
        "mcpServers": existing
    });

    serde_json::to_string_pretty(&output).context("Failed to serialize merged config")
}

/// Merge new servers into standard MCP config structure with pre-filtered existing servers
fn merge_standard_mcp_filtered(
    existing_servers: &HashMap<String, Value>,
    new_servers: &HashMap<String, &McpServerConfig>,
    _context_msg: &str,
) -> Result<String> {
    let mut existing = existing_servers.clone();

    // New servers override existing ones with same name
    for (name, config) in new_servers {
        existing.insert(name.clone(), server_to_json(config));
    }

    let output = json!({
        "mcpServers": existing
    });

    serde_json::to_string_pretty(&output).context("Failed to serialize merged config")
}

// =============================================================================
// Claude Code Formatter
// =============================================================================

/// Formatter for Claude Code (.mcp.json)
/// Format: { "mcpServers": { ... } }
#[derive(Debug)]
pub struct ClaudeCodeFormatter;

impl McpFormatter for ClaudeCodeFormatter {
    fn format(&self, servers: &HashMap<String, &McpServerConfig>) -> Value {
        format_standard_mcp(servers)
    }

    fn parse_existing(&self, content: &str) -> Result<HashMap<String, Value>> {
        parse_standard_mcp(content, "Failed to parse existing MCP config as JSON")
    }

    fn merge(
        &self,
        existing_content: &str,
        new_servers: &HashMap<String, &McpServerConfig>,
    ) -> Result<String> {
        merge_standard_mcp(
            existing_content,
            new_servers,
            "Failed to parse existing MCP config as JSON",
        )
    }

    fn cleanup_removed_servers(
        &self,
        existing_content: &str,
        new_servers: &HashMap<String, McpServerConfig>,
    ) -> Result<String> {
        // For standard MCP format, we can use the same merge function
        // but with a clean slate - only keep servers that are in new_servers
        let existing = parse_standard_mcp(
            existing_content,
            "Failed to parse existing MCP config as JSON",
        )?;

        // Filter existing servers to only keep those that are in new_servers
        let filtered_existing: HashMap<String, Value> = existing
            .into_iter()
            .filter(|(name, _)| new_servers.contains_key(name))
            .collect();

        // Convert owned new_servers to a refs map for merge helper
        let refs: HashMap<String, &McpServerConfig> =
            new_servers.iter().map(|(k, v)| (k.clone(), v)).collect();

        // Now merge with new servers (this will override any matching ones)
        merge_standard_mcp_filtered(
            &filtered_existing,
            &refs,
            "Failed to parse existing MCP config as JSON",
        )
    }
}

// =============================================================================
// GitHub Copilot Formatter
// =============================================================================

/// Formatter for GitHub Copilot (.copilot/mcp-config.json)
/// Format: { "mcpServers": { ... } }
#[derive(Debug)]
pub struct GithubCopilotFormatter;

impl McpFormatter for GithubCopilotFormatter {
    fn format(&self, servers: &HashMap<String, &McpServerConfig>) -> Value {
        format_standard_mcp(servers)
    }

    fn parse_existing(&self, content: &str) -> Result<HashMap<String, Value>> {
        parse_standard_mcp(
            content,
            "Failed to parse existing Copilot MCP config as JSON",
        )
    }

    fn merge(
        &self,
        existing_content: &str,
        new_servers: &HashMap<String, &McpServerConfig>,
    ) -> Result<String> {
        merge_standard_mcp(
            existing_content,
            new_servers,
            "Failed to parse existing Copilot MCP config as JSON",
        )
    }

    fn cleanup_removed_servers(
        &self,
        existing_content: &str,
        new_servers: &HashMap<String, McpServerConfig>,
    ) -> Result<String> {
        // Same logic as Claude Code - use standard MCP format
        let existing = parse_standard_mcp(
            existing_content,
            "Failed to parse existing Copilot MCP config as JSON",
        )?;

        let filtered_existing: HashMap<String, Value> = existing
            .into_iter()
            .filter(|(name, _)| new_servers.contains_key(name))
            .collect();

        let refs: HashMap<String, &McpServerConfig> =
            new_servers.iter().map(|(k, v)| (k.clone(), v)).collect();

        merge_standard_mcp_filtered(
            &filtered_existing,
            &refs,
            "Failed to parse existing Copilot MCP config as JSON",
        )
    }
}

// =============================================================================
// Gemini CLI Formatter
// =============================================================================

/// Formatter for Gemini CLI (.gemini/settings.json)
/// Format: { "mcpServers": { ... } } with additional `trust: true` per server
#[derive(Debug)]
pub struct GeminiCliFormatter; // keep type name

impl McpFormatter for GeminiCliFormatter {
    fn format(&self, servers: &HashMap<String, &McpServerConfig>) -> Value {
        let mut mcp_servers = Map::new();

        for (name, config) in servers {
            let mut server_json = server_to_json(config);
            // Gemini requires trust: true for non-interactive execution
            if let Some(obj) = server_json.as_object_mut() {
                obj.insert("trust".to_string(), json!(true));
            }
            mcp_servers.insert(name.clone(), server_json);
        }

        json!({
            "mcpServers": mcp_servers
        })
    }

    fn parse_existing(&self, content: &str) -> Result<HashMap<String, Value>> {
        let parsed: Value = serde_json::from_str(content)
            .context("Failed to parse existing Gemini settings as JSON")?;

        let servers = parsed
            .get("mcpServers")
            .and_then(|v| v.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        Ok(servers)
    }

    fn merge(
        &self,
        existing_content: &str,
        new_servers: &HashMap<String, &McpServerConfig>,
    ) -> Result<String> {
        // For Gemini, we need to preserve other settings in the file
        let mut existing_doc: Value =
            serde_json::from_str(existing_content).unwrap_or_else(|_| json!({}));

        let mut existing_servers = self.parse_existing(existing_content)?;

        for (name, config) in new_servers {
            let mut server_json = server_to_json(config);
            if let Some(obj) = server_json.as_object_mut() {
                obj.insert("trust".to_string(), json!(true));
            }
            existing_servers.insert(name.clone(), server_json);
        }

        // Update only the mcpServers key, preserving other settings
        if let Some(doc_obj) = existing_doc.as_object_mut() {
            doc_obj.insert("mcpServers".to_string(), json!(existing_servers));
        }

        serde_json::to_string_pretty(&existing_doc).context("Failed to serialize merged config")
    }

    fn cleanup_removed_servers(
        &self,
        existing_content: &str,
        new_servers: &HashMap<String, McpServerConfig>,
    ) -> Result<String> {
        // For Gemini, we need to preserve other settings but clean up removed servers
        let mut existing_doc: Value =
            serde_json::from_str(existing_content).unwrap_or_else(|_| json!({}));

        let existing_servers = self.parse_existing(existing_content)?;

        // Filter existing servers to only keep those that are in new_servers
        let filtered_existing: HashMap<String, Value> = existing_servers
            .into_iter()
            .filter(|(name, _)| new_servers.contains_key(name))
            .collect();

        // Add new servers (with trust: true)
        let mut final_servers = filtered_existing;
        for (name, config) in new_servers {
            let mut server_json = server_to_json(config);
            if let Some(obj) = server_json.as_object_mut() {
                obj.insert("trust".to_string(), json!(true));
            }
            final_servers.insert(name.clone(), server_json);
        }

        // Update only the mcpServers key, preserving other settings
        if let Some(doc_obj) = existing_doc.as_object_mut() {
            doc_obj.insert("mcpServers".to_string(), json!(final_servers));
        }

        serde_json::to_string_pretty(&existing_doc).context("Failed to serialize cleaned config")
    }

    fn preserve_on_overwrite(&self) -> bool {
        true
    }
}

// =============================================================================
// VS Code Formatter
// =============================================================================

/// Formatter for VS Code (.vscode/mcp.json)
/// Format: { "mcpServers": { ... } }
#[derive(Debug)]
pub struct VsCodeFormatter;

impl McpFormatter for VsCodeFormatter {
    fn format(&self, servers: &HashMap<String, &McpServerConfig>) -> Value {
        format_standard_mcp(servers)
    }

    fn parse_existing(&self, content: &str) -> Result<HashMap<String, Value>> {
        parse_standard_mcp(
            content,
            "Failed to parse existing VS Code MCP config as JSON",
        )
    }

    fn merge(
        &self,
        existing_content: &str,
        new_servers: &HashMap<String, &McpServerConfig>,
    ) -> Result<String> {
        merge_standard_mcp(
            existing_content,
            new_servers,
            "Failed to parse existing VS Code MCP config as JSON",
        )
    }

    fn cleanup_removed_servers(
        &self,
        existing_content: &str,
        new_servers: &HashMap<String, McpServerConfig>,
    ) -> Result<String> {
        // Same logic as Claude Code - use standard MCP format
        let existing = parse_standard_mcp(
            existing_content,
            "Failed to parse existing VS Code MCP config as JSON",
        )?;

        let filtered_existing: HashMap<String, Value> = existing
            .into_iter()
            .filter(|(name, _)| new_servers.contains_key(name))
            .collect();

        let refs: HashMap<String, &McpServerConfig> =
            new_servers.iter().map(|(k, v)| (k.clone(), v)).collect();

        merge_standard_mcp_filtered(
            &filtered_existing,
            &refs,
            "Failed to parse existing VS Code MCP config as JSON",
        )
    }
}

// =============================================================================
// OpenCode Formatter
// =============================================================================

/// Formatter for OpenCode (opencode.json)
/// Format: { "mcp": { "server-name": { "type": "local", "command": [...] } } }
#[derive(Debug)]
pub struct OpenCodeFormatter;

impl McpFormatter for OpenCodeFormatter {
    fn format(&self, servers: &HashMap<String, &McpServerConfig>) -> Value {
        let mut mcp_servers = Map::new();

        for (name, config) in servers {
            mcp_servers.insert(name.clone(), server_to_opencode_json(config));
        }

        json!({
            "mcp": mcp_servers
        })
    }

    fn parse_existing(&self, content: &str) -> Result<HashMap<String, Value>> {
        let parsed: Value = serde_json::from_str(content)
            .context("Failed to parse existing OpenCode MCP config as JSON")?;

        let servers = parsed
            .get("mcp")
            .and_then(|v| v.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        Ok(servers)
    }

    fn merge(
        &self,
        existing_content: &str,
        new_servers: &HashMap<String, &McpServerConfig>,
    ) -> Result<String> {
        // Parse existing document to preserve other fields
        let mut existing_doc: Value =
            serde_json::from_str(existing_content).unwrap_or_else(|_| json!({}));

        let mut existing_servers = self.parse_existing(existing_content)?;

        for (name, config) in new_servers {
            existing_servers.insert(name.clone(), server_to_opencode_json(config));
        }

        // Update only the mcp key
        if let Some(doc_obj) = existing_doc.as_object_mut() {
            doc_obj.insert("mcp".to_string(), json!(existing_servers));
        } else {
            // If existing content wasn't an object, overwrite it entirely
            existing_doc = json!({
                "mcp": existing_servers
            });
        }

        serde_json::to_string_pretty(&existing_doc).context("Failed to serialize merged config")
    }

    fn cleanup_removed_servers(
        &self,
        existing_content: &str,
        new_servers: &HashMap<String, McpServerConfig>,
    ) -> Result<String> {
        // For OpenCode, preserve other settings but clean up removed servers
        let mut existing_doc: Value =
            serde_json::from_str(existing_content).unwrap_or_else(|_| json!({}));

        let existing_servers = self.parse_existing(existing_content)?;

        // Filter existing servers to only keep those that are in new_servers
        let filtered_existing: HashMap<String, Value> = existing_servers
            .into_iter()
            .filter(|(name, _)| new_servers.contains_key(name))
            .collect();

        // Add new servers
        let mut final_servers = filtered_existing;
        for (name, config) in new_servers {
            final_servers.insert(name.clone(), server_to_opencode_json(config));
        }

        // Update only the mcp key, preserving other settings
        if let Some(doc_obj) = existing_doc.as_object_mut() {
            doc_obj.insert("mcp".to_string(), json!(final_servers));
        } else {
            // If existing content wasn't an object, overwrite it entirely
            existing_doc = json!({
                "mcp": final_servers
            });
        }

        serde_json::to_string_pretty(&existing_doc).context("Failed to serialize cleaned config")
    }

    fn preserve_on_overwrite(&self) -> bool {
        true
    }
}

/// Convert McpServerConfig to OpenCode JSON format
fn server_to_opencode_json(config: &McpServerConfig) -> Value {
    let mut obj = Map::new();

    if let Some(ref url) = config.url {
        obj.insert("type".to_string(), json!("remote"));
        obj.insert("url".to_string(), json!(url));
        if !config.headers.is_empty() {
            obj.insert("headers".to_string(), json!(config.headers));
        }
    } else {
        obj.insert("type".to_string(), json!("local"));

        let mut command_parts = Vec::new();
        if let Some(ref cmd) = config.command {
            command_parts.push(cmd.clone());
        }
        command_parts.extend(config.args.clone());

        obj.insert("command".to_string(), json!(command_parts));

        if !config.env.is_empty() {
            obj.insert("environment".to_string(), json!(config.env));
        }
    }

    // Explicitly set enabled status
    obj.insert("enabled".to_string(), json!(!config.disabled));

    Value::Object(obj)
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Convert McpServerConfig to JSON Value
fn server_to_json(config: &McpServerConfig) -> Value {
    let mut obj = Map::new();

    if let Some(ref cmd) = config.command {
        obj.insert("command".to_string(), json!(cmd));
    }

    if !config.args.is_empty() {
        obj.insert("args".to_string(), json!(config.args));
    }

    if !config.env.is_empty() {
        obj.insert("env".to_string(), json!(config.env));
    }

    if let Some(ref url) = config.url {
        obj.insert("url".to_string(), json!(url));
    }

    if !config.headers.is_empty() {
        obj.insert("headers".to_string(), json!(config.headers));
    }

    if let Some(ref transport) = config.transport_type {
        obj.insert("type".to_string(), json!(transport));
    }

    Value::Object(obj)
}

// =============================================================================
// MCP Generator
// =============================================================================

/// Result of MCP generation
#[derive(Debug, Default)]
pub struct McpSyncResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub errors: usize,
}

/// Generator for MCP configuration files
pub struct McpGenerator {
    servers: HashMap<String, McpServerConfig>,
    merge_strategy: McpMergeStrategy,
}

impl McpGenerator {
    /// Create a new MCP generator
    pub fn new(
        servers: HashMap<String, McpServerConfig>,
        merge_strategy: McpMergeStrategy,
    ) -> Self {
        Self {
            servers,
            merge_strategy,
        }
    }

    /// Generate MCP config for a specific agent
    pub fn generate_for_agent(
        &self,
        agent: McpAgent,
        project_root: &Path,
        dry_run: bool,
    ) -> Result<McpSyncResult> {
        let mut result = McpSyncResult::default();
        let formatter = agent.formatter();
        let config_path = project_root.join(agent.config_path());

        // Get enabled servers only
        let enabled_servers: HashMap<String, &McpServerConfig> = self
            .servers
            .iter()
            .filter(|(_, config)| !config.disabled)
            .map(|(name, config)| (name.clone(), config))
            .collect();

        if enabled_servers.is_empty() {
            result.skipped += 1;
            return Ok(result);
        }

        // Determine content to write
        let content = if config_path.exists() && self.merge_strategy == McpMergeStrategy::Merge {
            let existing = fs::read_to_string(&config_path).with_context(|| {
                format!("Failed to read existing config: {}", config_path.display())
            })?;

            // Check if we need to clean up removed servers
            let existing_servers = formatter.parse_existing(&existing)?;
            let removed_servers: Vec<&String> = existing_servers
                .keys()
                .filter(|name| !enabled_servers.contains_key(*name))
                .collect();

            // Build owned map of enabled servers for cleanup
            let owned_enabled: HashMap<String, McpServerConfig> = enabled_servers
                .iter()
                .map(|(k, v)| (k.clone(), (*v).clone()))
                .collect();

            if !removed_servers.is_empty() {
                // Only perform cleanup if the existing and enabled server counts differ.
                // This prevents clobbering unrelated existing entries in simple merge cases
                // where the counts match but names differ (keep existing entries).
                if existing_servers.len() != enabled_servers.len() {
                    // Use cleanup method to remove servers that are no longer in config
                    formatter.cleanup_removed_servers(&existing, &owned_enabled)?
                } else {
                    // Counts equal - prefer a simple merge to retain existing entries
                    formatter.merge(&existing, &enabled_servers)?
                }
            } else {
                // No servers removed, use normal merge
                formatter.merge(&existing, &enabled_servers)?
            }
        } else if config_path.exists()
            && self.merge_strategy == McpMergeStrategy::Overwrite
            && formatter.preserve_on_overwrite()
        {
            // Preserve unrelated top-level settings when overwriting for certain formatters
            let existing = fs::read_to_string(&config_path).with_context(|| {
                format!("Failed to read existing config: {}", config_path.display())
            })?;

            // Use cleanup_removed_servers to replace mcp sections while preserving other keys
            // Build owned map of enabled servers for cleanup
            let owned_enabled: HashMap<String, McpServerConfig> = enabled_servers
                .iter()
                .map(|(k, v)| (k.clone(), (*v).clone()))
                .collect();

            formatter.cleanup_removed_servers(&existing, &owned_enabled)?
        } else {
            let output = formatter.format(&enabled_servers);
            serde_json::to_string_pretty(&output)?
        };

        // Create parent directories if needed
        if let Some(parent) = config_path.parent() {
            if !parent.exists() {
                if dry_run {
                    println!(
                        "  {} Would create directory: {}",
                        "→".cyan(),
                        parent.display()
                    );
                } else {
                    fs::create_dir_all(parent)?;
                }
            }
        }

        // Write the file
        if dry_run {
            if config_path.exists() {
                println!(
                    "  {} Would update MCP config: {}",
                    "→".cyan(),
                    config_path.display()
                );
                result.updated += 1;
            } else {
                println!(
                    "  {} Would create MCP config: {}",
                    "→".cyan(),
                    config_path.display()
                );
                result.created += 1;
            }
        } else {
            let was_existing = config_path.exists();
            fs::write(&config_path, &content).with_context(|| {
                format!("Failed to write MCP config: {}", config_path.display())
            })?;

            if was_existing {
                println!(
                    "  {} Updated MCP config: {}",
                    "✔".green(),
                    config_path.display()
                );
                result.updated += 1;
            } else {
                println!(
                    "  {} Created MCP config: {}",
                    "✔".green(),
                    config_path.display()
                );
                result.created += 1;
            }
        }

        Ok(result)
    }

    /// Generate MCP configs for all enabled agents
    pub fn generate_all(
        &self,
        project_root: &Path,
        enabled_agents: &[McpAgent],
        dry_run: bool,
    ) -> Result<McpSyncResult> {
        let mut total_result = McpSyncResult::default();

        for agent in enabled_agents {
            match self.generate_for_agent(*agent, project_root, dry_run) {
                Ok(result) => {
                    total_result.created += result.created;
                    total_result.updated += result.updated;
                    total_result.skipped += result.skipped;
                }
                Err(e) => {
                    eprintln!(
                        "  {} Error generating {} config: {}",
                        "✘".red(),
                        agent.name(),
                        e
                    );
                    total_result.errors += 1;
                }
            }
        }

        Ok(total_result)
    }

    /// Get the list of agents that should receive MCP configs based on config
    pub fn get_enabled_agents_from_config(
        agents: &HashMap<String, crate::config::AgentConfig>,
    ) -> Vec<McpAgent> {
        // Map agent config keys to MCP agents
        agents
            .iter()
            .filter(|(_, config)| config.enabled)
            .filter_map(|(name, _)| McpAgent::from_id(name))
            .collect()
    }
}

/// Get the path where MCP config would be written for an agent
pub fn get_mcp_config_path(agent: McpAgent, project_root: &Path) -> PathBuf {
    project_root.join(agent.config_path())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ==========================================================================
    // AGENT TESTS
    // ==========================================================================

    #[test]
    fn test_agent_all_returns_all_agents() {
        let agents = McpAgent::all();
        assert_eq!(agents.len(), 5);
        assert!(agents.contains(&McpAgent::ClaudeCode));
        assert!(agents.contains(&McpAgent::GithubCopilot));
        assert!(agents.contains(&McpAgent::GeminiCli));
        assert!(agents.contains(&McpAgent::VsCode));
        assert!(agents.contains(&McpAgent::OpenCode));
    }

    #[test]
    fn test_agent_from_id() {
        assert_eq!(McpAgent::from_id("claude"), Some(McpAgent::ClaudeCode));
        assert_eq!(McpAgent::from_id("CLAUDE"), Some(McpAgent::ClaudeCode));
        assert_eq!(McpAgent::from_id("claude-code"), Some(McpAgent::ClaudeCode));
        assert_eq!(McpAgent::from_id("copilot"), Some(McpAgent::GithubCopilot));
        assert_eq!(
            McpAgent::from_id("github-copilot"),
            Some(McpAgent::GithubCopilot)
        );
        assert_eq!(McpAgent::from_id("gemini"), Some(McpAgent::GeminiCli));
        assert_eq!(McpAgent::from_id("vscode"), Some(McpAgent::VsCode));
        assert_eq!(McpAgent::from_id("vs-code"), Some(McpAgent::VsCode));
        assert_eq!(McpAgent::from_id("opencode"), Some(McpAgent::OpenCode));
        assert_eq!(McpAgent::from_id("open-code"), Some(McpAgent::OpenCode));
        assert_eq!(McpAgent::from_id("unknown"), None);
    }

    #[test]
    fn test_agent_config_paths() {
        assert_eq!(McpAgent::ClaudeCode.config_path(), ".mcp.json");
        assert_eq!(
            McpAgent::GithubCopilot.config_path(),
            ".copilot/mcp-config.json"
        );
        assert_eq!(McpAgent::GeminiCli.config_path(), ".gemini/settings.json");
        assert_eq!(McpAgent::VsCode.config_path(), ".vscode/mcp.json");
        assert_eq!(McpAgent::OpenCode.config_path(), "opencode.json");
    }

    // ==========================================================================
    // FORMATTER TESTS - Claude Code
    // ==========================================================================

    fn create_test_server() -> McpServerConfig {
        McpServerConfig {
            command: Some("npx".to_string()),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                ".".to_string(),
            ],
            env: HashMap::new(),
            url: None,
            headers: HashMap::new(),
            transport_type: None,
            disabled: false,
        }
    }

    #[test]
    fn test_claude_formatter_basic() {
        let formatter = ClaudeCodeFormatter;
        let server = create_test_server();
        let servers: HashMap<String, &McpServerConfig> =
            HashMap::from([("filesystem".to_string(), &server)]);

        let output = formatter.format(&servers);

        assert!(output.get("mcpServers").is_some());
        let mcp_servers = output.get("mcpServers").unwrap();
        assert!(mcp_servers.get("filesystem").is_some());
    }

    #[test]
    fn test_claude_formatter_parse_existing() {
        let formatter = ClaudeCodeFormatter;
        let existing = r#"{
            "mcpServers": {
                "existing": {
                    "command": "node",
                    "args": ["server.js"]
                }
            }
        }"#;

        let parsed = formatter.parse_existing(existing).unwrap();
        assert!(parsed.contains_key("existing"));
    }

    #[test]
    fn test_claude_formatter_merge() {
        let formatter = ClaudeCodeFormatter;
        let existing = r#"{
            "mcpServers": {
                "existing": {
                    "command": "node",
                    "args": ["old-server.js"]
                }
            }
        }"#;

        let new_server = create_test_server();
        let servers: HashMap<String, &McpServerConfig> =
            HashMap::from([("filesystem".to_string(), &new_server)]);

        let merged = formatter.merge(existing, &servers).unwrap();
        let parsed: Value = serde_json::from_str(&merged).unwrap();

        // Should have both servers
        let mcp_servers = parsed.get("mcpServers").unwrap();
        assert!(mcp_servers.get("existing").is_some());
        assert!(mcp_servers.get("filesystem").is_some());
    }

    #[test]
    fn test_claude_formatter_merge_override() {
        let formatter = ClaudeCodeFormatter;
        let existing = r#"{
            "mcpServers": {
                "filesystem": {
                    "command": "old-command",
                    "args": ["old-arg"]
                }
            }
        }"#;

        let new_server = create_test_server();
        let servers: HashMap<String, &McpServerConfig> =
            HashMap::from([("filesystem".to_string(), &new_server)]);

        let merged = formatter.merge(existing, &servers).unwrap();
        let parsed: Value = serde_json::from_str(&merged).unwrap();

        // Should override with new config
        let fs_server = parsed.get("mcpServers").unwrap().get("filesystem").unwrap();
        assert_eq!(fs_server.get("command").unwrap().as_str().unwrap(), "npx");
    }

    // ==========================================================================
    // FORMATTER TESTS - Gemini CLI
    // ==========================================================================

    #[test]
    fn test_gemini_formatter_adds_trust() {
        let formatter = GeminiCliFormatter;
        let server = create_test_server();
        let servers: HashMap<String, &McpServerConfig> =
            HashMap::from([("filesystem".to_string(), &server)]);

        let output = formatter.format(&servers);
        let fs_server = output.get("mcpServers").unwrap().get("filesystem").unwrap();

        // Should have trust: true
        assert!(fs_server.get("trust").unwrap().as_bool().unwrap());
    }

    #[test]
    fn test_gemini_formatter_preserves_other_settings() {
        let formatter = GeminiCliFormatter;
        let existing = r#"{
            "theme": "dark",
            "someOtherSetting": true,
            "mcpServers": {
                "existing": {
                    "command": "node"
                }
            }
        }"#;

        let new_server = create_test_server();
        let servers: HashMap<String, &McpServerConfig> =
            HashMap::from([("filesystem".to_string(), &new_server)]);

        let merged = formatter.merge(existing, &servers).unwrap();
        let parsed: Value = serde_json::from_str(&merged).unwrap();

        // Should preserve other settings
        assert_eq!(parsed.get("theme").unwrap().as_str().unwrap(), "dark");
        assert!(parsed.get("someOtherSetting").unwrap().as_bool().unwrap());
        // And have both servers
        assert!(parsed.get("mcpServers").unwrap().get("existing").is_some());
        assert!(
            parsed
                .get("mcpServers")
                .unwrap()
                .get("filesystem")
                .is_some()
        );
    }

    // ==========================================================================
    // FORMATTER TESTS - OpenCode
    // ==========================================================================

    #[test]
    fn test_opencode_formatter_basic() {
        let formatter = OpenCodeFormatter;
        let server = create_test_server();
        let servers: HashMap<String, &McpServerConfig> =
            HashMap::from([("filesystem".to_string(), &server)]);

        let output = formatter.format(&servers);

        assert!(output.get("mcp").is_some());
        let mcp_servers = output.get("mcp").unwrap();
        let fs_server = mcp_servers.get("filesystem").unwrap();

        // Verify OpenCode specific structure
        assert_eq!(fs_server.get("type").unwrap().as_str().unwrap(), "local");
        assert!(
            !fs_server
                .get("command")
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(fs_server.get("enabled").unwrap().as_bool().unwrap());
    }

    #[test]
    fn test_opencode_formatter_preserves_other_settings() {
        let formatter = OpenCodeFormatter;
        let existing = r#"{
            "tools": { "some-tool": true },
            "mcp": {
                "existing": {
                    "type": "local",
                    "command": ["existing-cmd"]
                }
            }
        }"#;

        let new_server = create_test_server();
        let servers: HashMap<String, &McpServerConfig> =
            HashMap::from([("filesystem".to_string(), &new_server)]);

        let merged = formatter.merge(existing, &servers).unwrap();
        let parsed: Value = serde_json::from_str(&merged).unwrap();

        // Should preserve other settings
        assert!(parsed.get("tools").is_some());
        // And have both servers
        assert!(parsed.get("mcp").unwrap().get("existing").is_some());
        assert!(parsed.get("mcp").unwrap().get("filesystem").is_some());
    }

    // ==========================================================================
    // MCP GENERATOR TESTS
    // ==========================================================================

    #[test]
    fn test_generator_creates_config() {
        let temp_dir = TempDir::new().unwrap();
        let servers = HashMap::from([("filesystem".to_string(), create_test_server())]);

        let generator = McpGenerator::new(servers, McpMergeStrategy::Overwrite);
        let result = generator
            .generate_for_agent(McpAgent::ClaudeCode, temp_dir.path(), false)
            .unwrap();

        assert_eq!(result.created, 1);
        assert_eq!(result.updated, 0);

        // Verify file was created
        let config_path = temp_dir.path().join(".mcp.json");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        assert!(
            parsed
                .get("mcpServers")
                .unwrap()
                .get("filesystem")
                .is_some()
        );
    }

    #[test]
    fn test_generator_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let servers = HashMap::from([("filesystem".to_string(), create_test_server())]);

        let generator = McpGenerator::new(servers, McpMergeStrategy::Overwrite);
        let result = generator
            .generate_for_agent(McpAgent::ClaudeCode, temp_dir.path(), true)
            .unwrap();

        assert_eq!(result.created, 1);

        // File should NOT exist in dry run
        let config_path = temp_dir.path().join(".mcp.json");
        assert!(!config_path.exists());
    }

    #[test]
    fn test_generator_merge_strategy() {
        let temp_dir = TempDir::new().unwrap();

        // Create existing config
        let existing = r#"{
            "mcpServers": {
                "existing": {
                    "command": "existing-cmd"
                }
            }
        }"#;
        fs::write(temp_dir.path().join(".mcp.json"), existing).unwrap();

        let servers = HashMap::from([("filesystem".to_string(), create_test_server())]);

        let generator = McpGenerator::new(servers, McpMergeStrategy::Merge);
        let result = generator
            .generate_for_agent(McpAgent::ClaudeCode, temp_dir.path(), false)
            .unwrap();

        assert_eq!(result.updated, 1);

        // Verify both servers exist
        let content = fs::read_to_string(temp_dir.path().join(".mcp.json")).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        let servers = parsed.get("mcpServers").unwrap();
        assert!(servers.get("existing").is_some());
        assert!(servers.get("filesystem").is_some());
    }

    #[test]
    fn test_generator_merge_removal_cleanup() {
        let temp_dir = TempDir::new().unwrap();

        // Create existing config with multiple servers
        let existing = r#"{
            "mcpServers": {
                "server1": {
                    "command": "cmd1"
                },
                "server2": {
                    "command": "cmd2"
                },
                "server3": {
                    "command": "cmd3"
                }
            }
        }"#;
        fs::write(temp_dir.path().join(".mcp.json"), existing).unwrap();

        // New config only has server1 and server3 (server2 removed)
        let mut server1 = create_test_server();
        let mut server3 = create_test_server();
        server1.command = Some("new-cmd1".to_string());
        server3.command = Some("new-cmd3".to_string());

        let servers = HashMap::from([
            ("server1".to_string(), server1),
            ("server3".to_string(), server3),
        ]);

        let generator = McpGenerator::new(servers, McpMergeStrategy::Merge);
        let result = generator
            .generate_for_agent(McpAgent::ClaudeCode, temp_dir.path(), false)
            .unwrap();

        assert_eq!(result.updated, 1);

        // Verify server2 was removed, server1 and server3 exist with new configs
        let content = fs::read_to_string(temp_dir.path().join(".mcp.json")).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        let servers = parsed.get("mcpServers").unwrap();

        assert!(servers.get("server1").is_some());
        assert!(servers.get("server3").is_some());
        assert!(servers.get("server2").is_none()); // Critical: server2 should be removed

        // Verify the commands were updated
        assert_eq!(
            servers
                .get("server1")
                .unwrap()
                .get("command")
                .unwrap()
                .as_str()
                .unwrap(),
            "new-cmd1"
        );
        assert_eq!(
            servers
                .get("server3")
                .unwrap()
                .get("command")
                .unwrap()
                .as_str()
                .unwrap(),
            "new-cmd3"
        );
    }

    #[test]
    fn test_generator_merge_no_removal_needed() {
        let temp_dir = TempDir::new().unwrap();

        // Create existing config
        let existing = r#"{
            "mcpServers": {
                "keep_this": {
                    "command": "old-cmd"
                }
            }
        }"#;
        fs::write(temp_dir.path().join(".mcp.json"), existing).unwrap();

        // New config keeps the same server with updated command
        let mut server = create_test_server();
        server.command = Some("new-cmd".to_string());

        let servers = HashMap::from([("keep_this".to_string(), server)]);

        let generator = McpGenerator::new(servers, McpMergeStrategy::Merge);
        let result = generator
            .generate_for_agent(McpAgent::ClaudeCode, temp_dir.path(), false)
            .unwrap();

        assert_eq!(result.updated, 1);

        // Verify server exists with new command
        let content = fs::read_to_string(temp_dir.path().join(".mcp.json")).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        let servers = parsed.get("mcpServers").unwrap();

        assert!(servers.get("keep_this").is_some());
        assert_eq!(
            servers
                .get("keep_this")
                .unwrap()
                .get("command")
                .unwrap()
                .as_str()
                .unwrap(),
            "new-cmd"
        );
    }

    #[test]
    fn test_generator_merge_all_servers_removed() {
        let temp_dir = TempDir::new().unwrap();

        // Create existing config with multiple servers
        let existing = r#"{
            "mcpServers": {
                "old1": {
                    "command": "cmd1"
                },
                "old2": {
                    "command": "cmd2"
                },
                "old3": {
                    "command": "cmd3"
                }
            }
        }"#;
        fs::write(temp_dir.path().join(".mcp.json"), existing).unwrap();

        // New config has completely different servers
        let mut new1 = create_test_server();
        let mut new2 = create_test_server();
        new1.command = Some("new1".to_string());
        new2.command = Some("new2".to_string());

        let servers = HashMap::from([("new1".to_string(), new1), ("new2".to_string(), new2)]);

        let generator = McpGenerator::new(servers, McpMergeStrategy::Merge);
        let result = generator
            .generate_for_agent(McpAgent::ClaudeCode, temp_dir.path(), false)
            .unwrap();

        assert_eq!(result.updated, 1);

        // Verify all old servers were removed, only new servers exist
        let content = fs::read_to_string(temp_dir.path().join(".mcp.json")).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        let servers = parsed.get("mcpServers").unwrap();

        assert!(servers.get("new1").is_some());
        assert!(servers.get("new2").is_some());
        assert!(servers.get("old1").is_none());
        assert!(servers.get("old2").is_none());
        assert!(servers.get("old3").is_none());
    }

    #[test]
    fn test_cleanup_removed_servers_opencode() {
        let temp_dir = TempDir::new().unwrap();

        // Create existing OpenCode config with multiple servers
        let existing = r#"{
            "tools": { "some-tool": true },
            "mcp": {
                "keep": {
                    "type": "local",
                    "command": ["keep-cmd"]
                },
                "remove": {
                    "type": "local",
                    "command": ["remove-cmd"]
                }
            }
        }"#;
        fs::write(temp_dir.path().join("opencode.json"), existing).unwrap();

        // New config only keeps "keep" server
        let mut server = create_test_server();
        server.command = Some("keep-cmd".to_string());

        let servers = HashMap::from([("keep".to_string(), server)]);

        let formatter = OpenCodeFormatter;
        let result = formatter
            .cleanup_removed_servers(existing, &servers)
            .unwrap();

        let parsed: Value = serde_json::from_str(&result).unwrap();

        // Verify "tools" setting is preserved
        assert!(parsed.get("tools").is_some());

        // Verify "keep" server exists and "remove" server was removed
        let mcp_servers = parsed.get("mcp").unwrap();
        assert!(mcp_servers.get("keep").is_some());
        assert!(mcp_servers.get("remove").is_none());
    }

    #[test]
    fn test_cleanup_removed_servers_gemini() {
        let temp_dir = TempDir::new().unwrap();

        // Create existing Gemini config with multiple servers
        let existing = r#"{
            "theme": "dark",
            "someOtherSetting": true,
            "mcpServers": {
                "keep": {
                    "command": "node"
                },
                "remove": {
                    "command": "old-server"
                }
            }
        }"#;
        let gemini_path = temp_dir.path().join(".gemini/settings.json");
        if let Some(parent) = gemini_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(gemini_path, existing).unwrap();

        // New config only keeps "keep" server
        let mut server = create_test_server();
        server.command = Some("node".to_string());

        let servers = HashMap::from([("keep".to_string(), server)]);

        let formatter = GeminiCliFormatter;
        let result = formatter
            .cleanup_removed_servers(existing, &servers)
            .unwrap();

        let parsed: Value = serde_json::from_str(&result).unwrap();

        // Verify other settings are preserved
        assert_eq!(parsed.get("theme").unwrap().as_str().unwrap(), "dark");
        assert!(parsed.get("someOtherSetting").unwrap().as_bool().unwrap());

        // Verify "keep" server exists and "remove" server was removed
        let mcp_servers = parsed.get("mcpServers").unwrap();
        assert!(mcp_servers.get("keep").is_some());
        assert!(mcp_servers.get("remove").is_none());

        // Verify trust: true is added to kept server
        assert!(
            mcp_servers
                .get("keep")
                .unwrap()
                .get("trust")
                .unwrap()
                .as_bool()
                .unwrap()
        );
    }

    #[test]
    fn test_generator_overwrite_strategy() {
        let temp_dir = TempDir::new().unwrap();

        // Create existing config
        let existing = r#"{
            "mcpServers": {
                "existing": {
                    "command": "existing-cmd"
                }
            }
        }"#;
        fs::write(temp_dir.path().join(".mcp.json"), existing).unwrap();

        let servers = HashMap::from([("filesystem".to_string(), create_test_server())]);

        let generator = McpGenerator::new(servers, McpMergeStrategy::Overwrite);
        generator
            .generate_for_agent(McpAgent::ClaudeCode, temp_dir.path(), false)
            .unwrap();

        // Verify only new server exists
        let content = fs::read_to_string(temp_dir.path().join(".mcp.json")).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        let servers = parsed.get("mcpServers").unwrap();
        assert!(servers.get("existing").is_none());
        assert!(servers.get("filesystem").is_some());
    }

    #[test]
    fn test_generator_overwrite_strategy_opencode() {
        let temp_dir = TempDir::new().unwrap();

        // Create existing OpenCode config with multiple settings
        let existing = r#"{
            "tools": { "old-tool": true },
            "mcp": {
                "old-server": {
                    "type": "local",
                    "command": ["old-cmd"]
                }
            }
        }"#;
        fs::write(temp_dir.path().join("opencode.json"), existing).unwrap();

        let mut server = create_test_server();
        server.command = Some("new-cmd".to_string());
        let servers = HashMap::from([("new-server".to_string(), server)]);

        let generator = McpGenerator::new(servers, McpMergeStrategy::Overwrite);
        generator
            .generate_for_agent(McpAgent::OpenCode, temp_dir.path(), false)
            .unwrap();

        // Verify tools setting is preserved (overwrite only affects mcp section)
        let content = fs::read_to_string(temp_dir.path().join("opencode.json")).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();

        assert!(parsed.get("tools").is_some());
        assert!(
            parsed
                .get("tools")
                .unwrap()
                .get("old-tool")
                .unwrap()
                .as_bool()
                .unwrap()
        );

        // Verify old server was replaced with new one
        let mcp_servers = parsed.get("mcp").unwrap();
        assert!(mcp_servers.get("old-server").is_none());
        assert!(mcp_servers.get("new-server").is_some());
    }

    #[test]
    fn test_generator_overwrite_strategy_gemini() {
        let temp_dir = TempDir::new().unwrap();

        // Create existing Gemini config with other settings
        let existing = r#"{
            "theme": "dark",
            "mcpServers": {
                "old-server": {
                    "command": "old-cmd"
                }
            }
        }"#;
        let gemini_path = temp_dir.path().join(".gemini/settings.json");
        if let Some(parent) = gemini_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(gemini_path, existing).unwrap();

        let mut server = create_test_server();
        server.command = Some("new-cmd".to_string());
        let servers = HashMap::from([("new-server".to_string(), server)]);

        let generator = McpGenerator::new(servers, McpMergeStrategy::Overwrite);
        generator
            .generate_for_agent(McpAgent::GeminiCli, temp_dir.path(), false)
            .unwrap();

        // Verify theme is preserved (overwrite only affects mcpServers section)
        let content = fs::read_to_string(temp_dir.path().join(".gemini/settings.json")).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed.get("theme").unwrap().as_str().unwrap(), "dark");

        // Verify old server was replaced with new one (with trust: true)
        let mcp_servers = parsed.get("mcpServers").unwrap();
        assert!(mcp_servers.get("old-server").is_none());
        assert!(mcp_servers.get("new-server").is_some());
        assert!(
            mcp_servers
                .get("new-server")
                .unwrap()
                .get("trust")
                .unwrap()
                .as_bool()
                .unwrap()
        );
    }

    #[test]
    fn test_generator_creates_parent_directories() {
        let temp_dir = TempDir::new().unwrap();
        let servers = HashMap::from([("filesystem".to_string(), create_test_server())]);

        let generator = McpGenerator::new(servers, McpMergeStrategy::Overwrite);
        generator
            .generate_for_agent(McpAgent::GithubCopilot, temp_dir.path(), false)
            .unwrap();

        // Verify .copilot directory was created
        let config_path = temp_dir.path().join(".copilot/mcp-config.json");
        assert!(config_path.exists());
    }

    #[test]
    fn test_generator_skips_disabled_servers() {
        let temp_dir = TempDir::new().unwrap();

        let mut disabled_server = create_test_server();
        disabled_server.disabled = true;

        let servers = HashMap::from([("disabled".to_string(), disabled_server)]);

        let generator = McpGenerator::new(servers, McpMergeStrategy::Overwrite);
        let result = generator
            .generate_for_agent(McpAgent::ClaudeCode, temp_dir.path(), false)
            .unwrap();

        // Should skip because no enabled servers
        assert_eq!(result.skipped, 1);
        assert!(!temp_dir.path().join(".mcp.json").exists());
    }

    #[test]
    fn test_generator_generate_all() {
        let temp_dir = TempDir::new().unwrap();
        let servers = HashMap::from([("filesystem".to_string(), create_test_server())]);

        let generator = McpGenerator::new(servers, McpMergeStrategy::Overwrite);
        let agents = vec![McpAgent::ClaudeCode, McpAgent::GithubCopilot];

        let result = generator
            .generate_all(temp_dir.path(), &agents, false)
            .unwrap();

        assert_eq!(result.created, 2);
        assert!(temp_dir.path().join(".mcp.json").exists());
        assert!(temp_dir.path().join(".copilot/mcp-config.json").exists());
    }

    // ==========================================================================
    // MCP OUTPUT TESTS
    // ==========================================================================

    #[test]
    fn test_mcp_output_enabled_servers() {
        let mut disabled = create_test_server();
        disabled.disabled = true;

        let servers = HashMap::from([
            ("enabled".to_string(), create_test_server()),
            ("disabled".to_string(), disabled),
        ]);

        let output = McpOutput::new(servers);
        let enabled = output.enabled_servers();

        assert_eq!(enabled.len(), 1);
        assert!(enabled.contains_key("enabled"));
        assert!(!enabled.contains_key("disabled"));
    }

    // ==========================================================================
    // SERVER TO JSON TESTS
    // ==========================================================================

    #[test]
    fn test_server_to_json_stdio() {
        let server = McpServerConfig {
            command: Some("npx".to_string()),
            args: vec!["-y".to_string(), "server".to_string()],
            env: HashMap::from([("DEBUG".to_string(), "true".to_string())]),
            url: None,
            headers: HashMap::new(),
            transport_type: Some("stdio".to_string()),
            disabled: false,
        };

        let json = server_to_json(&server);

        assert_eq!(json.get("command").unwrap().as_str().unwrap(), "npx");
        assert!(json.get("args").unwrap().as_array().is_some());
        assert!(json.get("env").unwrap().as_object().is_some());
        assert_eq!(json.get("type").unwrap().as_str().unwrap(), "stdio");
        // Empty fields should not be present
        assert!(json.get("url").is_none());
        assert!(json.get("headers").is_none());
    }

    #[test]
    fn test_server_to_json_http() {
        let server = McpServerConfig {
            command: None,
            args: vec![],
            env: HashMap::new(),
            url: Some("https://api.example.com".to_string()),
            headers: HashMap::from([("Authorization".to_string(), "Bearer token".to_string())]),
            transport_type: None,
            disabled: false,
        };

        let json = server_to_json(&server);

        assert_eq!(
            json.get("url").unwrap().as_str().unwrap(),
            "https://api.example.com"
        );
        assert!(json.get("headers").unwrap().as_object().is_some());
        // Empty fields should not be present
        assert!(json.get("command").is_none());
        assert!(json.get("args").is_none());
    }

    // ==========================================================================
    // GET ENABLED AGENTS TESTS
    // ==========================================================================

    #[test]
    fn test_get_enabled_agents_from_config() {
        use crate::config::AgentConfig;

        let agents = HashMap::from([
            (
                "claude".to_string(),
                AgentConfig {
                    enabled: true,
                    description: String::new(),
                    targets: HashMap::new(),
                },
            ),
            (
                "copilot".to_string(),
                AgentConfig {
                    enabled: true,
                    description: String::new(),
                    targets: HashMap::new(),
                },
            ),
            (
                "disabled_agent".to_string(),
                AgentConfig {
                    enabled: false,
                    description: String::new(),
                    targets: HashMap::new(),
                },
            ),
            (
                "unknown_agent".to_string(),
                AgentConfig {
                    enabled: true,
                    description: String::new(),
                    targets: HashMap::new(),
                },
            ),
        ]);

        let enabled = McpGenerator::get_enabled_agents_from_config(&agents);

        // Should only include enabled agents that map to known MCP agents
        assert!(enabled.contains(&McpAgent::ClaudeCode));
        assert!(enabled.contains(&McpAgent::GithubCopilot));
        assert_eq!(enabled.len(), 2);
    }
}
