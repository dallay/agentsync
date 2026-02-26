//! Shared agent identifier normalization and metadata helpers.
//!
//! This module centralizes alias handling so MCP parsing, filtering, and
//! gitignore pattern generation remain consistent.

/// Normalize a user-provided agent identifier to a canonical MCP ID.
pub fn canonical_mcp_agent_id(id: &str) -> Option<&'static str> {
    let normalized = id.replace('_', "-").to_lowercase();
    match normalized.as_str() {
        "claude" | "claude-code" => Some("claude"),
        "copilot" | "github-copilot" => Some("copilot"),
        "codex" | "codex-cli" => Some("codex"),
        "gemini" | "gemini-cli" => Some("gemini"),
        "vscode" | "vs-code" => Some("vscode"),
        "cursor" => Some("cursor"),
        "opencode" | "open-code" => Some("opencode"),
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
        Some("copilot") => &[".vscode/mcp.json", ".mcp.json"],
        Some("codex") => &[".codex/config.toml"],
        Some("gemini") => &[
            "GEMINI.md",
            ".gemini/settings.json",
            ".gemini/commands/",
            ".gemini/skills/",
        ],
        Some("opencode") => &["opencode.json"],
        Some("cursor") => &[".cursor/mcp.json", ".cursor/skills/", ".mcp.json"],
        Some("vscode") => &[".vscode/mcp.json", ".mcp.json"],
        // New agents
        Some("amp") => &[".mcp.json", ".agents/skills/"],
        Some("antigravity") => &[".mcp.json", ".agent/skills/"],
        Some("augment") => &[".mcp.json", ".augment/skills/"],
        Some("openclaw") => &[".mcp.json", "skills/openclaw/"],
        Some("cline") => &[".mcp.json", ".cline/skills/"],
        Some("codebuddy") => &[".mcp.json", ".codebuddy/skills/"],
        Some("command-code") => &[".mcp.json", ".commandcode/skills/"],
        Some("continue") => &[".continue/config.json", ".continue/skills/"],
        Some("cortex") => &[".mcp.json", ".cortex/skills/"],
        Some("crush") => &[".mcp.json", ".crush/skills/"],
        Some("droid") => &[".mcp.json", ".factory/skills/"],
        Some("goose") => &[".goose/config.yaml", ".goose/skills/"],
        Some("junie") => &[".mcp.json", ".junie/skills/"],
        Some("iflow") => &[".mcp.json", ".iflow/skills/"],
        Some("kilo") => &[".mcp.json", ".kilocode/skills/"],
        Some("kimi") => &[".mcp.json", ".agents/skills/"],
        Some("kiro") => &[".mcp.json", ".kiro/skills/"],
        Some("kode") => &[".mcp.json", ".kode/skills/"],
        Some("mcpjam") => &[".mcp.json", ".mcpjam/skills/"],
        Some("vibe") => &[".mcp.json", ".vibe/skills/"],
        Some("mux") => &[".mcp.json", ".mux/skills/"],
        Some("openhands") => &[".mcp.json", ".openhands/skills/"],
        Some("pi") => &[".mcp.json", ".pi/skills/"],
        Some("qoder") => &[".mcp.json", ".qoder/skills/"],
        Some("qwen") => &[".mcp.json", ".qwen/skills/"],
        Some("replit") => &[".mcp.json", ".replit", ".agents/skills/"],
        Some("roo") => &[".mcp.json", ".roo/skills/"],
        Some("trae") => &[".trae/mcp_config.json", ".trae/skills/"],
        Some("trae-cn") => &[".trae/mcp_config.json", ".trae/skills/"],
        Some("windsurf") => &[".windsurf/mcp_config.json", ".windsurf/skills/"],
        Some("zencoder") => &[".mcp.json", ".zencoder/skills/"],
        Some("neovate") => &[".mcp.json", ".neovate/skills/"],
        Some("pochi") => &[".mcp.json", ".pochi/skills/"],
        Some("adal") => &[".mcp.json", ".adal/skills/"],
        _ => &[],
    }
}

/// Match a canonical agent ID against a filter token.
///
/// If `filter` is a known alias/canonical ID, this performs exact canonical
/// matching. Otherwise it falls back to legacy case-insensitive substring
/// matching against the canonical ID.
pub fn mcp_filter_matches(agent_id: &str, filter: &str) -> bool {
    let filter_normalized = filter.replace('_', "-").to_lowercase();
    if let Some(canonical_filter) = canonical_mcp_agent_id(&filter_normalized) {
        canonical_filter == agent_id
    } else {
        agent_id.to_lowercase().contains(&filter_normalized)
    }
}

/// Match a configured agent name against a sync filter token.
///
/// If `filter` is a known alias/canonical ID and the configured agent is also
/// known, this performs exact canonical matching. Otherwise it falls back to
/// legacy case-insensitive substring matching against the configured name.
pub fn sync_filter_matches(config_agent_name: &str, filter: &str) -> bool {
    let filter_normalized = filter.replace('_', "-").to_lowercase();
    let config_normalized = config_agent_name.replace('_', "-").to_lowercase();
    if let Some(canonical_filter) = canonical_mcp_agent_id(&filter_normalized) {
        if let Some(canonical_agent) = canonical_mcp_agent_id(&config_normalized) {
            canonical_agent == canonical_filter
        } else {
            config_normalized.contains(&filter_normalized)
        }
    } else {
        config_normalized.contains(&filter_normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalization_table() {
        let cases = [
            ("claude", "claude"),
            ("claude-code", "claude"),
            ("claude_code", "claude"),
            ("github_copilot", "copilot"),
            ("codex_cli", "codex"),
            ("gemini_cli", "gemini"),
            ("vs_code", "vscode"),
            ("open_code", "opencode"),
            ("open-claw", "openclaw"),
            ("clawdbot", "openclaw"),
            ("moltbot", "openclaw"),
            ("roo-code", "roo"),
            ("roo_code", "roo"),
            ("iflow_cli", "iflow"),
            ("trae_cn", "trae-cn"),
        ];

        for (input, expected) in cases {
            assert_eq!(
                canonical_mcp_agent_id(input),
                Some(expected),
                "Failed to normalize '{}'",
                input
            );
        }
    }

    #[test]
    fn test_all_known_ignore_patterns_non_empty() {
        let agents = [
            "claude",
            "copilot",
            "codex",
            "gemini",
            "vscode",
            "cursor",
            "opencode",
            "amp",
            "antigravity",
            "augment",
            "openclaw",
            "cline",
            "codebuddy",
            "command-code",
            "continue",
            "cortex",
            "crush",
            "droid",
            "goose",
            "junie",
            "iflow",
            "kilo",
            "kimi",
            "kiro",
            "kode",
            "mcpjam",
            "vibe",
            "mux",
            "openhands",
            "pi",
            "qoder",
            "qwen",
            "replit",
            "roo",
            "trae",
            "trae-cn",
            "windsurf",
            "zencoder",
            "neovate",
            "pochi",
            "adal",
        ];

        for agent in agents {
            let patterns = known_ignore_patterns(agent);
            assert!(
                !patterns.is_empty(),
                "Agent '{}' has empty ignore patterns",
                agent
            );

            // Check for .mcp.json if applicable
            let needs_mcp = !matches!(
                agent,
                "codex"
                    | "gemini"
                    | "opencode"
                    | "goose"
                    | "trae"
                    | "trae-cn"
                    | "windsurf"
                    | "continue"
            );
            if needs_mcp {
                assert!(
                    patterns.contains(&".mcp.json"),
                    "Agent '{}' missing .mcp.json in ignore patterns",
                    agent
                );
            }
        }
    }

    #[test]
    fn test_filter_matches_table() {
        let cases = [
            ("codex", "codex-cli", true),
            ("codex", "codex_cli", true),
            ("copilot", "pilot", true),
            ("copilot", "github_copilot", true),
            ("openclaw", "clawdbot", true),
            ("openclaw", "moltbot", true),
            ("roo", "roo_code", true),
            ("roo", "roo-code", true),
            ("trae-cn", "trae_cn", true),
        ];

        for (agent_id, filter, expected) in cases {
            assert_eq!(
                mcp_filter_matches(agent_id, filter),
                expected,
                "mcp_filter_matches failed for agent '{}' filter '{}'",
                agent_id,
                filter
            );
        }
    }

    #[test]
    fn test_sync_filter_matches_table() {
        let cases = [
            ("codex-cli", "codex", true),
            ("codex_cli", "codex", true),
            ("moltbot", "openclaw", true),
            ("custom-copilot-helper", "pilot", true),
            ("custom_copilot_helper", "copilot", true),
            ("roo-code", "roocode", true),
        ];

        for (config_name, filter, expected) in cases {
            assert_eq!(
                sync_filter_matches(config_name, filter),
                expected,
                "sync_filter_matches failed for config '{}' filter '{}'",
                config_name,
                filter
            );
        }
    }
}
