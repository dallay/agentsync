//! Developer-only hidden `dev-bench` subcommand.
//!
//! Runs the real [`Linker::sync`] path against deterministic [`fixtures`]
//! (in-memory BTreeMap `Config` + `TempDir` trees) across the benchmark matrix
//! from the change design — flat `symlink-contents` × deep `nested-glob`
//! (`**/AGENTS.md`) at N = 100/1,000/5,000 plus a small-repo gate (flat, 4) —
//! and attributes wall-clock time to the four benchmarked phases via the
//! [`TimingSink`] installed on the linker.
//!
//! The command is registered with `#[command(hide = true)]` (precedent:
//! `DevInstall` in `src/main.rs`), so it never appears in `--help` and never
//! collides with `tests/contracts/` machine-readable output.
//!
//! Timing methodology (per design):
//! * cold = run 1, warm = median of runs 2..=R (`--runs`, default 5, min 2)
//! * attribution from the final run: discovery = walk span, link creation =
//!   Σ `create_symlink` spans, canonicalize = Σ `relative_path` spans,
//!   metadata = target − links − discovery (no double counting)
//! * every cell asserts `created == N` and `errors == 0`
//! * release profile only (`cargo run --release -- dev-bench`)
//!
//! The sync engine prints an unconditional per-link line, so cell runs swap
//! stdout to `/dev/null` via `extern "C" dup2` on Unix (documented no-op on
//! Windows: skip-and-record). No new dependencies: std-only `Instant` timing.

use anyhow::{Context, Result};
use clap::Args;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use agentsync::config::{AgentConfig, Config, SyncType, TargetConfig};
use agentsync::linker::{Linker, SyncOptions, TimingSink};
use tempfile::TempDir;

/// Arguments for the hidden `dev-bench` subcommand.
#[derive(Debug, Clone, Args)]
pub(crate) struct DevBenchArgs {
    /// Number of runs per matrix cell; cold = run 1, warm = median of runs 2..=R.
    #[arg(long, default_value_t = 5)]
    pub(crate) runs: usize,

    /// Emit machine-readable JSON metrics instead of the human table.
    #[arg(long)]
    pub(crate) json: bool,
}

/// Deterministic in-memory fixtures for the benchmark.
///
/// Mirrors the `tests/unit/linker_security.rs` BTreeMap config helpers and the
/// `Config::project_root` / `Config::source_dir` semantics: the config file
/// lives at `{root}/agentsync.toml` and the source directory is
/// `{root}/.agents`. File names are `f{0:04}.md` with content `"FIXED\n"` —
/// no `rand` and no wall-clock-derived names, so identical parameters produce
/// identical trees across runs and machines.
pub(crate) mod fixtures {
    use super::*;

    pub(crate) const FIXED_CONTENT: &str = "FIXED\n";

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Shape {
        Flat,
        Deep,
    }

    impl Shape {
        pub(crate) fn name(self) -> &'static str {
            match self {
                Shape::Flat => "symlink-contents",
                Shape::Deep => "nested-glob",
            }
        }
    }

    pub(crate) struct Fixture {
        /// Kept alive for the whole benchmark run: dropping the `TempDir`
        /// deletes the fixture files while `config`/`config_path` are still in
        /// use. Read directly only by tests (via `root.path()`).
        #[allow(dead_code)]
        pub(crate) root: TempDir,
        pub(crate) config: Config,
        pub(crate) config_path: PathBuf,
        /// Managed destination root (parent of all symlinks this fixture's
        /// target creates). Warm runs reset ONLY this tree between runs; the
        /// source tree under `.agents/` must survive.
        pub(crate) dest_root: PathBuf,
    }

    /// Fixed source file name for the flat shape.
    pub(crate) fn file_name(i: usize) -> String {
        format!("f{i:04}.md")
    }

    /// Build a deterministic fixture for `shape` with exactly `n` source items:
    ///
    /// * flat: `.agents/flat/` with `n` children; `symlink-contents` → `links/`.
    /// * deep: `.agents/deep/{i % 8}/{i / 8 % 8}/{i:05}/AGENTS.md`;
    ///   `nested-glob` `**/AGENTS.md` → `docs/{relative_path}`.
    pub(crate) fn build(shape: Shape, n: usize) -> Fixture {
        let root = TempDir::new().expect("failed to create benchmark TempDir");
        let root_path = root.path();
        let source_dir = root_path.join(".agents");

        match shape {
            Shape::Flat => {
                let flat = source_dir.join("flat");
                fs::create_dir_all(&flat).expect("failed to create flat source dir");
                for i in 0..n {
                    fs::write(flat.join(file_name(i)), FIXED_CONTENT)
                        .expect("failed to write flat fixture file");
                }
            }
            Shape::Deep => {
                for i in 0..n {
                    // 8×8 fan-out (design) plus a zero-padded per-file level:
                    // `{i % 8}/{i / 8 % 8}` alone yields only 64 unique dirs, so
                    // N > 64 would collide and silently create fewer files.
                    // The `{i:05}` level keeps every file unique for N ≤ 100_000.
                    let dir = source_dir
                        .join("deep")
                        .join(format!("{}", i % 8))
                        .join(format!("{}", (i / 8) % 8))
                        .join(format!("{i:05}"));
                    fs::create_dir_all(&dir).expect("failed to create deep source dir");
                    fs::write(dir.join("AGENTS.md"), FIXED_CONTENT)
                        .expect("failed to write deep fixture file");
                }
            }
        }

        let target = match shape {
            Shape::Flat => TargetConfig {
                source: "flat".to_string(),
                destination: "links".to_string(),
                sync_type: SyncType::SymlinkContents,
                pattern: None,
                exclude: vec![],
                mappings: vec![],
            },
            Shape::Deep => TargetConfig {
                source: ".agents/deep".to_string(),
                destination: "docs/{relative_path}".to_string(),
                sync_type: SyncType::NestedGlob,
                pattern: Some("**/AGENTS.md".to_string()),
                exclude: vec![],
                mappings: vec![],
            },
        };

        let config = make_config(target);
        let config_path = root_path.join("agentsync.toml");
        // The in-memory Config is authoritative; the empty marker file anchors
        // `Config::project_root` semantics at `{root}`.
        fs::write(&config_path, "").expect("failed to write config path marker");

        let dest_root = match shape {
            Shape::Flat => root_path.join("links"),
            Shape::Deep => root_path.join("docs"),
        };

        Fixture {
            root,
            config,
            config_path,
            dest_root,
        }
    }

    fn make_config(target: TargetConfig) -> Config {
        let mut targets = BTreeMap::new();
        targets.insert("target".to_string(), target);

        let agent_config = AgentConfig {
            enabled: true,
            description: String::new(),
            targets,
        };

        let mut agents = BTreeMap::new();
        agents.insert("test".to_string(), agent_config);

        Config {
            source_dir: ".agents".to_string(),
            compress_agents_md: false,
            default_agents: vec![],
            agents,
            gitignore: Default::default(),
            mcp: Default::default(),
            mcp_servers: Default::default(),
        }
    }
}

/// Suppress the sync engine's unconditional per-link `println!` output during
/// a benchmark cell so the report stays readable (and JSON stays parseable).
///
/// Unix: swaps file descriptor 1 to `/dev/null` for the cell duration and
/// restores it on drop — Rust's `std::io::stdout` writes through fd 1, so
/// every `println!` inside the sync lands in `/dev/null`. `dup`/`dup2` are
/// declared `extern "C"` to avoid adding a dependency.
///
/// Windows: documented no-op (skip-and-record) — per-link output is left
/// intact and a note is printed with the report.
#[cfg(unix)]
pub(crate) struct SuppressedStdout {
    saved: i32,
}

#[cfg(unix)]
unsafe extern "C" {
    fn dup(fd: i32) -> i32;
    fn dup2(oldfd: i32, newfd: i32) -> i32;
    fn close(fd: i32) -> i32;
}

#[cfg(unix)]
impl SuppressedStdout {
    pub(crate) fn suppress() -> std::io::Result<Self> {
        use std::os::fd::AsRawFd;

        let devnull = fs::File::open("/dev/null")?;
        let devnull_fd = devnull.as_raw_fd();

        let saved = unsafe { dup(1) };
        if saved < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if unsafe { dup2(devnull_fd, 1) } < 0 {
            let err = std::io::Error::last_os_error();
            unsafe { close(saved) };
            return Err(err);
        }
        Ok(SuppressedStdout { saved })
    }
}

#[cfg(unix)]
impl Drop for SuppressedStdout {
    fn drop(&mut self) {
        unsafe {
            dup2(self.saved, 1);
            close(self.saved);
        }
    }
}

#[cfg(not(unix))]
pub(crate) struct SuppressedStdout;

#[cfg(not(unix))]
impl SuppressedStdout {
    pub(crate) fn suppress() -> std::io::Result<Self> {
        // Windows: skip-and-record — per-link output is not suppressed.
        Ok(SuppressedStdout)
    }
}

/// Phase-level attribution for one benchmark run (no double counting).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Attribution {
    pub(crate) metadata: Duration,
    pub(crate) canonicalize: Duration,
    pub(crate) discovery: Duration,
    pub(crate) link_creation: Duration,
}

impl Attribution {
    fn from_sink(sink: &TimingSink) -> Self {
        Attribution {
            metadata: sink.metadata(),
            canonicalize: sink.canonicalize(),
            discovery: sink.discovery(),
            link_creation: sink.link_creation(),
        }
    }
}

/// Per-cell benchmark result.
#[derive(Debug)]
pub(crate) struct CellReport {
    pub(crate) shape: &'static str,
    pub(crate) n: usize,
    pub(crate) created: usize,
    pub(crate) errors: usize,
    pub(crate) cold: Duration,
    pub(crate) warm: Option<Duration>,
    pub(crate) metadata: Duration,
    pub(crate) canonicalize: Duration,
    pub(crate) discovery: Duration,
    pub(crate) link_creation: Duration,
}

/// Matrix: small-repo gate (flat, 4) plus flat × deep at 100 / 1,000 / 5,000.
fn matrix_cells() -> Vec<(fixtures::Shape, usize)> {
    let mut cells = Vec::new();
    cells.push((fixtures::Shape::Flat, 4)); // small-repo no-regression gate (3–5 band)
    for n in [100, 1_000, 5_000] {
        cells.push((fixtures::Shape::Flat, n));
        cells.push((fixtures::Shape::Deep, n));
    }
    cells
}

/// Run one matrix cell: a fresh fixture, linker, and sink per run. Cold = run
/// 1; warm = median of runs 2..=R; attribution from the final run. Every run
/// must create exactly `n` links with zero errors.
///
/// The source fixture is built ONCE per cell and reused across all runs;
/// between runs only the managed destination tree is reset. Runs 2..=R
/// therefore measure repeated synchronization of the same repository state
/// (same sources, same config, same link targets) rather than fresh-fixture
/// builds, which is what "warm" is documented to mean. A fresh [`Linker`] is
/// created per run so link-time caches cannot leak between samples.
fn run_cell(shape: fixtures::Shape, n: usize, runs: usize) -> Result<CellReport> {
    let fixture = fixtures::build(shape, n);
    let mut cold = None;
    let mut warm_runs: Vec<Duration> = Vec::with_capacity(runs.saturating_sub(1));
    let mut attribution = None;
    let mut created = 0usize;
    let mut errors = 0usize;

    for run in 1..=runs {
        if run > 1 {
            reset_managed_destination(&fixture)?;
        }
        let sink = Rc::new(RefCell::new(TimingSink::default()));
        let linker = Linker::new(fixture.config.clone(), fixture.config_path.clone());
        linker.set_timing(Some(Rc::clone(&sink)));
        sink.borrow_mut().reset();

        let _suppressed = SuppressedStdout::suppress()
            .with_context(|| "failed to suppress per-link output during benchmark cell")?;

        let start = Instant::now();
        let result = linker.sync(&SyncOptions::default())?;
        let elapsed = start.elapsed();

        drop(_suppressed);

        anyhow::ensure!(
            result.created == n,
            "cell {} x {n}: expected {n} links created, got {}",
            shape.name(),
            result.created
        );
        anyhow::ensure!(
            result.errors == 0,
            "cell {} x {n}: expected zero errors, got {}",
            shape.name(),
            result.errors
        );

        created = result.created;
        errors = result.errors;
        if run == 1 {
            cold = Some(elapsed);
        } else {
            warm_runs.push(elapsed);
        }
        attribution = Some(Attribution::from_sink(&sink.borrow()));
    }

    Ok(CellReport {
        shape: shape.name(),
        n,
        created,
        errors,
        cold: cold.expect("at least one run"),
        warm: median(&mut warm_runs),
        metadata: attribution.expect("at least one run").metadata,
        canonicalize: attribution.expect("at least one run").canonicalize,
        discovery: attribution.expect("at least one run").discovery,
        link_creation: attribution.expect("at least one run").link_creation,
    })
}

/// Median of a set of durations; for even counts, the average of the two
/// middle values.
fn median(durations: &mut [Duration]) -> Option<Duration> {
    durations.sort();
    let len = durations.len();
    match len {
        0 => None,
        1 => Some(durations[0]),
        _ if len % 2 == 1 => Some(durations[len / 2]),
        _ => Some((durations[len / 2 - 1] + durations[len / 2]) / 2),
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn format_ms(duration: Duration) -> String {
    format!("{:.3}ms", ms(duration))
}

/// Reset the managed destination tree of a fixture between benchmark runs so
/// warm samples re-sync the same repository state. Only the destination root
/// created by the previous run is removed; the source tree under `.agents/`
/// and the in-memory config survive, so runs 2..=R measure repeated
/// synchronization instead of fresh-fixture builds.
fn reset_managed_destination(fixture: &fixtures::Fixture) -> Result<()> {
    if fixture.dest_root.exists() {
        fs::remove_dir_all(&fixture.dest_root)
            .with_context(|| format!("failed to reset {}", fixture.dest_root.display()))?;
    }
    Ok(())
}

/// Release-only contract: the benchmark exists to record release baseline
/// data, so debug runs must fail loudly instead of labeling debug timings as
/// `"profile": "release"` (see `emit_json`).
#[cfg(debug_assertions)]
fn ensure_release_profile() -> Result<()> {
    anyhow::bail!(
        "dev-bench is release-only: run `cargo run --release -- dev-bench` so timings cannot be recorded as release baseline data"
    );
}

#[cfg(not(debug_assertions))]
fn ensure_release_profile() -> Result<()> {
    Ok(())
}

/// `--json` validity across platforms. On non-Unix targets per-link output
/// cannot be suppressed (see [`SuppressedStdout`]), so a JSON document would
/// be corrupt; reject early instead of emitting garbage.
#[cfg(unix)]
fn ensure_json_supported(_args: &DevBenchArgs) -> Result<()> {
    Ok(())
}

#[cfg(not(unix))]
fn ensure_json_supported(args: &DevBenchArgs) -> Result<()> {
    if args.json {
        anyhow::bail!(
            "dev-bench --json is not supported on this platform: per-link output cannot be suppressed, so stdout would not contain a parseable JSON document"
        );
    }
    Ok(())
}

/// The reported statistic is a median of cold samples (run 1) and warm
/// samples (runs 2..=R). A run count below 2 cannot produce both, so it must
/// be rejected instead of clamped with `.max(1)`.
fn ensure_runs_sufficient(runs: usize) -> Result<()> {
    anyhow::ensure!(
        runs >= 2,
        "dev-bench --runs must be at least 2 (one cold sample plus at least one warm sample for the median), got {runs}"
    );
    Ok(())
}

/// Run the hidden linker benchmark and emit the report.
pub(crate) fn run_dev_bench(args: DevBenchArgs) -> Result<()> {
    ensure_release_profile()?;
    ensure_json_supported(&args)?;
    ensure_runs_sufficient(args.runs)?;

    let runs = args.runs;

    // Flush any buffered pre-bench output so it cannot be lost to /dev/null
    // once cell runs suppress per-link stdout.
    use std::io::Write;
    std::io::stdout()
        .flush()
        .context("failed to flush stdout")?;

    let mut reports = Vec::new();
    for (shape, n) in matrix_cells() {
        reports.push(run_cell(shape, n, runs)?);
    }

    if args.json {
        emit_json(&reports, runs)?;
    } else {
        emit_table(&reports, runs);
        #[cfg(not(unix))]
        println!("note: per-link output is not suppressed on this platform (skip-and-record)");
    }
    Ok(())
}

fn emit_table(reports: &[CellReport], runs: usize) {
    println!("dev-bench: linker sync benchmark ({runs} runs per cell, all timings in ms)");
    println!(
        "{:<18} {:>6} {:>8} {:>12} {:>12} {:>12} {:>13} {:>12} {:>13}",
        "shape",
        "n",
        "created",
        "cold",
        "warm",
        "metadata",
        "canonicalize",
        "discovery",
        "link-creation"
    );
    for report in reports {
        println!(
            "{:<18} {:>6} {:>8} {:>12} {:>12} {:>12} {:>13} {:>12} {:>13}",
            report.shape,
            report.n,
            report.created,
            format_ms(report.cold),
            report
                .warm
                .map(format_ms)
                .unwrap_or_else(|| "-".to_string()),
            format_ms(report.metadata),
            format_ms(report.canonicalize),
            format_ms(report.discovery),
            format_ms(report.link_creation),
        );
    }
}

fn emit_json(reports: &[CellReport], runs: usize) -> Result<()> {
    let cells: Vec<serde_json::Value> = reports
        .iter()
        .map(|report| {
            serde_json::json!({
                "shape": report.shape,
                "n": report.n,
                "created": report.created,
                "errors": report.errors,
                "runs": runs,
                "cold_ms": ms(report.cold),
                "warm_ms": report.warm.map(ms),
                "metadata_ms": ms(report.metadata),
                "canonicalize_ms": ms(report.canonicalize),
                "discovery_ms": ms(report.discovery),
                "link_creation_ms": ms(report.link_creation),
            })
        })
        .collect();
    let report = serde_json::json!({
        "benchmark": "dev-bench",
        "profile": "release",
        "runs_per_cell": runs,
        "cells": cells,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collect the sorted file-name set a fixture would produce for `shape`.
    fn sorted_names(shape: fixtures::Shape, fixture: &fixtures::Fixture) -> Vec<String> {
        let mut names: Vec<String> = match shape {
            fixtures::Shape::Flat => {
                let dir = fixture.root.path().join(".agents").join("flat");
                fs::read_dir(&dir)
                    .unwrap()
                    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            }
            fixtures::Shape::Deep => {
                let root = fixture.root.path().join(".agents").join("deep");
                walkdir::WalkDir::new(&root)
                    .into_iter()
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| entry.file_type().is_file())
                    .map(|entry| {
                        entry
                            .path()
                            .strip_prefix(&root)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect()
            }
        };
        names.sort();
        names
    }

    #[test]
    fn fixtures_two_builds_produce_identical_sorted_name_sets() {
        for shape in [fixtures::Shape::Flat, fixtures::Shape::Deep] {
            let first = fixtures::build(shape, 100);
            let second = fixtures::build(shape, 100);
            let names_first = sorted_names(shape, &first);
            let names_second = sorted_names(shape, &second);

            assert_eq!(
                names_first,
                names_second,
                "{} fixture name sets must be identical across builds",
                shape.name()
            );
            assert_eq!(
                names_first.len(),
                100,
                "{} fixture count must be fixed",
                shape.name()
            );
            assert!(
                names_first.windows(2).all(|window| window[0] < window[1]),
                "{} fixture names must be sorted",
                shape.name()
            );
        }
    }

    #[test]
    fn flat_fixture_names_match_fixed_format_and_content() {
        let fixture = fixtures::build(fixtures::Shape::Flat, 4);
        let names = sorted_names(fixtures::Shape::Flat, &fixture);
        assert_eq!(names, vec!["f0000.md", "f0001.md", "f0002.md", "f0003.md"]);

        let content =
            fs::read_to_string(fixture.root.path().join(".agents/flat/f0000.md")).unwrap();
        assert_eq!(content, fixtures::FIXED_CONTENT);
    }

    /// Full-pipeline smoke: run a tiny matrix through the real `sync()` path.
    ///
    /// Ignored by default (it exercises the real engine against TempDir
    /// fixtures); run explicitly with:
    /// `cargo test --release --bin agentsync dev_bench_smoke -- --ignored`
    #[test]
    #[ignore = "runs the real sync engine; execute explicitly to keep the default suite fast"]
    fn dev_bench_smoke() {
        let reports: Vec<CellReport> = vec![
            run_cell(fixtures::Shape::Flat, 4, 2).unwrap(),
            run_cell(fixtures::Shape::Deep, 4, 2).unwrap(),
        ];

        for report in &reports {
            assert_eq!(report.created, report.n);
            assert_eq!(report.errors, 0);
            assert!(report.cold > Duration::ZERO);
            assert!(report.warm.is_some(), "warm = median of runs 2..=R");
        }

        let flat = &reports[0];
        assert_eq!(
            flat.discovery,
            Duration::ZERO,
            "flat sync must not walk globs"
        );
        assert!(flat.link_creation > Duration::ZERO);
        assert!(flat.canonicalize > Duration::ZERO);
        assert!(ms(flat.metadata) >= 0.0);

        let deep = &reports[1];
        assert!(
            deep.discovery > Duration::ZERO,
            "deep sync must time the walk"
        );
    }

    #[test]
    fn median_of_empty_slice_is_none() {
        let mut durations: Vec<Duration> = vec![];
        assert_eq!(median(&mut durations), None);
    }

    #[test]
    fn median_of_single_value_is_that_value() {
        let mut durations = vec![Duration::from_millis(7)];
        assert_eq!(median(&mut durations), Some(Duration::from_millis(7)));
    }

    #[test]
    fn median_of_odd_count_is_middle_value() {
        let mut durations = vec![
            Duration::from_millis(30),
            Duration::from_millis(10),
            Duration::from_millis(20),
        ];
        assert_eq!(median(&mut durations), Some(Duration::from_millis(20)));
    }

    #[test]
    fn median_of_even_count_is_average_of_middle_two() {
        let mut durations = vec![
            Duration::from_millis(10),
            Duration::from_millis(40),
            Duration::from_millis(20),
            Duration::from_millis(30),
        ];
        // sorted: 10, 20, 30, 40 -> (20 + 30) / 2 = 25
        assert_eq!(median(&mut durations), Some(Duration::from_millis(25)));
    }

    #[test]
    fn ms_and_format_ms_convert_durations_correctly() {
        assert_eq!(ms(Duration::from_millis(1500)), 1500.0);
        assert_eq!(format_ms(Duration::from_micros(1234)), "1.234ms");
        assert_eq!(format_ms(Duration::ZERO), "0.000ms");
    }

    #[test]
    fn matrix_cells_contains_small_gate_and_full_matrix() {
        let cells = matrix_cells();

        // Small-repo no-regression gate: flat, N=4, must be first.
        assert_eq!(cells[0], (fixtures::Shape::Flat, 4));

        // Flat x Deep at each of 100 / 1,000 / 5,000.
        for n in [100usize, 1_000, 5_000] {
            assert!(
                cells.contains(&(fixtures::Shape::Flat, n)),
                "matrix must include flat at N={n}"
            );
            assert!(
                cells.contains(&(fixtures::Shape::Deep, n)),
                "matrix must include deep at N={n}"
            );
        }

        // Gate cell + 3 sizes x 2 shapes = 7 cells total.
        assert_eq!(cells.len(), 7);
    }

    #[test]
    fn attribution_from_sink_extracts_all_four_fields() {
        let sink = TimingSink::default();
        sink.add_target("nested-glob", Duration::from_millis(100));
        sink.add_link_creation(Duration::from_millis(30));
        sink.add_canonicalize(Duration::from_millis(10));
        sink.add_discovery(Duration::from_millis(20));

        let attribution = Attribution::from_sink(&sink);

        assert_eq!(attribution.canonicalize, Duration::from_millis(10));
        assert_eq!(attribution.discovery, Duration::from_millis(20));
        assert_eq!(attribution.link_creation, Duration::from_millis(30));
        // metadata = target - link_creation - discovery = 100 - 30 - 20 = 50
        assert_eq!(attribution.metadata, Duration::from_millis(50));
    }

    #[test]
    fn deep_fixture_produces_unique_agents_md_relative_paths() {
        // Exercise a size beyond the 8x8 fan-out (64) to ensure the {i:05}
        // level (see fixtures::build) keeps every generated path unique
        // instead of silently colliding and producing fewer files.
        let fixture = fixtures::build(fixtures::Shape::Deep, 200);
        let names = sorted_names(fixtures::Shape::Deep, &fixture);

        assert_eq!(names.len(), 200, "no two deep fixture files may collide");
        let mut deduped = names.clone();
        deduped.dedup();
        assert_eq!(deduped.len(), 200, "all deep fixture paths must be unique");
        assert!(
            names.iter().all(|name| name.ends_with("AGENTS.md")),
            "every deep fixture file must be named AGENTS.md"
        );
    }

    #[test]
    fn run_cell_small_flat_produces_consistent_report() {
        // Lightweight (non-ignored) regression check on the run_cell/median
        // wiring, independent of the heavier #[ignore]d dev_bench_smoke test.
        let report = run_cell(fixtures::Shape::Flat, 2, 2).unwrap();

        assert_eq!(report.shape, "symlink-contents");
        assert_eq!(report.n, 2);
        assert_eq!(report.created, 2);
        assert_eq!(report.errors, 0);
        assert!(report.warm.is_some(), "2 runs must yield a warm median");
        assert_eq!(
            report.discovery,
            Duration::ZERO,
            "flat cells must not record discovery time"
        );
    }

    #[test]
    fn run_cell_single_run_has_no_warm_median() {
        // Boundary case: --runs 1 means only a cold run, so warm must be None
        // (median of an empty slice) rather than panicking or defaulting.
        let report = run_cell(fixtures::Shape::Flat, 2, 1).unwrap();

        assert_eq!(report.created, 2);
        assert_eq!(report.errors, 0);
        assert!(
            report.warm.is_none(),
            "a single run has no warm runs to take the median of"
        );
    }

    #[test]
    fn dev_bench_args_parses_defaults_and_overrides() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(flatten)]
            args: DevBenchArgs,
        }

        let defaults = TestCli::parse_from(["dev-bench"]);
        assert_eq!(defaults.args.runs, 5);
        assert!(!defaults.args.json);

        let overridden = TestCli::parse_from(["dev-bench", "--runs", "3", "--json"]);
        assert_eq!(overridden.args.runs, 3);
        assert!(overridden.args.json);
    }

    #[cfg(unix)]
    #[test]
    fn suppressed_stdout_restores_fd_on_drop() {
        use std::io::Write;

        // Sanity: stdout is writable before suppression.
        writeln!(std::io::stdout(), "before").unwrap();

        {
            let _suppressed = SuppressedStdout::suppress().unwrap();
            // Writes here are redirected to /dev/null; must not error or panic.
            writeln!(std::io::stdout(), "suppressed").unwrap();
        }

        // fd 1 must be restored and usable again after the guard drops.
        writeln!(std::io::stdout(), "after").unwrap();
    }

    #[cfg(debug_assertions)]
    #[test]
    fn run_dev_bench_rejects_debug_builds() {
        // Release-only contract: someone running `cargo run -- dev-bench`
        // must get a clear error instead of silently recording debug timings
        // as release baseline data (the `"profile": "release"` label).
        let err = run_dev_bench(DevBenchArgs {
            runs: 1,
            json: true,
        })
        .unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("release"),
            "debug builds must be rejected with a release-profile message, got: {message}"
        );
    }

    #[test]
    fn ensure_runs_sufficient_rejects_below_two_and_accepts_two() {
        // The statistic needs at least one cold and one warm sample; `--runs 1`
        // (or 0) cannot produce a warm median, so it must be rejected rather
        // than silently clamped with `.max(1)`.
        assert!(
            ensure_runs_sufficient(0).is_err(),
            "--runs 0 must be rejected"
        );
        assert!(
            ensure_runs_sufficient(1).is_err(),
            "--runs 1 must be rejected"
        );
        assert!(
            ensure_runs_sufficient(2).is_ok(),
            "--runs 2 must be accepted"
        );
        assert!(
            ensure_runs_sufficient(5).is_ok(),
            "--runs 5 must be accepted"
        );
        let message = ensure_runs_sufficient(1).unwrap_err().to_string();
        assert!(
            message.contains("at least 2"),
            "rejection must explain the sample minimum, got: {message}"
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    fn reset_managed_destination_clears_dest_but_keeps_sources() {
        let fixture = fixtures::build(fixtures::Shape::Flat, 2);

        // Simulate a completed first run: managed destination exists and the
        // source tree still holds the fixture files.
        fs::create_dir_all(&fixture.dest_root).unwrap();
        fs::write(fixture.dest_root.join("f0000.md"), fixtures::FIXED_CONTENT).unwrap();
        let source_file = fixture.root.path().join(".agents/flat/f0000.md");
        assert!(source_file.exists());

        reset_managed_destination(&fixture).unwrap();

        assert!(
            !fixture.dest_root.exists(),
            "managed destination must be cleared between warm runs"
        );
        assert!(
            source_file.exists(),
            "source fixture tree must survive warm-run resets"
        );
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn run_dev_bench_allows_release_profiles() {
        // Compiles only in release builds: the release-only contract must NOT
        // reject release runs (the smoke test exercises the same profile).
        run_dev_bench(DevBenchArgs {
            runs: 2,
            json: false,
        })
        .unwrap();
    }
}
