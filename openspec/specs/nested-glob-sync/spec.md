# Specification: Nested-Glob Sync Type

**Type**: RETROSPEC  
**Date**: 2026-04-01  
**Status**: RETROSPEC  
**Source of Truth**: `src/linker.rs`, `src/config.rs`, `src/commands/doctor.rs`

## Purpose

Define the behavior of the `nested-glob` sync type, which recursively discovers files matching a
glob pattern under a search root directory and creates a symlink for each matched file at a
destination path computed by expanding a template string with per-file placeholders.

This is a **retrospec** — every requirement and scenario is traced to existing code behavior in
`src/linker.rs` and verified by existing tests.

### Cross-References

- **Base symlink behavior** (relative paths, backup, idempotency, update, dry-run messaging) is
  specified in `core-sync-engine/spec.md`. This spec covers only nested-glob-specific behavior.
- **Gitignore exclusion** for nested-glob template destinations is specified in
  `config-schema/spec.md` REQ-CS-014. Nested-glob destinations are templates, not literal paths,
  and MUST NOT be added to `.gitignore`.
- **Destination path safety** (absolute path rejection, traversal rejection, TOCTOU re-validation)
  is specified in `core-sync-engine/spec.md` REQ-006. Nested-glob applies these checks both to the
  template itself and to each expanded destination.

---

## Data Model

### TargetConfig Fields (nested-glob specific)

| Field         | Type             | Required | Description                                                               |
|---------------|------------------|----------|---------------------------------------------------------------------------|
| `source`      | `String`         | Yes      | Root directory to search, relative to **project root** (not `source_dir`) |
| `destination` | `String`         | Yes      | Template string with placeholders, expanded per matched file              |
| `sync_type`   | `SyncType`       | Yes      | Must be `"nested-glob"` (deserialized as `SyncType::NestedGlob`)          |
| `pattern`     | `Option<String>` | No       | Recursive glob pattern for file discovery (default: `**/AGENTS.md`)       |
| `exclude`     | `Vec<String>`    | No       | Glob patterns to exclude from traversal (matches relative paths)          |

### Destination Template Placeholders

| Placeholder       | Description                                                                         | Example (for `clients/agent-runtime/AGENTS.md`) |
|-------------------|-------------------------------------------------------------------------------------|-------------------------------------------------|
| `{relative_path}` | Parent directory of the matched file, relative to search root. `"."` for root files | `clients/agent-runtime`                         |
| `{file_name}`     | Full filename including extension                                                   | `AGENTS.md`                                     |
| `{stem}`          | Filename without extension                                                          | `AGENTS`                                        |
| `{ext}`           | File extension without leading dot                                                  | `md`                                            |

### SyncType Enum Entry

| Variant      | Serialized As   | Description                                                      |
|--------------|-----------------|------------------------------------------------------------------|
| `NestedGlob` | `"nested-glob"` | Recursively discover files and create template-expanded symlinks |

---

## Requirements

### REQ-NG-001: Source Path Resolution

The system MUST resolve the `source` field relative to the **project root**, not `source_dir`.

This differs from `symlink` and `symlink-contents` types, which resolve `source` relative to
`source_dir`.

The resolved path is the **search root** for recursive file discovery.

#### Scenario: SC-NG-001a — Source resolved relative to project root

- GIVEN a config with `source = "."` and the config file at `<project>/.agents/agentsync.toml`
- WHEN the nested-glob target is processed
- THEN the search root MUST be `<project>/` (the project root itself)

#### Scenario: SC-NG-001b — Source with subdirectory

- GIVEN a config with `source = "packages"`
- WHEN the nested-glob target is processed
- THEN the search root MUST be `<project>/packages/`

**Code**: `linker.rs:311-312` — `let search_root = self.project_root.join(&target.source);`

---

### REQ-NG-002: Default Glob Pattern

When `pattern` is not specified, the system MUST default to `**/AGENTS.md`.

#### Scenario: SC-NG-002a — Default pattern matches AGENTS.md files

- GIVEN a config with `type = "nested-glob"` and no `pattern` field
- WHEN the target is processed
- THEN only files named `AGENTS.md` at any depth MUST be matched

**Code**: `linker.rs:315` — `target.pattern.as_deref().unwrap_or("**/AGENTS.md")`

---

### REQ-NG-003: Recursive Directory Traversal

The system MUST walk the search root recursively using `WalkDir`.

The system MUST NOT follow symbolic links during traversal (`follow_links = false`).

The system MUST skip non-file entries (directories and other non-regular-file types).

The system MUST skip the search root entry itself (empty relative path).

#### Scenario: SC-NG-003a — Discovers files at multiple depths

- GIVEN a search root containing `clients/agent-runtime/AGENTS.md` and
  `modules/core-kmp/AGENTS.md`
- AND `pattern = "**/AGENTS.md"`
- WHEN the nested-glob target is processed
- THEN both files MUST be discovered and matched
- AND `SyncResult.created` MUST be 2

#### Scenario: SC-NG-003b — Skips directories

- GIVEN a search root containing directories that match the pattern name
- WHEN the nested-glob target is processed
- THEN only regular files MUST be matched, not directories

**Code**: `linker.rs:770` — `WalkDir::new(search_root).follow_links(false)`,
`linker.rs:828-830` — `if !entry.file_type().is_file() { continue; }`

---

### REQ-NG-004: Glob Pattern Matching

The system MUST match each discovered file's relative path (relative to search root) against the
configured glob pattern using `matches_path_glob`.

The glob matcher MUST support:

- `*` — matches any characters within a single path segment
- `**` — matches zero or more path segments

Only files whose relative path matches the pattern MUST be processed.

#### Scenario: SC-NG-004a — Pattern filters specific filenames

- GIVEN `pattern = "**/AGENTS.md"` and files `clients/AGENTS.md` and `clients/README.md`
- WHEN the nested-glob target is processed
- THEN only `clients/AGENTS.md` MUST be matched
- AND `clients/README.md` MUST NOT produce a symlink

**Code**: `linker.rs:832-834` — `if !matches_path_glob(&rel_str, glob_pattern) { continue; }`

---

### REQ-NG-005: Exclude Patterns

The system MUST support an `exclude` field containing a list of glob patterns.

Each discovered path (relative to search root) MUST be checked against all exclude patterns.

If a file or directory matches any exclude pattern, it MUST be skipped.

When a **directory** matches an exclude pattern, the system MUST prune the entire subtree
(`skip_current_dir`) to avoid descending into it.

When the exclude list is empty, no exclusion checks MUST be performed.

The system MUST use `find()` (not `any()`) for exclude matching to enable early-exit on first match.

#### Scenario: SC-NG-005a — Excludes node_modules

- GIVEN `exclude = [".agents/**", "node_modules/**"]`
- AND files exist at `clients/AGENTS.md` and `node_modules/some-pkg/AGENTS.md`
- WHEN the nested-glob target is processed
- THEN only `clients/AGENTS.md` MUST be matched
- AND `node_modules/some-pkg/AGENTS.md` MUST NOT produce a symlink
- AND `SyncResult.created` MUST be 1

#### Scenario: SC-NG-005b — Directory pruning prevents subtree traversal

- GIVEN `exclude = ["node_modules/**"]`
- AND the `node_modules` directory matches the exclude pattern
- WHEN the traversal encounters the `node_modules` directory entry
- THEN `skip_current_dir` MUST be called to prune the subtree

**Code**: `linker.rs:809-826` — exclude matching with `find()` and `skip_current_dir()`

---

### REQ-NG-006: Destination Template Expansion

The system MUST expand the destination template for each matched file by replacing placeholders with
values derived from the file's relative path.

The following placeholders MUST be supported:

- `{relative_path}` — the parent directory of the file relative to search root
- `{file_name}` — the file's full name including extension
- `{stem}` — the file name without extension
- `{ext}` — the file extension without leading dot

For files directly inside the search root (no parent directory), `{relative_path}` MUST expand to
`"."` to avoid producing absolute or empty path segments.

For files without an extension, `{ext}` MUST expand to an empty string.

For files without a stem (edge case), `{stem}` MUST expand to an empty string.

#### Scenario: SC-NG-006a — Template expansion for nested file

- GIVEN a matched file at `clients/agent-runtime/AGENTS.md` (relative to search root)
- AND `destination = "{relative_path}/CLAUDE.md"`
- WHEN the template is expanded
- THEN the result MUST be `"clients/agent-runtime/CLAUDE.md"`

#### Scenario: SC-NG-006b — Template expansion for root-level file

- GIVEN a matched file at `AGENTS.md` (directly in search root)
- AND `destination = "{relative_path}/{file_name}"`
- WHEN the template is expanded
- THEN the result MUST be `"./AGENTS.md"` (`{relative_path}` = `"."`)

#### Scenario: SC-NG-006c — Template with stem and ext placeholders

- GIVEN a matched file at `AGENTS.md`
- WHEN `{stem}` is expanded
- THEN it MUST produce `"AGENTS"`
- AND when `{ext}` is expanded it MUST produce `"md"`

#### Scenario: SC-NG-006d — Template with all placeholders for nested file

- GIVEN a matched file at `clients/agent-runtime/AGENTS.md`
- AND `destination = "{relative_path}/{file_name}"`
- WHEN the template is expanded
- THEN the result MUST be `"clients/agent-runtime/AGENTS.md"`

**Code**: `linker.rs:624-659` — `expand_destination_template()`

---

### REQ-NG-007: Destination Safety Validation (Pre-expansion)

The system MUST validate the raw destination template string via `ensure_safe_destination` before
processing any files.

If the template itself fails validation (e.g., contains `..` or is absolute), the system MUST
return an error before any traversal occurs.

#### Scenario: SC-NG-007a — Template validated before file traversal

- GIVEN a destination template of `"../escape/{file_name}"`
- WHEN the nested-glob target is processed
- THEN the system MUST return an error before any directory traversal

**Code**: `linker.rs:307-309` — `let _ = self.ensure_safe_destination(&target.destination)?;`

---

### REQ-NG-008: Destination Safety Validation (Post-expansion)

The system MUST validate each expanded destination path via `ensure_safe_destination`.

If an expanded destination is unsafe, the system MUST skip that file and increment
`SyncResult.skipped`.

In verbose mode, the system SHOULD print a skip message with the reason.

#### Scenario: SC-NG-008a — Invalid expanded destination skipped

- GIVEN a destination template of `"{relative_path}"` and a root-level file `AGENTS.md`
- WHEN the template expands to `"."` (an empty/current-dir path)
- THEN `ensure_safe_destination` MUST reject the path
- AND `SyncResult.skipped` MUST be 1
- AND `SyncResult.created` MUST be 0

**Code**: `linker.rs:726-740` — `ensure_safe_destination(&dest_str)` with skip on error

---

### REQ-NG-009: Empty Expanded Destination Handling

If the expanded destination template produces an empty string, the system MUST skip that file.

The system MUST increment `SyncResult.skipped`.

In verbose mode, the system SHOULD print a message indicating the template produced an empty path.

#### Scenario: SC-NG-009a — Empty expansion from extensionless file

- GIVEN a destination template of `"{ext}"` and a file named `AGENTS` (no extension)
- WHEN `{ext}` expands to `""`
- THEN the file MUST be skipped
- AND `SyncResult.skipped` MUST be 1

**Code**: `linker.rs:714-724` — empty string check before `ensure_safe_destination`

---

### REQ-NG-010: Symlink Creation per Matched File

For each matched file that passes validation, the system MUST create a symlink at the expanded
destination path pointing to the matched file's absolute path.

Symlink creation MUST delegate to `create_symlink()`, inheriting all base behavior from
`core-sync-engine/spec.md`:

- Relative symlink paths
- Idempotent skip when already correct
- Update when pointing to wrong target
- Backup of non-symlink files
- Parent directory creation

The system MUST accumulate `created`, `updated`, and `skipped` counts from each individual
`create_symlink()` call into the aggregate `SyncResult`.

#### Scenario: SC-NG-010a — Symlinks created for all matched files

- GIVEN files at `clients/agent-runtime/AGENTS.md` and `modules/core-kmp/AGENTS.md`
- AND `destination = "{relative_path}/CLAUDE.md"`
- WHEN `linker.sync()` is run
- THEN symlinks MUST exist at `clients/agent-runtime/CLAUDE.md` and `modules/core-kmp/CLAUDE.md`
- AND each symlink MUST point to the corresponding `AGENTS.md` file
- AND `SyncResult.created` MUST be 2

**Code**: `linker.rs:742-751` — `create_symlink(&resolved, &dest, options)` with result accumulation

---

### REQ-NG-011: Missing Search Root Handling

When the search root directory does not exist or is not a directory, the system MUST skip the
target.

The system MUST print a warning message indicating the missing search root.

The system MUST increment `SyncResult.skipped` by 1.

The system MUST NOT return an error.

#### Scenario: SC-NG-011a — Nonexistent source directory skipped

- GIVEN `source = "nonexistent-dir"` and the directory does not exist
- WHEN `linker.sync()` is run
- THEN `SyncResult.skipped` MUST be 1
- AND `SyncResult.created` MUST be 0
- AND no error MUST be returned

**Code**: `linker.rs:697-705` — existence check with skip and warning

---

### REQ-NG-012: No Matching Files (Empty Result)

When the search root exists but no files match the glob pattern (after exclusions), the system MUST
return a `SyncResult` with all counters at zero.

No error MUST be raised.

#### Scenario: SC-NG-012a — No matching files returns zero counts

- GIVEN a search root with files that do not match `pattern = "**/AGENTS.md"`
- WHEN `linker.sync()` is run
- THEN `SyncResult.created` MUST be 0
- AND `SyncResult.skipped` MUST be 0

**Code**: `linker.rs:695-696` — `SyncResult::default()` returned unchanged if no matches

---

### REQ-NG-013: WalkDir Error Handling

When `WalkDir` encounters an error for an individual entry (e.g., permission denied), the system
MUST skip that entry and continue traversal.

In verbose mode, the system SHOULD print a warning message with the error details.

The system MUST log the error via `tracing::debug!`.

#### Scenario: SC-NG-013a — Permission error on entry skipped gracefully

- GIVEN a directory entry that cannot be read (permission denied)
- WHEN the traversal encounters the error
- THEN the entry MUST be skipped
- AND traversal MUST continue with remaining entries

**Code**: `linker.rs:773-791` — `WalkDir` error handling with `continue`

---

### REQ-NG-014: Clean Operation

The `clean()` method MUST re-discover files using the same traversal and matching logic as sync
(`for_each_nested_glob_match`).

For each matched file, the system MUST expand the destination template and check if the expanded
path is a symlink.

If the destination is a symlink, the system MUST remove it (or report removal in dry-run).

If the destination is not a symlink, the system MUST NOT remove it.

The system MUST increment `SyncResult.removed` for each removed symlink.

If the search root does not exist, the clean operation MUST silently skip.

If the expanded destination is empty or fails safety validation, the clean operation MUST silently
skip that entry.

The system MUST re-validate the destination path via `revalidate_destination_path` immediately
before `fs::remove_file` (TOCTOU protection).

#### Scenario: SC-NG-014a — Clean removes previously created symlinks

- GIVEN nested-glob symlinks created by a prior sync
- WHEN `linker.clean()` is run
- THEN all symlinks at expanded destinations MUST be removed
- AND `SyncResult.removed` MUST equal the number of removed symlinks

#### Scenario: SC-NG-014b — Clean skips when search root missing

- GIVEN `source = "nonexistent-dir"` and the directory does not exist
- WHEN `linker.clean()` is run
- THEN `SyncResult.removed` MUST be 0
- AND no error MUST be returned

#### Scenario: SC-NG-014c — Clean skips invalid expanded destinations

- GIVEN a destination template of `"{relative_path}"` that expands to `"."`
- WHEN `linker.clean()` is run
- THEN `SyncResult.removed` MUST be 0

**Code**: `linker.rs:960-1012` — `clean()` NestedGlob branch

---

### REQ-NG-015: Dry-Run Mode

In dry-run mode, the system MUST NOT create any symlinks or directories.

The system MUST still traverse, match, expand templates, and accumulate counts.

Symlink creation delegates to `create_symlink()` which handles dry-run messaging per
`core-sync-engine/spec.md` REQ-014.

Clean in dry-run MUST print "Would remove: ..." messages without actually removing.

#### Scenario: SC-NG-015a — Dry-run creates no files

- GIVEN a valid nested-glob config with matching files
- AND `SyncOptions.dry_run = true`
- WHEN `linker.sync()` is run
- THEN no symlinks MUST exist on disk
- AND no directories MUST be created

#### Scenario: SC-NG-015b — Clean dry-run reports but preserves

- GIVEN existing nested-glob symlinks
- AND `SyncOptions.dry_run = true`
- WHEN `linker.clean()` is run
- THEN `SyncResult.removed` MUST reflect what would be removed
- AND symlinks MUST still exist on disk

**Code**: `linker.rs:996-1001` — dry-run branch in clean, `linker.rs:747` — delegates to
`create_symlink` which handles dry-run per core spec

---

### REQ-NG-016: Gitignore Exclusion

Nested-glob destination templates MUST NOT be added to `.gitignore` entries.

Because destinations are template strings (e.g., `{relative_path}/CLAUDE.md`), not literal paths,
including them verbatim in `.gitignore` would be meaningless.

#### Scenario: SC-NG-016a — Template not in gitignore

- GIVEN a nested-glob target with `destination = "{relative_path}/CLAUDE.md"`
- WHEN `all_gitignore_entries()` is called
- THEN the entries MUST NOT contain `"{relative_path}/CLAUDE.md"`

**Code**: `config.rs:360-364` — `if target.sync_type == SyncType::NestedGlob { continue; }`

---

### REQ-NG-017: Doctor Source Validation

The `doctor` command MUST validate that the nested-glob search root exists.

The search root MUST be resolved relative to the **project root** (not `source_dir`), matching
the resolution in `process_nested_glob`.

If the search root does not exist, doctor MUST report a `MissingSourceIssue`.

If the search root exists, doctor MUST report no issues for this target.

#### Scenario: SC-NG-017a — Doctor reports missing search root

- GIVEN `source = "nonexistent-dir"` and the directory does not exist
- WHEN doctor checks missing sources
- THEN a `MissingSourceIssue` MUST be reported with `path = <project_root>/nonexistent-dir`

#### Scenario: SC-NG-017b — Doctor passes when search root exists

- GIVEN `source = "."` and the project root exists
- WHEN doctor checks missing sources
- THEN no `MissingSourceIssue` MUST be reported

**Code**: `doctor.rs:482-495` — `SyncType::NestedGlob` branch in `check_missing_sources`

---

### REQ-NG-018: Doctor Destination Expansion

The `expand_target_destinations` function in doctor uses a catch-all `_` branch for nested-glob,
returning the raw template string as the destination.

This means nested-glob destinations appear as template strings (e.g., `{relative_path}/CLAUDE.md`)
in doctor's destination conflict checks.

Since template strings are unlikely to collide with literal destination paths, this is effectively
a no-op for conflict detection.

#### Scenario: SC-NG-018a — Doctor uses raw template for conflict check

- GIVEN a nested-glob target with `destination = "{relative_path}/CLAUDE.md"`
- WHEN `expand_target_destinations` is called
- THEN the result MUST contain the tuple `("{relative_path}/CLAUDE.md", agent, target)`

**Code**: `doctor.rs:531-536` — `_ => vec![(...)]` catch-all branch

---

### REQ-NG-019: Compression Not Applied

The `compress_agents_md` feature MUST NOT apply to nested-glob targets.

The `should_compress_agents_md` function explicitly restricts compression to `Symlink` and
`SymlinkContents` types only.

Nested-glob creates symlinks directly to the original discovered files without compression.

#### Scenario: SC-NG-019a — Compression skipped for nested-glob

- GIVEN `compress_agents_md = true` in config
- AND a nested-glob target matching `AGENTS.md` files
- WHEN `linker.sync()` is run
- THEN symlinks MUST point to the original `AGENTS.md` files, not `AGENTS.compact.md`

**Code**: `linker.rs:358-365` —
`matches!(target.sync_type, SyncType::Symlink | SyncType::SymlinkContents)`

---

## Non-Functional Requirements

### NF-NG-1: Traversal Safety

The WalkDir traversal MUST NOT follow symbolic links (`follow_links = false`) to prevent cycles
and escape-via-symlink attacks.

### NF-NG-2: Exclusion Performance

The system MUST use `find()` instead of `any()` for exclusion checks to enable early-exit on the
first matching pattern.

When the exclude list is empty, the system MUST avoid iteration overhead entirely.

### NF-NG-3: Subtree Pruning

When a directory matches an exclude pattern, the system MUST call `skip_current_dir()` to prune
the entire subtree from traversal, avoiding unnecessary filesystem I/O.

### NF-NG-4: Deterministic Path Normalization

Relative paths used for glob matching MUST be normalized using forward-slash (`/`) separators
regardless of platform, by joining path components with `/`.

**Code**: `linker.rs:799-803` — component-based join with `"/"`

---

## Acceptance Criteria

1. `source` is resolved relative to project root, not `source_dir`
2. Default pattern `**/AGENTS.md` is used when `pattern` is omitted
3. Recursive traversal discovers files at all depths
4. Symlinks are not followed during traversal
5. Non-file entries (directories) are skipped
6. Glob pattern correctly filters matched files
7. Exclude patterns skip matching files and prune matching directories
8. Destination template expands `{relative_path}`, `{file_name}`, `{stem}`, `{ext}` correctly
9. `{relative_path}` produces `"."` for root-level files
10. Destination template is validated before traversal
11. Each expanded destination is validated individually
12. Empty expanded destinations are skipped with `skipped` count
13. Invalid expanded destinations are skipped with `skipped` count
14. Missing search root produces `skipped = 1`, not an error
15. No matching files produces zero counts, no error
16. WalkDir errors are logged and skipped gracefully
17. Symlink creation delegates to `create_symlink()` (inheriting core-sync-engine behavior)
18. Clean re-discovers files and removes symlinks at expanded destinations
19. Clean skips missing search root silently
20. Clean skips non-symlink files at expanded destinations
21. Clean re-validates destination paths before removal (TOCTOU)
22. Dry-run creates no files, removes no files
23. Template destinations are excluded from `.gitignore`
24. Doctor validates search root existence relative to project root
25. `compress_agents_md` does not apply to nested-glob targets
26. All existing nested-glob tests pass (no regressions)

---

## Code References

| Requirement | Primary Code Location                               | Test Coverage                                                                                          |
|-------------|-----------------------------------------------------|--------------------------------------------------------------------------------------------------------|
| REQ-NG-001  | `linker.rs:311-312` (`project_root.join`)           | `test_nested_glob_creates_symlinks_for_discovered_files`                                               |
| REQ-NG-002  | `linker.rs:315` (default pattern)                   | `test_nested_glob_skips_missing_search_root` (no pattern)                                              |
| REQ-NG-003  | `linker.rs:770,828-830` (WalkDir + file check)      | `test_nested_glob_creates_symlinks_for_discovered_files`                                               |
| REQ-NG-004  | `linker.rs:832-834` (`matches_path_glob`)           | `test_nested_glob_creates_symlinks_for_discovered_files`                                               |
| REQ-NG-005  | `linker.rs:809-826` (exclude + `skip_current_dir`)  | `test_nested_glob_excludes_patterns`                                                                   |
| REQ-NG-006  | `linker.rs:624-659` (`expand_destination_template`) | `test_expand_destination_template_root_file`, `test_expand_destination_template_nested_file`           |
| REQ-NG-007  | `linker.rs:307-309` (pre-expansion validation)      | Implicit (safe_destination tests in core spec)                                                         |
| REQ-NG-008  | `linker.rs:726-740` (post-expansion validation)     | `test_nested_glob_sync_skips_invalid_expanded_destination`                                             |
| REQ-NG-009  | `linker.rs:714-724` (empty check)                   | `test_nested_glob_sync_skips_empty_expanded_destination`                                               |
| REQ-NG-010  | `linker.rs:742-751` (`create_symlink` delegation)   | `test_nested_glob_creates_symlinks_for_discovered_files`                                               |
| REQ-NG-011  | `linker.rs:697-705` (missing search root)           | `test_nested_glob_skips_missing_search_root`                                                           |
| REQ-NG-012  | `linker.rs:695-696` (default SyncResult)            | Implicit (no dedicated test)                                                                           |
| REQ-NG-013  | `linker.rs:773-791` (WalkDir error handling)        | Implicit (no dedicated test)                                                                           |
| REQ-NG-014  | `linker.rs:960-1012` (clean NestedGlob branch)      | `test_nested_glob_clean_removes_symlinks`, `test_nested_glob_clean_skips_invalid_expanded_destination` |
| REQ-NG-015  | `linker.rs:996-1001,747` (dry-run paths)            | `test_nested_glob_dry_run_does_not_create_files`                                                       |
| REQ-NG-016  | `config.rs:360-364` (gitignore skip)                | `test_nested_glob_destination_not_added_to_gitignore`                                                  |
| REQ-NG-017  | `doctor.rs:482-495` (doctor source check)           | Implicit (doctor integration)                                                                          |
| REQ-NG-018  | `doctor.rs:531-536` (expand_target_destinations)    | Implicit (catch-all branch)                                                                            |
| REQ-NG-019  | `linker.rs:358-365` (compression exclusion)         | Implicit (compress tests cover symlink/symlink-contents only)                                          |
