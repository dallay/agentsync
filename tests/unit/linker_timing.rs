//! Black-box tests for the `Linker::set_timing` / `TimingSink` benchmark API
//! (see `src/linker/mod.rs::set_timing` and `src/linker/timing.rs`), exercised
//! only through the public `agentsync` crate API — mirrors the conventions in
//! `tests/unit/linker_security.rs`.

use agentsync::config::{AgentConfig, Config, SyncType, TargetConfig};
use agentsync::linker::{Linker, SyncOptions, TimingSink};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::rc::Rc;
use std::time::Duration;
use tempfile::TempDir;

/// Helper to create a target config.
fn make_target(source: &str, destination: &str, sync_type: SyncType) -> TargetConfig {
    TargetConfig {
        source: source.to_string(),
        destination: destination.to_string(),
        sync_type,
        pattern: None,
        exclude: vec![],
        mappings: vec![],
    }
}

/// Helper to create a config with one agent and one target.
fn make_config_with_target(target: TargetConfig) -> Config {
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
        plugins: Default::default(),
    }
}

#[test]
fn sync_without_timing_sink_succeeds_and_records_nothing() {
    // Default (non-bench) usage: no sink is ever installed. This must not
    // change sync behavior in any way.
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let agents_dir = project_root.join(".agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("source.md"), "# Source").unwrap();

    let target = make_target("source.md", "dest.md", SyncType::Symlink);
    let config = make_config_with_target(target);
    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let result = linker.sync(&SyncOptions {
        clean: false,
        dry_run: false,
        verbose: false,
        agents: Some(vec!["test".to_string()]),
    });

    assert!(result.is_ok());
    assert_eq!(result.unwrap().created, 1);
}

#[test]
fn set_timing_records_link_creation_and_canonicalize_for_symlink_contents() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let flat = project_root.join(".agents").join("flat");
    fs::create_dir_all(&flat).unwrap();
    for i in 0..3 {
        fs::write(flat.join(format!("f{i}.md")), "content").unwrap();
    }

    let target = make_target("flat", "links", SyncType::SymlinkContents);
    let config = make_config_with_target(target);
    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let sink = Rc::new(RefCell::new(TimingSink::default()));
    linker.set_timing(Some(Rc::clone(&sink)));

    let result = linker.sync(&SyncOptions::default()).unwrap();
    assert_eq!(result.created, 3);

    let sink_ref = sink.borrow();
    assert_eq!(sink_ref.target_spans().len(), 1);
    assert_eq!(sink_ref.target_spans()[0].sync_type, "symlink-contents");
    assert!(sink_ref.link_creation() > Duration::ZERO);
    assert!(sink_ref.canonicalize() > Duration::ZERO);
    assert_eq!(
        sink_ref.discovery(),
        Duration::ZERO,
        "symlink-contents must not record discovery time"
    );
}

#[test]
fn set_timing_records_discovery_for_nested_glob() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let deep = project_root.join(".agents").join("deep");
    for i in 0..2 {
        let dir = deep.join(format!("child{i}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("AGENTS.md"), "content").unwrap();
    }

    let mut target = make_target(".agents/deep", "docs/{relative_path}", SyncType::NestedGlob);
    target.pattern = Some("**/AGENTS.md".to_string());
    let config = make_config_with_target(target);
    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let sink = Rc::new(RefCell::new(TimingSink::default()));
    linker.set_timing(Some(Rc::clone(&sink)));

    let result = linker.sync(&SyncOptions::default()).unwrap();
    assert_eq!(result.created, 2);

    let sink_ref = sink.borrow();
    assert!(
        sink_ref.discovery() > Duration::ZERO,
        "nested-glob must record discovery (walk) time"
    );
    assert_eq!(sink_ref.target_spans()[0].sync_type, "nested-glob");
}

#[test]
fn timing_sink_reset_clears_stale_values_between_sync_runs() {
    // Regression: reusing the same sink across two `sync()` calls without a
    // `reset()` in between must not silently mix stale values from the first
    // run into the second run's report.
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let flat = project_root.join(".agents").join("flat");
    fs::create_dir_all(&flat).unwrap();
    fs::write(flat.join("a.md"), "content").unwrap();

    let target = make_target("flat", "links", SyncType::SymlinkContents);
    let config = make_config_with_target(target);
    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let sink = Rc::new(RefCell::new(TimingSink::default()));
    linker.set_timing(Some(Rc::clone(&sink)));

    linker.sync(&SyncOptions::default()).unwrap();
    assert_eq!(sink.borrow().target_spans().len(), 1);

    sink.borrow_mut().reset();
    assert!(sink.borrow().target_spans().is_empty());
    assert!(sink.borrow().link_creation().is_zero());

    linker.sync(&SyncOptions::default()).unwrap();
    // A fresh single span from the second run, not two accumulated spans.
    assert_eq!(sink.borrow().target_spans().len(), 1);
}

#[test]
fn set_timing_none_stops_recording_into_a_previously_installed_sink() {
    let temp = TempDir::new().unwrap();
    let project_root = temp.path();

    let flat = project_root.join(".agents").join("flat");
    fs::create_dir_all(&flat).unwrap();
    fs::write(flat.join("a.md"), "content").unwrap();

    let target = make_target("flat", "links", SyncType::SymlinkContents);
    let config = make_config_with_target(target);
    let config_path = project_root.join("agentsync.toml");
    let linker = Linker::new(config, config_path);

    let sink = Rc::new(RefCell::new(TimingSink::default()));
    linker.set_timing(Some(Rc::clone(&sink)));
    linker.sync(&SyncOptions::default()).unwrap();
    assert_eq!(sink.borrow().target_spans().len(), 1);

    // Detach the sink; further syncs must not update it.
    linker.set_timing(None);
    sink.borrow_mut().reset();

    let result = linker.sync(&SyncOptions::default());
    assert!(
        result.is_ok(),
        "sync must still succeed with no sink installed"
    );
    assert!(
        sink.borrow().target_spans().is_empty(),
        "a detached sink must not receive further updates"
    );
}
