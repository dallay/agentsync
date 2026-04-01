# Specification: MCP Configuration Generation

**Type**: RETROSPEC  
**Date**: 2026-04-01  
**Status**: RETROSPEC  
**Source of Truth**: `src/mcp.rs`, `src/config.rs`, `src/linker.rs` (sync_mcp)

## Purpose

Define the behavior of the MCP (Model Context Protocol) configuration generation module which
produces agent-specific MCP server configuration files from a centralized `[mcp_servers]` definition
in `agentsync.toml`. This spec covers supported agents, per-agent output formats, merge strategies,
disabled server handling, server transport types, dry-run behavior, deduplication, idempotency,
error
handling, and CLI/default agent filtering.

This is a **retrospec** — every requirement and scenario is traced to existing code behavior in
`src/mcp.rs`, `src/config.rs`, and `src/linker.rs`, and verified by existing tests.

---

## Data Model

### McpGlobalConfig

Global MCP settings from the `[mcp]` section in `agentsync.toml`:

| Field            | Type               | Default | Description                                   |
|------------------|--------------------|---------|-----------------------------------------------|
| `enabled`        | `bool`             | `true`  | Enable/disable MCP propagation globally       |
| `merge_strategy` | `McpMergeStrategy` | `Merge` | How to handle existing agent MCP config files |

**Code ref**: `src/config.rs:222–239`

### McpMergeStrategy

| Variant     | Serialized As | Description                                          |
|-------------|---------------|------------------------------------------------------|
| `Merge`     | `"merge"`     | Merge new servers into existing config (default)     |
| `Overwrite` | `"overwrite"` | Replace MCP section; some agents preserve other keys |

**Code ref**: `src/config.rs:244–250`

### McpServerConfig

Configuration for a single MCP server, defined under `[mcp_servers.<name>]`:

| Field            | Type                      | Required | Default | Description                                |
|------------------|---------------------------|----------|---------|--------------------------------------------|
| `command`        | `Option<String>`          | No*      | `None`  | Command to execute (stdio transport)       |
| `args`           | `Vec<String>`             | No       | `[]`    | Arguments for the command                  |
| `env`            | `BTreeMap<String,String>` | No       | `{}`    | Environment variables                      |
| `url`            | `Option<String>`          | No*      | `None`  | URL for HTTP/SSE transport                 |
| `headers`        | `BTreeMap<String,String>` | No       | `{}`    | HTTP headers (for remote servers)          |
| `transport_type` | `Option<String>`          | No       | `None`  | Transport type (`stdio`, `http`, `sse`)    |
| `disabled`       | `bool`                    | No       | `false` | Whether the server is excluded from output |

\* A server typically has either `command` (stdio) or `url` (remote), but neither is strictly
required at the type level.

**Serialization rules** (via `#[serde(skip_serializing_if)]`):

- `command`: omitted when `None`
- `args`: omitted when empty
- `env`: omitted when empty
- `url`: omitted when `None`
- `headers`: omitted when empty
- `transport_type`: serialized as `"type"` in JSON; omitted when `None`
- `disabled`: omitted when `false`

**Code ref**: `src/config.rs:254–282`

### McpAgent Enum

Supported MCP-compatible agents:

| Variant         | ID           | Human Name       | Config Path             | Format |
|-----------------|--------------|------------------|-------------------------|--------|
| `ClaudeCode`    | `"claude"`   | Claude Code      | `.mcp.json`             | JSON   |
| `GithubCopilot` | `"copilot"`  | GitHub Copilot   | `.vscode/mcp.json`      | JSON   |
| `CodexCli`      | `"codex"`    | OpenAI Codex CLI | `.codex/config.toml`    | TOML   |
| `GeminiCli`     | `"gemini"`   | Gemini CLI       | `.gemini/settings.json` | JSON   |
| `VsCode`        | `"vscode"`   | VS Code          | `.vscode/mcp.json`      | JSON   |
| `Cursor`        | `"cursor"`   | Cursor           | `.cursor/mcp.json`      | JSON   |
| `OpenCode`      | `"opencode"` | OpenCode         | `opencode.json`         | JSON   |

Note: `GithubCopilot` and `VsCode` share the same config path (`.vscode/mcp.json`).

**Code ref**: `src/mcp.rs:56–151`

### McpSyncResult

Counters returned from MCP sync operations:

| Field     | Type    | Description                              |
|-----------|---------|------------------------------------------|
| `created` | `usize` | New MCP config files created             |
| `updated` | `usize` | Existing MCP config files updated        |
| `skipped` | `usize` | Agents skipped (no change or no servers) |
| `errors`  | `usize` | Agents that failed processing            |

**Code ref**: `src/mcp.rs:1027–1032`

### Agent-Specific Output Formats

| Agent         | Root Key      | Server Key    | Extra Fields per Server        | Preserves Other Keys |
|---------------|---------------|---------------|--------------------------------|----------------------|
| ClaudeCode    | `mcpServers`  | standard JSON | —                              | No                   |
| GithubCopilot | `mcpServers`  | standard JSON | —                              | No                   |
| VsCode        | `mcpServers`  | standard JSON | —                              | No                   |
| Cursor        | `mcpServers`  | standard JSON | —                              | No                   |
| GeminiCli     | `mcpServers`  | standard JSON | `trust: true` added per server | Yes                  |
| CodexCli      | `mcp_servers` | TOML tables   | `http_headers` (not `headers`) | Yes                  |
| OpenCode      | `mcp`         | OpenCode JSON | `type`, `command[]`, `enabled` | Yes                  |

**Code ref**: `src/mcp.rs:302–902`

---

## Requirements

### REQ-MCP-001: Global MCP Enable/Disable

The system MUST skip all MCP config generation when `mcp.enabled = false` in `agentsync.toml`.

The system MUST default `mcp.enabled` to `true` when the `[mcp]` section is absent.

#### Scenario: SC-MCP-001a — MCP disabled returns empty result

- GIVEN a config with `[mcp] enabled = false` and at least one `[mcp_servers]` entry
- WHEN `sync_mcp()` is called
- THEN `McpSyncResult` MUST have all counters at 0
- AND no MCP config files MUST be created

**Test ref**: `linker.rs::test_sync_mcp_disabled_returns_empty`

#### Scenario: SC-MCP-001b — MCP defaults to enabled

- GIVEN a config with no `[mcp]` section
- WHEN the config is parsed
- THEN `mcp.enabled` MUST be `true`
- AND `mcp.merge_strategy` MUST be `Merge`

**Test ref**: `config.rs::test_mcp_config_defaults`

---

### REQ-MCP-002: No Servers Returns Early

The system MUST return an empty `McpSyncResult` when no `[mcp_servers]` entries are defined,
regardless of whether MCP is enabled.

#### Scenario: SC-MCP-002a — No servers defined

- GIVEN a config with `[mcp] enabled = true` and no `[mcp_servers]` entries
- WHEN `sync_mcp()` is called
- THEN `McpSyncResult.created` MUST be 0
- AND no MCP config files MUST be created

**Test ref**: `linker.rs::test_sync_mcp_no_servers_returns_empty`

---

### REQ-MCP-003: Agent Scope — Only Configured Agents

The system MUST only generate MCP config files for agents that are both explicitly configured under
`[agents.<name>]` in `agentsync.toml` AND have `enabled = true`.

Agents that are MCP-capable but not configured MUST NOT receive MCP config files.

If no agents are configured at all, the system MUST return an empty result even if MCP servers are
defined.

#### Scenario: SC-MCP-003a — Only configured agents get MCP configs

- GIVEN a config with `agents.claude` and `agents.copilot` enabled
- AND MCP server `filesystem` defined
- WHEN `sync_mcp()` is called
- THEN `.mcp.json` MUST be created (for Claude)
- AND `.vscode/mcp.json` MUST be created (for Copilot)
- AND `.cursor/mcp.json` MUST NOT exist
- AND `.gemini/settings.json` MUST NOT exist
- AND `opencode.json` MUST NOT exist
- AND `.codex/config.toml` MUST NOT exist

**Test ref**: `linker.rs::test_sync_mcp_only_creates_for_configured_agents`

#### Scenario: SC-MCP-003b — No agents configured returns empty

- GIVEN a config with `[mcp] enabled = true` and `[mcp_servers.filesystem]` defined
- AND no `[agents]` section
- WHEN `sync_mcp()` is called
- THEN all MCP counters MUST be 0
- AND no MCP config files MUST be created

**Test ref**: `linker.rs::test_sync_mcp_no_agents_configured_returns_empty`

#### Scenario: SC-MCP-003c — Disabled agents excluded

- GIVEN an agent with `enabled = false`
- WHEN `get_enabled_agents_from_config()` is called
- THEN that agent MUST NOT appear in the returned list

**Test ref**: `mcp.rs::test_get_enabled_agents_from_config`

#### Scenario: SC-MCP-003d — Unknown agent names ignored

- GIVEN an agent name that does not map to any `McpAgent` variant (e.g. `"unknown_agent"`)
- WHEN `get_enabled_agents_from_config()` is called
- THEN that agent MUST be silently ignored

**Test ref**: `mcp.rs::test_get_enabled_agents_from_config`

---

### REQ-MCP-004: Agent ID Resolution and Aliases

The system MUST resolve agent identifiers through `canonical_mcp_agent_id()`, supporting both
canonical IDs and common aliases.

Supported aliases (non-exhaustive, derived from `agent_ids.rs`):

| Input(s)                          | Resolves To |
|-----------------------------------|-------------|
| `claude`, `CLAUDE`, `claude-code` | `claude`    |
| `copilot`, `github-copilot`       | `copilot`   |
| `codex`, `codex-cli`              | `codex`     |
| `gemini`                          | `gemini`    |
| `vscode`, `vs-code`               | `vscode`    |
| `cursor`                          | `cursor`    |
| `opencode`, `open-code`           | `opencode`  |

Resolution is case-insensitive.

#### Scenario: SC-MCP-004a — Agent ID aliases resolve correctly

- GIVEN the input string `"claude-code"`
- WHEN `McpAgent::from_id()` is called
- THEN it MUST return `Some(McpAgent::ClaudeCode)`

- GIVEN the input string `"CLAUDE"`
- WHEN `McpAgent::from_id()` is called
- THEN it MUST return `Some(McpAgent::ClaudeCode)`

- GIVEN the input string `"unknown"`
- WHEN `McpAgent::from_id()` is called
- THEN it MUST return `None`

**Test ref**: `mcp.rs::test_agent_from_id`

---

### REQ-MCP-005: CLI Agent Filtering

The system MUST support filtering MCP generation to specific agents via CLI `--agents` argument.

The filter MUST support both canonical IDs and aliases (e.g. `"codex-cli"` matches the Codex agent).

When no CLI filter is provided but `default_agents` is configured, the system MUST use
`default_agents` as the filter.

When neither CLI filter nor `default_agents` is set, all enabled agents receive MCP configs.

#### Scenario: SC-MCP-005a — CLI filter with alias

- GIVEN a config with `agents.codex-cli` enabled and MCP servers defined
- WHEN `sync_mcp()` is called with filter `["codex-cli"]`
- THEN `.codex/config.toml` MUST be created
- AND `McpSyncResult.created` MUST be 1

**Test ref**: `linker.rs::test_sync_mcp_cli_filter_supports_aliases`

#### Scenario: SC-MCP-005b — default_agents filter with alias

- GIVEN a config with `default_agents = ["codex-cli"]` and `agents.codex-cli` enabled
- WHEN `sync_mcp()` is called with no CLI filter
- THEN `.codex/config.toml` MUST be created
- AND `McpSyncResult.created` MUST be 1

**Test ref**: `linker.rs::test_sync_mcp_default_agents_support_aliases`

---

### REQ-MCP-006: Disabled Servers Excluded

The system MUST exclude servers with `disabled = true` from all generated MCP config files.

If all servers are disabled, the agent MUST be skipped (counted as `skipped`).

#### Scenario: SC-MCP-006a — All servers disabled skips agent

- GIVEN a config with one MCP server that has `disabled = true`
- AND a configured agent
- WHEN `generate_for_agent()` is called
- THEN `McpSyncResult.skipped` MUST be 1
- AND no config file MUST be created

**Test ref**: `mcp.rs::test_generator_skips_disabled_servers`

#### Scenario: SC-MCP-006b — McpOutput filters disabled servers

- GIVEN an `McpOutput` with one enabled and one disabled server
- WHEN `enabled_servers()` is called
- THEN only the enabled server MUST be returned

**Test ref**: `mcp.rs::test_mcp_output_enabled_servers`

---

### REQ-MCP-007: Standard JSON Format (Claude, Copilot, VS Code, Cursor)

The system MUST produce JSON files with the structure `{ "mcpServers": { "<name>": { ... } } }` for
Claude Code, GitHub Copilot, VS Code, and Cursor agents.

Server entries MUST include only non-empty fields per the serialization skip rules.

Server names MUST be ordered deterministically (alphabetical via BTreeMap).

#### Scenario: SC-MCP-007a — Claude Code basic JSON output

- GIVEN a server `filesystem` with `command = "npx"` and args
- WHEN the ClaudeCode formatter produces output
- THEN the JSON MUST have `mcpServers.filesystem` with the correct `command` and `args`

**Test ref**: `mcp.rs::test_claude_formatter_basic`

#### Scenario: SC-MCP-007b — Deterministic server ordering

- GIVEN servers named `"zeta"`, `"alpha"`, `"mid"`
- WHEN the ClaudeCode formatter produces output
- THEN the `mcpServers` keys MUST appear in order: `["alpha", "mid", "zeta"]`

**Test ref**: `mcp.rs::test_claude_formatter_orders_servers_deterministically`

#### Scenario: SC-MCP-007c — Stdio server JSON serialization

- GIVEN a server with `command = "npx"`, `args = ["-y", "server"]`, `env = { DEBUG: "true" }`,
  `type = "stdio"`
- WHEN `server_to_json()` is called
- THEN the JSON MUST include `command`, `args`, `env`, and `type`
- AND `url` and `headers` MUST NOT be present (they are empty/None)

**Test ref**: `mcp.rs::test_server_to_json_stdio`

#### Scenario: SC-MCP-007d — HTTP server JSON serialization

- GIVEN a server with `url = "https://api.example.com"` and
  `headers = { Authorization: "Bearer token" }`
- AND no `command` or `args`
- WHEN `server_to_json()` is called
- THEN the JSON MUST include `url` and `headers`
- AND `command` and `args` MUST NOT be present

**Test ref**: `mcp.rs::test_server_to_json_http`

#### Scenario: SC-MCP-007e — Env and headers are alphabetically ordered

- GIVEN a server with env keys `["ZZZ", "AAA"]` and header keys `["X-Zebra", "X-Alpha"]`
- WHEN `server_to_json()` is called
- THEN env keys MUST appear in order `["AAA", "ZZZ"]`
- AND header keys MUST appear in order `["X-Alpha", "X-Zebra"]`

**Test ref**: `mcp.rs::test_server_to_json_orders_env_and_headers`

---

### REQ-MCP-008: Gemini CLI Format

The system MUST produce JSON with `{ "mcpServers": { ... } }` structure for Gemini CLI.

Each server entry MUST include an additional `trust: true` field for non-interactive execution.

The formatter MUST preserve other top-level settings (e.g. `theme`, `someOtherSetting`) in the
Gemini settings file during both merge and overwrite operations.

#### Scenario: SC-MCP-008a — Gemini adds trust field

- GIVEN a server `filesystem` with `command = "npx"` and args
- WHEN the GeminiCli formatter produces output
- THEN `mcpServers.filesystem.trust` MUST be `true`

**Test ref**: `mcp.rs::test_gemini_formatter_adds_trust`

#### Scenario: SC-MCP-008b — Gemini merge preserves other settings

- GIVEN an existing `.gemini/settings.json` with `theme: "dark"` and `someOtherSetting: true`
- WHEN new servers are merged
- THEN `theme` and `someOtherSetting` MUST be preserved in the output
- AND both existing and new servers MUST be present under `mcpServers`

**Test ref**: `mcp.rs::test_gemini_formatter_preserves_other_settings`

#### Scenario: SC-MCP-008c — Gemini overwrite preserves non-MCP settings

- GIVEN an existing Gemini config with `theme: "dark"` and an old server
- WHEN overwrite strategy is used
- THEN `theme` MUST be preserved
- AND the old server MUST be replaced with the new server
- AND the new server MUST have `trust: true`

**Test ref**: `mcp.rs::test_generator_overwrite_strategy_gemini`

---

### REQ-MCP-009: Codex CLI Format (TOML)

The system MUST produce TOML files with `[mcp_servers.<name>]` table structure for Codex CLI.

The Codex formatter MUST:

- Serialize `command` as a TOML string
- Serialize `args` as a TOML array of strings
- Serialize `env` as a nested TOML table
- Serialize `headers` as `http_headers` (Codex-specific field name)
- NOT include a `type` field in TOML output
- Preserve other top-level settings (e.g. `model`) during merge and overwrite operations

#### Scenario: SC-MCP-009a — Codex basic TOML output

- GIVEN a server `filesystem` with `command = "npx"` and args
- WHEN the CodexCli formatter's `format_to_string()` is called
- THEN the output MUST be valid TOML
- AND `mcp_servers.filesystem` MUST exist with correct values

**Test ref**: `mcp.rs::test_codex_formatter_format_to_string`

#### Scenario: SC-MCP-009b — Codex uses http_headers and omits type

- GIVEN a remote server with `url`, `headers`, and `transport_type`
- WHEN the CodexCli formatter serializes it
- THEN the TOML MUST contain `http_headers` (not `headers`)
- AND `type` MUST NOT be present

**Test ref**: `mcp.rs::test_codex_formatter_uses_http_headers_and_omits_type`

#### Scenario: SC-MCP-009c — Codex merge preserves other settings

- GIVEN an existing `.codex/config.toml` with `model = "gpt-5-codex"` and an existing server
- WHEN new servers are merged
- THEN `model` MUST be preserved
- AND both existing and new servers MUST be present

**Test ref**: `mcp.rs::test_codex_formatter_merge_preserves_other_settings`

#### Scenario: SC-MCP-009d — Codex merge overrides same-name server

- GIVEN an existing server `filesystem` with `command = "old-command"`
- WHEN a new `filesystem` server with `command = "npx"` is merged
- THEN the server's `command` MUST be `"npx"` (new value)

**Test ref**: `mcp.rs::test_codex_formatter_merge_override`

#### Scenario: SC-MCP-009e — Codex cleanup removes absent servers

- GIVEN an existing config with servers `keep` and `remove`
- WHEN cleanup is called with only server `keep` in the new config
- THEN `keep` MUST be present in output
- AND `remove` MUST NOT be present

**Test ref**: `mcp.rs::test_codex_formatter_cleanup_removed_servers`

---

### REQ-MCP-010: OpenCode Format

The system MUST produce JSON with `{ "$schema": "https://opencode.ai/config.json", "mcp": { ... } }`
structure for OpenCode.

Each server entry MUST use the OpenCode-specific format:

- Stdio servers: `type = "local"`, `command` as an array (command + args combined), `enabled` field
- Remote servers: `type = "remote"`, `url`, optional `headers`

The `enabled` field MUST be set to `!disabled` (i.e. `true` for enabled servers).

The formatter MUST preserve other top-level settings during merge and overwrite operations.

The formatter MUST add `$schema` if missing during merge operations.

#### Scenario: SC-MCP-010a — OpenCode basic output structure

- GIVEN a stdio server `filesystem`
- WHEN the OpenCode formatter produces output
- THEN the JSON MUST have `$schema` set to `"https://opencode.ai/config.json"`
- AND `mcp.filesystem.type` MUST be `"local"`
- AND `mcp.filesystem.command` MUST be an array (command + args)
- AND `mcp.filesystem.enabled` MUST be `true`

**Test ref**: `mcp.rs::test_opencode_formatter_basic`

#### Scenario: SC-MCP-010b — OpenCode merge preserves other settings

- GIVEN an existing `opencode.json` with `tools: { "some-tool": true }` and an existing server
- WHEN new servers are merged
- THEN `tools` MUST be preserved
- AND both existing and new servers MUST be present under `mcp`

**Test ref**: `mcp.rs::test_opencode_formatter_preserves_other_settings`

#### Scenario: SC-MCP-010c — OpenCode merge orders deterministically

- GIVEN existing servers `"zeta"` and `"alpha"`, and a new server `"mid"`
- WHEN merge is performed
- THEN `mcp` keys MUST appear in order: `["alpha", "mid", "zeta"]`

**Test ref**: `mcp.rs::test_opencode_formatter_merge_orders_servers_deterministically`

#### Scenario: SC-MCP-010d — OpenCode cleanup removes absent servers

- GIVEN an existing config with servers `keep` and `remove` and a `tools` setting
- WHEN cleanup is called with only `keep` in the new config
- THEN `keep` MUST be present, `remove` MUST NOT
- AND `tools` MUST be preserved

**Test ref**: `mcp.rs::test_cleanup_removed_servers_opencode`

#### Scenario: SC-MCP-010e — OpenCode overwrite preserves non-MCP keys

- GIVEN an existing `opencode.json` with `theme: "dark"` and an old server
- WHEN overwrite strategy is used
- THEN `theme` MUST be preserved
- AND the old server MUST be replaced with the new server

**Test ref**: `mcp.rs::test_generator_overwrite_strategy_opencode`

---

### REQ-MCP-011: Merge Strategy

When `merge_strategy = "merge"` (default), the system MUST merge new servers into existing MCP
config files, preserving servers already present in the file.

New servers with the same name as existing servers MUST override the existing server entry.

#### Scenario: SC-MCP-011a — Merge adds new servers alongside existing

- GIVEN an existing `.mcp.json` with server `existing`
- AND a new config defining server `filesystem`
- WHEN `generate_for_agent()` is called with Merge strategy
- THEN the output MUST contain both `existing` and `filesystem` servers
- AND `McpSyncResult.updated` MUST be 1

**Test ref**: `mcp.rs::test_generator_merge_strategy`

#### Scenario: SC-MCP-011b — Merge overrides same-name server

- GIVEN an existing `.mcp.json` with `filesystem` having `command = "old-command"`
- AND new config defining `filesystem` with `command = "npx"`
- WHEN merge is performed
- THEN `filesystem.command` MUST be `"npx"`

**Test ref**: `mcp.rs::test_claude_formatter_merge_override`

---

### REQ-MCP-012: Merge Cleanup — Removed Server Detection

When using Merge strategy, the system MUST detect servers present in the existing config file but
absent from the new config, and remove them.

The cleanup logic uses a count-based heuristic: cleanup is triggered when the number of existing
servers differs from the number of new enabled servers. When counts are equal but names differ,
a simple merge is performed (retaining existing entries).

#### Scenario: SC-MCP-012a — Removed servers are cleaned up

- GIVEN an existing config with servers `server1`, `server2`, `server3`
- AND new config defines only `server1` and `server3` (server2 removed)
- WHEN `generate_for_agent()` is called with Merge strategy
- THEN `server1` and `server3` MUST be present with updated values
- AND `server2` MUST NOT be present

**Test ref**: `mcp.rs::test_generator_merge_removal_cleanup`

#### Scenario: SC-MCP-012b — No removal needed

- GIVEN an existing config with server `keep_this`
- AND new config also defines `keep_this` with updated command
- WHEN merge is performed
- THEN `keep_this` MUST be present with the new command value

**Test ref**: `mcp.rs::test_generator_merge_no_removal_needed`

#### Scenario: SC-MCP-012c — All servers replaced (complete turnover)

- GIVEN an existing config with servers `old1`, `old2`, `old3`
- AND new config defines completely different servers `new1`, `new2`
- WHEN merge cleanup is performed
- THEN `new1` and `new2` MUST be present
- AND `old1`, `old2`, `old3` MUST NOT be present

**Test ref**: `mcp.rs::test_generator_merge_all_servers_removed`

#### Scenario: SC-MCP-012d — Gemini cleanup preserves non-MCP settings

- GIVEN an existing Gemini config with `theme`, `someOtherSetting`, and servers `keep` and `remove`
- WHEN cleanup is called with only `keep` in the new config
- THEN `theme` and `someOtherSetting` MUST be preserved
- AND `keep` MUST be present with `trust: true`
- AND `remove` MUST NOT be present

**Test ref**: `mcp.rs::test_cleanup_removed_servers_gemini`

---

### REQ-MCP-013: Overwrite Strategy

When `merge_strategy = "overwrite"`, the system MUST replace the MCP section entirely with the new
server definitions.

For agents whose formatter has `preserve_on_overwrite() = true` (Gemini, Codex, OpenCode), the
system MUST preserve non-MCP top-level keys while replacing only the MCP server section.

For agents whose formatter has `preserve_on_overwrite() = false` (Claude, Copilot, VS Code, Cursor),
the system MUST write a fresh config containing only the new servers.

#### Scenario: SC-MCP-013a — Overwrite replaces all servers (standard agents)

- GIVEN an existing `.mcp.json` with server `existing`
- AND new config defining server `filesystem`
- WHEN `generate_for_agent()` is called with Overwrite strategy
- THEN `filesystem` MUST be present
- AND `existing` MUST NOT be present

**Test ref**: `mcp.rs::test_generator_overwrite_strategy`

#### Scenario: SC-MCP-013b — Overwrite preserves non-MCP keys (OpenCode)

- GIVEN an existing `opencode.json` with `theme: "dark"` and server `existing`
- AND new config defining server `filesystem`
- WHEN Overwrite strategy is used
- THEN `theme` MUST be preserved
- AND `existing` MUST NOT be present
- AND `filesystem` MUST be present

**Test ref**: `mcp.rs::test_generator_overwrite_strategy_opencode`

---

### REQ-MCP-014: File Creation and Parent Directories

The system MUST create parent directories if they do not exist before writing the MCP config file.

The system MUST distinguish between creating a new file (`created` counter) and updating an existing
file (`updated` counter).

#### Scenario: SC-MCP-014a — Creates parent directories for Copilot

- GIVEN no `.vscode/` directory exists
- WHEN `generate_for_agent(GithubCopilot)` is called
- THEN `.vscode/` directory MUST be created
- AND `.vscode/mcp.json` MUST exist

**Test ref**: `mcp.rs::test_generator_creates_parent_directories`

#### Scenario: SC-MCP-014b — Creates parent directories for Codex

- GIVEN no `.codex/` directory exists
- WHEN `generate_for_agent(CodexCli)` is called
- THEN `.codex/` directory MUST be created
- AND `.codex/config.toml` MUST exist

**Test ref**: `mcp.rs::test_generator_creates_parent_directories_codex`

#### Scenario: SC-MCP-014c — New file counted as created

- GIVEN no existing MCP config file
- WHEN `generate_for_agent()` is called
- THEN `McpSyncResult.created` MUST be 1

**Test ref**: `mcp.rs::test_generator_creates_config`

---

### REQ-MCP-015: Idempotency — Skip Identical Content

The system MUST skip writing when the generated content is identical to the existing file content.

The skip MUST be counted in `McpSyncResult.skipped`.

#### Scenario: SC-MCP-015a — Second run skips unchanged content

- GIVEN a first run that creates `.mcp.json`
- WHEN `generate_all()` is called a second time with the same servers
- THEN `McpSyncResult.skipped` MUST be 1
- AND `created` and `updated` MUST be 0

**Test ref**: `mcp.rs::test_generator_skips_identical_content`

---

### REQ-MCP-016: Dry-Run Behavior

When `dry_run = true`, the system MUST NOT write any files or create any directories.

The system MUST still report what would be done via `McpSyncResult` counters (`created`/`updated`).

The system MUST print descriptive messages indicating what would happen (prefixed with `"→"`).

#### Scenario: SC-MCP-016a — Dry run does not create files

- GIVEN a config with servers and an enabled agent
- WHEN `generate_for_agent()` is called with `dry_run = true`
- THEN `McpSyncResult.created` MUST be 1
- AND the config file MUST NOT exist on disk

**Test ref**: `mcp.rs::test_generator_dry_run`

---

### REQ-MCP-017: Shared Config Path Deduplication

When multiple agents share the same config file path (e.g. VS Code and GitHub Copilot both use
`.vscode/mcp.json`), the system MUST write the file only once.

The `generate_all()` method tracks handled paths and skips agents whose path was already processed.

#### Scenario: SC-MCP-017a — VS Code and Copilot deduplicated

- GIVEN both `VsCode` and `GithubCopilot` agents enabled
- WHEN `generate_all()` is called
- THEN only 1 file operation MUST occur (created + updated = 1)

**Test ref**: `mcp.rs::test_generator_deduplicates_shared_paths`

---

### REQ-MCP-018: Generate All Agents

The `generate_all()` method MUST iterate over all provided agents, generate configs, and aggregate
results.

Errors for individual agents MUST be logged but MUST NOT prevent other agents from being processed.
Failed agents are counted in `McpSyncResult.errors`.

#### Scenario: SC-MCP-018a — Generate for multiple agents

- GIVEN servers defined and agents `ClaudeCode` and `GithubCopilot` enabled
- WHEN `generate_all()` is called
- THEN `McpSyncResult.created` MUST be 2
- AND `.mcp.json` and `.vscode/mcp.json` MUST both exist

**Test ref**: `mcp.rs::test_generator_generate_all`

---

### REQ-MCP-019: sync_mcp Entry Point

The `Linker::sync_mcp()` method is the main entry point, called from the `apply` command.

It MUST:

1. Return early with empty result if `mcp.enabled` is false
2. Return early with empty result if `mcp_servers` is empty
3. Determine enabled agents from `[agents]` config
4. Return early if no agents are MCP-capable
5. Apply CLI agent filter (or `default_agents` if no CLI filter)
6. Return early if filtering eliminates all agents
7. Create an `McpGenerator` with the configured servers and merge strategy
8. Call `generate_all()` with the filtered agents

The `apply` command in `main.rs` invokes `sync_mcp()` when `mcp.enabled` is true and servers are
non-empty, and reports the result counts to stdout. Errors are logged and added to the overall
result error count.

**Code ref**: `src/linker.rs:1115–1169`, `src/main.rs:230–247`

#### Scenario: SC-MCP-019a — Full end-to-end sync

- GIVEN a complete config with MCP enabled, a `filesystem` server, and `agents.claude` enabled
- WHEN `sync_mcp()` is called
- THEN `.mcp.json` MUST be created
- AND it MUST contain `mcpServers.filesystem` with correct `command` and `args`

**Test ref**: `linker.rs::test_sync_mcp_creates_config_files`

#### Scenario: SC-MCP-019b — End-to-end Codex TOML generation

- GIVEN a config with `agents.codex` enabled and a `filesystem` server
- WHEN `sync_mcp()` is called
- THEN `.codex/config.toml` MUST be created
- AND it MUST contain valid TOML with `mcp_servers.filesystem`

**Test ref**: `linker.rs::test_sync_mcp_creates_codex_config_file`

---

### REQ-MCP-020: Config Serialization Skip Rules

When serializing `McpServerConfig` to JSON, the system MUST omit fields that are empty or at their
default value to produce clean output.

| Field            | Omitted When |
|------------------|--------------|
| `command`        | `None`       |
| `args`           | Empty vec    |
| `env`            | Empty map    |
| `url`            | `None`       |
| `headers`        | Empty map    |
| `transport_type` | `None`       |
| `disabled`       | `false`      |

#### Scenario: SC-MCP-020a — Empty fields omitted in serialization

- GIVEN a server with `command = "npx"`, `args = ["-y", "server"]`, `type = "stdio"`, and no
  url/headers
- WHEN the server is serialized to JSON
- THEN `command`, `args`, and `type` MUST be present
- AND `url`, `headers`, and `disabled` MUST NOT be present

**Test ref**: `config.rs::test_mcp_server_config_serialization`

---

## Acceptance Criteria

1. All 7 MCP agents produce correctly formatted config files at their documented paths
2. Merge strategy preserves existing servers and adds/overrides new ones
3. Overwrite strategy replaces MCP section; agents with `preserve_on_overwrite` keep non-MCP keys
4. Disabled servers are excluded from all output
5. CLI and default_agents filtering correctly restricts which agents receive configs
6. Shared config paths (VS Code / Copilot) are written only once
7. Identical content is not re-written (idempotency)
8. Dry-run reports actions without writing files
9. Parent directories are created automatically
10. Server ordering is deterministic (alphabetical)
11. Codex uses TOML format with `http_headers` and no `type` field
12. Gemini adds `trust: true` to every server
13. OpenCode uses `type: "local"/"remote"`, array `command`, and `enabled` field
14. Error in one agent does not prevent processing of other agents
