//! E2E test: verify that every `dallay/agents-skills/*` entry in the embedded
//! catalog points to a skill directory that actually exists in the remote
//! `dallay/agents-skills` GitHub repository.
//!
//! Gated behind the `RUN_E2E` environment variable so it never runs in normal
//! CI or local `cargo test` invocations.
//!
//! Run manually:
//! ```bash
//! RUN_E2E=1 cargo test --test test_catalog_integrity -- --nocapture
//! ```

use agentsync::skills::catalog::EmbeddedSkillCatalog;

const DALLAY_SKILLS_PREFIX: &str = "dallay/agents-skills/";

/// Verify that every dallay-owned skill in the catalog resolves to an existing
/// `skills/{name}/SKILL.md` in the `dallay/agents-skills` repository.
#[test]
#[ignore]
fn catalog_dallay_skill_urls_are_reachable() {
    if std::env::var("RUN_E2E").is_err() {
        eprintln!("Skipping catalog integrity test (set RUN_E2E=1 to enable)");
        return;
    }

    let catalog = EmbeddedSkillCatalog::default();

    // Collect all dallay/agents-skills/* provider_skill_ids from the catalog.
    let dallay_skills: Vec<&str> = catalog
        .skill_definitions()
        .filter(|s| s.provider_skill_id.starts_with(DALLAY_SKILLS_PREFIX))
        .map(|s| s.provider_skill_id.as_str())
        .collect();

    assert!(
        !dallay_skills.is_empty(),
        "Catalog should contain at least one dallay/agents-skills/* entry"
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("failed to build HTTP client");

    let mut failures: Vec<String> = Vec::new();

    for provider_skill_id in &dallay_skills {
        let skill_name = provider_skill_id
            .strip_prefix(DALLAY_SKILLS_PREFIX)
            .unwrap();

        // Use the GitHub Contents API to check for the skill directory.
        let url = format!(
            "https://api.github.com/repos/dallay/agents-skills/contents/skills/{}/SKILL.md",
            skill_name
        );

        let resp = client
            .get(&url)
            .header("User-Agent", "agentsync-catalog-integrity-test")
            .send();

        match resp {
            Ok(r) if r.status().is_success() => {
                eprintln!("  OK: {}", provider_skill_id);
            }
            Ok(r) => {
                failures.push(format!(
                    "{} → HTTP {} at {}",
                    provider_skill_id,
                    r.status(),
                    url
                ));
            }
            Err(e) => {
                failures.push(format!("{} → network error: {}", provider_skill_id, e));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Catalog integrity check failed — {} of {} dallay skills are unreachable:\n  {}",
            failures.len(),
            dallay_skills.len(),
            failures.join("\n  ")
        );
    }

    eprintln!(
        "All {} dallay/agents-skills/* entries are reachable.",
        dallay_skills.len()
    );
}
