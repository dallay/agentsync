use agentsync::skills::provider::{Provider, SkillInstallInfo};

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
