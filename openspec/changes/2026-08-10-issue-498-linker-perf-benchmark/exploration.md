# Exploration: Linker Performance (issue #498)

## Findings

- All-sequential pipeline; `Linker` is `Rc`/`RefCell` (non-`Send`). No concurrency anywhere.
- `path_cache` EXISTS but is invalidated after every symlink mutation
  (`src/linker/symlinks.rs:99,138,165`; `clean.rs:96`), so `canonicalize_cached`
  (`paths.rs:213`) always misses in large runs. Every link pays ~3 uncached
  `fs::canonicalize` (`paths.rs:249,258` via `relative_path`; `paths.rs:67`
  `canonicalize_uncached` in `ensure_safe_path`) plus 4–8 `stat`-class calls
  (ancestor `.exists()` walks in `ensure_safe_path` `paths.rs:57`,
  `dest.is_symlink` `symlinks.rs:46`, `dest.exists` `symlinks.rs:57`,
  `read_link` `symlinks.rs:119`).
- `glob_cache` works (`discovery.rs:159-195`); `ensured_dirs` works;
  `compression_cache` works.
- Discovery is allocation-light, `follow_links(false)`, not the bottleneck.
- Redundancies: `dest.exists()` after `dest.is_symlink()` (`symlinks.rs:46-57`);
  `resolve_source_path` `source.exists()` per child (`apply.rs:236`) duplicated
  from `read_dir`; `read_dir` (`symlinks.rs:237`) and `WalkDir`
  (`discovery.rs:213`) are UNSORTED = pre-existing nondeterminism.
- Determinism already OK: `BTreeMap` iteration, aggregate summary counters
  (`output.rs:229`), exit code from errors count.
- No `benches/` dir; dev-deps only `rand`. `tempfile` is a main dep. Fixture
  pattern: `TempDir` + in-memory `Config` with `BTreeMap` helpers
  (`tests/unit/linker_security.rs:8-42`). Precedent for hidden subcommand:
  `DevInstall` (`src/main.rs:165`).
- Contract tests exist under `tests/contracts/`.

## Recommended Decisions (defaults — user may veto at approval)

1. Change name: `2026-08-10-issue-498-linker-perf-benchmark`.
2. Harness: hidden `dev-bench` CLI subcommand (`src/commands/dev_bench.rs`,
   `#[command(hide = true)]`), NOT criterion (keep dev-deps minimal). Optional
   `#[ignore]`d timing test as CI smoke gate. JSON or plain text output.
3. Fixtures: deterministic in-memory `Config` + `TempDir`; N = 100 / 1,000 /
   5,000 links; flat symlink-contents (thousands of children) and deep
   nested-glob (`**/AGENTS.md`); small-repo case (3–5 links) as no-regression
   gate. Cold vs warm runs, median of runs, `--release`.
4. Measurement: external phase timing with `Instant::now` around
   `process_target` (per sync type) and `get_nested_glob_matches` (walk). No
   `#[cfg(feature="bench")]` syscall counters initially — only if baseline data
   shows attribution is needed.
5. Delivery: TWO sequential change phases within ONE proposal (Phase A: harness
   + documented baseline; Phase B: targeted optimizations + before/after
   metrics). Phase B candidates ranked: (1) fix `path_cache` invalidation so
   `canonicalize_cached` survives within a run / route `ensure_safe_path`
   through cache; (2) remove redundant `dest.exists()`; (3) reuse `DirEntry`
   existence in symlink-contents; (4) sort `read_dir` + `WalkDir` for
   determinism; (5) DEFERRED: bounded concurrency (requires `Send`/`Sync`
   refactor of `Linker`, high risk) — out of scope unless baseline proves it's
   the bottleneck AND user approves.
6. Baseline doc: change artifact `benchmark-baseline.md` (+ optional
   `website/docs` page — ask user).
