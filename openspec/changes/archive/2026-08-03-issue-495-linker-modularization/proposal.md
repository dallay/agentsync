# Proposal: Modularize the linker synchronization engine

## Intent

Issue #495 addresses the maintenance cost of `src/linker.rs` (4,687 lines), which owns the
synchronization engine. Make path safety, symlink mutation, discovery, apply,
and clean behavior reviewable without changing observable behavior.

## Scope

### In Scope

- Mechanically extract `src/linker.rs` into Rust modules while preserving APIs, state,
  errors, counters, output, cache resets, and all four sync types.
- Preserve apply, clean, MCP, compression, path-safety/TOCTOU, backup, and cross-platform behavior.
- Keep callers and tests working; verify path and symlink responsibilities separately.

### Out of Scope

- Parallelization, new caching, optimization, or async conversion.
- CLI behavior or output changes.
- Changes to security rules or path-validation semantics.
- New synchronization features or redesign of the public API.

## Capabilities

### New Capabilities

- None — this is a behavior-preserving refactor.

### Modified Capabilities

- None — `openspec/specs/core-sync-engine/spec.md` remains the behavioral contract and is not
  updated in this phase.

## Approach

Atomically transition `src/linker.rs` to `src/linker/mod.rs`, retaining `Linker`, shared state, and
public types. Add:

```text
src/linker/{mod.rs,apply.rs,clean.rs,discovery.rs,paths.rs,symlinks.rs}
```

Extract blocks in dependency order: paths, symlinks, discovery, apply, then clean. Use
`pub(super)` only for required sibling access; avoid new services, traits, or caches. Keep inline
tests in the root until the boundary compiles; validate each move with formatting, compilation,
focused tests, then security/status/module-map and relevant full-suite tests.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/linker/` | Modified | Six-file implementation with unchanged façade and behavior. |
| `src/lib.rs`, `src/main.rs` | Verified | Exports and apply/clean/MCP callers remain unchanged. |
| `src/commands/{status,doctor}.rs` | Verified | Public linker helpers/accessors remain compatible. |
| `openspec/specs/core-sync-engine/spec.md` | Reference | Existing behavioral contract remains unchanged. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Rust module transition or visibility errors | High | Move atomically; compile after each extraction; use minimal `pub(super)`. |
| Borrow/cache ordering or platform behavior drifts | Med | Preserve function bodies, mutation order, cache invalidation, and cfg gates. |
| Clean/apply filtering or result counting changes | Med | Run focused and full existing test coverage; compare CLI behavior. |

## Rollback Plan

Revert the extraction commits and restore `src/linker.rs`; no API or data migration is required.

## Dependencies

- Existing Rust toolchain, tests, and the `core-sync-engine` behavioral contract.

## Success Criteria

- [x] The six-module structure exists and `src/linker.rs` is no longer the monolith.
- [x] Public APIs and apply/clean behavior remain unchanged for issue #495 acceptance cases.
- [x] `cargo fmt --all -- --check`, `cargo check --all-targets --all-features`, linker,
      security/status/module-map tests, and the full suite pass.
- [x] No base specs, CLI/output contracts, security rules, caches, or performance algorithms change.
