# Delta for Agent Adoption

This delta spec extends the existing `skill-adoption` specification to cover universal agent adoption — detecting and migrating skills, commands, instruction files, and MCP configs from all known agents, not just Claude.

## ADDED Requirements

### Requirement: Scan Detects Agent Skill Directories

The `AgentFileType` enum MUST include variants for each agent's skill directory: `CursorSkills`, `CodexSkills`, `GeminiSkills`, `OpenCodeSkills`, `RooSkills`, `FactorySkills`, `VibeSkills`, `AntigravitySkills`.

The `scan_agent_files()` function MUST detect each agent's skill directory when it exists as a directory and contains at least one entry.

The function MUST NOT report a skill directory that is empty (zero entries).

#### Scenario: Scan detects Cursor skills directory

- GIVEN a project directory containing `.cursor/skills/my-cursor-skill/SKILL.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to `AgentFileType::CursorSkills`
- AND the `path` field MUST be `.cursor/skills`
- AND the `display_name` MUST contain "Cursor" and "skills"

#### Scenario: Scan detects Gemini skills directory

- GIVEN a project directory containing `.gemini/skills/data-analysis/SKILL.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to `AgentFileType::GeminiSkills`
- AND the `path` field MUST be `.gemini/skills`

#### Scenario: Scan detects OpenCode skills directory

- GIVEN a project directory containing `.opencode/skills/my-skill/SKILL.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to `AgentFileType::OpenCodeSkills`
- AND the `path` field MUST be `.opencode/skills`

#### Scenario: Scan detects skills from multiple agents simultaneously

- GIVEN a project directory containing `.claude/skills/skill-a/SKILL.md`, `.cursor/skills/skill-b/SKILL.md`, and `.codex/skills/skill-c/SKILL.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include entries for `ClaudeSkills`, `CursorSkills`, and `CodexSkills`
- AND each entry MUST have a distinct `path` value

#### Scenario: Scan ignores empty agent skill directory

- GIVEN a project directory containing `.gemini/skills/` as an empty directory
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST NOT include any entry with `file_type` equal to `AgentFileType::GeminiSkills`

#### Scenario: Scan detects skills inside a directory also copied as-is

- GIVEN a project directory containing `.cursor/rules/some-rule.mdc` and `.cursor/skills/my-skill/SKILL.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include an entry for `CursorDirectory` (the parent `.cursor/` dir)
- AND the result MUST include a separate entry for `CursorSkills` (`.cursor/skills/`)

---

### Requirement: Wizard Migrates Skills from All Agents

When the init wizard discovers any agent skill directory (any `*Skills` variant) and the user selects it for migration, the wizard MUST copy each immediate child of that skill directory into `.agents/skills/`.

The wizard MUST handle name collisions: if a child name already exists in `.agents/skills/`, the wizard MUST skip that child and print a warning message identifying the conflicting name and source agent.

The wizard MUST process skill directories in scan order. If skills from an earlier-scanned agent are migrated first, later agents with same-named skills SHALL be skipped with warnings.

#### Scenario: Wizard migrates Cursor skills into .agents/skills

- GIVEN a project directory containing `.cursor/skills/cursor-tool/SKILL.md`
- AND no `.agents/` directory exists
- WHEN the user runs `agentsync init --wizard`
- AND the user selects `.cursor/skills/` for migration
- THEN `.agents/skills/cursor-tool/SKILL.md` MUST exist with the same content as the original

#### Scenario: Wizard migrates skills from multiple agents

- GIVEN a project with `.claude/skills/shared-lib/SKILL.md` and `.gemini/skills/data-helper/SKILL.md`
- AND no `.agents/` directory exists
- WHEN the user runs `agentsync init --wizard`
- AND the user selects both skill directories for migration
- THEN `.agents/skills/shared-lib/SKILL.md` MUST exist
- AND `.agents/skills/data-helper/SKILL.md` MUST exist

#### Scenario: Wizard handles cross-agent skill name collision

- GIVEN a project with `.claude/skills/common/SKILL.md` and `.cursor/skills/common/SKILL.md` (different content)
- AND no `.agents/` directory exists
- WHEN the user runs `agentsync init --wizard`
- AND the user selects both skill directories for migration
- THEN `.agents/skills/common/` MUST contain the content from the first-processed agent
- AND a warning message MUST be printed that includes the name "common"
- AND the warning MUST indicate the skill was skipped due to a collision

#### Scenario: Wizard skips empty skill directories during migration

- GIVEN a project with `.roo/skills/` existing but empty
- WHEN `scan_agent_files()` is called
- THEN no `RooSkills` entry SHALL appear in the results
- AND the wizard SHALL NOT offer an empty directory for migration

---

### Requirement: Scan Detects Command Directories

The `AgentFileType` enum MUST include variants: `ClaudeCommands`, `GeminiCommands`, `OpenCodeCommands`.

The `scan_agent_files()` function MUST detect command directories when they exist and contain at least one entry:
- `.claude/commands/` for `ClaudeCommands`
- `.gemini/commands/` for `GeminiCommands`
- `.opencode/command/` for `OpenCodeCommands` (note: singular "command")

#### Scenario: Scan detects Claude commands directory

- GIVEN a project directory containing `.claude/commands/review.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to `AgentFileType::ClaudeCommands`
- AND the `path` field MUST be `.claude/commands`
- AND the `display_name` MUST contain "Claude" and "commands"

#### Scenario: Scan detects OpenCode command directory

- GIVEN a project directory containing `.opencode/command/deploy.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to `AgentFileType::OpenCodeCommands`
- AND the `path` field MUST be `.opencode/command`

#### Scenario: Scan ignores empty command directory

- GIVEN a project directory containing `.claude/commands/` as an empty directory
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST NOT include any entry with `file_type` equal to `AgentFileType::ClaudeCommands`

---

### Requirement: Wizard Migrates Commands to Canonical Location

The wizard MUST create `.agents/commands/` as a canonical commands directory during `init()`, alongside the existing `.agents/skills/` directory.

When the wizard discovers command directories and the user selects them for migration, it MUST copy each immediate child file into `.agents/commands/`.

Collision handling MUST follow the same pattern as skills: skip and warn on name conflicts.

#### Scenario: Init creates commands directory

- GIVEN a project directory with no `.agents/` directory
- WHEN the user runs `agentsync init`
- THEN `.agents/commands/` MUST be created as an empty directory

#### Scenario: Wizard migrates Claude commands into .agents/commands

- GIVEN a project directory containing `.claude/commands/review.md` and `.claude/commands/deploy.md`
- AND no `.agents/` directory exists
- WHEN the user runs `agentsync init --wizard`
- AND the user selects `.claude/commands/` for migration
- THEN `.agents/commands/review.md` MUST exist with the same content as the original
- AND `.agents/commands/deploy.md` MUST exist with the same content as the original

#### Scenario: Wizard migrates commands from multiple agents

- GIVEN a project with `.claude/commands/review.md` and `.gemini/commands/analyze.md`
- WHEN the user runs `agentsync init --wizard`
- AND the user selects both command directories for migration
- THEN `.agents/commands/review.md` MUST exist
- AND `.agents/commands/analyze.md` MUST exist

#### Scenario: Wizard handles command name collision across agents

- GIVEN a project with `.claude/commands/deploy.md` and `.gemini/commands/deploy.md` (different content)
- WHEN the user runs `agentsync init --wizard`
- AND the user selects both command directories for migration
- THEN `.agents/commands/deploy.md` MUST contain the content from the first-processed agent
- AND a warning message MUST be printed that includes the name "deploy"
- AND the warning MUST indicate the command file was skipped due to a collision

#### Scenario: Apply syncs commands to .claude/commands via symlink-contents

- GIVEN a freshly initialized project with `[agents.claude.targets.commands]` configured
- AND `.agents/commands/` contains `review.md`
- WHEN the user runs `agentsync apply`
- THEN a symlink MUST be created at `.claude/commands/review.md` pointing to `.agents/commands/review.md`

---

### Requirement: Scan Detects Missing Instruction Files

The `scan_agent_files()` function MUST detect the following instruction files when they exist at the project root:
- `.windsurfrules` → `AgentFileType::WindsurfRules`
- `OPENCODE.md` → a new `AgentFileType::OpenCodeInstructions` variant
- `AMPCODE.md` → `AgentFileType::AmpInstructions` (already in enum, wire scan)

These instruction files, when detected, MUST be included in the merged `AGENTS.md` during wizard migration, following the same pattern as existing instruction file types.

#### Scenario: Scan detects .windsurfrules file

- GIVEN a project directory containing a `.windsurfrules` file with content
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to `AgentFileType::WindsurfRules`
- AND the `path` field MUST be `.windsurfrules`

#### Scenario: Scan detects OPENCODE.md file

- GIVEN a project directory containing an `OPENCODE.md` file with content
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to `AgentFileType::OpenCodeInstructions`
- AND the `path` field MUST be `OPENCODE.md`

#### Scenario: Scan detects AMPCODE.md file

- GIVEN a project directory containing an `AMPCODE.md` file with content
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to `AgentFileType::AmpInstructions`
- AND the `path` field MUST be `AMPCODE.md`

#### Scenario: Wizard merges newly-detected instruction file into AGENTS.md

- GIVEN a project with `.windsurfrules` containing "Use TypeScript strict mode"
- AND no `.agents/` directory exists
- WHEN the user runs `agentsync init --wizard`
- AND the user selects `.windsurfrules` for migration
- THEN `.agents/AGENTS.md` MUST contain the text "Use TypeScript strict mode"

#### Scenario: Scan finds instruction files alongside existing detections

- GIVEN a project with `CLAUDE.md`, `.windsurfrules`, and `OPENCODE.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include entries for `ClaudeInstructions`, `WindsurfRules`, and `OpenCodeInstructions`

---

### Requirement: Scan Detects Agent-Specific MCP Configs

The `AgentFileType` enum MUST include variants for agent-specific MCP configuration files: `CursorMcpConfig`, `WindsurfMcpConfig`, `CodexConfig`, `RooMcpConfig`, `KiroMcpConfig`, `AmazonQMcpConfig`, `KilocodeMcpConfig`, `FactoryMcpConfig`, `OpenCodeConfig`.

The `scan_agent_files()` function MUST detect these files when they exist:
- `.cursor/mcp.json` → `CursorMcpConfig`
- `.windsurf/mcp_config.json` → `WindsurfMcpConfig`
- `.codex/config.toml` → `CodexConfig`
- `.roo/mcp.json` → `RooMcpConfig`
- `.kiro/settings/mcp.json` → `KiroMcpConfig`
- `.amazonq/mcp.json` → `AmazonQMcpConfig`
- `.kilocode/mcp.json` → `KilocodeMcpConfig`
- `.factory/mcp.json` → `FactoryMcpConfig`
- `opencode.json` → `OpenCodeConfig`

The wizard MUST NOT attempt to parse or import these configs. It MUST only note their existence and print a message suggesting the user configure MCP servers in `agentsync.toml`.

#### Scenario: Scan detects Cursor MCP config

- GIVEN a project directory containing `.cursor/mcp.json` with valid JSON content
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to `AgentFileType::CursorMcpConfig`
- AND the `path` field MUST be `.cursor/mcp.json`

#### Scenario: Scan detects Windsurf MCP config

- GIVEN a project directory containing `.windsurf/mcp_config.json` with content
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to `AgentFileType::WindsurfMcpConfig`

#### Scenario: Scan detects OpenCode config at project root

- GIVEN a project directory containing `opencode.json` with content
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to `AgentFileType::OpenCodeConfig`
- AND the `path` field MUST be `opencode.json`

#### Scenario: Wizard notes MCP config with migration suggestion

- GIVEN a project with `.cursor/mcp.json` detected
- WHEN the user runs `agentsync init --wizard`
- THEN the wizard MUST print a note message containing the path `.cursor/mcp.json`
- AND the note MUST suggest configuring MCP servers in `agentsync.toml`
- AND the wizard MUST NOT attempt to parse the JSON content
- AND the wizard MUST NOT copy the file into `.agents/`

#### Scenario: Wizard notes multiple MCP configs

- GIVEN a project with `.cursor/mcp.json`, `.roo/mcp.json`, and `opencode.json`
- WHEN the user runs `agentsync init --wizard`
- THEN the wizard MUST print a note for each detected MCP config file
- AND each note MUST contain the respective file path

#### Scenario: MCP config inside directory that is also copied as-is

- GIVEN a project with `.cursor/rules/my-rule.mdc` (triggers `CursorDirectory` detection) and `.cursor/mcp.json`
- WHEN `scan_agent_files()` is called
- THEN the result MUST include both `CursorDirectory` and `CursorMcpConfig` as separate entries
- AND during wizard migration, the MCP config SHALL be noted (not duplicated into `.agents/`)

---

### Requirement: DEFAULT_CONFIG Includes Commands Target and New Agent Sections

The `DEFAULT_CONFIG` static string MUST include a `[agents.claude.targets.commands]` entry with:
- `source = "commands"`
- `destination = ".claude/commands"`
- `type = "symlink-contents"`

The `DEFAULT_CONFIG` MUST include new agent sections for `gemini` and `opencode` with at minimum `instructions` and `skills` targets.

The `DEFAULT_CONFIG` MUST remain valid TOML that parses into a `Config` struct without errors.

#### Scenario: DEFAULT_CONFIG contains Claude commands target

- GIVEN the `DEFAULT_CONFIG` constant
- WHEN it is parsed as TOML into a `Config` struct
- THEN the `claude` agent MUST have a `commands` target in its `targets` map
- AND the `commands` target MUST have `source` equal to `"commands"`
- AND the `commands` target MUST have `destination` equal to `".claude/commands"`
- AND the `commands` target MUST have `sync_type` equal to `SymlinkContents`

#### Scenario: DEFAULT_CONFIG contains Gemini agent section

- GIVEN the `DEFAULT_CONFIG` constant
- WHEN it is parsed as TOML into a `Config` struct
- THEN there MUST be an `agents` entry keyed `"gemini"`
- AND the `gemini` agent MUST have an `instructions` target with `destination` containing `"GEMINI.md"`
- AND the `gemini` agent MUST have a `skills` target with `destination` containing `".gemini/skills"`

#### Scenario: DEFAULT_CONFIG contains OpenCode agent section

- GIVEN the `DEFAULT_CONFIG` constant
- WHEN it is parsed as TOML into a `Config` struct
- THEN there MUST be an `agents` entry keyed `"opencode"`
- AND the `opencode` agent MUST have an `instructions` target with `destination` containing `"OPENCODE.md"`
- AND the `opencode` agent MUST have a `skills` target with `destination` containing `".opencode/skills"`

#### Scenario: Fresh init with no existing agent files uses updated DEFAULT_CONFIG

- GIVEN a project directory with no agent files at all (no `.claude/`, no `.cursor/`, no instruction files)
- WHEN the user runs `agentsync init`
- THEN the generated `.agents/agentsync.toml` MUST contain sections for `claude`, `copilot`, `cursor`, `codex`, `gemini`, `opencode`, and `root`
- AND the `claude` section MUST include `instructions`, `skills`, and `commands` targets
- AND `.agents/commands/` MUST exist as an empty directory

#### Scenario: DEFAULT_CONFIG remains valid TOML after updates

- GIVEN the updated `DEFAULT_CONFIG` constant
- WHEN it is parsed as TOML into a `Config` struct
- THEN parsing MUST succeed without errors
- AND all existing agent sections (`claude`, `copilot`, `cursor`, `codex`, `root`) MUST still be present
- AND no existing target definitions SHALL be removed or altered

## MODIFIED Requirements

### Requirement: Wizard Migrates Claude Skills (Updated)

(Previously: Only `ClaudeSkills` variant was handled in the skill migration match arm.)

The skill migration match arm MUST handle ALL `*Skills` variants (`ClaudeSkills`, `CursorSkills`, `CodexSkills`, `GeminiSkills`, `OpenCodeSkills`, `RooSkills`, `FactorySkills`, `VibeSkills`, `AntigravitySkills`) using the same copy-contents-with-collision-detection pattern.

All skill variants MUST share the same migration logic: iterate immediate children, check for collision in `.agents/skills/`, skip with warning on collision, copy on no collision.

#### Scenario: All skill variants use same migration pattern

- GIVEN a project with `.codex/skills/codex-tool/SKILL.md`
- WHEN the wizard migrates `CodexSkills`
- THEN the migration behavior MUST be identical to `ClaudeSkills` migration
- AND `.agents/skills/codex-tool/SKILL.md` MUST exist with the original content

### Requirement: MCP Config Note Handling (Updated)

(Previously: Only `McpConfig` and `ZedSettings` were noted during migration.)

The noted-but-not-migrated match arm MUST handle ALL agent-specific MCP config variants (`CursorMcpConfig`, `WindsurfMcpConfig`, `CodexConfig`, `RooMcpConfig`, `KiroMcpConfig`, `AmazonQMcpConfig`, `KilocodeMcpConfig`, `FactoryMcpConfig`, `OpenCodeConfig`) in addition to the existing `McpConfig` and `ZedSettings`.

Each noted config MUST print a message containing the detected file path and a suggestion to configure MCP servers in `agentsync.toml`.

#### Scenario: All MCP config variants are noted consistently

- GIVEN a project with `.kiro/settings/mcp.json` and `.factory/mcp.json`
- WHEN the wizard processes these during migration
- THEN a note message MUST be printed for each
- AND each note MUST contain the respective file path
- AND no file SHALL be copied into `.agents/`

## Acceptance Criteria

1. `scan_agent_files()` detects skill directories for all 8 agents (Cursor, Codex, Gemini, OpenCode, Roo, Factory, Vibe, Antigravity) when they exist with content, and ignores them when empty or absent.
2. `scan_agent_files()` detects command directories for Claude, Gemini, and OpenCode when they exist with content.
3. `scan_agent_files()` detects `.windsurfrules`, `OPENCODE.md`, and `AMPCODE.md` as instruction files.
4. `scan_agent_files()` detects 9 agent-specific MCP config files when present.
5. The wizard migrates all agent skill directories into `.agents/skills/` with collision detection (warn + skip).
6. The wizard migrates command directories into `.agents/commands/` with collision detection.
7. The wizard notes all detected MCP configs with migration suggestion messages without parsing or copying them.
8. `init()` creates `.agents/commands/` alongside `.agents/skills/`.
9. `DEFAULT_CONFIG` includes `[agents.claude.targets.commands]` with correct source, destination, and type.
10. `DEFAULT_CONFIG` includes `gemini` and `opencode` agent sections with instructions and skills targets.
11. `DEFAULT_CONFIG` remains valid TOML that parses into a `Config` struct.
12. All existing tests pass (no regressions).
13. New tests cover each new scan entry, migration path, collision scenario, and config parsing.
