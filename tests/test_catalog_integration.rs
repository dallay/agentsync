//! End-to-end catalog installation verification.
//!
//! Deterministic offline integration coverage for catalog and curated registry resolution.

use agentsync::skills::catalog::EmbeddedSkillCatalog;
use agentsync::skills::install::{blocking_fetch_and_install_skill, install_from_dir};
use agentsync::skills::provider::{SkillsShProvider, resolve_catalog_install_source};
use agentsync::skills::registry::read_registry;
use std::path::Path;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// Serialises tests that mutate `AGENTSYNC_LOCAL_SKILLS_REPO` so they cannot race the focused
/// Phase 1 test (which silently inherits whatever env var the surrounding `cargo test`
/// invocation set). The same mutex pattern lives in `tests/unit/provider.rs`.
static LOCAL_SKILLS_REPO_ENV_LOCK: Mutex<()> = Mutex::new(());

/// RAII guard that snapshots the source override variables on construction and restores them on
/// drop. Reuses the precedence contract documented in `src/skills/provider.rs:191-218`.
const SOURCE_OVERRIDE_ENV_VARS: [&str; 2] = [
    "AGENTSYNC_LOCAL_SKILLS_REPO",
    "AGENTSYNC_TEST_SKILL_SOURCE_DIR",
];

struct LocalSkillsRepoEnvGuard {
    previous: [(&'static str, Option<std::ffi::OsString>); 2],
}

impl LocalSkillsRepoEnvGuard {
    fn new() -> Self {
        let previous = SOURCE_OVERRIDE_ENV_VARS.map(|name| (name, std::env::var_os(name)));
        for name in SOURCE_OVERRIDE_ENV_VARS {
            unsafe { std::env::remove_var(name) };
        }
        Self { previous }
    }
}

impl Drop for LocalSkillsRepoEnvGuard {
    fn drop(&mut self) {
        for (name, value) in &mut self.previous {
            match value.take() {
                Some(value) => unsafe { std::env::set_var(*name, value) },
                None => unsafe { std::env::remove_var(*name) },
            }
        }
    }
}

fn project_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn resolve_install_source(
    provider: &SkillsShProvider,
    provider_skill_id: &str,
    local_skill_id: &str,
) -> anyhow::Result<String> {
    let catalog = EmbeddedSkillCatalog::default();
    resolve_catalog_install_source(
        &catalog,
        provider,
        provider_skill_id,
        local_skill_id,
        Some(project_root()),
    )
}

fn install_with_retry(skill_id: &str, source: &str, target_root: &Path) -> anyhow::Result<()> {
    match blocking_fetch_and_install_skill(skill_id, source, target_root) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            eprintln!(
                "Initial install attempt failed for {skill_id} from {source}: {first_error}. Retrying once..."
            );
            thread::sleep(Duration::from_secs(2));
            blocking_fetch_and_install_skill(skill_id, source, target_root).map_err(
                |second_error| {
                    anyhow::anyhow!("first attempt: {first_error}; retry: {second_error}")
                },
            )
        }
    }
}

#[test]
fn offline_catalog_e2e_is_reproducible() {
    let source = Path::new("tests/fixtures/curated-skills/valid-skill");
    let outcomes = (0..2)
        .map(|_| {
            let temp = TempDir::new().unwrap();
            let target = temp.path().join("skills");
            std::fs::create_dir_all(&target).unwrap();
            install_from_dir("valid-skill", source, &target).unwrap();
            std::fs::read(target.join("valid-skill/SKILL.md")).unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(outcomes[0], outcomes[1]);
}

#[test]
fn phase1_bobmatnyc_catalog_entries_install_offline_and_register_local_ids() {
    let _lock = LOCAL_SKILLS_REPO_ENV_LOCK.lock().unwrap();
    let catalog = EmbeddedSkillCatalog::default();
    let provider = SkillsShProvider;
    let expected = [
        (
            "dallay/agents-skills/drizzle-orm",
            "drizzle-orm",
            Vec::<&str>::from([
                "references/advanced-schemas.md",
                "references/performance.md",
                "references/query-patterns.md",
                "references/vs-prisma.md",
            ]),
        ),
        (
            "dallay/agents-skills/pydantic",
            "pydantic",
            Vec::<&str>::from(["references/full-source.md"]),
        ),
        (
            "dallay/agents-skills/sqlalchemy",
            "sqlalchemy",
            Vec::<&str>::from(["references/sql-quality-antipatterns.md"]),
        ),
    ];

    for (provider_skill_id, local_skill_id, companions) in expected {
        let definition = catalog
            .get_skill_definition(provider_skill_id)
            .expect("focused test must cover every approved definition");
        let temp = TempDir::new().unwrap();
        let target_root = temp.path().join(".agents/skills");
        std::fs::create_dir_all(&target_root).unwrap();
        let source = resolve_catalog_install_source(
            &catalog,
            &provider,
            &definition.provider_skill_id,
            &definition.local_skill_id,
            Some(project_root()),
        )
        .unwrap();

        assert!(
            Path::new(&source).is_dir(),
            "{local_skill_id} resolved online"
        );
        install_from_dir(local_skill_id, Path::new(&source), &target_root).unwrap();

        let skill_dir = target_root.join(local_skill_id);
        assert!(skill_dir.join("SKILL.md").is_file());
        for companion in companions {
            assert!(
                skill_dir.join(companion).is_file(),
                "{local_skill_id} is missing companion {companion}"
            );
        }

        let registry = read_registry(&target_root.join("registry.json")).unwrap();
        assert!(
            registry
                .skills
                .unwrap_or_default()
                .contains_key(local_skill_id)
        );
    }
}

/// Regression for REQ-SKILLREC-001 / REQ-SKILLREC-002 — codifies the contract that the
/// resolver MUST honour `AGENTSYNC_LOCAL_SKILLS_REPO` when set and MUST fall back to the
/// sibling `<project_root_parent>/agents-skills` checkout when the env var is unset. The
/// CI workflow in `.github/workflows/catalog-e2e.yml` is the canonical place where the env
/// var is set today; the sibling fallback is what a developer workstation relies on when
/// running this test outside CI.
#[test]
fn phase1_bobmatnyc_catalog_resolver_uses_env_var_or_sibling_fallback() {
    let _lock = LOCAL_SKILLS_REPO_ENV_LOCK.lock().unwrap();
    let _env_guard = LocalSkillsRepoEnvGuard::new();

    let catalog = EmbeddedSkillCatalog::default();
    let provider = SkillsShProvider;
    let phase1_ids: [(&str, &str); 3] = [
        ("dallay/agents-skills/drizzle-orm", "drizzle-orm"),
        ("dallay/agents-skills/pydantic", "pydantic"),
        ("dallay/agents-skills/sqlalchemy", "sqlalchemy"),
    ];

    // ---- Case A: AGENTSYNC_LOCAL_SKILLS_REPO unset, project_root has the sibling. ----
    // Build an isolated sibling checkout so this assertion does not depend on the caller's
    // filesystem layout. This is the contract that a developer workstation relies on when
    // `cargo test` is run without CI env vars.
    let sibling_parent = TempDir::new().unwrap();
    let sibling_project_root = sibling_parent.path().join("agentsync");
    let sibling_skills_root = sibling_parent.path().join("agents-skills").join("skills");
    for (_, local_skill_id) in phase1_ids {
        let skill_dir = sibling_skills_root.join(local_skill_id);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# sibling fallback fixture\n").unwrap();
    }

    for (provider_skill_id, local_skill_id) in phase1_ids {
        let resolved = resolve_catalog_install_source(
            &catalog,
            &provider,
            provider_skill_id,
            local_skill_id,
            Some(&sibling_project_root),
        )
        .unwrap_or_else(|err| {
            panic!(
                "sibling fallback must resolve {local_skill_id} when AGENTSYNC_LOCAL_SKILLS_REPO is unset: {err}"
            )
        });
        let resolved_path = Path::new(&resolved);
        assert!(
            resolved_path.is_dir(),
            "{local_skill_id}: sibling fallback returned non-directory {resolved}"
        );
        let canonical =
            std::fs::canonicalize(resolved_path).unwrap_or_else(|_| resolved_path.to_path_buf());
        let canonical_str = canonical.to_string_lossy();
        assert!(
            canonical_str.contains("agents-skills"),
            "{local_skill_id}: sibling fallback did not resolve under an agents-skills \
             directory (got {canonical:?})"
        );
        assert!(
            canonical.ends_with(format!("skills/{local_skill_id}").as_str()),
            "{local_skill_id}: sibling fallback returned {canonical:?}, expected .../skills/{local_skill_id}"
        );
    }

    // ---- Case B: AGENTSYNC_LOCAL_SKILLS_REPO set, project_root has NO sibling. ----
    // The resolver MUST use the env var path. We point project_root at an isolated temp
    // directory so the sibling fallback cannot accidentally satisfy the assertion.
    let temp_root = TempDir::new().unwrap();
    let skills_root = temp_root.path().join("skills");
    for (_, local_skill_id) in phase1_ids {
        let skill_dir = skills_root.join(local_skill_id);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "# regression fixture — not a real skill\n",
        )
        .unwrap();
    }
    let isolated_root = TempDir::new().unwrap();
    unsafe { std::env::set_var("AGENTSYNC_LOCAL_SKILLS_REPO", temp_root.path()) };

    for (provider_skill_id, local_skill_id) in phase1_ids {
        let resolved = resolve_catalog_install_source(
            &catalog,
            &provider,
            provider_skill_id,
            local_skill_id,
            Some(isolated_root.path()),
        )
        .unwrap_or_else(|err| {
            panic!("AGENTSYNC_LOCAL_SKILLS_REPO must drive resolution for {local_skill_id}: {err}")
        });
        let expected = temp_root.path().join("skills").join(local_skill_id);
        let resolved_path = Path::new(&resolved);
        assert_eq!(
            resolved_path, expected,
            "{local_skill_id}: resolver must return exactly the env var path \
             (expected {expected:?}, got {resolved_path:?})"
        );
        assert!(
            resolved_path.starts_with(temp_root.path()),
            "{local_skill_id}: resolver must not silently fall back to the sibling \
             checkout when AGENTSYNC_LOCAL_SKILLS_REPO is set (got {resolved_path:?})"
        );
    }
}

#[test]
#[ignore]
fn every_catalog_skill_installs_successfully() {
    if std::env::var("RUN_E2E").is_err() {
        eprintln!("Skipping catalog installation test (set RUN_E2E=1 to enable)");
        return;
    }

    let catalog = EmbeddedSkillCatalog::default();
    let provider = SkillsShProvider;
    let mut failures = Vec::new();

    for definition in catalog.skill_definitions() {
        let temp = TempDir::new().expect("temp dir should be created");
        let target_root = temp.path().join(".agents").join("skills");
        std::fs::create_dir_all(&target_root).expect("target root should be created");

        let source = match resolve_install_source(
            &provider,
            &definition.provider_skill_id,
            &definition.local_skill_id,
        ) {
            Ok(source) => source,
            Err(error) => {
                failures.push(format!(
                    "{} [{}] failed to resolve source: {}",
                    definition.local_skill_id, definition.provider_skill_id, error
                ));
                continue;
            }
        };

        if let Err(error) = install_with_retry(&definition.local_skill_id, &source, &target_root) {
            failures.push(format!(
                "{} [{}] failed to install from {}: {}",
                definition.local_skill_id, definition.provider_skill_id, source, error
            ));
            continue;
        }

        let skill_dir = target_root.join(&definition.local_skill_id);
        let manifest_path = skill_dir.join("SKILL.md");
        if !manifest_path.exists() {
            failures.push(format!(
                "{} [{}] installed without SKILL.md at {}",
                definition.local_skill_id,
                definition.provider_skill_id,
                manifest_path.display()
            ));
            continue;
        }

        let registry_path = target_root.join("registry.json");
        match read_registry(&registry_path) {
            Ok(registry) => {
                let has_entry = registry
                    .skills
                    .unwrap_or_default()
                    .contains_key(&definition.local_skill_id);
                if !has_entry {
                    failures.push(format!(
                        "{} [{}] installed but registry.json is missing its canonical key",
                        definition.local_skill_id, definition.provider_skill_id
                    ));
                }
            }
            Err(error) => failures.push(format!(
                "{} [{}] installed but registry.json could not be read: {}",
                definition.local_skill_id, definition.provider_skill_id, error
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "{} catalog skills failed installation validation:\n- {}",
        failures.len(),
        failures.join("\n- ")
    );
}
