use std::fs;
use std::process::Command;
use tempfile::TempDir;

const MIN_ASTRO_RECOMMENDATIONS: usize = 5;

fn agentsync_bin() -> &'static str {
    env!("CARGO_BIN_EXE_agentsync")
}

#[test]
fn skill_suggest_json_contract_includes_required_fields() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .args(["skill", "suggest", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --json");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.get("detections").is_some());
    assert!(value.get("recommendations").is_some());
    assert!(value.get("summary").is_some());

    let detection = value["detections"].as_array().unwrap().first().unwrap();
    assert_eq!(detection["technology"], "rust");
    assert_eq!(detection["confidence"], "high");
    assert!(detection["evidence"].is_array());

    let recommendation = value["recommendations"]
        .as_array()
        .unwrap()
        .first()
        .unwrap();
    assert_eq!(recommendation["skill_id"], "rust-async-patterns");
    assert!(recommendation["matched_technologies"].is_array());
    assert!(recommendation["reasons"].is_array());
    assert_eq!(recommendation["installed"], false);

    assert_eq!(value["summary"]["detected_count"], 1);
    assert_eq!(value["summary"]["recommended_count"], 1);
    assert_eq!(value["summary"]["installable_count"], 1);
}

#[test]
fn skill_suggest_json_contract_is_well_formed_when_empty() {
    let temp_dir = TempDir::new().unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(temp_dir.path())
        .args(["skill", "suggest", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --json");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["detections"], serde_json::json!([]));
    assert_eq!(value["recommendations"], serde_json::json!([]));
    assert_eq!(value["summary"]["detected_count"], 0);
    assert_eq!(value["summary"]["recommended_count"], 0);
    assert_eq!(value["summary"]["installable_count"], 0);
}

#[test]
fn skill_suggest_json_contract_uses_spec_compliant_non_rust_technology_ids() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::create_dir_all(root.join(".github/workflows")).unwrap();
    fs::write(root.join("package.json"), "{\"name\":\"demo\"}\n").unwrap();
    fs::write(root.join(".github/workflows/ci.yml"), "name: ci\n").unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .args(["skill", "suggest", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --json");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let detections = value["detections"].as_array().unwrap();
    let technologies = detections
        .iter()
        .map(|detection| detection["technology"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(technologies.contains(&"node_typescript"));
    assert!(technologies.contains(&"github_actions"));

    let recommendations = value["recommendations"].as_array().unwrap();
    let best_practices = recommendations
        .iter()
        .find(|recommendation| recommendation["skill_id"] == "best-practices")
        .unwrap();
    let github_actions = recommendations
        .iter()
        .find(|recommendation| recommendation["skill_id"] == "github-actions")
        .unwrap();

    assert!(
        best_practices["matched_technologies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|technology| technology == "node_typescript")
    );
    assert!(
        github_actions["matched_technologies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|technology| technology == "github_actions")
    );
}

#[test]
fn skill_suggest_json_contract_supports_multiple_recommendations_for_one_technology() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("package.json"), "{\"name\":\"demo\"}\n").unwrap();
    fs::write(root.join("astro.config.mjs"), "export default {}\n").unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .args(["skill", "suggest", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --json");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let recommendations = value["recommendations"].as_array().unwrap();
    let astro_recommendations = recommendations
        .iter()
        .filter(|recommendation| {
            recommendation["matched_technologies"]
                .as_array()
                .unwrap()
                .iter()
                .any(|technology| technology == "astro")
        })
        .collect::<Vec<_>>();

    assert!(astro_recommendations.len() >= MIN_ASTRO_RECOMMENDATIONS);
    assert!(astro_recommendations.iter().all(|recommendation| {
        recommendation.get("skill_id").is_some()
            && recommendation.get("matched_technologies").is_some()
            && recommendation.get("reasons").is_some()
            && recommendation.get("installed").is_some()
    }));
}

#[test]
fn skill_suggest_install_all_json_contract_extends_suggest_shape() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let source_root = root.join("skill-sources");

    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();
    fs::create_dir_all(source_root.join("rust-async-patterns")).unwrap();
    fs::write(
        source_root.join("rust-async-patterns").join("SKILL.md"),
        "---\nname: rust-async-patterns\nversion: 1.0.0\n---\n# Rust Async Patterns\n",
    )
    .unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .env("AGENTSYNC_TEST_SKILL_SOURCE_DIR", &source_root)
        .args(["skill", "suggest", "--install", "--all", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --install --all --json");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.get("detections").is_some());
    assert!(value.get("recommendations").is_some());
    assert!(value.get("summary").is_some());
    assert_eq!(value["mode"], "install_all");
    assert!(value["selected_skill_ids"].is_array());
    assert!(value["results"].is_array());
}

#[test]
fn skill_suggest_install_without_tty_returns_structured_error() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(root.join("Cargo.toml"), "[package]\nname='demo'\n").unwrap();

    let output = Command::new(agentsync_bin())
        .current_dir(root)
        .args(["skill", "suggest", "--install", "--json"])
        .output()
        .expect("failed to run agentsync skill suggest --install --json");

    assert!(!output.status.success());

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["code"], "interactive_tty_required");
    assert!(
        value["error"]
            .as_str()
            .unwrap()
            .contains("interactive terminal")
    );
    assert!(
        value["remediation"]
            .as_str()
            .unwrap()
            .contains("--install --all")
    );
}
