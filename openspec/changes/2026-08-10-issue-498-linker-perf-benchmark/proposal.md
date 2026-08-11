# Proposal: Benchmark and Optimize Large Linker Synchronization Runs

**id:** `issue-498-linker-perf-benchmark` · **status:** proposed · **summary:** Reproducible large-sync benchmark + documented baseline, then measured optimizations (scoped cache invalidation, redundant-stat removal, deterministic ordering). Concurrency deferred.

## Intent

GitHub #498: large syncs (hundreds/thousands of files) are slow, no data on where time goes. Exploration confirms an all-sequential pipeline; `path_cache` is invalidated after **every** symlink mutation (`symlinks.rs:99,138,165; clean.rs:96`), so `canonicalize_cached` always misses — each link pays ~3 `fs::canonicalize` + 4–8 `stat` calls. Optimize only after measuring.

## Scope

**In scope**
- Hidden `dev-bench` subcommand (`src/commands/dev_bench.rs`, `#[command(hide = true)]`), no criterion dep, JSON/plain metrics, optional `#[ignore]`d smoke test.
- Deterministic fixtures: in-memory `Config` + `TempDir`; N = 100/1,000/5,000; flat symlink-contents + deep nested-glob (`**/AGENTS.md`) + small-repo (3–5 links) regression gate; cold/warm, median-of-runs, `--release`.
- Timing around `process_target` (per sync type) and `get_nested_glob_matches`; baseline doc `benchmark-baseline.md` in change folder.
- Phase B (after data): (1) scoped `path_cache` invalidation so `canonicalize_cached` survives within a run — REQ-023 still honored (cleared between runs; mutated paths re-canonicalized); (2) drop redundant `dest.exists()`; (3) reuse `DirEntry` existence in symlink-contents; (4) sort `read_dir`/`WalkDir` (determinism).

**Out of scope:** unbounded concurrency; Rayon/Tokio unless baseline proves bottleneck **and** user approves; criterion; syscall counters unless attribution needed; public CLI/output changes.

## Capabilities

**New:** None — `dev-bench` is dev tooling.

**Modified:** `core-sync-engine` — delta adds determinism + performance requirements; amends REQ-023 cache semantics (scoped invalidation, still cleared between runs).

## Approach

Two sequential phases, one change:
- **Phase A** — harness + baseline (fixtures, metrics, baseline doc, small-repo gate).
- **Phase B** — optimize per data, rerun bench, record before/after in `benchmark-baseline.md`.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/commands/dev_bench.rs` | New | Hidden bench subcommand + fixtures |
| `src/main.rs` | Modified | Wire hidden subcommand |
| `src/linker/timing.rs` | Modified (A) | Phase A timing sink + guarded spans |
| `src/linker/{paths,symlinks,discovery,apply}.rs` | Modified (A), planned (B) | Phase A timing spans; Phase B cache scoping, stat removal, sorted walks |
| `src/linker/clean.rs` | Planned (B) | Phase B cache-aware clean (future unit) |
| `tests/unit/linker_security.rs` | Verified | TOCTOU/cache stays green |
| `tests/contracts/` | Verified | Output contracts unchanged |
| `openspec/specs/core-sync-engine/spec.md` | Reference | Delta in Phase B |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Cache change breaks TOCTOU/security tests | Med | Scoped invalidation only; keep `linker_security.rs` + REQ-023 green |
| Small-repo regression | Med | 3–5 link gate + full suite |
| Benchmark noise | Med | Median-of-runs, cold/warm split, release profile |
| Sorting changes error/output order | Med | Deliberate determinism; update contract tests explicitly |
| Hidden surface drifts from contracts | Low | Keep hidden; contracts untouched |

## Rollback Plan

`dev-bench` is hidden and removable; each Phase B optimization is an independently revertible commit; baseline doc preserves pre-change numbers.

## Dependencies

Rust 1.89, std-only timing (`Instant`); no new runtime/dev dependencies.

## Open Decisions

1. Delivery: two phases in one change vs. separate changes.
2. Sorting scope: all sync types vs. symlink-contents only.
3. Baseline doc: change artifact only vs. `website/docs` page.
4. Windows: skip bench or run and record.

## Acceptance Criteria

- [ ] Reproducible benchmark (N = 100/1,000/5,000; flat + deep; small gate), documented baseline.
- [ ] Before/after metrics for metadata, canonicalize, discovery, link creation.
- [ ] Bottleneck identified from data; optimizations follow measurement.
- [ ] No unbounded concurrency.
- [ ] No small-repo regression; deterministic errors/output; `linker_security.rs` green.
