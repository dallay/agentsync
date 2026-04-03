# Skill Adoption Specification

## Purpose

Defines how AgentSync detects, migrates, and manages skills, commands, instruction files, and MCP
configs from all known agents so that `.agents/skills/` and `.agents/commands/` are the single
sources of truth and artifacts are automatically symlinked to agent-specific locations out of the
box.

## Requirements

### Requirement: Default Config Includes Claude Skills Target

The `DEFAULT_CONFIG` template MUST include an `[agents.claude.targets.skills]` entry that maps the
`skills` source directory to `.claude/skills` using the `symlink` sync type.

The entry MUST be placed within the existing `[agents.claude]` section, after the
`[agents.claude.targets.instructions]` entry.

The `DEFAULT_CONFIG` template MUST remain valid TOML that parses into a `Config` struct without
errors.

#### Scenario: Fresh init generates config with directory symlink for skills

- GIVEN a project directory with no `.agents/` directory
- WHEN the user runs `agentsync init`
- THEN the generated `.agents/agentsync.toml` MUST contain an `[agents.claude.targets.skills]`
  section
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

The `DEFAULT_CONFIG` template MUST specify `type = "symlink"` for the skills target of every agent
that has one (claude, codex, gemini, opencode, and any other agent with a skills target in the
template).

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

When a skills target has `type = "symlink"` and the source is a directory, the `apply` command MUST
create a single symlink at the destination pointing to the source directory. The destination itself
MUST be a symlink, not a real directory.

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
- THEN `.claude/skills/new-skill/SKILL.md` MUST be immediately readable without running
  `agentsync apply` again

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

Projects with existing `agentsync.toml` configs that specify `type = "symlink-contents"` for skills
targets MUST continue to work without modification. The `symlink-contents` sync type MUST NOT be
deprecated, removed, or have its behavior altered by this change.

#### Scenario: Existing symlink-contents config continues to work

- GIVEN a project with `.agents/agentsync.toml` specifying `type = "symlink-contents"` for the
  claude skills target
- AND `.agents/skills/` contains `skill-a/SKILL.md`
- WHEN the user runs `agentsync apply`
- THEN `.claude/skills/` MUST be a real directory (not a symlink)
- AND `.claude/skills/skill-a` MUST be an individual symlink pointing to `.agents/skills/skill-a`

#### Scenario: Updated binary with old config produces no change

- GIVEN a project initialized with an older agentsync version (config says
  `type = "symlink-contents"` for skills)
- WHEN the user updates the agentsync binary but does NOT edit their config
- AND the user runs `agentsync apply`
- THEN the behavior MUST be identical to the previous version
- AND no directory symlinks SHALL be created for skills

---

### Requirement: Clean Transition from symlink-contents to symlink

When a user changes their skills target type from `symlink-contents` to `symlink` and runs
`agentsync sync --clean`, the clean operation MUST remove the old per-entry symlinks and their
containing real directory, and the subsequent sync MUST create a single directory symlink.

#### Scenario: Clean + sync transitions from per-skill symlinks to directory symlink

- GIVEN a project previously synced with `type = "symlink-contents"` for skills
- AND `.claude/skills/` is a real directory containing individual symlinks (`skill-a` →
  `../../.agents/skills/skill-a`)
- AND the user has changed the config to `type = "symlink"`
- WHEN the user runs `agentsync sync --clean`
- THEN the individual symlinks inside `.claude/skills/` MUST be removed
- AND the `.claude/skills/` real directory MUST be removed
- AND a new symlink MUST be created at `.claude/skills` pointing to `.agents/skills`

#### Scenario: Backup of existing real directory at destination

- GIVEN a project where `.claude/skills/` is a real directory containing non-symlink files (
  user-created content)
- AND the config specifies `type = "symlink"` for the skills target
- WHEN the user runs `agentsync apply`
- THEN the existing `.claude/skills/` directory MUST be renamed to `.claude/skills.bak`
- AND a new symlink MUST be created at `.claude/skills` pointing to `.agents/skills`
- AND no user-created content SHALL be lost

---

### Requirement: Dry-run Reports Symlink Strategy

The `--dry-run` output MUST clearly indicate the sync strategy being used for each target,
distinguishing between `symlink` (directory symlink) and `symlink-contents` (per-entry symlinks).

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

When `agentsync apply` attempts to create a directory symlink for a target with `type = "symlink"`
and the symlink creation fails (e.g., due to insufficient permissions, filesystem limitations, or
Windows requiring elevated privileges for `symlink_dir`), the command MUST emit a clear error
message that includes the target path and the underlying OS error, and MUST exit with a non-zero
status code. There is no silent fallback to `symlink-contents` behavior.

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

The `AgentFileType` enum MUST include a `ClaudeSkills` variant representing the `.claude/skills/`
directory.

The `scan_agent_files()` function MUST detect `.claude/skills/` when it exists as a directory and
contains at least one entry (file or subdirectory).

#### Scenario: Scan finds .claude/skills with content

- GIVEN a project directory containing `.claude/skills/my-skill/SKILL.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to
  `AgentFileType::ClaudeSkills`
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

The `AgentFileType` enum MUST include variants for each agent's skill directory: `CursorSkills`,
`CodexSkills`, `GeminiSkills`, `OpenCodeSkills`, `RooSkills`, `FactorySkills`, `VibeSkills`,
`AntigravitySkills`.

The `scan_agent_files()` function MUST detect each agent's skill directory when it exists as a
directory and contains at least one entry.

The function MUST NOT report a skill directory that is empty (zero entries).

#### Scenario: Scan detects Cursor skills directory

- GIVEN a project directory containing `.cursor/skills/my-cursor-skill/SKILL.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to
  `AgentFileType::CursorSkills`
- AND the `path` field MUST be `.cursor/skills`
- AND the `display_name` MUST contain "Cursor" and "skills"

#### Scenario: Scan detects Gemini skills directory

- GIVEN a project directory containing `.gemini/skills/data-analysis/SKILL.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to
  `AgentFileType::GeminiSkills`
- AND the `path` field MUST be `.gemini/skills`

#### Scenario: Scan detects OpenCode skills directory

- GIVEN a project directory containing `.opencode/skills/my-skill/SKILL.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to
  `AgentFileType::OpenCodeSkills`
- AND the `path` field MUST be `.opencode/skills`

#### Scenario: Scan detects skills from multiple agents simultaneously

- GIVEN a project directory containing `.claude/skills/skill-a/SKILL.md`,
  `.cursor/skills/skill-b/SKILL.md`, and `.codex/skills/skill-c/SKILL.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include entries for `ClaudeSkills`, `CursorSkills`, and `CodexSkills`
- AND each entry MUST have a distinct `path` value

#### Scenario: Scan ignores empty agent skill directory

- GIVEN a project directory containing `.gemini/skills/` as an empty directory
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST NOT include any entry with `file_type` equal to `AgentFileType::GeminiSkills`

#### Scenario: Scan detects skills inside a directory also copied as-is

- GIVEN a project directory containing `.cursor/rules/some-rule.mdc` and
  `.cursor/skills/my-skill/SKILL.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include an entry for `CursorDirectory` (the parent `.cursor/` dir)
- AND the result MUST include a separate entry for `CursorSkills` (`.cursor/skills/`)

---

### Requirement: Wizard Migrates Skills from All Agents

When the init wizard discovers any agent skill directory (any `*Skills` variant) and the user
selects it for migration, the wizard MUST copy each immediate child of that skill directory into
`.agents/skills/`.

The wizard MUST handle name collisions: if a child name already exists in `.agents/skills/`, the
wizard MUST skip that child and print a warning message identifying the conflicting name and source
agent.

The wizard MUST process skill directories in scan order. If skills from an earlier-scanned agent are
migrated first, later agents with same-named skills SHALL be skipped with warnings.

The skill migration match arm MUST handle ALL `*Skills` variants (`ClaudeSkills`, `CursorSkills`,
`CodexSkills`, `GeminiSkills`, `OpenCodeSkills`, `RooSkills`, `FactorySkills`, `VibeSkills`,
`AntigravitySkills`) using the same copy-contents-with-collision-detection pattern.

The wizard MUST NOT modify or delete the original skill directories during migration (deletion
happens only if the user opts into the backup step).

#### Scenario: Wizard migrates skills from .claude/skills to .agents/skills

- GIVEN a project directory containing `.claude/skills/skill-a/SKILL.md` and
  `.claude/skills/skill-b/SKILL.md`
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

- GIVEN a project with `.claude/skills/shared-lib/SKILL.md` and
  `.gemini/skills/data-helper/SKILL.md`
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

- GIVEN a project with `.claude/skills/common/SKILL.md` and `.cursor/skills/common/SKILL.md` (
  different content)
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

- GIVEN a project that was previously initialized with an old config (no
  `[agents.claude.targets.skills]`)
- WHEN the user runs `agentsync init --force`
- THEN `.agents/agentsync.toml` MUST be overwritten with the updated `DEFAULT_CONFIG`
- AND the new config MUST contain `[agents.claude.targets.skills]`

---

### Requirement: Scan Detects Command Directories

The `AgentFileType` enum MUST include variants: `ClaudeCommands`, `GeminiCommands`,
`OpenCodeCommands`.

The `scan_agent_files()` function MUST detect command directories when they exist and contain at
least one entry:

- `.claude/commands/` for `ClaudeCommands`
- `.gemini/commands/` for `GeminiCommands`
- `.opencode/command/` for `OpenCodeCommands` (note: singular "command")

#### Scenario: Scan detects Claude commands directory

- GIVEN a project directory containing `.claude/commands/review.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to
  `AgentFileType::ClaudeCommands`
- AND the `path` field MUST be `.claude/commands`
- AND the `display_name` MUST contain "Claude" and "commands"

#### Scenario: Scan detects OpenCode command directory

- GIVEN a project directory containing `.opencode/command/deploy.md`
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to
  `AgentFileType::OpenCodeCommands`
- AND the `path` field MUST be `.opencode/command`

#### Scenario: Scan ignores empty command directory

- GIVEN a project directory containing `.claude/commands/` as an empty directory
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST NOT include any entry with `file_type` equal to
  `AgentFileType::ClaudeCommands`

---

### Requirement: Wizard Migrates Commands to Canonical Location

The wizard MUST create `.agents/commands/` as a canonical commands directory during `init()`,
alongside the existing `.agents/skills/` directory.

When the wizard discovers command directories and the user selects them for migration, it MUST copy
each immediate child file into `.agents/commands/`.

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

- GIVEN a project with `.claude/commands/deploy.md` and `.gemini/commands/deploy.md` (different
  content)
- WHEN the user runs `agentsync init --wizard`
- AND the user selects both command directories for migration
- THEN `.agents/commands/deploy.md` MUST contain the content from the first-processed agent
- AND a warning message MUST be printed that includes the name "deploy"
- AND the warning MUST indicate the command file was skipped due to a collision

#### Scenario: Apply syncs commands to .claude/commands via symlink-contents

- GIVEN a freshly initialized project with `[agents.claude.targets.commands]` configured
- AND `.agents/commands/` contains `review.md`
- WHEN the user runs `agentsync apply`
- THEN a symlink MUST be created at `.claude/commands/review.md` pointing to
  `.agents/commands/review.md`

---

### Requirement: Scan Detects Missing Instruction Files

The `scan_agent_files()` function MUST detect the following instruction files when they exist at the
project root:

- `.windsurfrules` → `AgentFileType::WindsurfRules`
- `OPENCODE.md` → a new `AgentFileType::OpenCodeInstructions` variant
- `AMPCODE.md` → `AgentFileType::AmpInstructions` (already in enum, wire scan)

These instruction files, when detected, MUST be included in the merged `AGENTS.md` during wizard
migration, following the same pattern as existing instruction file types.

#### Scenario: Scan detects .windsurfrules file

- GIVEN a project directory containing a `.windsurfrules` file with content
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to
  `AgentFileType::WindsurfRules`
- AND the `path` field MUST be `.windsurfrules`

#### Scenario: Scan detects OPENCODE.md file

- GIVEN a project directory containing an `OPENCODE.md` file with content
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to
  `AgentFileType::OpenCodeInstructions`
- AND the `path` field MUST be `OPENCODE.md`

#### Scenario: Scan detects AMPCODE.md file

- GIVEN a project directory containing an `AMPCODE.md` file with content
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to
  `AgentFileType::AmpInstructions`
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
- THEN the result MUST include entries for `ClaudeInstructions`, `WindsurfRules`, and
  `OpenCodeInstructions`

---

### Requirement: Scan Detects Agent-Specific MCP Configs

The `AgentFileType` enum MUST include variants for agent-specific MCP configuration files:
`CursorMcpConfig`, `WindsurfMcpConfig`, `CodexConfig`, `RooMcpConfig`, `KiroMcpConfig`,
`AmazonQMcpConfig`, `KilocodeMcpConfig`, `FactoryMcpConfig`, `OpenCodeConfig`.

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

The wizard MUST NOT attempt to parse or import these configs. It MUST only note their existence and
print a message suggesting the user configure MCP servers in `agentsync.toml`.

The noted-but-not-migrated match arm MUST handle ALL agent-specific MCP config variants in addition
to the existing `McpConfig` and `ZedSettings`.

Each noted config MUST print a message containing the detected file path and a suggestion to
configure MCP servers in `agentsync.toml`.

#### Scenario: Scan detects Cursor MCP config

- GIVEN a project directory containing `.cursor/mcp.json` with valid JSON content
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to
  `AgentFileType::CursorMcpConfig`
- AND the `path` field MUST be `.cursor/mcp.json`

#### Scenario: Scan detects Windsurf MCP config

- GIVEN a project directory containing `.windsurf/mcp_config.json` with content
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to
  `AgentFileType::WindsurfMcpConfig`

#### Scenario: Scan detects OpenCode config at project root

- GIVEN a project directory containing `opencode.json` with content
- WHEN `scan_agent_files()` is called on that project root
- THEN the result MUST include a `DiscoveredFile` with `file_type` equal to
  `AgentFileType::OpenCodeConfig`
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

- GIVEN a project with `.cursor/rules/my-rule.mdc` (triggers `CursorDirectory` detection) and
  `.cursor/mcp.json`
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

The `DEFAULT_CONFIG` MUST include new agent sections for `gemini` and `opencode` with at minimum
`instructions` and `skills` targets.

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
- AND the `gemini` agent MUST have an `instructions` target with `destination` containing
  `"GEMINI.md"`
- AND the `gemini` agent MUST have a `skills` target with `destination` containing
  `".gemini/skills"`

#### Scenario: DEFAULT_CONFIG contains OpenCode agent section

- GIVEN the `DEFAULT_CONFIG` constant
- WHEN it is parsed as TOML into a `Config` struct
- THEN there MUST be an `agents` entry keyed `"opencode"`
- AND the `opencode` agent MUST have an `instructions` target with `destination` containing
  `"OPENCODE.md"`
- AND the `opencode` agent MUST have a `skills` target with `destination` containing
  `".opencode/skills"`

#### Scenario: Fresh init with no existing agent files uses updated DEFAULT_CONFIG

- GIVEN a project directory with no agent files at all (no `.claude/`, no `.cursor/`, no instruction
  files)
- WHEN the user runs `agentsync init`
- THEN the generated `.agents/agentsync.toml` MUST contain sections for `claude`, `copilot`,
  `cursor`, `codex`, `gemini`, `opencode`, and `root`
- AND the `claude` section MUST include `instructions`, `skills`, and `commands` targets
- AND `.agents/commands/` MUST exist as an empty directory

#### Scenario: DEFAULT_CONFIG remains valid TOML after updates

- GIVEN the updated `DEFAULT_CONFIG` constant
- WHEN it is parsed as TOML into a `Config` struct
- THEN parsing MUST succeed without errors
- AND all existing agent sections (`claude`, `copilot`, `cursor`, `codex`, `root`) MUST still be
  present
- AND no existing target definitions SHALL be removed or altered

---

### Requirement: Apply-Time Diagnostic for Unmanaged Claude Skills

After processing all sync targets, the `apply` command SHOULD check whether `.claude/skills/` exists
at the project root, contains at least one entry, and is not the destination of any enabled target
in the current configuration.

If all three conditions are true, the system MUST print a warning message to standard output.

The warning MUST NOT be treated as an error (the apply command MUST still exit successfully).

The diagnostic MUST be suppressed when any enabled target already has `.claude/skills` (or a path
that resolves to it) as its destination.

#### Scenario: Apply warns about unmanaged .claude/skills

- GIVEN a project with `.agents/agentsync.toml` that does NOT have a target with destination
  `.claude/skills`
- AND `.claude/skills/` exists and contains `orphan-skill/SKILL.md`
- WHEN the user runs `agentsync apply`
- THEN the command MUST print a warning containing ".claude/skills"
- AND the warning SHOULD suggest running `agentsync init --wizard` to adopt
- AND the apply command MUST exit with a success status

#### Scenario: Apply does not warn when .claude/skills is managed

- GIVEN a project with `.agents/agentsync.toml` containing `[agents.claude.targets.skills]` with
  `destination = ".claude/skills"`
- AND `.claude/skills/` exists with content (created by symlink-contents)
- WHEN the user runs `agentsync apply`
- THEN no warning about unmanaged `.claude/skills` SHALL be printed

#### Scenario: Apply does not warn when .claude/skills is absent

- GIVEN a project with `.agents/agentsync.toml`
- AND `.claude/skills/` does not exist
- WHEN the user runs `agentsync apply`
- THEN no warning about unmanaged `.claude/skills` SHALL be printed

#### Scenario: Apply does not warn when .claude/skills is empty

- GIVEN a project with `.agents/agentsync.toml` that does NOT have a target managing
  `.claude/skills`
- AND `.claude/skills/` exists but is empty (no files or subdirectories)
- WHEN the user runs `agentsync apply`
- THEN no warning about unmanaged `.claude/skills` SHALL be printed

#### Scenario: Dry-run also shows unmanaged skills diagnostic

- GIVEN a project where `.claude/skills/` exists with content and no target manages it
- WHEN the user runs `agentsync apply --dry-run`
- THEN the unmanaged skills warning MUST still be printed
- AND no files SHALL be modified

---

### Requirement: Wizard Makes Skills Link Strategy Explicit

For each generated skills target, `agentsync init --wizard` MUST present the skills link strategy as
an explicit user choice between `symlink` and `symlink-contents`.

The wizard MUST explain the observable difference between the two modes and MUST show a recommended
choice for the target before the user confirms it.

When the existing destination for that skills target is already a directory symlink to the canonical
`.agents/skills` source, the wizard MUST recommend `symlink` and MUST keep that recommendation as
the default selection.

The wizard MUST allow the user to override the recommendation before config is written.

#### Scenario: Wizard recommends preserving an existing directory symlink

- GIVEN `agentsync init --wizard` is generating a skills target for an agent
- AND the agent's skills destination already exists as a directory symlink to the canonical
  `.agents/skills` source
- WHEN the wizard asks how that skills target should sync
- THEN the wizard MUST show both `symlink` and `symlink-contents` as explicit choices
- AND the wizard MUST recommend `symlink`
- AND accepting the default MUST write `type = "symlink"` for that target

#### Scenario: User overrides the recommended skills mode

- GIVEN `agentsync init --wizard` is generating a skills target for an agent
- WHEN the wizard shows the explicit skills link strategy choices and recommendation
- AND the user selects the non-recommended mode
- THEN the generated config MUST use the mode chosen by the user for that target

### Requirement: Post-Init Validation Warns About Skills Mode Mismatches

After `agentsync init --wizard` writes config, the system MUST run a validation summary before the
command finishes.

If a generated skills target's configured mode does not match the on-disk destination shape in a way
that would cause different sync semantics, the validation summary MUST warn about that target before
the user runs `agentsync apply`.

The warning MUST identify the affected target and MUST describe the mismatch in user-facing terms.

#### Scenario: Wizard validation warns after a mode override creates mismatch

- GIVEN an agent's skills destination already exists as a directory symlink to the canonical
  `.agents/skills` source
- AND the user finishes `agentsync init --wizard` with that target configured as `symlink-contents`
- WHEN the wizard prints its post-init validation summary
- THEN the summary MUST warn that the configured mode does not match the current destination shape
- AND the warning MUST appear before the command exits

#### Scenario: Wizard validation stays quiet for matching directory symlink mode

- GIVEN an agent's skills destination already exists as a directory symlink to the canonical
  `.agents/skills` source
- AND the user finishes `agentsync init --wizard` with that target configured as `symlink`
- WHEN the wizard prints its post-init validation summary
- THEN the summary MUST NOT warn about a mode-semantic mismatch for that target

### Requirement: Doctor Clearly Reports Skills Mode-Semantic Mismatches

`agentsync doctor` MUST detect the case where a skills target is configured as `symlink-contents`
while the destination already exists as a directory symlink to the canonical source for that target.

When that mismatch is detected, `agentsync doctor` MUST report it clearly as a mode-semantic
mismatch rather than as a healthy target.

The diagnostic MUST identify the affected target, MUST describe the configured mode and observed
destination shape, and MUST warn that applying the config can cause avoidable churn.

#### Scenario: Doctor reports directory-symlink versus symlink-contents mismatch

- GIVEN a project with a skills target configured as `symlink-contents`
- AND that target's destination already exists as a directory symlink to the canonical source
- WHEN the user runs `agentsync doctor`
- THEN the output MUST report a mode-semantic mismatch for that target
- AND the output MUST mention both `symlink-contents` and the existing directory symlink shape
- AND the output MUST warn before the user runs `agentsync apply`

#### Scenario: Doctor does not report mismatch for matching directory symlink mode

- GIVEN a project with a skills target configured as `symlink`
- AND that target's destination already exists as a directory symlink to the canonical source
- WHEN the user runs `agentsync doctor`
- THEN the output MUST NOT report a mode-semantic mismatch for that target

### Requirement: Status Gives a Focused Hint for Recognized Skills Mode Mismatches

When `agentsync status` inspects a skills target whose destination resolves to the canonical source
but whose configured mode does not match the observed destination shape, it MUST include a focused
hint about the mode-semantic mismatch.

The hint MUST identify the affected target without requiring the command to treat the destination as
broken.

#### Scenario: Status hints on recognized mode mismatch

- GIVEN a project with a skills target configured as `symlink-contents`
- AND that target's destination already exists as a directory symlink to the canonical source
- WHEN the user runs `agentsync status`
- THEN the output MUST include a hint that the target's configured mode does not match the current
  destination shape
- AND the hint MUST identify the affected skills target

### Requirement: Skills Documentation Matches Shipped Link Behavior

The published documentation for skills configuration and wizard behavior MUST describe skills
targets as defaulting to `symlink`.

The documentation MUST explain when `symlink-contents` remains a valid choice, MUST describe the
wizard's recommendation and preservation behavior for existing correct directory symlinks, and MUST
describe how users can detect mode-semantic mismatches before apply.

#### Scenario: Reference docs describe current default and preservation guidance

- GIVEN the published configuration and skills guidance for AgentSync
- WHEN a user reads the skills sync documentation
- THEN the documentation MUST describe `symlink` as the default skills mode
- AND the documentation MUST explain that existing correct directory symlinks are preserved by
  default in the wizard

#### Scenario: CLI docs describe validation and diagnostics

- GIVEN the published CLI documentation for init and diagnostics
- WHEN a user reads about `agentsync init --wizard` and `agentsync doctor`
- THEN the documentation MUST describe the post-init validation summary for mode mismatches
- AND the documentation MUST describe how `doctor` reports the recognized mismatch case before apply

---

### Requirement: Wizard Summary Identifies Canonical Source Of Truth

After `agentsync init --wizard` migrates agent assets into `.agents/`, the completion summary MUST
describe `.agents/` as the canonical source of truth for the generated configuration and migrated
content.

The summary MUST describe downstream agent-specific files as follow-on targets to be reconciled
later, not as the authoritative source after migration.

#### Scenario: Summary declares canonical `.agents/` ownership after migration

- GIVEN the user runs `agentsync init --wizard`
- AND the wizard migrates one or more agent files into `.agents/`
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST state that `.agents/` is the canonical source of truth
- AND the summary MUST describe the migrated config or content as now living under `.agents/`

#### Scenario: Summary does not treat legacy agent paths as authoritative

- GIVEN the user runs `agentsync init --wizard`
- AND the wizard migrates content from one or more agent-specific directories
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST NOT describe the original agent-specific locations as the ongoing source of
  truth

---

### Requirement: Wizard Summary States Apply As The Next Required Step

When `agentsync init --wizard` completes, the final post-migration summary MUST tell the user that
running `agentsync apply` is the next step required to reconcile managed downstream files.

The summary MUST NOT claim or imply that `agentsync apply` has already run.

The summary MAY explain that `agentsync apply` can update managed targets according to the generated
configuration, but it MUST present those updates as future follow-up work rather than completed
work.

#### Scenario: Summary instructs user to run apply next

- GIVEN the user completes `agentsync init --wizard`
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST tell the user to run `agentsync apply` next
- AND the summary MUST describe `apply` as the step that reconciles downstream managed files

#### Scenario: Summary avoids claiming apply already ran

- GIVEN the user completes `agentsync init --wizard`
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST NOT say or imply that `agentsync apply` already ran
- AND the summary MUST NOT present downstream targets as already reconciled solely because init
  completed

---

### Requirement: Wizard Summary Gives Cautious Git-State Guidance

The final post-migration summary MUST give cautious git guidance based only on facts known during
the wizard run.

The summary MUST tell the user to review repository changes with normal git workflows after running
the wizard and any follow-up apply step.

The summary MUST explain that the exact file changes shown by git MAY vary by repository depending
on what files, managed blocks, or prior agent artifacts already existed.

The summary MUST NOT claim that the repository is clean, dirty, staged, unstaged, ready to commit,
or otherwise describe actual git state that the wizard did not inspect.

#### Scenario: Summary warns that git changes depend on repository history

- GIVEN the user completes `agentsync init --wizard`
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST tell the user to review git changes manually
- AND the summary MUST explain that repo-specific differences may appear depending on what existed
  before

#### Scenario: Summary does not overstate current git status

- GIVEN the user completes `agentsync init --wizard`
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST NOT claim that git changes are already reviewed, complete, or safe to commit
- AND the summary MUST NOT report any staged or unstaged state unless the wizard actually inspected
  git state

---

### Requirement: Wizard Summary Explains Default Gitignore-Managed Collaboration Expectations

When the generated configuration keeps gitignore management enabled, the final post-migration
summary MUST explain collaborator expectations for that mode.

That explanation MUST state that collaborators who use the repo SHOULD run `agentsync apply` so
managed targets, including `.gitignore` behavior governed by configuration, stay aligned with
`.agents/`.

The summary MAY warn that `apply` can introduce repository-specific `.gitignore` changes in that
mode, but it MUST NOT claim that `.gitignore` has already been updated during the wizard.

#### Scenario: Default gitignore-managed mode includes collaborator warning

- GIVEN the user completes `agentsync init --wizard`
- AND the generated configuration keeps `[gitignore].enabled = true`
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST explain that collaborators are expected to use `agentsync apply` to stay
  aligned with the canonical `.agents/` config
- AND the summary MUST mention that `.gitignore` behavior is still governed by configuration during
  apply

#### Scenario: Default gitignore-managed mode does not overclaim `.gitignore` changes

- GIVEN the user completes `agentsync init --wizard`
- AND the generated configuration keeps `[gitignore].enabled = true`
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST NOT say that `.gitignore` was already updated
- AND the summary MUST NOT say that `.gitignore` requires no further review

---

### Requirement: Wizard Summary Reports Backup Outcomes When Relevant

If the wizard creates backups of migrated source files or directories, the final post-migration
summary MUST report that backup outcome.

The backup messaging MUST identify that backups were created and SHOULD tell the user where they
were written or how to find them.

If no backup was created, the summary MUST NOT imply that a backup exists.

#### Scenario: Summary reports created backup

- GIVEN the user runs `agentsync init --wizard`
- AND the wizard creates a backup of migrated source files or directories
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST say that a backup was created
- AND the summary SHOULD identify the backup location or how to find it

#### Scenario: Summary stays accurate when no backup exists

- GIVEN the user runs `agentsync init --wizard`
- AND the wizard does not create any backup
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST NOT imply that a backup was created

---

### Requirement: Wizard Completion Output Avoids Conflicting Generic Footer Messaging

When the wizard prints its purpose-built post-migration summary, the overall command completion
output MUST remain coherent and MUST NOT include duplicate or conflicting generic footer guidance.

If a generic init footer would repeat, contradict, or oversimplify wizard-specific migration
guidance, the system MUST suppress or replace that generic footer for wizard runs.

#### Scenario: Wizard run emits one coherent completion message

- GIVEN the user completes `agentsync init --wizard`
- WHEN command completion output is printed
- THEN the output MUST contain one coherent set of post-migration next steps
- AND the output MUST NOT repeat the same follow-up instruction in both a wizard-specific summary
  and a generic footer

#### Scenario: Generic footer does not contradict wizard summary

- GIVEN the user completes `agentsync init --wizard`
- AND the wizard-specific summary explains that apply and git review still remain
- WHEN command completion output is printed
- THEN no later generic footer text MUST imply that initialization is fully reconciled already
- AND no later generic footer text MUST omit or contradict the wizard-specific migration caveats

### Requirement: Wizard-Written AGENTS Includes Managed Agent Config Layout Section

When `agentsync init --wizard` writes `.agents/AGENTS.md`, the generated file MUST include exactly
one managed section titled `Agent config layout`.

The managed section MUST be placed prominently near the top of the file, after the file's opening
title or introductory context when present, and before the remainder of the wizard-written
instruction body.

The managed section MUST use stable begin/end markers so a later forced wizard rewrite can replace
the generated section instead of duplicating it.

The managed section MUST explain, in actionable language for a fresh agent, that `.agents/` is the
canonical source for the generated instructions, skills, and commands layout.

#### Scenario: Fresh wizard output includes one managed explainer block near the top

- GIVEN a project with no existing `.agents/AGENTS.md`
- WHEN the user runs `agentsync init --wizard`
- THEN the written `.agents/AGENTS.md` MUST contain exactly one managed `Agent config layout`
  section
- AND that section MUST appear after the file title or opening introduction
- AND that section MUST appear before later wizard-written instruction content

#### Scenario: Wizard output with migrated instruction content keeps explainer block prominent

- GIVEN the wizard is writing `.agents/AGENTS.md` with migrated instruction content from discovered
  agent files
- WHEN the final file is rendered
- THEN the managed `Agent config layout` section MUST remain near the top of the file
- AND the migrated instruction content MUST remain after that managed section

#### Scenario: Forced rewrite replaces existing managed block instead of duplicating it

- GIVEN `.agents/AGENTS.md` already contains the managed `Agent config layout` section written by a
  previous wizard run
- WHEN the user runs `agentsync init --wizard --force`
- THEN the resulting `.agents/AGENTS.md` MUST still contain exactly one managed
  `Agent config layout` section
- AND the regenerated section MUST replace the earlier managed block rather than append a second
  copy

---

### Requirement: Agent Config Layout Section Reflects Wizard-Generated Destinations And Sync Semantics

The managed `Agent config layout` section MUST be derived from the wizard-generated configuration
that is being written for the project.

The section MUST describe the actual configured canonical source content and the actual enabled
default destinations for instructions, skills, and commands that the wizard generated.

The section MUST NOT invent destinations, agent targets, or sync behavior that are absent from the
generated configuration.

When a described target uses `symlink`, the wording MUST indicate that the destination reflects the
canonical source directly.

When a described target uses `symlink-contents`, the wording MUST indicate that the destination is
populated from the canonical source by sync and that adding, removing, or renaming entries requires
rerunning `agentsync apply`.

#### Scenario: Default wizard layout lists generated instruction, skills, and commands destinations

- GIVEN the wizard generates the default target layout
- WHEN `.agents/AGENTS.md` is written
- THEN the managed `Agent config layout` section MUST describe `.agents/AGENTS.md` as the canonical
  instructions source
- AND the section MUST include the generated instruction destinations `CLAUDE.md`,
  `.github/copilot-instructions.md`, `GEMINI.md`, `OPENCODE.md`, and `AGENTS.md`
- AND the section MUST include the generated skills destinations `.claude/skills`, `.codex/skills`,
  `.gemini/skills`, and `.opencode/skills`
- AND the section MUST include the generated commands destinations `.claude/commands`,
  `.gemini/commands`, and `.opencode/command`

#### Scenario: Skills wording changes with selected sync mode

- GIVEN the wizard generates skills targets where one enabled default target uses `symlink`
- AND another enabled default target uses `symlink-contents`
- WHEN `.agents/AGENTS.md` is written
- THEN the `Agent config layout` section MUST describe the `symlink` skills destination as
  reflecting the canonical skills source directly
- AND the section MUST describe the `symlink-contents` skills destination as requiring
  `agentsync apply` to propagate added, removed, or renamed skill entries

#### Scenario: Layout block omits targets that are not present in generated config

- GIVEN the wizard-generated configuration does not include a default destination for a particular
  instructions, skills, or commands target
- WHEN `.agents/AGENTS.md` is written
- THEN the managed `Agent config layout` section MUST NOT claim that missing destination exists
- AND the section MUST describe only the enabled default targets present in the generated
  configuration

---

### Requirement: Wizard AGENTS Layout Generation Preserves Existing Non-Forced Behavior And Excludes Apply Mutation

If `.agents/AGENTS.md` already exists and `--force` is not used, `agentsync init --wizard` MUST
preserve the existing file unchanged.

In that case, the wizard MUST NOT inject, prepend, append, or partially replace a managed
`Agent config layout` section.

This change is wizard-only in v1. `agentsync apply` MUST NOT create, refresh, or mutate the managed
`Agent config layout` section in `.agents/AGENTS.md`.

#### Scenario: Existing AGENTS file is preserved without force

- GIVEN `.agents/AGENTS.md` already exists before the wizard runs
- WHEN the user runs `agentsync init --wizard` without `--force`
- THEN `.agents/AGENTS.md` MUST remain byte-for-byte unchanged
- AND no managed `Agent config layout` section SHALL be inserted or updated as part of that run

#### Scenario: Forced rewrite stays idempotent across repeated runs

- GIVEN the wizard can write `.agents/AGENTS.md`
- WHEN the user runs `agentsync init --wizard --force` multiple times without changing the generated
  configuration
- THEN each run MUST produce the same single managed `Agent config layout` section
- AND repeated forced runs MUST NOT accumulate duplicate markers or duplicate explainer content

#### Scenario: Apply does not own AGENTS layout regeneration

- GIVEN `.agents/AGENTS.md` contains a wizard-generated managed `Agent config layout` section
- WHEN the user later runs `agentsync apply`
- THEN `agentsync apply` MUST NOT rewrite that managed section
- AND `agentsync apply` MUST NOT add a new `Agent config layout` section if one is missing

---

## Non-functional Requirements

### Requirement: No New Dependencies for Symlink Change

The symlink-contents-to-symlink change MUST NOT introduce any new crate dependencies or external
libraries. All functionality MUST use existing code paths (`SyncType::Symlink` for directory
sources).

### Requirement: No New Sync Types for Symlink Change

The symlink-contents-to-symlink change MUST NOT add new variants to `SyncType` or new fields to
`TargetConfig`. The existing `SyncType::Symlink` variant handles directory sources and MUST be used
as-is.

### Requirement: Existing symlink-contents Behavior Unchanged

The observable semantics of `SyncType::SymlinkContents` MUST remain identical after the symlink
change. Its cleanup behavior, create behavior, and pattern-filtering results MUST remain unchanged
for callers and clients, and tests or acceptance criteria MUST continue validating those behaviors
as the public contract.

---

## Acceptance Criteria

1. `DEFAULT_CONFIG` parses as valid TOML and includes `[agents.claude.targets.skills]` with
   `type = "symlink"` (directory symlink).
2. `DEFAULT_CONFIG` specifies `type = "symlink"` for all agent skills targets (claude, codex,
   gemini, opencode).
3. `DEFAULT_CONFIG` keeps `type = "symlink-contents"` for commands targets.
4. This repo's `.agents/agentsync.toml` uses `type = "symlink"` for all skills targets.
5. `agentsync apply` creates a directory symlink (not a real directory) for skills targets with
   `type = "symlink"`.
6. New skills/renames/deletions in the source directory are immediately visible through the
   directory symlink without re-running apply.
7. Existing `symlink-contents` configs continue to work unchanged (backward compatible).
8. `agentsync sync --clean` correctly transitions from per-entry symlinks to a directory symlink.
9. `--dry-run` output distinguishes between `symlink` and `symlink-contents` strategies.
10. `scan_agent_files()` detects `.claude/skills/` when it exists with content and ignores it when
    empty or absent.
11. The init wizard copies skills from all agent skill directories into `.agents/skills/`, skipping
    collisions with a warning.
12. `agentsync apply` prints a diagnostic warning when `.claude/skills/` has unmanaged content.
13. All existing tests continue to pass (no regressions).
14. New tests cover: template parsing with skills target, scan detection, collision handling, and
    diagnostic warning logic.
15. `scan_agent_files()` detects skill directories for all 8 agents (Cursor, Codex, Gemini,
    OpenCode, Roo, Factory, Vibe, Antigravity) when they exist with content, and ignores them when
    empty or absent.
16. `scan_agent_files()` detects command directories for Claude, Gemini, and OpenCode when they
    exist with content.
17. `scan_agent_files()` detects `.windsurfrules`, `OPENCODE.md`, and `AMPCODE.md` as instruction
    files.
18. `scan_agent_files()` detects the presence of 10 agent-specific MCP configuration files/settings
    paths (e.g., `.cursor/mcp.json`, `.vscode/mcp.json`) when present.
19. The wizard migrates command directories into `.agents/commands/` with collision detection.
20. The wizard notes all detected MCP configs with migration suggestion messages without parsing or
    copying them.
21. `init()` creates `.agents/commands/` alongside `.agents/skills/`.
22. `DEFAULT_CONFIG` includes `[agents.claude.targets.commands]` with correct source, destination,
    and type.
23. `DEFAULT_CONFIG` includes `gemini` and `opencode` agent sections with instructions and skills
    targets.
24. `DEFAULT_CONFIG` remains valid TOML that parses into a `Config` struct.
25. New tests cover each new scan entry, migration path, collision scenario, and config parsing.
26. `agentsync init --wizard` generates a managed `Agent config layout` section in `.agents/AGENTS.md`
    derived from the wizard's final target layout.
27. The managed layout section uses stable markers for idempotent replacement on `--force` reruns.
28. The layout section reflects actual enabled destinations and sync-type-specific wording.
29. Existing `.agents/AGENTS.md` is preserved byte-for-byte when `--force` is not used.
30. `agentsync apply` does not create, refresh, or mutate the managed layout section.
