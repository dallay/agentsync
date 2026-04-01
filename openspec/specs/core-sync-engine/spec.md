# Specification: Core Sync Engine

**Type**: RETROSPEC  
**Date**: 2026-04-01  
**Status**: RETROSPEC  
**Source of Truth**: `src/linker.rs`, `src/config.rs`, `src/main.rs`

## Purpose

Define the behavior of the core sync engine (the `Linker` module) which creates, updates, and
removes symbolic links for AI agent configuration synchronization. This spec covers the `symlink`
and
`symlink-contents` sync types, the `SyncOptions` control surface, agent filtering, backup behavior,
clean operations, idempotency, error handling, path safety, and result reporting.

This is a **retrospec** — every requirement and scenario is traced to existing code behavior in
`src/linker.rs` and verified by existing tests.

---

## Data Model

### SyncOptions

Options controlling a sync or clean operation:

| Field     | Type                  | Default | Description                                       |
|-----------|-----------------------|---------|---------------------------------------------------|
| `clean`   | `bool`                | `false` | Remove existing symlinks before creating new ones |
| `dry_run` | `bool`                | `false` | Show what would be done without making changes    |
| `verbose` | `bool`                | `false` | Show detailed output                              |
| `agents`  | `Option<Vec<String>>` | `None`  | Filter to specific agents (from CLI `--agents`)   |

### SyncResult

Counters returned from sync/clean operations:

| Field     | Type    | Description                                         |
|-----------|---------|-----------------------------------------------------|
| `created` | `usize` | New symlinks created                                |
| `updated` | `usize` | Existing symlinks/files replaced (includes backups) |
| `skipped` | `usize` | Targets skipped (already correct or missing source) |
| `removed` | `usize` | Symlinks removed (clean operations)                 |
| `errors`  | `usize` | Targets that failed processing                      |

### SyncType Enum (subset covered by this spec)

| Variant           | Serialized As        | Description                                              |
|-------------------|----------------------|----------------------------------------------------------|
| `Symlink`         | `"symlink"`          | Creates a single symlink from source to destination      |
| `SymlinkContents` | `"symlink-contents"` | Creates symlinks for each item inside a source directory |

### TargetConfig (fields relevant to this spec)

| Field         | Type             | Required | Description                                                   |
|---------------|------------------|----------|---------------------------------------------------------------|
| `source`      | `String`         | Yes      | Source file or directory, relative to `source_dir`            |
| `destination` | `String`         | Yes      | Destination path for the symlink, relative to project root    |
| `sync_type`   | `SyncType`       | Yes      | The type of synchronization (`symlink` or `symlink-contents`) |
| `pattern`     | `Option<String>` | No       | For `symlink-contents`: glob filter for items to link         |

### Linker Struct

| Field                | Type                                 | Description                                 |
|----------------------|--------------------------------------|---------------------------------------------|
| `config`             | `Config`                             | Parsed agentsync.toml configuration         |
| `config_path`        | `PathBuf`                            | Path to the configuration file              |
| `project_root`       | `PathBuf`                            | Root directory of the project               |
| `source_dir`         | `PathBuf`                            | Directory where agent source files live     |
| `path_cache`         | `RefCell<HashMap<PathBuf, PathBuf>>` | Cached canonicalized paths for performance  |
| `compression_cache`  | `RefCell<HashMap<PathBuf, String>>`  | Cached compressed AGENTS.md content         |
| `ensured_dirs`       | `RefCell<HashSet<PathBuf>>`          | Tracks directories already ensured to exist |
| `ensured_compressed` | `RefCell<HashSet<PathBuf>>`          | Tracks compressed files already written     |

---

## Requirements

### REQ-001: Linker Initialization

The system MUST create a `Linker` from a `Config` and a config file path.

The system MUST derive `project_root` from the config path: if the config is inside a `.agents/`
directory, the project root is the parent of `.agents/`; otherwise it is the config file's parent
directory.

The system MUST derive `source_dir` by joining the config file's parent directory with the
`source_dir` field from config (defaults to `"."`).

#### Scenario: SC-001a — Linker created from config inside .agents

- GIVEN a config file at `<tmpdir>/.agents/agentsync.toml`
- WHEN a Linker is created
- THEN `project_root` MUST equal `<tmpdir>`

#### Scenario: SC-001b — Linker created from config at project root

- GIVEN a config file at `<tmpdir>/agentsync.toml`
- WHEN a Linker is created
- THEN `project_root` MUST equal `<tmpdir>`

---

### REQ-002: Symlink Sync Type (Apply)

The system MUST create a single symlink from the resolved source path to the destination path for
targets with `type = "symlink"`.

The symlink MUST use a relative path from the destination to the source.

The source path MUST be resolved relative to `source_dir`.

The destination path MUST be resolved relative to `project_root`.

#### Scenario: SC-002a — Basic symlink creation

- GIVEN a config with `source = "AGENTS.md"`, `destination = "TEST.md"`, `type = "symlink"`
- AND the source file exists at `<source_dir>/AGENTS.md`
- WHEN `linker.sync()` is run
- THEN a symlink MUST be created at `<project_root>/TEST.md`
- AND `SyncResult.created` MUST be 1

#### Scenario: SC-002b — Symlink for a directory

- GIVEN a config with `source = "skills"`, `destination = "output_skills"`, `type = "symlink"`
- AND the source is a directory containing subdirectories with files
- WHEN `linker.sync()` is run
- THEN a single symlink MUST be created at `<project_root>/output_skills`
- AND the symlink MUST point to the source directory
- AND subdirectory contents MUST be accessible through the symlink

---

### REQ-003: Symlink-Contents Sync Type (Apply)

The system MUST create individual symlinks for each item inside the source directory at the
destination directory for targets with `type = "symlink-contents"`.

The destination directory MUST be created if it does not exist.

Each symlink MUST use a relative path from the destination to the source item.

#### Scenario: SC-003a — Symlink-contents creates links for all items

- GIVEN a config with `source = "skills"`, `destination = "output_skills"`,
  `type = "symlink-contents"`
- AND the source directory contains `skill1.md`, `skill2.md`, and `readme.txt`
- WHEN `linker.sync()` is run
- THEN symlinks MUST be created for all 3 items inside `output_skills/`
- AND `SyncResult.created` MUST be 3

#### Scenario: SC-003b — Symlink-contents with pattern filter

- GIVEN a config with `type = "symlink-contents"` and `pattern = "*.md"`
- AND the source directory contains `skill1.md`, `skill2.md`, and `readme.txt`
- WHEN `linker.sync()` is run
- THEN only `skill1.md` and `skill2.md` MUST be linked
- AND `readme.txt` MUST NOT be linked
- AND `SyncResult.created` MUST be 2

#### Scenario: SC-003c — Symlink-contents with missing source directory

- GIVEN a config with `type = "symlink-contents"`
- AND the source directory does not exist
- WHEN `linker.sync()` is run
- THEN `SyncResult.skipped` MUST be 1
- AND no symlinks MUST be created

---

### REQ-004: Relative Symlink Path Resolution

The system MUST calculate symlink targets as relative paths from the destination's parent directory
to the source file's canonical location.

The system MUST use canonicalized (resolved) paths for accurate relative path calculation.

When the destination directory does not yet exist, the system MUST compute the relative path using
the project root as a base.

The system MUST cache canonicalized paths via `path_cache` to avoid redundant filesystem I/O.

#### Scenario: SC-004a — Relative path for simple symlink

- GIVEN a source at `.agents/AGENTS.md` and destination at `TEST.md` (both in project root)
- WHEN the symlink is created
- THEN the symlink target MUST be a relative path (e.g., `.agents/AGENTS.md`)
- AND the symlink MUST NOT use an absolute path

#### Scenario: SC-004b — Relative path for nested destination

- GIVEN a source at `.agents/AGENTS.md` and destination at `deep/nested/dir/TEST.md`
- WHEN the symlink is created
- THEN the symlink target MUST be a relative path traversing up from `deep/nested/dir/` to
  `.agents/AGENTS.md`

---

### REQ-005: Parent Directory Creation

The system MUST create parent directories for a destination path if they do not exist.

The system MUST use `fs::create_dir_all` for recursive directory creation.

The system MUST cache which directories have been ensured via `ensured_dirs` to avoid redundant I/O.

In dry-run mode, the system MUST NOT create directories but SHOULD print a "Would create directory"
message when verbose is enabled.

#### Scenario: SC-005a — Creates intermediate directories

- GIVEN a destination of `deep/nested/dir/TEST.md` where the directories do not exist
- WHEN `linker.sync()` is run
- THEN the directories `deep/nested/dir/` MUST be created
- AND the symlink MUST be created inside the directory

#### Scenario: SC-005b — Dry-run does not create directories

- GIVEN a destination requiring directory creation
- WHEN `linker.sync()` is run with `dry_run = true`
- THEN no directories MUST be created on disk

---

### REQ-006: Destination Path Safety Validation

The system MUST validate all destination paths before any filesystem mutation.

The system MUST reject absolute destination paths.

The system MUST reject destination paths containing parent directory traversal (`..`).

The system MUST reject empty destination paths (e.g., `""` or `"."`).

The system MUST verify that the resolved destination path is within the project root by
canonicalizing the nearest existing ancestor and checking it starts with the canonicalized project
root.

The system MUST re-validate the destination path immediately before each filesystem mutation
(`revalidate_destination_path`) to guard against TOCTOU race conditions where symlink ancestors may
be swapped between validation and mutation.

#### Scenario: SC-006a — Rejects absolute path

- GIVEN a destination that is an absolute path (e.g., `/tmp/escape.md`)
- WHEN the destination is validated
- THEN the system MUST return an error

#### Scenario: SC-006b — Rejects parent traversal

- GIVEN a destination of `../escape.md`
- WHEN the destination is validated
- THEN the system MUST return an error

#### Scenario: SC-006c — Rejects empty destination

- GIVEN a destination of `""` or `"."`
- WHEN the destination is validated
- THEN the system MUST return an error

#### Scenario: SC-006d — Rejects symlink ancestor escape

- GIVEN a symlink `escape-link` inside the project root that points outside the project
- AND a destination of `escape-link/linked.md`
- WHEN the destination is validated
- THEN the system MUST return an error (canonicalization resolves the escape)

#### Scenario: SC-006e — Accepts valid relative paths

- GIVEN a destination of `nested/output.md`
- WHEN the destination is validated
- THEN it MUST resolve to `<project_root>/nested/output.md`

#### Scenario: SC-006f — Detects TOCTOU symlink swap

- GIVEN a symlink `dynamic-link` initially pointing to a safe directory inside the project
- AND the destination `dynamic-link/linked.md` passes initial validation
- WHEN the symlink target is swapped to a directory outside the project before mutation
- THEN re-validation MUST detect the escape and return an error

---

### REQ-007: Idempotent Re-run (Skip Already Correct)

When the destination is already a symlink pointing to the correct relative source path, the system
MUST skip re-creation.

The system MUST increment `SyncResult.skipped` for skipped symlinks.

In verbose mode, the system SHOULD print an "Already linked" message.

#### Scenario: SC-007a — Second sync skips already-correct symlink

- GIVEN a symlink target that already exists and points to the correct source
- WHEN `linker.sync()` is run a second time
- THEN `SyncResult.created` MUST be 0
- AND `SyncResult.updated` MUST be 0
- AND `SyncResult.skipped` MUST be 1
- AND the existing symlink MUST remain unchanged

---

### REQ-008: Update Existing Symlink with Wrong Target

When the destination is a symlink but points to a different target than expected, the system MUST
remove the old symlink and create a new one pointing to the correct source.

The system MUST increment `SyncResult.updated`.

#### Scenario: SC-008a — Updates symlink pointing to wrong source

- GIVEN an existing symlink at `TEST.md` pointing to `source1.md`
- AND the config now specifies `source = "source2.md"`
- WHEN `linker.sync()` is run
- THEN the symlink MUST be updated to point to `source2.md`
- AND `SyncResult.updated` MUST be 1
- AND `SyncResult.created` MUST be 0

---

### REQ-009: Backup Existing Non-Symlink Files

When the destination exists as a regular file or directory (not a symlink), the system MUST back it
up before replacing it with a symlink.

The backup MUST be created at `<destination>.bak`.

If a `.bak` file/directory already exists, the system MUST remove the old backup before creating the
new one.

The system MUST increment `SyncResult.updated`.

#### Scenario: SC-009a — Backs up existing regular file

- GIVEN a regular file at `output_skills` (not a symlink)
- AND the config specifies `destination = "output_skills"` with `type = "symlink"`
- WHEN `linker.sync()` is run
- THEN the original file/directory MUST be moved to `output_skills.bak`
- AND a symlink MUST be created at `output_skills`
- AND `SyncResult.updated` MUST be >= 1

#### Scenario: SC-009b — Replaces stale backup

- GIVEN both `output_skills` (a directory) and `output_skills.bak` (stale backup) exist
- WHEN `linker.sync()` creates a new backup
- THEN `output_skills.bak` MUST contain the contents from the current `output_skills`
- AND the stale backup content MUST be removed

---

### REQ-010: Missing Source Handling

When a source file does not exist, the system MUST skip the target without error.

The system MUST print a warning message indicating the missing source.

The system MUST increment `SyncResult.skipped`.

#### Scenario: SC-010a — Skips missing source file

- GIVEN a config with `source = "NONEXISTENT.md"`
- AND the source file does not exist
- WHEN `linker.sync()` is run
- THEN `SyncResult.skipped` MUST be 1
- AND `SyncResult.created` MUST be 0
- AND no symlink MUST be created

---

### REQ-011: Disabled Agent Skipping

When an agent has `enabled = false`, the system MUST skip all of its targets.

In verbose mode, the system SHOULD print a "Skipping disabled agent" message.

#### Scenario: SC-011a — Skips disabled agent

- GIVEN an agent with `enabled = false`
- WHEN `linker.sync()` is run
- THEN no symlinks MUST be created for that agent
- AND `SyncResult.created` MUST be 0

---

### REQ-012: Agent Filtering (CLI --agents)

When `SyncOptions.agents` is set (from CLI `--agents`), the system MUST only process agents whose
names match the filter.

Matching MUST use the `sync_filter_matches` function from `agent_ids`, which supports:

- Case-insensitive matching
- Alias resolution (e.g., `"codex-cli"` matches agent named `"codex"`)
- Substring matching for legacy compatibility

The CLI `--agents` filter MUST take priority over `default_agents` in config.

#### Scenario: SC-012a — Filters to single agent

- GIVEN agents `claude` and `copilot` both enabled
- AND `SyncOptions.agents = Some(["claude"])`
- WHEN `linker.sync()` is run
- THEN only `claude`'s symlinks MUST be created
- AND `copilot`'s symlinks MUST NOT be created

#### Scenario: SC-012b — Case-insensitive filter matching

- GIVEN an agent named `"GitHub-Copilot"`
- AND `SyncOptions.agents = Some(["copilot"])`
- WHEN `linker.sync()` is run
- THEN the agent MUST be processed (case-insensitive match)

#### Scenario: SC-012c — Alias filter matching

- GIVEN an agent named `"codex"`
- AND `SyncOptions.agents = Some(["codex-cli"])`
- WHEN `linker.sync()` is run
- THEN the agent MUST be processed (alias resolution)

#### Scenario: SC-012d — CLI --agents overrides default_agents

- GIVEN `default_agents = ["claude"]` in config
- AND `SyncOptions.agents = Some(["copilot"])`
- WHEN `linker.sync()` is run
- THEN only `copilot` MUST be processed
- AND `claude` MUST NOT be processed

---

### REQ-013: Default Agents Config

When `SyncOptions.agents` is `None` (no CLI filter) and `default_agents` is non-empty in the config,
the system MUST only process agents matching the `default_agents` list.

The matching uses the same `sync_filter_matches` logic (case-insensitive, alias-aware).

When both `SyncOptions.agents` is `None` and `default_agents` is empty, the system MUST process all
enabled agents.

#### Scenario: SC-013a — Uses default_agents when no CLI filter

- GIVEN `default_agents = ["claude", "copilot"]` and agents `claude`, `copilot`, `cursor` all
  enabled
- AND no CLI `--agents` filter
- WHEN `linker.sync()` is run
- THEN `claude` and `copilot` MUST be processed
- AND `cursor` MUST NOT be processed

#### Scenario: SC-013b — Default_agents case-insensitive with aliases

- GIVEN `default_agents = ["CLAUDE", "COPILOT"]` and agents named `claude-code` and
  `GitHub-Copilot`
- WHEN `linker.sync()` is run with no CLI filter
- THEN both agents MUST be processed

#### Scenario: SC-013c — All enabled agents when no default_agents and no CLI filter

- GIVEN no `default_agents` in config and no CLI `--agents` filter
- AND agents `claude` and `copilot` both enabled
- WHEN `linker.sync()` is run
- THEN both agents MUST be processed

---

### REQ-014: Dry-Run Mode

When `SyncOptions.dry_run` is `true`, the system MUST NOT make any filesystem changes.

The system MUST print "Running in dry-run mode" at the start.

The system MUST print "Would link: ..." messages for symlinks that would be created.

The system MUST print "Would update symlink: ..." for symlinks that would be updated.

The system MUST print "Would backup and replace: ..." for non-symlink files that would be backed up.

The system MUST still populate `SyncResult` counters accurately.

#### Scenario: SC-014a — Dry-run creates no files

- GIVEN a valid config with a symlink target
- AND `SyncOptions.dry_run = true`
- WHEN `linker.sync()` is run
- THEN no symlinks MUST exist on disk
- AND no directories MUST be created

#### Scenario: SC-014b — Dry-run reports accurate counts

- GIVEN a valid config that would create 1 symlink
- AND `SyncOptions.dry_run = true`
- WHEN `linker.sync()` is run
- THEN `SyncResult.created` MUST be 1

---

### REQ-015: Clean Operation (Symlink Type)

The `clean()` method MUST remove the destination symlink for `type = "symlink"` targets.

The system MUST only remove files that are symlinks (not regular files).

If the destination is not a symlink, the system MUST NOT remove it.

The system MUST increment `SyncResult.removed` for each removed symlink.

#### Scenario: SC-015a — Clean removes symlink target

- GIVEN a synced symlink at `TEST.md`
- WHEN `linker.clean()` is run
- THEN the symlink MUST be removed
- AND `SyncResult.removed` MUST be 1

#### Scenario: SC-015b — Clean skips non-symlink files

- GIVEN a regular file at a destination path (not a symlink)
- WHEN `linker.clean()` is run
- THEN the file MUST NOT be removed

---

### REQ-016: Clean Operation (Symlink-Contents Type)

The `clean()` method MUST iterate the destination directory and remove all symlinks inside it for
`type = "symlink-contents"` targets.

The system MUST only remove entries that are symlinks (not regular files inside the directory).

After removing symlinks, the system SHOULD attempt to remove the destination directory if it is
empty (best-effort, no error on failure).

#### Scenario: SC-016a — Clean removes symlink-contents symlinks

- GIVEN a synced `symlink-contents` target with 2 items linked inside `output_skills/`
- WHEN `linker.clean()` is run
- THEN both symlinks inside `output_skills/` MUST be removed
- AND `SyncResult.removed` MUST be 2

---

### REQ-017: Clean Dry-Run

When `SyncOptions.dry_run` is `true`, the `clean()` method MUST NOT remove any symlinks.

The system MUST print "Would remove: ..." messages for each symlink that would be removed.

The system MUST still populate `SyncResult.removed` accurately.

#### Scenario: SC-017a — Clean dry-run preserves symlinks

- GIVEN a synced symlink at `TEST.md`
- AND `SyncOptions.dry_run = true`
- WHEN `linker.clean()` is run
- THEN `SyncResult.removed` MUST be 1
- AND the symlink MUST still exist on disk

---

### REQ-018: Apply with Prior Clean (--clean flag)

When the `apply` command is invoked with `--clean`, the system MUST run `linker.clean()` before
running `linker.sync()`.

The clean step MUST use the same `dry_run` and `verbose` options as the apply step.

The clean step MUST NOT pass an `agents` filter (it cleans all managed symlinks regardless of
agent filtering).

#### Scenario: SC-018a — Apply with --clean removes then recreates

- GIVEN an existing synced state with symlinks
- WHEN `agentsync apply --clean` is run
- THEN existing symlinks MUST be removed first
- AND then new symlinks MUST be created

---

### REQ-019: Target Processing Error Handling

When processing a target fails (e.g., permission error, I/O error), the system MUST catch the error
and increment `SyncResult.errors`.

The system MUST continue processing remaining targets for the same and other agents.

The error MUST be logged via `tracing::error!`.

#### Scenario: SC-019a — Error in one target does not block others

- GIVEN a config with multiple targets, one of which has an invalid destination
- WHEN `linker.sync()` is run
- THEN `SyncResult.errors` MUST be incremented for the failing target
- AND valid targets MUST still be processed successfully

---

### REQ-020: Gitignore Entry Generation

The system MUST generate gitignore entries for all enabled agents' target destinations.

For `symlink` and `symlink-contents` targets, the system MUST include both the destination path and
a `<destination>.bak` pattern.

Entries MUST be deduplicated using a `BTreeSet`.

Destination paths without path separators, wildcards, or special characters MUST be prefixed with
`/` to anchor them.

Disabled agents MUST NOT contribute gitignore entries.

The system MUST include a defensive pattern `.agents/skills/*.bak`.

#### Scenario: SC-020a — Gitignore entries for symlink targets

- GIVEN an enabled agent with `destination = "CLAUDE.md"` and `type = "symlink"`
- WHEN `all_gitignore_entries()` is called
- THEN the entries MUST include `/CLAUDE.md` and `/CLAUDE.md.bak`

#### Scenario: SC-020b — Gitignore skips disabled agents

- GIVEN a disabled agent with targets
- WHEN `all_gitignore_entries()` is called
- THEN no entries from that agent's targets MUST appear

#### Scenario: SC-020c — Gitignore deduplicates entries

- GIVEN two agents with identical destination paths
- WHEN `all_gitignore_entries()` is called
- THEN each path MUST appear exactly once

---

### REQ-021: Compress AGENTS.md

When `compress_agents_md = true` in config, the system MUST generate a compressed version of any
source file named `AGENTS.md` before linking.

Compression MUST:

- Collapse consecutive blank lines to a single blank line
- Normalize inline whitespace (multiple spaces/tabs → single space)
- Preserve leading whitespace (indentation)
- Preserve content inside fenced code blocks (``` or ~~~) verbatim

The compressed file MUST be named `AGENTS.compact.md` in the same directory as the source.

The symlink MUST point to the compressed file instead of the original.

Compression MUST apply to both `symlink` and `symlink-contents` sync types.

Compression MUST NOT apply to `nested-glob` or `module-map` sync types.

The system MUST skip re-writing the compressed file if its content is unchanged (content-based
caching).

#### Scenario: SC-021a — Compression creates compact file and links to it

- GIVEN `compress_agents_md = true` and a source `AGENTS.md` with extra whitespace
- WHEN `linker.sync()` is run
- THEN `AGENTS.compact.md` MUST be created alongside the source
- AND the destination symlink MUST point to `AGENTS.compact.md`
- AND inline whitespace MUST be normalized
- AND code block content MUST be preserved verbatim

#### Scenario: SC-021b — Compression in symlink-contents

- GIVEN `compress_agents_md = true` and a `symlink-contents` target whose source directory contains
  `AGENTS.md` and `OTHER.md`
- WHEN `linker.sync()` is run
- THEN `AGENTS.compact.md` MUST be created
- AND the symlink for `AGENTS.md` MUST point to `AGENTS.compact.md`
- AND the symlink for `OTHER.md` MUST point to the original `OTHER.md`

---

### REQ-022: Cache Clearing Between Sync Runs

The system MUST clear all internal caches (`path_cache`, `compression_cache`, `ensured_dirs`,
`ensured_compressed`) at the start of each `sync()` call.

This ensures that filesystem changes between consecutive runs on the same `Linker` instance are
reflected correctly.

#### Scenario: SC-022a — Caches reset between runs

- GIVEN a `Linker` instance that has already run `sync()`
- AND the source file is modified between runs
- WHEN `sync()` is run again on the same instance
- THEN the updated content MUST be reflected (caches are cleared)

---

### REQ-023: Cross-Platform Symlink Creation

On Unix systems, the system MUST use `std::os::unix::fs::symlink`.

On Windows systems, the system MUST use `std::os::windows::fs::symlink_dir` for directory sources
and `std::os::windows::fs::symlink_file` for file sources.

On Windows, the `remove_symlink` helper MUST use `fs::remove_dir` for directory symlinks (detected
via `FileTypeExt::is_symlink_dir`) and `fs::remove_file` for file symlinks.

#### Scenario: SC-023a — Unix symlink creation

- GIVEN a Unix platform
- WHEN a symlink is created
- THEN `std::os::unix::fs::symlink` MUST be used

---

### REQ-024: Apply Command Integration

The `apply` command MUST:

1. Optionally run `linker.clean()` first if `--clean` flag is set
2. Run `linker.sync()` with the provided options
3. Update `.gitignore` if gitignore is enabled and `--no-gitignore` is not set
4. Sync MCP configurations if MCP is enabled and servers are configured

The final output MUST print a summary with created, updated, skipped, and error counts.

#### Scenario: SC-024a — Full apply flow

- GIVEN a valid config with enabled agents and gitignore enabled
- WHEN `agentsync apply` is run
- THEN symlinks MUST be created
- AND `.gitignore` MUST be updated
- AND a summary MUST be printed

---

## Non-Functional Requirements

### NF-1: Idempotency

Running `agentsync apply` multiple times with unchanged config and sources MUST produce the same
filesystem state. The second and subsequent runs SHOULD report all targets as skipped.

### NF-2: Performance

The Linker MUST cache canonicalized paths (`path_cache`), ensured directories (`ensured_dirs`),
and compressed content (`compression_cache`) to minimize redundant filesystem I/O within a single
sync run.

### NF-3: Error Resilience

Processing MUST continue through all agents and targets even if individual targets fail. Errors
MUST be counted in `SyncResult.errors` and logged via `tracing::error!`.

### NF-4: Security

All destination paths MUST be validated to prevent directory traversal attacks. Destinations MUST
resolve within the project root. Re-validation MUST occur immediately before each filesystem write
to prevent TOCTOU exploits.

### NF-5: Deterministic Ordering

Agent processing order MUST follow `BTreeMap` ordering (alphabetical by agent name). This ensures
deterministic output across runs.

---

## Acceptance Criteria

1. `Linker::new()` correctly derives `project_root` and `source_dir` from config path location
2. `SyncType::Symlink` creates a single relative symlink from source to destination
3. `SyncType::SymlinkContents` creates individual symlinks for each item inside the source directory
4. `SymlinkContents` with `pattern` filters items by glob match
5. Destination paths are validated: rejects absolute, traversal, empty, and escape-via-symlink
6. Re-validation occurs before every filesystem mutation (TOCTOU protection)
7. Missing source files result in `skipped` count, not errors
8. Disabled agents are completely skipped
9. CLI `--agents` filter works case-insensitively with alias support
10. `default_agents` config filters agents when no CLI filter is provided
11. CLI `--agents` overrides `default_agents`
12. All enabled agents processed when neither CLI filter nor default_agents are set
13. Existing correct symlinks are skipped (idempotent)
14. Existing symlinks with wrong targets are updated (removed + recreated)
15. Existing non-symlink files are backed up to `<dest>.bak` before replacement
16. Stale `.bak` files are removed before creating new backups
17. `--dry-run` makes no filesystem changes but reports accurate counts
18. `clean()` removes managed symlinks for all sync types
19. `clean()` only removes symlinks, never regular files
20. `clean()` attempts to remove empty destination directories for `symlink-contents`
21. `clean --dry-run` reports but does not remove
22. `--clean` flag on apply runs clean before sync
23. `compress_agents_md` generates `AGENTS.compact.md` and redirects symlinks
24. Compression preserves code blocks and normalizes inline whitespace
25. Caches are cleared between consecutive `sync()` calls on the same Linker
26. `.gitignore` entries include destinations and `.bak` patterns for enabled agents
27. All existing tests pass (no regressions)

---

## Code References

| Requirement | Primary Code Location                                                                            | Test Coverage                                                                                                                                                                                          |
|-------------|--------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| REQ-001     | `Linker::new()` (linker.rs:64-78)                                                                | `test_linker_new`, `test_linker_project_root_accessor`                                                                                                                                                 |
| REQ-002     | `process_target` → `create_symlink` (linker.rs:291-295)                                          | `test_sync_creates_symlink`, `test_sync_symlink_directory_for_skills`                                                                                                                                  |
| REQ-003     | `create_symlinks_for_contents` (linker.rs:563-613)                                               | `test_sync_symlink_contents`, `test_sync_symlink_contents_with_pattern`                                                                                                                                |
| REQ-004     | `relative_path` (linker.rs:917-949)                                                              | Implicit in all symlink creation tests                                                                                                                                                                 |
| REQ-005     | `ensure_directory` (linker.rs:369-390)                                                           | `test_sync_creates_parent_directories`                                                                                                                                                                 |
| REQ-006     | `ensure_safe_destination` (linker.rs:137-167), `revalidate_destination_path` (linker.rs:170-173) | `test_ensure_safe_destination_*` (4 tests)                                                                                                                                                             |
| REQ-007     | `create_symlink` already-linked branch (linker.rs:469-476)                                       | `test_sync_skips_already_correct_symlink`                                                                                                                                                              |
| REQ-008     | `create_symlink` wrong-target branch (linker.rs:477-498)                                         | `test_sync_updates_existing_symlink_with_different_target`                                                                                                                                             |
| REQ-009     | `create_symlink` regular-file branch (linker.rs:500-524)                                         | `test_sync_symlink_directory_upgrades_existing_dir`, `test_sync_symlink_directory_replaces_existing_backup`                                                                                            |
| REQ-010     | `create_symlink` source-check (linker.rs:449-457)                                                | `test_sync_skips_missing_source`                                                                                                                                                                       |
| REQ-011     | `sync` enabled check (linker.rs:213-218)                                                         | `test_sync_skips_disabled_agents`                                                                                                                                                                      |
| REQ-012     | `sync` agents filter (linker.rs:222-231)                                                         | `test_sync_filters_by_agent_name`, `test_sync_filters_by_agent_name_case_insensitive`, `test_sync_cli_filter_supports_aliases`, `test_sync_cli_agents_overrides_default_agents`                        |
| REQ-013     | `sync` default_agents (linker.rs:232-247)                                                        | `test_sync_uses_default_agents_when_no_cli_filter`, `test_sync_default_agents_case_insensitive`, `test_sync_default_agents_support_aliases`, `test_sync_all_enabled_when_no_default_agents_and_no_cli` |
| REQ-014     | `create_symlink` dry-run branches                                                                | `test_sync_dry_run_does_not_create_files`                                                                                                                                                              |
| REQ-015     | `clean` Symlink branch (linker.rs:1053-1068)                                                     | `test_clean_removes_symlinks`                                                                                                                                                                          |
| REQ-016     | `clean` SymlinkContents branch (linker.rs:1014-1051)                                             | `test_clean_symlink_contents`                                                                                                                                                                          |
| REQ-017     | `clean` dry-run paths                                                                            | `test_clean_dry_run`                                                                                                                                                                                   |
| REQ-018     | `main.rs` apply command (main.rs:194-201)                                                        | Integration via CLI                                                                                                                                                                                    |
| REQ-019     | `sync` error catch (linker.rs:270-274)                                                           | Implicit; error paths in process_target                                                                                                                                                                |
| REQ-020     | `Config::all_gitignore_entries` (config.rs:351-393)                                              | Config module tests                                                                                                                                                                                    |
| REQ-021     | `should_compress_agents_md`, `write_compressed_agents_md`, `compress_agents_md_content`          | `test_sync_compresses_agents_md_when_enabled`, `test_sync_symlink_contents_compresses_agents_md`                                                                                                       |
| REQ-022     | `sync` cache clearing (linker.rs:200-203)                                                        | `test_sync_resets_caches_between_runs`                                                                                                                                                                 |
| REQ-023     | Platform-specific symlink creation (linker.rs:538-549)                                           | Unix tests (cfg(unix))                                                                                                                                                                                 |
| REQ-024     | `main.rs` apply flow (main.rs:193-259)                                                           | Integration via CLI                                                                                                                                                                                    |
