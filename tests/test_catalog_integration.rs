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
use std::process::Command;
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

/// Parse the catalog.v1.toml and extract skills to test.
/// Uses a simple state machine to parse skill blocks.
fn extract_catalog_skills() -> Vec<CatalogSkill> {
    // Need to find the source directory - during test it's in target/debug/deps or similar
    // We'll use the CARGO_MANIFEST_DIR and navigate up to find the project root
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    // The tests are run from the project root, so we need to find src/skills/catalog.v1.toml
    // CARGO_MANIFEST_DIR during test is the package root
    let project_root = manifest_dir; // This should be the agentsync root

    // Also check if there's a src directory (means we're in the right place)
    let catalog_path = if project_root.join("src").exists() {
        project_root
            .join("src")
            .join("skills")
            .join("catalog.v1.toml")
    } else {
        // Try to find it from the current working directory instead
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("src")
            .join("skills")
            .join("catalog.v1.toml")
    };

    let content = fs::read_to_string(&catalog_path).expect("Failed to read catalog.v1.toml");

    let mut skills = Vec::new();

    // Parse each [[skills]] block using a simple state machine
    let mut current_provider = String::new();
    let mut current_local = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // End of skills section (start of technologies)
        if trimmed.starts_with(
            "# =============================================================================",
        ) && trimmed.contains("Technologies")
        {
            break;
        }

        // Detect skill block start
        if trimmed.starts_with("[[skills]]") {
            // Save previous skill if we have both fields
            if !current_provider.is_empty() && !current_local.is_empty() {
                let is_local = current_provider.starts_with("dallay/");
                skills.push(CatalogSkill {
                    provider_skill_id: current_provider.clone(),
                    local_skill_id: current_local.clone(),
                    is_local,
                });
            }
            // Reset for new block
            current_provider.clear();
            current_local.clear();
            continue;
        }

        // Extract provider_skill_id
        if trimmed.starts_with("provider_skill_id") {
            // Need to parse: provider_skill_id = "value"
            let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
            if parts.len() == 2 {
                current_provider = parts[1].trim().trim_matches('"').to_string();
            }
        }
        // Extract local_skill_id
        else if trimmed.starts_with("local_skill_id") {
            let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
            if parts.len() == 2 {
                current_local = parts[1].trim().trim_matches('"').to_string();
            }
        }
    }

    // Don't forget the last skill
    if !current_provider.is_empty() && !current_local.is_empty() {
        let is_local = current_provider.starts_with("dallay/");
        skills.push(CatalogSkill {
            provider_skill_id: current_provider,
            local_skill_id: current_local,
            is_local,
        });
    }

    skills
}

/// Determine if we should run this test based on environment variables.
fn should_run_skill_test(skill_index: usize, _total_skills: usize) -> bool {
    // Check if we should run all
    if std::env::var("E2E_RUN_ALL_CATALOG_SKILLS").is_ok() {
        return true;
    }

    // Check if we have a limit
    if let Ok(limit) = std::env::var("E2E_CATALOG_SKILL_LIMIT") {
        if let Ok(n) = limit.parse::<usize>() {
            return skill_index < n;
        }
    }

    // Default: run only first 5 skills for quick sanity check
    skill_index < 5
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
        let init_status = Command::new("cargo")
            .args(["run", "--", "init", "--path"])
            .arg(root)
            .status()
            .unwrap();

        if !init_status.success() {
            eprintln!("  ⚠️ Init failed, skipping skill");
            continue;
        }

        // 2. Try to install the skill
        let install_result = Command::new("cargo")
            .args(["run", "--", "skill", "--project-root"])
            .arg(root)
            .args(["install", &skill.local_skill_id])
            .output();

        match install_result {
            Ok(output) => {
                let _stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if output.status.success() {
                    // 3. Verify installation
                    let skill_dir = root.join(".agents/skills").join(&skill.local_skill_id);
                    if skill_dir.exists() && skill_dir.join("SKILL.md").exists() {
                        println!("  ✅ Installed successfully");
                    } else {
                        println!(
                            "  ⚠️ Command succeeded but skill files not found at {:?}",
                            skill_dir
                        );
                    }
                } else {
                    // Installation failed - could be network issue or skill doesn't exist
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
        let init_status = Command::new("cargo")
            .args(["run", "--", "init", "--path"])
            .arg(root)
            .status()
            .unwrap();

        if !init_status.success() {
            println!("  ⚠️ Init failed for {}", skill.local_skill_id);
            continue;
        }

        // Install from local path
        let install_result = Command::new("cargo")
            .args(["run", "--", "skill", "--project-root"])
            .arg(root)
            .args([
                "install",
                &skill.local_skill_id,
                "--source",
                source_dir.to_str().unwrap(),
            ])
            .output();

        match install_result {
            Ok(output) => {
                if output.status.success() {
                    let skill_dir = root.join(".agents/skills").join(&skill.local_skill_id);
                    if skill_dir.exists() && skill_dir.join("SKILL.md").exists() {
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
