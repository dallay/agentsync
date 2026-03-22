# Design: Module-Map Sync Type

## Technical Approach

Add a `module-map` sync type that creates per-mapping symlinks from centrally-managed `.agents/` source files to specific module directories, with automatic filename resolution based on agent convention (CLAUDE.md, AGENTS.md, etc.). The implementation follows the exact same dispatch pattern used by `Symlink`, `SymlinkContents`, and `NestedGlob` — no new abstractions, no structural refactoring, just a new variant wired through every existing code path.

This maps directly to the proposal's approach: config layer first, then agent conventions, linker dispatch, commands, and tests.

## Architecture Decisions

### Decision: ModuleMapping as a flat struct on TargetConfig

**Choice**: Add `mappings: Vec<ModuleMapping>` to `TargetConfig` with `#[serde(default)]`.

**Alternatives considered**:
- Separate top-level `[module_maps]` config section — rejected because it breaks the `agent → targets` hierarchy that all sync types follow.
- Inline mappings as a HashMap — rejected because ordering matters for deterministic gitignore output.

**Rationale**: Every existing sync type is configured via `TargetConfig`. Users already understand this pattern. Adding an optional field with serde default keeps backward compatibility — existing configs parse identically. The TOML `[[agents.claude.targets.modules.mappings]]` array-of-tables syntax is slightly verbose but explicit and standard.

### Decision: Convention filename function in agent_ids.rs

**Choice**: Add `agent_convention_filename(agent_name: &str) -> Option<&'static str>` to `agent_ids.rs`, returning convention filenames for known agents and `None` otherwise.

**Alternatives considered**:
- Hardcode filename logic inside `process_module_map()` — rejected because it duplicates agent identity knowledge that already lives in `agent_ids.rs`.
- Use the agent config's existing target destinations to infer filenames — rejected because module-map is a new target and there's no guarantee other targets exist or use the convention filename.

**Rationale**: `agent_ids.rs` is the single source of truth for agent identity (aliases, ignore patterns, filter matching). Convention filenames are agent identity metadata. This function reuses the same `canonical_mcp_agent_id` / `canonical_configurable_agent_id` normalization before the lookup, so aliases work automatically.

### Decision: Filename resolution order: override > convention > source basename

**Choice**: Three-tier resolution:
1. `mapping.filename_override` — explicit user choice
2. `agent_convention_filename(agent_name)` when it returns `Some(...)`
3. Source file's basename — fallback for unknown agents

**Alternatives considered**:
- Only support explicit filenames — rejected because it defeats the purpose of convention-based mapping.
- Only convention filenames — rejected because users may need non-standard names.

**Rationale**: The override gives full control. The convention gives zero-config ergonomics for known agents. Returning `None` for unknown agents keeps `agent_ids.rs` focused on known agent metadata while `resolve_module_map_filename()` applies the basename fallback for custom agents.

### Decision: Source resolution uses source_dir (not project_root)

**Choice**: Mapping `source` paths resolve relative to `source_dir` (typically `.agents/<agent>/`), matching `Symlink` and `SymlinkContents` behavior.

**Alternatives considered**:
- Resolve relative to project root (like `NestedGlob`) — rejected because module-map sources are centrally managed instruction files, not discovered project files.

**Rationale**: Consistency with existing sync types that have explicit source files. Users expect `source = "api-context.md"` to resolve from the same base as `source = "AGENTS.md"` in a `symlink` target.

### Decision: Expand mappings into individual gitignore entries

**Choice**: Each mapping generates its own gitignore entry: `<destination>/<resolved_filename>`.

**Alternatives considered**:
- Skip gitignore for module-map entirely (like `NestedGlob` skips templates) — rejected because module-map destinations are known at config time, unlike NestedGlob's dynamic discovery.
- Add directory-level entries — rejected because it would be too broad and could mask user files.

**Rationale**: Module-map destinations are statically known from config. Each resolved symlink path should be in gitignore to prevent accidental commits. The `.bak.*` pattern is also added per mapping, consistent with `Symlink` behavior.

## Data Flow

```
agentsync.toml                        Filesystem
─────────────────                     ──────────────────────────
                                      
[agents.claude.targets.modules]       .agents/claude/
  type = "module-map"                   ├── api-context.md
  [[mappings]]                          └── ui-context.md
    source = "api-context.md"          
    destination = "src/api"            src/api/
  [[mappings]]                           └── CLAUDE.md → ../../.agents/claude/api-context.md
    source = "ui-context.md"           
    destination = "src/ui"             src/ui/
                                         └── CLAUDE.md → ../../.agents/claude/ui-context.md


Filename Resolution Flow:

  mapping.filename_override ──→ Use override
           │ (None)
           ▼
  agent_convention_filename(agent_name)
           │
       ┌───┴───┐
    Known    Unknown
       │        │
       ▼        ▼
  "CLAUDE.md"  source.file_name()
  "AGENTS.md"  (basename fallback)
  etc.

Sync Flow:

  sync() → process_target() → match SyncType::ModuleMap
                                  │
                                  ▼
                          process_module_map(agent_name, target, source_dir)
                                  │
                                  ▼
                          for each mapping in target.mappings:
                            1. Resolve source: source_dir / mapping.source
                            2. Resolve filename: override > convention > basename
                            3. Resolve dest: project_root / mapping.destination / filename
                            4. create_symlink(source, dest, options)
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/config.rs` | Modify | Add `ModuleMap` to `SyncType`, add `ModuleMapping` struct, add `mappings` field to `TargetConfig`, update `all_gitignore_entries()` to expand module-map mappings |
| `src/agent_ids.rs` | Modify | Add `agent_convention_filename()` function mapping agent names to their convention instruction filenames |
| `src/linker.rs` | Modify | Add `SyncType::ModuleMap` arm to `process_target()`, implement `process_module_map()`, add `SyncType::ModuleMap` arm to `clean()` |
| `src/commands/doctor.rs` | Modify | Add per-mapping source existence validation, expand mappings for destination conflict detection |
| `src/commands/status.rs` | Modify | Expand module-map targets into individual `StatusEntry` items per mapping |
| `src/gitignore.rs` | No change | Gitignore module is already generic — it just writes whatever entries `all_gitignore_entries()` returns |

## Interfaces / Contracts

### New struct: `ModuleMapping` (config.rs)

```rust
/// A single source-to-destination mapping within a `module-map` target.
/// Maps a centrally-managed source file to a specific module directory
/// with an optional filename override.
#[derive(Debug, Deserialize, Clone)]
pub struct ModuleMapping {
    /// Source file path, relative to `source_dir` (e.g., "api-context.md").
    pub source: String,

    /// Destination directory, relative to project root (e.g., "src/api").
    pub destination: String,

    /// Override the output filename. If `None`, uses the agent's convention
    /// filename (e.g., CLAUDE.md for claude) or falls back to the source
    /// file's basename.
    #[serde(default)]
    pub filename_override: Option<String>,
}
```

### Extended enum: `SyncType` (config.rs)

```rust
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SyncType {
    Symlink,
    SymlinkContents,
    NestedGlob,
    /// Maps centrally-managed source files to specific module directories,
    /// creating a symlink per mapping with convention-based filenames.
    ModuleMap,
}
```

### Extended struct: `TargetConfig` (config.rs)

```rust
pub struct TargetConfig {
    pub source: String,
    pub destination: String,
    #[serde(rename = "type")]
    pub sync_type: SyncType,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Mappings for `module-map` targets. Each mapping defines a
    /// source file and destination directory pair. Ignored for other
    /// sync types.
    #[serde(default)]
    pub mappings: Vec<ModuleMapping>,
}
```

Note: Using `Vec<ModuleMapping>` with `#[serde(default)]` instead of `Option<Vec<ModuleMapping>>` — this is simpler (empty vec = no mappings), avoids `Option` unwrapping everywhere, and `#[serde(default)]` handles missing field by producing an empty vec. This matches the pattern used by `exclude: Vec<String>` on the same struct.

### New function: `agent_convention_filename` (agent_ids.rs)

```rust
/// Return the convention instruction filename for a known agent.
///
/// Uses canonical ID resolution so aliases (e.g., "claude-code", "codex-cli")
/// map to the same filename as their canonical form.
///
/// Returns `None` for unknown agents — callers should fall back to the
/// source file's basename.
pub fn agent_convention_filename(agent_name: &str) -> Option<&'static str> {
    // Normalize via canonical_mcp_agent_id first, then configurable
    let canonical = canonical_mcp_agent_id(agent_name)
        .or_else(|| canonical_configurable_agent_id(agent_name))
        .unwrap_or(agent_name);

    match canonical {
        "claude"    => Some("CLAUDE.md"),
        "copilot"   => Some(".github/copilot-instructions.md"),
        "codex"     => Some("AGENTS.md"),
        "gemini"    => Some("GEMINI.md"),
        "cursor"    => Some(".cursor/rules/agentsync.mdc"),
        "windsurf"  => Some(".windsurfrules"),
        "opencode"  => Some("OPENCODE.md"),
        "crush"     => Some("CRUSH.md"),
        "warp"      => Some("WARP.md"),
        "amp"       => Some("AMPCODE.md"),
        _ => None,
    }
}
```

### New method: `process_module_map` (linker.rs)

```rust
/// Process a `module-map` target: iterate mappings and create a symlink
/// for each one, resolving the destination filename from:
/// 1. mapping.filename_override (explicit user choice)
/// 2. agent_convention_filename (per-agent convention)
/// 3. source file basename (fallback)
fn process_module_map(
    &self,
    agent_name: &str,
    target: &TargetConfig,
    options: &SyncOptions,
) -> Result<SyncResult> {
    let mut result = SyncResult::default();

    if target.mappings.is_empty() {
        if options.verbose {
            println!("  {} No mappings defined for module-map target", "!".yellow());
        }
        return Ok(result);
    }

    for mapping in &target.mappings {
        let source_path = self.source_dir.join(&mapping.source);

        // Resolve destination filename
        let filename = if let Some(ref override_name) = mapping.filename_override {
            override_name.clone()
        } else if let Some(convention) = crate::agent_ids::agent_convention_filename(agent_name) {
            convention.to_string()
        } else {
            // Fallback: use the source file's basename
            Path::new(&mapping.source)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| mapping.source.clone())
        };

        let dest = self.project_root
            .join(&mapping.destination)
            .join(&filename);

        let resolved = ResolvedSource {
            path: source_path,
            exists: self.source_dir.join(&mapping.source).exists(),
        };

        let item_result = self.create_symlink(&resolved, &dest, options)?;
        result.created += item_result.created;
        result.updated += item_result.updated;
        result.skipped += item_result.skipped;
    }

    Ok(result)
}
```

### Updated dispatch in `process_target` (linker.rs)

```rust
fn process_target(
    &self,
    agent_name: &str,  // NEW parameter — threaded from sync() loop
    target: &TargetConfig,
    options: &SyncOptions,
) -> Result<SyncResult> {
    let source = self.source_dir.join(&target.source);
    let dest = self.project_root.join(&target.destination);

    match target.sync_type {
        SyncType::Symlink => { /* existing */ }
        SyncType::SymlinkContents => { /* existing */ }
        SyncType::NestedGlob => { /* existing */ }
        SyncType::ModuleMap => self.process_module_map(agent_name, target, options),
    }
}
```

**Important**: `process_target` currently does NOT receive `agent_name`. The `sync()` method iterates `(agent_name, agent_config)` and then iterates targets, but only passes `target_config` down. We need to thread `agent_name` through to `process_target` so `process_module_map` can resolve the convention filename.

This is a minimal signature change: add `agent_name: &str` as the first parameter of `process_target`. The only caller is the loop inside `sync()`, which already has `agent_name` in scope.

### Updated clean dispatch (linker.rs)

```rust
// Inside clean() match block:
SyncType::ModuleMap => {
    for mapping in &target_config.mappings {
        // Resolve filename same way as process_module_map
        let filename = if let Some(ref override_name) = mapping.filename_override {
            override_name.clone()
        } else if let Some(convention) =
            crate::agent_ids::agent_convention_filename(agent_name)
        {
            convention.to_string()
        } else {
            Path::new(&mapping.source)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| mapping.source.clone())
        };

        let dest = self.project_root
            .join(&mapping.destination)
            .join(&filename);

        if dest.is_symlink() {
            if options.dry_run {
                println!("  {} Would remove: {}", "→".cyan(), dest.display());
            } else {
                fs::remove_file(&dest)?;
                println!("  {} Removed: {}", "✔".green(), dest.display());
            }
            result.removed += 1;
        }
    }
}
```

**Important**: `clean()` also doesn't currently receive `agent_name` per-target. The loop iterates `agent_config.targets` but discards the agent name. Similar to `process_target`, we need to adjust the `clean()` method's inner loop to have access to `agent_name`. The simplest approach: iterate `self.config.agents` and destructure `(agent_name, agent_config)`, which the clean loop already does at the outer level.

### Updated `all_gitignore_entries` (config.rs)

```rust
// Inside the target iteration in all_gitignore_entries():
if target.sync_type == SyncType::NestedGlob {
    continue;
}
if target.sync_type == SyncType::ModuleMap {
    // Expand each mapping into its own gitignore entry
    for mapping in &target.mappings {
        let filename = if let Some(ref override_name) = mapping.filename_override {
            override_name.clone()
        } else if let Some(convention) =
            crate::agent_ids::agent_convention_filename(agent_name)
        {
            convention.to_string()
        } else {
            std::path::Path::new(&mapping.source)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| mapping.source.clone())
        };
        let entry = format!("{}/{}", mapping.destination, filename);
        entries.insert(entry.clone());
        entries.insert(format!("{}.bak.*", entry));
    }
    continue;
}
// existing: entries.insert(target.destination.clone()); ...
```

### Updated doctor (commands/doctor.rs)

```rust
// In section 3 (Target Source Existence Check):
// After the existing check, add module-map expansion:
if target.sync_type == agentsync::config::SyncType::ModuleMap {
    for mapping in &target.mappings {
        let mapping_source = source_dir.join(&mapping.source);
        if !mapping_source.exists() {
            println!(
                "  {} Missing mapping source for agent {} (target {}, mapping {}): {}",
                "✗".red(),
                agent_name.bold(),
                target_name.dimmed(),
                mapping.source.dimmed(),
                mapping_source.display()
            );
            issues += 1;
            missing_targets += 1;
        }
    }
}

// In section 4 (Destination Path Conflict Check):
// Expand module-map mappings into individual destinations:
if target.sync_type == agentsync::config::SyncType::ModuleMap {
    for mapping in &target.mappings {
        let filename = /* same resolution logic */;
        let dest_path = format!("{}/{}", mapping.destination, filename);
        let normalized = normalize_path(&dest_path);
        destinations.push((normalized, agent_name.clone(), target_name.clone()));
    }
} else {
    // existing behavior
    let normalized = normalize_path(&target.destination);
    destinations.push((normalized, agent_name.clone(), target_name.clone()));
}
```

### Updated status (commands/status.rs)

```rust
// Inside the target iteration:
if target.sync_type == agentsync::config::SyncType::ModuleMap {
    for mapping in &target.mappings {
        let filename = /* same resolution logic */;
        let dest = linker.project_root()
            .join(&mapping.destination)
            .join(&filename);
        let source = linker.config()
            .source_dir(&config_path)
            .join(&mapping.source);

        // Build StatusEntry same as existing code...
        entries.push(StatusEntry { /* ... */ });
    }
} else {
    // existing behavior for Symlink/SymlinkContents/NestedGlob
}
```

## Filename Resolution Helper

The filename resolution logic (override > convention > basename) appears in 4 places: `process_module_map`, `clean`, `all_gitignore_entries`, and the commands. To avoid duplication, extract a free function:

```rust
// In config.rs or a shared location:
/// Resolve the output filename for a module-map mapping.
///
/// Priority: filename_override > agent convention > source basename.
pub fn resolve_module_map_filename(
    mapping: &ModuleMapping,
    agent_name: &str,
) -> String {
    if let Some(ref override_name) = mapping.filename_override {
        return override_name.clone();
    }
    if let Some(convention) = crate::agent_ids::agent_convention_filename(agent_name) {
        return convention.to_string();
    }
    std::path::Path::new(&mapping.source)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| mapping.source.clone())
}
```

This function lives on `ModuleMapping` as an associated method or as a free function in `config.rs`. I recommend the free function approach since it needs `agent_name` which isn't part of `ModuleMapping`.

## Threading `agent_name` Through Existing APIs

### Impact Analysis

Currently:
- `sync()` has `agent_name` in its loop but calls `process_target(target_config, options)` without it.
- `clean()` iterates `agent_config.targets.values()` but doesn't pass agent name to per-target logic.

Required changes:
1. **`process_target` signature**: Add `agent_name: &str` as first parameter. Only one call site (`sync()`), trivial change.
2. **`clean()` inner loop**: Already iterates `(agent_name, agent_config)` at the outer level — just needs to reference `agent_name` inside the target match. No signature change needed since `clean()` already has access.

For `all_gitignore_entries()`: Already iterates `(agent_name, agent)` — has access.

For `doctor.rs` and `status.rs`: Already iterate `(agent_name, agent)` — have access.

**No public API breakage.** `process_target` is a private method.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `ModuleMapping` deserialization from TOML | Parse TOML strings with `toml::from_str`, assert struct fields |
| Unit | `SyncType::ModuleMap` serde parsing | Parse `type = "module-map"`, assert `SyncType::ModuleMap` |
| Unit | `agent_convention_filename()` for all known agents + aliases + unknown | Direct function calls, assert expected filenames |
| Unit | `resolve_module_map_filename()` priority chain | Test override > convention > basename with various inputs |
| Unit | `all_gitignore_entries()` with module-map targets | Parse config with module-map targets, assert expanded entries appear |
| Unit | `all_gitignore_entries()` skips module-map targets for disabled agents | Same as above with `enabled = false` |
| Integration | `process_module_map()` creates symlinks | TempDir + source files + TOML config, call `sync()`, assert symlinks at expected paths |
| Integration | Convention filename resolution (claude → CLAUDE.md) | Same TempDir pattern, assert filename is CLAUDE.md |
| Integration | `filename_override` takes precedence | Same pattern with override, assert override filename used |
| Integration | Unknown agent falls back to source basename | Agent named "custom-agent", assert source basename used |
| Integration | Nested destination paths (`.github/copilot-instructions.md`) | Copilot agent, assert intermediate dirs created |
| Integration | `clean()` removes module-map symlinks | Sync then clean, assert symlinks removed |
| Integration | `clean()` dry-run for module-map | Sync, clean with dry_run, assert symlinks still exist |
| Integration | `sync()` dry-run for module-map | Dry-run sync, assert no symlinks created |
| Integration | Missing mapping source is skipped | Source file doesn't exist, assert skipped count |
| Integration | Empty mappings produces no errors | Module-map target with no mappings, assert no crash |
| Unit | Doctor validates per-mapping source existence | Create config with missing mapping source, verify doctor reports issue |
| Unit | Doctor detects destination conflicts across mappings | Two mappings to same destination, verify conflict reported |
| Unit | Status expands mappings into entries | Parse config, verify StatusEntry count matches mapping count |

All integration tests follow the existing `tempfile::TempDir` pattern used throughout `linker.rs` tests.

## Migration / Rollout

No migration required. This is a purely additive change:

- New `SyncType::ModuleMap` variant is only activated when users explicitly write `type = "module-map"` in their config.
- The `mappings` field defaults to an empty vec — existing configs are unaffected.
- No database, no external state, no feature flags needed.
- Rollback: revert commits, users remove `module-map` targets from their config, run `agentsync clean` first if they want to remove symlinks.

## Open Questions

- [x] Should `TargetConfig.source` and `TargetConfig.destination` be required for `module-map` targets? **Decision**: Keep them required by serde for backward compatibility, but treat them as ignored placeholders for `module-map`. Mapping sources still resolve from `source_dir`, and mapping destinations remain the only output paths that matter.
- [ ] Should `compress_agents_md` apply to module-map sources that are named `AGENTS.md`? **Recommendation**: No — module-map sources have user-chosen names (like `api-context.md`). Compression is specifically for the canonical `AGENTS.md` file. If a mapping source happens to be named `AGENTS.md`, the user is doing something unusual and compression would be surprising.
