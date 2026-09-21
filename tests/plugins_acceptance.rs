use agentsync::Config;
use agentsync::plugins::{
    LockedSource, LockedSourceKind, PluginApplyMode, PluginApplyResult, PluginLock, PluginManager,
    PluginMcpApproval, PluginSelection,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::{TempDir, tempdir};

type FetchLog = Arc<Mutex<Vec<LockedSource>>>;
type AcceptanceSetup = (
    TempDir,
    PathBuf,
    PluginManager,
    PluginSelection,
    Vec<u8>,
    Vec<u8>,
    FetchLog,
);

const LOCKED_REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
const LOCKED_REPOSITORY: &str = "https://github.com/dallay/acceptance-marketplace";

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugin-marketplace")
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &target_path);
        } else {
            fs::create_dir_all(target_path.parent().unwrap()).unwrap();
            fs::copy(source_path, target_path).unwrap();
        }
    }
}

fn write_project_config(project: &Path) -> PathBuf {
    let agents = project.join(".agents");
    fs::create_dir_all(&agents).unwrap();
    let config_path = agents.join("agentsync.toml");
    fs::write(
        &config_path,
        r#"
source_dir = ".agents"

[plugins]
enabled = true
lockfile = "plugins.lock.toml"

[plugins.marketplaces.internal]
source = "../marketplace"
reference = "main"

[[plugins.selections]]
marketplace = "internal"
plugin = "engineering"
"#,
    )
    .unwrap();
    config_path
}

fn selection() -> PluginSelection {
    PluginSelection {
        marketplace: "internal".to_string(),
        plugin: "engineering".to_string(),
    }
}

fn setup_locked_git_project() -> AcceptanceSetup {
    let project = tempdir().unwrap();
    copy_tree(&fixture_root(), &project.path().join("marketplace"));
    let config_path = write_project_config(project.path());
    let config = Config::load(&config_path).unwrap();
    let initial_manager = PluginManager::new(
        Config::project_root(&config_path),
        config_path.clone(),
        config.plugins,
    );
    initial_manager.add(&selection()).unwrap();

    let mut config_body = fs::read_to_string(&config_path).unwrap();
    config_body = config_body.replace(
        "source = \"../marketplace\"",
        "source = \"https://github.com/configured/different-repository\"",
    );
    fs::write(&config_path, config_body).unwrap();

    let lock_path = config_path.parent().unwrap().join("plugins.lock.toml");
    let mut lock = PluginLock::load(&lock_path).unwrap();
    let locked = lock.plugins.get_mut("internal/engineering").unwrap();
    locked.source = LockedSource {
        kind: LockedSourceKind::Git,
        location: LOCKED_REPOSITORY.to_string(),
        revision: LOCKED_REVISION.to_string(),
    };
    locked.provenance.resolved_revision = LOCKED_REVISION.to_string();
    lock.save_atomic(&lock_path).unwrap();

    fs::remove_dir_all(project.path().join(".agents/skills")).unwrap();
    let config_before = fs::read(&config_path).unwrap();
    let lock_before = fs::read(&lock_path).unwrap();
    let config = Config::load(&config_path).unwrap();
    let fetched_sources = Arc::new(Mutex::new(Vec::new()));
    let fetcher_sources = fetched_sources.clone();
    let manager = PluginManager::new(
        Config::project_root(&config_path),
        config_path.clone(),
        config.plugins,
    )
    .with_git_snapshot_fetcher(move |source| {
        fetcher_sources.lock().unwrap().push(source.clone());
        let snapshot = TempDir::new()?;
        copy_tree(&fixture_root(), snapshot.path());
        Ok(snapshot)
    });

    (
        project,
        config_path,
        manager,
        selection(),
        config_before,
        lock_before,
        fetched_sources,
    )
}

fn assert_clean_clone_result(result: PluginApplyResult) {
    assert!(result.created > 0 || result.updated > 0);
    assert_eq!(result.errors, 0);
}

#[test]
fn clean_clone_restores_exact_locked_git_snapshot_then_runs_offline() {
    let (project, config_path, manager, selection, config_before, lock_before, fetched_sources) =
        setup_locked_git_project();
    let cache_root = project.path().join(".agents/.agentsync-plugin-sources");

    let online = manager
        .apply_with(PluginApplyMode {
            dry_run: false,
            offline: false,
        })
        .unwrap();
    assert_clean_clone_result(online);
    assert!(
        project
            .path()
            .join(".agents/skills/review/SKILL.md")
            .is_file()
    );
    assert_eq!(fs::read(&config_path).unwrap(), config_before);
    assert_eq!(
        fs::read(config_path.parent().unwrap().join("plugins.lock.toml")).unwrap(),
        lock_before
    );

    let fetched = fetched_sources.lock().unwrap();
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].location, LOCKED_REPOSITORY);
    assert_eq!(fetched[0].revision, LOCKED_REVISION);
    drop(fetched);

    fs::remove_dir_all(&cache_root).unwrap();
    fs::remove_dir_all(project.path().join(".agents/skills")).unwrap();
    let offline_error = manager
        .apply_with(PluginApplyMode {
            dry_run: false,
            offline: true,
        })
        .expect_err("offline apply must reject a missing Git snapshot");
    let message = offline_error.to_string();
    assert!(message.contains("plugin restore"), "{message}");
    assert!(!message.contains("plugin update"), "{message}");
    assert!(
        !project
            .path()
            .join(".agents/skills/review/SKILL.md")
            .exists()
    );

    let restore = manager.restore(Some(&selection)).unwrap();
    assert_eq!(restore.created, 1);
    let offline = manager
        .apply_with(PluginApplyMode {
            dry_run: false,
            offline: true,
        })
        .unwrap();
    assert_clean_clone_result(offline);
    assert!(
        project
            .path()
            .join(".agents/skills/review/SKILL.md")
            .is_file()
    );
}

#[test]
fn acceptance_status_reports_raw_pending_mcp_and_remove_revokes_plugin() {
    let (project, _config_path, manager, selection, _, _, _) = setup_locked_git_project();
    manager.restore(Some(&selection)).unwrap();

    let report = manager.status_report().unwrap();
    assert_eq!(report.skills, 1);
    assert_eq!(report.servers.len(), 1);
    assert!(matches!(
        report.servers[0].approval,
        PluginMcpApproval::Pending
    ));
    assert_eq!(
        report.servers[0].server.command.as_deref(),
        Some("/bin/false")
    );

    manager.restore(Some(&selection)).unwrap();
    manager
        .apply_with(PluginApplyMode {
            dry_run: false,
            offline: true,
        })
        .unwrap();
    assert!(
        project
            .path()
            .join(".agents/skills/review/SKILL.md")
            .is_file()
    );

    manager.remove(&selection, false).unwrap();
    assert!(
        !project
            .path()
            .join(".agents/skills/review/SKILL.md")
            .exists()
    );
    let config = fs::read_to_string(project.path().join(".agents/agentsync.toml")).unwrap();
    assert!(!config.contains("[[plugins.selections]]"));
    assert!(config.contains("[plugins.marketplaces.internal]"));
}
