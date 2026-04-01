# Skill Adoption Specification

## Purpose

Defines how AgentSync detects, migrates, and manages Claude skills so that `.agents/skills/` is the
single source of truth and skills are automatically symlinked to `.claude/skills/` out of the box.

## Requirements

### Requirement: Default Config Includes Claude Skills Target

The `DEFAULT_CONFIG` template MUST include an `[agents.claude.targets.skills]` entry that maps the
`skills` source directory to `.claude/skills` using the `symlink-contents` sync type.

The entry MUST be placed within the existing `[agents.claude]` section, after the
`[agents.claude.targets.instructions]` entry.

The `DEFAULT_CONFIG` template MUST remain valid TOML that parses into a `Config` struct without
errors.

#### Scenario: Fresh init generates config with Claude skills target

- GIVEN a project directory with no `.agents/` directory
- WHEN the user runs `agentsync init`
- THEN the generated `.agents/agentsync.toml` MUST contain an `[agents.claude.targets.skills]`
  section
- AND the section MUST specify `source = "skills"`
- AND the section MUST specify `destination = ".claude/skills"`
- AND the section MUST specify `type = "symlink-contents"`

#### Scenario: Fresh init config is parseable with skills target

- GIVEN the `DEFAULT_CONFIG` constant
- WHEN it is parsed as TOML into a `Config` struct
- THEN the `claude` agent MUST have a `skills` target in its `targets` map
- AND the `skills` target MUST have `sync_type` equal to `SymlinkContents`

#### Scenario: Apply symlinks skills to .claude/skills on fresh project

- GIVEN a freshly initialized project (via `agentsync init`)
- AND `.agents/skills/` contains a skill subdirectory `my-skill/` with a `SKILL.md` file
- WHEN the user runs `agentsync apply`
- THEN a symlink MUST be created at `.claude/skills/my-skill` pointing to the corresponding source
  in `.agents/skills/my-skill`

#### Scenario: Apply with empty skills directory

- GIVEN a freshly initialized project (via `agentsync init`)
- AND `.agents/skills/` exists but is empty
- WHEN the user runs `agentsync apply`
- THEN the `.claude/skills/` directory SHOULD be created (or already exist)
- AND no symlinks SHALL be created inside `.claude/skills/`
- AND the command MUST NOT produce an error

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

### Requirement: Wizard Migrates Claude Skills

When the init wizard discovers `.claude/skills/` and the user selects it for migration, the wizard
MUST copy each immediate child (subdirectory or file) of `.claude/skills/` into `.agents/skills/`.

The wizard MUST handle name collisions: if a child name in `.claude/skills/` already exists in
`.agents/skills/`, the wizard MUST skip that child and print a warning message identifying the
conflicting name.

The wizard MUST NOT modify or delete the original `.claude/skills/` directory during migration (
deletion happens only if the user opts into the backup step).

#### Scenario: Wizard migrates skills from .claude/skills to .agents/skills

- GIVEN a project directory containing `.claude/skills/skill-a/SKILL.md` and
  `.claude/skills/skill-b/SKILL.md`
- AND no `.agents/` directory exists
- WHEN the user runs `agentsync init --wizard`
- AND the user selects `.claude/skills/` for migration
- THEN `.agents/skills/skill-a/SKILL.md` MUST exist with the same content as the original
- AND `.agents/skills/skill-b/SKILL.md` MUST exist with the same content as the original

#### Scenario: Wizard handles skill name collision

- GIVEN a project directory containing `.claude/skills/shared-skill/SKILL.md`
- AND `.agents/skills/shared-skill/SKILL.md` already exists with different content
- WHEN the user runs `agentsync init --wizard`
- AND the user selects `.claude/skills/` for migration
- THEN `.agents/skills/shared-skill/` MUST NOT be overwritten
- AND a warning message MUST be printed that includes the name "shared-skill"
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

## Acceptance Criteria

1. `DEFAULT_CONFIG` parses as valid TOML and includes `[agents.claude.targets.skills]` with correct
   source, destination, and type.
2. `scan_agent_files()` detects `.claude/skills/` when it exists with content and ignores it when
   empty or absent.
3. The init wizard copies skills from `.claude/skills/` into `.agents/skills/`, skipping collisions
   with a warning.
4. `agentsync apply` prints a diagnostic warning when `.claude/skills/` has unmanaged content.
5. All existing tests continue to pass (no regressions).
6. New tests cover: template parsing with skills target, scan detection, collision handling, and
   diagnostic warning logic.
