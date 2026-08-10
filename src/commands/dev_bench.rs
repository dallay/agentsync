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
//! * cold = run 1, warm = median of runs 2..=R (`--runs`, default 5)
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

        Fixture {
            root,
            config,
            config_path,
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
fn run_cell(shape: fixtures::Shape, n: usize, runs: usize) -> Result<CellReport> {
    let mut cold = None;
    let mut warm_runs: Vec<Duration> = Vec::with_capacity(runs.saturating_sub(1));
    let mut attribution = None;
    let mut created = 0usize;
    let mut errors = 0usize;

    for run in 1..=runs {
        let fixture = fixtures::build(shape, n);
        let sink = Rc::new(RefCell::new(TimingSink::default()));
        let linker = Linker::new(fixture.config, fixture.config_path);
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

/// Run the hidden linker benchmark and emit the report.
pub(crate) fn run_dev_bench(args: DevBenchArgs) -> Result<()> {
    let runs = args.runs.max(1);

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
}
