# Skill Adoption Specification

## Purpose

Defines how AgentSync detects, migrates, and manages skills, commands, instruction files, and MCP configs from all known agents so that `.agents/skills/` and `.agents/commands/` are the single sources of truth and artifacts are automatically symlinked to agent-specific locations out of the box.

## Requirements

### Requirement: Default Config Includes Claude Skills Target

The `DEFAULT_CONFIG` template MUST include an `[agents.claude.targets.skills]` entry that maps the `skills` source directory to `.claude/skills` using the `symlink` sync type.

The entry MUST be placed within the existing `[agents.claude]` section, after the `[agents.claude.targets.instructions]` entry.

The `DEFAULT_CONFIG` template MUST remain valid TOML that parses into a `Config` struct without errors.

#### Scenario: Fresh init generates config with directory symlink for skills

- GIVEN a project directory with no `.agents/` directory
- WHEN the user runs `agentsync init`
- THEN the generated `.agents/agentsync.toml` MUST contain an `[agents.claude.targets.skills]` section
- AND the section MUST specify `source = "skills"`
- AND the section MUST specify `destination = ".claude/skills"`
- AND the section MUST specify `type = "symlink"`

#### Scenario: Fresh init config parses with symlink type for skills

- GIVEN the `DEFAULT_CONFIG` constant
- WHEN it is parsed as TOML into a `Config` struct
- THEN the `claude` agent MUST have a `skills` target in its `targets` map
- AND the `skills` target MUST have `sync_type` equal to `Symlink`

#### Scenario: Apply with empty skills directory

- GIVEN a freshly initialized project with skills target `type = "symlink"`
- AND `.agents/skills/` exists but is empty
- WHEN the user runs `agentsync apply`
- THEN `.claude/skills` MUST be a symlink pointing to `.agents/skills`
- AND the command MUST NOT produce an error

---

### Requirement: Default Config Uses Symlink Type for All Agent Skills Targets

The `DEFAULT_CONFIG` template MUST specify `type = "symlink"` for the skills target of every agent that has one (claude, codex, gemini, opencode, and any other agent with a skills target in the template).

The `commands` targets MUST remain `type = "symlink-contents"` — this change applies only to skills.

#### Scenario: All agent skills targets use symlink type in fresh init

- GIVEN the `DEFAULT_CONFIG` constant
- WHEN it is parsed as TOML into a `Config` struct
- THEN the `claude` agent's `skills` target MUST have `sync_type` equal to `Symlink`
- AND the `codex` agent's `skills` target MUST have `sync_type` equal to `Symlink`
- AND the `gemini` agent's `skills` target MUST have `sync_type` equal to `Symlink`
- AND the `opencode` agent's `skills` target MUST have `sync_type` equal to `Symlink`

#### Scenario: Commands targets remain symlink-contents

- GIVEN the `DEFAULT_CONFIG` constant
- WHEN it is parsed as TOML into a `Config` struct
- THEN the `claude` agent's `commands` target MUST have `sync_type` equal to `SymlinkContents`

---

### Requirement: This Repo's Config Uses Symlink Type for Skills

The `.agents/agentsync.toml` in this repository MUST use `type = "symlink"` for all skills targets.

#### Scenario: Repo config specifies symlink for opencode skills

- GIVEN the `.agents/agentsync.toml` in this repository
- WHEN the opencode agent's skills target is inspected
- THEN `type` MUST equal `"symlink"`

#### Scenario: Repo config specifies symlink for copilot skills

- GIVEN the `.agents/agentsync.toml` in this repository
- WHEN the copilot agent's skills target is inspected
- THEN `type` MUST equal `"symlink"`

---

### Requirement: Apply Creates Directory Symlink for Skills

When a skills target has `type = "symlink"` and the source is a directory, the `apply` command MUST create a single symlink at the destination pointing to the source directory. The destination itself MUST be a symlink, not a real directory.

#### Scenario: Apply creates single directory symlink for skills

- GIVEN a freshly initialized project with skills target `type = "symlink"`
- AND `.agents/skills/` contains `my-skill/SKILL.md`
- WHEN the user runs `agentsync apply`
- THEN `.claude/skills` MUST be a symlink pointing to the `.agents/skills` directory
- AND `.claude/skills` MUST NOT be a real directory
- AND `.claude/skills/my-skill/SKILL.md` MUST be readable through the symlink

#### Scenario: New skill visible without re-running sync

- GIVEN a project where `agentsync apply` has already run with skills target `type = "symlink"`
- AND `.claude/skills` is a directory symlink pointing to `.agents/skills`
- WHEN a new directory `.agents/skills/new-skill/` is created with a `SKILL.md` file
- THEN `.claude/skills/new-skill/SKILL.md` MUST be immediately readable without running `agentsync apply` again

#### Scenario: Renamed skill dir visible without re-running sync

- GIVEN a project where `agentsync apply` has already run with skills target `type = "symlink"`
- AND `.agents/skills/old-name/` exists
- WHEN `.agents/skills/old-name/` is renamed to `.agents/skills/new-name/`
- THEN `.claude/skills/new-name/` MUST be immediately visible
- AND `.claude/skills/old-name/` MUST NOT exist
- AND no stale symlink or entry SHALL remain at the old name

#### Scenario: Deleted skill dir disappears without re-running sync

- GIVEN a project where `agentsync apply` has already run with skills target `type = "symlink"`
- AND `.agents/skills/stale-skill/` exists
- WHEN `.agents/skills/stale-skill/` is deleted
- THEN `.claude/skills/stale-skill/` MUST NOT exist
- AND no stale entry SHALL remain

---

### Requirement: Backward Compatibility with Existing symlink-contents Configs

Projects with existing `agentsync.toml` configs that specify `type = "symlink-contents"` for skills targets MUST continue to work without modification. The `symlink-contents` sync type MUST NOT be deprecated, removed, or have its behavior altered by this change.

#### Scenario: Existing symlink-contents config continues to work

- GIVEN a project with `.agents/agentsync.toml` specifying `type = "symlink-contents"` for the claude skills target
- AND `.agents/skills/` contains `skill-a/SKILL.md`
- WHEN the user runs `agentsync apply`
- THEN `.claude/skills/` MUST be a real directory (not a symlink)
- AND `.claude/skills/skill-a` MUST be an individual symlink pointing to `.agents/skills/skill-a`

#### Scenario: Updated binary with old config produces no change

- GIVEN a project initialized with an older agentsync version (config says `type = "symlink-contents"` for skills)
- WHEN the user updates the agentsync binary but does NOT edit their config
- AND the user runs `agentsync apply`
- THEN the behavior MUST be identical to the previous version
- AND no directory symlinks SHALL be created for skills

---

### Requirement: Clean Transition from symlink-contents to symlink

When a user changes their skills target type from `symlink-contents` to `symlink` and runs `agentsync sync --clean`, the clean operation MUST remove the old per-entry symlinks and their containing real directory, and the subsequent sync MUST create a single directory symlink.

#### Scenario: Clean + sync transitions from per-skill symlinks to directory symlink

- GIVEN a project previously synced with `type = "symlink-contents"` for skills
- AND `.claude/skills/` is a real directory containing individual symlinks (`skill-a` → `../../.agents/skills/skill-a`)
- AND the user has changed the config to `type = "symlink"`
- WHEN the user runs `agentsync sync --clean`
- THEN the individual symlinks inside `.claude/skills/` MUST be removed
- AND the `.claude/skills/` real directory MUST be removed
- AND a new symlink MUST be created at `.claude/skills` pointing to `.agents/skills`

#### Scenario: Backup of existing real directory at destination

- GIVEN a project where `.claude/skills/` is a real directory containing non-symlink files (user-created content)
- AND the config specifies `type = "symlink"` for the skills target
- WHEN the user runs `agentsync apply`
- THEN the existing `.claude/skills/` directory MUST be renamed to `.claude/skills.bak`
- AND a new symlink MUST be created at `.claude/skills` pointing to `.agents/skills`
- AND no user-created content SHALL be lost

---

### Requirement: Dry-run Reports Symlink Strategy

The `--dry-run` output MUST clearly indicate the sync strategy being used for each target, distinguishing between `symlink` (directory symlink) and `symlink-contents` (per-entry symlinks).

#### Scenario: Dry-run output shows symlink strategy for skills

- GIVEN a project with skills target `type = "symlink"`
- WHEN the user runs `agentsync apply --dry-run`
- THEN the output MUST include information about the skills target
- AND the output MUST indicate that a directory symlink will be created (not per-entry symlinks)

#### Scenario: Dry-run output shows symlink-contents strategy for commands

- GIVEN a project with commands target `type = "symlink-contents"`
- WHEN the user runs `agentsync apply --dry-run`
- THEN the output for the commands target MUST indicate that per-entry symlinks will be created

---

### Requirement: Deterministic Failure When Directory Symlink Cannot Be Created

When `agentsync apply` attempts to create a directory symlink for a target with `type = "symlink"` and the symlink creation fails (e.g., due to insufficient permissions, filesystem limitations, or Windows requiring elevated privileges for `symlink_dir`), the command MUST emit a clear error message that includes the target path and the underlying OS error, and MUST exit with a non-zero status code. There is no silent fallback to `symlink-contents` behavior.

#### Scenario: Symlink creation fails due to permissions

- GIVEN a project with skills target `type = "symlink"`
- AND the filesystem or OS prevents symlink creation at the destination path
- WHEN the user runs `agentsync apply`
- THEN the command MUST exit with a non-zero status code
- AND the error output MUST include "Failed to create symlink" and the destination path
- AND no partial or fallback symlink layout SHALL be created

#### Scenario: Dry-run reports strategy even when creation would fail

- GIVEN a project with skills target `type = "symlink"`
- WHEN the user runs `agentsync apply --dry-run`
- THEN the output MUST report the intended symlink strategy
- AND the command MUST exit with zero status (dry-run does not attempt creation)

---

### Requirement: Scan Detects Claude Skills Directory

The `AgentFileType` enum MUST include a `ClaudeSkills` variant representing the `.claude/skills/` directory.

The `scan_agent_files()` function MUST detect `.claude/skills/` when it exists as a directory and contains at least one entry (file or subdirectory).

#### Scenario: Scan finds .claude/skills with content

- GIVEN a project directory containing `.claude/skills/my-skill/SKILL.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to `AgentFileType::ClaudeSkills`
- AND the `path` field MUST be `.claude/skills`
- AND the `display_name` MUST contain "Claude" and "skills"

#### Scenario: Scan ignores empty .claude/skills directory

- GIVEN a project directory containing `.claude/skills/` as an empty directory
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST NOT include any entry with `file_type` equal to `AgentFileType::ClaudeSkills`

#### Scenario: Scan ignores absent .claude/skills

- GIVEN a project directory with no `.claude/` directory
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST NOT include any entry with `file_type` equal to `AgentFileType::ClaudeSkills`

#### Scenario: Scan detects .claude/skills alongside CLAUDE.md

- GIVEN a project directory containing both `CLAUDE.md` and `.claude/skills/my-skill/SKILL.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include an entry for `ClaudeInstructions` (CLAUDE.md)
- AND the result MUST include a separate entry for `ClaudeSkills` (.claude/skills)

---

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

The skill migration match arm MUST handle ALL `*Skills` variants (`ClaudeSkills`, `CursorSkills`, `CodexSkills`, `GeminiSkills`, `OpenCodeSkills`, `RooSkills`, `FactorySkills`, `VibeSkills`, `AntigravitySkills`) using the same copy-contents-with-collision-detection pattern.

The wizard MUST NOT modify or delete the original skill directories during migration (deletion happens only if the user opts into the backup step).

#### Scenario: Wizard migrates skills from .claude/skills to .agents/skills

- GIVEN a project directory containing `.claude/skills/skill-a/SKILL.md` and `.claude/skills/skill-b/SKILL.md`
- AND no `.agents/` directory exists
- WHEN the user runs `agentsync init --wizard`
- AND the user selects `.claude/skills/` for migration
- THEN `.agents/skills/skill-a/SKILL.md` MUST exist with the same content as the original
- AND `.agents/skills/skill-b/SKILL.md` MUST exist with the same content as the original

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

#### Scenario: Wizard handles skill name collision

- GIVEN a project directory containing `.claude/skills/shared-skill/SKILL.md`
- AND `.agents/skills/shared-skill/SKILL.md` already exists with different content
- WHEN the user runs `agentsync init --wizard`
- AND the user selects `.claude/skills/` for migration
- THEN `.agents/skills/shared-skill/` MUST NOT be overwritten
- AND a warning message MUST be printed that includes the name "shared-skill"
- AND the warning MUST indicate the skill was skipped due to a collision

#### Scenario: Wizard handles cross-agent skill name collision

- GIVEN a project with `.claude/skills/common/SKILL.md` and `.cursor/skills/common/SKILL.md` (different content)
- AND no `.agents/` directory exists
- WHEN the user runs `agentsync init --wizard`
- AND the user selects both skill directories for migration
- THEN `.agents/skills/common/` MUST contain the content from the first-processed agent
- AND a warning message MUST be printed that includes the name "common"
- AND the warning MUST indicate the skill was skipped due to a collision

#### Scenario: Wizard with no skills selected for migration

- GIVEN a project directory containing `.claude/skills/skill-a/SKILL.md`
- AND no `.agents/` directory exists
- WHEN the user runs `agentsync init --wizard`
- AND the user deselects `.claude/skills/` from the migration selection
- THEN `.agents/skills/` MUST be created (standard init behavior)
- AND `.agents/skills/` MUST NOT contain `skill-a/`

#### Scenario: Wizard with .claude/skills containing mixed content

- GIVEN `.claude/skills/` contains a subdirectory `valid-skill/` and a loose file `notes.txt`
- WHEN the user selects `.claude/skills/` for migration during the wizard
- THEN both `valid-skill/` and `notes.txt` MUST be copied into `.agents/skills/`

#### Scenario: Wizard skips empty skill directories during migration

- GIVEN a project with `.roo/skills/` existing but empty
- WHEN `scan_agent_files()` is called
- THEN no `RooSkills` entry SHALL appear in the results
- AND the wizard SHALL NOT offer an empty directory for migration

#### Scenario: All skill variants use same migration pattern

- GIVEN a project with `.codex/skills/codex-tool/SKILL.md`
- WHEN the wizard migrates `CodexSkills`
- THEN the migration behavior MUST be identical to `ClaudeSkills` migration
- AND `.agents/skills/codex-tool/SKILL.md` MUST exist with the original content

#### Scenario: Re-init on already-initialized project without force

- GIVEN a project directory that has already been initialized (`.agents/agentsync.toml` exists)
- AND `.claude/skills/new-skill/SKILL.md` exists
- WHEN the user runs `agentsync init --wizard` without `--force`
- THEN the existing `.agents/agentsync.toml` MUST NOT be overwritten
- AND a message MUST be printed indicating the config already exists
- AND the wizard MAY still offer to migrate skills into `.agents/skills/`

#### Scenario: Re-init with --force overwrites config

- GIVEN a project that was previously initialized with an old config (no `[agents.claude.targets.skills]`)
- WHEN the user runs `agentsync init --force`
- THEN `.agents/agentsync.toml` MUST be overwritten with the updated `DEFAULT_CONFIG`
- AND the new config MUST contain `[agents.claude.targets.skills]`

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

The noted-but-not-migrated match arm MUST handle ALL agent-specific MCP config variants in addition to the existing `McpConfig` and `ZedSettings`.

Each noted config MUST print a message containing the detected file path and a suggestion to configure MCP servers in `agentsync.toml`.

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

#### Scenario: All MCP config variants are noted consistently

- GIVEN a project with `.kiro/settings/mcp.json` and `.factory/mcp.json`
- WHEN the wizard processes these during migration
- THEN a note message MUST be printed for each
- AND each note MUST contain the respective file path
- AND no file SHALL be copied into `.agents/`

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

---

### Requirement: Apply-Time Diagnostic for Unmanaged Claude Skills

After processing all sync targets, the `apply` command SHOULD check whether `.claude/skills/` exists at the project root, contains at least one entry, and is not the destination of any enabled target in the current configuration.

If all three conditions are true, the system MUST print a warning message to standard output.

The warning MUST NOT be treated as an error (the apply command MUST still exit successfully).

The diagnostic MUST be suppressed when any enabled target already has `.claude/skills` (or a path that resolves to it) as its destination.

#### Scenario: Apply warns about unmanaged .claude/skills

- GIVEN a project with `.agents/agentsync.toml` that does NOT have a target with destination `.claude/skills`
- AND `.claude/skills/` exists and contains `orphan-skill/SKILL.md`
- WHEN the user runs `agentsync apply`
- THEN the command MUST print a warning containing ".claude/skills"
- AND the warning SHOULD suggest running `agentsync init --wizard` to adopt
- AND the apply command MUST exit with a success status

#### Scenario: Apply does not warn when .claude/skills is managed

- GIVEN a project with `.agents/agentsync.toml` containing `[agents.claude.targets.skills]` with `destination = ".claude/skills"`
- AND `.claude/skills/` exists with content (created by symlink-contents)
- WHEN the user runs `agentsync apply`
- THEN no warning about unmanaged `.claude/skills` SHALL be printed

#### Scenario: Apply does not warn when .claude/skills is absent

- GIVEN a project with `.agents/agentsync.toml`
- AND `.claude/skills/` does not exist
- WHEN the user runs `agentsync apply`
- THEN no warning about unmanaged `.claude/skills` SHALL be printed

#### Scenario: Apply does not warn when .claude/skills is empty

- GIVEN a project with `.agents/agentsync.toml` that does NOT have a target managing `.claude/skills`
- AND `.claude/skills/` exists but is empty (no files or subdirectories)
- WHEN the user runs `agentsync apply`
- THEN no warning about unmanaged `.claude/skills` SHALL be printed

#### Scenario: Dry-run also shows unmanaged skills diagnostic

- GIVEN a project where `.claude/skills/` exists with content and no target manages it
- WHEN the user runs `agentsync apply --dry-run`
- THEN the unmanaged skills warning MUST still be printed
- AND no files SHALL be modified

## Non-functional Requirements

### Requirement: No New Dependencies for Symlink Change

The symlink-contents-to-symlink change MUST NOT introduce any new crate dependencies or external libraries. All functionality MUST use existing code paths (`SyncType::Symlink` for directory sources).

### Requirement: No New Sync Types for Symlink Change

The symlink-contents-to-symlink change MUST NOT add new variants to `SyncType` or new fields to `TargetConfig`. The existing `SyncType::Symlink` variant handles directory sources and MUST be used as-is.

### Requirement: Existing symlink-contents Behavior Unchanged

The observable semantics of `SyncType::SymlinkContents` MUST remain identical after the symlink change. Its cleanup behavior, create behavior, and pattern-filtering results MUST remain unchanged for callers and clients, and tests or acceptance criteria MUST continue validating those behaviors as the public contract.

---

## Acceptance Criteria

1. `DEFAULT_CONFIG` parses as valid TOML and includes `[agents.claude.targets.skills]` with `type = "symlink"` (directory symlink).
2. `DEFAULT_CONFIG` specifies `type = "symlink"` for all agent skills targets (claude, codex, gemini, opencode).
3. `DEFAULT_CONFIG` keeps `type = "symlink-contents"` for commands targets.
4. This repo's `.agents/agentsync.toml` uses `type = "symlink"` for all skills targets.
5. `agentsync apply` creates a directory symlink (not a real directory) for skills targets with `type = "symlink"`.
6. New skills/renames/deletions in the source directory are immediately visible through the directory symlink without re-running apply.
7. Existing `symlink-contents` configs continue to work unchanged (backward compatible).
8. `agentsync sync --clean` correctly transitions from per-entry symlinks to a directory symlink.
9. `--dry-run` output distinguishes between `symlink` and `symlink-contents` strategies.
10. `scan_agent_files()` detects `.claude/skills/` when it exists with content and ignores it when empty or absent.
11. The init wizard copies skills from all agent skill directories into `.agents/skills/`, skipping collisions with a warning.
12. `agentsync apply` prints a diagnostic warning when `.claude/skills/` has unmanaged content.
13. All existing tests continue to pass (no regressions).
14. New tests cover: template parsing with skills target, scan detection, collision handling, and diagnostic warning logic.
15. `scan_agent_files()` detects skill directories for all 8 agents (Cursor, Codex, Gemini, OpenCode, Roo, Factory, Vibe, Antigravity) when they exist with content, and ignores them when empty or absent.
16. `scan_agent_files()` detects command directories for Claude, Gemini, and OpenCode when they exist with content.
17. `scan_agent_files()` detects `.windsurfrules`, `OPENCODE.md`, and `AMPCODE.md` as instruction files.
18. `scan_agent_files()` detects the presence of 10 agent-specific MCP configuration files/settings paths (e.g., `.cursor/mcp.json`, `.vscode/mcp.json`) when present.
19. The wizard migrates command directories into `.agents/commands/` with collision detection.
20. The wizard notes all detected MCP configs with migration suggestion messages without parsing or copying them.
21. `init()` creates `.agents/commands/` alongside `.agents/skills/`.
22. `DEFAULT_CONFIG` includes `[agents.claude.targets.commands]` with correct source, destination, and type.
23. `DEFAULT_CONFIG` includes `gemini` and `opencode` agent sections with instructions and skills targets.
24. `DEFAULT_CONFIG` remains valid TOML that parses into a `Config` struct.
25. New tests cover each new scan entry, migration path, collision scenario, and config parsing.
