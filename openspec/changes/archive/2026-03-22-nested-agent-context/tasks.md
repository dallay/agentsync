# Tasks: Module-Map Sync Type

## Phase 1: Foundation (Data Types & Agent Identity)

- [x] 1.1 Add `ModuleMap` variant to `SyncType` enum in `src/config.rs`
    - File: `src/config.rs` (line ~126)
    - Add `ModuleMap` variant with doc comment to the `SyncType` enum (after `NestedGlob`)
    - Serde `rename_all = "kebab-case"` will automatically produce `"module-map"`
    - Acceptance: `type = "module-map"` parses to `SyncType::ModuleMap`; `type = "invalid"` still
      fails; existing tests pass

- [x] 1.2 Add `ModuleMapping` struct in `src/config.rs`
    - File: `src/config.rs` (after `SyncType` enum, ~line 138)
    - Create `ModuleMapping` struct with `source: String`, `destination: String`,
      `filename_override: Option<String>` (with `#[serde(default)]` on filename_override)
    - Derive `Debug, Deserialize, Clone`
    - Acceptance: `ModuleMapping` can be deserialized from TOML; all three fields parse correctly

- [x] 1.3 Add `mappings` field to `TargetConfig` in `src/config.rs`
    - File: `src/config.rs` (line ~121, inside `TargetConfig`)
    - Add `pub mappings: Vec<ModuleMapping>` with `#[serde(default)]`
    - Follows the same pattern as `exclude: Vec<String>` on the same struct
    - Acceptance: Existing configs without `mappings` still parse (empty vec default); configs with
      `[[...mappings]]` array-of-tables parse correctly

- [x] 1.4 Add `agent_convention_filename()` function in `src/agent_ids.rs`
    - File: `src/agent_ids.rs` (after `canonical_configurable_agent_id`, ~line 60)
    - Make `canonical_configurable_agent_id` **pub** (currently private, needed by the new function)
    - Create `pub fn agent_convention_filename(agent_name: &str) -> Option<&'static str>` that
      normalizes via `canonical_mcp_agent_id` then `canonical_configurable_agent_id`, then matches
      canonical to convention filenames
    - Map: claude→CLAUDE.md, copilot→.github/copilot-instructions.md, codex→AGENTS.md,
      gemini→GEMINI.md, cursor→.cursor/rules/agentsync.mdc, windsurf→.windsurfrules,
      opencode→OPENCODE.md, crush→CRUSH.md, warp→WARP.md, amp→AMPCODE.md
    - Return `None` for unknown agents
    - Acceptance: `agent_convention_filename("claude")` returns `Some("CLAUDE.md")`;
      `agent_convention_filename("claude-code")` returns `Some("CLAUDE.md")` (alias);
      `agent_convention_filename("unknown-xyz")` returns `None`

- [x] 1.5 Add `resolve_module_map_filename()` helper in `src/config.rs`
    - File: `src/config.rs` (near `ModuleMapping` struct or as associated function)
    - Create
      `pub fn resolve_module_map_filename(mapping: &ModuleMapping, agent_name: &str) -> String`
    - Priority: `filename_override` > `agent_convention_filename(agent_name)` > source file basename
    - Acceptance: Override returns override; known agent without override returns convention;
      unknown agent without override returns source basename

- [x] 1.6 Write unit tests for Phase 1 types and functions
    - Files: `src/config.rs` (tests module), `src/agent_ids.rs` (tests module)
    - Tests to add:
        - `test_parse_module_map_sync_type`: parse `type = "module-map"` → `SyncType::ModuleMap`
        - `test_parse_module_mapping_struct`: parse TOML with source, destination, filename_override
        - `test_parse_target_config_with_mappings`: parse full TargetConfig with `[[...mappings]]`
        - `test_parse_target_config_without_mappings_defaults`: existing config without mappings →
          empty vec
        - `test_agent_convention_filename_known_agents`: all 10 known agents return correct
          filenames
        - `test_agent_convention_filename_aliases`: aliases resolve correctly (claude-code,
          codex-cli, etc.)
        - `test_agent_convention_filename_unknown`: returns None
        - `test_resolve_module_map_filename_override`: override takes precedence
        - `test_resolve_module_map_filename_convention`: known agent uses convention
        - `test_resolve_module_map_filename_fallback`: unknown agent uses source basename
    - Acceptance: All new tests pass; all existing tests still pass

## Phase 2: Core Sync & Clean (Linker)

- [x] 2.1 Update `process_target()` signature to accept `agent_name: &str` in `src/linker.rs`
    - File: `src/linker.rs` (line ~191)
    - Add `agent_name: &str` as first parameter to `process_target()`
    - Update the single call site in `sync()` (line ~173) to pass `agent_name`
    - Acceptance: Compiles; all existing sync tests pass unchanged

- [x] 2.2 Add `SyncType::ModuleMap` dispatch arm to `process_target()` in `src/linker.rs`
    - File: `src/linker.rs` (line ~195, inside the match block)
    - Add `SyncType::ModuleMap => self.process_module_map(agent_name, target, options)`
    - This will initially not compile until 2.3 is done — implement together with 2.3
    - Acceptance: Match is exhaustive; compiles with 2.3

- [x] 2.3 Implement `process_module_map()` method in `src/linker.rs`
    - File: `src/linker.rs` (after `process_nested_glob()`, ~line 658)
    - Signature:
      `fn process_module_map(&self, agent_name: &str, target: &TargetConfig, options: &SyncOptions) -> Result<SyncResult>`
    - Logic: iterate `target.mappings`, resolve source from `self.source_dir`, resolve filename via
      `resolve_module_map_filename`, build dest as `project_root / mapping.destination / filename`,
      create `ResolvedSource`, call `self.create_symlink()`
    - Handle empty mappings with verbose warning and return `Ok(SyncResult::default())`
    - Import `resolve_module_map_filename` from config (or use inline
      `crate::config::resolve_module_map_filename`)
    - Acceptance: Module-map targets create correct symlinks; empty mappings don't crash

- [x] 2.4 Add `SyncType::ModuleMap` arm to `clean()` in `src/linker.rs`
    - File: `src/linker.rs` (line ~716, inside clean's match block)
    - The clean loop iterates `self.config.agents.values()` — need `agent_name` in scope. Adjust
      outer loop from `.values()` to `.iter()` to destructure `(agent_name, agent_config)`
    - For each mapping: resolve filename same way (via `resolve_module_map_filename`), compute dest,
      check `is_symlink()`, remove or print dry-run message
    - Acceptance: `clean()` removes module-map symlinks; dry-run reports without removing; existing
      clean tests pass

- [x] 2.5 Write integration tests for sync and clean of module-map targets
    - File: `src/linker.rs` (tests module)
    - Tests to add:
        - `test_module_map_creates_symlinks`: TempDir, 2 source files, 2 mappings, assert 2 symlinks
          created with correct convention filenames
        - `test_module_map_convention_filename_claude`: claude agent → CLAUDE.md at each mapping
          dest
        - `test_module_map_filename_override`: mapping with `filename_override = "custom.md"` →
          custom.md used
        - `test_module_map_unknown_agent_uses_source_basename`: agent "custom-agent" → source
          basename used
        - `test_module_map_nested_destination_paths`: copilot agent →
          `.github/copilot-instructions.md` with intermediate dirs created
        - `test_module_map_clean_removes_symlinks`: sync then clean, assert removed
        - `test_module_map_clean_dry_run`: sync, clean with dry_run, assert symlinks still exist
        - `test_module_map_sync_dry_run`: dry_run sync, assert no symlinks created
        - `test_module_map_missing_source_skipped`: source doesn't exist → skipped count incremented
        - `test_module_map_empty_mappings`: no mappings → no crash, no error
    - Acceptance: All tests pass; no regressions

## Phase 3: Gitignore Integration

- [x] 3.1 Update `all_gitignore_entries()` in `src/config.rs` for module-map targets
    - File: `src/config.rs` (line ~317, inside `all_gitignore_entries()`)
    - After the existing `NestedGlob` skip, add a `ModuleMap` check:
        - If `target.sync_type == SyncType::ModuleMap`, iterate mappings, resolve filename via
          `resolve_module_map_filename`, insert `"{destination}/{filename}"` and
          `"{destination}/{filename}.bak.*"` entries, then `continue`
    - `agent_name` is already available in the iteration (line 311:
      `for (agent_name, agent) in &self.agents`)
    - Acceptance: Module-map mappings produce individual gitignore entries; NestedGlob and Symlink
      behavior unchanged

- [x] 3.2 Write unit tests for gitignore integration
    - File: `src/config.rs` (tests module)
    - Tests to add:
        - `test_all_gitignore_entries_module_map_expands_mappings`: config with module-map target,
          assert each mapping produces a gitignore entry
        - `test_all_gitignore_entries_module_map_with_backup_patterns`: assert `.bak.*` entries
          generated per mapping
        - `test_all_gitignore_entries_module_map_disabled_agent_skipped`: disabled agent's
          module-map targets excluded
        - `test_all_gitignore_entries_module_map_with_filename_override`: override filename appears
          in entry
    - Acceptance: All tests pass

## Phase 4: Commands (Doctor & Status)

- [x] 4.1 Update `doctor.rs` for per-mapping source validation
    - File: `src/commands/doctor.rs` (after line ~78, inside Section 3)
    - After existing target source check, if `target.sync_type == SyncType::ModuleMap`, iterate
      mappings and check each `mapping.source` exists in `source_dir`
    - Print specific error per missing mapping source
    - Acceptance: Doctor reports missing mapping sources individually

- [x] 4.2 Update `doctor.rs` for expanded destination conflict detection
    - File: `src/commands/doctor.rs` (line ~90, inside Section 4)
    - If `target.sync_type == SyncType::ModuleMap`, expand mappings into individual destination
      entries (resolve filename, build path `{destination}/{filename}`)
    - Push each expanded path into the destinations vec for conflict detection
    - Otherwise, use existing destination as-is
    - Acceptance: Doctor detects conflicts between module-map expanded destinations and other
      targets

- [x] 4.3 Update `status.rs` to expand module-map targets into individual entries
    - File: `src/commands/status.rs` (line ~39, inside the target iteration loop)
    - If `target.sync_type == SyncType::ModuleMap`, iterate mappings, resolve filename, build dest
      and source paths, create `StatusEntry` per mapping
    - Otherwise, use existing single-entry logic
    - Note: `agent_name` is not currently available in the status loop — need to change from
      `agent.targets.values()` to iterating `(agent_name, agent)` at the outer level (similar
      pattern to clean)
    - Acceptance: `agentsync status` shows individual entries per module-map mapping

- [x] 4.4 Write tests for doctor and status module-map support
    - File: `src/commands/doctor.rs` and `src/commands/status.rs` (or integration tests)
    - Tests to add:
        - Doctor: config with missing mapping source → reports issue
        - Doctor: two mappings to same resolved destination → reports conflict
        - Status: module-map target with 3 mappings → 3 StatusEntry items
    - Recovery update: Added helper-backed tests in `src/commands/doctor_tests.rs` and
      `src/commands/status_tests.rs` covering placeholder-source handling, expanded destination
      conflicts, per-mapping status entries, stale symlink detection, and JSON serialization
      behavior
    - Acceptance: ✅ `cargo test` passes with the new doctor/status coverage

## Phase 5: Verification & Cleanup

- [x] 5.1 Run full test suite and fix any regressions
    - Command: `cargo test`
    - Result: 319 tests passed (279 lib + 34 main + 2 integration + 1 bug + 3 update), 0 failed, 4
      ignored
    - Acceptance: ✅ `cargo test` exits 0; no warnings

- [x] 5.2 Run `cargo clippy` and fix any lints
    - Command: `cargo clippy -- -D warnings`
    - Result: Clean — no warnings or errors
    - Acceptance: ✅ No clippy warnings

- [x] 5.3 Manual smoke test with example config
    - Create a test `agentsync.toml` with module-map targets, run `agentsync apply`,
      `agentsync status`, `agentsync doctor`, `agentsync clean`
    - Verify correct behavior end-to-end
    - Recovery update: Ran a temp-project smoke flow using placeholder target-level `source`/
      `destination`; `apply` created two module-map symlinks, `status --json` emitted two expanded
      entries, `doctor` reported no false missing-source issue, and `clean` removed both symlinks
    - Acceptance: ✅ All commands work correctly with module-map targets in the recovery smoke pass

## Dependency Graph

```
Phase 1: Foundation
  1.1 (SyncType) ─┐
  1.2 (ModuleMapping) ─┤
  1.3 (mappings field) ─┤── All independent of each other, but all needed before Phase 2
  1.4 (convention fn) ──┤
  1.5 (resolve fn) ─────┤── Depends on 1.2 + 1.4
  1.6 (unit tests) ─────┘── Depends on 1.1-1.5

Phase 2: Core Sync & Clean
  2.1 (process_target sig) ──┐
  2.2 (dispatch arm) ────────┤── 2.2 + 2.3 must be done together
  2.3 (process_module_map) ──┤── Depends on 1.1-1.5
  2.4 (clean arm) ───────────┤── Independent of 2.2-2.3
  2.5 (integration tests) ───┘── Depends on 2.1-2.4

Phase 3: Gitignore
  3.1 (gitignore entries) ───┐── Depends on 1.5
  3.2 (gitignore tests) ─────┘── Depends on 3.1

Phase 4: Commands
  4.1 (doctor source) ───────┐
  4.2 (doctor conflicts) ────┤── Depends on 1.1-1.5
  4.3 (status expansion) ────┤── Depends on 1.1-1.5
  4.4 (command tests) ───────┘── Depends on 4.1-4.3

Phase 5: Verification
  5.1-5.3 ── Depends on all above
```

## Parallelization Notes

- Tasks 1.1-1.4 can be done in a single pass through config.rs and agent_ids.rs
- Tasks 1.5 depends on 1.2 and 1.4 (both must exist)
- Tasks 2.1-2.4 can be done in a single pass through linker.rs
- Tasks 3.1 and 4.1-4.3 are independent of each other (can be parallelized)
- Phase 5 is strictly sequential after everything else

## Total: 19 tasks across 5 phases
