# Delta for Skill Adoption

## MODIFIED Requirements

### Requirement: Default Config Includes Claude Skills Target

The `DEFAULT_CONFIG` template MUST include an `[agents.claude.targets.skills]` entry that maps the `skills` source directory to `.claude/skills` using the `symlink` sync type.

(Previously: used `symlink-contents` sync type)

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

### Requirement: Default Config Uses Symlink Type for All Agent Skills Targets

The `DEFAULT_CONFIG` template MUST specify `type = "symlink"` for the skills target of every agent that has one (claude, codex, gemini, opencode, and any other agent with a skills target in the template).

(Previously: all agents used `type = "symlink-contents"` for skills targets)

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

### Requirement: This Repo's Config Uses Symlink Type for Skills

The `.agents/agentsync.toml` in this repository MUST use `type = "symlink"` for all skills targets.

(Previously: used `type = "symlink-contents"` for opencode and copilot skills targets)

#### Scenario: Repo config specifies symlink for opencode skills

- GIVEN the `.agents/agentsync.toml` in this repository
- WHEN the opencode agent's skills target is inspected
- THEN `type` MUST equal `"symlink"`

#### Scenario: Repo config specifies symlink for copilot skills

- GIVEN the `.agents/agentsync.toml` in this repository
- WHEN the copilot agent's skills target is inspected
- THEN `type` MUST equal `"symlink"`

### Requirement: Apply Creates Directory Symlink for Skills

When a skills target has `type = "symlink"` and the source is a directory, the `apply` command MUST create a single symlink at the destination pointing to the source directory. The destination itself MUST be a symlink, not a real directory.

(Previously: with `symlink-contents`, apply created a real directory at destination and individual symlinks per entry inside it)

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

#### Scenario: Apply with empty skills directory

- GIVEN a freshly initialized project with skills target `type = "symlink"`
- AND `.agents/skills/` exists but is empty
- WHEN the user runs `agentsync apply`
- THEN `.claude/skills` MUST be a symlink pointing to `.agents/skills`
- AND the command MUST NOT produce an error

## ADDED Requirements

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
- THEN the existing `.claude/skills/` directory MUST be renamed to `.claude/skills.bak.<timestamp>`
- AND a new symlink MUST be created at `.claude/skills` pointing to `.agents/skills`
- AND no user-created content SHALL be lost

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

## Non-functional Requirements

### Requirement: No New Dependencies

This change MUST NOT introduce any new crate dependencies or external libraries. All functionality MUST use existing code paths (`SyncType::Symlink` for directory sources).

### Requirement: No New Sync Types

This change MUST NOT add new variants to `SyncType` or new fields to `TargetConfig`. The existing `SyncType::Symlink` variant handles directory sources and MUST be used as-is.

### Requirement: Existing symlink-contents Behavior Unchanged

The implementation of `SyncType::SymlinkContents` in the linker MUST NOT be modified by this change. Its clean logic, create logic, and pattern-filtering behavior MUST remain identical.
