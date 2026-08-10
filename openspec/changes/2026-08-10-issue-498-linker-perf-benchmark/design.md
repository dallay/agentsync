# Design: Benchmark and Optimize Large Linker Synchronization Runs

## Technical Approach

Two phases, one change. **Phase A** ships a hidden `dev-bench` subcommand driving the real
`Linker::sync()` path against deterministic `TempDir` fixtures, attributes wall-clock time to the
four issue targets, records `benchmark-baseline.md`. **Phase B** applies measured optimizations
(scoped cache invalidation, single existence probe, `DirEntry` reuse, sorted iteration) — each an
independently revertible commit with before/after metrics. Engine stays sequential (`Rc`/`RefCell`);
no criterion, no syscall counters.

## Architecture Decisions

| Decision | Alternatives | Rationale |
|---|---|---|
| Hidden `dev-bench` subcommand (`src/commands/dev_bench.rs`, `#[command(hide = true)]`, precedent `DevInstall` `main.rs:165/244`) | criterion harness | No new dep; exercises real binary + real `sync()`; hidden → `--help`/`tests/contracts/` untouched (no apply-order contracts exist). |
| Timing sink on `Linker` (`set_timing`, `Option<Rc<RefCell<TimingSink>>>`) | Field on `SyncOptions`; bench re-implementing orchestration | `SyncOptions` literals (`main.rs:358`, integration tests) break with a new field; a private `Linker` field needs no call-site changes. |
| External `Instant::now` spans at function boundaries | Syscall/feature counters | Spec NON-GOAL: counters deferred. `Instant` wall-clock, guarded by `Option::is_some()` — zero cost when unset. |
| Scoped invalidation (`invalidate_path`) at mutation sites | Full persistence; full clear per mutation | Full persistence breaks TOCTOU; full clear is the measured cost. `ensure_safe_path`/`revalidate_path` use `canonicalize_uncached` (`paths.rs:67`) → security stays fresh (REQ-023 SC-023c/d). |
| Concurrency deferred | Rayon/Tokio | `Linker` is `Rc`-based; spec needs data + approval first. |

## Phase A — dev-bench Harness

**Fixtures** (`mod fixtures` in `dev_bench.rs`, `pub(crate)`): mirror `tests/unit/linker_security.rs:8-42`
BTreeMap helpers and `Config::project_root`/`source_dir` semantics (`config.rs:329/344`): `TempDir
{root}`, config at `{root}/agentsync.toml`, `source_dir: ".agents"` → `{root}/.agents`, one agent,
one target. Deterministic: `0..n`, content `"FIXED\n"`, names `f{0:04}.md`; no `rand`.

- **flat**: `.agents/flat/`, N children; `SymlinkContents`, dest `links/` → N links.
- **deep**: `.agents/deep/{i%8}/{i/8%8}/AGENTS.md`; `NestedGlob` `**/AGENTS.md`, template
  `docs/{relative_path}` → N links.
- **small gate**: flat, N=4 (3–5 band).

**Cell run** (fresh `Linker` per cell; `sync()` clears caches at `apply.rs:28-35` between runs):

```text
dev-bench ──► for (shape,size) in [(flat|deep) × (100,1000,5000)] + small:
  build_fixture → Linker::new + set_timing(sink) → runs 1..R (R=--runs, default 5):
    sink.reset(); t0=now(); sync(&opts)                     [real path]
      process_target ──► span:target            (apply.rs:158)
        symlink-contents: read_dir→resolve→create_symlink  [span:link, symlinks.rs:17]
          └─ relative_path                                 [span:canonicalize, paths.rs:239]
        nested-glob: get_nested_glob_matches               [span:discovery, discovery.rs:159]
    t1=now(); assert created==size, errors==0
  report: cold=run1, warm=median(runs 2..R)
```

**Attribution** (no double counting): discovery = walk span; link creation = Σ create_symlink spans
(incl. canonicalize); canonicalize = Σ relative_path spans (subset); metadata = target span −
links (− discovery for glob).

**Output**: human table or `--json` doc. Per-link `println!` (`symlinks.rs:101`) is unconditional,
so JSON mode swaps stdout fd to `/dev/null` via `extern "C" dup2` (Unix; Windows: skip-and-record).
Hidden command never collides with `tests/contracts/`.

**Baseline**: `benchmark-baseline.md` (change artifact) — cells × {cold, warm, metadata,
canonicalize, discovery, link creation}, pre-optimization, then before/after per B commit.
Run: `cargo build --release` (lto, codegen-units=1, opt-level=3).

## Phase B — Optimizations (after measurement, independently revertible)

| # | Change | Location | Testing |
|---|---|---|---|
| B1 | `invalidate_path(path)`: remove exact key + keys `starts_with(path)`; replaces full clears at `symlinks.rs:99,138,165`, `clean.rs:96`, `apply.rs:283`. Full clear kept at `sync()` start → SC-023a; from_dir/source survive in-run → SC-023b; mutated dest re-canonicalized → SC-023c; `linker_security.rs` green → SC-023d | `paths.rs:14` + sites | Unit (private cache access): siblings reuse cached from_dir; non-empty mid-run; empty after 2nd `sync()`; TOCTOU suite |
| B2 | `fs::symlink_metadata(dest)` match: symlink→existing-symlink, `Ok(_)`→backup, `NotFound`→created — one lstat decides; broken symlink lstat-succeeds → symlink path (same semantics) | `symlinks.rs:46-57` | Unit: nonexistent→created; regular→updated+.bak; broken symlink→no backup; counters unchanged |
| B3 | `resolve_source_path_with_hint(..., Some(true))` in contents loop — `DirEntry` implies existence, drops per-child `source.exists()` stat (`apply.rs:235`); compression + `revalidate_path` unchanged | `apply.rs:206`, `symlinks.rs:255` | Unit: counters identical; integration determinism |
| B4 | `read_dir` → collect + `sort_by_key(file_name)` (`symlinks.rs:237`); `WalkDir::new(..).sort_by_file_name()` (`discovery.rs:213`) — per-dir stable DFS → deterministic order + output | both sites | Unit: two runs identical order; integration: byte-identical stdout; update any order-asserting contract (none exist) |

## Determinism & Contracts

`config.agents`/`targets` are `BTreeMap` (sorted — unchanged); sorted dirs/walks make per-link order
and output deterministic. Exit contract unchanged: `errors > 0` → `Err` → exit 1 (`main.rs:176`);
bench asserts per-cell `created == size`, aggregates counters in the report.

## Interfaces / Contracts

```rust
// src/linker/timing.rs (new, pub)
#[derive(Debug, Default)] pub struct TimingSink { /* targets: RefCell<Vec<TargetSpan>>,
   link_creation, canonicalize, discovery: RefCell<Duration> */ }
#[derive(Debug, Clone, Copy)] pub struct TargetSpan { pub sync_type: &'static str, pub elapsed: Duration }
impl TimingSink { pub fn reset(&self); /* add_* + getters, one per span kind */ }

// src/linker/mod.rs — private Linker field:
//   timing: RefCell<Option<Rc<RefCell<TimingSink>>>>
//   pub fn set_timing(&self, sink: Option<Rc<RefCell<TimingSink>>>); // bench-only
```

## Testing Strategy

| Layer | What | Approach |
|---|---|---|
| Unit | Each B opt + fixture determinism | Inline `#[cfg(test)]`; identical name sets; small gate N=4 non-ignored |
| Integration | Determinism + counters, two fresh runs | Identical created/updated/skipped + output order |
| Smoke | `dev_bench_smoke` `#[ignore]`d, tiny N | `cargo test --release --bin agentsync dev_bench_smoke -- --ignored` |
| Regression | `linker_security.rs`, `tests/contracts/`, exit code | `cargo test --all-features` |
| Quality | fmt + clippy | `cargo fmt --all -- --check`; `cargo clippy --all-targets --all-features -- -D warnings` |

## Migration / Rollout

No migration. `dev-bench` hidden and removable; each B-opt a separate revertible commit;
`benchmark-baseline.md` preserves pre-change numbers. Release profile for measurements only.

## Open Questions

- Windows: skip bench or implement `SetStdHandle` suppression (skip-and-record default).
- If baseline shows attribution gaps, MAY add syscall counters later (spec-permitted).
