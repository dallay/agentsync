# Tasks: Modularización del motor de sincronización del linker

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: github-stacked-prs
400-line budget risk: High

## Review Workload Forecast

Estimated changed lines: 1,000–2,000 moved/visibility lines; tests stay in place.
Delivery strategy: ask-on-risk (default). Suggested split: PR 1 root; PR 2 paths; PR 3 symlinks; PR 4 discovery; PR 5 apply/clean.

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Baseline and atomic root move | PR 1 | Issue #495; start from trunk |
| 2 | `paths.rs` safety boundary | PR 2 | Depends on PR 1; focused tests |
| 3 | `symlinks.rs` mutation boundary | PR 3 | Depends on PR 2; preserve cfg/counters |
| 4 | `discovery.rs` glob/template boundary | PR 4 | Depends on PR 3; stay synchronous |
| 5 | `apply.rs` and `clean.rs` orchestration | PR 5 | Depends on PR 4; final suite |

## Approved Stack Layers

```text
main
└── issue-495-linker-root
    └── issue-495-linker-paths
        └── issue-495-linker-symlinks
            └── issue-495-linker-discovery
                └── issue-495-linker-apply-clean
```

Only one layer is implemented per apply pass. Stack creation, commits, pushes, and pull requests
require their own explicit authorization and are out of scope for this working-tree apply.

## Phase 1: Preparación, TDD y baseline

- [x] 1.1 Record `git status --short`, `wc -l src/linker.rs`, 83-test output, focused integration results, and `cargo test --test all_tests unit::linker_security`; do not modify code.
- [x] 1.2 This is mechanical: add no behavior tests. If a regression/structural test is needed, write and run it to failure before production changes; otherwise use the 83 inline tests and suite as the safety net.
- [x] 1.3 Snapshot counters, output, dry-run, filtering, clean, security, status, adoption, module-map, compression, MCP, nested-glob, and repeated-sync behavior.

## Phase 2: Foundation and atomic module transition

- [x] 2.1 Create `src/linker/` and atomically `git mv src/linker.rs src/linker/mod.rs`; never leave both paths; declare the five private child modules.
- [x] 2.2 Keep shared state/types, public façade, status helpers, `sync_mcp`, and all 83 inline tests in `mod.rs`; run format and `cargo check --all-targets --all-features`.

## Phase 3: Ordered extraction

- [x] 3.1 Move canonicalization, safety, TOCTOU, unlink validation, caches, and `relative_path` to `paths.rs`; use minimal `pub(super)`, compile, run `cargo test --lib linker`.
- [x] 3.2 Move link creation/update, backups, contents, removal, accounting, and Unix/Windows gates to `symlinks.rs`; compile and rerun linker/security tests.
- [x] 3.3 Move nested-glob walking, matching, excludes, templates, and cache access to `discovery.rs`; preserve ordering; compile and run linker/module-map tests.
- [x] 3.4 Move `sync`, dispatch, source/compression/module-map application to `apply.rs`; preserve filters, aggregation, reset, errors, MCP, and output; compile and run linker/adoption/status/module-map tests.
- [x] 3.5 Move `clean` and all four clean implementations to `clean.rs`; preserve no-filter cleaning, link-only removal, and empty-directory behavior; compile and run linker/security tests.
- [x] 3.6 After each 3.x step run `cargo fmt --all -- --check`, `cargo check --all-targets --all-features`, and narrow tests before continuing.

## Phase 4: Structural and final verification

- [x] 4.1 Assert `src/linker.rs` is absent, six module files exist, public exports/signatures are unchanged, and callers (`src/lib.rs`, `src/main.rs`, `src/commands/status.rs`, `src/commands/doctor.rs`) have no diff.
- [x] 4.2 Confirm no new traits/services/caches, parallelism, async, optimization, CLI/output, security, or algorithm changes; compare the snapshot.
- [x] 4.3 Run `cargo test --test test_security --test test_agent_adoption --test test_module_map_cli --test test_status_cli`, `cargo test --test all_tests unit::linker_security`, then `cargo test --all-features`.

## Exit Criteria and Risks

- [x] Six-file structure, unchanged callers/API, 83 inline tests retained, all scenarios/commands pass, and no base specs/state change.
- Risks: file collision, visibility/borrow errors, cache-order drift, platform symlink behavior, clean/apply filters, and scope creep; rollback via `git revert` of move/extraction commits.
