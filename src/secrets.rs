//! Secret reference resolution and redaction.
//!
//! Implements the secret-reference syntax (`${ENV_NAME}`), redaction rules,
//! and provider integration interface as specified in ADR-002.
//! This module provides a unified API for secret handling across all output channels.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

lazy_static::lazy_static! {
    static ref SENSITIVE_KEYS_RE: Regex = Regex::new(r"(?i)(token|secret|password|passwd|credential|api[_-]?key|apikey|authorization|cookie)").unwrap();
}

/// Classification of a string value for secret handling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretClassification {
    /// Value is an environment variable reference like `${ENV_NAME}`.
    EnvironmentReference,
    /// Value is a plaintext literal in a sensitive context.
    OpaqueSensitiveLiteral,
    /// Value is a plaintext literal in a non-sensitive context.
    SafeLiteral,
}

/// Result of resolving a secret reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretResolution {
    /// The original reference string.
    pub reference: String,
    /// Whether the reference was successfully resolved.
    pub resolved: bool,
    /// The resolved value (only if resolved is true).
    pub value: Option<String>,
    /// The classification of this value.
    pub classification: SecretClassification,
}

/// Configuration for secret handling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretConfig {
    /// Whether to allow environment variable resolution.
    pub allow_env_resolution: bool,
    /// Custom sensitive key patterns to add.
    pub additional_sensitive_patterns: Vec<String>,
}

impl Default for SecretConfig {
    fn default() -> Self {
        Self {
            allow_env_resolution: true,
            additional_sensitive_patterns: Vec::new(),
        }
    }
}

/// Check if a key name is sensitive based on known patterns.
pub fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace(['-', '.'], "_");
    SENSITIVE_KEYS_RE.is_match(&normalized)
}

/// Check if a value is an environment variable reference.
pub fn is_environment_reference(value: &str) -> bool {
    let value = value.trim();
    let valid_name = |name: &str| {
        !name.is_empty()
            && name.chars().enumerate().all(|(index, character)| {
                (index == 0 && (character.is_ascii_alphabetic() || character == '_'))
                    || (index > 0 && (character.is_ascii_alphanumeric() || character == '_'))
            })
    };

    if let Some(name) = value.strip_prefix("${").and_then(|v| v.strip_suffix('}')) {
        return valid_name(name);
    }
    value.strip_prefix('$').is_some_and(valid_name)
}

/// Check if a header value is an environment reference (including "Bearer ${TOKEN}" format).
pub fn is_header_environment_reference(value: &str) -> bool {
    let value = value.trim();
    is_environment_reference(value)
        || value
            .strip_prefix("Bearer ")
            .is_some_and(is_environment_reference)
}

/// Classify a value based on its content and the sensitivity of its key.
pub fn classify_value(key: &str, value: &str) -> SecretClassification {
    if is_environment_reference(value) {
        SecretClassification::EnvironmentReference
    } else if is_sensitive_key(key) {
        SecretClassification::OpaqueSensitiveLiteral
    } else {
        SecretClassification::SafeLiteral
    }
}

/// Redact a value based on its classification.
pub fn redact_value(key: &str, value: &str) -> String {
    match classify_value(key, value) {
        SecretClassification::EnvironmentReference => value.to_string(),
        SecretClassification::OpaqueSensitiveLiteral => "<redacted>".to_string(),
        SecretClassification::SafeLiteral => value.to_string(),
    }
}

/// Redact sensitive values in a BTreeMap, preserving environment references.
pub fn redact_map_values(values: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    values
        .iter()
        .map(|(key, value)| (key.clone(), redact_value(key, value)))
        .collect()
}

/// Classify values in a BTreeMap, returning the classification instead of the value.
pub fn classify_map_values(values: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    values
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                match classify_value(key, value) {
                    SecretClassification::EnvironmentReference => "environment-reference".into(),
                    SecretClassification::OpaqueSensitiveLiteral => "redacted-literal".into(),
                    SecretClassification::SafeLiteral => "literal-redacted".into(),
                },
            )
        })
        .collect()
}

/// Redact sensitive arguments in a command line.
pub fn redact_args(args: &[String]) -> Vec<String> {
    let mut redact_next = false;
    args.iter()
        .map(|arg| {
            if redact_next {
                redact_next = false;
                return "<redacted>".into();
            }
            let flag = arg.trim_start_matches('-');
            if flag.starts_with("--") || arg.starts_with('-') {
                let (key, value) = flag.split_once('=').unwrap_or((flag, ""));
                if is_sensitive_key(key) && !value.is_empty() {
                    return format!("--{key}=<redacted>");
                }
                if let Some((key, _)) = flag.split_once(':')
                    && is_sensitive_key(key)
                {
                    return format!("--{key}:<redacted>");
                }
                redact_next = is_sensitive_key(key) && value.is_empty();
                return arg.clone();
            }
            redact_next = false;
            let value = if arg.contains("://") {
                redact_url(arg)
            } else {
                arg.clone()
            };
            if is_environment_reference(&value) {
                "<environment-reference-redacted>".into()
            } else {
                redact_inline_secret(&value)
            }
        })
        .collect()
}

/// Redact an inline secret in key=value or key: value format.
fn redact_inline_secret(value: &str) -> String {
    if let Some((key, _)) = value.split_once('=')
        && is_sensitive_key(key.trim().trim_start_matches('-'))
    {
        return format!("{key}=<redacted>");
    }
    if let Some((key, _)) = value.split_once(':')
        && is_sensitive_key(key.trim().trim_start_matches('-'))
    {
        return format!("{key}: <redacted>");
    }
    value.to_string()
}

/// Redact sensitive information in a URL (credentials in authority, sensitive query params).
pub fn redact_url(value: &str) -> String {
    let (scheme, rest) = value.split_once("://").unwrap_or(("", value));
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, path_query) = rest.split_at(authority_end);

    let safe_authority = if let Some((_, host)) = authority.rsplit_once('@') {
        format!("<redacted>@{host}")
    } else if authority.starts_with('[') {
        authority.to_string()
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        if is_sensitive_key(host) {
            format!("<redacted>:{port}")
        } else {
            authority.to_string()
        }
    } else {
        authority.to_string()
    };
    let base_value = if scheme.is_empty() {
        format!("{safe_authority}{path_query}")
    } else {
        format!("{scheme}://{safe_authority}{path_query}")
    };
    if let Some((base, query)) = base_value.split_once('?') {
        let parts = query
            .split('&')
            .map(|part| {
                let (key, val) = part.split_once('=').unwrap_or((part, ""));
                if is_sensitive_key(key) {
                    format!("{key}=<redacted>")
                } else if is_environment_reference(val) {
                    format!("{key}=<environment-reference-redacted>")
                } else {
                    part.into()
                }
            })
            .collect::<Vec<_>>();
        format!("{base}?{}", parts.join("&"))
    } else {
        base_value
    }
}

/// Resolve a secret reference using environment variables.
pub fn resolve_secret_reference(reference: &str, config: &SecretConfig) -> SecretResolution {
    if !is_environment_reference(reference) {
        return SecretResolution {
            reference: reference.to_string(),
            resolved: false,
            value: None,
            classification: SecretClassification::OpaqueSensitiveLiteral,
        };
    }

    let env_name = reference
        .strip_prefix("${")
        .and_then(|v| v.strip_suffix('}'))
        .or_else(|| reference.strip_prefix('$'))
        .unwrap_or(reference);

    if config.allow_env_resolution
        && let Ok(value) = std::env::var(env_name)
    {
        return SecretResolution {
            reference: reference.to_string(),
            resolved: true,
            value: Some(value),
            classification: SecretClassification::EnvironmentReference,
        };
    }

    SecretResolution {
        reference: reference.to_string(),
        resolved: false,
        value: None,
        classification: SecretClassification::EnvironmentReference,
    }
}

/// Detect plaintext secrets in sensitive contexts and return warnings.
pub fn detect_plaintext_secrets(
    env: &BTreeMap<String, String>,
    headers: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut warnings = Vec::new();

    for (key, value) in env {
        if is_sensitive_key(key) && !is_environment_reference(value) {
            warnings.push(format!(
                "Environment variable '{}' contains a plaintext secret; use an environment reference such as ${{{} }}",
                key,
                key.to_uppercase()
            ));
        }
    }

    for (key, value) in headers {
        if is_sensitive_key(key) && !is_header_environment_reference(value) {
            warnings.push(format!(
                "Header '{}' contains a plaintext secret; use an environment reference such as ${{{} }}",
                key,
                key.to_uppercase()
            ));
        }
    }

    warnings
}

/// Escape a literal `${` sequence to prevent it from being interpreted as a reference.
pub fn escape_literal_dollar_brace(input: &str) -> String {
    input.replace("${", "$$${")
}

/// Unescape a `$$` escape prefix back to a literal `${`.
pub fn unescape_literal_dollar_brace(input: &str) -> String {
    input.replace("$$${", "${")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_sensitive_key_detects_common_patterns() {
        assert!(is_sensitive_key("token"));
        assert!(is_sensitive_key("api_key"));
        assert!(is_sensitive_key("API-KEY"));
        assert!(is_sensitive_key("authorization"));
        assert!(is_sensitive_key("Authorization"));
        assert!(is_sensitive_key("x-api-key"));
        assert!(is_sensitive_key("secret_token"));
        assert!(!is_sensitive_key("username"));
        assert!(!is_sensitive_key("host"));
    }

    #[test]
    fn test_is_environment_reference_valid_formats() {
        assert!(is_environment_reference("${TOKEN}"));
        assert!(is_environment_reference("$TOKEN"));
        assert!(is_environment_reference("${API_KEY}"));
        assert!(is_environment_reference("${_PRIVATE}"));
        assert!(!is_environment_reference("${INVALID-NAME}"));
        assert!(!is_environment_reference("${123INVALID}"));
        assert!(!is_environment_reference("plaintext_value"));
    }

    #[test]
    fn test_classify_value_environment_reference() {
        let classification = classify_value("api_key", "${API_KEY}");
        assert_eq!(classification, SecretClassification::EnvironmentReference);
    }

    #[test]
    fn test_classify_value_opaque_sensitive() {
        let classification = classify_value("api_key", "sk-abc123");
        assert_eq!(classification, SecretClassification::OpaqueSensitiveLiteral);
    }

    #[test]
    fn test_classify_value_safe_literal() {
        let classification = classify_value("host", "localhost");
        assert_eq!(classification, SecretClassification::SafeLiteral);
    }

    #[test]
    fn test_redact_value_preserves_env_refs() {
        let redacted = redact_value("api_key", "${API_KEY}");
        assert_eq!(redacted, "${API_KEY}");
    }

    #[test]
    fn test_redact_value_redacts_sensitive_literals() {
        let redacted = redact_value("api_key", "sk-abc123");
        assert_eq!(redacted, "<redacted>");
    }

    #[test]
    fn test_redact_value_preserves_safe_literals() {
        let redacted = redact_value("host", "localhost");
        assert_eq!(redacted, "localhost");
    }

    #[test]
    fn test_redact_args_sensitive_flags() {
        let args = vec![
            "--api-key".to_string(),
            "sk-abc123".to_string(),
            "--host".to_string(),
            "localhost".to_string(),
        ];
        let redacted = redact_args(&args);
        assert_eq!(redacted[0], "--api-key");
        assert_eq!(redacted[1], "<redacted>");
        assert_eq!(redacted[2], "--host");
        assert_eq!(redacted[3], "localhost");
    }

    #[test]
    fn test_redact_args_inline_sensitive() {
        let args = vec!["--token=sk-abc123".to_string()];
        let redacted = redact_args(&args);
        assert_eq!(redacted[0], "--token=<redacted>");
    }

    #[test]
    fn test_redact_url_credentials_in_authority() {
        let url = "https://user:password@api.example.com/path";
        let redacted = redact_url(url);
        assert!(redacted.contains("<redacted>@api.example.com"));
        assert!(!redacted.contains("password"));
    }

    #[test]
    fn test_redact_url_sensitive_query_params() {
        let url = "https://api.example.com/path?token=sk-abc123&host=localhost";
        let redacted = redact_url(url);
        assert!(redacted.contains("token=<redacted>"));
        assert!(redacted.contains("host=localhost"));
    }

    #[test]
    fn test_redact_url_env_ref_in_query() {
        // Token is a sensitive key, so even env refs get redacted for sensitive keys
        let url = "https://api.example.com/path?token=${API_TOKEN}";
        let redacted = redact_url(url);
        assert!(redacted.contains("token=<redacted>"));
        // Non-sensitive key with env ref shows the env ref indicator
        let url2 = "https://api.example.com/path?host=${API_HOST}";
        let redacted2 = redact_url(url2);
        assert!(redacted2.contains("host=<environment-reference-redacted>"));
    }

    #[test]
    fn test_resolve_secret_reference_valid() {
        unsafe {
            std::env::set_var("TEST_SECRET_CANARY", "test_value_123");
        }
        let config = SecretConfig::default();
        let resolution = resolve_secret_reference("${TEST_SECRET_CANARY}", &config);
        assert_eq!(
            resolution.classification,
            SecretClassification::EnvironmentReference
        );
        assert!(resolution.resolved);
        assert_eq!(resolution.value, Some("test_value_123".to_string()));
        unsafe {
            std::env::remove_var("TEST_SECRET_CANARY");
        }
    }

    #[test]
    fn test_resolve_secret_reference_unresolved() {
        let config = SecretConfig::default();
        let resolution = resolve_secret_reference("${NONEXISTENT_VAR}", &config);
        assert_eq!(
            resolution.classification,
            SecretClassification::EnvironmentReference
        );
        assert!(!resolution.resolved);
        assert!(resolution.value.is_none());
    }

    #[test]
    fn test_resolve_secret_reference_invalid() {
        let config = SecretConfig::default();
        let resolution = resolve_secret_reference("sk-abc123", &config);
        assert_eq!(
            resolution.classification,
            SecretClassification::OpaqueSensitiveLiteral
        );
        assert!(!resolution.resolved);
    }

    #[test]
    fn test_detect_plaintext_secrets_env() {
        let mut env = BTreeMap::new();
        env.insert("api_key".to_string(), "sk-abc123".to_string());
        env.insert("host".to_string(), "localhost".to_string());
        let headers = BTreeMap::new();
        let warnings = detect_plaintext_secrets(&env, &headers);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("api_key"));
    }

    #[test]
    fn test_detect_plaintext_secrets_headers() {
        let env = BTreeMap::new();
        let mut headers = BTreeMap::new();
        headers.insert("Authorization".to_string(), "Bearer sk-abc123".to_string());
        let warnings = detect_plaintext_secrets(&env, &headers);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Authorization"));
    }

    #[test]
    fn test_detect_plaintext_secrets_no_warnings_for_env_refs() {
        let mut env = BTreeMap::new();
        env.insert("api_key".to_string(), "${API_KEY}".to_string());
        let mut headers = BTreeMap::new();
        headers.insert("Authorization".to_string(), "Bearer ${TOKEN}".to_string());
        let warnings = detect_plaintext_secrets(&env, &headers);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_escape_unescape_literal_dollar_brace() {
        let input = "This is a literal ${ENV_NAME} reference";
        let escaped = escape_literal_dollar_brace(input);
        assert_eq!(escaped, "This is a literal $$${ENV_NAME} reference");
        let unescaped = unescape_literal_dollar_brace(&escaped);
        assert_eq!(unescaped, input);
    }

    #[test]
    fn test_header_environment_reference_bearer_format() {
        assert!(is_header_environment_reference("Bearer ${TOKEN}"));
        assert!(is_header_environment_reference("${TOKEN}"));
        assert!(!is_header_environment_reference("Bearer sk-abc123"));
    }

    #[test]
    fn test_secret_canary_never_leaked() {
        // Verify that the test secret canary value never appears in redacted output
        let canary = "TEST_SECRET_CANARY_12345";
        let redacted = redact_value("api_key", canary);
        assert_eq!(redacted, "<redacted>");
        assert!(!redacted.contains(canary));
    }

    #[test]
    fn test_classify_map_values() {
        let mut values = BTreeMap::new();
        values.insert("api_key".to_string(), "${API_KEY}".to_string());
        values.insert("token".to_string(), "sk-abc123".to_string());
        values.insert("host".to_string(), "localhost".to_string());
        let classified = classify_map_values(&values);
        assert_eq!(classified["api_key"], "environment-reference");
        assert_eq!(classified["token"], "redacted-literal");
        assert_eq!(classified["host"], "literal-redacted");
    }

    #[test]
    fn test_redact_map_values() {
        let mut values = BTreeMap::new();
        values.insert("api_key".to_string(), "${API_KEY}".to_string());
        values.insert("token".to_string(), "sk-abc123".to_string());
        values.insert("host".to_string(), "localhost".to_string());
        let redacted = redact_map_values(&values);
        assert_eq!(redacted["api_key"], "${API_KEY}");
        assert_eq!(redacted["token"], "<redacted>");
        assert_eq!(redacted["host"], "localhost");
    }
}
