# Specification: Module-Map Sync Type

**Change**: nested-agent-context  
**Date**: 2026-03-22  
**Status**: COMPLETE  

## Purpose

Define the behavior of a new `module-map` sync type that maps centrally-managed agent instruction files (in `.agents/`) to arbitrary subdirectories with convention-based filenames per agent. This enables per-module agent context — different instruction files for different parts of a codebase — managed through AgentSync's symlink infrastructure.

---

## Data Model

### ModuleMapping Struct

A mapping entry within a `module-map` target:

| Field               | Type              | Required | Description                                                                          |
|---------------------|-------------------|----------|--------------------------------------------------------------------------------------|
| `source`            | `String`          | Yes      | Source file path, relative to `source_dir` (e.g., `"api-context.md"`)                |
| `destination`       | `String`          | Yes      | Destination directory, relative to project root (e.g., `"src/api"`)                  |
| `filename_override` | `Option<String>`  | No       | Explicit destination filename; supersedes convention-based resolution when present    |

### TargetConfig Extension

A new field on `TargetConfig`:

| Field      | Type                          | Default | Description                                              |
|------------|-------------------------------|---------|----------------------------------------------------------|
| `mappings` | `Vec<ModuleMapping>`         | `[]`    | List of source→destination mappings for `module-map` type |

### SyncType Enum Extension

New variant: `ModuleMap` (serialized as `"module-map"` via `#[serde(rename_all = "kebab-case")]`)

### Convention Filename Resolution

A new function `agent_convention_filename(agent_name: &str) -> Option<&'static str>` in `agent_ids.rs`:

| Agent       | Convention Filename                   |
|-------------|---------------------------------------|
| `claude`    | `CLAUDE.md`                           |
| `copilot`   | `.github/copilot-instructions.md`     |
| `codex`     | `AGENTS.md`                           |
| `gemini`    | `GEMINI.md`                           |
| `cursor`    | `.cursor/rules/agentsync.mdc`         |
| `windsurf`  | `.windsurfrules`                      |
| `opencode`  | `OPENCODE.md`                         |
| (unknown)   | `None` (fallback handled by `resolve_module_map_filename()`) |

---

## Requirements

### Requirement: TOML Configuration Parsing

The system MUST parse `module-map` as a valid `SyncType` value.

The system MUST parse `mappings` as an array-of-tables on `TargetConfig`, defaulting to an empty list when absent.

The system MUST deserialize `ModuleMapping` structs with `source` (required), `destination` (required), and `filename_override` (optional) fields.

Existing configurations without `mappings` MUST continue to parse and function identically (no regression).

#### Scenario: Parse a module-map target with mappings

- GIVEN a TOML config with `type = "module-map"` and two `[[agents.claude.targets.modules.mappings]]` entries
- WHEN the config is loaded
- THEN the `SyncType` MUST be `ModuleMap`
- AND `mappings` MUST contain exactly 2 `ModuleMapping` entries
- AND each entry MUST have its `source` and `destination` fields set correctly

#### Scenario: Parse a module-map target with filename_override

- GIVEN a TOML config with a mapping that includes `filename_override = "CUSTOM.md"`
- WHEN the config is loaded
- THEN the corresponding `ModuleMapping.filename_override` MUST be `Some("CUSTOM.md")`

#### Scenario: Parse existing symlink target unchanged

- GIVEN a TOML config with only `type = "symlink"` targets (no mappings field)
- WHEN the config is loaded
- THEN `mappings` MUST be empty
- AND all existing behavior MUST be preserved

#### Scenario: Reject invalid sync type

- GIVEN a TOML config with `type = "module-map"` but no `mappings` entries
- WHEN the config is loaded
- THEN the config MUST parse successfully (empty mappings is valid at parse time; doctor validates)

#### Scenario: Reject truly invalid type string

- GIVEN a TOML config with `type = "invalid-type"`
- WHEN the config is loaded
- THEN parsing MUST fail with a deserialization error

---

### Requirement: Convention Filename Resolution

The system MUST provide a function that maps an agent name to its convention filename.

The function MUST use the canonical agent name (after alias resolution) for lookup.

The function MUST return `None` for unrecognized agents.

#### Scenario: Known agent convention filenames

- GIVEN agent name `"claude"`
- WHEN `agent_convention_filename("claude")` is called
- THEN the result MUST be `"CLAUDE.md"`

#### Scenario: Known agent with nested convention path

- GIVEN agent name `"copilot"`
- WHEN `agent_convention_filename("copilot")` is called
- THEN the result MUST be `".github/copilot-instructions.md"`

#### Scenario: Unknown agent returns no convention filename

- GIVEN agent name `"my-custom-agent"`
- WHEN `agent_convention_filename("my-custom-agent")` is called
- THEN the result MUST be `None`

#### Scenario: Case-insensitive agent matching

- GIVEN agent name `"Claude"` (mixed case)
- WHEN `agent_convention_filename("Claude")` is called
- THEN the result MUST be `"CLAUDE.md"` (same as lowercase input)

---

### Requirement: Destination Filename Resolution Order

When processing a module-map mapping, the system MUST resolve the destination filename using the following precedence:

1. `filename_override` — if set on the mapping, use it directly
2. Convention filename — from `agent_convention_filename()` for the current agent when it returns `Some(...)`
3. Source basename — the filename component of the mapping's `source` field (last resort fallback)

The system MUST NOT fall through to step 3 if step 2 returns a value.

#### Scenario: filename_override takes precedence

- GIVEN a mapping with `source = "api-rules.md"`, `destination = "src/api"`, `filename_override = "CUSTOM.md"` for agent `"claude"`
- WHEN the mapping is processed
- THEN the symlink destination MUST be `src/api/CUSTOM.md`

#### Scenario: Convention filename used when no override

- GIVEN a mapping with `source = "api-rules.md"`, `destination = "src/api"`, no `filename_override` for agent `"claude"`
- WHEN the mapping is processed
- THEN the symlink destination MUST be `src/api/CLAUDE.md`

#### Scenario: Convention filename for codex agent

- GIVEN a mapping with `source = "api-rules.md"`, `destination = "src/api"`, no `filename_override` for agent `"codex"`
- WHEN the mapping is processed
- THEN the symlink destination MUST be `src/api/AGENTS.md`

---

### Requirement: Module-Map Sync (Apply)

The system MUST create a symlink for each mapping in a `module-map` target.

Each symlink MUST point from the resolved destination path (destination dir + resolved filename) to the source file (relative to `source_dir`).

The system MUST create intermediate directories for destination paths that don't exist.

The system MUST use relative symlinks, consistent with existing `Symlink` behavior.

The source path MUST be resolved relative to `source_dir` (`.agents/<agent>/`), not project root.

#### Scenario: Sync creates symlinks for all mappings

- GIVEN a module-map target with 3 mappings for agent `"claude"` pointing to `src/api`, `src/ui`, and `src/db`
- AND the source files exist in `.agents/claude/`
- WHEN `agentsync apply` is run
- THEN symlinks MUST be created at `src/api/CLAUDE.md`, `src/ui/CLAUDE.md`, and `src/db/CLAUDE.md`
- AND each symlink MUST point to the corresponding source file via relative path

#### Scenario: Sync creates intermediate directories

- GIVEN a mapping with `destination = "src/deep/nested/module"` and the directory does not exist
- WHEN `agentsync apply` is run
- THEN the directory `src/deep/nested/module/` MUST be created
- AND the symlink MUST be created inside it

#### Scenario: Sync with nested convention path (copilot)

- GIVEN a mapping for agent `"copilot"` with `destination = "src/api"`
- WHEN `agentsync apply` is run
- THEN the symlink destination MUST be `src/api/.github/copilot-instructions.md`
- AND intermediate directory `src/api/.github/` MUST be created if needed

#### Scenario: Sync skips mapping when source doesn't exist

- GIVEN a mapping whose source file does not exist in `.agents/claude/`
- WHEN `agentsync apply` is run
- THEN the system SHOULD log a warning for the missing source
- AND processing SHOULD continue with remaining mappings
- AND the result MUST reflect the error count

#### Scenario: Sync handles existing symlink (idempotent re-run)

- GIVEN a mapping whose destination symlink already exists and points to the correct source
- WHEN `agentsync apply` is run again
- THEN the system SHOULD skip the mapping without error
- AND the existing symlink MUST remain unchanged

---

### Requirement: Module-Map Dry-Run

When `--dry-run` is specified, the system MUST print a "Would create symlink: ..." message for each mapping without performing any filesystem changes.

#### Scenario: Dry-run prints per-mapping messages

- GIVEN a module-map target with 2 mappings
- WHEN `agentsync apply --dry-run` is run
- THEN the output MUST contain 2 "Would..." messages, one per mapping
- AND no symlinks MUST be created on disk
- AND no directories MUST be created on disk

---

### Requirement: Module-Map Clean

The `clean` command MUST iterate all mappings in a `module-map` target and remove each destination symlink.

The system MUST only remove files that are symlinks (not regular files).

The system SHOULD attempt to remove empty parent directories created by the sync (best-effort, no error on failure).

#### Scenario: Clean removes all module-map symlinks

- GIVEN a module-map target with 3 mappings and all 3 symlinks exist
- WHEN `agentsync clean` is run
- THEN all 3 symlinks MUST be removed
- AND the result `removed` count MUST be 3

#### Scenario: Clean skips non-symlink files

- GIVEN a destination path that exists as a regular file (not a symlink)
- WHEN `agentsync clean` is run
- THEN the regular file MUST NOT be removed
- AND no error MUST be raised for that path

#### Scenario: Clean dry-run prints per-mapping messages

- GIVEN a module-map target with 2 existing symlinks
- WHEN `agentsync clean --dry-run` is run
- THEN the output MUST contain 2 "Would remove: ..." messages
- AND no symlinks MUST be removed from disk

---

### Requirement: Module-Map Status

The `status` command MUST expand each mapping in a `module-map` target into an individual `StatusEntry`.

Each entry MUST show the resolved destination path, existence state, symlink state, and expected source.

#### Scenario: Status shows per-mapping entries

- GIVEN a module-map target with 3 mappings, 2 of which have correct symlinks and 1 is missing
- WHEN `agentsync status` is run
- THEN the output MUST contain 3 entries
- AND 2 entries MUST show as correct symlinks
- AND 1 entry MUST show as not existing

#### Scenario: Status detects stale symlink

- GIVEN a mapping whose symlink exists but points to a different file than expected
- WHEN `agentsync status` is run
- THEN the entry MUST show `is_symlink = true`
- AND `points_to` MUST differ from `expected_source`
- AND the entry MUST be flagged as problematic

#### Scenario: Status JSON output includes module-map entries

- GIVEN a module-map target with mappings
- WHEN `agentsync status --json` is run
- THEN the JSON output MUST contain individual entries for each mapping
- AND each entry MUST follow the existing `StatusEntry` schema

---

### Requirement: Module-Map Gitignore Integration

The `all_gitignore_entries()` method MUST expand `module-map` targets into individual destination entries.

Each expanded entry MUST be the full resolved path (destination dir + resolved filename).

Backup patterns (`{destination}.bak`) MUST be generated for each expanded entry.

#### Scenario: Gitignore entries expanded from mappings

- GIVEN a module-map target for agent `"claude"` with mappings to `src/api` and `src/ui`
- WHEN `all_gitignore_entries()` is called
- THEN the entries MUST include `src/api/CLAUDE.md` and `src/ui/CLAUDE.md`
- AND the entries MUST include `src/api/CLAUDE.md.bak` and `src/ui/CLAUDE.md.bak`

#### Scenario: Gitignore entries with filename_override

- GIVEN a module-map mapping with `filename_override = "CUSTOM.md"` and `destination = "src/api"`
- WHEN `all_gitignore_entries()` is called
- THEN the entries MUST include `src/api/CUSTOM.md`
- AND MUST NOT include `src/api/CLAUDE.md`

#### Scenario: Gitignore skips disabled agents

- GIVEN a disabled agent with a module-map target
- WHEN `all_gitignore_entries()` is called
- THEN NO entries from that agent's module-map mappings MUST appear

#### Scenario: Gitignore deduplicates expanded entries

- GIVEN two agents whose module-map mappings resolve to the same destination path
- WHEN `all_gitignore_entries()` is called
- THEN each destination path MUST appear exactly once (BTreeSet deduplication)

---

### Requirement: Module-Map Doctor Validation

The doctor command MUST validate each mapping's source file exists in `source_dir`.

The doctor command MUST include expanded module-map destinations in the destination conflict check.

The doctor SHOULD warn when `mappings` is set on a target whose `sync_type` is not `module-map`.

The doctor SHOULD warn when `sync_type` is `module-map` but `mappings` is empty or absent.

#### Scenario: Doctor detects missing mapping source

- GIVEN a module-map target with 3 mappings, where the source for the second mapping does not exist
- WHEN `agentsync doctor` is run
- THEN the output MUST report the missing source file for the second mapping
- AND the output MUST identify the agent name, target name, and mapping source
- AND the issue count MUST be incremented

#### Scenario: Doctor detects destination conflict across mappings

- GIVEN two mappings (in the same or different targets) that resolve to the same destination path
- WHEN `agentsync doctor` is run
- THEN the output MUST report a duplicate destination conflict
- AND the issue count MUST be incremented

#### Scenario: Doctor detects destination overlap with other target types

- GIVEN a `symlink` target with `destination = "src/api/CLAUDE.md"` and a module-map mapping resolving to `src/api/CLAUDE.md`
- WHEN `agentsync doctor` is run
- THEN the output MUST report a duplicate destination conflict

#### Scenario: Doctor warns on mappings with wrong sync_type

- GIVEN a target with `type = "symlink"` that also has `mappings` configured
- WHEN `agentsync doctor` is run
- THEN the output SHOULD include a warning that `mappings` is only valid for `module-map` targets

#### Scenario: Doctor warns on empty module-map

- GIVEN a target with `type = "module-map"` but no `mappings` array (or empty array)
- WHEN `agentsync doctor` is run
- THEN the output SHOULD include a warning that the module-map target has no mappings configured

#### Scenario: Doctor passes for valid module-map config

- GIVEN a well-formed module-map target where all sources exist and no destinations conflict
- WHEN `agentsync doctor` is run
- THEN no module-map-related issues MUST be reported

---

### Requirement: TargetConfig Field Applicability

The `source` and `destination` fields on `TargetConfig` SHOULD be semantically reused for `module-map` targets:
- `source` field on TargetConfig is NOT used by `module-map` (individual mapping sources are used instead)
- `destination` field on TargetConfig is NOT used by `module-map` (individual mapping destinations are used instead)

The system MUST still require `source` and `destination` at the TOML parse level (they are required fields on `TargetConfig`). For `module-map` targets, users MAY set them to empty strings or placeholder values.

NOTE: This is a pragmatic trade-off to avoid breaking the existing struct. Doctor SHOULD guide users with a clear message if these fields are set to meaningful values on a module-map target, as they will be ignored.

#### Scenario: module-map target with placeholder source/destination

- GIVEN a module-map target with `source = ""` and `destination = ""` and valid mappings
- WHEN the config is loaded and sync is run
- THEN the mappings MUST be processed correctly
- AND the empty `source`/`destination` MUST NOT cause errors

---

## Non-Functional Requirements

### NF-1: Backward Compatibility

Adding `SyncType::ModuleMap` and the `mappings` field MUST NOT affect existing configurations. All existing tests MUST continue to pass without modification.

### NF-2: Performance

Module-map processing MUST be O(n) in the number of mappings. No recursive filesystem walks are needed (unlike `NestedGlob`).

### NF-3: Error Resilience

Processing SHOULD continue through all mappings even if individual mappings fail (e.g., missing source). Errors MUST be counted and reported in the `SyncResult`.

### NF-4: Consistent UX

Module-map output (status, dry-run, clean) MUST follow the same formatting patterns as existing sync types (emoji prefixes, color coding, path display).

---

## Acceptance Criteria

1. `SyncType::ModuleMap` parses correctly from TOML `"module-map"`
2. `ModuleMapping` struct deserializes with `source`, `destination`, and optional `filename_override`
3. `agent_convention_filename()` returns correct filenames for known agents and aliases
4. `agent_convention_filename()` returns `None` for unknown agents, with basename fallback handled by `resolve_module_map_filename()`
5. Filename resolution order: `filename_override` > convention > source basename
6. `process_module_map()` creates one symlink per mapping with correct source and destination
7. Intermediate destination directories are created automatically
8. `agentsync clean` removes all module-map symlinks
9. `agentsync status` shows individual entries per mapping
10. `agentsync doctor` validates per-mapping source existence
11. `agentsync doctor` detects destination conflicts including expanded module-map paths
12. `agentsync apply --dry-run` prints "Would..." for each mapping without filesystem changes
13. `.gitignore` entries are expanded from module-map mappings (one per resolved destination)
14. All existing tests pass without modification (no regressions)
15. New unit tests cover: config parsing, convention filenames, sync, clean, status, doctor, gitignore

---

## TOML Configuration Examples

### Mirror Layout (directory tree mirrors modules)

```toml
[agents.claude.targets.modules]
source = ""
destination = ""
type = "module-map"

[[agents.claude.targets.modules.mappings]]
source = "api-context.md"
destination = "src/api"

[[agents.claude.targets.modules.mappings]]
source = "ui-context.md"
destination = "src/ui"

[[agents.claude.targets.modules.mappings]]
source = "db-context.md"
destination = "src/db"
```

Result:
- `.agents/claude/api-context.md` -> `src/api/CLAUDE.md`
- `.agents/claude/ui-context.md` -> `src/ui/CLAUDE.md`
- `.agents/claude/db-context.md` -> `src/db/CLAUDE.md`

### Flat/Named Layout with Override

```toml
[agents.codex.targets.modules]
source = ""
destination = ""
type = "module-map"

[[agents.codex.targets.modules.mappings]]
source = "api-instructions.md"
destination = "packages/api"

[[agents.codex.targets.modules.mappings]]
source = "shared-utils.md"
destination = "packages/shared"
filename_override = "CONTRIBUTING-AGENTS.md"
```

Result:
- `.agents/codex/api-instructions.md` -> `packages/api/AGENTS.md`
- `.agents/codex/shared-utils.md` -> `packages/shared/CONTRIBUTING-AGENTS.md`

### Mixed with Existing Targets

```toml
[agents.claude.targets.root]
source = "AGENTS.md"
destination = "CLAUDE.md"
type = "symlink"

[agents.claude.targets.modules]
source = ""
destination = ""
type = "module-map"

[[agents.claude.targets.modules.mappings]]
source = "api-context.md"
destination = "src/api"
```
