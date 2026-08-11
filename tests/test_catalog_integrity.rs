//! E2E test: verify that shipped curated entries are reachable at their fixed commits.
//!
//! Gated behind the `RUN_E2E` environment variable so it never runs in normal
//! CI or local `cargo test` invocations.
//!
//! Run manually:
//! ```bash
//! RUN_E2E=1 cargo test --test test_catalog_integrity -- --nocapture
//! ```
//!
//! **Rate limits:** Unauthenticated GitHub API requests are limited to 60/hour.
//! Set `GITHUB_TOKEN` to use authenticated requests (5,000/hour):
//! ```bash
//! RUN_E2E=1 GITHUB_TOKEN="ghp_..." cargo test --test test_catalog_integrity -- --nocapture
//! ```

use agentsync::skills::registry::load_curated_registry;
use std::time::Duration;

/// Verify that every dallay-owned skill in the catalog resolves to an existing
/// `skills/{name}/SKILL.md` in the `dallay/agents-skills` repository.
#[tokio::test]
async fn catalog_dallay_skill_urls_are_reachable() {
    if std::env::var("RUN_E2E").is_err() {
        eprintln!("Skipping catalog integrity test (set RUN_E2E=1 to enable)");
        return;
    }

    let registry = load_curated_registry(std::path::Path::new("src/skills/registry.v1.toml"))
        .expect("shipped registry should validate");
    assert!(!registry.entries.is_empty());

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("failed to build HTTP client");

    // Optional: use GITHUB_TOKEN for authenticated requests (5,000 req/hr vs 60).
    let github_token = std::env::var("GITHUB_TOKEN").ok();
    if github_token.is_some() {
        eprintln!("Using GITHUB_TOKEN for authenticated API requests");
    } else {
        eprintln!("No GITHUB_TOKEN set — using unauthenticated requests (60/hr limit)");
    }

    let mut failures: Vec<String> = Vec::new();

    for entry in registry.entries.values() {
        let provider_skill_id = &entry.provider_skill_id;
        // Use the GitHub Contents API to check for the SKILL.md file.
        let repository = entry
            .source
            .repository
            .trim_end_matches('/')
            .trim_end_matches(".git");
        let repository = repository
            .strip_prefix("https://github.com/")
            .unwrap_or(repository)
            .trim_end_matches('/');
        let url = format!(
            "https://api.github.com/repos/{repository}/contents/{}/SKILL.md?ref={}",
            entry.source.subpath, entry.source.commit
        );

        let send_request = || {
            let mut req = client
                .get(&url)
                .header("User-Agent", "agentsync-catalog-integrity-test");
            if let Some(ref token) = github_token {
                req = req.header("Authorization", format!("Bearer {}", token));
            }
            req.send()
        };

        let resp = match send_request().await {
            Ok(r) => Ok(r),
            Err(_) => {
                // Retry once after a short delay to avoid flaky CI failures.
                tokio::time::sleep(Duration::from_secs(2)).await;
                send_request().await
            }
        };

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
                failures.push(format!(
                    "{} → network error (after retry): {}",
                    provider_skill_id, e
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Catalog integrity check failed — {} of {} curated skills are unreachable:\n  {}",
            failures.len(),
            registry.entries.len(),
            failures.join("\n  ")
        );
    }

    eprintln!(
        "All {} curated entries are reachable at their pinned commits.",
        registry.entries.len()
    );
}
