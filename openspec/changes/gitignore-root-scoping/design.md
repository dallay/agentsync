# Design: Root-scope auto-generated .gitignore entries

## Technical Approach

Keep `.gitignore` rendering in `src/gitignore.rs` unchanged and fix scope earlier in `src/config.rs`, where managed entries are assembled for both sync and doctor. The implementation will introduce a small normalization helper that is applied only to auto-generated managed entries (target destinations, backup entries, built-in known patterns) before they are inserted into the final `BTreeSet`, while preserving verbatim user-authored `[gitignore].entries`.

## Architecture Decisions

### Decision: Normalize in `src/config.rs`

**Choice**: Add a private helper in `src/config.rs` and call it from `Config::all_gitignore_entries()` before returning auto-generated managed entries.
**Alternatives considered**: Normalize inside `src/gitignore.rs::update_gitignore()`; normalize separately in doctor.
**Rationale**: `all_gitignore_entries()` is already the shared source for sync rendering and doctor auditing. Normalizing once there avoids diverging behavior between update and audit flows.

### Decision: Scope only auto-generated concrete root-file entries

**Choice**: Prefix `/` only when a managed auto-generated entry is a concrete root-level file pattern that does not already contain `/`.
**Alternatives considered**: Prefix all managed entries; use an allowlist of specific filenames only; rewrite manual `[gitignore].entries` too.
**Rationale**: The spec requires preserving slash-containing entries and manual entries verbatim. Shape-based normalization matches the current generation model and naturally covers destinations, backups, and known root artifacts without broad policy changes.

### Decision: Leave nested-glob templates and slash-containing expansions untouched

**Choice**: Keep the existing NestedGlob skip behavior and do not rewrite ModuleMap or other generated entries that already contain `/`.
**Alternatives considered**: Normalize all generated paths to absolute-root style.
**Rationale**: Existing slash-containing paths intentionally express nested placement. Rewriting them would change semantics and violate spec scenarios for nested destinations and module-map expansions.

## Data Flow

Managed gitignore generation after the change:

```text
manual [gitignore].entries -------------------------------> BTreeSet (verbatim)
enabled targets -> destination/backup -> normalize_managed_gitignore_entry()
known agent patterns -----------------> normalize_managed_gitignore_entry()
module-map expansions (with /) -------> helper returns unchanged
nested-glob templates -----------------> skipped

final sorted Vec<String>
        ├──> src/main.rs -> gitignore::update_gitignore()
        └──> commands/doctor.rs -> managed section audit
```

Sequence for shared behavior:

```text
Config::all_gitignore_entries()
    -> collect manual entries unchanged
    -> generate managed entries
    -> normalize managed root-file entries once
    -> return sorted entries

sync command -------------------------> update_gitignore() writes normalized output
doctor command -----------------------> compares managed section against same normalized set
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/config.rs` | Modify | Add a private managed-entry normalization helper and route auto-generated destinations, backup entries, and known patterns through it inside `all_gitignore_entries()`. |
| `src/commands/doctor.rs` | No functional change expected | Continue consuming `all_gitignore_entries()` so audits inherit normalized expectations automatically. |
| `src/gitignore.rs` | No functional change expected | Rendering remains verbatim over already-normalized entries. |
| `src/config.rs` tests | Modify | Add unit coverage for normalized root files, unchanged manual entries, unchanged slash-containing entries, and backups/module-map cases. |
| `src/commands/doctor_tests.rs` | Modify | Add audit-oriented coverage showing normalized managed entries are accepted and legacy unscoped root entries are treated as drift. |

## Interfaces / Contracts

Planned private helper in `src/config.rs`:

```rust
fn normalize_managed_gitignore_entry(entry: &str) -> String
```

Expected contract:

- Transform:
  - `AGENTS.md` -> `/AGENTS.md`
  - `AGENTS.md.bak` -> `/AGENTS.md.bak`
  - `.mcp.json` -> `/.mcp.json`
  - `opencode.json` -> `/opencode.json`
  - `WARP.md` -> `/WARP.md`
- Leave untouched:
  - `docs/AGENTS.md`
  - `src/api/CLAUDE.md.bak`
  - `.claude/commands/`
  - `.agents/skills/*.bak`
  - user `gitignore.entries` values of any form

Operational rule: only auto-generated managed inputs pass through the helper; manual entries are inserted directly into the result set without normalization.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | Root-level generated destinations, backups, and known root-file patterns gain leading `/` | Extend `src/config.rs` tests around `all_gitignore_entries()` with representative managed filenames. |
| Unit | Slash-containing generated entries remain unchanged | Add config tests for nested destinations, module-map expansions, `.claude/commands/`, and `.agents/skills/*.bak`. |
| Unit | Manual `[gitignore].entries` remain verbatim | Add config tests mixing manual bare filenames with managed entries and assert only managed entries are normalized. |
| Integration-ish/unit | Doctor uses same normalized set | Add `doctor_tests` coverage that extracted `/AGENTS.md` matches expectations while legacy `AGENTS.md` is missing/extra drift relative to `all_gitignore_entries()`. |
| Existing render tests | Rendering contract remains verbatim | Keep `src/gitignore.rs` tests unchanged except optional assertion updates if a normalized entry is passed in. |

## Migration / Rollout

No migration required. Repositories using managed `.gitignore` will observe a one-time corrective diff for root-scoped managed file entries after the next sync, and doctor will report old unscoped managed entries as out of date until regenerated.

## Open Questions

- [ ] None blocking. During implementation, confirm whether any built-in root-file pattern currently includes wildcard characters but no slash; if so, keep it unchanged unless it clearly represents a single concrete root file.
