# Exploration: nested-agent-context

**Change**: nested-agent-context  
**Date**: 2026-03-22  
**Status**: COMPLETE  

## Summary

Add a new `module-map` sync type to AgentSync that allows users to maintain module-specific agent instruction files inside `.agents/` and map them to specific module directories. This enables per-module agent context (e.g., a CLAUDE.md for `src/api/`, another for `src/ui/`) managed centrally and synced via symlinks.

## Exploration Goals & Findings

### 1. SyncType Enum Extension Point

**Location**: `src/config.rs:124-137`

The `SyncType` enum uses `#[serde(rename_all = "kebab-case")]` so adding `ModuleMap` will automatically parse as `"module-map"` from TOML. Current variants: `Symlink`, `SymlinkContents`, `NestedGlob`. Adding a new variant is trivial — no breaking changes.

### 2. TargetConfig Struct Extensibility

**Location**: `src/config.rs:86-121`

`TargetConfig` has flat fields: `source`, `destination`, `sync_type`, `pattern`, `exclude`. For `ModuleMap`, a new field `mappings: Vec<ModuleMapping>` with `#[serde(default)]` fits cleanly. The TOML syntax would be:

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
```

A new `ModuleMapping` struct is needed:
```rust
#[derive(Debug, Deserialize, Clone)]
pub struct ModuleMapping {
    pub source: String,
    pub destination: String,
    pub filename_override: Option<String>,
}
```

### 3. Linker Dispatch Pattern

**Location**: `src/linker.rs:191-219`

`process_target()` uses a `match` on `target.sync_type` dispatching to type-specific methods. Adding `SyncType::ModuleMap => self.process_module_map(agent, target, agent_source_dir)` follows the established pattern exactly. Source resolution should use `source_dir` (like Symlink/SymlinkContents) since sources live in `.agents/`.

### 4. Agent Convention Filenames

**Location**: `src/agent_ids.rs`

Currently has `canonical_mcp_agent_id()` and `canonical_configurable_agent_id()` for alias resolution, but there is **no function mapping agent_name → convention filename** (CLAUDE.md, AGENTS.md, GEMINI.md, `.github/copilot-instructions.md`, etc.). This function must be created. Convention table:

| Agent     | Default Filename              |
|-----------|-------------------------------|
| claude    | CLAUDE.md                     |
| copilot   | .github/copilot-instructions.md |
| codex     | AGENTS.md                     |
| gemini    | GEMINI.md                     |
| cursor    | .cursor/rules/agentsync.mdc   |
| windsurf  | .windsurfrules                |
| opencode  | OPENCODE.md                   |

### 5. .gitignore Integration

**Location**: `src/config.rs:306-332`, `src/gitignore.rs`

`all_gitignore_entries()` collects destinations from all enabled agents' targets, skipping `NestedGlob` templates. ModuleMap would need to **expand mappings into individual destination entries**. Each mapping destination + resolved filename should be added to gitignore.

### 6. Doctor Checks

**Location**: `src/commands/doctor.rs`

Doctor validates: config loading, source dir existence, target source existence, destination conflicts (duplicates + overlaps), MCP commands, .gitignore audit. ModuleMap needs:
- Source existence check for **each mapping** (not just the target source)
- Destination conflict detection across expanded mapping outputs
- Validation that `filename_override` doesn't conflict with existing targets

### 7. Status Output

**Location**: `src/commands/status.rs`

Iterates `agent.targets`, checks dest existence/symlink-correctness using `linker.expected_source_path()`. ModuleMap must expand mappings into individual `StatusEntry` items, each showing source→destination correctly.

### 8. Dry-run Mode

**Location**: `src/linker.rs` (throughout)

`SyncOptions.dry_run` bool gates filesystem mutations, printing "Would..." messages. ModuleMap follows the same pattern — each mapping's symlink creation would check `dry_run` before acting. No special handling needed beyond what `create_symlink()` already does.

### 9. Clean Command

**Location**: `src/linker.rs:709-822`

Clean dispatches by `sync_type` in a match block. ModuleMap clean would iterate mappings and remove each destination symlink. Pattern mirrors `SymlinkContents` clean (multiple symlinks per target).

### 10. Init Wizard

**Location**: `src/init.rs:540-916`

`scan_agent_files()` scans for ~30+ agent file types. `init_wizard()` offers migration with file selection via `dialoguer`. ModuleMap integration would optionally scan for existing module-level instruction files (nested CLAUDE.md, AGENTS.md in subdirectories) and offer to centralize them into `.agents/` with mappings.

### 11. TOML Config Parsing

Uses serde `Deserialize` derive throughout. Adding `mappings: Vec<ModuleMapping>` with `#[serde(default)]` to `TargetConfig` works cleanly. Existing targets without mappings are unaffected (defaults to an empty vec).

### 12. Test Patterns

Extensive unit tests using `tempfile::TempDir`. Pattern: create config TOML → load → create Linker → call operation → assert filesystem state. ModuleMap tests would follow the same pattern, creating source files in `.agents/` temp dir, writing TOML with mappings, syncing, then asserting symlinks at each mapping destination.

## Key Design Decisions Identified

1. **ModuleMapping on TargetConfig vs. new top-level section**: TargetConfig is the right place — consistent with how all sync types are configured today.
2. **Convention filename resolution**: Needs a new function in `agent_ids.rs`. Must be codified rather than left implicit in TOML.
3. **Source path base**: Use `source_dir` (`.agents/<agent>/`) not `project_root`, matching Symlink/SymlinkContents pattern.
4. **Filename resolution order**: `filename_override` > convention-based from agent name > fallback to source filename.

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| TargetConfig struct grows complex with optional fields | Medium | Document which fields apply to which sync_type; consider validation in doctor |
| Convention filename mapping may be incomplete or wrong | Low | Start with known agents, make it extensible, validate in doctor |
| Nested destination paths (e.g., `.github/copilot-instructions.md`) add complexity | Medium | Ensure `create_symlink()` creates intermediate directories, test edge cases |
| TOML array-of-tables syntax may confuse users | Low | Provide clear examples in docs and `init --wizard` |

## Next Recommended

Proceed to **PROPOSE** phase to formalize the change scope, approach, and rollback plan.
