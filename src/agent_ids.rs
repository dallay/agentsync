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
        // New agents
        "amp" => Some("amp"),
        "antigravity" => Some("antigravity"),
        "augment" => Some("augment"),
        "openclaw" | "open-claw" | "clawdbot" | "moltbot" => Some("openclaw"),
        "cline" => Some("cline"),
        "codebuddy" => Some("codebuddy"),
        "command-code" | "commandcode" => Some("command-code"),
        "continue" => Some("continue"),
        "cortex" => Some("cortex"),
        "crush" => Some("crush"),
        "droid" | "factory" => Some("droid"),
        "goose" => Some("goose"),
        "junie" => Some("junie"),
        "iflow" | "iflow-cli" => Some("iflow"),
        "kilo" => Some("kilo"),
        "kimi" | "kimi-cli" => Some("kimi"),
        "kiro" | "kiro-cli" => Some("kiro"),
        "kode" => Some("kode"),
        "mcpjam" => Some("mcpjam"),
        "vibe" | "mistral-vibe" => Some("vibe"),
        "mux" => Some("mux"),
        "openhands" => Some("openhands"),
        "pi" => Some("pi"),
        "qoder" => Some("qoder"),
        "qwen" | "qwen-code" => Some("qwen"),
        "replit" => Some("replit"),
        "roo" | "roocode" | "roo-code" => Some("roo"),
        "trae" => Some("trae"),
        "trae-cn" => Some("trae-cn"),
        "windsurf" => Some("windsurf"),
        "zencoder" => Some("zencoder"),
        "neovate" => Some("neovate"),
        "pochi" => Some("pochi"),
        "adal" => Some("adal"),
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
        // New agents
        Some("amp") => &[".mcp.json", ".agents/skills/"],
        Some("antigravity") => &[".mcp.json", ".agent/skills/"],
        Some("augment") => &[".augment/skills/"],
        Some("openclaw") => &["skills/"],
        Some("cline") => &[".mcp.json", ".cline/skills/"],
        Some("codebuddy") => &[".codebuddy/skills/"],
        Some("command-code") => &[".commandcode/skills/"],
        Some("continue") => &[".continue/config.json", ".continue/skills/"],
        Some("cortex") => &[".cortex/skills/"],
        Some("crush") => &[".crush/skills/"],
        Some("droid") => &[".factory/skills/"],
        Some("goose") => &[".goose/config.yaml", ".goose/skills/"],
        Some("junie") => &[".junie/skills/"],
        Some("iflow") => &[".iflow/skills/"],
        Some("kilo") => &[".kilocode/skills/"],
        Some("kimi") => &[".agents/skills/"],
        Some("kiro") => &[".kiro/skills/"],
        Some("kode") => &[".kode/skills/"],
        Some("mcpjam") => &[".mcpjam/skills/"],
        Some("vibe") => &[".vibe/skills/"],
        Some("mux") => &[".mux/skills/"],
        Some("openhands") => &[".openhands/skills/"],
        Some("pi") => &[".pi/skills/"],
        Some("qoder") => &[".qoder/skills/"],
        Some("qwen") => &[".qwen/skills/"],
        Some("replit") => &[".replit", ".agents/skills/"],
        Some("roo") => &[".mcp.json", ".roo/skills/"],
        Some("trae") => &[".trae/mcp_config.json", ".trae/skills/"],
        Some("trae-cn") => &[".trae/mcp_config.json", ".trae/skills/"],
        Some("windsurf") => &[".windsurf/mcp_config.json", ".windsurf/skills/"],
        Some("zencoder") => &[".zencoder/skills/"],
        Some("neovate") => &[".neovate/skills/"],
        Some("pochi") => &[".pochi/skills/"],
        Some("adal") => &[".adal/skills/"],
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
        assert_eq!(canonical_mcp_agent_id("cline"), Some("cline"));
        assert_eq!(canonical_mcp_agent_id("roo-code"), Some("roo"));
        assert_eq!(canonical_mcp_agent_id("windsurf"), Some("windsurf"));
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
        assert!(mcp_filter_matches("cline", "cline"));
    }

    #[test]
    fn test_sync_filter_matches_alias_and_substring() {
        assert!(sync_filter_matches("codex", "codex-cli"));
        assert!(sync_filter_matches("codex-cli", "codex"));
        assert!(sync_filter_matches("custom-copilot-helper", "pilot"));
        assert!(!sync_filter_matches("custom-copilot-helper", "codex-cli"));
        assert!(sync_filter_matches("cline", "cline"));
    }
}
