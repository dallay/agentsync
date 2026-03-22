# Proposal: Module-Map Sync Type

## Intent

Users need to provide **per-module agent context** — different instruction files for different parts of their codebase (e.g., API modules get backend-focused instructions, UI modules get frontend-focused instructions). Today, AgentSync only supports project-root-level syncing (symlink, symlink-contents, nested-glob). There is no way to map centrally-managed `.agents/` files to arbitrary subdirectories with convention-based filenames.

This change adds a `module-map` sync type that lets users define source→destination mappings, with automatic filename resolution per agent convention (CLAUDE.md, AGENTS.md, GEMINI.md, etc.).

## Scope

### In Scope
- New `SyncType::ModuleMap` enum variant in `config.rs`
- New `ModuleMapping` struct (source, destination, filename_override)
- `mappings: Vec<ModuleMapping>` field on `TargetConfig` with `#[serde(default)]`
- New `process_module_map()` method in `linker.rs`
- New `agent_convention_filename()` function in `agent_ids.rs`
- `.gitignore` integration: expand mappings into individual entries
- Doctor: per-mapping source validation + destination conflict checks
- Status: expand mappings into individual status entries
- Clean: remove per-mapping symlinks
- Dry-run: standard "Would..." output for each mapping
- Unit tests for all new logic following existing `tempfile::TempDir` patterns

### Out of Scope
- Init wizard auto-detection of existing module-level files (deferred — useful but not MVP)
- Glob-based mapping patterns (e.g., `source = "modules/*.md"`) — future enhancement
- Bidirectional sync or file copying (AgentSync is symlink-only)
- Documentation updates beyond code comments (separate change)

## Approach

**Follow every existing pattern** — this change mirrors how `NestedGlob` was added:

1. **Config layer** (`config.rs`):
   - Add `ModuleMap` to `SyncType` enum
   - Add `ModuleMapping` struct with serde `Deserialize`
   - Add `mappings` field to `TargetConfig` with `#[serde(default)]`
   - Extend `all_gitignore_entries()` to expand module-map mappings

2. **Agent conventions** (`agent_ids.rs`):
   - Add `agent_convention_filename(agent_name: &str) -> Option<&'static str>`
   - Map known agents to their convention filenames (CLAUDE.md, AGENTS.md, etc.)
   - Keep unknown-agent fallback in `resolve_module_map_filename()` via the source basename

3. **Linker** (`linker.rs`):
   - Add `SyncType::ModuleMap` arm to `process_target()` dispatch
   - Implement `process_module_map()`: iterate mappings, resolve filename (override > convention > source basename), create symlink for each
   - Add `SyncType::ModuleMap` arm to `clean()` dispatch
   - Update `expected_source_path()` if needed for status checks

4. **Commands**:
   - `doctor.rs`: validate each mapping source exists, check destination conflicts across expanded mappings
   - `status.rs`: expand mappings into individual `StatusEntry` items

5. **Tests**: unit tests in each modified module following existing patterns

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/config.rs` | Modified | New SyncType variant, ModuleMapping struct, mappings field on TargetConfig, gitignore expansion |
| `src/agent_ids.rs` | Modified | New `agent_convention_filename()` function |
| `src/linker.rs` | Modified | New `process_module_map()`, clean arm, dispatch arm |
| `src/commands/doctor.rs` | Modified | Per-mapping source validation, destination conflict checks |
| `src/commands/status.rs` | Modified | Expand mappings to status entries |
| `src/init.rs` | Not changed | Init wizard deferred to future change |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| TargetConfig struct becomes a grab-bag of optional fields | Medium | Doctor validates that `mappings` is only set when `sync_type = "module-map"`; doc comments clarify field applicability |
| Convention filename table is incomplete | Low | Start with known agents, keep unknown-agent basename fallback in filename resolution |
| Nested destination paths (`.github/copilot-instructions.md`) need intermediate dirs | Medium | `create_symlink()` already handles parent dir creation; add explicit test |
| TOML array-of-tables syntax confuses users | Low | Clear error messages from serde; future `init --wizard` can generate config |

## Rollback Plan

1. **Code**: Revert the commits adding ModuleMap support. No database, no migrations, no external state.
2. **Config**: Users remove `sync_type = "module-map"` targets from their `agentsync.toml`. Running `agentsync clean` before rollback removes any created symlinks.
3. **Git**: Any `.gitignore` entries added by module-map targets are removed automatically when the config no longer contains them and `agentsync apply` runs.

Risk: **Very Low**. This is purely additive — no existing behavior is modified. The new enum variant and struct are only activated when users explicitly configure `module-map` targets.

## Dependencies

- None. All changes are internal to the agentsync codebase with no new external dependencies.

## Success Criteria

- [ ] `SyncType::ModuleMap` parses correctly from TOML config
- [ ] `process_module_map()` creates symlinks at each mapping destination with correct convention filename
- [ ] `filename_override` takes precedence over convention filename
- [ ] `agentsync clean` removes all module-map symlinks
- [ ] `agentsync status` shows individual entries per mapping
- [ ] `agentsync doctor` validates mapping sources exist and detects destination conflicts
- [ ] `agentsync apply --dry-run` prints "Would..." for each mapping without filesystem changes
- [ ] `.gitignore` is updated with all expanded mapping destinations
- [ ] All existing tests continue to pass (no regressions)
- [ ] New unit tests cover: sync, clean, status, doctor, gitignore for module-map targets
