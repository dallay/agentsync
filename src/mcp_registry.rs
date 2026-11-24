//! MCP Registry Discovery.
//!
//! Provides optional, opt-in connectivity diagnostics for discovering MCP servers
//! from official registries. Network access requires explicit user opt-in per the
//! offline-first principle.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A discovered MCP server from a registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredServer {
    /// Server name/ID.
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: String,
    /// Server version.
    #[serde(default)]
    pub version: String,
    /// Installation command or URL.
    #[serde(default)]
    pub install_command: String,
    /// Source registry.
    pub source: String,
    /// Tags for categorization.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Result of a registry lookup.
#[derive(Debug, Clone, Serialize)]
pub struct RegistryResult {
    /// Whether the lookup was successful.
    pub success: bool,
    /// Discovered servers.
    pub servers: Vec<DiscoveredServer>,
    /// Warnings or errors.
    pub warnings: Vec<String>,
    /// Source registry.
    pub source: String,
}

/// Configuration for registry discovery.
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    /// Whether network access is enabled (opt-in).
    pub network_enabled: bool,
    /// Timeout for network requests in seconds.
    pub timeout_seconds: u64,
    /// Registry URL to query.
    pub registry_url: String,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            network_enabled: false,
            timeout_seconds: 10,
            registry_url: "https://registry.agent-sync.dev/v1/servers".to_string(),
        }
    }
}

/// Check connectivity to a registry endpoint.
pub fn check_connectivity(config: &RegistryConfig) -> Result<bool> {
    if !config.network_enabled {
        return Ok(false);
    }

    // Use std::net to check connectivity without external dependencies
    let url = &config.registry_url;
    let timeout = Duration::from_secs(config.timeout_seconds);

    // Simple connectivity check using DNS resolution
    if let Some(host) = extract_host(url) {
        match std::net::TcpStream::connect_timeout(&format!("{}:443", host).parse()?, timeout) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    } else {
        Ok(false)
    }
}

/// Extract host from a URL.
fn extract_host(url: &str) -> Option<&str> {
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .and_then(|u| u.split('/').next())
        .map(|h| h.split(':').next().unwrap_or(h))
}

/// Discover MCP servers from a registry.
pub fn discover_servers(config: &RegistryConfig, _query: Option<&str>) -> Result<RegistryResult> {
    if !config.network_enabled {
        return Ok(RegistryResult {
            success: false,
            servers: Vec::new(),
            warnings: vec![
                "Network access is disabled. Enable it with --network flag.".to_string(),
            ],
            source: config.registry_url.clone(),
        });
    }

    let connectivity = check_connectivity(config)?;
    if !connectivity {
        return Ok(RegistryResult {
            success: false,
            servers: Vec::new(),
            warnings: vec![format!("Cannot reach registry: {}", config.registry_url)],
            source: config.registry_url.clone(),
        });
    }

    // In a real implementation, this would make an HTTP request to the registry.
    // For now, return a placeholder result indicating the registry is reachable.
    Ok(RegistryResult {
        success: true,
        servers: Vec::new(),
        warnings: vec!["Registry discovery is a placeholder implementation".to_string()],
        source: config.registry_url.clone(),
    })
}

/// Search for MCP servers by name or tag.
pub fn search_servers(config: &RegistryConfig, query: &str) -> Result<RegistryResult> {
    discover_servers(config, Some(query))
}

/// Get information about a specific MCP server.
pub fn get_server_info(
    config: &RegistryConfig,
    server_name: &str,
) -> Result<Option<DiscoveredServer>> {
    if !config.network_enabled {
        return Ok(None);
    }

    let result = discover_servers(config, None)?;
    Ok(result
        .servers
        .iter()
        .find(|s| s.name == server_name)
        .cloned())
}

/// List available registries.
pub fn list_registries() -> Vec<RegistryConfig> {
    vec![
        RegistryConfig {
            network_enabled: false,
            timeout_seconds: 10,
            registry_url: "https://registry.agent-sync.dev/v1/servers".to_string(),
        },
        RegistryConfig {
            network_enabled: false,
            timeout_seconds: 10,
            registry_url: "https://mcp.directory/v1/servers".to_string(),
        },
    ]
}

/// Validate that a server configuration is safe to use.
pub fn validate_server_safety(server: &DiscoveredServer) -> Vec<String> {
    let mut warnings = Vec::new();

    // Check for suspicious install commands
    let cmd = &server.install_command;
    if cmd.contains("curl") && cmd.contains("|") && (cmd.contains("bash") || cmd.contains("sh")) {
        warnings.push(
            "Install command uses pipe-to-shell pattern — verify before executing".to_string(),
        );
    }

    if cmd.contains("eval") {
        warnings.push("Install command uses eval — high risk".to_string());
    }

    if cmd.contains("sudo") {
        warnings.push("Install command requires sudo — elevated privileges".to_string());
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_config_default() {
        let config = RegistryConfig::default();
        assert!(!config.network_enabled);
        assert_eq!(config.timeout_seconds, 10);
    }

    #[test]
    fn test_check_connectivity_disabled() {
        let config = RegistryConfig::default();
        let result = check_connectivity(&config).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_extract_host() {
        assert_eq!(
            extract_host("https://example.com/path"),
            Some("example.com")
        );
        assert_eq!(
            extract_host("http://example.com:443/path"),
            Some("example.com")
        );
        assert_eq!(extract_host("not-a-url"), None);
    }

    #[test]
    fn test_discover_servers_disabled() {
        let config = RegistryConfig::default();
        let result = discover_servers(&config, None).unwrap();
        assert!(!result.success);
        assert!(result.warnings.iter().any(|w| w.contains("disabled")));
    }

    #[test]
    fn test_list_registries() {
        let registries = list_registries();
        assert!(registries.len() >= 2);
        for registry in &registries {
            assert!(!registry.network_enabled);
        }
    }

    #[test]
    fn test_validate_server_safety_safe() {
        let server = DiscoveredServer {
            name: "test".to_string(),
            description: "Test server".to_string(),
            version: "1.0".to_string(),
            install_command: "npm install -g @test/server".to_string(),
            source: "registry".to_string(),
            tags: vec![],
        };
        let warnings = validate_server_safety(&server);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_validate_server_safety_pipe_to_shell() {
        let server = DiscoveredServer {
            name: "test".to_string(),
            description: "Test server".to_string(),
            version: "1.0".to_string(),
            install_command: "curl https://example.com/install.sh | bash".to_string(),
            source: "registry".to_string(),
            tags: vec![],
        };
        let warnings = validate_server_safety(&server);
        assert!(warnings.iter().any(|w| w.contains("pipe-to-shell")));
    }

    #[test]
    fn test_validate_server_safety_eval() {
        let server = DiscoveredServer {
            name: "test".to_string(),
            description: "Test server".to_string(),
            version: "1.0".to_string(),
            install_command: "eval $(curl https://example.com/setup)".to_string(),
            source: "registry".to_string(),
            tags: vec![],
        };
        let warnings = validate_server_safety(&server);
        assert!(warnings.iter().any(|w| w.contains("eval")));
    }

    #[test]
    fn test_validate_server_safety_sudo() {
        let server = DiscoveredServer {
            name: "test".to_string(),
            description: "Test server".to_string(),
            version: "1.0".to_string(),
            install_command: "sudo apt install test-server".to_string(),
            source: "registry".to_string(),
            tags: vec![],
        };
        let warnings = validate_server_safety(&server);
        assert!(warnings.iter().any(|w| w.contains("sudo")));
    }

    #[test]
    fn test_registry_result_serialization() {
        let result = RegistryResult {
            success: true,
            servers: vec![DiscoveredServer {
                name: "test".to_string(),
                description: "Test".to_string(),
                version: "1.0".to_string(),
                install_command: "npm install".to_string(),
                source: "registry".to_string(),
                tags: vec!["test".to_string()],
            }],
            warnings: vec![],
            source: "test-registry".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("test"));
    }

    #[test]
    fn test_search_servers_disabled() {
        let config = RegistryConfig::default();
        let result = search_servers(&config, "test").unwrap();
        assert!(!result.success);
    }

    #[test]
    fn test_get_server_info_disabled() {
        let config = RegistryConfig::default();
        let result = get_server_info(&config, "test").unwrap();
        assert!(result.is_none());
    }
}
