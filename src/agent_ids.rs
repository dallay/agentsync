//! Shared agent identifier normalization and metadata helpers.
//!
//! This module centralizes alias handling so MCP parsing, filtering, and
//! gitignore pattern generation remain consistent.

/// Normalize a user-provided agent identifier to a canonical MCP ID.
///
/// Returns `Some(canonical)` for agents with native MCP generation support,
/// `None` for agents that use only configurable symlink targets.
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

/// Normalize a user-provided agent identifier to a canonical non-MCP agent ID.
///
/// This covers all agents that are supported via configurable symlink targets
/// but do not have native MCP generation. Returns a canonical string for use
/// in `known_ignore_patterns`.
fn canonical_configurable_agent_id(id: &str) -> Option<&'static str> {
    match id.to_lowercase().as_str() {
        // Rules-only agents with a unique rules file location
        "windsurf" => Some("windsurf"),
        "cline" => Some("cline"),
        "crush" => Some("crush"),
        "amp" => Some("amp"),
        "antigravity" => Some("antigravity"),
        "amazonq" | "amazon-q" | "amazon_q" | "amazon-q-cli" | "amazon_q_cli" => Some("amazonq"),
        "aider" => Some("aider"),
        "firebase" | "firebase-studio" | "firebase_studio" | "idx" => Some("firebase"),
        "openhands" | "open-hands" | "open_hands" => Some("openhands"),
        "junie" => Some("junie"),
        "augment" | "augmentcode" | "augment-code" | "augment_code" => Some("augment"),
        "kilocode" | "kilo-code" | "kilo_code" | "kilo" => Some("kilocode"),
        "goose" => Some("goose"),
        "qwen" | "qwen-code" | "qwen_code" => Some("qwen"),
        "roo" | "roocode" | "roo-code" | "roo_code" => Some("roo"),
        "zed" => Some("zed"),
        "trae" | "trae-ai" | "trae_ai" => Some("trae"),
        "warp" => Some("warp"),
        "kiro" => Some("kiro"),
        "firebender" | "fire-bender" | "fire_bender" => Some("firebender"),
        "factory" | "factory-droid" | "factory_droid" => Some("factory"),
        "vibe" | "mistral-vibe" | "mistral_vibe" | "mistralivibe" => Some("vibe"),
        "jetbrains" | "jetbrains-ai" | "jetbrains_ai" | "aiassistant" | "ai-assistant" => {
            Some("jetbrains")
        }
        "pi" | "pi-coding" | "pi_coding" | "pi-coding-agent" | "pi_coding_agent" => Some("pi"),
        "jules" => Some("jules"),
        _ => None,
    }
}

/// Known gitignore patterns for an agent identifier (canonical or alias).
///
/// Covers both native-MCP agents (via `canonical_mcp_agent_id`) and
/// configurable-only agents (via `canonical_configurable_agent_id`).
pub fn known_ignore_patterns(agent_name: &str) -> &'static [&'static str] {
    // Check native MCP agents first
    if let Some(canonical) = canonical_mcp_agent_id(agent_name) {
        return match canonical {
            "claude" => &[".mcp.json", ".claude/commands/", ".claude/skills/"],
            "copilot" => &[".vscode/mcp.json"],
            "codex" => &[".codex/config.toml"],
            "gemini" => &[
                "GEMINI.md",
                ".gemini/settings.json",
                ".gemini/commands/",
                ".gemini/skills/",
            ],
            "opencode" => &["opencode.json", ".opencode/command/", ".opencode/skill/"],
            "cursor" => &[".cursor/mcp.json", ".cursor/skills/"],
            "vscode" => &[".vscode/mcp.json"],
            _ => &[],
        };
    }

    // Check configurable agents
    match canonical_configurable_agent_id(agent_name) {
        Some("windsurf") => &[".windsurf/mcp_config.json"],
        Some("cline") => &[".clinerules"],
        Some("crush") => &["CRUSH.md", ".crush.json"],
        Some("amp") => &[],
        Some("antigravity") => &[".agent/rules/", ".agent/skills/"],
        Some("amazonq") => &[".amazonq/rules/", ".amazonq/mcp.json"],
        Some("aider") => &[".aider.conf.yml", ".aiderignore"],
        Some("firebase") => &[".idx/airules.md", ".idx/mcp.json"],
        Some("openhands") => &[".openhands/microagents/"],
        Some("junie") => &[".junie/"],
        Some("augment") => &[".augment/rules/"],
        Some("kilocode") => &[".kilocode/mcp.json"],
        Some("goose") => &[".goosehints"],
        Some("qwen") => &[".qwen/settings.json"],
        Some("roo") => &[".roo/mcp.json", ".roo/rules/", ".roo/skills/"],
        Some("zed") => &[".zed/settings.json"],
        Some("trae") => &[".trae/rules/"],
        Some("warp") => &["WARP.md"],
        Some("kiro") => &[".kiro/steering/", ".kiro/settings/mcp.json"],
        Some("firebender") => &["firebender.json"],
        Some("factory") => &[".factory/mcp.json", ".factory/skills/"],
        Some("vibe") => &[".vibe/config.toml", ".vibe/skills/"],
        Some("jetbrains") => &[".aiassistant/rules/"],
        Some("pi") | Some("jules") => &[],
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

/// Canonicalize an agent identifier across both MCP-native and configurable agents.
///
/// Returns `Some(canonical)` if the ID is recognized by either registry,
/// `None` otherwise.
fn canonical_any_agent_id(id: &str) -> Option<&'static str> {
    canonical_mcp_agent_id(id).or_else(|| canonical_configurable_agent_id(id))
}

/// Match a configured agent name against a sync filter token.
///
/// Canonicalizes both inputs across MCP-native and configurable-agent aliases.
/// When both sides resolve to a canonical ID, performs exact canonical
/// matching. Otherwise falls back to legacy case-insensitive substring
/// comparison against the configured name.
pub fn sync_filter_matches(config_agent_name: &str, filter: &str) -> bool {
    if let Some(canonical_filter) = canonical_any_agent_id(filter) {
        if let Some(canonical_agent) = canonical_any_agent_id(config_agent_name) {
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

    #[test]
    fn test_sync_filter_matches_configurable_agent_aliases() {
        // Both sides resolve via canonical_configurable_agent_id
        assert!(sync_filter_matches("roocode", "roo-code"));
        assert!(sync_filter_matches("roo-code", "roocode"));
        assert!(sync_filter_matches("amazon-q", "amazonq"));
        assert!(sync_filter_matches("augmentcode", "augment-code"));
        assert!(sync_filter_matches("trae-ai", "trae"));
        // Configurable agent on one side, alias on the other
        assert!(sync_filter_matches("kilo-code", "kilocode"));
        // Should NOT match unrelated agents
        assert!(!sync_filter_matches("roo", "cline"));
        assert!(!sync_filter_matches("amazonq", "augment"));
    }

    // -------------------------------------------------------------------------
    // canonical_configurable_agent_id aliases
    // -------------------------------------------------------------------------

    #[test]
    fn test_canonical_configurable_agent_id_aliases() {
        assert_eq!(canonical_configurable_agent_id("amazon-q"), Some("amazonq"));
        assert_eq!(
            canonical_configurable_agent_id("amazon_q_cli"),
            Some("amazonq")
        );
        assert_eq!(
            canonical_configurable_agent_id("firebase-studio"),
            Some("firebase")
        );
        assert_eq!(canonical_configurable_agent_id("idx"), Some("firebase"));
        assert_eq!(
            canonical_configurable_agent_id("open-hands"),
            Some("openhands")
        );
        assert_eq!(
            canonical_configurable_agent_id("augmentcode"),
            Some("augment")
        );
        assert_eq!(
            canonical_configurable_agent_id("kilo-code"),
            Some("kilocode")
        );
        assert_eq!(canonical_configurable_agent_id("roocode"), Some("roo"));
        assert_eq!(canonical_configurable_agent_id("trae-ai"), Some("trae"));
        assert_eq!(
            canonical_configurable_agent_id("jetbrains-ai"),
            Some("jetbrains")
        );
        assert_eq!(
            canonical_configurable_agent_id("aiassistant"),
            Some("jetbrains")
        );
        assert_eq!(
            canonical_configurable_agent_id("pi-coding-agent"),
            Some("pi")
        );
        assert_eq!(
            canonical_configurable_agent_id("factory-droid"),
            Some("factory")
        );
        assert_eq!(
            canonical_configurable_agent_id("mistral-vibe"),
            Some("vibe")
        );
        assert_eq!(canonical_configurable_agent_id("qwen-code"), Some("qwen"));
        assert_eq!(canonical_configurable_agent_id("unknown-xyz"), None);
    }

    // -------------------------------------------------------------------------
    // known_ignore_patterns for configurable agents
    // -------------------------------------------------------------------------

    #[test]
    fn test_known_ignore_patterns_windsurf() {
        let patterns = known_ignore_patterns("windsurf");
        assert!(patterns.contains(&".windsurf/mcp_config.json"));
    }

    #[test]
    fn test_known_ignore_patterns_cline() {
        let patterns = known_ignore_patterns("cline");
        assert!(patterns.contains(&".clinerules"));
    }

    #[test]
    fn test_known_ignore_patterns_amazonq() {
        let patterns = known_ignore_patterns("amazonq");
        assert!(patterns.contains(&".amazonq/rules/"));
        assert!(patterns.contains(&".amazonq/mcp.json"));
        // Alias should return same patterns
        assert_eq!(
            known_ignore_patterns("amazon-q"),
            known_ignore_patterns("amazonq")
        );
    }

    #[test]
    fn test_known_ignore_patterns_aider() {
        let patterns = known_ignore_patterns("aider");
        assert!(patterns.contains(&".aider.conf.yml"));
        assert!(patterns.contains(&".aiderignore"));
    }

    #[test]
    fn test_known_ignore_patterns_firebase() {
        let patterns = known_ignore_patterns("firebase");
        assert!(patterns.contains(&".idx/airules.md"));
        assert!(patterns.contains(&".idx/mcp.json"));
        // IDX alias
        assert_eq!(
            known_ignore_patterns("idx"),
            known_ignore_patterns("firebase")
        );
    }

    #[test]
    fn test_known_ignore_patterns_openhands() {
        let patterns = known_ignore_patterns("openhands");
        assert!(patterns.contains(&".openhands/microagents/"));
    }

    #[test]
    fn test_known_ignore_patterns_augment() {
        let patterns = known_ignore_patterns("augment");
        assert!(patterns.contains(&".augment/rules/"));
        assert_eq!(
            known_ignore_patterns("augmentcode"),
            known_ignore_patterns("augment")
        );
    }

    #[test]
    fn test_known_ignore_patterns_kilocode() {
        let patterns = known_ignore_patterns("kilocode");
        assert!(patterns.contains(&".kilocode/mcp.json"));
        assert_eq!(
            known_ignore_patterns("kilo-code"),
            known_ignore_patterns("kilocode")
        );
    }

    #[test]
    fn test_known_ignore_patterns_goose() {
        let patterns = known_ignore_patterns("goose");
        assert!(patterns.contains(&".goosehints"));
    }

    #[test]
    fn test_known_ignore_patterns_roo() {
        let patterns = known_ignore_patterns("roo");
        assert!(patterns.contains(&".roo/mcp.json"));
        assert!(patterns.contains(&".roo/rules/"));
        assert!(patterns.contains(&".roo/skills/"));
        assert_eq!(
            known_ignore_patterns("roocode"),
            known_ignore_patterns("roo")
        );
    }

    #[test]
    fn test_known_ignore_patterns_trae() {
        let patterns = known_ignore_patterns("trae");
        assert!(patterns.contains(&".trae/rules/"));
        assert_eq!(
            known_ignore_patterns("trae-ai"),
            known_ignore_patterns("trae")
        );
    }

    #[test]
    fn test_known_ignore_patterns_kiro() {
        let patterns = known_ignore_patterns("kiro");
        assert!(patterns.contains(&".kiro/steering/"));
        assert!(patterns.contains(&".kiro/settings/mcp.json"));
    }

    #[test]
    fn test_known_ignore_patterns_firebender() {
        let patterns = known_ignore_patterns("firebender");
        assert!(patterns.contains(&"firebender.json"));
    }

    #[test]
    fn test_known_ignore_patterns_factory() {
        let patterns = known_ignore_patterns("factory");
        assert!(patterns.contains(&".factory/mcp.json"));
        assert!(patterns.contains(&".factory/skills/"));
        assert_eq!(
            known_ignore_patterns("factory-droid"),
            known_ignore_patterns("factory")
        );
    }

    #[test]
    fn test_known_ignore_patterns_vibe() {
        let patterns = known_ignore_patterns("vibe");
        assert!(patterns.contains(&".vibe/config.toml"));
        assert!(patterns.contains(&".vibe/skills/"));
        assert_eq!(
            known_ignore_patterns("mistral-vibe"),
            known_ignore_patterns("vibe")
        );
    }

    #[test]
    fn test_known_ignore_patterns_jetbrains() {
        let patterns = known_ignore_patterns("jetbrains");
        assert!(patterns.contains(&".aiassistant/rules/"));
        assert_eq!(
            known_ignore_patterns("aiassistant"),
            known_ignore_patterns("jetbrains")
        );
    }

    #[test]
    fn test_known_ignore_patterns_crush() {
        let patterns = known_ignore_patterns("crush");
        assert!(patterns.contains(&"CRUSH.md"));
    }

    #[test]
    fn test_known_ignore_patterns_warp() {
        let patterns = known_ignore_patterns("warp");
        assert!(patterns.contains(&"WARP.md"));
    }

    #[test]
    fn test_known_ignore_patterns_opencode_dirs() {
        let patterns = known_ignore_patterns("opencode");
        assert!(patterns.contains(&"opencode.json"));
        assert!(patterns.contains(&".opencode/command/"));
        assert!(patterns.contains(&".opencode/skill/"));
    }

    #[test]
    fn test_known_ignore_patterns_unknown_returns_empty() {
        let patterns = known_ignore_patterns("nonexistent-agent-xyz");
        assert!(patterns.is_empty());
    }
}
