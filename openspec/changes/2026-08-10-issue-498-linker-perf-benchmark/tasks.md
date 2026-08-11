# Tasks: Benchmark and Optimize Large Linker Synchronization Runs

Decision needed before apply: Yes
Chained PRs recommended: Yes
Chain strategy: pending
400-line budget risk: High

## Review Workload Forecast

Changed lines est.: 800–1,150 · delivery: ask-on-risk.

### Suggested Work Units (#498; pending)

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Phase A: dev-bench + timing + baseline | PR 1 | base `main`; hidden cmd + baseline doc |
| 2 | B1 scoped `path_cache` invalidation | PR 2 | base PR 1; REQ-023, security gate |
| 3 | B2 single existence probe | PR 3 | base PR 2; `symlinks.rs:46-57` |
| 4 | B3 `DirEntry` reuse | PR 4 | base PR 3; `apply.rs`/`symlinks.rs` |
| 5 | B4 sorted iteration + final metrics | PR 5 | base PR 4; determinism + baseline update |

## Phase 1: Infrastructure — dev-bench harness and timing

- [x] 1.1 Create `src/linker/timing.rs` (sink types, `reset`, add-* + getters); `cargo check --all-targets --all-features`.
- [x] 1.2 TDD: failing unit asserting sink records spans; add private `timing` + `set_timing` to `Linker` (`mod.rs`); guarded spans at `process_target` (apply.rs:158), `create_symlink` (symlinks.rs:17), `relative_path` (paths.rs:239), `get_nested_glob_matches` (discovery.rs:159).
- [x] 1.3 Create `src/commands/dev_bench.rs` with `mod fixtures` (BTreeMap `Config` + `TempDir`, names `f{0:04}.md`, content `"FIXED\n"`, no `rand`); wire `#[command(hide = true)]` in `main.rs` (precedent `DevInstall`); unit: two builds → identical sorted name sets.
- [x] 1.4 Cell runner: matrix flat `symlink-contents` × deep `nested-glob` (`**/AGENTS.md`) × N=100/1,000/5,000 + small gate N=4; fresh `Linker`/cell; `--runs` default 5, min 2 (reject below, do not clamp); cold=run1, warm=median(2..R); assert `created==N`; human table + `--json`; stdout `/dev/null` via `extern "C" dup2` (Unix; Windows skip).
- [x] 1.5 Add `#[ignore]`d `dev_bench_smoke`; verify `cargo test --release --bin agentsync dev_bench_smoke -- --ignored`.

## Phase 2: Implementation — baseline capture

- [x] 2.1 `cargo build --release` (lto, codegen-units=1, opt-level=3), then `cargo run --release -- dev-bench --runs 5`; record all cells (cold/warm + 4 attributions) in `benchmark-baseline.md`.
- [x] 2.2 Verify `--help` hides dev-bench; `tests/contracts/` unchanged: `cargo test --all-features`.

## Phase 3: Implementation — Phase B optimizations (TDD, one commit each)

- [x] 3.1 B1: failing unit (siblings reuse cached `from_dir` mid-run; empty after 2nd `sync()`) → `invalidate_path(path)` in `src/linker/paths.rs` (drop exact key + `starts_with` keys); replace full clears at `symlinks.rs:99,138,165`, `clean.rs:96`, `apply.rs:283`; keep full clear at `sync()` start; `cargo test --test all_tests unit::linker_security`.
- [x] 3.2 B2: failing unit (nonexistent→created, regular→`.bak`, broken symlink→no backup) → single `fs::symlink_metadata(dest)` match in `create_symlink` (`symlinks.rs:46-57`); counters unchanged.
- [x] 3.3 B3: failing unit (no duplicate per-child stat; counters identical) → `resolve_source_path_with_hint(..., Some(true))` (`apply.rs:206/:235`) from contents loop (`symlinks.rs:255`); compression/revalidate unchanged.
- [ ] 3.4 B4: failing integration (two runs → byte-identical stdout) → `read_dir` collect + `sort_by_key(file_name)` (`symlinks.rs:237`); `WalkDir::new(..).sort_by_file_name()` (`discovery.rs:213`); update order-asserting contracts (none exist); `cargo test --test all_tests`.

## Phase 4: Testing — final benchmark and verification

- [ ] 4.1 Rerun `cargo run --release -- dev-bench --runs 5`; record before/after per opt in `benchmark-baseline.md`; small gate within noise band.
- [ ] 4.2 Quality gate: `cargo fmt --all -- --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all-features`; confirm hidden `--help`, exit-code contract, `linker_security.rs` green.
