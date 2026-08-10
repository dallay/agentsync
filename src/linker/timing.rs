//! Wall-clock timing sink for the developer-only benchmark harness.
//!
//! [`TimingSink`] records external `Instant::now` spans recorded around the
//! four benchmarked phases of the sync engine:
//!
//! * **target** — the whole [`Linker::process_target`](super::Linker) call,
//!   attributed per sync type.
//! * **link creation** — every [`create_symlink`](super::Linker) span
//!   (including the canonicalize work it performs).
//! * **canonicalize** — every [`relative_path`](super::Linker) span (a subset
//!   of link creation).
//! * **discovery** — every [`get_nested_glob_matches`](super::Linker) walk.
//!
//! Metadata time is deliberately NOT recorded directly: per the change design
//! it is derived as `target − link creation − discovery` so attribution never
//! double counts. See `metadata()`.
//!
//! A sink is only active on a [`Linker`] when `set_timing` was called by the
//! bench harness; normal runs keep the field `None` and pay no `Instant::now`
//! cost (the span guards short-circuit on a single `RefCell` borrow).

use crate::config::SyncType;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::Linker;

/// A single `process_target` span attributed to a sync type.
#[derive(Debug, Clone, Copy)]
pub struct TargetSpan {
    /// Stable sync-type name (e.g. `symlink-contents`, `nested-glob`).
    pub sync_type: &'static str,
    /// Wall-clock time the target processing took.
    pub elapsed: Duration,
}

/// Accumulates wall-clock spans recorded by the guarded sync engine.
#[derive(Debug, Default)]
pub struct TimingSink {
    targets: RefCell<Vec<TargetSpan>>,
    link_creation: RefCell<Duration>,
    canonicalize: RefCell<Duration>,
    discovery: RefCell<Duration>,
}

impl TimingSink {
    /// Clear every recorded span; call before each timed run.
    pub fn reset(&self) {
        self.targets.borrow_mut().clear();
        *self.link_creation.borrow_mut() = Duration::ZERO;
        *self.canonicalize.borrow_mut() = Duration::ZERO;
        *self.discovery.borrow_mut() = Duration::ZERO;
    }

    /// Record one `process_target` span for a sync type.
    pub fn add_target(&self, sync_type: &'static str, elapsed: Duration) {
        self.targets
            .borrow_mut()
            .push(TargetSpan { sync_type, elapsed });
    }

    /// Accumulate one `create_symlink` span.
    pub fn add_link_creation(&self, elapsed: Duration) {
        *self.link_creation.borrow_mut() += elapsed;
    }

    /// Accumulate one `relative_path` span (canonicalize work).
    pub fn add_canonicalize(&self, elapsed: Duration) {
        *self.canonicalize.borrow_mut() += elapsed;
    }

    /// Accumulate one `get_nested_glob_matches` walk span.
    pub fn add_discovery(&self, elapsed: Duration) {
        *self.discovery.borrow_mut() += elapsed;
    }

    /// All recorded `process_target` spans.
    pub fn target_spans(&self) -> Vec<TargetSpan> {
        self.targets.borrow().clone()
    }

    /// Total `process_target` time across all recorded spans.
    pub fn target_total(&self) -> Duration {
        self.targets.borrow().iter().map(|span| span.elapsed).sum()
    }

    /// Total `create_symlink` time.
    pub fn link_creation(&self) -> Duration {
        *self.link_creation.borrow()
    }

    /// Total `relative_path` (canonicalize) time.
    pub fn canonicalize(&self) -> Duration {
        *self.canonicalize.borrow()
    }

    /// Total `get_nested_glob_matches` walk time.
    pub fn discovery(&self) -> Duration {
        *self.discovery.borrow()
    }

    /// Derived metadata time: target spans minus link creation minus discovery.
    ///
    /// This is the design's no-double-counting attribution: discovery and link
    /// creation are nested inside the target span, and canonicalize is a subset
    /// of link creation, so `metadata = target − links − discovery` accounts
    /// for every benchmarked phase exactly once.
    pub fn metadata(&self) -> Duration {
        self.target_total()
            .saturating_sub(self.link_creation())
            .saturating_sub(self.discovery())
    }
}

/// Stable sync-type name used for target-span attribution.
pub(crate) fn sync_type_name(sync_type: SyncType) -> &'static str {
    match sync_type {
        SyncType::Symlink => "symlink",
        SyncType::SymlinkContents => "symlink-contents",
        SyncType::NestedGlob => "nested-glob",
        SyncType::ModuleMap => "module-map",
    }
}

/// Kind of span being timed; maps to the matching `TimingSink` accumulator.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SpanKind {
    /// `process_target` span, attributed per sync type.
    Target(&'static str),
    /// `create_symlink` span.
    LinkCreation,
    /// `relative_path` span (canonicalize work).
    Canonicalize,
    /// `get_nested_glob_matches` walk span.
    Discovery,
}

/// RAII guard recording one timed span into the sink when dropped.
///
/// Only ever created when a sink is installed (bench mode); normal runs keep
/// `timing_span` returning `None` and never pay the `Instant::now` cost.
#[derive(Debug)]
pub(crate) struct SpanGuard {
    sink: Rc<RefCell<TimingSink>>,
    kind: SpanKind,
    start: Instant,
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        let sink = self.sink.borrow_mut();
        match self.kind {
            SpanKind::Target(sync_type) => sink.add_target(sync_type, elapsed),
            SpanKind::LinkCreation => sink.add_link_creation(elapsed),
            SpanKind::Canonicalize => sink.add_canonicalize(elapsed),
            SpanKind::Discovery => sink.add_discovery(elapsed),
        }
    }
}

impl Linker {
    /// Start a guarded timing span; returns `None` (zero overhead) when no
    /// sink is installed — normal runs never install one.
    pub(super) fn timing_span(&self, kind: SpanKind) -> Option<SpanGuard> {
        let sink = self.timing.borrow().clone()?;
        Some(SpanGuard {
            sink,
            kind,
            start: Instant::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AgentConfig, Config, SyncType, TargetConfig};
    use crate::linker::{Linker, SyncOptions};
    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::TempDir;

    fn make_config_with_contents_target() -> Config {
        let mut targets = BTreeMap::new();
        targets.insert(
            "target".to_string(),
            TargetConfig {
                source: "flat".to_string(),
                destination: "links".to_string(),
                sync_type: SyncType::SymlinkContents,
                pattern: None,
                exclude: vec![],
                mappings: vec![],
            },
        );

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

    #[test]
    fn timing_sink_reset_clears_all_spans() {
        let sink = TimingSink::default();
        sink.add_target("symlink-contents", Duration::from_millis(100));
        sink.add_link_creation(Duration::from_millis(60));
        sink.add_canonicalize(Duration::from_millis(40));
        sink.add_discovery(Duration::from_millis(5));

        assert_eq!(sink.target_spans().len(), 1);
        assert!(sink.link_creation() > Duration::ZERO);

        sink.reset();

        assert!(sink.target_spans().is_empty());
        assert!(sink.link_creation().is_zero());
        assert!(sink.canonicalize().is_zero());
        assert!(sink.discovery().is_zero());
        assert!(sink.metadata().is_zero());
    }

    #[test]
    fn timing_sink_derives_metadata_without_double_counting() {
        let sink = TimingSink::default();
        sink.add_target("symlink-contents", Duration::from_millis(100));
        sink.add_link_creation(Duration::from_millis(60));
        sink.add_canonicalize(Duration::from_millis(40));
        // discovery is zero for a flat (non-glob) cell.
        assert_eq!(sink.metadata(), Duration::from_millis(40));
        assert_eq!(sink.metadata() + sink.link_creation(), sink.target_total());
    }

    #[test]
    fn timing_sink_metadata_subtracts_discovery_for_glob_cells() {
        let sink = TimingSink::default();
        sink.add_target("nested-glob", Duration::from_millis(200));
        sink.add_discovery(Duration::from_millis(80));
        sink.add_link_creation(Duration::from_millis(70));
        assert_eq!(sink.metadata(), Duration::from_millis(50));
    }

    #[test]
    fn linker_sync_records_timing_spans() {
        // TDD: this test drives the `Linker::set_timing` wiring and the
        // guarded spans at process_target / create_symlink / relative_path.
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let flat = root.join(".agents").join("flat");
        fs::create_dir_all(&flat).unwrap();
        for i in 0..4 {
            fs::write(flat.join(format!("f{i:04}.md")), "FIXED\n").unwrap();
        }

        let config_path = root.join("agentsync.toml");
        fs::write(&config_path, "").unwrap();
        let linker = Linker::new(make_config_with_contents_target(), config_path);

        let sink = Rc::new(RefCell::new(TimingSink::default()));
        linker.set_timing(Some(Rc::clone(&sink)));

        let result = linker.sync(&SyncOptions::default()).unwrap();
        assert_eq!(result.created, 4);

        let sink = sink.borrow();
        assert_eq!(sink.target_spans().len(), 1);
        assert_eq!(sink.target_spans()[0].sync_type, "symlink-contents");
        assert!(sink.target_total() > Duration::ZERO);
        assert!(sink.link_creation() > Duration::ZERO);
        assert!(sink.canonicalize() > Duration::ZERO);
        assert!(sink.discovery().is_zero(), "flat sync must not walk globs");
        // Exact arithmetic derivation: metadata + links == target.
        assert_eq!(sink.metadata() + sink.link_creation(), sink.target_total());
    }
}
