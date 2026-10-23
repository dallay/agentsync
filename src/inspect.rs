//! Deterministic projection of configuration owned by AgentSync.
//!
//! This is not a snapshot of provider runtime state. Global, organization,
//! account, session, and other vendor-managed overlays are outside this model.

use crate::Linker;
use crate::config::{Config, McpServerConfig, SyncType};
use crate::mcp::McpAgent;
use crate::plugins::PluginInspectError;
use crate::status::{
    AgentStatusEntry, DestinationKind, StatusIssueKind, collect_agent_status_entries, paths_match,
    sync_type_label,
};
use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Component, Path};

pub const INSPECT_SCHEMA_VERSION: u32 = 1;
pub const INSPECT_SCOPE: &str = "agentsync-managed";
pub const SCOPE_NOTICE: &str = "This report describes only AgentSync-managed configuration. It is not a claim about full native runtime-effective settings; global, user, organization, and session overlays outside AgentSync's control are unknown.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ItemState {
    Configured,
    Projected,
    Observed,
    Unsupported,
    Conflicted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticCode {
    AgentNotConfigured,
    MissingSource,
    DriftedSymlink,
    BrokenSymlink,
    DestinationCollision,
    McpNameCollision,
    UnsupportedTarget,
    MissingPluginSnapshot,
    DisabledMcpServer,
    PendingPluginApproval,
    UnsupportedPluginComponent,
    ExistingMcpServer,
    DisabledAgent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub target: Option<String>,
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ObservedState {
    pub exists: bool,
    pub kind: String,
    pub points_to: Option<String>,
    pub matches_projection: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedChild {
    pub source: String,
    pub destination: String,
    pub state: ItemState,
    pub observed: ObservedState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedItem {
    pub kind: String,
    pub target: String,
    pub sync_mode: String,
    pub source: String,
    pub destination: String,
    pub origin: String,
    pub owner: String,
    /// `unknown` means the provider-native semantics are not asserted here.
    pub provider_support: String,
    pub state: ItemState,
    pub observed: ObservedState,
    pub children: Vec<ManagedChild>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpServerReport {
    pub name: String,
    pub origin: String,
    pub owner: Option<String>,
    pub disabled: bool,
    pub included: bool,
    pub state: ItemState,
    pub transport: Option<String>,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
    /// Environment variable names and value classifications only. Values are never emitted.
    pub environment: BTreeMap<String, String>,
    /// Header names and value classifications only. Values are never emitted.
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpReport {
    pub capability: String,
    pub enabled: bool,
    pub merge_strategy: String,
    pub destination: Option<String>,
    pub servers: Vec<McpServerReport>,
    pub existing_server_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedConfiguration {
    pub schema_version: u32,
    pub scope: String,
    pub agent: String,
    pub items: Vec<ManagedItem>,
    pub mcp: McpReport,
    pub diagnostics: Vec<Diagnostic>,
    pub scope_notice: String,
}

/// Resolve one configured assistant without writing files, expanding environment variables,
/// downloading plugin snapshots, or invoking child processes.
pub fn resolve_configuration(
    config: &Config,
    config_path: &Path,
    agent_id: &str,
) -> Result<ResolvedConfiguration> {
    let requested = agent_id.trim().to_ascii_lowercase();
    let agent_name = config
        .agents
        .keys()
        .find(|name| name.to_ascii_lowercase() == requested)
        .cloned()
        .unwrap_or_else(|| requested.clone());
    let project_root = Config::project_root(config_path);
    let linker = Linker::new(config.clone(), config_path.to_path_buf());
    let mut diagnostics = Vec::new();
    let Some(agent) = config.agents.get(&agent_name) else {
        diagnostics.push(Diagnostic { code: DiagnosticCode::AgentNotConfigured, severity: DiagnosticSeverity::Warning, target: None, path: None, message: "Agent is not configured in agentsync.toml; AgentSync-managed configuration is unknown.".into() });
        return Ok(ResolvedConfiguration {
            schema_version: INSPECT_SCHEMA_VERSION,
            scope: INSPECT_SCOPE.into(),
            agent: agent_name,
            items: vec![],
            mcp: empty_mcp_report(),
            diagnostics,
            scope_notice: SCOPE_NOTICE.into(),
        });
    };

    let status = collect_agent_status_entries(&linker, config_path)?;
    let source_root = config.source_dir(config_path);
    let mut items = Vec::new();
    let mut destinations = BTreeMap::<String, String>::new();
    if !agent.enabled {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::DisabledAgent,
            severity: DiagnosticSeverity::Info,
            target: None,
            path: None,
            message: "Agent is disabled in agentsync.toml; its targets are not projected.".into(),
        });
    }
    if agent.enabled {
        let mut context = TargetResolutionContext {
            agent_name: &agent_name,
            project_root: &project_root,
            source_root: &source_root,
            status: &status,
            destinations: &mut destinations,
            diagnostics: &mut diagnostics,
        };
        for (target_name, target) in &agent.targets {
            items.push(resolve_target(target_name, target, &mut context));
        }
    }
    items.sort_by(|a, b| {
        (&a.kind, &a.target, &a.source, &a.destination).cmp(&(
            &b.kind,
            &b.target,
            &b.source,
            &b.destination,
        ))
    });

    let mcp = resolve_mcp(
        config,
        config_path,
        &agent_name,
        &project_root,
        &mut diagnostics,
    )?;
    diagnostics.sort_by(|a, b| {
        (&a.code, &a.target, &a.path, &a.message).cmp(&(&b.code, &b.target, &b.path, &b.message))
    });
    Ok(ResolvedConfiguration {
        schema_version: INSPECT_SCHEMA_VERSION,
        scope: INSPECT_SCOPE.into(),
        agent: agent_name,
        items,
        mcp,
        diagnostics,
        scope_notice: SCOPE_NOTICE.into(),
    })
}

struct TargetResolutionContext<'a> {
    agent_name: &'a str,
    project_root: &'a Path,
    source_root: &'a Path,
    status: &'a [AgentStatusEntry],
    destinations: &'a mut BTreeMap<String, String>,
    diagnostics: &'a mut Vec<Diagnostic>,
}

fn resolve_target(
    target_name: &String,
    target: &crate::config::TargetConfig,
    context: &mut TargetResolutionContext<'_>,
) -> ManagedItem {
    let agent_name = context.agent_name;
    let project_root = context.project_root;
    let source_root = context.source_root;
    let status = context.status;
    let destinations = &mut *context.destinations;
    let diagnostics = &mut *context.diagnostics;
    let kind = classify_target(target_name);
    let is_module_map = target.sync_type == SyncType::ModuleMap;
    let source = if target.sync_type == SyncType::NestedGlob {
        project_root.join(&target.source)
    } else {
        source_root.join(&target.source)
    };
    let mappings = if is_module_map {
        target
            .mappings
            .iter()
            .map(|mapping| {
                (
                    source_root.join(&mapping.source),
                    project_root.join(&mapping.destination).join(
                        crate::config::resolve_module_map_filename(mapping, agent_name),
                    ),
                )
            })
            .collect::<Vec<_>>()
    } else {
        vec![(source.clone(), project_root.join(&target.destination))]
    };
    let (projection_source, projection_destination) = mappings
        .first()
        .cloned()
        .unwrap_or_else(|| (source.clone(), project_root.join(&target.destination)));
    let portable_source = portable_path(project_root, &projection_source);
    let portable_destination = portable_path(project_root, &projection_destination);
    for (_, mapped_destination) in &mappings {
        let mapped_destination = portable_path(project_root, mapped_destination);
        if let Some(previous) = destinations.insert(mapped_destination.clone(), target_name.clone())
        {
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::DestinationCollision,
                severity: DiagnosticSeverity::Error,
                target: Some(target_name.clone()),
                path: Some(mapped_destination),
                message: format!(
                    "Targets {previous} and {target_name} project to the same destination."
                ),
            });
        }
    }
    let matched = status
        .iter()
        .filter(|entry| entry.agent == agent_name && entry.target == *target_name)
        .collect::<Vec<_>>();
    let observed = observed_for(&matched);
    let source_exists = mappings
        .iter()
        .all(|(mapped_source, _)| mapped_source.exists());
    let destination_occupied = target.sync_type == SyncType::Symlink
        && std::fs::symlink_metadata(&projection_destination).is_ok();
    let mut state = if !source_exists {
        ItemState::Configured
    } else if observed.matches_projection == Some(true) {
        ItemState::Observed
    } else if observed.exists || destination_occupied {
        ItemState::Conflicted
    } else {
        ItemState::Projected
    };
    if !source_exists {
        let missing_source = mappings
            .iter()
            .find(|(mapped_source, _)| !mapped_source.exists())
            .map(|(mapped_source, _)| portable_path(project_root, mapped_source))
            .unwrap_or_else(|| portable_source.clone());
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::MissingSource,
            severity: DiagnosticSeverity::Error,
            target: Some(target_name.clone()),
            path: Some(missing_source),
            message: "Configured source does not exist.".into(),
        });
    }
    for (index, entry) in matched.iter().enumerate() {
        let entry_state = if is_module_map {
            let source_exists = mappings
                .get(index)
                .is_some_and(|(source, _)| source.exists());
            if !source_exists {
                ItemState::Configured
            } else if entry.entry.issues.is_empty() && entry.entry.exists {
                ItemState::Observed
            } else if entry.entry.exists {
                ItemState::Conflicted
            } else {
                ItemState::Projected
            }
        } else {
            state
        };
        diagnostics.extend(status_diagnostics(
            entry,
            target_name,
            project_root,
            entry_state,
        ));
        if entry.entry.destination_kind == DestinationKind::Symlink
            && entry.entry.points_to.as_deref().is_some_and(|target| {
                let target = Path::new(target);
                let resolved = if target.is_absolute() {
                    target.to_path_buf()
                } else {
                    Path::new(&entry.entry.destination)
                        .parent()
                        .unwrap_or(Path::new("."))
                        .join(target)
                };
                !resolved.exists()
            })
        {
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::BrokenSymlink,
                severity: DiagnosticSeverity::Error,
                target: Some(target_name.clone()),
                path: Some(portable_path(
                    project_root,
                    Path::new(&entry.entry.destination),
                )),
                message: "Destination is a broken or unreadable symlink.".into(),
            });
            state = ItemState::Conflicted;
        }
    }
    let mut children = Vec::new();
    if is_module_map {
        for ((mapped_source, mapped_destination), entry) in mappings.iter().zip(matched.iter()) {
            let exists = mapped_source.exists();
            let matches = entry.entry.issues.is_empty() && entry.entry.exists;
            children.push(ManagedChild {
                source: portable_path(project_root, mapped_source),
                destination: portable_path(project_root, mapped_destination),
                state: if !exists {
                    ItemState::Configured
                } else if matches {
                    ItemState::Observed
                } else if entry.entry.exists {
                    ItemState::Conflicted
                } else {
                    ItemState::Projected
                },
                observed: ObservedState {
                    exists: entry.entry.exists,
                    kind: format!("{:?}", entry.entry.destination_kind).to_ascii_lowercase(),
                    points_to: entry
                        .entry
                        .points_to
                        .as_ref()
                        .map(|path| portable_path(project_root, Path::new(path))),
                    matches_projection: Some(matches),
                },
            });
        }
    } else if let Some(entry) = matched
        .iter()
        .find(|entry| entry.entry.managed_children.is_some())
        && let Some(entry) = entry.entry.managed_children.as_ref()
    {
        for child in entry {
            let child_source = portable_path(project_root, Path::new(&child.expected_source));
            let child_destination = portable_path(project_root, Path::new(&child.path));
            let child_matches = child.exists
                && child.is_symlink
                && child.points_to.as_deref().is_some_and(|actual| {
                    paths_match(
                        Path::new(&child.path),
                        Path::new(actual),
                        Path::new(&child.expected_source),
                    )
                });
            let child_state = if !child.exists {
                ItemState::Projected
            } else if child_matches {
                ItemState::Observed
            } else {
                ItemState::Conflicted
            };
            children.push(ManagedChild {
                source: child_source,
                destination: child_destination,
                state: child_state,
                observed: ObservedState {
                    exists: child.exists,
                    kind: if child.is_symlink {
                        "symlink"
                    } else if child.exists {
                        "other"
                    } else {
                        "missing"
                    }
                    .into(),
                    points_to: child
                        .points_to
                        .as_deref()
                        .map(|path| portable_path(project_root, Path::new(path))),
                    matches_projection: Some(child_matches),
                },
            });
        }
    }
    ManagedItem {
        kind,
        target: target_name.clone(),
        sync_mode: sync_type_label(target.sync_type).into(),
        source: portable_source,
        destination: portable_destination,
        origin: "config".into(),
        owner: "agentsync".into(),
        provider_support: "unknown".into(),
        state,
        observed,
        children,
    }
}

fn empty_mcp_report() -> McpReport {
    McpReport {
        capability: "not-configured".into(),
        enabled: false,
        merge_strategy: "merge".into(),
        destination: None,
        servers: vec![],
        existing_server_names: vec![],
    }
}

fn resolve_mcp(
    config: &Config,
    config_path: &Path,
    agent_id: &str,
    root: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<McpReport> {
    let Some(agent) = McpAgent::from_id(agent_id) else {
        if config.mcp.enabled
            && (!config.mcp_servers.is_empty() || !config.plugins.selections.is_empty())
        {
            diagnostics.push(Diagnostic { code: DiagnosticCode::UnsupportedTarget, severity: DiagnosticSeverity::Warning, target: Some("mcp".into()), path: None, message: "This assistant has no AgentSync MCP formatter; configured MCP servers are not projected for it.".into() });
        }
        return Ok(McpReport {
            capability: if config.mcp.enabled
                && (!config.mcp_servers.is_empty() || !config.plugins.selections.is_empty())
            {
                "unsupported"
            } else {
                "not-configured"
            }
            .into(),
            enabled: config.mcp.enabled,
            merge_strategy: match config.mcp.merge_strategy {
                crate::config::McpMergeStrategy::Merge => "merge",
                crate::config::McpMergeStrategy::Overwrite => "overwrite",
            }
            .into(),
            destination: None,
            servers: vec![],
            existing_server_names: vec![],
        });
    };
    let Some(path) = agent.resolved_config_path(root) else {
        return Ok(empty_mcp_report());
    };
    let dest = portable_path(root, &path);
    let existing_content = std::fs::read_to_string(&path).ok();
    let existing_servers = if let Some(content) = existing_content.as_deref() {
        agent
            .formatter()
            .parse_existing(content)
            .unwrap_or_default()
    } else {
        BTreeMap::new()
    };
    let existing_names = existing_servers.keys().cloned().collect::<Vec<_>>();
    let projected = config
        .mcp_servers
        .iter()
        .map(|(name, server)| (name.as_str(), server))
        .collect::<BTreeMap<_, _>>();
    let formatter = agent.formatter();
    let formatted = formatter.format_to_string(&projected)?;
    let formatted_servers = formatter.parse_existing(&formatted).unwrap_or_default();
    let mut servers = Vec::new();
    if !config.mcp.enabled
        && (!config.mcp_servers.is_empty() || !config.plugins.selections.is_empty())
    {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::DisabledMcpServer,
            severity: DiagnosticSeverity::Info,
            target: Some("mcp".into()),
            path: Some(dest.clone()),
            message: "MCP generation is disabled by [mcp].enabled; configured declarations are not projected.".into(),
        });
    }
    if config.mcp.enabled {
        for (name, server) in &config.mcp_servers {
            let existing_equal = match (existing_servers.get(name), formatted_servers.get(name)) {
                (Some(existing), Some(expected)) => {
                    normalized_mcp_value(existing) == normalized_mcp_value(expected)
                }
                _ => false,
            };
            let mut server_report =
                mcp_server_report(name, server, "config", None, existing_equal, root, &dest);
            server_report.state = if server.disabled {
                ItemState::Configured
            } else if existing_equal {
                ItemState::Observed
            } else {
                ItemState::Projected
            };
            servers.push(server_report);
            if existing_servers.contains_key(name) && !existing_equal && !server.disabled {
                diagnostics.push(Diagnostic {
                    code: DiagnosticCode::ExistingMcpServer,
                    severity: DiagnosticSeverity::Warning,
                    target: Some("mcp".into()),
                    path: Some(dest.clone()),
                    message: format!("Managed server {name} replaces a differing destination entry under the current merge rules."),
                });
            }
            if server.disabled {
                diagnostics.push(Diagnostic {
                    code: DiagnosticCode::DisabledMcpServer,
                    severity: DiagnosticSeverity::Info,
                    target: Some("mcp".into()),
                    path: Some(dest.clone()),
                    message: format!(
                        "Explicit MCP server {name} is disabled and is not generated."
                    ),
                });
            }
        }
        append_plugin_mcp_servers(
            PluginMcpContext {
                config,
                config_path,
                agent,
                root,
                destination: &dest,
                existing_servers: &existing_servers,
            },
            &mut servers,
            diagnostics,
        )?;
    }
    servers.sort_by(|a, b| a.name.cmp(&b.name));
    let existing_unmanaged = existing_names
        .into_iter()
        .filter(|name| {
            config.mcp.merge_strategy == crate::config::McpMergeStrategy::Merge
                && !servers.iter().any(|server| server.name == *name)
        })
        .collect::<Vec<_>>();
    for name in &existing_unmanaged {
        diagnostics.push(Diagnostic { code: DiagnosticCode::ExistingMcpServer, severity: DiagnosticSeverity::Info, target: Some("mcp".into()), path: Some(dest.clone()), message: format!("Existing destination contains unmanaged MCP server {name}; merge preserves it unless an AgentSync server with the same name replaces it." ) });
    }
    if !config.mcp.enabled {
        servers.clear();
    }
    Ok(McpReport {
        capability: if config.mcp.enabled
            && (!config.mcp_servers.is_empty() || !config.plugins.selections.is_empty())
        {
            "managed"
        } else {
            "not-configured"
        }
        .into(),
        enabled: config.mcp.enabled,
        merge_strategy: match config.mcp.merge_strategy {
            crate::config::McpMergeStrategy::Merge => "merge",
            crate::config::McpMergeStrategy::Overwrite => "overwrite",
        }
        .into(),
        destination: Some(dest),
        servers,
        existing_server_names: existing_unmanaged,
    })
}

struct PluginMcpContext<'a> {
    config: &'a Config,
    config_path: &'a Path,
    agent: McpAgent,
    root: &'a Path,
    destination: &'a str,
    existing_servers: &'a BTreeMap<String, serde_json::Value>,
}

fn append_plugin_mcp_servers(
    context: PluginMcpContext<'_>,
    servers: &mut Vec<McpServerReport>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<()> {
    let PluginMcpContext {
        config,
        config_path,
        agent,
        root,
        destination,
        existing_servers,
    } = context;
    let dest = destination;
    let plugins = &config.plugins;
    if !plugins.enabled || plugins.selections.is_empty() {
        return Ok(());
    }
    let manager = crate::plugins::PluginManager::new(
        root.to_path_buf(),
        config_path.to_path_buf(),
        plugins.clone(),
    );
    let formatter = agent.formatter();
    match manager.inspect_status_report_read_only() {
        Ok(report) => {
            for item in report.servers {
                let approved = matches!(item.approval, crate::plugins::PluginMcpApproval::Allowed);
                let collision = config
                    .mcp_servers
                    .get(&item.name)
                    .is_some_and(|explicit| explicit != &item.server);
                if collision {
                    diagnostics.push(Diagnostic { code: DiagnosticCode::McpNameCollision, severity: DiagnosticSeverity::Error, target: Some("mcp".into()), path: Some(dest.into()), message: format!("Explicit MCP declaration and plugin contribution collide at {}; apply rejects differing definitions.", item.name) });
                }
                let plugin_expected = formatter
                    .format_to_string(&BTreeMap::from([(item.name.as_str(), &item.server)]))
                    .ok()
                    .and_then(|formatted| formatter.parse_existing(&formatted).ok())
                    .and_then(|servers| servers.get(&item.name).cloned());
                let existing_equal = existing_servers
                    .get(&item.name)
                    .zip(plugin_expected.as_ref())
                    .is_some_and(|(existing, expected)| {
                        normalized_mcp_value(existing) == normalized_mcp_value(expected)
                    });
                let mut server_report = mcp_server_report(
                    &item.name,
                    &item.server,
                    "plugin",
                    Some(plugin_owner(&item.name)),
                    existing_equal,
                    root,
                    dest,
                );
                server_report.included = approved && !collision && !item.server.disabled;
                server_report.state = if collision {
                    ItemState::Conflicted
                } else if item.server.disabled || !approved {
                    ItemState::Configured
                } else if existing_equal {
                    ItemState::Observed
                } else {
                    ItemState::Projected
                };
                servers.push(server_report);
                if !approved {
                    diagnostics.push(Diagnostic { code: DiagnosticCode::PendingPluginApproval, severity: DiagnosticSeverity::Warning, target: Some("mcp".into()), path: Some(dest.into()), message: format!("Plugin MCP server {} is not approved by plugins.allowed_mcp and is not included.", item.name) });
                }
            }
        }
        Err(error) => {
            let missing_snapshot = matches!(
                error.downcast_ref::<PluginInspectError>(),
                Some(PluginInspectError::MissingSnapshot)
            );
            let path = manager
                .lock_path()
                .ok()
                .map(|path| portable_path(root, &path));
            diagnostics.push(Diagnostic { code: if missing_snapshot { DiagnosticCode::MissingPluginSnapshot } else { DiagnosticCode::UnsupportedPluginComponent }, severity: DiagnosticSeverity::Warning, target: Some("plugins".into()), path, message: if missing_snapshot { "A required local plugin snapshot is unavailable; plugin MCP contributions are unknown.".into() } else { format!("Plugin contributions could not be resolved offline: {}", error.to_string().split(':').next().unwrap_or("plugin metadata error")) } });
        }
    }
    Ok(())
}

fn normalized_mcp_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) => {
            let normalized = object
                .iter()
                .filter(|(_, value)| {
                    !value.is_null()
                        && !matches!(value, serde_json::Value::Array(values) if values.is_empty())
                        && !matches!(value, serde_json::Value::Object(values) if values.is_empty())
                })
                .map(|(key, value)| (key.clone(), normalized_mcp_value(value)))
                .collect::<serde_json::Map<_, _>>();
            serde_json::Value::Object(normalized)
        }
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(normalized_mcp_value).collect())
        }
        _ => value.clone(),
    }
}

fn mcp_server_report(
    name: &str,
    server: &McpServerConfig,
    origin: &str,
    owner: Option<String>,
    exists: bool,
    root: &Path,
    destination: &str,
) -> McpServerReport {
    let mut report = McpServerReport {
        name: name.into(),
        origin: origin.into(),
        owner,
        disabled: server.disabled,
        included: !server.disabled,
        state: if server.disabled {
            ItemState::Configured
        } else if exists {
            ItemState::Observed
        } else {
            ItemState::Projected
        },
        transport: server.transport_type.clone(),
        command: server.command.as_deref().map(|command| {
            redact_args(&[command.to_string()])
                .into_iter()
                .next()
                .unwrap_or_default()
        }),
        args: redact_args(&server.args),
        url: server.url.as_deref().map(redact_url),
        environment: classify_values(&server.env),
        headers: classify_values(&server.headers),
    };
    if exists && !server.disabled {
        report.state = ItemState::Observed;
    }
    let _ = (root, destination);
    report
}

fn classify_values(values: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    values
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                if crate::plugins::is_environment_reference(value) {
                    "environment-reference".into()
                } else if is_sensitive_key(key) {
                    "redacted-literal".into()
                } else {
                    "literal-redacted".into()
                },
            )
        })
        .collect()
}
fn is_sensitive_key(key: &str) -> bool {
    crate::plugins::is_sensitive_mcp_key(key)
}
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

fn redact_args(args: &[String]) -> Vec<String> {
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
            if crate::plugins::is_environment_reference(&value) {
                "<environment-reference-redacted>".into()
            } else {
                redact_inline_secret(&value)
            }
        })
        .collect()
}
fn redact_url(value: &str) -> String {
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
                } else if crate::plugins::is_environment_reference(val) {
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
fn plugin_owner(name: &str) -> String {
    let parts = name.split('/').collect::<Vec<_>>();
    if parts.len() >= 4 {
        format!("{}/{}", parts[1], parts[2])
    } else {
        "plugin".into()
    }
}

fn status_diagnostics(
    entry: &AgentStatusEntry,
    target: &str,
    root: &Path,
    state: ItemState,
) -> Vec<Diagnostic> {
    entry
        .entry
        .issues
        .iter()
        .filter(|issue| {
            !(state == ItemState::Projected
                && matches!(
                    issue.kind,
                    StatusIssueKind::MissingExpectedSource | StatusIssueKind::MissingDestination
                ))
        })
        .map(|issue| {
            let code = match issue.kind {
                StatusIssueKind::IncorrectLinkTarget => DiagnosticCode::DriftedSymlink,
                StatusIssueKind::MissingExpectedSource => DiagnosticCode::MissingSource,
                _ => DiagnosticCode::DriftedSymlink,
            };
            Diagnostic {
                code,
                severity: DiagnosticSeverity::Warning,
                target: Some(target.into()),
                path: Some(portable_path(root, Path::new(&issue.path))),
                message: format!(
                    "Materialized destination differs from the configured projection ({:?}).",
                    issue.kind
                ),
            }
        })
        .collect()
}
fn observed_for(entries: &[&AgentStatusEntry]) -> ObservedState {
    let Some(entry) = entries
        .iter()
        .find(|entry| {
            entry.entry.sync_type != "module-map" || entry.entry.managed_children.is_some()
        })
        .or_else(|| entries.first())
    else {
        return ObservedState {
            exists: false,
            kind: "missing".into(),
            points_to: None,
            matches_projection: None,
        };
    };
    let e = &entry.entry;
    let matches = entries
        .iter()
        .all(|entry| entry.entry.issues.is_empty() && entry.entry.exists);
    ObservedState {
        exists: entries.iter().any(|entry| entry.entry.exists),
        kind: format!("{:?}", e.destination_kind).to_ascii_lowercase(),
        points_to: e.points_to.clone(),
        matches_projection: Some(matches),
    }
}
fn classify_target(name: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "instructions" | "agents" | "agent" => "instructions",
        "skill" | "skills" => "skills",
        "command" | "commands" => "commands",
        _ => "other",
    }
    .into()
}
fn portable_path(root: &Path, path: &Path) -> String {
    let value = path.strip_prefix(root).unwrap_or(path);
    let mut result = String::new();
    for component in value.components() {
        match component {
            Component::Normal(part) => {
                if !result.is_empty() && !result.ends_with('/') {
                    result.push('/');
                }
                result.push_str(&part.to_string_lossy());
            }
            Component::CurDir => {
                if !result.is_empty() && !result.ends_with('/') {
                    result.push('/');
                }
                result.push('.');
            }
            Component::ParentDir => {
                if !result.is_empty() && !result.ends_with('/') {
                    result.push('/');
                }
                result.push_str("..");
            }
            Component::RootDir => result.push('/'),
            Component::Prefix(prefix) => result.push_str(&prefix.as_os_str().to_string_lossy()),
        }
    }
    result
}
