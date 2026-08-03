# Delta for Core Sync Engine

## ADDED Requirements

### Requirement: Stable Facade and Focused Internals

The refactor MUST preserve the public `agentsync::linker` API and signatures for `Linker`,
`SyncOptions`, `SyncResult`, `SymlinkContentsChildExpectation`, construction, accessors, `sync`,
`clean`, `sync_mcp`, `expected_source_path`, and `symlink_contents_expected_children`. Internal
responsibilities MUST be separated into private apply, clean, discovery, paths, and symlink modules.

#### Scenario: Existing callers remain compatible

- GIVEN callers in `src/lib.rs`, `src/main.rs`, `src/commands/status.rs`, and `src/commands/doctor.rs`
- WHEN the linker is split into modules
- THEN they MUST compile and behave without import or signature changes

#### Scenario: Focused tests remain isolatable

- GIVEN path tests in `tests/test_security.rs` and `tests/unit/linker_security.rs`, and linker
  symlink tests in `src/linker.rs`
- WHEN responsibilities are moved
- THEN equivalent path and symlink tests MUST remain independently runnable without contract changes

### Requirement: Apply and Clean Preserve Behavior

`sync` and `clean` MUST preserve agent selection precedence, deterministic `BTreeMap` ordering, all
four sync types (`symlink`, `symlink-contents`, `nested-glob`, `module-map`), counters, per-target
error continuation/logging, output, dry-run behavior, and sync-start cache reset. `clean` MUST still
process all configured targets regardless of apply filters and apply MUST retain clean-first,
compression, gitignore, and MCP coordination.

#### Scenario: Filtered and repeated apply stays equivalent

- GIVEN enabled/disabled agents, `default_agents`, CLI filters, and one reused `Linker`
- WHEN `sync` runs before and after modular extraction
- THEN selection, output, counters, errors, and path/compression/glob/ensured/canonical-root cache reset MUST match

#### Scenario: Every configured sync type remains dispatchable

- GIVEN targets of each sync type and optional dry-run or `--clean`
- WHEN apply and clean run
- THEN filesystem state, cleanup, counters, errors, and output MUST match the existing contract

**References:** `src/linker.rs::{sync, process_target, clean}`; `src/main.rs::{handle_apply, handle_clean}`.

### Requirement: Path Safety and Symlink Mutation Are Unchanged

Paths MUST retain absolute, empty, traversal, source-escape, project-containment, ancestor
canonicalization, relative-target, safe-unlink, and immediate TOCTOU revalidation semantics.
Symlink operations MUST retain creation/update/skip, broken-link handling, backups and stale-backup
replacement, cleanup-only-of-links, circular-destination guards, dry-run accounting/output, and
Unix/Windows creation/removal behavior.

#### Scenario: Unsafe paths still fail before mutation

- GIVEN an unsafe destination or an ancestor swapped outside the project after initial validation
- WHEN validation or revalidation runs
- THEN the operation MUST fail before filesystem mutation

#### Scenario: Existing destinations retain their outcomes

- GIVEN a correct, wrong-target, broken symlink, regular file, or directory destination
- WHEN apply or clean processes it
- THEN the same filesystem result, backup/removal behavior, counters, and output MUST occur

**References:** `src/linker.rs::{ensure_safe_destination, revalidate_path, revalidate_unlink_path, relative_path, create_symlink, remove_symlink}`; `tests/test_security.rs`; `tests/unit/linker_security.rs`.

### Requirement: Discovery Is Deterministic and Scope-Limited

Discovery MUST preserve glob/nested-glob matching, excludes, destination templates, non-following
walks, module-map filename conventions, deterministic ordering, and apply/clean coordination. The
change MUST NOT introduce functional, CLI, performance-algorithm, concurrency, async, or new-cache
changes; it is a mechanical extraction of existing behavior.

#### Scenario: Discovery and mapping produce the same result

- GIVEN unchanged nested-glob or module-map configuration and source paths
- WHEN apply and clean run after extraction
- THEN destinations, link set, ordering, filenames, counters, and cleanup MUST be unchanged

#### Scenario: Execution model remains synchronous

- GIVEN the existing configuration and filesystem
- WHEN the modular engine runs
- THEN it MUST use the existing synchronous algorithms and caches with no parallel or asynchronous work

**References:** `src/linker.rs::{get_nested_glob_matches, process_nested_glob, expand_destination_template}`; `src/config.rs::resolve_module_map_filename`; `tests/test_module_map_cli.rs`.
