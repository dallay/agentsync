## Exploration: Issue #495 linker modularization

### Current State
`src/linker.rs` is a single 4,687-line Rust module. It currently compiles and owns the complete synchronization engine: public API/types (`SyncOptions`, `SyncResult`, `Linker`, `SymlinkContentsChildExpectation`), linker construction/accessors, path safety and canonicalization, apply orchestration, source/compression resolution, symlink creation/update/backup, `symlink-contents`, nested-glob discovery and template expansion, module-map processing, relative path calculation, clean for all sync types, MCP delegation, low-level glob/compression/filesystem helpers, and an inline test module.

The public boundary is already narrow. `src/lib.rs:11,18` exposes `linker` and re-exports `Linker`, `SyncOptions`, and `SyncResult`. `src/main.rs:267-334,388-418` constructs `Linker`, calls `clean`, `sync`, `config`, `project_root`, and `sync_mcp`; it also merges clean results and handles gitignore/MCP presentation. `src/commands/status.rs:4,71-136,186-217,532-590` uses `Linker`, `expected_source_path`, `symlink_contents_expected_children`, `config`, and `project_root`. `src/commands/doctor.rs:37-113,141-180` uses the config/project-root accessors. `tests` also use the public API directly.

The current engine has four configured sync types, not only the issue’s examples: `Symlink`, `SymlinkContents`, `NestedGlob`, and `ModuleMap`. `sync()` at `src/linker.rs:386-469` resets all existing caches, applies disabled-agent/default-agent/CLI-agent filtering, iterates deterministic `BTreeMap` configuration, calls `process_target`, aggregates counters, and catches per-target errors. `process_target()` at `src/linker.rs:471-518` dispatches each sync type. `clean()` at `src/linker.rs:1286-1317` dispatches all four clean paths and intentionally cleans all configured managed links without applying the sync agent filter.

Path resolution and safety are tightly shared by every mutating path. `Linker` state at `src/linker.rs:69-82` contains `config`, `project_root`, `source_dir`, `path_cache`, `compression_cache`, `glob_cache`, `ensured_dirs`, `ensured_compressed`, and `canonical_project_root`. Safety helpers at `src/linker.rs:114-315` validate traversal/absolute paths, canonicalize existing ancestors, protect against symlink-ancestor escape, and distinguish safe unlink validation (the managed symlink may point outside the project) from write validation. `relative_path()` at `src/linker.rs:1238-1284` uses the existing canonicalization cache and project-root fallback when a destination parent does not yet exist. Do not change these semantics during the refactor.

Baseline evidence collected before any production/test edit:
- `wc -l src/linker.rs` => `4687 src/linker.rs`.
- `cargo test --lib linker` => 83 linker unit tests passed.
- `cargo test --test test_security --test test_agent_adoption --test test_module_map_cli --test test_status_cli` => 12 external/security/adoption/module-map/status tests passed.
- An attempted target named `linker_security` is invalid because `tests/unit/linker_security.rs` is included by the `all_tests` harness rather than a standalone Cargo test target. The correct focused command is `cargo test --test all_tests ...` after inspecting the harness, or the full `cargo test --all-features` baseline.

### Affected Areas
- `src/linker.rs:1-82` — current module imports, constants, internal types, public `Linker` state, and cache state; this becomes the module root/state contract.
- `src/linker.rs:84-315` — constructor/accessors plus path canonicalization, destination safety, TOCTOU revalidation, and unlink validation; central shared path boundary.
- `src/linker.rs:317-383` — public status-facing source/child expectation helpers; must remain reachable as `agentsync::linker::*` with unchanged signatures.
- `src/linker.rs:385-638` — apply orchestration, target dispatch, compression/source resolution, directory creation, and compressed-file writes; split across `apply.rs` with shared path/symlink helpers.
- `src/linker.rs:640-879` — single-link and contents-link handling, existing-link update, backup replacement, and circular-destination guard; `symlinks.rs`.
- `src/linker.rs:881-1174` — destination-template expansion plus nested-glob cache/walk/process; `discovery.rs` (template/glob helpers may be private to that sibling but cross-module callers need `pub(super)`).
- `src/linker.rs:1176-1284` — module-map apply, canonicalized relative path resolution; module-map dispatch stays in `apply.rs`, path calculation in `paths.rs`.
- `src/linker.rs:1286-1476` — public clean orchestration and four clean implementations; `clean.rs`, sharing path and symlink-removal helpers.
- `src/linker.rs:1478-1537` — public `sync_mcp`; likely remains in `mod.rs` or a narrow `apply.rs` section because it is a public façade used by `main.rs` and delegates to `McpGenerator`, not part of link discovery/path I/O.
- `src/linker.rs:1540-1822` — MCP filter, compression algorithm helpers, simple/path-aware glob matchers, backup/remove filesystem helpers; distribute compression to `apply.rs`/`discovery.rs` only if needed, and low-level backup/removal to `symlinks.rs`.
- `src/linker.rs:1824-4687` — one inline `#[cfg(test)] mod tests` containing 83 unit tests for patterns, templates, initialization, safety, apply, clean, MCP, nested-glob, and module-map behavior; preserve tests during the move, then split only if needed after compiling.
- `openspec/specs/core-sync-engine/spec.md:1-468` — retrospec source of truth covering construction, symlink/symlink-contents, status expectations, relative paths, safety/TOCTOU, filtering, dry-run, clean, backups, compression, cache reset, cross-platform behavior, error handling, gitignore, and apply integration.
- `src/lib.rs:7-19` — public module/re-export boundary; no external API rename is needed.
- `src/main.rs:181-418` — apply/clean CLI integration and MCP/gitignore orchestration; must remain behaviorally untouched.
- `src/commands/status.rs:71-136,532-590` — status integration depends on the public child expectation type and two public Linker helper methods.
- `src/commands/doctor.rs:37-113,141-180` — diagnostic callers depend on `config()` and `project_root()` only.
- `tests/unit/linker_security.rs:44-372` — external path safety, repeated sync, unlink, nested-glob error, and missing-source coverage.
- `tests/test_security.rs:7-193` and `tests/security_repro/mod.rs:8-167` — traversal, absolute destination, source escape, and symlink-ancestor regression coverage.
- `src/commands/status_tests.rs:17-821`, `tests/test_agent_adoption.rs:66-544`, and `tests/test_module_map_cli.rs:33-107` — status/apply/module-map integration coverage.

### Responsibility and Dependency Graph

1. **Module root / state / public façade (`mod.rs`)**
   - Keep `SyncOptions`, `SyncResult`, `ResolvedSource`, `SymlinkContentsChildExpectation`, `ExistingSymlinkAction` if sibling modules use it, nested-glob cache aliases, the `Linker` fields, constructor, `project_root()`, `config()`, public `sync()`, public `clean()`, public `sync_mcp()`, and public status-facing helpers (or their `impl` blocks in sibling files with the same module visibility).
   - Declare `mod apply; mod clean; mod discovery; mod paths; mod symlinks;` and own the imports only needed by root/types. Avoid duplicate module names if migrating from `src/linker.rs`: Rust resolves `src/linker.rs` and `src/linker/mod.rs` as competing module files, so the implementation must be moved/renamed atomically rather than creating both.
   - Keep the external surface unchanged: no caller should import `linker::paths` or `linker::symlinks` unless deliberately made public later.

2. **Apply/orchestration (`apply.rs`)**
   - `sync()` body and per-target dispatch (`src/linker.rs:386-518`) because it owns agent selection, result aggregation, and the four sync-type entry points.
   - Source resolution/compression decisions (`520-560`), `ensure_directory` if treated as a mutation prerequisite, `write_compressed_agents_md` (`587-638`), and module-map application (`1176-1236`).
   - `sync_mcp()` (`1478-1537`) can stay in `mod.rs` as public API or move here as an `impl Linker`; it is a façade over `McpGenerator` and must not be accidentally coupled to path/symlink internals.
   - Dependencies: config iteration/filtering (`crate::agent_ids`, `Config`, `SyncType`, `TargetConfig`), `paths` for destination/source validation and relative paths, `symlinks` for actual link creation, `discovery` for nested-glob processing.

3. **Clean (`clean.rs`)
   - Public `clean()` and `clean_symlink_target`, `clean_symlink_contents_target`, `clean_nested_glob_target`, and `clean_module_map_target` (`1286-1476`).
   - Dependencies: `paths::{ensure_safe_destination, revalidate_path, revalidate_unlink_path}`, `discovery::{get_nested_glob_matches, expand_destination_template}`, `symlinks::remove_symlink`, config module-map filename resolution, and shared `SyncOptions/SyncResult`.
   - Preserve the deliberate behavior that clean does not apply the CLI/default-agent filter and only removes symlinks, never regular files; `symlink-contents` does best-effort empty-directory removal.

4. **Paths (`paths.rs`)
   - Constructor-independent path/cache operations `invalidate_path_cache`, `canonicalize_uncached`, `get_canonical_project_root`, `ensure_safe_path`, `ensure_safe_destination`, `revalidate_path`, `revalidate_unlink_path`, `validate_absolute_unlink_path`, `validate_relative_unlink_parent`, `canonicalize_cached`, and `relative_path` (`114-315`, `1238-1284`).
   - Keep `project_root`, path caches, and canonical root cache on `Linker`; do not introduce a new path service or cache. `paths.rs` should access them through `pub(super)` fields/methods, or retain one private implementation boundary in `mod.rs` if this avoids exposing state.
   - This is the primary isolated path-resolution test boundary. Existing tests in `src/linker.rs:2033-2124` and external security tests must continue to run; new tests are not required in explore and no tests should be changed here.

5. **Symlinks (`symlinks.rs`)
   - `create_symlink`, `handle_existing_symlink`, `backup_existing_destination`, `create_symlinks_for_contents`, `remove_existing_path`, `remove_symlink`, and `backup_path_for_destination` (`641-879`, `1787-1822`).
   - This is the symlink-handling boundary, including Unix/Windows creation, Windows directory-link removal, relative targets supplied by `paths`, dry-run accounting, replacement/backup, circular destination guard, compression-aware child resolution, and per-child result aggregation.
   - Dependencies: paths validation/revalidation, apply source resolution, compression helper, pattern matcher, and shared state caches. Avoid changing the order of `ensure_directory`, relative target calculation, existing-link handling, and final revalidation.

6. **Discovery (`discovery.rs`)
   - `expand_destination_template`, `get_nested_glob_matches`, `process_nested_glob`, `for_each_nested_glob_match`, `matches_pattern`, `matches_path_glob` (test helper), `path_glob_match_iter`, and `try_match_segment` (`881-1174`, `1650-1785`).
   - This owns directory walking (`WalkDir::follow_links(false)`), pattern/exclude semantics, path templates, glob cache, and nested-glob apply/clean coordination. Keep the existing `NestedGlobKey`/`NestedGlobMatches` types and cache reset behavior; do not optimize or parallelize.
   - `expand_destination_template` is currently an associated private method called by internal tests and nested-glob code. Moving it may require `pub(super)` (or preserving an associated wrapper in `mod.rs`) so current inline tests remain valid.

7. **Compression/helper placement
   - Compression decision and writing are called from apply and symlink-contents, while `expected_source_path` and child discovery also need `is_agents_md_path`/`compressed_agents_md_path`. Keep the path-name helpers in a shared/private location (`mod.rs` or `apply.rs` with `pub(super)`), and move only the content algorithm (`detect_fence_delimiter`, `toggle_fence`, `compress_agents_md_content`, `split_leading_whitespace`, `normalize_inline_whitespace_to`) as a cohesive group. The behavior is specified by `core-sync-engine` REQ-022 and tested at `src/linker.rs:2171-2220` and `2823-2867`.

### Approaches
1. **Direct file-to-module extraction with a thin `mod.rs` façade (recommended)** — Convert `src/linker.rs` into `src/linker/mod.rs`, declare the five focused child modules, move existing function bodies without redesign, and use `pub(super)` only for sibling access. Preserve `Linker` state and public signatures; move the inline tests last or keep them temporarily in `mod.rs` until each extraction compiles.
   - Pros: smallest semantic delta; preserves `Linker` as the shared state owner; supports the requested file layout; makes path and symlink responsibilities independently reviewable/testable; rollback is a mechanical file move.
   - Cons: Rust visibility/import work is non-trivial; `impl Linker` methods are distributed across files; test module placement must be handled carefully.
   - Effort: Medium/High.

2. **Extract standalone free-function services with explicit context structs** — Introduce path/symlink/discovery contexts and pass them through apply/clean.
   - Pros: stronger compile-time boundaries and potentially easier unit tests.
   - Cons: changes signatures and ownership/borrowing patterns; risks functional drift and a larger diff; likely introduces abstractions outside issue scope; harder to preserve current `RefCell` cache behavior.
   - Effort: High.

3. **Keep `src/linker.rs` and add wrapper modules/re-exports** — Leave implementation in the monolith while exposing thin modules.
   - Pros: low immediate borrow/visibility risk.
   - Cons: fails the issue’s purpose and requested focused module structure; responsibility remains mixed; does not genuinely isolate path/symlink handling.
   - Effort: Low, but not acceptable for #495.

### Recommendation
Use approach 1. Treat this as a mechanical module extraction, not a redesign. First rename the file to `src/linker/mod.rs` and add child declarations. Keep all state and public types in the root module, then extract in dependency order: `paths.rs` (shared safety/canonicalization), `symlinks.rs` (link mutation primitives), `discovery.rs` (glob/template traversal), `apply.rs` (sync orchestration/source/module-map/MCP), and `clean.rs` (clean dispatch). A safer implementation sequence is to move one cohesive block at a time, run `cargo fmt --all -- --check` and focused linker tests after each block, and only then split the inline tests. Preserve all existing algorithms, output strings, cache invalidation calls, cfg gates, and error propagation. Do not add a trait, new cache, concurrency, or performance work.

`mod.rs` should remain the stable API and state hub, not a second implementation. It should declare children, own shared types/constants/state, expose the existing public methods, and provide only minimal `pub(super)` shared helpers. Child modules should contain `impl Linker` blocks, not duplicate state or new services. Keep `sync_mcp` behavior as-is even though it is not a link primitive because it is part of the current `Linker` public API and `apply` CLI flow.

### Risks
- **Rust module-file transition:** `src/linker.rs` and `src/linker/mod.rs` cannot coexist as the same module; the rename/move must be atomic and verified with `cargo check --all-targets --all-features`.
- **Visibility changes:** private root items are not automatically accessible to child modules. Use `pub(super)` selectively for shared types, fields, helpers, and methods; do not make internals `pub` without a caller requirement.
- **Borrow checker / `RefCell` ordering:** existing cache borrows, especially `get_nested_glob_matches`, `write_compressed_agents_md`, and cache invalidation after mutations, must keep the same scope and order. Moving code can accidentally extend a `RefCell` borrow across a call.
- **Inline test access:** tests currently live inside `linker.rs` and directly call private methods such as `ensure_safe_destination`, `process_nested_glob`, and `write_compressed_agents_md`. Moving tests into child modules changes `super::*` visibility; keep tests in `mod.rs` initially or add only `pub(super)` access needed by tests, without public API expansion.
- **Cross-module associated methods:** `Linker::expand_destination_template` and private helpers called from multiple extraction units may need `pub(super)` or root wrappers. Do not change call sites’ behavior while resolving this.
- **Status coupling:** `src/commands/status.rs` imports `SymlinkContentsChildExpectation` and calls two public helper methods. Their paths/signatures and semantics must remain unchanged.
- **Clean/apply semantics:** clean intentionally processes all configured targets regardless of agent filtering; apply filters disabled/default/CLI agents and catches target errors. Combining or sharing loops carelessly could change behavior.
- **All four sync types:** requested filenames emphasize apply/clean/discovery/paths/symlinks, but `nested-glob`, `module-map`, compression, and MCP are currently part of the engine and acceptance-sensitive. They cannot be dropped or silently reclassified.
- **Platform gates:** preserve `#[cfg(unix)]`, `#[cfg(windows)]`, and `FileTypeExt` behavior in symlink creation/removal; a Unix-only validation is insufficient.
- **Behavioral drift in output/counting:** dry-run messages, created/updated/skipped/removed/error counters, backup replacement, and cache invalidation are externally observable and covered by tests.
- **Test command shape:** `tests/unit/linker_security.rs` is not a standalone Cargo target; use `cargo test --test all_tests` with the appropriate module filter or run the full suite when validating extraction.
- **Scope creep:** the current file contains comments describing performance optimizations. The issue explicitly excludes new parallelization, caching, and optimization; retain existing behavior but do not improve algorithms during extraction.

### Ready for Proposal
Yes. The repository, current implementation, existing `core-sync-engine` retrospec, callers, tests, and baseline commands are sufficiently understood for `sdd-propose`. The proposal should state that #495 is a behavior-preserving mechanical extraction with the requested six-file structure, include rollback via reverting the module move, and explicitly exclude feature changes, parallelization, new caching, and optimization work.
