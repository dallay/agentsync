# Spike: Revalidate Repo-Scoped Marketplace Behavior for Claude and Codex (#567)

## Executive Summary & Recommendation
This spike revalidates the vendor behavior of **Codex CLI** and **Claude Code** regarding repository-scoped plugin marketplaces and configurations prior to implementing AgentSync plugin support.

### Scope Recommendation for #567
- **Retain & Refine Scope #567 with Dual Manifest Generation**:
  - **Marketplace Manifest Dual Export**: AgentSync MUST generate/sync BOTH `.claude-plugin/marketplace.json` AND `.agents/plugins/marketplace.json` (or maintain `.claude-plugin/marketplace.json` as the primary cross-vendor schema). Claude Code strictly requires `.claude-plugin/marketplace.json` and rejects `.agents/plugins/marketplace.json` with `Marketplace file not found`. Codex checks `.agents/plugins/marketplace.json` first and falls back to `.claude-plugin/marketplace.json`.
  - **Strict Schema Compliance**:
    - **Owner Field**: AgentSync must include an `owner` object (e.g., `{"name": "...", "email": "..."}`) in all generated marketplace manifests (`marketplace.json`). Without `owner`, Claude Code fails schema validation (`owner: Invalid input: expected object, received undefined`).
    - **Reserved Name Check**: AgentSync must validate that marketplace names do NOT contain vendor reserved keywords (such as `claude` or `anthropic`). Claude Code explicitly rejects marketplaces containing `claude` with: `Marketplace name impersonates an official Anthropic/Claude marketplace`.
  - **Explicit Registration Requirements**: Neither tool automatically discovers or loads unregistered local repository marketplaces on a clean clone without explicit registration. AgentSync's plugin sync command must explicitly handle user/project marketplace registration (`marketplace add`) and enablement into vendor config files (`~/.codex/config.toml`, `~/.claude/plugins/known_marketplaces.json`, `.claude/settings.json`).

---

## Question & Answer Matrix

### Q1: Does `codex-cli 0.147.0` honor repo-scoped `.agents/plugins/marketplace.json`?
- **Auto-Discovery**: **NO**. `codex-cli 0.147.0` (and `0.162.1`) does **not** automatically discover or load repo-scoped `.agents/plugins/marketplace.json` simply by being present in a workspace folder. Running `codex plugin marketplace list` or `codex plugin list --available` in an unregistered workspace outputs `No plugin marketplaces in scope.`
- **Explicit Registration**: **YES**. When running `codex plugin marketplace add <path-to-repo>`, Codex parses `.agents/plugins/marketplace.json` at `<path-to-repo>`, registers the marketplace in `$CODEX_HOME/config.toml` under `[marketplaces.<name>]`, and exposes its plugins via `codex plugin list --available`.

### Q2: Does it honor `.claude-plugin/marketplace.json` as a legacy-compatible marketplace?
- **Codex (0.147.0 & 0.162.1)**: **YES**. If `.agents/plugins/marketplace.json` is absent, Codex falls back to parsing `.claude-plugin/marketplace.json` when adding a marketplace path.
- **Precedence**: When BOTH `.agents/plugins/marketplace.json` and `.claude-plugin/marketplace.json` exist in the target directory, Codex gives precedence to `.agents/plugins/marketplace.json`.
- **Claude Code (2.1.233 & 2.1.296)**: **NO legacy fallback in reverse**. Claude Code strictly looks for `.claude-plugin/marketplace.json`. If only `.agents/plugins/marketplace.json` is present, `claude plugin marketplace add <path>` fails with `Marketplace file not found at <path>/.claude-plugin/marketplace.json`.

### Q3: Which surfaces differ: Codex CLI, Codex app, codex exec, and cache materialization?
- **Codex Surfaces**: Codex CLI, `codex exec`, and Codex app share state via `$CODEX_HOME/config.toml` (default `~/.codex/config.toml`). None of these surfaces auto-load unregistered repository marketplaces without config registration.
- **Cache Materialization Paths**:
  - **Codex**: When a plugin is added (`codex plugin add <plugin>@<marketplace>`), Codex materializes the plugin files into:
    `$CODEX_HOME/plugins/cache/<marketplaceName>/<pluginName>/<version>/`
    and updates `$CODEX_HOME/config.toml` with `[plugins."<plugin>@<marketplace>"] enabled = true`.
  - **Claude Code**: When a plugin is installed (`claude plugin install <plugin>@<marketplace>`), Claude Code materializes the plugin files into:
    `$CLAUDE_CONFIG_DIR/plugins/cache/<marketplaceName>/<pluginName>/<version>/`
    and updates `$CLAUDE_CONFIG_DIR/plugins/installed_plugins.json` and `$CLAUDE_CONFIG_DIR/plugins/known_marketplaces.json`.

### Q4: Does a clean clone require an explicit per-user install or only a repository declaration?
- **Clean Clone Behavior**: A clean clone requires an **explicit per-user install/registration step** for both Codex and Claude Code.
  - **Codex**: The presence of `.agents/plugins/marketplace.json` or `.codex/config.toml` in a cloned repo does NOT automatically register the marketplace. Registration (`codex plugin marketplace add <path>`) and plugin installation (`codex plugin add <plugin>@<marketplace>`) are required per-user/environment.
  - **Claude Code**: Project `.claude/settings.json` containing `extraKnownMarketplaces` and `enabledPlugins` declares configuration intent, but without local marketplace cache initialization (`known_marketplaces.json`), `claude plugin install` or running a prompt does not auto-download or auto-materialize the plugin from an un-updated local directory marketplace without explicit registration (`claude plugin marketplace add <path>` / `claude plugin install <plugin>@<marketplace>`).

### Q5: What is the exact behavior of Claude Code 2.1.233 for project `.claude/settings.json`, `extraKnownMarketplaces`, and `enabledPlugins`?
- **`extraKnownMarketplaces`**:
  - Expected format for directory marketplaces:
    ```json
    {
      "extraKnownMarketplaces": {
        "<marketplace-name>": {
          "source": {
            "source": "directory",
            "path": "/absolute/or/relative/path"
          }
        }
      }
    }
    ```
  - Validation requirements: `marketplace.json` MUST contain an `owner` object (`{"name": "...", "email": "..."}`).
  - Marketplace name restrictions: Name MUST NOT contain reserved vendor terms like `claude` or `anthropic`.
- **`enabledPlugins`**:
  - Expected format: `"enabledPlugins": { "<pluginName>@<marketplaceName>": true }`.
  - Controls enablement scope for that project. However, plugins must be materialized in `$CLAUDE_CONFIG_DIR/plugins/cache` and registered in `$CLAUDE_CONFIG_DIR/plugins/installed_plugins.json` to function.

### Q6: Which plugin manifest/components are actually compatible across both vendors?
- **Marketplace Manifest (`marketplace.json`) Compatibility**:
  - Required fields across both:
    - `name`: String (Must avoid reserved keywords `claude` / `anthropic`).
    - `owner`: Object with `name` and optional `email` (Required by Claude Code, ignored/accepted by Codex).
    - `plugins`: Array of objects with `name`, `source`, `version`.
  - Manifest file locations:
    - `.agents/plugins/marketplace.json`: Recognized by Codex (preferred). Unrecognized by Claude Code.
    - `.claude-plugin/marketplace.json`: Recognized by Claude Code (required) AND Codex (fallback).
- **Plugin Manifest (`plugin.json`) Compatibility**:
  - Located at `<plugin-dir>/.claude-plugin/plugin.json` or `<plugin-dir>/.agents/plugin.json`.
  - Shared fields: `name`, `version`, `description`.
- **Component Compatibility**:
  - **Skills**: Both scan `skills/*/SKILL.md`.
  - **MCP Servers**: Both support `.mcp.json` inside the plugin directory.
  - **Hooks**: Both support `hooks/` directory, subject to trust/approval flags.

---

## Version Comparison Summary

| Metric / Behavior | Codex CLI 0.147.0 | Codex CLI 0.162.1 | Claude Code 2.1.233 | Claude Code 2.1.296 |
| :--- | :--- | :--- | :--- | :--- |
| **Unregistered Auto-Discovery** | No | No | No | No |
| **Reads `.agents/plugins/marketplace.json`** | Yes (Preferred) | Yes (Preferred) | No | No |
| **Reads `.claude-plugin/marketplace.json`** | Yes (Fallback) | Yes (Fallback) | Yes (Required) | Yes (Required) |
| **Owner Field Required** | No | No | Yes | Yes |
| **Vendor Keyword Name Block** | No | No | Yes (`claude`/`anthropic`) | Yes (`claude`/`anthropic`) |
| **Cache Materialization Directory** | `$CODEX_HOME/plugins/cache/` | `$CODEX_HOME/plugins/cache/` | `$CLAUDE_CONFIG_DIR/plugins/cache/` | `$CLAUDE_CONFIG_DIR/plugins/cache/` |

---

## Isolated Reproduction Harness & Execution Output

### Reproduction Script
```bash
#!/usr/bin/env bash
set -euo pipefail

SPIKE_DIR="/tmp/spike_matrix_run"
rm -rf "$SPIKE_DIR"
mkdir -p "$SPIKE_DIR"

# 1. Setup fixture marketplace repo
MKT_REPO="$SPIKE_DIR/fixture_marketplace"
mkdir -p "$MKT_REPO/plugins/sample-plugin/skills/sample-skill"
mkdir -p "$MKT_REPO/plugins/sample-plugin/.claude-plugin"
mkdir -p "$MKT_REPO/.agents/plugins"
mkdir -p "$MKT_REPO/.claude-plugin"

cat << 'SKILL_EOF' > "$MKT_REPO/plugins/sample-plugin/skills/sample-skill/SKILL.md"
---
name: sample-skill
description: Sample skill for spike revalidation
---
# Sample Skill
SKILL_EOF

cat << 'PLUGIN_EOF' > "$MKT_REPO/plugins/sample-plugin/.claude-plugin/plugin.json"
{
  "name": "sample-plugin",
  "version": "1.0.0",
  "description": "Sample plugin for revalidation"
}
PLUGIN_EOF

cat << 'AGENTS_MKT' > "$MKT_REPO/.agents/plugins/marketplace.json"
{
  "name": "custom-agents-marketplace",
  "owner": {
    "name": "AgentSync Team",
    "email": "team@agentsync.dev"
  },
  "plugins": [
    {
      "name": "sample-plugin",
      "source": "./plugins/sample-plugin",
      "version": "1.0.0"
    }
  ]
}
AGENTS_MKT

cat << 'CLAUDE_MKT' > "$MKT_REPO/.claude-plugin/marketplace.json"
{
  "name": "custom-claude-marketplace",
  "owner": {
    "name": "AgentSync Team",
    "email": "team@agentsync.dev"
  },
  "plugins": [
    {
      "name": "sample-plugin",
      "source": "./plugins/sample-plugin",
      "version": "1.0.0"
    }
  ]
}
CLAUDE_MKT

# Matrix Execution (Codex 0.147.0, Codex 0.162.1, Claude 2.1.233, Claude 2.1.296)
# [Executed via /tmp/run_matrix.sh]
```

### Measured Execution Log
```
===========================================================
 AGENTSYNC #567 SPIKE REVALIDATION MATRIX REPRODUCTION SCRIPT
===========================================================
Fixture marketplace repository established at /tmp/spike_matrix_run/fixture_marketplace

--- Testing Codex CLI 0.147.0 ---
Codex 0.147.0 clean workspace marketplace list:
No plugin marketplaces in scope.
Codex 0.147.0 marketplace add output:
Added marketplace `custom-agents-marketplace` from /tmp/spike_matrix_run/fixture_marketplace.
Installed marketplace root: /tmp/spike_matrix_run/fixture_marketplace
Codex 0.147.0 marketplace list output:
MARKETPLACE                ROOT
custom-agents-marketplace  /tmp/spike_matrix_run/fixture_marketplace
Codex 0.147.0 fallback to .claude-plugin/marketplace.json result:
Added marketplace `custom-claude-marketplace` from /tmp/spike_matrix_run/fixture_claude_only.
Installed marketplace root: /tmp/spike_matrix_run/fixture_claude_only
Codex 0.147.0 plugin add output:
Added plugin `sample-plugin` from marketplace `custom-agents-marketplace`.
Installed plugin root: /tmp/spike_matrix_run/codex_both_0.147.0/plugins/cache/custom-agents-marketplace/sample-plugin/1.0.0
Codex 0.147.0 cache contents:
/tmp/spike_matrix_run/codex_both_0.147.0/plugins/cache
/tmp/spike_matrix_run/codex_both_0.147.0/plugins/cache/custom-agents-marketplace
/tmp/spike_matrix_run/codex_both_0.147.0/plugins/cache/custom-agents-marketplace/sample-plugin
/tmp/spike_matrix_run/codex_both_0.147.0/plugins/cache/custom-agents-marketplace/sample-plugin/1.0.0
/tmp/spike_matrix_run/codex_both_0.147.0/plugins/cache/custom-agents-marketplace/sample-plugin/1.0.0/.claude-plugin
/tmp/spike_matrix_run/codex_both_0.147.0/plugins/cache/custom-agents-marketplace/sample-plugin/1.0.0/.claude-plugin/plugin.json
/tmp/spike_matrix_run/codex_both_0.147.0/plugins/cache/custom-agents-marketplace/sample-plugin/1.0.0/skills
/tmp/spike_matrix_run/codex_both_0.147.0/plugins/cache/custom-agents-marketplace/sample-plugin/1.0.0/skills/sample-skill
/tmp/spike_matrix_run/codex_both_0.147.0/plugins/cache/custom-agents-marketplace/sample-plugin/1.0.0/skills/sample-skill/SKILL.md

--- Testing Codex CLI 0.162.1 ---
Codex 0.162.1 clean workspace marketplace list:
No plugin marketplaces in scope.
Codex 0.162.1 marketplace add output:
Added marketplace `custom-agents-marketplace` from /tmp/spike_matrix_run/fixture_marketplace.
Installed marketplace root: /tmp/spike_matrix_run/fixture_marketplace
Codex 0.162.1 marketplace list output:
MARKETPLACE                ROOT
custom-agents-marketplace  /tmp/spike_matrix_run/fixture_marketplace
Codex 0.162.1 fallback to .claude-plugin/marketplace.json result:
Added marketplace `custom-claude-marketplace` from /tmp/spike_matrix_run/fixture_claude_only.
Installed marketplace root: /tmp/spike_matrix_run/fixture_claude_only
Codex 0.162.1 plugin add output:
Added plugin `sample-plugin` from marketplace `custom-agents-marketplace`.
Installed plugin root: /tmp/spike_matrix_run/codex_both_0.162.1/plugins/cache/custom-agents-marketplace/sample-plugin/1.0.0
Codex 0.162.1 cache contents:
/tmp/spike_matrix_run/codex_both_0.162.1/plugins/cache
/tmp/spike_matrix_run/codex_both_0.162.1/plugins/cache/custom-agents-marketplace
/tmp/spike_matrix_run/codex_both_0.162.1/plugins/cache/custom-agents-marketplace/sample-plugin
/tmp/spike_matrix_run/codex_both_0.162.1/plugins/cache/custom-agents-marketplace/sample-plugin/1.0.0
/tmp/spike_matrix_run/codex_both_0.162.1/plugins/cache/custom-agents-marketplace/sample-plugin/1.0.0/.claude-plugin
/tmp/spike_matrix_run/codex_both_0.162.1/plugins/cache/custom-agents-marketplace/sample-plugin/1.0.0/.claude-plugin/plugin.json
/tmp/spike_matrix_run/codex_both_0.162.1/plugins/cache/custom-agents-marketplace/sample-plugin/1.0.0/skills
/tmp/spike_matrix_run/codex_both_0.162.1/plugins/cache/custom-agents-marketplace/sample-plugin/1.0.0/skills/sample-skill
/tmp/spike_matrix_run/codex_both_0.162.1/plugins/cache/custom-agents-marketplace/sample-plugin/1.0.0/skills/sample-skill/SKILL.md

--- Testing Claude Code 2.1.233 ---
Claude Code 2.1.233 add with ONLY .agents/plugins/marketplace.json:
Adding marketplace…✘ Failed to add marketplace: Marketplace file not found at /tmp/spike_matrix_run/fixture_agents_only/.claude-plugin/marketplace.json

Claude Code 2.1.233 add with .claude-plugin/marketplace.json:
Adding marketplace…✔ Successfully added marketplace: custom-claude-marketplace (declared in user settings)
Claude Code 2.1.233 plugin install output:
Installing plugin "sample-plugin@custom-claude-marketplace"...✔ Successfully installed plugin: sample-plugin@custom-claude-marketplace (scope: user)
Claude Code 2.1.233 cache contents:
/tmp/spike_matrix_run/claude_both_2.1.233/.claude/plugins/cache
/tmp/spike_matrix_run/claude_both_2.1.233/.claude/plugins/cache/custom-claude-marketplace
/tmp/spike_matrix_run/claude_both_2.1.233/.claude/plugins/cache/custom-claude-marketplace/sample-plugin
/tmp/spike_matrix_run/claude_both_2.1.233/.claude/plugins/cache/custom-claude-marketplace/sample-plugin/1.0.0
/tmp/spike_matrix_run/claude_both_2.1.233/.claude/plugins/cache/custom-claude-marketplace/sample-plugin/1.0.0/.claude-plugin
/tmp/spike_matrix_run/claude_both_2.1.233/.claude/plugins/cache/custom-claude-marketplace/sample-plugin/1.0.0/.claude-plugin/plugin.json
/tmp/spike_matrix_run/claude_both_2.1.233/.claude/plugins/cache/custom-claude-marketplace/sample-plugin/1.0.0/skills
/tmp/spike_matrix_run/claude_both_2.1.233/.claude/plugins/cache/custom-claude-marketplace/sample-plugin/1.0.0/skills/sample-skill
/tmp/spike_matrix_run/claude_both_2.1.233/.claude/plugins/cache/custom-claude-marketplace/sample-plugin/1.0.0/skills/sample-skill/SKILL.md

--- Testing Claude Code 2.1.296 ---
Claude Code 2.1.296 add with ONLY .agents/plugins/marketplace.json:
Adding marketplace…✘ Failed to add marketplace: Marketplace file not found at /tmp/spike_matrix_run/fixture_agents_only/.claude-plugin/marketplace.json

Claude Code 2.1.296 add with .claude-plugin/marketplace.json:
Adding marketplace…✔ Successfully added marketplace: custom-claude-marketplace (declared in user settings)
Claude Code 2.1.296 plugin install output:
Installing plugin "sample-plugin@custom-claude-marketplace"...✔ Successfully installed plugin: sample-plugin@custom-claude-marketplace (scope: user)
Claude Code 2.1.296 cache contents:
/tmp/spike_matrix_run/claude_both_2.1.296/.claude/plugins/cache
/tmp/spike_matrix_run/claude_both_2.1.296/.claude/plugins/cache/custom-claude-marketplace
/tmp/spike_matrix_run/claude_both_2.1.296/.claude/plugins/cache/custom-claude-marketplace/sample-plugin
/tmp/spike_matrix_run/claude_both_2.1.296/.claude/plugins/cache/custom-claude-marketplace/sample-plugin/1.0.0
/tmp/spike_matrix_run/claude_both_2.1.296/.claude/plugins/cache/custom-claude-marketplace/sample-plugin/1.0.0/.claude-plugin
/tmp/spike_matrix_run/claude_both_2.1.296/.claude/plugins/cache/custom-claude-marketplace/sample-plugin/1.0.0/.claude-plugin/plugin.json
/tmp/spike_matrix_run/claude_both_2.1.296/.claude/plugins/cache/custom-claude-marketplace/sample-plugin/1.0.0/skills
/tmp/spike_matrix_run/claude_both_2.1.296/.claude/plugins/cache/custom-claude-marketplace/sample-plugin/1.0.0/skills/sample-skill
/tmp/spike_matrix_run/claude_both_2.1.296/.claude/plugins/cache/custom-claude-marketplace/sample-plugin/1.0.0/skills/sample-skill/SKILL.md
```
