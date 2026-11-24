//! Cross-agent capability adapters.
//!
//! Defines typed adapters for cross-agent capability mapping with loss reporting.
//! Each agent has a capability profile that describes what features it supports.
//! When mapping configuration between agents, the adapter reports any capabilities
//! that cannot be mapped (loss reporting).

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A capability that an agent may support.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// Agent supports MCP server configuration
    McpServers,
    /// Agent supports skill/instruction files
    Skills,
    /// Agent supports command definitions
    Commands,
    /// Agent supports custom rules
    Rules,
    /// Agent supports environment variables
    EnvironmentVariables,
    /// Agent supports model selection
    ModelSelection,
    /// Agent supports system prompts
    SystemPrompts,
    /// Agent supports file-based configuration
    FileConfig,
    /// Agent supports directory-based configuration
    DirectoryConfig,
    /// Agent supports nested configuration
    NestedConfig,
}

/// A capability adapter maps capabilities between agents.
pub trait CapabilityAdapter: Send + Sync {
    /// Return the canonical agent ID this adapter supports.
    fn agent_id(&self) -> &str;

    /// Return the set of capabilities this agent supports.
    fn supported_capabilities(&self) -> BTreeSet<Capability>;

    /// Check if this agent supports a specific capability.
    fn supports(&self, capability: &Capability) -> bool {
        self.supported_capabilities().contains(capability)
    }
}

/// Result of mapping capabilities between agents.
#[derive(Debug, Clone, Serialize)]
pub struct MappingResult {
    /// Source agent ID.
    pub source: String,
    /// Target agent ID.
    pub target: String,
    /// Capabilities that were successfully mapped.
    pub mapped: Vec<Capability>,
    /// Capabilities that could not be mapped (loss).
    pub unmapped: Vec<Capability>,
    /// Warnings about partial mappings.
    pub warnings: Vec<String>,
}

impl MappingResult {
    /// Create a new mapping result.
    pub fn new(source: String, target: String) -> Self {
        Self {
            source,
            target,
            mapped: Vec::new(),
            unmapped: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Add a successfully mapped capability.
    pub fn add_mapped(&mut self, capability: Capability) {
        self.mapped.push(capability);
    }

    /// Add an unmapped capability (loss).
    pub fn add_unmapped(&mut self, capability: Capability, reason: String) {
        self.unmapped.push(capability);
        self.warnings.push(reason);
    }

    /// Check if all capabilities were mapped successfully.
    pub fn is_complete(&self) -> bool {
        self.unmapped.is_empty()
    }

    /// Get the number of capabilities that were lost.
    pub fn loss_count(&self) -> usize {
        self.unmapped.len()
    }
}

/// A capability adapter for Claude Code.
pub struct ClaudeAdapter;

impl CapabilityAdapter for ClaudeAdapter {
    fn agent_id(&self) -> &str {
        "claude"
    }

    fn supported_capabilities(&self) -> BTreeSet<Capability> {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::McpServers);
        caps.insert(Capability::Skills);
        caps.insert(Capability::Commands);
        caps.insert(Capability::SystemPrompts);
        caps.insert(Capability::FileConfig);
        caps
    }
}

/// A capability adapter for GitHub Copilot.
pub struct CopilotAdapter;

impl CapabilityAdapter for CopilotAdapter {
    fn agent_id(&self) -> &str {
        "copilot"
    }

    fn supported_capabilities(&self) -> BTreeSet<Capability> {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::McpServers);
        caps.insert(Capability::SystemPrompts);
        caps.insert(Capability::FileConfig);
        caps.insert(Capability::DirectoryConfig);
        caps
    }
}

/// A capability adapter for Codex.
pub struct CodexAdapter;

impl CapabilityAdapter for CodexAdapter {
    fn agent_id(&self) -> &str {
        "codex"
    }

    fn supported_capabilities(&self) -> BTreeSet<Capability> {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::McpServers);
        caps.insert(Capability::Skills);
        caps.insert(Capability::Commands);
        caps.insert(Capability::SystemPrompts);
        caps.insert(Capability::FileConfig);
        caps.insert(Capability::EnvironmentVariables);
        caps.insert(Capability::ModelSelection);
        caps
    }
}

/// A capability adapter for Gemini CLI.
pub struct GeminiAdapter;

impl CapabilityAdapter for GeminiAdapter {
    fn agent_id(&self) -> &str {
        "gemini"
    }

    fn supported_capabilities(&self) -> BTreeSet<Capability> {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::McpServers);
        caps.insert(Capability::Skills);
        caps.insert(Capability::Commands);
        caps.insert(Capability::SystemPrompts);
        caps.insert(Capability::FileConfig);
        caps.insert(Capability::DirectoryConfig);
        caps
    }
}

/// A capability adapter for Cursor.
pub struct CursorAdapter;

impl CapabilityAdapter for CursorAdapter {
    fn agent_id(&self) -> &str {
        "cursor"
    }

    fn supported_capabilities(&self) -> BTreeSet<Capability> {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::McpServers);
        caps.insert(Capability::Skills);
        caps.insert(Capability::Rules);
        caps.insert(Capability::SystemPrompts);
        caps.insert(Capability::FileConfig);
        caps.insert(Capability::DirectoryConfig);
        caps
    }
}

/// A capability adapter for OpenCode.
pub struct OpenCodeAdapter;

impl CapabilityAdapter for OpenCodeAdapter {
    fn agent_id(&self) -> &str {
        "opencode"
    }

    fn supported_capabilities(&self) -> BTreeSet<Capability> {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::McpServers);
        caps.insert(Capability::Skills);
        caps.insert(Capability::Commands);
        caps.insert(Capability::SystemPrompts);
        caps.insert(Capability::FileConfig);
        caps
    }
}

/// A capability adapter for VS Code.
pub struct VsCodeAdapter;

impl CapabilityAdapter for VsCodeAdapter {
    fn agent_id(&self) -> &str {
        "vscode"
    }

    fn supported_capabilities(&self) -> BTreeSet<Capability> {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::McpServers);
        caps.insert(Capability::FileConfig);
        caps.insert(Capability::DirectoryConfig);
        caps
    }
}

/// A capability adapter for Windsurf.
pub struct WindsurfAdapter;

impl CapabilityAdapter for WindsurfAdapter {
    fn agent_id(&self) -> &str {
        "windsurf"
    }

    fn supported_capabilities(&self) -> BTreeSet<Capability> {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::McpServers);
        caps.insert(Capability::Rules);
        caps.insert(Capability::SystemPrompts);
        caps.insert(Capability::FileConfig);
        caps
    }
}

/// A capability adapter for Cline.
pub struct ClineAdapter;

impl CapabilityAdapter for ClineAdapter {
    fn agent_id(&self) -> &str {
        "cline"
    }

    fn supported_capabilities(&self) -> BTreeSet<Capability> {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::Rules);
        caps.insert(Capability::SystemPrompts);
        caps.insert(Capability::FileConfig);
        caps
    }
}

/// A capability adapter for Roo Code.
pub struct RooAdapter;

impl CapabilityAdapter for RooAdapter {
    fn agent_id(&self) -> &str {
        "roo"
    }

    fn supported_capabilities(&self) -> BTreeSet<Capability> {
        let mut caps = BTreeSet::new();
        caps.insert(Capability::McpServers);
        caps.insert(Capability::Skills);
        caps.insert(Capability::Rules);
        caps.insert(Capability::SystemPrompts);
        caps.insert(Capability::FileConfig);
        caps.insert(Capability::DirectoryConfig);
        caps
    }
}

/// Get a capability adapter for a given agent ID.
pub fn get_adapter(agent_id: &str) -> Option<Box<dyn CapabilityAdapter>> {
    match agent_id {
        "claude" => Some(Box::new(ClaudeAdapter)),
        "copilot" => Some(Box::new(CopilotAdapter)),
        "codex" => Some(Box::new(CodexAdapter)),
        "gemini" => Some(Box::new(GeminiAdapter)),
        "cursor" => Some(Box::new(CursorAdapter)),
        "opencode" => Some(Box::new(OpenCodeAdapter)),
        "vscode" => Some(Box::new(VsCodeAdapter)),
        "windsurf" => Some(Box::new(WindsurfAdapter)),
        "cline" => Some(Box::new(ClineAdapter)),
        "roo" => Some(Box::new(RooAdapter)),
        _ => None,
    }
}

/// Map capabilities from a source agent to a target agent.
///
/// Returns a `MappingResult` that describes which capabilities were mapped
/// successfully and which were lost.
pub fn map_capabilities(source_id: &str, target_id: &str) -> MappingResult {
    let source_adapter = get_adapter(source_id);
    let target_adapter = get_adapter(target_id);

    let mut result = MappingResult::new(source_id.to_string(), target_id.to_string());

    // If we don't have adapters for both agents, report all capabilities as unmapped
    let (source, target) = match (source_adapter, target_adapter) {
        (Some(s), Some(t)) => (s, t),
        (Some(source), None) => {
            for cap in source.supported_capabilities() {
                result.add_unmapped(
                    cap,
                    format!("No adapter available for target agent: {}", target_id),
                );
            }
            return result;
        }
        (None, _) => {
            result.warnings.push(format!(
                "No adapter available for source agent: {}",
                source_id
            ));
            return result;
        }
    };

    // Map each source capability to the target
    for cap in source.supported_capabilities() {
        if target.supports(&cap) {
            result.add_mapped(cap);
        } else {
            result.add_unmapped(
                cap.clone(),
                format!("Target agent '{}' does not support {:?}", target_id, cap),
            );
        }
    }

    result
}

/// Get all available agent IDs that have adapters.
pub fn available_agents() -> Vec<&'static str> {
    vec![
        "claude", "copilot", "codex", "gemini", "cursor", "opencode", "vscode", "windsurf",
        "cline", "roo",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claud_adapter_capabilities() {
        let adapter = ClaudeAdapter;
        assert_eq!(adapter.agent_id(), "claude");
        assert!(adapter.supports(&Capability::McpServers));
        assert!(adapter.supports(&Capability::Skills));
        assert!(adapter.supports(&Capability::Commands));
        assert!(!adapter.supports(&Capability::Rules));
    }

    #[test]
    fn test_codex_adapter_capabilities() {
        let adapter = CodexAdapter;
        assert_eq!(adapter.agent_id(), "codex");
        assert!(adapter.supports(&Capability::McpServers));
        assert!(adapter.supports(&Capability::Skills));
        assert!(adapter.supports(&Capability::EnvironmentVariables));
        assert!(adapter.supports(&Capability::ModelSelection));
    }

    #[test]
    fn test_cursor_adapter_capabilities() {
        let adapter = CursorAdapter;
        assert_eq!(adapter.agent_id(), "cursor");
        assert!(adapter.supports(&Capability::McpServers));
        assert!(adapter.supports(&Capability::Skills));
        assert!(adapter.supports(&Capability::Rules));
    }

    #[test]
    fn test_mapping_claud_to_codex() {
        let result = map_capabilities("claude", "codex");
        assert!(result.is_complete());
        assert_eq!(result.loss_count(), 0);
        assert_eq!(result.source, "claude");
        assert_eq!(result.target, "codex");
    }

    #[test]
    fn test_mapping_codex_to_claud() {
        // Codex has more capabilities than Claude, so there should be loss
        let result = map_capabilities("codex", "claude");
        assert!(!result.is_complete());
        assert!(result.loss_count() > 0);
    }

    #[test]
    fn test_mapping_unknown_agent() {
        let result = map_capabilities("unknown", "claude");
        // When source is unknown, there's nothing to map, so it's "complete" but with warnings
        assert!(result.warnings.iter().any(|w| w.contains("No adapter")));
        assert!(result.mapped.is_empty());
    }

    #[test]
    fn test_mapping_both_unknown() {
        let result = map_capabilities("unknown1", "unknown2");
        // When both are unknown, there's nothing to map
        assert!(result.warnings.iter().any(|w| w.contains("No adapter")));
        assert!(result.mapped.is_empty());
    }

    #[test]
    fn test_mapping_known_to_unknown() {
        let result = map_capabilities("codex", "unknown");
        // When target is unknown, all source capabilities are unmapped
        assert!(!result.is_complete());
        assert!(result.loss_count() > 0);
        assert!(result.warnings.iter().any(|w| w.contains("No adapter")));
    }

    #[test]
    fn test_get_adapter() {
        assert!(get_adapter("claude").is_some());
        assert!(get_adapter("codex").is_some());
        assert!(get_adapter("cursor").is_some());
        assert!(get_adapter("unknown").is_none());
    }

    #[test]
    fn test_available_agents() {
        let agents = available_agents();
        assert!(agents.contains(&"claude"));
        assert!(agents.contains(&"codex"));
        assert!(agents.contains(&"cursor"));
        assert!(agents.len() >= 10);
    }

    #[test]
    fn test_mapping_result_serialization() {
        let mut result = MappingResult::new("source".to_string(), "target".to_string());
        result.add_mapped(Capability::McpServers);
        result.add_unmapped(Capability::Skills, "Not supported".to_string());

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("mcp-servers"));
        assert!(json.contains("skills"));
    }

    #[test]
    fn test_round_trip_mapping() {
        // Map from codex to opencode and back
        let forward = map_capabilities("codex", "opencode");
        let backward = map_capabilities("opencode", "codex");

        // Forward mapping is valid
        let _ = forward.loss_count();

        // Backward should be complete (opencode is a subset of codex)
        assert!(backward.is_complete());
    }

    #[test]
    fn test_windsurf_adapter() {
        let adapter = WindsurfAdapter;
        assert_eq!(adapter.agent_id(), "windsurf");
        assert!(adapter.supports(&Capability::McpServers));
        assert!(adapter.supports(&Capability::Rules));
        assert!(!adapter.supports(&Capability::Skills));
    }

    #[test]
    fn test_cline_adapter() {
        let adapter = ClineAdapter;
        assert_eq!(adapter.agent_id(), "cline");
        assert!(adapter.supports(&Capability::Rules));
        assert!(!adapter.supports(&Capability::McpServers));
        assert!(!adapter.supports(&Capability::Skills));
    }

    #[test]
    fn test_roo_adapter() {
        let adapter = RooAdapter;
        assert_eq!(adapter.agent_id(), "roo");
        assert!(adapter.supports(&Capability::McpServers));
        assert!(adapter.supports(&Capability::Skills));
        assert!(adapter.supports(&Capability::Rules));
    }

    #[test]
    fn test_mapping_with_loss_reporting() {
        // Map from codex (full capabilities) to cline (limited)
        let result = map_capabilities("codex", "cline");

        // Should have significant loss
        assert!(!result.is_complete());
        assert!(result.loss_count() > 2);

        // Warnings should explain the loss
        for warning in &result.warnings {
            assert!(warning.contains("does not support"));
        }
    }
}
