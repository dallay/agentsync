# Delta for Core Sync Engine

**Change:** `issue-498-linker-perf-benchmark` — reproducible large-sync benchmark, documented baseline, then measured optimizations (scoped cache invalidation, redundant-stat removal, deterministic ordering). Concurrency deferred.

## ADDED Requirements

### Requirement: Hidden dev-bench Subcommand

The system MUST provide a `dev-bench` subcommand (`src/commands/dev_bench.rs`) registered in `src/main.rs` with `#[command(hide = true)]`. `dev-bench` MUST NOT appear in `agentsync --help` or subcommand help, MUST NOT be required for normal operation, and MUST NOT change `tests/contracts/` machine-readable output.

#### Scenario: Help hides dev-bench

- GIVEN the built binary
- WHEN `agentsync --help` is run
- THEN `dev-bench` MUST NOT be listed
- AND all `tests/contracts/` expectations MUST remain unchanged

#### Scenario: Bench runs standalone

- GIVEN a `--release` binary
- WHEN `agentsync dev-bench` is run
- THEN it MUST exit 0 and print JSON or plain-text metrics
- AND it MUST make no changes outside its own `TempDir` fixtures

**References:** `src/commands/dev_bench.rs` (new), `src/main.rs` (hidden wiring, precedent `DevInstall`), `tests/contracts/`.

### Requirement: Deterministic Benchmark Fixtures

Fixture generation MUST build an in-memory `Config` (BTreeMap-based) plus a `TempDir` file tree using fixed file names (e.g., `f0000.md`..`f4999.md`). It MUST NOT use `rand` or wall-clock-derived names, so identical parameters produce byte-identical fixtures across runs and machines.

#### Scenario: Same parameters, identical fixtures

- GIVEN fixed N and repo shape
- WHEN fixtures are generated twice
- THEN the file-name sets MUST be identical and in sorted order

#### Scenario: No randomness

- GIVEN repeated fixture generation
- WHEN executed with the same parameters
- THEN names and counts MUST NOT vary between runs

**References:** `src/commands/dev_bench.rs` (fixtures), `tests/unit/linker_security.rs` fixture helpers.

### Requirement: Benchmark Matrix and Small-Repo Gate

The benchmark MUST cover N = 100, 1,000, and 5,000 links for two repo shapes — flat `symlink-contents` (many children in one directory) and deep `nested-glob` (`**/AGENTS.md` matches) — plus a small-repo case of 3–5 links as a no-regression gate.

#### Scenario: Flat large sync

- GIVEN N child files in one source directory
- WHEN `dev-bench` runs `symlink-contents` at N
- THEN exactly N links MUST be created
- AND per-N metrics MUST be recorded

#### Scenario: Deep nested-glob sync

- GIVEN a directory tree containing N `AGENTS.md` files
- WHEN `dev-bench` runs `nested-glob` with `**/AGENTS.md`
- THEN exactly N links MUST be created
- AND walk metrics MUST be recorded

#### Scenario: Small-repo gate

- GIVEN a 3–5 link configuration
- WHEN the gate case runs
- THEN it MUST complete and report results within the documented noise band

**References:** `src/commands/dev_bench.rs`, `SyncType::SymlinkContents` (`symlinks.rs:178`), `SyncType::NestedGlob` (`discovery.rs`).

### Requirement: Phase-Level Timing Attribution

The benchmark MUST time external `Instant::now` spans around `process_target` (attributed per sync type: metadata, canonicalize, link creation) and around `get_nested_glob_matches` (discovery walk). The system MUST NOT add syscall counters initially; they MAY be added later only if baseline data shows attribution is needed.

#### Scenario: Per-phase breakdown

- GIVEN a large benchmark run
- WHEN `dev-bench` completes
- THEN the report MUST attribute time to metadata, canonicalize, discovery, and link creation

#### Scenario: Walk timed separately

- GIVEN a `nested-glob` run
- WHEN executed
- THEN discovery time MUST be reported from the `get_nested_glob_matches` span

**References:** `src/linker/apply.rs::process_target`, `src/linker/discovery.rs::get_nested_glob_matches`.

### Requirement: Benchmark Methodology and Baseline Documentation

The benchmark MUST run cold and warm, report the median of multiple runs, and be executed under the `--release` profile. The pre-optimization baseline MUST be documented in `benchmark-baseline.md` inside the change folder. Optimizations MUST be applied only after measurement identifies the bottleneck.

#### Scenario: Cold vs warm with median

- GIVEN a matrix cell
- WHEN the benchmark runs cold and warm
- THEN both MUST be reported with the median of runs

#### Scenario: Baseline artifact created

- GIVEN the Phase A harness
- WHEN the first benchmark run completes
- THEN `benchmark-baseline.md` MUST record the metrics

**References:** `benchmark-baseline.md` (new, change artifact).

### Requirement: Deterministic Directory Iteration Order

The system MUST sort `fs::read_dir` iteration (`src/linker/symlinks.rs:237`) and `WalkDir` iteration (`src/linker/discovery.rs:213`) so all sync types produce deterministic link-creation and output order. If any `tests/contracts/` expectation asserts order, it MUST be updated explicitly to the sorted order.

#### Scenario: Flat order deterministic

- GIVEN a `symlink-contents` target with many children
- WHEN sync runs twice
- THEN created-link order and printed output MUST be identical

#### Scenario: Glob order deterministic

- GIVEN a `nested-glob` tree
- WHEN sync runs twice
- THEN the discovered match order MUST be identical

**References:** `symlinks.rs:237` (`create_symlinks_for_contents`), `discovery.rs:213` (`get_nested_glob_matches`).

### Requirement: No Redundant Existence Probe

In destination handling the system MUST NOT issue a separate `dest.exists()` probe immediately after `dest.is_symlink()` when the symlink probe alone determines the branch outcome (`symlinks.rs:46-57`). One existence-class stat MUST decide the branch, with behavior for broken symlinks unchanged.

#### Scenario: Single probe per destination

- GIVEN an existing destination
- WHEN `create_symlink` processes it
- THEN exactly one existence-class syscall MUST decide the branch
- AND result counters MUST match the current contract

#### Scenario: Broken symlink still handled as symlink

- GIVEN a destination that is a symlink to a missing target
- WHEN `create_symlink` processes it
- THEN it MUST take the symlink path (skip/update), never the backup path

**References:** `src/linker/symlinks.rs:46-57` (`create_symlink`).

### Requirement: Reuse DirEntry Existence in Symlink-Contents

`symlink-contents` MUST reuse the `DirEntry` existence already obtained from `fs::read_dir` instead of issuing a duplicate per-child `source.exists()` stat in `resolve_source_path` (`apply.rs:206`, called from `symlinks.rs:255`). Skipped/created/updated counts MUST remain identical.

#### Scenario: No duplicate stat per child

- GIVEN a `symlink-contents` target with N children
- WHEN sync runs
- THEN source existence MUST come from the `read_dir` entry
- AND `SyncResult` counters MUST match the existing contract

**References:** `src/linker/apply.rs::resolve_source_path`, `src/linker/symlinks.rs:237-262`.

### Requirement: No Unbounded Concurrency

The system MUST NOT introduce unbounded concurrency. Rayon/Tokio parallelization MUST NOT be added unless the baseline proves it is the bottleneck AND the user approves; the engine MUST remain sequential for this change.

#### Scenario: Execution remains sequential

- GIVEN a large benchmark run
- WHEN `dev-bench` executes
- THEN the run MUST be single-threaded

#### Scenario: Deferral requires approval

- GIVEN a measured bottleneck in sequential execution
- WHEN parallelization is proposed
- THEN it MUST NOT be implemented without user approval

**References:** `src/linker/mod.rs` (`Rc`/`RefCell`, non-`Send`).

### Requirement: Before/After Metrics and Contract Preservation

Phase B MUST rerun the benchmark after each optimization and record before/after metrics in `benchmark-baseline.md`. The small-repo gate MUST NOT regress. The error/exit contract MUST remain unchanged: exit code derived from error count and aggregate summary counters preserved.

#### Scenario: Before/after recorded

- GIVEN a merged optimization
- WHEN the benchmark reruns
- THEN before and after metrics MUST be recorded in `benchmark-baseline.md`

#### Scenario: Small gate stays green

- GIVEN any Phase B commit
- WHEN the small-repo gate runs
- THEN it MUST NOT regress beyond the documented noise band

#### Scenario: Exit and summary contract unchanged

- GIVEN a run with failing targets
- WHEN the command exits
- THEN the exit code MUST still derive from the errors count
- AND aggregate summary counters MUST be unchanged

**References:** `src/output.rs` (summary counters), `src/main.rs` (exit code).

## MODIFIED Requirements

### Requirement: Scoped Path Cache Invalidation (REQ-023)

The system MUST clear all internal caches (`path_cache`, `compression_cache`, `ensured_dirs`, `ensured_compressed`) at the start of each `sync()` call, ensuring filesystem changes between consecutive runs on the same `Linker` instance are reflected correctly.

Within a single run, the system MUST NOT clear the entire `path_cache` after every symlink mutation (`symlinks.rs:99,138,165`; `clean.rs:96`). Instead, invalidation MUST be scoped: only paths whose canonical identity can change due to a mutation MUST be invalidated, so `canonicalize_cached` survives for unchanged paths. Any path mutated by create/update/backup/remove MUST be re-canonicalized on next use.

The security semantics of `ensure_safe_path` and `revalidate_path` MUST be preserved: all scenarios in `tests/unit/linker_security.rs` MUST stay green.
(Previously: `path_cache` was cleared entirely after every symlink mutation within a run, so `canonicalize_cached` always missed.)

#### Scenario: SC-023a — Caches reset between runs

- GIVEN a `Linker` instance that has already run `sync()`
- AND the source file is modified between runs
- WHEN `sync()` is run again on the same instance
- THEN the updated content MUST be reflected (caches are cleared)

#### Scenario: SC-023b — Cache survives within a run

- GIVEN a large `symlink-contents` run
- WHEN `process_target` runs for many children
- THEN `canonicalize_cached` MUST hit for unchanged parent/ancestor paths
- AND canonicalize syscalls MUST be fewer than the current per-link count

#### Scenario: SC-023c — Mutated paths are re-canonicalized

- GIVEN a destination created, updated, backed up, or removed mid-run
- WHEN that path is resolved again in the same run
- THEN its canonical identity MUST be recomputed, not served from a stale cache

#### Scenario: SC-023d — Security semantics preserved

- GIVEN the TOCTOU and path-safety scenarios in `tests/unit/linker_security.rs`
- WHEN the test suite runs after the change
- THEN all scenarios MUST pass with unchanged outcomes

**References:** `src/linker/paths.rs::{canonicalize_cached, invalidate_path_cache}`, `src/linker/symlinks.rs:99,138,165`, `src/linker/clean.rs:96`, `tests/unit/linker_security.rs`.

## NON-GOALS

- Unbounded concurrency, Rayon, or Tokio parallelization (deferred; requires data + user approval).
- `criterion` or any new benchmark/runtime dependency (std-only `Instant`).
- Syscall-level counters initially (MAY be added later if attribution requires them).
- Any public CLI change: `dev-bench` stays hidden; `--help` and `tests/contracts/` output unchanged.
- Windows benchmark parity (open decision — skip and record).
- `website/docs` benchmark page (baseline doc is a change artifact; docs page pending user decision).
- Splitting Phase A / Phase B into separate changes (one change, two phases).
- Changing error/exit contracts or aggregate summary output format.

## Acceptance Criterion Traceability

| Proposal acceptance criterion | Requirements |
|---|---|
| Reproducible benchmark (N matrix, flat + deep, small gate), documented baseline | Deterministic Benchmark Fixtures; Benchmark Matrix and Small-Repo Gate; Benchmark Methodology and Baseline Documentation |
| Before/after metrics for metadata, canonicalize, discovery, link creation | Phase-Level Timing Attribution; Before/After Metrics and Contract Preservation |
| Bottleneck identified from data; optimizations follow measurement | Benchmark Methodology and Baseline Documentation; Before/After Metrics and Contract Preservation |
| No unbounded concurrency | No Unbounded Concurrency |
| No small-repo regression; deterministic errors/output; `linker_security.rs` green | Before/After Metrics and Contract Preservation; Deterministic Directory Iteration Order; Scoped Path Cache Invalidation (REQ-023); Hidden dev-bench Subcommand |
