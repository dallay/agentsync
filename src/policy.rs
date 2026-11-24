//! Versioned policy packs for deterministic validation.
//!
//! Policy packs define validation rules that can be evaluated against the
//! current AgentSync configuration. They are stored in `.agents/policies/*.toml`
//! and use the shared diagnostic schema from `src/diagnostics.rs`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::diagnostics::{DIAGNOSTICS_SCHEMA_VERSION, Diagnostic, Severity};

/// Default directory for policy files relative to `.agents/`.
pub const POLICIES_DIR: &str = "policies";

/// Schema version for policy packs.
pub const POLICY_SCHEMA_VERSION: u32 = 1;

/// A policy pack containing validation rules.
#[derive(Debug, Deserialize, Clone)]
pub struct PolicyPack {
    /// Human-readable description.
    #[serde(default)]
    pub description: String,

    /// Schema version.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// Agents that are allowed. If empty, all agents are allowed.
    #[serde(default)]
    pub allowed_agents: Vec<String>,

    /// Agents that are explicitly denied.
    #[serde(default)]
    pub denied_agents: Vec<String>,

    /// Skills that are allowed. If empty, all skills are allowed.
    #[serde(default)]
    pub allowed_skills: Vec<String>,

    /// Skills that are explicitly denied.
    #[serde(default)]
    pub denied_skills: Vec<String>,

    /// MCP servers that are allowed. If empty, all MCP servers are allowed.
    #[serde(default)]
    pub allowed_mcp_servers: Vec<String>,

    /// MCP servers that are explicitly denied.
    #[serde(default)]
    pub denied_mcp_servers: Vec<String>,

    /// Maximum number of agents allowed.
    #[serde(default)]
    pub max_agents: Option<usize>,

    /// Maximum number of MCP servers allowed.
    #[serde(default)]
    pub max_mcp_servers: Option<usize>,

    /// Require all agents to have a description.
    #[serde(default)]
    pub require_agent_descriptions: bool,

    /// Require all agents to have at least one target.
    #[serde(default)]
    pub require_agent_targets: bool,
}

fn default_schema_version() -> u32 {
    POLICY_SCHEMA_VERSION
}

impl Default for PolicyPack {
    fn default() -> Self {
        Self {
            description: String::new(),
            schema_version: POLICY_SCHEMA_VERSION,
            allowed_agents: Vec::new(),
            denied_agents: Vec::new(),
            allowed_skills: Vec::new(),
            denied_skills: Vec::new(),
            allowed_mcp_servers: Vec::new(),
            denied_mcp_servers: Vec::new(),
            max_agents: None,
            max_mcp_servers: None,
            require_agent_descriptions: false,
            require_agent_targets: false,
        }
    }
}

/// Result of evaluating a policy pack.
#[derive(Debug, Clone, Serialize)]
pub struct PolicyReport {
    /// Schema version.
    pub schema_version: u32,
    /// Policy pack name.
    pub policy_name: String,
    /// Total number of diagnostics.
    pub total: usize,
    /// Count by severity.
    pub counts: std::collections::BTreeMap<Severity, usize>,
    /// Individual diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

impl PolicyReport {
    pub fn from_diagnostics(name: String, diagnostics: Vec<Diagnostic>) -> Self {
        let mut counts = std::collections::BTreeMap::new();
        counts.insert(Severity::Info, 0);
        counts.insert(Severity::Warning, 0);
        counts.insert(Severity::Error, 0);
        for d in &diagnostics {
            *counts.get_mut(&d.severity).unwrap() += 1;
        }
        Self {
            schema_version: DIAGNOSTICS_SCHEMA_VERSION,
            policy_name: name,
            total: diagnostics.len(),
            counts,
            diagnostics,
        }
    }

    /// Check if the policy passed (no errors).
    pub fn passed(&self) -> bool {
        *self.counts.get(&Severity::Error).unwrap_or(&0) == 0
    }
}

/// Load a policy pack by name.
pub fn load_policy(policies_dir: &Path, name: &str) -> Result<PolicyPack> {
    let policy_path = policies_dir.join(format!("{name}.toml"));
    let content = fs::read_to_string(&policy_path)
        .with_context(|| format!("Failed to read policy file: {}", policy_path.display()))?;
    let policy: PolicyPack = toml::from_str(&content)
        .with_context(|| format!("Failed to parse policy file: {}", policy_path.display()))?;
    Ok(policy)
}

/// List all available policy packs.
pub fn list_policies(policies_dir: &Path) -> Result<Vec<String>> {
    let mut policies = Vec::new();
    if !policies_dir.exists() {
        return Ok(policies);
    }
    for entry in fs::read_dir(policies_dir).with_context(|| {
        format!(
            "Failed to read policies directory: {}",
            policies_dir.display()
        )
    })? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("toml")
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
        {
            policies.push(stem.to_string());
        }
    }
    policies.sort();
    Ok(policies)
}

/// Evaluate a policy pack against a config.
pub fn evaluate_policy(policy: &PolicyPack, config: &Config, project_root: &Path) -> PolicyReport {
    let mut diagnostics = Vec::new();

    // Check allowed agents
    if !policy.allowed_agents.is_empty() {
        for agent_name in config.agents.keys() {
            if !policy.allowed_agents.contains(agent_name) {
                diagnostics.push(Diagnostic {
                    rule_id: "allowed-agents".to_string(),
                    severity: Severity::Error,
                    source_path: ".agents/agentsync.toml".to_string(),
                    span: None,
                    message: format!("Agent '{}' is not in the allowed agents list", agent_name),
                    evidence: Some(format!("Allowed: {:?}", policy.allowed_agents)),
                    fingerprint: format!("allowed-agents:{}", agent_name),
                });
            }
        }
    }

    // Check denied agents
    for agent_name in config.agents.keys() {
        if policy.denied_agents.contains(agent_name) {
            diagnostics.push(Diagnostic {
                rule_id: "denied-agents".to_string(),
                severity: Severity::Error,
                source_path: ".agents/agentsync.toml".to_string(),
                span: None,
                message: format!("Agent '{}' is explicitly denied", agent_name),
                evidence: None,
                fingerprint: format!("denied-agents:{}", agent_name),
            });
        }
    }

    // Check allowed MCP servers
    if !policy.allowed_mcp_servers.is_empty() {
        for server_name in config.mcp_servers.keys() {
            if !policy.allowed_mcp_servers.contains(server_name) {
                diagnostics.push(Diagnostic {
                    rule_id: "allowed-mcp-servers".to_string(),
                    severity: Severity::Warning,
                    source_path: ".agents/agentsync.toml".to_string(),
                    span: None,
                    message: format!("MCP server '{}' is not in the allowed list", server_name),
                    evidence: Some(format!("Allowed: {:?}", policy.allowed_mcp_servers)),
                    fingerprint: format!("allowed-mcp-servers:{}", server_name),
                });
            }
        }
    }

    // Check denied MCP servers
    for server_name in config.mcp_servers.keys() {
        if policy.denied_mcp_servers.contains(server_name) {
            diagnostics.push(Diagnostic {
                rule_id: "denied-mcp-servers".to_string(),
                severity: Severity::Error,
                source_path: ".agents/agentsync.toml".to_string(),
                span: None,
                message: format!("MCP server '{}' is explicitly denied", server_name),
                evidence: None,
                fingerprint: format!("denied-mcp-servers:{}", server_name),
            });
        }
    }

    // Check max agents
    if let Some(max) = policy.max_agents
        && config.agents.len() > max
    {
        diagnostics.push(Diagnostic {
            rule_id: "max-agents".to_string(),
            severity: Severity::Warning,
            source_path: ".agents/agentsync.toml".to_string(),
            span: None,
            message: format!(
                "{} agents configured, maximum is {}",
                config.agents.len(),
                max
            ),
            evidence: None,
            fingerprint: "max-agents".to_string(),
        });
    }

    // Check max MCP servers
    if let Some(max) = policy.max_mcp_servers
        && config.mcp_servers.len() > max
    {
        diagnostics.push(Diagnostic {
            rule_id: "max-mcp-servers".to_string(),
            severity: Severity::Warning,
            source_path: ".agents/agentsync.toml".to_string(),
            span: None,
            message: format!(
                "{} MCP servers configured, maximum is {}",
                config.mcp_servers.len(),
                max
            ),
            evidence: None,
            fingerprint: "max-mcp-servers".to_string(),
        });
    }

    // Check require agent descriptions
    if policy.require_agent_descriptions {
        for (name, agent) in &config.agents {
            if agent.description.is_empty() {
                diagnostics.push(Diagnostic {
                    rule_id: "require-agent-descriptions".to_string(),
                    severity: Severity::Info,
                    source_path: ".agents/agentsync.toml".to_string(),
                    span: None,
                    message: format!("Agent '{}' is missing a description", name),
                    evidence: None,
                    fingerprint: format!("require-agent-descriptions:{}", name),
                });
            }
        }
    }

    // Check require agent targets
    if policy.require_agent_targets {
        for (name, agent) in &config.agents {
            if agent.targets.is_empty() {
                diagnostics.push(Diagnostic {
                    rule_id: "require-agent-targets".to_string(),
                    severity: Severity::Warning,
                    source_path: ".agents/agentsync.toml".to_string(),
                    span: None,
                    message: format!("Agent '{}' has no targets configured", name),
                    evidence: None,
                    fingerprint: format!("require-agent-targets:{}", name),
                });
            }
        }
    }

    // Check skills against policy
    if !policy.allowed_skills.is_empty() || !policy.denied_skills.is_empty() {
        let skills_dir = project_root.join(".agents").join("skills");
        if skills_dir.exists()
            && let Ok(entries) = fs::read_dir(&skills_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                    if !policy.allowed_skills.is_empty()
                        && !policy.allowed_skills.contains(&name.to_string())
                    {
                        diagnostics.push(Diagnostic {
                            rule_id: "allowed-skills".to_string(),
                            severity: Severity::Warning,
                            source_path: format!(
                                ".agents/skills/{}",
                                path.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default()
                            ),
                            span: None,
                            message: format!("Skill '{}' is not in the allowed list", name),
                            evidence: Some(format!("Allowed: {:?}", policy.allowed_skills)),
                            fingerprint: format!("allowed-skills:{}", name),
                        });
                    }
                    if policy.denied_skills.contains(&name.to_string()) {
                        diagnostics.push(Diagnostic {
                            rule_id: "denied-skills".to_string(),
                            severity: Severity::Error,
                            source_path: format!(
                                ".agents/skills/{}",
                                path.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default()
                            ),
                            span: None,
                            message: format!("Skill '{}' is explicitly denied", name),
                            evidence: None,
                            fingerprint: format!("denied-skills:{}", name),
                        });
                    }
                }
            }
        }
    }

    let name = "policy".to_string();
    PolicyReport::from_diagnostics(name, diagnostics)
}

/// Get the policies directory for a project.
pub fn policies_dir_for_project(project_root: &Path) -> PathBuf {
    project_root.join(".agents").join(POLICIES_DIR)
}

/// Evaluate all policy packs against a config.
pub fn evaluate_all_policies(project_root: &Path, config: &Config) -> Result<Vec<PolicyReport>> {
    let policies_dir = policies_dir_for_project(project_root);
    let mut reports = Vec::new();

    if !policies_dir.exists() {
        return Ok(reports);
    }

    let policy_names = list_policies(&policies_dir)?;
    for name in policy_names {
        let policy = match load_policy(&policies_dir, &name) {
            Ok(p) => p,
            Err(e) => {
                let diagnostics = vec![Diagnostic {
                    rule_id: "policy-parse".to_string(),
                    severity: Severity::Error,
                    source_path: format!(".agents/policies/{}.toml", name),
                    span: None,
                    message: format!("Failed to parse policy: {}", e),
                    evidence: None,
                    fingerprint: format!("policy-parse:{}", name),
                }];
                reports.push(PolicyReport::from_diagnostics(name, diagnostics));
                continue;
            }
        };
        let report = evaluate_policy(&policy, config, project_root);
        reports.push(report);
    }

    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_policies(temp_dir: &TempDir, policies: &[(&str, &str)]) {
        let policies_dir = temp_dir.path().join(POLICIES_DIR);
        fs::create_dir_all(&policies_dir).unwrap();
        for (name, content) in policies {
            let path = policies_dir.join(format!("{name}.toml"));
            fs::write(&path, *content).unwrap();
        }
    }

    fn config_with_agents(agent_names: &[&str]) -> Config {
        let mut config = Config::default();
        for name in agent_names {
            let agent = crate::config::AgentConfig::default();
            config.agents.insert(name.to_string(), agent);
        }
        config
    }

    #[test]
    fn test_allowed_agents_pass() {
        let policy = PolicyPack {
            allowed_agents: vec!["claude".to_string(), "copilot".to_string()],
            ..Default::default()
        };
        let config = config_with_agents(&["claude", "copilot"]);
        let report = evaluate_policy(&policy, &config, Path::new("/tmp"));
        assert!(report.passed());
    }

    #[test]
    fn test_allowed_agents_fail() {
        let policy = PolicyPack {
            allowed_agents: vec!["claude".to_string()],
            ..Default::default()
        };
        let config = config_with_agents(&["claude", "codex"]);
        let report = evaluate_policy(&policy, &config, Path::new("/tmp"));
        assert!(!report.passed());
        assert_eq!(report.total, 1);
    }

    #[test]
    fn test_denied_agents() {
        let policy = PolicyPack {
            denied_agents: vec!["codex".to_string()],
            ..Default::default()
        };
        let config = config_with_agents(&["claude", "codex"]);
        let report = evaluate_policy(&policy, &config, Path::new("/tmp"));
        assert!(!report.passed());
    }

    #[test]
    fn test_max_agents() {
        let policy = PolicyPack {
            max_agents: Some(2),
            ..Default::default()
        };
        let config = config_with_agents(&["claude", "copilot", "codex"]);
        let report = evaluate_policy(&policy, &config, Path::new("/tmp"));
        assert!(report.passed()); // Warning, not error
        assert_eq!(report.total, 1);
    }

    #[test]
    fn test_require_agent_descriptions() {
        let policy = PolicyPack {
            require_agent_descriptions: true,
            ..Default::default()
        };
        let config = config_with_agents(&["claude"]);
        let report = evaluate_policy(&policy, &config, Path::new("/tmp"));
        assert!(report.passed()); // Info, not error
        assert_eq!(report.total, 1);
    }

    #[test]
    fn test_list_policies() {
        let temp_dir = TempDir::new().unwrap();
        create_test_policies(
            &temp_dir,
            &[
                ("security", r#"description = "Security policy""#),
                ("compliance", r#"description = "Compliance policy""#),
            ],
        );
        let policies_dir = temp_dir.path().join(POLICIES_DIR);
        let policies = list_policies(&policies_dir).unwrap();
        assert_eq!(policies, vec!["compliance", "security"]);
    }

    #[test]
    fn test_load_policy() {
        let temp_dir = TempDir::new().unwrap();
        create_test_policies(
            &temp_dir,
            &[(
                "test",
                r#"
description = "Test policy"
allowed_agents = ["claude", "copilot"]
max_agents = 5
require_agent_descriptions = true
"#,
            )],
        );
        let policies_dir = temp_dir.path().join(POLICIES_DIR);
        let policy = load_policy(&policies_dir, "test").unwrap();
        assert_eq!(policy.description, "Test policy");
        assert_eq!(policy.allowed_agents, vec!["claude", "copilot"]);
        assert_eq!(policy.max_agents, Some(5));
        assert!(policy.require_agent_descriptions);
    }
}
