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

Every cell asserts `created == N` and `errors == 0` (asserted inside the benchmark).

## Results — human table (all timings in ms)

```
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
- **Biggest cost center**: link creation (incl. canonicalize) — 80–90% of total on the 5000 cells, consistent with per-link `source.exists()` probe + `relative_path` canonicalization. These are the Phase B targets (B1/B2/B3).

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
| B2 single existence probe | (pending) | | | |
| B3 DirEntry reuse | (pending) | | | |
| B4 sorted iteration + final metrics | (pending) | | | |

## Reproduce

```bash
cargo build --release
AGENTSYNC_NO_UPDATE_CHECK=1 cargo run --release -- dev-bench --runs 5      # human table
AGENTSYNC_NO_UPDATE_CHECK=1 cargo run --release -- dev-bench --runs 5 --json
```

`dev-bench` is hidden from `--help`; it exists for developer benchmarking only and does not
affect `tests/contracts/` machine-readable output.
