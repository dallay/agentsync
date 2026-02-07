//! Shared agent identifier normalization and metadata helpers.
//!
//! This module centralizes alias handling so MCP parsing, filtering, and
//! gitignore pattern generation remain consistent.

/// Normalize a user-provided agent identifier to a canonical MCP ID.
pub fn canonical_mcp_agent_id(id: &str) -> Option<&'static str> {
    match id.to_lowercase().as_str() {
        "claude" | "claude-code" | "claude_code" => Some("claude"),
        "copilot" | "github-copilot" | "github_copilot" => Some("copilot"),
        "codex" | "codex-cli" | "codex_cli" => Some("codex"),
        "gemini" | "gemini-cli" | "gemini_cli" => Some("gemini"),
        "vscode" | "vs-code" | "vs_code" => Some("vscode"),
        "cursor" => Some("cursor"),
        "opencode" | "open-code" | "open_code" => Some("opencode"),
        _ => None,
    }
}

/// Known gitignore patterns for an agent identifier (canonical or alias).
pub fn known_ignore_patterns(agent_name: &str) -> &'static [&'static str] {
    match canonical_mcp_agent_id(agent_name) {
        Some("claude") => &[".mcp.json", ".claude/commands/", ".claude/skills/"],
        Some("copilot") => &[".vscode/mcp.json"],
        Some("codex") => &[".codex/config.toml"],
        Some("gemini") => &[
            "GEMINI.md",
            ".gemini/settings.json",
            ".gemini/commands/",
            ".gemini/skills/",
        ],
        Some("opencode") => &["opencode.json"],
        Some("cursor") => &[".cursor/mcp.json", ".cursor/skills/"],
        Some("vscode") => &[".vscode/mcp.json"],
        _ => &[],
    }
}

/// Match a canonical agent ID against a filter token.
///
/// If `filter` is a known alias/canonical ID, this performs exact canonical
/// matching. Otherwise it falls back to legacy case-insensitive substring
/// matching against the canonical ID.
pub fn mcp_filter_matches(agent_id: &str, filter: &str) -> bool {
    if let Some(canonical_filter) = canonical_mcp_agent_id(filter) {
        canonical_filter == agent_id
    } else {
        let filter_lower = filter.to_lowercase();
        agent_id.to_lowercase().contains(&filter_lower)
    }
}

/// Match a configured agent name against a sync filter token.
///
/// If `filter` is a known alias/canonical ID and the configured agent is also
/// known, this performs exact canonical matching. Otherwise it falls back to
/// legacy case-insensitive substring matching against the configured name.
pub fn sync_filter_matches(config_agent_name: &str, filter: &str) -> bool {
    if let Some(canonical_filter) = canonical_mcp_agent_id(filter) {
        if let Some(canonical_agent) = canonical_mcp_agent_id(config_agent_name) {
            canonical_agent == canonical_filter
        } else {
            let filter_lower = filter.to_lowercase();
            config_agent_name.to_lowercase().contains(&filter_lower)
        }
    } else {
        let filter_lower = filter.to_lowercase();
        config_agent_name.to_lowercase().contains(&filter_lower)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_mcp_agent_id_aliases() {
        assert_eq!(canonical_mcp_agent_id("claude"), Some("claude"));
        assert_eq!(canonical_mcp_agent_id("claude-code"), Some("claude"));
        assert_eq!(canonical_mcp_agent_id("github-copilot"), Some("copilot"));
        assert_eq!(canonical_mcp_agent_id("codex_cli"), Some("codex"));
        assert_eq!(canonical_mcp_agent_id("gemini-cli"), Some("gemini"));
        assert_eq!(canonical_mcp_agent_id("vs-code"), Some("vscode"));
        assert_eq!(canonical_mcp_agent_id("open-code"), Some("opencode"));
        assert_eq!(canonical_mcp_agent_id("unknown"), None);
    }

    #[test]
    fn test_known_ignore_patterns_uses_aliases() {
        assert_eq!(
            known_ignore_patterns("codex"),
            known_ignore_patterns("codex-cli")
        );
        assert_eq!(
            known_ignore_patterns("vscode"),
            known_ignore_patterns("vs-code")
        );
    }

    #[test]
    fn test_mcp_filter_matches_alias_and_substring() {
        assert!(mcp_filter_matches("codex", "codex-cli"));
        assert!(mcp_filter_matches("copilot", "pilot"));
        assert!(!mcp_filter_matches("codex", "gemini-cli"));
    }

    #[test]
    fn test_sync_filter_matches_alias_and_substring() {
        assert!(sync_filter_matches("codex", "codex-cli"));
        assert!(sync_filter_matches("codex-cli", "codex"));
        assert!(sync_filter_matches("custom-copilot-helper", "pilot"));
        assert!(!sync_filter_matches("custom-copilot-helper", "codex-cli"));
    }
}
