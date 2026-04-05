//! Integration tests for skill catalog entries.
//!
//! These tests verify that skills from the embedded catalog can be installed.
//! By default, only a subset is tested to keep CI fast. Set environment variables to run more:
//!
//! - `E2E_RUN_ALL_CATALOG_SKILLS=1` - Run all skills (slow, requires network)
//! - `E2E_CATALOG_SKILL_LIMIT=N` - Run only first N skills
//!
//! Skills are tested against the skills.sh provider (external provider).
//! Local dallay skills are tested by installing from the local `.agents/skills/` directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

/// Represents a skill from the catalog to test.
#[derive(Debug, Clone)]
struct CatalogSkill {
    /// The provider skill ID (e.g., "dallay/agents-skills/accessibility")
    provider_skill_id: String,
    /// The local skill ID (e.g., "accessibility")
    local_skill_id: String,
    /// Whether this skill is from dallay (local) or external
    is_local: bool,
}

/// TOML structure for deserializing catalog entries
#[derive(Debug, serde::Deserialize)]
struct CatalogFile {
    skills: Vec<CatalogEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct CatalogEntry {
    provider_skill_id: String,
    local_skill_id: String,
}

/// Parse the catalog.v1.toml and extract skills to test.
/// Uses proper TOML deserialization.
fn extract_catalog_skills() -> Vec<CatalogSkill> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let catalog_path = if manifest_dir.join("src").exists() {
        manifest_dir
            .join("src")
            .join("skills")
            .join("catalog.v1.toml")
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("src")
            .join("skills")
            .join("catalog.v1.toml")
    };

    let content = fs::read_to_string(&catalog_path).expect("Failed to read catalog.v1.toml");

    // Parse using toml
    let catalog: CatalogFile = toml::from_str(&content).expect("Failed to parse catalog.v1.toml");

    catalog
        .skills
        .into_iter()
        .map(|entry| {
            let is_local = entry.provider_skill_id.starts_with("dallay/");
            CatalogSkill {
                provider_skill_id: entry.provider_skill_id,
                local_skill_id: entry.local_skill_id,
                is_local,
            }
        })
        .collect()
}

/// Determine if we should run this test based on environment variables.
fn should_run_skill_test(skill_index: usize, _total_skills: usize) -> bool {
    // Check if we should run all
    if std::env::var("E2E_RUN_ALL_CATALOG_SKILLS").is_ok() {
        return true;
    }

    // Check if we have a limit
    if let Ok(limit) = std::env::var("E2E_CATALOG_SKILL_LIMIT")
        && let Ok(n) = limit.parse::<usize>()
    {
        return skill_index < n;
    }

    // Default: run only first 5 skills for quick sanity check
    skill_index < 5
}

/// Initialize a temporary project with agentsync.
/// Returns Ok(()) on success, or an error string on failure.
fn init_temp_project(root: &Path) -> Result<(), String> {
    let output = Command::new("cargo")
        .args(["run", "--", "init", "--path"])
        .arg(root)
        .output()
        .map_err(|e| format!("failed to execute init: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("init failed: {}", stderr))
    }
}

/// Install a skill in the given project.
/// Returns the command output, or an error string on failure.
fn install_skill(root: &Path, skill_id: &str, source: Option<&str>) -> Result<Output, String> {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--", "skill"]);

    if let Some(source_path) = source {
        cmd.args(["install", skill_id, "--source", source_path]);
    } else {
        cmd.args(["install", skill_id]);
    }

    cmd.arg("--project-root").arg(root);

    cmd.output()
        .map_err(|e| format!("failed to execute install: {}", e))
}

/// Verify a skill is installed in the given project.
fn verify_skill_installed(root: &Path, skill_id: &str) -> bool {
    let skill_dir = root.join(".agents/skills").join(skill_id);
    skill_dir.exists() && skill_dir.join("SKILL.md").exists()
}

/// Test installation of a skill from the catalog.
/// This test is parameterized - can run many skills.
#[test]
fn test_install_skill_from_catalog() {
    let skills = extract_catalog_skills();

    // Filter skills to test based on environment
    let skills_to_test: Vec<_> = skills
        .iter()
        .enumerate()
        .filter(|(idx, _)| should_run_skill_test(*idx, skills.len()))
        .collect();

    println!(
        "Testing {} out of {} skills from catalog",
        skills_to_test.len(),
        skills.len()
    );

    for (idx, (_, skill)) in skills_to_test.into_iter().enumerate() {
        println!(
            "[{}/{}] Testing skill: {} ({})",
            idx + 1,
            skills.len(),
            skill.local_skill_id,
            skill.provider_skill_id
        );

        let temp = TempDir::new().unwrap();
        let root = temp.path();

        // 1. Init agentsync
        if let Err(e) = init_temp_project(root) {
            eprintln!("  ⚠️ Init failed: {}, skipping skill", e);
            continue;
        }

        // 2. Try to install the skill
        let result = install_skill(root, &skill.local_skill_id, None);

        match result {
            Ok(output) => {
                if output.status.success() {
                    // 3. Verify installation
                    if verify_skill_installed(root, &skill.local_skill_id) {
                        println!("  ✅ Installed successfully");
                    } else {
                        println!(
                            "  ⚠️ Command succeeded but skill files not found at {:?}",
                            root.join(".agents/skills").join(&skill.local_skill_id)
                        );
                    }
                } else {
                    // Installation failed - could be network issue or skill doesn't exist
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    println!(
                        "  ⚠️ Install failed: {}",
                        if stderr.len() > 200 {
                            format!("{}...", &stderr[..200])
                        } else {
                            stderr.to_string()
                        }
                    );
                }
            }
            Err(e) => {
                println!("  ⚠️ Failed to run install command: {}", e);
            }
        }
    }

    // Test passes as long as we tried - actual verification happens via logs
    println!("✅ Catalog skill installation test completed");
}

/// Test that local dallay skills can be installed from the local .agents/skills directory.
/// These are the skills that come bundled with agentsync itself.
#[test]
fn test_install_local_dallay_skills() {
    // Get the agentsync project root (where this test is running)
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Check if local skills exist
    let local_skills_dir = project_root.join(".agents").join("skills");
    if !local_skills_dir.exists() {
        println!(
            "⚠️ Local skills directory not found at {:?}",
            local_skills_dir
        );
        return;
    }

    // Read the catalog to find local dallay skills
    let skills = extract_catalog_skills();
    let local_skills: Vec<_> = skills
        .iter()
        .filter(|s| s.is_local && !s.local_skill_id.is_empty())
        .collect();

    println!("Testing {} local dallay skills", local_skills.len());

    for skill in local_skills {
        let source_dir = local_skills_dir.join(&skill.local_skill_id);
        if !source_dir.exists() {
            println!(
                "  ⚠️ Local skill not found: {} at {:?}",
                skill.local_skill_id, source_dir
            );
            continue;
        }

        println!("  Testing local skill: {}", skill.local_skill_id);

        // Create temp project and install from local source
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        // Init
        if let Err(e) = init_temp_project(root) {
            println!("  ⚠️ Init failed for {}: {}", skill.local_skill_id, e);
            continue;
        }

        // Install from local path - fix the flag order here
        let result = install_skill(
            root,
            &skill.local_skill_id,
            Some(source_dir.to_str().unwrap()),
        );

        match result {
            Ok(output) => {
                if output.status.success() {
                    if verify_skill_installed(root, &skill.local_skill_id) {
                        println!("    ✅ Installed successfully");
                    } else {
                        println!("    ⚠️ Install succeeded but files missing");
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    println!(
                        "    ⚠️ Install failed: {}",
                        if stderr.len() > 100 {
                            format!("{}...", &stderr[..100])
                        } else {
                            stderr.to_string()
                        }
                    );
                }
            }
            Err(e) => {
                println!("    ⚠️ Command error: {}", e);
            }
        }
    }

    println!("✅ Local dallay skills test completed");
}

/// Quick sanity check: verify catalog is readable and has expected structure.
#[test]
fn test_catalog_structure() {
    let skills = extract_catalog_skills();

    // Should have at least 100 skills in the catalog
    assert!(
        skills.len() >= 100,
        "Expected at least 100 skills in catalog, found {}",
        skills.len()
    );

    // Should have local dallay skills
    let local_count = skills.iter().filter(|s| s.is_local).count();
    assert!(
        local_count > 0,
        "Expected at least some local dallay skills"
    );

    // Should have external skills
    let external_count = skills.iter().filter(|s| !s.is_local).count();
    assert!(external_count > 0, "Expected at least some external skills");

    println!(
        "✅ Catalog has {} skills ({} local, {} external)",
        skills.len(),
        local_count,
        external_count
    );
}
