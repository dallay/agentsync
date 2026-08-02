use agentsync::skills::provider::{Provider, SkillInstallInfo, SkillsShProvider};
use agentsync::skills::registry::load_curated_registry;

struct DummyProvider;

impl Provider for DummyProvider {
    fn manifest(&self) -> anyhow::Result<String> {
        Ok("{\"id\": \"dummy\"}".to_string())
    }

    fn resolve(&self, id: &str) -> anyhow::Result<SkillInstallInfo> {
        Ok(SkillInstallInfo {
            download_url: format!("https://example.org/{}/download.zip", id),
            format: "zip".to_string(),
        })
    }
}

#[test]
fn dummy_provider_resolves() {
    let p = DummyProvider;
    let m = p.manifest().unwrap();
    assert!(m.contains("dummy"));

    let info = p.resolve("sample").unwrap();
    assert!(info.download_url.contains("sample"));
    assert_eq!(info.format, "zip");
}

// ---------------------------------------------------------------------------
// SkillsShProvider deterministic resolve tests
// ---------------------------------------------------------------------------

#[test]
fn resolve_deterministic_owner_repo_skill_format() {
    let provider = SkillsShProvider;
    let info = provider
        .resolve("vercel-labs/agent-skills/vercel-react-best-practices")
        .unwrap();

    assert_eq!(
        info.download_url,
        "https://github.com/vercel-labs/agent-skills/archive/HEAD.zip#skills/vercel-react-best-practices"
    );
    assert_eq!(info.format, "zip");
}

#[test]
fn resolve_deterministic_non_skills_repo_omits_skills_prefix() {
    let provider = SkillsShProvider;
    let info = provider.resolve("acme/my-repo/my-skill-name").unwrap();

    // "my-repo" is NOT in SKILLS_REPO_NAMES, so no "skills/" prefix
    assert_eq!(
        info.download_url,
        "https://github.com/acme/my-repo/archive/HEAD.zip#my-skill-name"
    );
    assert_eq!(info.format, "zip");
}

#[test]
fn resolve_deterministic_repo_suffix_skills_adds_skills_prefix() {
    let provider = SkillsShProvider;

    let info = provider
        .resolve("krutikJain/android-agent-skills/android-kotlin-core")
        .unwrap();

    assert_eq!(
        info.download_url,
        "https://github.com/krutikJain/android-agent-skills/archive/HEAD.zip#skills/android-kotlin-core"
    );
    assert_eq!(info.format, "zip");
}

#[test]
fn resolve_deterministic_embedded_catalog_can_omit_fragment_for_repo_root_skill() {
    let provider = SkillsShProvider;

    let info = provider
        .resolve("currents-dev/playwright-best-practices-skill/playwright-best-practices")
        .unwrap();

    assert_eq!(
        info.download_url,
        "https://github.com/currents-dev/playwright-best-practices-skill/archive/HEAD.zip"
    );
    assert_eq!(info.format, "zip");
}

#[test]
fn resolve_deterministic_skills_repo_adds_skills_prefix() {
    let provider = SkillsShProvider;

    // "skills" is in SKILLS_REPO_NAMES
    let info = provider
        .resolve("cloudflare/skills/durable-objects")
        .unwrap();
    assert_eq!(
        info.download_url,
        "https://github.com/cloudflare/skills/archive/HEAD.zip#skills/durable-objects"
    );

    // "agent-skills" is also in SKILLS_REPO_NAMES
    let info = provider
        .resolve("krutikJain/agent-skills/android-kotlin-core")
        .unwrap();
    assert_eq!(
        info.download_url,
        "https://github.com/krutikJain/agent-skills/archive/HEAD.zip#skills/android-kotlin-core"
    );

    // "agents-skills" is also in SKILLS_REPO_NAMES (dallay monorepo)
    let info = provider
        .resolve("dallay/agents-skills/docker-expert")
        .unwrap();
    assert_eq!(
        info.download_url,
        "https://github.com/dallay/agents-skills/archive/HEAD.zip#skills/docker-expert"
    );
}

#[test]
fn resolve_deterministic_rejects_invalid_ids() {
    let provider = SkillsShProvider;

    // Missing repo component (only 1 slash but we need 2+ for deterministic)
    // This would fall through to search API, so we just test that owner/repo/skill works
    // and that empty components are rejected
    let result = provider.resolve("//empty-parts");
    assert!(result.is_err(), "should reject empty owner component");

    let result = provider.resolve("owner//empty-skill");
    assert!(result.is_err(), "should reject empty repo component");
}

/// Skill IDs with spaces in the name (e.g., "angular/angular/PR Review" from the catalog)
/// pass through to the URL fragment unencoded. This test documents that the space is
/// preserved as-is — the install layer is responsible for URL-encoding if needed.
#[test]
fn resolve_deterministic_skill_name_with_space_passes_through_unencoded() {
    let provider = SkillsShProvider;
    // "angular" is not in SKILLS_REPO_NAMES, so no "skills/" prefix is added.
    let info = provider.resolve("angular/angular/PR Review").unwrap();

    assert_eq!(
        info.download_url,
        "https://github.com/angular/angular/archive/HEAD.zip#PR Review"
    );
    assert_eq!(info.format, "zip");
}

#[test]
fn pinned_provider_prefers_local_content_and_never_uses_head() {
    let root = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(root.path().join("valid-skill")).unwrap();
    let registry = load_curated_registry(std::path::Path::new(
        "tests/fixtures/curated-skills/registry.v1.toml",
    ))
    .unwrap();
    let provider = agentsync::skills::provider::PinnedProvider::new(registry, root.path());
    let info = provider.resolve("owner/valid-skill").unwrap();
    assert_eq!(info.format, "dir");
    assert!(!info.download_url.contains("HEAD"));
    assert!(info.download_url.ends_with("valid-skill"));
}

#[test]
fn pinned_provider_requires_explicit_fallback() {
    let registry = load_curated_registry(std::path::Path::new(
        "tests/fixtures/curated-skills/registry.v1.toml",
    ))
    .unwrap();
    let root = tempfile::TempDir::new().unwrap();
    let provider = agentsync::skills::provider::PinnedProvider::new(registry, root.path());
    let error = provider
        .without_remote_fallback()
        .resolve("owner/valid-skill")
        .unwrap_err();
    assert!(error.to_string().contains("remote fallback is disabled"));
}

#[test]
fn pinned_provider_builds_commit_archive_only_when_fallback_is_enabled() {
    let registry = load_curated_registry(std::path::Path::new(
        "tests/fixtures/curated-skills/registry.v1.toml",
    ))
    .unwrap();
    let root = tempfile::TempDir::new().unwrap();
    let provider = agentsync::skills::provider::PinnedProvider::new(registry, root.path());
    let info = provider
        .allow_remote_fallback()
        .resolve("owner/valid-skill")
        .unwrap();
    assert!(
        info.download_url
            .contains("0123456789abcdef0123456789abcdef0123456a.zip")
    );
    assert!(!info.download_url.contains("HEAD"));
}

#[test]
fn pinned_provider_rejects_ambiguous_provider_or_local_identifier() {
    let registry = load_curated_registry(std::path::Path::new(
        "tests/fixtures/curated-skills/registry.v1.toml",
    ))
    .unwrap();
    let mut entries = registry.entries;
    let mut duplicate = entries["valid-skill"].clone();
    duplicate.provider_skill_id = "valid-skill".into();
    entries.insert("other-skill".into(), duplicate);
    let registry = agentsync::skills::registry::RegistryDocument {
        schema_version: "v1".into(),
        entries,
    };
    let root = tempfile::TempDir::new().unwrap();
    let provider = agentsync::skills::provider::PinnedProvider::new(registry, root.path());
    let error = provider.resolve("valid-skill").unwrap_err();
    assert!(error.to_string().contains("ambiguous"));
}
