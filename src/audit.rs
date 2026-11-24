//! Static security audit for AgentSync-managed configuration.
//!
//! This module provides read-only, offline analysis of skills, plugins, and MCP
//! configurations to detect credential exposure, risky shell patterns, file traversal
//! issues, and unexpected privilege escalation. It uses the shared `Diagnostic` schema
//! from `src/diagnostics.rs` and never executes code or accesses the network.

use crate::config::McpServerConfig;
use crate::diagnostics::{
    BoundedTraversal, DIAGNOSTICS_SCHEMA_VERSION, Diagnostic, Severity, Span, ValidationRule,
};
use crate::plugins::{is_environment_reference, is_sensitive_mcp_key};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Overall audit report with summary counts.
#[derive(Debug, Clone, Serialize)]
pub struct AuditReport {
    /// Schema version for the JSON contract.
    pub schema_version: u32,
    /// Total number of diagnostics found.
    pub total: usize,
    /// Count by severity.
    pub counts: BTreeMap<Severity, usize>,
    /// Individual diagnostic findings.
    pub diagnostics: Vec<Diagnostic>,
}

impl AuditReport {
    pub fn from_diagnostics(diagnostics: Vec<Diagnostic>) -> Self {
        let mut counts = BTreeMap::new();
        counts.insert(Severity::Info, 0);
        counts.insert(Severity::Warning, 0);
        counts.insert(Severity::Error, 0);
        for d in &diagnostics {
            *counts.get_mut(&d.severity).unwrap() += 1;
        }
        Self {
            schema_version: DIAGNOSTICS_SCHEMA_VERSION,
            total: diagnostics.len(),
            counts,
            diagnostics,
        }
    }
}

/// Detects credential-like literals in configuration files.
pub struct CredentialLiteralRule {
    _root: PathBuf,
}

impl CredentialLiteralRule {
    pub fn new(root: &Path) -> Self {
        Self {
            _root: root.to_path_buf(),
        }
    }
}

impl ValidationRule for CredentialLiteralRule {
    fn rule_id(&self) -> &str {
        "credential-literal"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check_file(&self, path: &Path, content: &[u8]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let Ok(text) = std::str::from_utf8(content) else {
            return diagnostics;
        };

        // Check for TOML/JSON files that might contain MCP server configs
        let extension = path.extension().and_then(|e| e.to_str());
        if !matches!(
            extension,
            Some("toml") | Some("json") | Some("yaml") | Some("yml")
        ) {
            return diagnostics;
        }

        // Look for sensitive key patterns with literal values
        for (line_idx, line) in text.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with('#') || trimmed.starts_with("//") {
                continue;
            }

            // Check for sensitive keys with literal values
            for sensitive_key in SENSITIVE_KEY_PATTERNS {
                if let Some(_eq_pos) = find_key_value(line, sensitive_key) {
                    // Extract the value after the key
                    let value = extract_value(line, sensitive_key);

                    // If the value is not an environment reference, flag it
                    if !value.is_empty() && !is_environment_reference(&value) {
                        // Check if it looks like a real secret (not a placeholder)
                        if looks_like_secret(&value) {
                            let relative_path = portable_path(&self._root, path);
                            let fp = fingerprint(
                                self.rule_id(),
                                &relative_path,
                                (line_idx + 1) as u32,
                                sensitive_key,
                            );
                            diagnostics.push(Diagnostic {
                                rule_id: self.rule_id().to_string(),
                                severity: self.default_severity(),
                                source_path: relative_path,
                                span: Some(Span::line((line_idx + 1) as u32)),
                                message: format!(
                                    "Potential credential literal in sensitive key '{}'",
                                    sensitive_key
                                ),
                                evidence: Some(format!(
                                    "Key: {}, Value: <redacted> (length: {})",
                                    sensitive_key,
                                    value.len()
                                )),
                                fingerprint: fp,
                            });
                        }
                    }
                }
            }
        }

        diagnostics
    }
}

/// Patterns for sensitive keys in configuration files.
const SENSITIVE_KEY_PATTERNS: &[&str] = &[
    "api_key",
    "apikey",
    "api_secret",
    "secret_key",
    "access_token",
    "auth_token",
    "bearer_token",
    "password",
    "passwd",
    "token",
    "private_key",
    "client_secret",
    "app_secret",
    "secret",
];

/// Check if a value looks like a real secret rather than a placeholder.
fn looks_like_secret(value: &str) -> bool {
    let trimmed = value.trim().trim_matches('"').trim_matches('\'');

    // Skip empty values
    if trimmed.is_empty() {
        return false;
    }

    // Skip obvious placeholders
    let placeholders = [
        "your_",
        "placeholder",
        "example",
        "changeme",
        "xxx",
        "TODO",
        "FIXME",
    ];
    let lower = trimmed.to_lowercase();
    if placeholders.iter().any(|p| lower.contains(p)) {
        return false;
    }

    // Skip short values that are likely not secrets
    if trimmed.len() < 8 {
        return false;
    }

    // Skip values that are clearly environment references
    if is_environment_reference(trimmed) {
        return false;
    }

    true
}

/// Find a key-value pair in a line and return the position of the value.
fn find_key_value(line: &str, key: &str) -> Option<usize> {
    let lower = line.to_lowercase();
    let key_lower = key.to_lowercase();

    // Look for key = or key: patterns (with optional whitespace)
    let patterns = [
        format!("{}=", key_lower),
        format!("{} =", key_lower),
        format!("{}:", key_lower),
        format!("{} :", key_lower),
        format!("\"{}\"", key_lower),
        format!("'{}'", key_lower),
    ];

    for pattern in &patterns {
        if let Some(pos) = lower.find(pattern) {
            return Some(pos);
        }
    }

    None
}

/// Extract the value from a key-value pair.
fn extract_value(line: &str, key: &str) -> String {
    let lower = line.to_lowercase();
    let key_lower = key.to_lowercase();

    // Try to find the key and extract the value (with optional whitespace around = or :)
    for separator in ["=", " =", ":", " :"] {
        let search = format!("{}{}", key_lower, separator);
        if let Some(pos) = lower.find(&search) {
            let after_sep = &line[pos + search.len()..];
            let trimmed = after_sep.trim();

            // If the value starts with a quote, extract until the closing quote
            if let Some(stripped) = trimmed.strip_prefix('"') {
                if let Some(end) = stripped.find('"') {
                    return stripped[..end].to_string();
                }
            } else if let Some(stripped) = trimmed.strip_prefix('\'')
                && let Some(end) = stripped.find('\'')
            {
                return stripped[..end].to_string();
            }

            // Otherwise trim common delimiters
            return trimmed
                .trim_matches('"')
                .trim_matches('\'')
                .trim_matches(|c| c == ',' || c == '}')
                .to_string();
        }
    }

    String::new()
}

/// Detects risky fetch-and-execute shell patterns.
pub struct FetchAndExecuteRule {
    _root: PathBuf,
}

impl FetchAndExecuteRule {
    pub fn new(root: &Path) -> Self {
        Self {
            _root: root.to_path_buf(),
        }
    }
}

impl ValidationRule for FetchAndExecuteRule {
    fn rule_id(&self) -> &str {
        "fetch-and-execute"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_file(&self, path: &Path, content: &[u8]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let Ok(text) = std::str::from_utf8(content) else {
            return diagnostics;
        };

        // Only check shell scripts and similar files
        let extension = path.extension().and_then(|e| e.to_str());
        if !matches!(
            extension,
            Some("sh") | Some("bash") | Some("zsh") | Some("ps1") | Some("bat")
        ) {
            return diagnostics;
        }

        // Patterns that indicate fetch-and-execute (each tuple: list of substrings that must all be present)
        let patterns: &[(&[&str], &str)] = &[
            (&["curl", "|", "bash"], "curl piped to bash"),
            (&["curl", "|", "sh"], "curl piped to sh"),
            (&["wget", "|", "bash"], "wget piped to bash"),
            (&["wget", "|", "sh"], "wget piped to sh"),
            (
                &["curl", "-o", "&&", "bash"],
                "curl download followed by bash execution",
            ),
            (
                &["curl", "-o", "&&", "sh"],
                "curl download followed by sh execution",
            ),
            (
                &["invoke-expression", "invoke-webrequest"],
                "PowerShell Invoke-Expression with Invoke-WebRequest",
            ),
            (&["iex", "irm"], "PowerShell IEX with IRM"),
        ];

        for (line_idx, line) in text.lines().enumerate() {
            let trimmed = line.trim().to_lowercase();

            // Skip comments
            if trimmed.starts_with('#') || trimmed.starts_with('>') {
                continue;
            }

            for (substrings, description) in patterns {
                if substrings.iter().all(|s| trimmed.contains(*s)) {
                    let relative_path = portable_path(&self._root, path);
                    let fp = fingerprint(
                        self.rule_id(),
                        &relative_path,
                        (line_idx + 1) as u32,
                        description,
                    );
                    diagnostics.push(Diagnostic {
                        rule_id: self.rule_id().to_string(),
                        severity: self.default_severity(),
                        source_path: relative_path,
                        span: Some(Span::line((line_idx + 1) as u32)),
                        message: format!("Risky fetch-and-execute pattern: {}", description),
                        evidence: Some(format!(
                            "Line: {}",
                            line.trim().chars().take(80).collect::<String>()
                        )),
                        fingerprint: fp,
                    });
                }
            }
        }

        diagnostics
    }
}

/// Detects file traversal and symlink escape attempts.
pub struct FileTraversalRule {
    _root: PathBuf,
}

impl FileTraversalRule {
    pub fn new(root: &Path) -> Self {
        Self {
            _root: root.to_path_buf(),
        }
    }
}

impl ValidationRule for FileTraversalRule {
    fn rule_id(&self) -> &str {
        "file-traversal"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_file(&self, path: &Path, content: &[u8]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let Ok(text) = std::str::from_utf8(content) else {
            return diagnostics;
        };

        // Check for path traversal patterns
        let patterns = [
            ("../", "parent directory traversal"),
            ("..\\", "parent directory traversal (Windows)"),
            ("/etc/passwd", "access to system password file"),
            ("/etc/shadow", "access to system shadow file"),
            ("%00", "null byte injection"),
        ];

        for (line_idx, line) in text.lines().enumerate() {
            for (pattern, description) in &patterns {
                if line.contains(pattern) {
                    let relative_path = portable_path(&self._root, path);
                    let fp = fingerprint(
                        self.rule_id(),
                        &relative_path,
                        (line_idx + 1) as u32,
                        description,
                    );
                    diagnostics.push(Diagnostic {
                        rule_id: self.rule_id().to_string(),
                        severity: self.default_severity(),
                        source_path: relative_path,
                        span: Some(Span::line((line_idx + 1) as u32)),
                        message: format!("Potential file traversal: {}", description),
                        evidence: Some(format!("Pattern: {}", pattern)),
                        fingerprint: fp,
                    });
                }
            }
        }

        diagnostics
    }
}

/// Detects unexpected MCP server privilege escalation.
pub struct McpPrivilegeRule {
    _root: PathBuf,
}

impl McpPrivilegeRule {
    pub fn new(root: &Path) -> Self {
        Self {
            _root: root.to_path_buf(),
        }
    }
}

impl ValidationRule for McpPrivilegeRule {
    fn rule_id(&self) -> &str {
        "mcp-privilege"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check_file(&self, path: &Path, content: &[u8]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Only check TOML config files
        let extension = path.extension().and_then(|e| e.to_str());
        if extension != Some("toml") {
            return diagnostics;
        }

        let Ok(text) = std::str::from_utf8(content) else {
            return diagnostics;
        };

        // Check for MCP servers with potentially dangerous commands
        let dangerous_patterns = [
            ("sudo", "MCP server command uses sudo"),
            ("chmod", "MCP server command modifies permissions"),
            ("chown", "MCP server command changes ownership"),
            ("rm -rf", "MCP server command uses dangerous removal"),
            ("mkfs", "MCP server command formats filesystem"),
        ];

        for (line_idx, line) in text.lines().enumerate() {
            let lower = line.to_lowercase();

            for (pattern, description) in &dangerous_patterns {
                if lower.contains(pattern) {
                    let relative_path = portable_path(&self._root, path);
                    let fp = fingerprint(
                        self.rule_id(),
                        &relative_path,
                        (line_idx + 1) as u32,
                        description,
                    );
                    diagnostics.push(Diagnostic {
                        rule_id: self.rule_id().to_string(),
                        severity: self.default_severity(),
                        source_path: relative_path,
                        span: Some(Span::line((line_idx + 1) as u32)),
                        message: description.to_string(),
                        evidence: Some(format!(
                            "Line: {}",
                            line.trim().chars().take(80).collect::<String>()
                        )),
                        fingerprint: fp,
                    });
                }
            }
        }

        diagnostics
    }
}

/// Audit MCP server configuration for credential exposure.
pub fn audit_mcp_server(
    name: &str,
    server: &McpServerConfig,
    source_path: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Check env values
    for (key, value) in &server.env {
        if is_sensitive_mcp_key(key) && !is_environment_reference(value) {
            diagnostics.push(Diagnostic {
                rule_id: "credential-literal".to_string(),
                severity: Severity::Error,
                source_path: source_path.to_string(),
                span: None,
                message: format!(
                    "MCP server '{}' has a literal secret in '{}'; use an environment reference such as ${{TOKEN}}",
                    name, key
                ),
                evidence: Some(format!(
                    "Key: {}, Value: <redacted> (length: {})",
                    key,
                    value.len()
                )),
                fingerprint: fingerprint("credential-literal", source_path, 0, key),
            });
        }
    }

    // Check headers
    for (key, value) in &server.headers {
        if is_sensitive_mcp_key(key) {
            // Check if it's a Bearer token or similar
            let is_safe = value.starts_with("${")
                || value.starts_with("$")
                || value.starts_with("Bearer ${")
                || value.starts_with("Bearer $");

            if !is_safe {
                diagnostics.push(Diagnostic {
                    rule_id: "credential-literal".to_string(),
                    severity: Severity::Error,
                    source_path: source_path.to_string(),
                    span: None,
                    message: format!(
                        "MCP server '{}' has a literal secret in header '{}'",
                        name, key
                    ),
                    evidence: Some(format!(
                        "Header: {}, Value: <redacted> (length: {})",
                        key,
                        value.len()
                    )),
                    fingerprint: fingerprint("credential-literal", source_path, 0, key),
                });
            }
        }
    }

    // Check args
    for arg in &server.args {
        // Check if any arg looks like a secret
        if let Some(stripped) = arg.strip_prefix("--") {
            let arg_lower = stripped.to_lowercase();
            if (arg_lower.contains("token")
                || arg_lower.contains("secret")
                || arg_lower.contains("key"))
                && let Some(eq_pos) = stripped.find('=')
            {
                let value = &stripped[eq_pos + 1..];
                if !is_environment_reference(value) && value.len() > 8 {
                    diagnostics.push(Diagnostic {
                        rule_id: "credential-literal".to_string(),
                        severity: Severity::Warning,
                        source_path: source_path.to_string(),
                        span: None,
                        message: format!(
                            "MCP server '{}' may have a literal secret in command args",
                            name
                        ),
                        evidence: Some(format!(
                            "Arg: {}, Value: <redacted>",
                            arg.split('=').next().unwrap_or("")
                        )),
                        fingerprint: fingerprint("credential-literal", source_path, 0, "args"),
                    });
                }
            }
        }
    }

    diagnostics
}

/// Run all audit rules on a directory.
pub fn audit_directory(root: &Path) -> AuditReport {
    let mut all_diagnostics = Vec::new();

    let rules: Vec<Box<dyn ValidationRule>> = vec![
        Box::new(CredentialLiteralRule::new(root)),
        Box::new(FetchAndExecuteRule::new(root)),
        Box::new(FileTraversalRule::new(root)),
        Box::new(McpPrivilegeRule::new(root)),
    ];

    let traversal = BoundedTraversal::new();

    if let Err(e) = traversal.traverse(root, |path, content| {
        for rule in &rules {
            let diagnostics = rule.check_file(path, content);
            all_diagnostics.extend(diagnostics);
        }
        true
    }) {
        eprintln!("Warning: Traversal error: {:?}", e);
    }

    AuditReport::from_diagnostics(all_diagnostics)
}

/// Convert a path to a portable relative path.
fn portable_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

/// Generate a stable fingerprint for deduplication.
fn fingerprint(rule_id: &str, source_path: &str, line: u32, context: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    rule_id.hash(&mut hasher);
    source_path.hash(&mut hasher);
    line.hash(&mut hasher);
    context.hash(&mut hasher);
    let hash = hasher.finish();
    format!("{:016x}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_credential_literal_detection() {
        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("config.toml");

        fs::write(
            &config_file,
            r#"
api_key = "sk-1234567890abcdef"
"#,
        )
        .unwrap();

        let rule = CredentialLiteralRule::new(temp_dir.path());
        let content = fs::read(&config_file).unwrap();
        let diagnostics = rule.check_file(&config_file, &content);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id, "credential-literal");
        assert_eq!(diagnostics[0].severity, Severity::Error);

        // Verify the actual secret is not in the evidence
        assert!(
            !diagnostics[0]
                .evidence
                .as_ref()
                .unwrap()
                .contains("sk-1234567890abcdef")
        );
    }

    #[test]
    fn test_credential_literal_with_env_ref() {
        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("config.toml");

        fs::write(
            &config_file,
            r#"
[agents.opencode.mcp-servers.test]
command = "npx"
env = { API_KEY = "${API_KEY}" }
"#,
        )
        .unwrap();

        let rule = CredentialLiteralRule::new(temp_dir.path());
        let content = fs::read(&config_file).unwrap();
        let diagnostics = rule.check_file(&config_file, &content);

        // Should not flag environment references
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn test_fetch_and_execute_detection() {
        let temp_dir = TempDir::new().unwrap();
        let script_file = temp_dir.path().join("setup.sh");

        fs::write(
            &script_file,
            r#"#!/bin/bash
curl | bash
"#,
        )
        .unwrap();

        let rule = FetchAndExecuteRule::new(temp_dir.path());
        let content = fs::read(&script_file).unwrap();
        let diagnostics = rule.check_file(&script_file, &content);

        assert!(!diagnostics.is_empty());
        assert_eq!(diagnostics[0].rule_id, "fetch-and-execute");
    }

    #[test]
    fn test_file_traversal_detection() {
        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("config.toml");

        fs::write(
            &config_file,
            r#"
source = "../../../etc/passwd"
"#,
        )
        .unwrap();

        let rule = FileTraversalRule::new(temp_dir.path());
        let content = fs::read(&config_file).unwrap();
        let diagnostics = rule.check_file(&config_file, &content);

        assert!(!diagnostics.is_empty());
        assert_eq!(diagnostics[0].rule_id, "file-traversal");
    }

    #[test]
    fn test_mcp_server_audit() {
        let server = McpServerConfig {
            command: Some("npx".to_string()),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
            ],
            env: BTreeMap::from([(
                "API_KEY".to_string(),
                "sk-plaintext-secret-value".to_string(),
            )]),
            headers: BTreeMap::new(),
            disabled: false,
            url: None,
            transport_type: None,
        };

        let diagnostics = audit_mcp_server("test-server", &server, "agentsync.toml");

        assert!(!diagnostics.is_empty());
        assert_eq!(diagnostics[0].rule_id, "credential-literal");

        // Verify redaction
        if let Some(evidence) = &diagnostics[0].evidence {
            assert!(!evidence.contains("sk-plaintext-secret-value"));
        }
    }

    #[test]
    fn test_audit_report_counts() {
        let diagnostics = vec![
            Diagnostic {
                rule_id: "test".to_string(),
                severity: Severity::Error,
                source_path: "test.toml".to_string(),
                span: None,
                message: "Test error".to_string(),
                evidence: None,
                fingerprint: "test1".to_string(),
            },
            Diagnostic {
                rule_id: "test".to_string(),
                severity: Severity::Warning,
                source_path: "test.toml".to_string(),
                span: None,
                message: "Test warning".to_string(),
                evidence: None,
                fingerprint: "test2".to_string(),
            },
        ];

        let report = AuditReport::from_diagnostics(diagnostics);
        assert_eq!(report.total, 2);
        assert_eq!(*report.counts.get(&Severity::Error).unwrap(), 1);
        assert_eq!(*report.counts.get(&Severity::Warning).unwrap(), 1);
    }
}
