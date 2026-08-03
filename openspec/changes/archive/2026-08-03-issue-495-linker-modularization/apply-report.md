# Apply Report: issue-495-linker-modularization

## Layer

- Strategy: `github-stacked-prs`
- Layer: `issue-495-linker-apply-clean`
- Position: 5 (after `issue-495-linker-discovery`, with `main` as trunk)
- Intended parent/base: `issue-495-linker-discovery`
- Branch: unchanged (`main` worktree; no branch or stack mutation performed)
- Scope: final apply/clean extraction; no stack, GitHub, commit, or push mutation

## Completed

- Moved the path canonicalization, destination-safety, TOCTOU revalidation, safe-unlink,
  path-cache, and `relative_path` implementations from `src/linker/mod.rs` to
  `src/linker/paths.rs` without changing operation order or behavior.
- Kept `Linker`, its fields/caches, public façade helpers, `sync_mcp`, callers, and all 83 inline
  tests in `mod.rs`; `expected_source_path` and `symlink_contents_expected_children` were not moved.
- Used `pub(super)` only for sibling/root callers: `invalidate_path_cache`,
  `ensure_safe_destination`, `revalidate_path`, `revalidate_unlink_path`, and `relative_path`.
  Canonicalization and validation helpers used only inside `paths.rs` remain private.
- Updated `tasks.md`: 2.1, 2.2, 3.1, 3.2, 3.3, 3.6, and the previously completed 4.1 remain
  complete; apply, clean, snapshot, and final verification tasks remain pending.
- Updated `state.yaml`: `current_phase: apply`, global apply remains partial, and
  `next: issue-495-linker-apply-clean`.
- Moved link creation/update, destination backups, symlink-contents iteration, removal helpers,
  accounting, and Unix/Windows symlink gates to `src/linker/symlinks.rs` without changing bodies,
  operation order, safety checks, cache invalidation, output, dry-run behavior, or counters.
- Kept shared types, façade, inline tests, discovery/apply/clean orchestration, and common
  `ensure_directory` in `mod.rs`; `ensure_directory` remains shared with compressed-source writes.
- Used `pub(super)` only for root/future sibling callers: `create_symlink`,
  `create_symlinks_for_contents`, and `remove_symlink`; the shared `ensure_directory` remains
  private in the root module.
- Moved nested-glob template expansion, cached matching, recursive no-follow walking, exclude
  matching/pruning, path normalization, and glob matcher helpers from `src/linker/mod.rs` to
  `src/linker/discovery.rs` without changing ordering, cache keys, destination placeholders, or
  WalkDir behavior.
- Moved `process_nested_glob` as the existing discovery-plus-per-match symlink dispatch boundary;
  its `create_symlink` call remains coupled to the discovery loop. Overall sync/process-target
  orchestration and module-map application remain in `mod.rs` for the next apply/clean layer.
- Kept `Linker`, shared types/state/caches, public façade, `sync`, `clean`, `process_target`,
  `resolve_source_path`, `ensure_directory`, `sync_mcp`, module-map processing, and all 83 inline
  tests in `mod.rs`.
- Updated `tasks.md`: 3.3 is complete; apply/clean and final verification tasks remain pending.
- Moved `sync`, `process_target`, source/compression resolution, compression helpers, and
  `process_module_map` to `src/linker/apply.rs`. Agent selection, BTreeMap iteration, four-way
  dispatch, counter aggregation, per-target error continuation/tracing, output, dry-run behavior,
  cache resets, compression, and the existing MCP façade were preserved.
- Kept `ensure_directory` in `src/linker/mod.rs`: it is shared by symlink mutation and compressed
  source writes, so moving it or duplicating it would widen ownership without improving the module
  boundary. The ownership decision is documented at the helper declaration.
- Moved `clean`, `clean_nested_glob_target`, `clean_symlink_contents_target`,
  `clean_symlink_target`, and `clean_module_map_target` to `src/linker/clean.rs`. Cleanup still
  visits every configured target without apply filters, removes only symlinks, preserves broken
  link handling, empty-directory best effort, counters/output/errors, and Unix/Windows removal
  behavior.
- Updated `tasks.md`: 3.4, 3.5, 4.2, and 4.3 are complete. `state.yaml` now points to `verify`.

## Verification

- `cargo fmt --all -- --check` — PASS.
- `cargo check --all-targets --all-features` — PASS.
- `cargo test --lib linker` — PASS, 83 passed, 0 failed.
- `cargo test --test test_security --test test_agent_adoption --test test_module_map_cli --test test_status_cli` — PASS, 12 passed, 0 failed (4 security, 6 adoption, 1 module-map, 1 status).
- `cargo test --test all_tests unit::linker_security` — PASS, 11 passed, 0 failed.
- `cargo test --all-features` — PASS; all executed test binaries passed (ignored tests remained ignored).
- Structural assertion — PASS: `src/linker.rs` absent; `src/linker/{mod,apply,clean,discovery,paths,symlinks}.rs`
  present; each apply/clean definition has one owner; callers and the base spec have no diff.
- Scope assertion — PASS: no new traits, services, caches, async, parallelism, optimization,
  CLI/output, security, or algorithm changes; `sync_mcp` and public status façade remain in `mod.rs`.
- No regression test was added because this layer is a mechanical extraction; existing tests provide
  the required safety net.

## Final Status

The paths, symlinks, discovery, apply, and clean layers are complete. The final implementation checks
pass, so the next recommended phase is `verify`.
