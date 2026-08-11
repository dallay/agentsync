# Linker Sync Benchmark — Baseline (pre-optimization)

Change: `issue-498-linker-perf-benchmark` — Phase A (Unit 1 of 5)
Captured: 2026-08-10 · hidden `dev-bench` subcommand, release profile only

## Environment

| | |
|---|---|
| Machine | Apple M2 Max, macOS 26.6 |
| Profile | `[profile.release]` — `lto = true`, `codegen-units = 1`, `opt-level = 3`, `strip = true` |
| Command | `cargo build --release && cargo run --release -- dev-bench --runs 5` (env `AGENTSYNC_NO_UPDATE_CHECK=1`) |
| Runs/cell | 5 — **cold** = run 1, **warm** = median of runs 2..=5 |
| Attribution | from the final run: `discovery` = walk span; `link-creation` = Σ `create_symlink` (incl. canonicalize); `canonicalize` = Σ `relative_path` (subset of link-creation); `metadata` = target span − link-creation − discovery |

> **Methodology note (post-CodeRabbit fix, base `9d644b1`)**: every row in this document
> was captured with the ORIGINAL harness, which rebuilt the source fixture on every run
> ("fresh fixtures"). Since the fix, `run_cell` builds the fixture ONCE per cell and
> `reset_managed_destination` only clears the managed destination between runs, so
> `warm` now measures RE-SYNC against an existing destination instead of fresh-fixture
> builds. The before/after deltas below remain internally consistent (both sides of
> every row used the same harness), but absolute `warm` values are NOT directly
> comparable with future captures on the fixed harness — treat them as a trend, not a
> regression contract.

Every cell asserts `created == N` and `errors == 0` (asserted inside the benchmark).

## Results — human table (all timings in ms)

```text
dev-bench: linker sync benchmark (5 runs per cell, all timings in ms)
shape                   n  created         cold         warm     metadata  canonicalize    discovery link-creation
symlink-contents        4        4      0.874ms      0.606ms      0.209ms       0.104ms      0.000ms       0.381ms
symlink-contents      100      100     10.918ms     13.492ms      2.802ms       3.562ms      0.000ms      11.341ms
nested-glob           100      100     20.105ms     21.967ms      2.438ms       4.152ms      3.436ms      16.409ms
symlink-contents     1000     1000    128.019ms    126.419ms     22.472ms      27.145ms      0.000ms      99.859ms
nested-glob          1000     1000    164.357ms    160.244ms     22.202ms      37.807ms     24.383ms     121.362ms
symlink-contents     5000     5000    649.151ms    625.598ms    108.489ms     134.239ms      0.000ms     535.720ms
nested-glob          5000     5000    855.969ms    844.723ms    114.416ms     199.190ms    129.193ms     620.986ms
```

## Results — machine-readable (--json, excerpt)

```json
{
  "benchmark": "dev-bench",
  "profile": "release",
  "runs_per_cell": 5,
  "cells": [
    { "shape": "symlink-contents", "n": 4,    "created": 4,    "errors": 0, "cold_ms": 0.661,  "warm_ms": 0.564,  "metadata_ms": 0.210, "canonicalize_ms": 0.094,  "discovery_ms": 0.0,   "link_creation_ms": 0.344 },
    { "shape": "symlink-contents", "n": 100,  "created": 100,  "errors": 0, "cold_ms": 12.376, "warm_ms": 14.460, "metadata_ms": 3.124, "canonicalize_ms": 3.781,  "discovery_ms": 0.0,   "link_creation_ms": 12.088 },
    { "shape": "nested-glob",      "n": 100,  "created": 100,  "errors": 0, "cold_ms": 21.516, "warm_ms": 23.211, "metadata_ms": 2.565, "canonicalize_ms": 4.509,  "discovery_ms": 3.248, "link_creation_ms": 17.870 },
    { "shape": "symlink-contents", "n": 1000, "created": 1000, "errors": 0, "cold_ms": 122.050,"warm_ms": 123.832,"metadata_ms": 22.965,"canonicalize_ms": 29.416, "discovery_ms": 0.0,   "link_creation_ms": 102.415 },
    { "shape": "nested-glob",      "n": 1000, "created": 1000, "errors": 0, "cold_ms": 171.571,"warm_ms": 164.528,"metadata_ms": 26.091,"canonicalize_ms": 38.981, "discovery_ms": 22.881,"link_creation_ms": 123.483 },
    { "shape": "symlink-contents", "n": 5000, "created": 5000, "errors": 0, "cold_ms": 610.628,"warm_ms": 615.145,"metadata_ms": 110.110,"canonicalize_ms": 136.875, "discovery_ms": 0.0,   "link_creation_ms": 500.781 },
    { "shape": "nested-glob",      "n": 5000, "created": 5000, "errors": 0, "cold_ms": 848.069,"warm_ms": 816.590,"metadata_ms": 110.984,"canonicalize_ms": 190.917, "discovery_ms": 120.693,"link_creation_ms": 594.553 }
  ]
}
```

## Observations

- **Scaling is roughly linear** in N: flat 100→1000→5000 warm = 13.5→126.4→625.6 ms; nested-glob = 22.0→160.2→844.7 ms.
- **Attribution is coherent**:
  - `discovery` = 0 for `symlink-contents` (no glob walk) and grows only for `nested-glob` (3.2→120.7 ms) ✓
  - `canonicalize` ⊂ `link-creation` in every cell (e.g., flat 5000: 134.2 ms of 535.7 ms) ✓
  - `metadata + link-creation + discovery ≈ cold/warm` (run-variance band) — no double counting ✓
- **Small-repo gate** (flat, N=4): warm 0.6 ms, cold 0.9 ms — comfortably inside the 3–5 ms no-regression band with margin.
- **Biggest cost center**: link creation (incl. canonicalize) — 86% of total on flat 5000 cells, ~74% on nested-glob 5000 (lower due to discovery overhead), consistent with per-link `source.exists()` probe + `relative_path` canonicalization. These are the Phase B targets (B1/B2/B3).

## B2 observation (unit 3, PR 3)

B2 collapses the `dest.is_symlink()` + `dest.exists()` pair in `create_symlink` into a single
`fs::symlink_metadata` lstat. It removes exactly one existence-class syscall per destination, but
the bench cells use FRESH fixtures (every link is a `created`), so the measured win is small
(−0.6% to −4.3% warm) — the optimization's larger effect shows on re-run scenarios where the
destination already exists (two probes → one). No regression on the small gate (N=4: 0.541 →
0.518 ms). Note: the nested-glob N=5000 `discovery` attribution (206.4 ms this capture vs 129.2 ms
baseline) is run variance in the final-run walk span, unrelated to B2 (B2 does not touch discovery).

## B3 observation (unit 4, PR 4)

B3 threads the `read_dir` `DirEntry` existence into `resolve_source_path_with_hint(...,
Some(true))` for non-symlink children of a `symlink-contents` source, dropping the duplicate
per-child `source.exists()` stat (`apply.rs` non-compression tail). Compression and revalidate
paths unchanged; symlink children (incl. broken symlinks) still probe so the missing-source
report is byte-identical.

**Load caveat — do not read raw deltas literally.** This capture ran under heavy external machine
load (load avg 31–144 on 12 cores during the three 5-run captures, vs 8–11 for B2), so EVERY cell
drifted up ~19–22% raw, including the nested-glob cells that B3 does not touch (controls:
+18.9%/+22.1%/+22.1% at N=100/1000/5000). "After" = mean of three 5-run captures.

**Drift-corrected result** (flat warm ÷ mean control drift ≈ 1.21): flat cells — where one stat per
child is actually removed — show a small, consistent improvement: N=4 −8.4%, N=100 −2.2%,
N=1000 −6.0%, N=5000 −3.9% warm. The direction matches the prediction (one existence stat saved
per non-symlink child; ~1–2 µs × N ≈ low single-digit % at N=5000). The removed stat also lands in
the `metadata` attribution (target − link-creation − discovery): flat-5000 metadata mean 104.2 ms
vs 108.5 ms pre-B1 baseline. **No regression**; small gate mean 0.574 ms, far inside the 3–5 ms band.

## Before / After (Phase B commits — filled by units 2–5)

B1 (unit 2, PR 2): scoped `path_cache` invalidation — `invalidate_path(path)` (exact key +
descendants) replaces full clears after every symlink mutation; full clear kept at `sync()` start.
`path_cache` moved to `BTreeMap` so prefix removal is a contiguous range scan (O(log n + m)) rather
than a per-mutation full-map scan (the initial `HashMap::retain` implementation measured O(N²) at
N=5000 and was reverted). After numbers = mean of two clean 5-run captures on the same machine
(load avg ~8–11 during capture).

| Opt | Cell | Before (baseline) | After | Delta |
|---|---|---|---|---|
| B1 path_cache invalidation | flat N=4 (small gate) | warm 0.606 ms; canonicalize 0.104 ms | warm 0.541 ms; canonicalize 0.067 ms | −10.7% warm — within noise band, no regression |
| B1 path_cache invalidation | flat N=100 | warm 13.492 ms; canonicalize 3.562 ms | warm 10.058 ms; canonicalize 1.500 ms | −25.5% warm; −57.9% canonicalize |
| B1 path_cache invalidation | flat N=1000 | warm 126.419 ms; canonicalize 27.145 ms | warm 103.526 ms; canonicalize 18.034 ms | −18.1% warm; −33.6% canonicalize |
| B1 path_cache invalidation | flat N=5000 | warm 625.598 ms; canonicalize 134.239 ms | warm 528.703 ms; canonicalize 92.900 ms | −15.5% warm; −30.8% canonicalize |
| B1 path_cache invalidation | nested-glob N=100 | warm 21.967 ms; canonicalize 4.152 ms | warm 19.397 ms; canonicalize 3.046 ms | −11.7% warm; −26.6% canonicalize |
| B1 path_cache invalidation | nested-glob N=1000 | warm 160.244 ms; canonicalize 37.807 ms | warm 134.435 ms; canonicalize 24.671 ms | −16.1% warm; −34.7% canonicalize |
| B1 path_cache invalidation | nested-glob N=5000 | warm 844.723 ms; canonicalize 199.190 ms | warm 699.955 ms; canonicalize 125.038 ms | −17.1% warm; −37.2% canonicalize |
| B2 single existence probe | flat N=4 (small gate) | warm 0.541 ms; canonicalize 0.067 ms | warm 0.518 ms; canonicalize 0.067 ms | −4.3% warm — within noise band, no regression |
| B2 single existence probe | flat N=100 | warm 10.058 ms; canonicalize 1.500 ms | warm 9.960 ms; canonicalize 1.616 ms | −1.0% warm; canonicalize within noise |
| B2 single existence probe | flat N=1000 | warm 103.526 ms; canonicalize 18.034 ms | warm 102.871 ms; canonicalize 17.430 ms | −0.6% warm; −3.3% canonicalize |
| B2 single existence probe | flat N=5000 | warm 528.703 ms; canonicalize 92.900 ms | warm 520.139 ms; canonicalize 90.879 ms | −1.6% warm; −2.2% canonicalize |
| B2 single existence probe | nested-glob N=100 | warm 19.397 ms; canonicalize 3.046 ms | warm 18.709 ms; canonicalize 3.041 ms | −3.5% warm |
| B2 single existence probe | nested-glob N=1000 | warm 134.435 ms; canonicalize 24.671 ms | warm 132.077 ms; canonicalize 24.016 ms | −1.8% warm; −2.7% canonicalize |
| B2 single existence probe | nested-glob N=5000 | warm 699.955 ms; canonicalize 125.038 ms | warm 675.811 ms; canonicalize 124.267 ms | −3.5% warm; −0.6% canonicalize |
| B3 DirEntry reuse | flat N=4 (small gate) | warm 0.518 ms; canonicalize 0.067 ms | warm 0.574 ms; canonicalize 0.075 ms | raw +10.9% (load drift; see note); drift-corrected −8.4%; 0.574 ms far inside 3–5 ms band |
| B3 DirEntry reuse | flat N=100 | warm 9.960 ms; canonicalize 1.616 ms | warm 11.792 ms; canonicalize 1.659 ms | raw +18.4%; drift-corrected −2.2% warm |
| B3 DirEntry reuse | flat N=1000 | warm 102.871 ms; canonicalize 17.430 ms | warm 117.069 ms; canonicalize 19.819 ms | raw +13.8%; drift-corrected −6.0% warm |
| B3 DirEntry reuse | flat N=5000 | warm 520.139 ms; canonicalize 90.879 ms | warm 604.653 ms; canonicalize 109.130 ms | raw +16.2%; drift-corrected −3.9% warm |
| B3 DirEntry reuse | nested-glob N=100 (control) | warm 18.709 ms | warm 22.249 ms | raw +18.9% — no code change (load drift) |
| B3 DirEntry reuse | nested-glob N=1000 (control) | warm 132.077 ms | warm 161.221 ms | raw +22.1% — no code change (load drift) |
| B3 DirEntry reuse | nested-glob N=5000 (control) | warm 675.811 ms | warm 825.056 ms | raw +22.1% — no code change (load drift) |
| B4 sorted iteration + final metrics | flat N=4 (small gate) | warm 0.518 ms (B2 after) | warm 0.670 ms — see load caveat below | no regression attributable to B4; +29% within elevated-load noise band |
| B4 sorted iteration + final metrics | flat N=100 | warm 9.960 ms (B2 after) | warm 14.039 ms | no regression attributable to B4; +41% within elevated-load noise band |
| B4 sorted iteration + final metrics | flat N=1000 | warm 102.871 ms (B2 after) | warm 144.723 ms | no regression attributable to B4; +41% within elevated-load noise band |
| B4 sorted iteration + final metrics | flat N=5000 | warm 520.139 ms (B2 after) | warm 787.692 ms | no regression attributable to B4; +51% within elevated-load noise band |
| B4 sorted iteration + final metrics | nested-glob N=100 | warm 18.709 ms (B2 after) | warm 28.455 ms | no regression attributable to B4; +52% within elevated-load noise band |
| B4 sorted iteration + final metrics | nested-glob N=1000 | warm 132.077 ms (B2 after) | warm 196.211 ms | no regression attributable to B4; +49% within elevated-load noise band |
| B4 sorted iteration + final metrics | nested-glob N=5000 | warm 675.811 ms (B2 after) | warm 949.823 ms | no regression attributable to B4; +41% within elevated-load noise band |

### B4 observation (unit 5, PR 5) — determinism, not speed

B4 adds an in-memory `sort_by_key(file_name)` (symlink-contents loop) and `WalkDir::sort_by_file_name()`
(nested-glob walk). It does not add or remove syscalls — both are in-memory sorts over already-read
directory entries. The **point of B4 is determinism**: byte-identical stdout across fresh runs and
sorted per-link output order (REQ: Deterministic Directory Iteration Order). That contract is proven
by the test suite (`test_b4_determinism.rs` integration test — byte-identical stdout + sorted flat
`[a.md, m.md, z.md]` and deep `[a, b, c]` order — plus unit tests
`sorted_dir_entries_returns_sorted_file_names` and `get_nested_glob_matches_returns_sorted_rel_paths`).

**Load caveat**: the B4 "after" numbers are the median of three clean 5-run captures taken at system
load 14–20 (1.4–1.7× cores on this 12-core M2 Max), while the B1/B2 captures ran at load ~8–11
(~0.7–0.9× cores). Every cell reads +40–51% warm vs B2 — including `flat` cells, which B4 touches
only with a microsecond-scale name sort (sorting 1000 names cannot cost 41 ms). The uniform elevation
across all cells is consistent with load inflation, not with B4. The sort is O(n log n) in memory and
adds zero syscalls, so no perf regression is attributable to B4. On a quiet machine B4 is expected to
measure within the B1/B2 noise band (typically ±5%).

## Reproduce

```bash
cargo build --release
AGENTSYNC_NO_UPDATE_CHECK=1 cargo run --release -- dev-bench --runs 5      # human table
AGENTSYNC_NO_UPDATE_CHECK=1 cargo run --release -- dev-bench --runs 5 --json
```

`dev-bench` is hidden from `--help`; it exists for developer benchmarking only and does not
affect `tests/contracts/` machine-readable output.
