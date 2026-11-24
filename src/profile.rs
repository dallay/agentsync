//! Composable configuration profiles.
//!
//! Profiles allow named configuration overlays that compose with the base
//! `agentsync.toml`. Each profile is a TOML file in `.agents/profiles/*.toml`
//! and can extend or replace sections of the base config.
//!
//! Profiles support inheritance via the `extends` field, enabling layered
//! configuration (e.g., base → dev → team-specific).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{AgentConfig, Config, GitignoreConfig, McpGlobalConfig, McpServerConfig};

/// Default directory for profile files relative to `.agents/`.
pub const PROFILES_DIR: &str = "profiles";

/// A named configuration profile that composes with base project config.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct Profile {
    /// Human-readable description of this profile.
    #[serde(default)]
    pub description: String,

    /// Another profile this profile extends. The extended profile is loaded
    /// and merged first, then this profile's values are layered on top.
    #[serde(default)]
    pub extends: Option<String>,

    /// Agents to override or add. Merges with base agents using additive
    /// semantics for new agents and field-level override for existing agents.
    #[serde(default)]
    pub agents: BTreeMap<String, AgentConfig>,

    /// MCP global settings override.
    #[serde(default)]
    pub mcp: Option<McpGlobalConfig>,

    /// MCP servers to override or add.
    #[serde(default)]
    pub mcp_servers: BTreeMap<String, McpServerConfig>,

    /// Gitignore settings override.
    #[serde(default)]
    pub gitignore: Option<GitignoreConfig>,

    /// If true, this profile replaces the entire agents map rather than merging.
    /// Defaults to false (additive merge).
    #[serde(default)]
    pub replace_agents: bool,

    /// If true, this profile replaces the entire mcp_servers map rather than merging.
    /// Defaults to false (additive merge).
    #[serde(default)]
    pub replace_mcp_servers: bool,
}

/// Result of applying a profile to a base config.
#[derive(Debug, Clone)]
pub struct ProfileApplyResult {
    /// The merged configuration.
    pub config: Config,
    /// Profiles that were applied in order (first = base-most).
    pub applied_profiles: Vec<String>,
    /// Warnings during merge (e.g., deprecated fields, conflicts).
    pub warnings: Vec<String>,
}

/// Load a profile by name from the profiles directory.
pub fn load_profile(profiles_dir: &Path, name: &str) -> Result<Profile> {
    let profile_path = profiles_dir.join(format!("{name}.toml"));
    let content = fs::read_to_string(&profile_path)
        .with_context(|| format!("Failed to read profile file: {}", profile_path.display()))?;
    let profile: Profile = toml::from_str(&content)
        .with_context(|| format!("Failed to parse profile file: {}", profile_path.display()))?;
    Ok(profile)
}

/// List all available profiles in the profiles directory.
pub fn list_profiles(profiles_dir: &Path) -> Result<Vec<String>> {
    let mut profiles = Vec::new();
    if !profiles_dir.exists() {
        return Ok(profiles);
    }
    for entry in fs::read_dir(profiles_dir).with_context(|| {
        format!(
            "Failed to read profiles directory: {}",
            profiles_dir.display()
        )
    })? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("toml")
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
        {
            profiles.push(stem.to_string());
        }
    }
    profiles.sort();
    Ok(profiles)
}

/// Resolve a profile chain (following `extends`) and return the ordered list
/// of profile names from base to leaf.
fn resolve_profile_chain(profiles_dir: &Path, name: &str) -> Result<Vec<String>> {
    let mut chain = Vec::new();
    let mut current: Option<String> = Some(name.to_string());
    let mut visited = BTreeSet::new();

    while let Some(profile_name) = current {
        if !visited.insert(profile_name.clone()) {
            anyhow::bail!(
                "Cycle detected in profile chain: '{}' appears twice",
                profile_name
            );
        }

        let profile = load_profile(profiles_dir, &profile_name)
            .with_context(|| format!("Failed to load profile: {}", profile_name))?;
        chain.push(profile_name.clone());

        current = profile.extends;
    }

    // Reverse so the base is first
    chain.reverse();
    Ok(chain)
}

/// Apply a named profile to a base config, following the extends chain.
pub fn apply_profile(
    base: &Config,
    profiles_dir: &Path,
    profile_name: &str,
) -> Result<ProfileApplyResult> {
    let chain = resolve_profile_chain(profiles_dir, profile_name)?;
    let mut current = base.clone();
    let mut warnings = Vec::new();

    for name in &chain {
        let profile = load_profile(profiles_dir, name)
            .with_context(|| format!("Failed to load profile in chain: {}", name))?;
        current = merge_profile(&current, &profile, &mut warnings);
    }

    Ok(ProfileApplyResult {
        config: current,
        applied_profiles: chain,
        warnings,
    })
}

/// Merge a profile into a base config.
fn merge_profile(base: &Config, profile: &Profile, _warnings: &mut Vec<String>) -> Config {
    let mut result = base.clone();

    // Merge agents
    if profile.replace_agents {
        result.agents = profile.agents.clone();
    } else if !profile.agents.is_empty() {
        for (agent_name, profile_agent) in &profile.agents {
            match result.agents.get_mut(agent_name) {
                Some(base_agent) => {
                    // Field-level override for existing agent
                    if !profile_agent.description.is_empty() {
                        base_agent.description = profile_agent.description.clone();
                    }
                    if profile_agent.enabled != base_agent.enabled {
                        base_agent.enabled = profile_agent.enabled;
                    }
                    // Merge targets additively
                    for (target_name, profile_target) in &profile_agent.targets {
                        base_agent
                            .targets
                            .insert(target_name.clone(), profile_target.clone());
                    }
                }
                None => {
                    // New agent added by profile
                    result
                        .agents
                        .insert(agent_name.clone(), profile_agent.clone());
                }
            }
        }
    }

    // Merge MCP global config
    if let Some(ref mcp) = profile.mcp {
        result.mcp = mcp.clone();
    }

    // Merge MCP servers
    if profile.replace_mcp_servers {
        result.mcp_servers = profile.mcp_servers.clone();
    } else if !profile.mcp_servers.is_empty() {
        for (server_name, server_config) in &profile.mcp_servers {
            result
                .mcp_servers
                .insert(server_name.clone(), server_config.clone());
        }
    }

    // Merge gitignore
    if let Some(ref gitignore) = profile.gitignore {
        result.gitignore = gitignore.clone();
    }

    result
}

/// Show profile information without applying it.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileInfo {
    /// Profile name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Parent profile if this profile extends another.
    pub extends: Option<String>,
    /// Number of agents defined in this profile.
    pub agent_count: usize,
    /// Number of MCP servers defined in this profile.
    pub mcp_server_count: usize,
}

/// Get information about a profile without loading the full chain.
pub fn get_profile_info(profiles_dir: &Path, name: &str) -> Result<ProfileInfo> {
    let profile = load_profile(profiles_dir, name)?;
    Ok(ProfileInfo {
        name: name.to_string(),
        description: profile.description,
        extends: profile.extends,
        agent_count: profile.agents.len(),
        mcp_server_count: profile.mcp_servers.len(),
    })
}

/// Validate that a profile chain is resolvable (no missing profiles, no cycles).
pub fn validate_profile_chain(profiles_dir: &Path, name: &str) -> Result<()> {
    resolve_profile_chain(profiles_dir, name)?;
    Ok(())
}

/// Get the profiles directory for a given project root.
pub fn profiles_dir_for_project(project_root: &Path) -> PathBuf {
    project_root.join(".agents").join(PROFILES_DIR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TargetConfig;
    use tempfile::TempDir;

    fn create_test_profiles(temp_dir: &TempDir, profiles: &[(&str, &str)]) {
        let profiles_dir = temp_dir.path().join(PROFILES_DIR);
        fs::create_dir_all(&profiles_dir).unwrap();
        for (name, content) in profiles {
            let path = profiles_dir.join(format!("{name}.toml"));
            fs::write(&path, *content).unwrap();
        }
    }

    fn base_config() -> Config {
        let mut config = Config::default();
        let mut agent = AgentConfig {
            description: "Base agent".to_string(),
            ..Default::default()
        };
        let target = TargetConfig {
            source: "source.md".to_string(),
            destination: "dest.md".to_string(),
            sync_type: crate::config::SyncType::Symlink,
            pattern: None,
            exclude: Vec::new(),
            mappings: Vec::new(),
        };
        agent.targets.insert("instructions".to_string(), target);
        config.agents.insert("test".to_string(), agent);
        config
    }

    #[test]
    fn test_list_profiles_empty() {
        let temp_dir = TempDir::new().unwrap();
        let profiles_dir = temp_dir.path().join(PROFILES_DIR);
        fs::create_dir_all(&profiles_dir).unwrap();
        let profiles = list_profiles(&profiles_dir).unwrap();
        assert!(profiles.is_empty());
    }

    #[test]
    fn test_list_profiles() {
        let temp_dir = TempDir::new().unwrap();
        create_test_profiles(
            &temp_dir,
            &[
                ("dev", r#"description = "Development""#),
                ("prod", r#"description = "Production""#),
            ],
        );
        let profiles_dir = temp_dir.path().join(PROFILES_DIR);
        let profiles = list_profiles(&profiles_dir).unwrap();
        assert_eq!(profiles, vec!["dev", "prod"]);
    }

    #[test]
    fn test_load_profile() {
        let temp_dir = TempDir::new().unwrap();
        create_test_profiles(
            &temp_dir,
            &[(
                "dev",
                r#"
description = "Development profile"

[agents.test]
enabled = true
description = "Dev test agent"
"#,
            )],
        );
        let profiles_dir = temp_dir.path().join(PROFILES_DIR);
        let profile = load_profile(&profiles_dir, "dev").unwrap();
        assert_eq!(profile.description, "Development profile");
        assert_eq!(profile.agents.len(), 1);
    }

    #[test]
    fn test_profile_merge_adds_agent() {
        let temp_dir = TempDir::new().unwrap();
        create_test_profiles(
            &temp_dir,
            &[(
                "add-agent",
                r#"
description = "Add new agent"

[agents.new_agent]
enabled = true
description = "New agent from profile"
"#,
            )],
        );
        let profiles_dir = temp_dir.path().join(PROFILES_DIR);
        let base = base_config();
        let result = apply_profile(&base, &profiles_dir, "add-agent").unwrap();
        assert!(result.config.agents.contains_key("test"));
        assert!(result.config.agents.contains_key("new_agent"));
    }

    #[test]
    fn test_profile_merge_overrides_agent() {
        let temp_dir = TempDir::new().unwrap();
        create_test_profiles(
            &temp_dir,
            &[(
                "override",
                r#"
description = "Override existing agent"

[agents.test]
enabled = false
description = "Overridden agent"
"#,
            )],
        );
        let profiles_dir = temp_dir.path().join(PROFILES_DIR);
        let base = base_config();
        let result = apply_profile(&base, &profiles_dir, "override").unwrap();
        let agent = result.config.agents.get("test").unwrap();
        assert!(!agent.enabled);
        assert_eq!(agent.description, "Overridden agent");
    }

    #[test]
    fn test_profile_extends_chain() {
        let temp_dir = TempDir::new().unwrap();
        create_test_profiles(
            &temp_dir,
            &[
                (
                    "base",
                    r#"
description = "Base profile"

[agents.base_agent]
enabled = true
description = "Base agent"
"#,
                ),
                (
                    "child",
                    r#"
description = "Child profile"
extends = "base"

[agents.child_agent]
enabled = true
description = "Child agent"
"#,
                ),
            ],
        );
        let profiles_dir = temp_dir.path().join(PROFILES_DIR);
        let base = Config::default();
        let result = apply_profile(&base, &profiles_dir, "child").unwrap();
        assert!(result.config.agents.contains_key("base_agent"));
        assert!(result.config.agents.contains_key("child_agent"));
        assert_eq!(result.applied_profiles, vec!["base", "child"]);
    }

    #[test]
    fn test_profile_cycle_detection() {
        let temp_dir = TempDir::new().unwrap();
        create_test_profiles(
            &temp_dir,
            &[
                (
                    "a",
                    r#"
description = "Profile A"
extends = "b"
"#,
                ),
                (
                    "b",
                    r#"
description = "Profile B"
extends = "a"
"#,
                ),
            ],
        );
        let profiles_dir = temp_dir.path().join(PROFILES_DIR);
        let result = validate_profile_chain(&profiles_dir, "a");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cycle detected"));
    }

    #[test]
    fn test_profile_missing_extends() {
        let temp_dir = TempDir::new().unwrap();
        create_test_profiles(
            &temp_dir,
            &[(
                "child",
                r#"
description = "Child profile"
extends = "nonexistent"
"#,
            )],
        );
        let profiles_dir = temp_dir.path().join(PROFILES_DIR);
        let result = validate_profile_chain(&profiles_dir, "child");
        assert!(result.is_err());
    }

    #[test]
    fn test_replace_agents() {
        let temp_dir = TempDir::new().unwrap();
        create_test_profiles(
            &temp_dir,
            &[(
                "replace",
                r#"
description = "Replace all agents"
replace_agents = true

[agents.only_this]
enabled = true
description = "Only this agent"
"#,
            )],
        );
        let profiles_dir = temp_dir.path().join(PROFILES_DIR);
        let base = base_config();
        let result = apply_profile(&base, &profiles_dir, "replace").unwrap();
        assert!(!result.config.agents.contains_key("test"));
        assert!(result.config.agents.contains_key("only_this"));
        assert_eq!(result.config.agents.len(), 1);
    }

    #[test]
    fn test_profile_info() {
        let temp_dir = TempDir::new().unwrap();
        create_test_profiles(
            &temp_dir,
            &[(
                "test",
                r#"
description = "Test profile"
extends = "base"

[agents.agent1]
enabled = true

[agents.agent2]
enabled = true

[mcp_servers.server1]
command = "npx"
"#,
            )],
        );
        let profiles_dir = temp_dir.path().join(PROFILES_DIR);
        let info = get_profile_info(&profiles_dir, "test").unwrap();
        assert_eq!(info.name, "test");
        assert_eq!(info.description, "Test profile");
        assert_eq!(info.extends, Some("base".to_string()));
        assert_eq!(info.agent_count, 2);
        assert_eq!(info.mcp_server_count, 1);
    }
}
