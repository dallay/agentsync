use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkillUninstallError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Registry error: {0}")]
    Registry(String),
    #[error("Skill not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
}

/// Uninstall a skill by removing its directory and registry entry.
///
/// # Arguments
/// * `skill_id` - The identifier of the skill to uninstall
/// * `target_root` - The root directory where skills are installed (e.g., `.agents/skills`)
///
/// # Returns
/// * `Ok(())` if the skill was successfully uninstalled
/// * `Err(SkillUninstallError)` if the skill was not found or an error occurred
pub fn uninstall_skill(skill_id: &str, target_root: &Path) -> Result<(), SkillUninstallError> {
    // Validate skill_id to prevent path traversal
    if skill_id.contains('/') || skill_id.contains('\\') || skill_id.is_empty() {
        return Err(SkillUninstallError::Validation(
            "Invalid skill ID: must not contain path separators or be empty".to_string(),
        ));
    }

    let skill_dir = target_root.join(skill_id);
    let registry_path = target_root.join("registry.json");

    // Check if the skill directory exists
    if !skill_dir.exists() {
        return Err(SkillUninstallError::NotFound(skill_id.to_string()));
    }

    // Remove the skill directory
    fs::remove_dir_all(&skill_dir).map_err(SkillUninstallError::Io)?;

    // Update the registry to remove the entry
    if registry_path.exists()
        && let Err(e) = remove_registry_entry(&registry_path, skill_id)
    {
        tracing::warn!(
            skill_id = %skill_id,
            error = %e,
            "Failed to update registry after uninstall, but skill directory was removed"
        );
    }

    Ok(())
}

/// Remove a skill entry from the registry.
fn remove_registry_entry(registry_path: &Path, skill_id: &str) -> Result<(), SkillUninstallError> {
    use crate::skills::registry::{Registry, read_registry};

    let mut reg: Registry =
        read_registry(registry_path).map_err(|e| SkillUninstallError::Registry(e.to_string()))?;

    if let Some(ref mut skills) = reg.skills {
        skills.remove(skill_id);
        reg.last_updated = Some(chrono::Utc::now().to_rfc3339());

        let body = serde_json::to_string_pretty(&reg)
            .map_err(|e| SkillUninstallError::Registry(e.to_string()))?;
        fs::write(registry_path, body).map_err(|e| SkillUninstallError::Registry(e.to_string()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_uninstall_skill_success() {
        let temp_dir = TempDir::new().unwrap();
        let target_root = temp_dir.path();

        // Create a mock skill directory
        let skill_dir = target_root.join("test-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# Test Skill").unwrap();

        // Create a mock registry
        let registry_path = target_root.join("registry.json");
        let registry_content = r#"{
            "schemaVersion": 1,
            "skills": {
                "test-skill": {
                    "name": "Test Skill",
                    "version": "1.0.0"
                }
            }
        }"#;
        fs::write(&registry_path, registry_content).unwrap();

        // Uninstall the skill
        let result = uninstall_skill("test-skill", target_root);
        assert!(result.is_ok());

        // Verify the skill directory is removed
        assert!(!skill_dir.exists());

        // Verify the registry entry is removed
        let updated_registry = fs::read_to_string(&registry_path).unwrap();
        assert!(!updated_registry.contains("test-skill"));
    }

    #[test]
    fn test_uninstall_skill_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let target_root = temp_dir.path();

        // Try to uninstall a skill that doesn't exist
        let result = uninstall_skill("non-existent-skill", target_root);
        assert!(matches!(result, Err(SkillUninstallError::NotFound(_))));
    }

    #[test]
    fn test_uninstall_skill_invalid_id() {
        let temp_dir = TempDir::new().unwrap();
        let target_root = temp_dir.path();

        // Try to uninstall with invalid skill ID
        let result = uninstall_skill("skill/with/slashes", target_root);
        assert!(matches!(result, Err(SkillUninstallError::Validation(_))));

        let result = uninstall_skill("", target_root);
        assert!(matches!(result, Err(SkillUninstallError::Validation(_))));
    }
}
