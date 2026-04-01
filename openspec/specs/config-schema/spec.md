# Specification: Configuration Schema

**Type**: RETROSPEC  
**Date**: 2026-04-01  
**Status**: RETROSPEC  
**Source of Truth**: `src/config.rs`, `src/agent_ids.rs`, `src/main.rs`

## Purpose

Define the schema, parsing, validation, defaults, and resolution logic of the `agentsync.toml`
configuration file. This spec covers the data model (all structs, enums, and fields), config file
discovery, TOML deserialization rules, source directory resolution, gitignore entry computation,
module-map filename resolution, MCP server schema, and serde serialization behavior.

This is a **retrospec** — every requirement and scenario is traced to existing code behavior in
`src/config.rs` and verified by existing tests.

> **Scope boundary**: This spec covers the configuration SCHEMA and PARSING. Runtime sync behavior
> is specified in `core-sync-engine/spec.md`. MCP config generation behavior is specified in
> `mcp-generation/spec.md`. Nested-glob runtime behavior is specified in `nested-glob/spec.md`.

---

## Data Model

### Config (Root)

The root configuration object, deserialized from `agentsync.toml`.

| Field                | Type                                | Default     | Serde Rule             | Description                                       |
|----------------------|-------------------------------------|-------------|------------------------|---------------------------------------------------|
| `source_dir`         | `String`                            | `"."`       | `#[serde(default)]` fn | Source directory relative to config file location |
| `compress_agents_md` | `bool`                              | `false`     | `#[serde(default)]`    | Generate compressed AGENTS.md and symlink to it   |
| `default_agents`     | `Vec<String>`                       | `[]`        | `#[serde(default)]`    | Default agents when `--agents` not specified      |
| `agents`             | `BTreeMap<String, AgentConfig>`     | `{}`        | `#[serde(default)]`    | Map of agent configurations keyed by agent name   |
| `gitignore`          | `GitignoreConfig`                   | (see below) | `#[serde(default)]`    | Settings for `.gitignore` management              |
| `mcp`                | `McpGlobalConfig`                   | (see below) | `#[serde(default)]`    | Global MCP integration settings                   |
| `mcp_servers`        | `BTreeMap<String, McpServerConfig>` | `{}`        | `#[serde(default)]`    | Map of MCP server configs keyed by server name    |

**Ordering**: Both `agents` and `mcp_servers` use `BTreeMap` for deterministic alphabetical
ordering in config and generated output.

### AgentConfig

Defines a single AI agent. Corresponds to `[agents.<name>]` in TOML.

| Field         | Type                             | Default | Serde Rule             | Description                                      |
|---------------|----------------------------------|---------|------------------------|--------------------------------------------------|
| `enabled`     | `bool`                           | `true`  | `#[serde(default)]` fn | Whether this agent is processed                  |
| `description` | `String`                         | `""`    | `#[serde(default)]`    | Human-readable description (not used at runtime) |
| `targets`     | `BTreeMap<String, TargetConfig>` | `{}`    | `#[serde(default)]`    | Map of sync targets keyed by logical name        |

### TargetConfig

Defines a single sync target. Corresponds to `[agents.<name>.targets.<target>]` in TOML.

| Field         | Type                 | Required | Serde Rule                  | Description                                                              |
|---------------|----------------------|----------|-----------------------------|--------------------------------------------------------------------------|
| `source`      | `String`             | Yes      | —                           | Source path, relative to `source_dir` (or project root for nested-glob)  |
| `destination` | `String`             | Yes      | —                           | Destination path (or template for nested-glob), relative to project root |
| `sync_type`   | `SyncType`           | Yes      | `#[serde(rename = "type")]` | Sync strategy to use                                                     |
| `pattern`     | `Option<String>`     | No       | `#[serde(default)]`         | Glob pattern for symlink-contents filtering or nested-glob matching      |
| `exclude`     | `Vec<String>`        | No       | `#[serde(default)]`         | Exclude patterns for nested-glob (ignored for other types)               |
| `mappings`    | `Vec<ModuleMapping>` | No       | `#[serde(default)]`         | Mappings for module-map targets (ignored for other types)                |

**Note on `source` semantics**: For `symlink` and `symlink-contents`, source is relative to
`source_dir`. For `nested-glob`, source is relative to the project root.

### SyncType Enum

Serialized as kebab-case via `#[serde(rename_all = "kebab-case")]`.

| Variant           | Serialized As        | Description                                                      |
|-------------------|----------------------|------------------------------------------------------------------|
| `Symlink`         | `"symlink"`          | Single symlink from source to destination                        |
| `SymlinkContents` | `"symlink-contents"` | Symlinks for each item inside a source directory                 |
| `NestedGlob`      | `"nested-glob"`      | Recursively match files by glob, create symlinks via template    |
| `ModuleMap`       | `"module-map"`       | Map source files to module directories with convention filenames |

**Derives**: `Debug, Clone, Copy, PartialEq, Eq`

### ModuleMapping

Defines a single mapping within a `module-map` target. Appears as
`[[agents.<name>.targets.<target>.mappings]]` in TOML.

| Field               | Type             | Required | Serde Rule          | Description                                             |
|---------------------|------------------|----------|---------------------|---------------------------------------------------------|
| `source`            | `String`         | Yes      | —                   | Source file path, relative to `source_dir`              |
| `destination`       | `String`         | Yes      | —                   | Destination directory, relative to project root         |
| `filename_override` | `Option<String>` | No       | `#[serde(default)]` | Override output filename (bypasses convention filename) |

**Derives**: `Debug, Clone`

### GitignoreConfig

Settings for `.gitignore` management. Corresponds to `[gitignore]` in TOML.

| Field     | Type          | Default               | Serde Rule             | Description                                    |
|-----------|---------------|-----------------------|------------------------|------------------------------------------------|
| `enabled` | `bool`        | `true`                | `#[serde(default)]` fn | Whether to manage `.gitignore`                 |
| `marker`  | `String`      | `"AI Agent Symlinks"` | `#[serde(default)]` fn | Marker text for managed section delimiters     |
| `entries` | `Vec<String>` | `[]`                  | `#[serde(default)]`    | Additional paths to include in managed section |

Implements `Default` trait with the values above.

### McpGlobalConfig

Global MCP settings. Corresponds to `[mcp]` in TOML.

| Field            | Type               | Default | Serde Rule             | Description                             |
|------------------|--------------------|---------|------------------------|-----------------------------------------|
| `enabled`        | `bool`             | `true`  | `#[serde(default)]` fn | Enable/disable MCP propagation globally |
| `merge_strategy` | `McpMergeStrategy` | `Merge` | `#[serde(default)]`    | How to handle existing MCP config files |

Implements `Default` trait. **Derives**: `Debug, Clone`.

### McpMergeStrategy Enum

Serialized as lowercase via `#[serde(rename_all = "lowercase")]`.

| Variant     | Serialized As | Description                                 |
|-------------|---------------|---------------------------------------------|
| `Merge`     | `"merge"`     | Merge with existing configuration (default) |
| `Overwrite` | `"overwrite"` | Overwrite existing configuration completely |

**Derives**: `Debug, Clone, Copy, PartialEq, Eq, Default` (`#[default]` on `Merge`).

### McpServerConfig

Configuration for a single MCP server. Corresponds to `[mcp_servers.<name>]` in TOML.

| Field            | Type                       | Default | Serde Rule                                   | Description                          |
|------------------|----------------------------|---------|----------------------------------------------|--------------------------------------|
| `command`        | `Option<String>`           | `None`  | `skip_serializing_if = "Option::is_none"`    | Command to execute (stdio transport) |
| `args`           | `Vec<String>`              | `[]`    | `skip_serializing_if = "Vec::is_empty"`      | Arguments for the command            |
| `env`            | `BTreeMap<String, String>` | `{}`    | `skip_serializing_if = "BTreeMap::is_empty"` | Environment variables                |
| `url`            | `Option<String>`           | `None`  | `skip_serializing_if = "Option::is_none"`    | URL for HTTP/SSE transport           |
| `headers`        | `BTreeMap<String, String>` | `{}`    | `skip_serializing_if = "BTreeMap::is_empty"` | HTTP headers (for remote servers)    |
| `transport_type` | `Option<String>`           | `None`  | `rename = "type"`, `skip_serializing_if`     | Transport type (stdio, http, sse)    |
| `disabled`       | `bool`                     | `false` | `skip_serializing_if = "is_false"`           | Whether the server is disabled       |

**Derives**: `Debug, Deserialize, Serialize, Clone`.

**Note**: Both `Deserialize` and `Serialize` are derived. The `skip_serializing_if` rules ensure
clean JSON output when generating agent-specific MCP configs (empty collections and `None` values
are omitted). MCP generation behavior is fully specified in `mcp-generation/spec.md`.

### Constants

| Constant             | Value              | Description                   |
|----------------------|--------------------|-------------------------------|
| `CONFIG_FILE_NAME`   | `"agentsync.toml"` | Default config file name      |
| `DEFAULT_SOURCE_DIR` | `".agents"`        | Default source directory name |

---

## Requirements

### REQ-CS-001: Config File Discovery (`find_config`)

The system MUST locate the configuration file by searching from a start directory upward through
parent directories.

At each directory level, the system MUST check for `<dir>/.agents/agentsync.toml` FIRST, then
`<dir>/agentsync.toml`.

The `.agents/agentsync.toml` location MUST take priority over root-level `agentsync.toml` when
both exist in the same directory.

If no config file is found after traversing to the filesystem root, the system MUST return an
error with the message: `"Could not find agentsync.toml in <start_dir> or any parent directory"`.

**Code ref**: `Config::find_config()` (config.rs:301-326)

#### Scenario: SC-CS-001a — Config found in .agents directory

- GIVEN a directory structure with `.agents/agentsync.toml`
- WHEN `Config::find_config()` is called with that directory
- THEN it MUST return the path `.agents/agentsync.toml`

#### Scenario: SC-CS-001b — Config found in project root

- GIVEN a directory with `agentsync.toml` at the root (no `.agents/` subdirectory)
- WHEN `Config::find_config()` is called
- THEN it MUST return the root-level `agentsync.toml` path

#### Scenario: SC-CS-001c — .agents config preferred over root config

- GIVEN a directory with BOTH `.agents/agentsync.toml` AND `agentsync.toml`
- WHEN `Config::find_config()` is called
- THEN it MUST return `.agents/agentsync.toml`

#### Scenario: SC-CS-001d — Config found in parent directory

- GIVEN a nested directory `sub1/sub2/sub3` with config only at the top-level ancestor
- WHEN `Config::find_config()` is called from the nested directory
- THEN it MUST find and return the ancestor's config path

#### Scenario: SC-CS-001e — Config not found

- GIVEN a directory tree with no `agentsync.toml` at any level
- WHEN `Config::find_config()` is called
- THEN it MUST return an error

---

### REQ-CS-002: Config Loading (`load`)

The system MUST read the config file as UTF-8 text and parse it as TOML into the `Config` struct.

If the file cannot be read, the system MUST return an error with context
`"Failed to read config file: <path>"`.

If the TOML is invalid or does not conform to the expected schema, the system MUST return an error
with context `"Failed to parse config file: <path>"`.

**Code ref**: `Config::load()` (config.rs:290-298)

#### Scenario: SC-CS-002a — Valid config loads successfully

- GIVEN a valid `agentsync.toml` file with `source_dir = "custom_dir"` and an agent definition
- WHEN `Config::load()` is called
- THEN it MUST return a `Config` with `source_dir == "custom_dir"`
- AND the agent MUST be present in `config.agents`

#### Scenario: SC-CS-002b — File not found

- GIVEN a path to a non-existent file
- WHEN `Config::load()` is called
- THEN it MUST return an error

#### Scenario: SC-CS-002c — Invalid TOML syntax

- GIVEN a file containing `"this is not { valid toml"`
- WHEN `Config::load()` is called
- THEN it MUST return an error

#### Scenario: SC-CS-002d — Invalid sync type value

- GIVEN a config with `type = "invalid-type"` on a target
- WHEN the TOML is parsed
- THEN it MUST return a deserialization error

#### Scenario: SC-CS-002e — Missing required field (destination)

- GIVEN a target config with `source` and `type` but no `destination`
- WHEN the TOML is parsed
- THEN it MUST return a deserialization error

---

### REQ-CS-003: Empty/Minimal Config Parsing

The system MUST accept an empty TOML string and produce a valid `Config` with all defaults applied.

**Code ref**: `test_parse_empty_config` (config.rs:482-490)

#### Scenario: SC-CS-003a — Empty config produces valid defaults

- GIVEN an empty string as TOML input
- WHEN it is parsed into `Config`
- THEN `agents` MUST be an empty map
- AND `source_dir` MUST be `"."`
- AND `compress_agents_md` MUST be `false`
- AND `gitignore.enabled` MUST be `true`
- AND `default_agents` MUST be empty
- AND `mcp.enabled` MUST be `true`
- AND `mcp.merge_strategy` MUST be `Merge`
- AND `mcp_servers` MUST be empty

#### Scenario: SC-CS-003b — Minimal config with agent but no explicit fields

- GIVEN a config with `[agents.test]` and a target, but no `enabled` or `description` fields
- WHEN it is parsed
- THEN `agents["test"].enabled` MUST be `true` (default)
- AND `agents["test"].description` MUST be `""` (default)
- AND `source_dir` MUST be `"."` (default)

---

### REQ-CS-004: Project Root Resolution (`project_root`)

The system MUST derive the project root from the config file path.

If the config file's parent directory is named `.agents`, the project root MUST be the
grandparent directory (parent of `.agents`).

Otherwise, the project root MUST be the config file's parent directory.

**Code ref**: `Config::project_root()` (config.rs:329-341)

#### Scenario: SC-CS-004a — Config inside .agents directory

- GIVEN a config path `/project/.agents/agentsync.toml`
- WHEN `Config::project_root()` is called
- THEN it MUST return `/project`

#### Scenario: SC-CS-004b — Config at project root

- GIVEN a config path `/project/agentsync.toml`
- WHEN `Config::project_root()` is called
- THEN it MUST return `/project`

#### Scenario: SC-CS-004c — .agents inside a subdirectory

- GIVEN a config path `/project/subdir/.agents/agentsync.toml`
- WHEN `Config::project_root()` is called
- THEN it MUST return `/project/subdir`

---

### REQ-CS-005: Source Directory Resolution (`source_dir`)

The system MUST resolve the source directory by joining the config file's parent directory with
the `source_dir` field value.

The `source_dir` field defaults to `"."`, meaning the source directory is the same as the config
file's directory.

**Code ref**: `Config::source_dir()` (config.rs:344-348)

#### Scenario: SC-CS-005a — Default source_dir with config in .agents

- GIVEN a config at `.agents/agentsync.toml` with `source_dir = "."`
- WHEN `config.source_dir(&config_path)` is called
- THEN it MUST resolve to the `.agents/` directory (i.e., `<agents_dir>/.`)

#### Scenario: SC-CS-005b — Custom source_dir

- GIVEN a config at `<root>/agentsync.toml` with `source_dir = "custom/sources"`
- WHEN `config.source_dir(&config_path)` is called
- THEN it MUST resolve to `<root>/custom/sources`

---

### REQ-CS-006: SyncType Deserialization

The `SyncType` enum MUST deserialize from kebab-case strings via
`#[serde(rename_all = "kebab-case")]`.

The `sync_type` field MUST be deserialized from the TOML key `type` via `#[serde(rename = "type")]`.

Any unrecognized string value MUST cause a deserialization error.

**Code ref**: `SyncType` enum (config.rs:132-148)

#### Scenario: SC-CS-006a — "symlink" deserializes to Symlink

- GIVEN `type = "symlink"` in a target config
- WHEN parsed
- THEN `sync_type` MUST be `SyncType::Symlink`

#### Scenario: SC-CS-006b — "symlink-contents" deserializes to SymlinkContents

- GIVEN `type = "symlink-contents"` in a target config
- WHEN parsed
- THEN `sync_type` MUST be `SyncType::SymlinkContents`

#### Scenario: SC-CS-006c — "nested-glob" deserializes to NestedGlob

- GIVEN `type = "nested-glob"` with `pattern = "**/AGENTS.md"` and `exclude` list
- WHEN parsed
- THEN `sync_type` MUST be `SyncType::NestedGlob`
- AND `pattern` MUST be `Some("**/AGENTS.md")`
- AND `exclude` MUST contain the specified patterns

#### Scenario: SC-CS-006d — "module-map" deserializes to ModuleMap

- GIVEN `type = "module-map"` in a target config
- WHEN parsed
- THEN `sync_type` MUST be `SyncType::ModuleMap`

#### Scenario: SC-CS-006e — Invalid type string rejected

- GIVEN `type = "invalid-type"` in a target config
- WHEN parsed
- THEN it MUST produce a deserialization error

---

### REQ-CS-007: Default Agents Configuration

The `default_agents` field MUST be a list of agent name strings.

When empty (the default), all enabled agents MUST be processed.

When non-empty, only agents matching the `default_agents` list MUST be processed (when no CLI
`--agents` filter is provided).

Matching uses `sync_filter_matches` from `agent_ids` which supports case-insensitive matching and
alias resolution.

The CLI `--agents` flag MUST override `default_agents` when specified.

**Code ref**: `Config::default_agents` (config.rs:33-35), applied in `linker.rs` sync logic

#### Scenario: SC-CS-007a — Empty default_agents by default

- GIVEN an empty config
- WHEN parsed
- THEN `default_agents` MUST be an empty `Vec`

#### Scenario: SC-CS-007b — Parsing a list of default agents

- GIVEN `default_agents = ["copilot", "claude", "gemini"]`
- WHEN parsed
- THEN `default_agents` MUST contain exactly those 3 strings
- AND agents not in the list (e.g., `cursor`) MUST NOT be in `default_agents`

#### Scenario: SC-CS-007c — Single default agent

- GIVEN `default_agents = ["claude"]`
- WHEN parsed
- THEN `default_agents` MUST have length 1 with value `"claude"`

#### Scenario: SC-CS-007d — Coexists with other config sections

- GIVEN a config with `source_dir`, `default_agents`, `[gitignore]`, and `[agents.*]` sections
- WHEN parsed
- THEN all fields MUST be correctly populated without interference

---

### REQ-CS-008: GitignoreConfig Defaults

The `GitignoreConfig` MUST default to `enabled = true`, `marker = "AI Agent Symlinks"`, and
`entries = []`.

These defaults MUST apply both via the `Default` trait implementation and via serde default
functions when the `[gitignore]` section is omitted from TOML.

**Code ref**: `GitignoreConfig::default()` (config.rs:206-214), default functions (config.rs:
189-204)

#### Scenario: SC-CS-008a — Defaults when section omitted

- GIVEN a config with no `[gitignore]` section
- WHEN parsed
- THEN `gitignore.enabled` MUST be `true`
- AND `gitignore.marker` MUST be `"AI Agent Symlinks"`
- AND `gitignore.entries` MUST be empty

#### Scenario: SC-CS-008b — Custom marker

- GIVEN `[gitignore]` with `marker = "Custom Marker"`
- WHEN parsed
- THEN `gitignore.marker` MUST be `"Custom Marker"`

#### Scenario: SC-CS-008c — Disabled gitignore

- GIVEN `[gitignore]` with `enabled = false`
- WHEN parsed
- THEN `gitignore.enabled` MUST be `false`

---

### REQ-CS-009: MCP Global Config Defaults

The `McpGlobalConfig` MUST default to `enabled = true` and `merge_strategy = Merge`.

When the `[mcp]` section is omitted, these defaults MUST apply.

**Code ref**: `McpGlobalConfig::default()` (config.rs:232-239)

#### Scenario: SC-CS-009a — Defaults when section omitted

- GIVEN a config with no `[mcp]` section
- WHEN parsed
- THEN `mcp.enabled` MUST be `true`
- AND `mcp.merge_strategy` MUST be `McpMergeStrategy::Merge`
- AND `mcp_servers` MUST be empty

#### Scenario: SC-CS-009b — MCP disabled

- GIVEN `[mcp]` with `enabled = false`
- WHEN parsed
- THEN `mcp.enabled` MUST be `false`

#### Scenario: SC-CS-009c — Overwrite merge strategy

- GIVEN `[mcp]` with `merge_strategy = "overwrite"`
- WHEN parsed
- THEN `mcp.merge_strategy` MUST be `McpMergeStrategy::Overwrite`

---

### REQ-CS-010: MCP Server Config Parsing

The system MUST support stdio servers (with `command` and `args`), remote servers (with `url` and
`headers`), and servers with environment variables (`env`).

Multiple servers MUST be supported as separate `[mcp_servers.<name>]` sections.

Servers MAY be disabled with `disabled = true`.

MCP server configs MUST coexist with agent configs in the same file.

**Code ref**: `McpServerConfig` (config.rs:253-282), tests (config.rs:1190-1322)

#### Scenario: SC-CS-010a — Stdio server config

- GIVEN `[mcp_servers.filesystem]` with `command = "npx"` and `args = ["-y", "server", "/path"]`
- WHEN parsed
- THEN `command` MUST be `Some("npx")`
- AND `args` MUST have 3 elements
- AND `url` MUST be `None`

#### Scenario: SC-CS-010b — Server with environment variables

- GIVEN a server config with `[mcp_servers.test.env]` containing `DEBUG = "true"` and
  `API_KEY = "${MY_API_KEY}"`
- WHEN parsed
- THEN `env["DEBUG"]` MUST be `"true"`
- AND `env["API_KEY"]` MUST be `"${MY_API_KEY}"` (literal, no interpolation)

#### Scenario: SC-CS-010c — Remote URL server

- GIVEN `[mcp_servers.remote]` with `url = "https://api.example.com"` and
  `[mcp_servers.remote.headers]` containing `Authorization = "Bearer token123"`
- WHEN parsed
- THEN `url` MUST be `Some("https://api.example.com")`
- AND `headers["Authorization"]` MUST be `"Bearer token123"`
- AND `command` MUST be `None`

#### Scenario: SC-CS-010d — Disabled server

- GIVEN a server config with `disabled = true`
- WHEN parsed
- THEN `server.disabled` MUST be `true`

#### Scenario: SC-CS-010e — Multiple servers

- GIVEN 3 server sections: `filesystem`, `git`, `postgres`
- WHEN parsed
- THEN `mcp_servers` MUST contain exactly 3 entries with those keys

#### Scenario: SC-CS-010f — MCP servers coexist with agents

- GIVEN a config with both `[mcp_servers.filesystem]` and `[agents.claude]`
- WHEN parsed
- THEN both `mcp.enabled` and `agents["claude"]` MUST be accessible
- AND `mcp_servers` MUST contain the server

---

### REQ-CS-011: MCP Server Config Serialization

`McpServerConfig` MUST implement both `Deserialize` and `Serialize`.

During JSON serialization, the following `skip_serializing_if` rules MUST apply:

- `command`: skip if `None`
- `args`: skip if empty `Vec`
- `env`: skip if empty `BTreeMap`
- `url`: skip if `None`
- `headers`: skip if empty `BTreeMap`
- `transport_type`: skip if `None` (serialized as `"type"`)
- `disabled`: skip if `false`

**Code ref**: `McpServerConfig` serde attributes (config.rs:253-282), test (config.rs:1325-1345)

#### Scenario: SC-CS-011a — Serialization omits empty/default fields

- GIVEN an `McpServerConfig` with `command = Some("npx")`, `args = ["-y", "server"]`,
  `env = {"DEBUG": "true"}`, `url = None`, `headers = {}`, `transport_type = Some("stdio")`,
  `disabled = false`
- WHEN serialized to JSON
- THEN the JSON MUST contain `"command"`, `"args"`, `"env"`, and `"type"`
- AND the JSON MUST NOT contain `"headers"` (empty map skipped)
- AND the JSON MUST NOT contain `"disabled"` (false skipped)
- AND the JSON MUST NOT contain `"url"` (None skipped)

---

### REQ-CS-012: Gitignore Entry Computation (`all_gitignore_entries`)

The system MUST compute the full set of gitignore entries from:

1. Manual entries from `gitignore.entries`
2. Target destinations from enabled agents (with `.bak` companion entries)
3. Known ignore patterns for each enabled agent (from `agent_ids`)
4. A defensive pattern `.agents/skills/*.bak`

Entries MUST be collected into a `BTreeSet` for deduplication and automatic sorting.

The returned `Vec<String>` MUST be sorted alphabetically.

**Code ref**: `Config::all_gitignore_entries()` (config.rs:351-393)

#### Scenario: SC-CS-012a — Collects target destinations

- GIVEN enabled agents with `destination = "CLAUDE.md"` and
  `destination = ".github/copilot-instructions.md"`
- AND `gitignore.entries = ["manual-entry.md"]`
- WHEN `all_gitignore_entries()` is called
- THEN entries MUST include `"manual-entry.md"`, `"/CLAUDE.md"`, and
  `".github/copilot-instructions.md"`

#### Scenario: SC-CS-012b — Skips disabled agents

- GIVEN one enabled agent with `destination = "enabled.md"` and one disabled agent with
  `destination = "disabled.md"`
- WHEN `all_gitignore_entries()` is called
- THEN entries MUST include `"/enabled.md"` and `"/enabled.md.bak"`
- AND entries MUST NOT include `"disabled.md"` or `"disabled.md.bak"`

#### Scenario: SC-CS-012c — Deduplicates entries

- GIVEN manual `entries = ["AGENTS.md"]` and an agent target with `destination = "AGENTS.md"`
- WHEN `all_gitignore_entries()` is called
- THEN the manual entry `"AGENTS.md"` and managed entry `"/AGENTS.md"` MUST both appear
  (different forms are not duplicates)

#### Scenario: SC-CS-012d — Entries are sorted

- GIVEN multiple agents producing multiple entries
- WHEN `all_gitignore_entries()` is called
- THEN the returned vector MUST be in alphabetical order

#### Scenario: SC-CS-012e — Includes backup patterns

- GIVEN an enabled agent with `destination = "OUTPUT.md"`
- WHEN `all_gitignore_entries()` is called
- THEN entries MUST include `"/OUTPUT.md"` and `"/OUTPUT.md.bak"`
- AND entries MUST include `".agents/skills/*.bak"` (defensive pattern)

#### Scenario: SC-CS-012f — Includes known agent ignore patterns

- GIVEN enabled agents `claude` and `opencode` (even without explicit targets)
- WHEN `all_gitignore_entries()` is called
- THEN entries MUST include `"/.mcp.json"`, `".claude/commands/"`, `".claude/skills/"` (claude)
- AND entries MUST include `"/opencode.json"` (opencode)

#### Scenario: SC-CS-012g — Disabled agents exclude known patterns

- GIVEN `claude` disabled and `opencode` enabled
- WHEN `all_gitignore_entries()` is called
- THEN entries MUST NOT include claude's known patterns
- AND entries MUST include opencode's known patterns

#### Scenario: SC-CS-012h — Known patterns for alias agents

- GIVEN an enabled agent named `codex-cli`
- WHEN `all_gitignore_entries()` is called
- THEN entries MUST include `".codex/config.toml"` (resolved via alias)

---

### REQ-CS-013: Gitignore Entry Normalization

When computing managed gitignore entries from target destinations and known patterns, bare
filenames (those without `/`, `*`, `?`, `[`, or leading `!`) MUST be prefixed with `/` to anchor
them to the project root.

Entries that already contain path separators, glob characters, or negation prefixes MUST be
preserved as-is.

Manual entries from `gitignore.entries` are inserted verbatim (normalization applies only to
managed entries from targets and known patterns).

**Code ref**: `normalize_managed_gitignore_entry()` (config.rs:402-413)

#### Scenario: SC-CS-013a — Bare filename gets root-scoped

- GIVEN a target destination `"CLAUDE.md"` (no path separator)
- WHEN it is added as a managed gitignore entry
- THEN it MUST become `"/CLAUDE.md"`

#### Scenario: SC-CS-013b — Path with separator preserved

- GIVEN a target destination `".github/copilot-instructions.md"`
- WHEN it is added as a managed gitignore entry
- THEN it MUST remain `".github/copilot-instructions.md"` (already contains `/`)

#### Scenario: SC-CS-013c — Glob patterns preserved

- GIVEN a pattern like `".agents/skills/*.bak"`
- WHEN it is added as a managed gitignore entry
- THEN it MUST remain unchanged (contains `*`)

#### Scenario: SC-CS-013d — Manual entries remain verbatim

- GIVEN `gitignore.entries = ["AGENTS.md", "docs/AGENTS.md"]`
- WHEN `all_gitignore_entries()` is called
- THEN `"AGENTS.md"` and `"docs/AGENTS.md"` MUST appear exactly as specified

---

### REQ-CS-014: Nested-Glob Destinations Excluded from Gitignore

Template destination strings from `nested-glob` targets MUST NOT be added to gitignore entries.

This prevents raw template strings like `"{relative_path}/CLAUDE.md"` from appearing in
`.gitignore`.

**Code ref**: `all_gitignore_entries()` NestedGlob check (config.rs:362-364)

#### Scenario: SC-CS-014a — Template destination not in gitignore

- GIVEN a nested-glob target with `destination = "{relative_path}/CLAUDE.md"`
- WHEN `all_gitignore_entries()` is called
- THEN `"{relative_path}/CLAUDE.md"` MUST NOT appear in the entries

---

### REQ-CS-015: Module-Map Gitignore Entry Expansion

For `module-map` targets, the system MUST NOT add the target's own `destination` to gitignore.
Instead, it MUST expand each mapping into individual gitignore entries using the resolved filename.

Each expanded entry MUST be formatted as `<mapping.destination>/<resolved_filename>`.

Each expanded entry MUST also have a `.bak` companion entry.

Filename resolution uses `resolve_module_map_filename()`.

**Code ref**: `all_gitignore_entries()` ModuleMap handling (config.rs:366-377)

#### Scenario: SC-CS-015a — Module-map expands to individual entries

- GIVEN a claude agent with module-map mappings to `src/api` and `src/ui`
- WHEN `all_gitignore_entries()` is called
- THEN entries MUST include `"src/api/CLAUDE.md"`, `"src/api/CLAUDE.md.bak"`,
  `"src/ui/CLAUDE.md"`, and `"src/ui/CLAUDE.md.bak"`

#### Scenario: SC-CS-015b — Module-map with filename_override

- GIVEN a mapping with `filename_override = "CUSTOM.md"` to `src/api`
- WHEN `all_gitignore_entries()` is called
- THEN entries MUST include `"src/api/CUSTOM.md"`
- AND entries MUST NOT include `"src/api/CLAUDE.md"`

#### Scenario: SC-CS-015c — Disabled agent's module-map skipped

- GIVEN a disabled agent with module-map targets
- WHEN `all_gitignore_entries()` is called
- THEN no entries from those mappings MUST appear

#### Scenario: SC-CS-015d — Module-map entries deduplicated

- GIVEN a claude module-map mapping to `src/shared` AND a codex symlink target with
  `destination = "src/shared/AGENTS.md"`
- WHEN `all_gitignore_entries()` is called
- THEN `"src/shared/AGENTS.md"` MUST appear exactly once (deduplicated by BTreeSet)

---

### REQ-CS-016: Module-Map Filename Resolution (`resolve_module_map_filename`)

The system MUST resolve the output filename for module-map mappings with the following priority:

1. **`filename_override`**: If set, use it directly
2. **Agent convention filename**: If the agent has a known convention filename (via
   `agent_ids::agent_convention_filename()`), use it
3. **Source basename fallback**: Use the source file's basename

Convention filename lookup MUST be case-insensitive (via canonical ID resolution in `agent_ids`).

**Code ref**: `resolve_module_map_filename()` (config.rs:171-182)

#### Scenario: SC-CS-016a — Filename override takes priority

- GIVEN a mapping with `filename_override = Some("custom-name.md")`
- WHEN `resolve_module_map_filename()` is called for agent `"claude"`
- THEN it MUST return `"custom-name.md"`

#### Scenario: SC-CS-016b — Convention filename for claude

- GIVEN a mapping with no `filename_override`
- WHEN `resolve_module_map_filename()` is called for agent `"claude"`
- THEN it MUST return `"CLAUDE.md"`

#### Scenario: SC-CS-016c — Convention filename is case-insensitive

- GIVEN a mapping with no `filename_override`
- WHEN `resolve_module_map_filename()` is called for agent `"Claude"` (mixed case)
- THEN it MUST return `"CLAUDE.md"`

#### Scenario: SC-CS-016d — Convention filename for copilot

- GIVEN a mapping with no `filename_override`
- WHEN `resolve_module_map_filename()` is called for agent `"copilot"`
- THEN it MUST return `".github/copilot-instructions.md"`

#### Scenario: SC-CS-016e — Fallback to source basename for unknown agent

- GIVEN a mapping with `source = "api-context.md"` and no `filename_override`
- WHEN `resolve_module_map_filename()` is called for agent `"unknown-agent"`
- THEN it MUST return `"api-context.md"`

#### Scenario: SC-CS-016f — Override beats convention

- GIVEN a mapping with `filename_override = Some("MY-RULES.md")`
- WHEN `resolve_module_map_filename()` is called for agent `"claude"`
- THEN it MUST return `"MY-RULES.md"` (not `"CLAUDE.md"`)

---

### REQ-CS-017: Known Ignore Patterns (`known_ignore_patterns`)

The system MUST return agent-specific gitignore patterns for all recognized agents.

Pattern lookup MUST be case-insensitive and alias-aware (via `canonical_mcp_agent_id` and
`canonical_configurable_agent_id`).

Unknown/unrecognized agent names MUST return an empty slice.

**Code ref**: `agent_ids::known_ignore_patterns()` (agent_ids.rs:93-141), delegated from
`Config::known_ignore_patterns()` (config.rs:397-399)

#### Scenario: SC-CS-017a — Claude patterns

- GIVEN agent name `"claude"`
- WHEN `known_ignore_patterns()` is called
- THEN it MUST return `[".mcp.json", ".claude/commands/", ".claude/skills/"]`

#### Scenario: SC-CS-017b — Case-insensitive lookup

- GIVEN agent name `"CLAUDE"` or `"Claude"`
- WHEN `known_ignore_patterns()` is called
- THEN it MUST return the same patterns as `"claude"`

#### Scenario: SC-CS-017c — Alias resolution (codex variants)

- GIVEN agent names `"codex"`, `"codex-cli"`, or `"codex_cli"`
- WHEN `known_ignore_patterns()` is called on any of them
- THEN they MUST all return `[".codex/config.toml"]`

#### Scenario: SC-CS-017d — Unknown agent returns empty

- GIVEN agent name `"unknown-agent"`
- WHEN `known_ignore_patterns()` is called
- THEN it MUST return an empty slice

---

### REQ-CS-018: TargetConfig Optional Fields Defaults

When optional fields are omitted from a target config, the system MUST apply these defaults:

- `pattern`: `None`
- `exclude`: empty `Vec`
- `mappings`: empty `Vec`

**Code ref**: `TargetConfig` struct serde defaults (config.rs:86-129)

#### Scenario: SC-CS-018a — Mappings default to empty

- GIVEN a `symlink` target with no `mappings` field
- WHEN parsed
- THEN `target.mappings` MUST be an empty `Vec`

#### Scenario: SC-CS-018b — Pattern defaults to None

- GIVEN a target with no `pattern` field
- WHEN parsed
- THEN `target.pattern` MUST be `None`

---

### REQ-CS-019: Compress Agents MD Flag

The `compress_agents_md` field MUST default to `false`.

When set to `true`, it signals the sync engine to generate compressed AGENTS.md files. The
compression behavior itself is specified in `core-sync-engine/spec.md` (REQ-021).

**Code ref**: `Config::compress_agents_md` (config.rs:28-29)

#### Scenario: SC-CS-019a — Default is false

- GIVEN an empty config
- WHEN parsed
- THEN `compress_agents_md` MUST be `false`

#### Scenario: SC-CS-019b — Explicitly enabled

- GIVEN `compress_agents_md = true` in config
- WHEN parsed
- THEN `compress_agents_md` MUST be `true`

---

### REQ-CS-020: CLI Config Loading Integration

The CLI `apply` and `clean` commands MUST:

1. Accept an optional `--config` path; if not provided, use `Config::find_config()` from
   the start directory (CWD or `--path`)
2. Call `Config::load()` on the resolved config path
3. Pass the loaded `Config` and config path to `Linker::new()`

The `apply` command MUST use the config to determine:

- Whether to update `.gitignore` (`config.gitignore.enabled` AND not `--no-gitignore`)
- Whether to sync MCP configs (`config.mcp.enabled` AND `mcp_servers` non-empty)
- The gitignore marker text (`config.gitignore.marker`)
- The gitignore entries (`config.all_gitignore_entries()`)

**Code ref**: `main.rs` Commands::Apply (main.rs:171-260), Commands::Clean (main.rs:261-283)

#### Scenario: SC-CS-020a — Apply uses find_config when no --config

- GIVEN no `--config` argument
- WHEN `agentsync apply` is run from a project directory
- THEN it MUST use `Config::find_config()` to locate the config

#### Scenario: SC-CS-020b — Apply uses explicit --config path

- GIVEN `--config custom/path/agentsync.toml`
- WHEN `agentsync apply` is run
- THEN it MUST load config from that exact path (no find_config)

#### Scenario: SC-CS-020c — Gitignore disabled skips update, runs cleanup

- GIVEN `gitignore.enabled = false` in config
- WHEN `agentsync apply` is run (without `--no-gitignore`)
- THEN it MUST run `gitignore::cleanup_gitignore()` instead of `update_gitignore()`

#### Scenario: SC-CS-020d — MCP sync conditional on config

- GIVEN `mcp.enabled = true` and `mcp_servers` is non-empty
- WHEN `agentsync apply` is run
- THEN MCP configs MUST be synced
- GIVEN `mcp.enabled = false` OR `mcp_servers` is empty
- WHEN `agentsync apply` is run
- THEN MCP config sync MUST be skipped

---

## Non-Functional Requirements

### NF-CS-1: Deterministic Ordering

`BTreeMap` MUST be used for `agents`, `mcp_servers`, `env`, `headers`, and `targets` to ensure
deterministic alphabetical ordering across all operations and output.

### NF-CS-2: Error Context

All error paths in config loading MUST use `anyhow::Context` to provide user-readable error
messages that include the file path.

### NF-CS-3: Backward Compatibility

The config schema MUST accept configs written for any previous version. All new fields MUST have
serde defaults so that older config files parse without error.

### NF-CS-4: No Runtime Validation Beyond Parsing

The `Config::load()` method performs only TOML deserialization. It does NOT validate that source
files exist, that destinations are safe, or that agent names are recognized. Those validations
occur at sync time in the Linker (specified in `core-sync-engine/spec.md`).

---

## Acceptance Criteria

1. Empty TOML string parses to valid `Config` with all defaults
2. `Config::find_config()` searches `.agents/` first, then root, then parent directories
3. `.agents/agentsync.toml` takes priority over root-level `agentsync.toml`
4. `Config::load()` returns contextual errors for missing files and invalid TOML
5. `Config::project_root()` correctly derives root from both config locations
6. `Config::source_dir()` joins config parent with `source_dir` field
7. `SyncType` deserializes all four kebab-case variants and rejects unknowns
8. `TargetConfig` requires `source`, `destination`, and `type`; all other fields have defaults
9. `ModuleMapping` parses `source`, `destination`, and optional `filename_override`
10. `resolve_module_map_filename()` follows priority: override > convention > source basename
11. Convention filename lookup is case-insensitive and alias-aware
12. `default_agents` defaults to empty and parses string lists
13. `GitignoreConfig` defaults: `enabled=true`, `marker="AI Agent Symlinks"`, `entries=[]`
14. `McpGlobalConfig` defaults: `enabled=true`, `merge_strategy=Merge`
15. `McpServerConfig` supports stdio (command+args), remote (url+headers), env vars, and disabled
    flag
16. `McpServerConfig` serialization skips empty/default fields via `skip_serializing_if`
17. `all_gitignore_entries()` collects manual entries, target destinations (with .bak), and known
    patterns
18. `all_gitignore_entries()` skips disabled agents entirely
19. `all_gitignore_entries()` skips nested-glob template destinations
20. `all_gitignore_entries()` expands module-map mappings individually
21. Entries are deduplicated via `BTreeSet` and returned sorted
22. Bare filenames are root-scoped with `/` prefix; paths with separators/globs preserved as-is
23. Known ignore patterns are case-insensitive and alias-aware
24. CLI commands load config via `find_config()` or explicit `--config` path
25. All existing config.rs tests pass (no regressions)

---

## Code References

| Requirement | Primary Code Location                                             | Test Coverage                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
|-------------|-------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| REQ-CS-001  | `Config::find_config()` (config.rs:301-326)                       | `test_find_config_in_agents_dir`, `test_find_config_in_root`, `test_find_config_prefers_agents_dir`, `test_find_config_searches_parent_dirs`, `test_find_config_not_found`                                                                                                                                                                                                                                                                                                                                                                                   |
| REQ-CS-002  | `Config::load()` (config.rs:290-298)                              | `test_load_config_from_file`, `test_load_config_file_not_found`, `test_load_config_invalid_toml`                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| REQ-CS-003  | Serde defaults on `Config` struct                                 | `test_parse_empty_config`, `test_parse_config_with_defaults`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| REQ-CS-004  | `Config::project_root()` (config.rs:329-341)                      | `test_project_root_from_agents_dir`, `test_project_root_from_root_config`, `test_project_root_nested_agents_dir`                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| REQ-CS-005  | `Config::source_dir()` (config.rs:344-348)                        | `test_source_dir_default`, `test_source_dir_custom`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| REQ-CS-006  | `SyncType` enum (config.rs:132-148)                               | `test_sync_type_symlink`, `test_sync_type_symlink_contents`, `test_parse_nested_glob_target`, `test_parse_module_map_sync_type`, `test_parse_invalid_sync_type`                                                                                                                                                                                                                                                                                                                                                                                              |
| REQ-CS-007  | `Config::default_agents` (config.rs:33-35)                        | `test_default_agents_empty_by_default`, `test_default_agents_parsing`, `test_default_agents_single_agent`, `test_default_agents_with_other_config`                                                                                                                                                                                                                                                                                                                                                                                                           |
| REQ-CS-008  | `GitignoreConfig` (config.rs:186-214)                             | `test_gitignore_config_defaults`, `test_gitignore_config_custom_marker`, `test_gitignore_config_disabled`                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| REQ-CS-009  | `McpGlobalConfig` (config.rs:222-250)                             | `test_mcp_config_defaults`, `test_mcp_config_disabled`, `test_mcp_merge_strategy_overwrite`                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| REQ-CS-010  | `McpServerConfig` (config.rs:253-282)                             | `test_mcp_server_stdio_config`, `test_mcp_server_with_env`, `test_mcp_server_remote_url`, `test_mcp_server_disabled`, `test_mcp_multiple_servers`, `test_mcp_full_config_with_agents`                                                                                                                                                                                                                                                                                                                                                                        |
| REQ-CS-011  | `McpServerConfig` serde attrs (config.rs:253-282)                 | `test_mcp_server_config_serialization`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| REQ-CS-012  | `Config::all_gitignore_entries()` (config.rs:351-393)             | `test_all_gitignore_entries_collects_destinations`, `test_all_gitignore_entries_skips_disabled_agents`, `test_all_gitignore_entries_deduplicates`, `test_all_gitignore_entries_sorted`, `test_all_gitignore_entries_includes_backup_patterns`, `test_all_gitignore_entries_includes_known_patterns`, `test_all_gitignore_entries_disabled_agents_no_known_patterns`, `test_all_gitignore_entries_deduplicates_known_patterns`, `test_all_gitignore_entries_manual_entries_plus_known`, `test_all_gitignore_entries_includes_known_patterns_for_alias_agents` |
| REQ-CS-013  | `normalize_managed_gitignore_entry()` (config.rs:402-413)         | `test_all_gitignore_entries_root_level_known_patterns_are_root_scoped`, `test_all_gitignore_entries_root_destinations_and_backups_are_root_scoped`, `test_all_gitignore_entries_manual_bare_entry_remains_unchanged`                                                                                                                                                                                                                                                                                                                                         |
| REQ-CS-014  | `all_gitignore_entries()` NestedGlob skip (config.rs:362-364)     | `test_nested_glob_destination_not_added_to_gitignore`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| REQ-CS-015  | `all_gitignore_entries()` ModuleMap expansion (config.rs:366-377) | `test_all_gitignore_entries_module_map_expands_mappings`, `test_all_gitignore_entries_module_map_disabled_agent_skipped`, `test_all_gitignore_entries_module_map_with_filename_override`, `test_all_gitignore_entries_module_map_deduplicates_expanded_entries`                                                                                                                                                                                                                                                                                              |
| REQ-CS-016  | `resolve_module_map_filename()` (config.rs:171-182)               | `test_resolve_module_map_filename_override`, `test_resolve_module_map_filename_convention_claude`, `test_resolve_module_map_filename_convention_is_case_insensitive`, `test_resolve_module_map_filename_convention_copilot`, `test_resolve_module_map_filename_fallback_unknown_agent`, `test_resolve_module_map_filename_override_beats_convention`                                                                                                                                                                                                         |
| REQ-CS-017  | `agent_ids::known_ignore_patterns()` (agent_ids.rs:93-141)        | `test_known_ignore_patterns_claude`, `test_known_ignore_patterns_copilot`, `test_known_ignore_patterns_codex`, `test_known_ignore_patterns_codex_aliases`, `test_known_ignore_patterns_gemini`, `test_known_ignore_patterns_opencode`, `test_known_ignore_patterns_cursor`, `test_known_ignore_patterns_vscode`, `test_known_ignore_patterns_case_insensitive`, `test_known_ignore_patterns_unknown_agent`                                                                                                                                                   |
| REQ-CS-018  | `TargetConfig` defaults (config.rs:86-129)                        | `test_parse_target_config_without_mappings_defaults`, `test_parse_config_with_defaults`                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| REQ-CS-019  | `Config::compress_agents_md` (config.rs:28-29)                    | `test_parse_empty_config`, `test_parse_compress_agents_md_enabled`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| REQ-CS-020  | `main.rs` Apply/Clean commands (main.rs:171-283)                  | Integration via CLI                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
