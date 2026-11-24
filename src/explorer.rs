//! Local configuration explorer data API.
//!
//! Provides structured data for the configuration explorer UI, including
//! configuration tree visualization, profile comparison, and audit results display.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// A node in the configuration tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigNode {
    /// Display name.
    pub name: String,
    /// Type of this node.
    pub node_type: NodeType,
    /// Path relative to the project root.
    pub path: String,
    /// Child nodes.
    pub children: Vec<ConfigNode>,
    /// Metadata for this node.
    pub metadata: BTreeMap<String, String>,
}

/// Type of a configuration node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeType {
    Directory,
    File,
    Symlink,
    Agent,
    Skill,
    McpServer,
    Profile,
    Policy,
}

/// Comparison result between two profiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileComparison {
    /// Name of the base profile.
    pub base: String,
    /// Name of the overlay profile.
    pub overlay: String,
    /// Keys only in the base profile.
    pub only_in_base: Vec<String>,
    /// Keys only in the overlay profile.
    pub only_in_overlay: Vec<String>,
    /// Keys with different values.
    pub different: Vec<ProfileDiff>,
    /// Keys with the same value.
    pub same: Vec<String>,
}

/// A single difference between two profiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDiff {
    /// The key being compared.
    pub key: String,
    /// Value in the base profile.
    pub base_value: String,
    /// Value in the overlay profile.
    pub overlay_value: String,
}

/// Audit results for display in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResults {
    /// Total number of issues found.
    pub total_issues: usize,
    /// Issues by severity.
    pub by_severity: BTreeMap<String, usize>,
    /// Individual issues.
    pub issues: Vec<AuditIssue>,
}

/// A single audit issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditIssue {
    /// Rule that triggered this issue.
    pub rule: String,
    /// Severity level.
    pub severity: String,
    /// File or path where the issue was found.
    pub location: String,
    /// Description of the issue.
    pub message: String,
}

/// Inspect data for display in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectData {
    /// Configuration tree.
    pub tree: ConfigNode,
    /// Profile comparisons.
    pub profile_comparisons: Vec<ProfileComparison>,
    /// Audit results.
    pub audit_results: AuditResults,
}

/// Build a configuration tree from the `.agents/` directory.
pub fn build_config_tree(agents_dir: &Path) -> ConfigNode {
    let mut root = ConfigNode {
        name: ".agents".to_string(),
        node_type: NodeType::Directory,
        path: ".".to_string(),
        children: Vec::new(),
        metadata: BTreeMap::new(),
    };

    if let Ok(entries) = std::fs::read_dir(agents_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            if path.is_dir() {
                let dir_name = name.to_lowercase();
                let node_type = if dir_name == "skills" {
                    NodeType::Skill
                } else if dir_name == "profiles" {
                    NodeType::Profile
                } else if dir_name == "policies" {
                    NodeType::Policy
                } else {
                    NodeType::Directory
                };

                let mut node = ConfigNode {
                    name: name.clone(),
                    node_type: node_type.clone(),
                    path: path
                        .strip_prefix(agents_dir)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string(),
                    children: Vec::new(),
                    metadata: BTreeMap::new(),
                };

                // Recurse into subdirectories
                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                    for sub_entry in sub_entries.flatten() {
                        let sub_path = sub_entry.path();
                        let sub_name = sub_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();

                        if sub_path.is_dir() {
                            node.children.push(ConfigNode {
                                name: sub_name,
                                node_type: NodeType::Directory,
                                path: sub_path
                                    .strip_prefix(agents_dir)
                                    .unwrap_or(&sub_path)
                                    .to_string_lossy()
                                    .to_string(),
                                children: Vec::new(),
                                metadata: BTreeMap::new(),
                            });
                        } else if sub_path.is_file() {
                            let extension =
                                sub_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                            let mut metadata = BTreeMap::new();
                            metadata.insert("extension".into(), extension.into());

                            if let Ok(size) = std::fs::metadata(&sub_path) {
                                metadata.insert("size".into(), size.len().to_string());
                            }

                            node.children.push(ConfigNode {
                                name: sub_name,
                                node_type: NodeType::File,
                                path: sub_path
                                    .strip_prefix(agents_dir)
                                    .unwrap_or(&sub_path)
                                    .to_string_lossy()
                                    .to_string(),
                                children: Vec::new(),
                                metadata,
                            });
                        }
                    }
                }

                root.children.push(node);
            } else if path.is_file() {
                let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

                let node_type = if extension == "toml" {
                    NodeType::File
                } else if extension == "json" {
                    NodeType::McpServer
                } else {
                    NodeType::File
                };

                let mut metadata = BTreeMap::new();
                metadata.insert("extension".into(), extension.into());

                if let Ok(size) = std::fs::metadata(&path) {
                    metadata.insert("size".into(), size.len().to_string());
                }

                root.children.push(ConfigNode {
                    name,
                    node_type,
                    path: path
                        .strip_prefix(agents_dir)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string(),
                    children: Vec::new(),
                    metadata,
                });
            }
        }
    }

    // Sort children by name
    root.children.sort_by(|a, b| a.name.cmp(&b.name));

    root
}

/// Compare two profiles and return differences.
pub fn compare_profiles(
    base_keys: &BTreeMap<String, String>,
    overlay_keys: &BTreeMap<String, String>,
    base_name: &str,
    overlay_name: &str,
) -> ProfileComparison {
    let mut only_in_base = Vec::new();
    let mut only_in_overlay = Vec::new();
    let mut different = Vec::new();
    let mut same = Vec::new();

    let all_keys: std::collections::BTreeSet<&String> =
        base_keys.keys().chain(overlay_keys.keys()).collect();

    for key in all_keys {
        match (base_keys.get(key), overlay_keys.get(key)) {
            (Some(base_val), Some(overlay_val)) => {
                if base_val == overlay_val {
                    same.push(key.clone());
                } else {
                    different.push(ProfileDiff {
                        key: key.clone(),
                        base_value: base_val.clone(),
                        overlay_value: overlay_val.clone(),
                    });
                }
            }
            (Some(_), None) => {
                only_in_base.push(key.clone());
            }
            (None, Some(_)) => {
                only_in_overlay.push(key.clone());
            }
            (None, None) => {}
        }
    }

    ProfileComparison {
        base: base_name.to_string(),
        overlay: overlay_name.to_string(),
        only_in_base,
        only_in_overlay,
        different,
        same,
    }
}

/// Convert audit diagnostics to UI-friendly format.
pub fn convert_audit_results(issues: &[crate::diagnostics::Diagnostic]) -> AuditResults {
    let mut by_severity = BTreeMap::new();
    let mut ui_issues = Vec::new();

    for issue in issues {
        let severity = format!("{}", issue.severity);
        *by_severity.entry(severity.clone()).or_insert(0) += 1;

        ui_issues.push(AuditIssue {
            rule: issue.rule_id.clone(),
            severity,
            location: issue.source_path.clone(),
            message: issue.message.clone(),
        });
    }

    AuditResults {
        total_issues: issues.len(),
        by_severity,
        issues: ui_issues,
    }
}

/// Generate full inspect data for the UI.
pub fn generate_inspect_data(agents_dir: &Path) -> InspectData {
    let tree = build_config_tree(agents_dir);

    InspectData {
        tree,
        profile_comparisons: Vec::new(),
        audit_results: AuditResults {
            total_issues: 0,
            by_severity: BTreeMap::new(),
            issues: Vec::new(),
        },
    }
}

/// Serialize inspect data to JSON for API consumption.
pub fn inspect_data_to_json(data: &InspectData) -> serde_json::Result<String> {
    serde_json::to_string_pretty(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_config_tree_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let tree = build_config_tree(temp_dir.path());
        assert_eq!(tree.name, ".agents");
        assert!(tree.children.is_empty());
    }

    #[test]
    fn test_build_config_tree_with_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("agentsync.toml"), "[project]").unwrap();
        let tree = build_config_tree(temp_dir.path());
        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].name, "agentsync.toml");
    }

    #[test]
    fn test_build_config_tree_with_dirs() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp_dir.path().join("skills")).unwrap();
        std::fs::create_dir(temp_dir.path().join("profiles")).unwrap();
        let tree = build_config_tree(temp_dir.path());
        assert_eq!(tree.children.len(), 2);
        // Children are sorted by name: "profiles" comes before "skills"
        assert_eq!(tree.children[0].node_type, NodeType::Profile);
        assert_eq!(tree.children[1].node_type, NodeType::Skill);
    }

    #[test]
    fn test_compare_profiles_identical() {
        let mut keys = BTreeMap::new();
        keys.insert("key1".to_string(), "value1".to_string());
        let comparison = compare_profiles(&keys, &keys, "base", "overlay");
        assert!(comparison.only_in_base.is_empty());
        assert!(comparison.only_in_overlay.is_empty());
        assert!(comparison.different.is_empty());
        assert_eq!(comparison.same.len(), 1);
    }

    #[test]
    fn test_compare_profiles_different() {
        let mut base = BTreeMap::new();
        base.insert("key1".to_string(), "value1".to_string());
        let mut overlay = BTreeMap::new();
        overlay.insert("key1".to_string(), "value2".to_string());
        let comparison = compare_profiles(&base, &overlay, "base", "overlay");
        assert_eq!(comparison.different.len(), 1);
        assert_eq!(comparison.different[0].base_value, "value1");
        assert_eq!(comparison.different[0].overlay_value, "value2");
    }

    #[test]
    fn test_compare_profiles_only_in_base() {
        let mut base = BTreeMap::new();
        base.insert("key1".to_string(), "value1".to_string());
        let overlay = BTreeMap::new();
        let comparison = compare_profiles(&base, &overlay, "base", "overlay");
        assert_eq!(comparison.only_in_base.len(), 1);
    }

    #[test]
    fn test_compare_profiles_only_in_overlay() {
        let base = BTreeMap::new();
        let mut overlay = BTreeMap::new();
        overlay.insert("key1".to_string(), "value1".to_string());
        let comparison = compare_profiles(&base, &overlay, "base", "overlay");
        assert_eq!(comparison.only_in_overlay.len(), 1);
    }

    #[test]
    fn test_convert_audit_results() {
        let issues = vec![crate::diagnostics::Diagnostic {
            rule_id: "test-rule".to_string(),
            severity: crate::diagnostics::Severity::Warning,
            source_path: String::new(),
            span: None,
            message: "Test message".to_string(),
            evidence: None,
            fingerprint: String::new(),
        }];
        let results = convert_audit_results(&issues);
        assert_eq!(results.total_issues, 1);
        assert_eq!(results.issues[0].rule, "test-rule");
    }

    #[test]
    fn test_inspect_data_to_json() {
        let data = generate_inspect_data(Path::new("/nonexistent"));
        let json = inspect_data_to_json(&data);
        assert!(json.is_ok());
        let json_str = json.unwrap();
        assert!(json_str.contains(".agents"));
    }

    #[test]
    fn test_node_type_serialization() {
        let node = NodeType::Symlink;
        let json = serde_json::to_string(&node).unwrap();
        assert_eq!(json, "\"symlink\"");
    }
}
