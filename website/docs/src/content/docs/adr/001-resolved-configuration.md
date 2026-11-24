# ADR-001: ResolvedConfiguration Contract, Provenance, and Capability Boundaries

**Status:** Accepted  
**Date:** 2026-11-23  
**Issue:** #701  
**Epic:** #686  
**Unblocks:** #689, #690, #693, #694, #696, #698

## Context

AgentSync needs a shared, read-only, provenance-aware representation of what it **intends** to manage. This contract enables inspection (#689), profiles (#690), native adapters (#693), policy packs (#694), and imports (#696) to share deterministic semantics without premature vendor-specific implementation.

The existing `src/inspect.rs` module already implements a working `ResolvedConfiguration` type. This ADR formalizes that implementation as the canonical contract.

## Decision

### ResolvedConfiguration Schema

The `ResolvedConfiguration` struct in `src/inspect.rs` is the canonical representation:

```rust
pub struct ResolvedConfiguration {
    pub schema_version: u32,      // Currently 1
    pub scope: String,            // "agentsync-managed"
    pub agent: String,            // Agent identifier
    pub items: Vec<ManagedItem>, // Configured targets
    pub mcp: McpReport,          // MCP server status
    pub diagnostics: Vec<Diagnostic>, // Issues found
    pub scope_notice: String,    // Scope disclaimer
}
```

### Item States

Each managed item has one of five states:

| State | Meaning |
|-------|---------|
| `configured` | Source does not exist yet |
| `projected` | Source exists, destination not materialized |
| `observed` | Source exists, destination matches projection |
| `unsupported` | Agent/target combination not supported |
| `conflicted` | Source exists, destination differs from projection |

### Provenance Model

Every item carries explicit provenance:
- `origin`: Where the configuration came from (`"config"` or `"plugin"`)
- `owner`: Who owns the configuration (`"agentsync"` or plugin identifier)
- `provider_support`: Whether the provider supports this target (`"unknown"` means not asserted)

### Capability Boundaries

**Config-internal fields** (no cross-agent semantic mapping needed):
- `kind`, `target`, `sync_mode`, `source`, `destination`
- `state`, `observed`, `children`

**Fields requiring future typed capability adapter** (#693):
- Agent roles/subagents
- Permissions
- Hooks
- Provider-specific semantics

### Source-of-Truth and Merge Precedence

1. Explicit `agentsync.toml` declarations have highest priority
2. Plugin contributions are secondary and require approval
3. Current materialized files take precedence over projections
4. Agent filters and disabled entries are respected
5. MCP name collisions between explicit and plugin declarations are errors

### Error/Warning Behavior

| Condition | Severity | Code |
|-----------|----------|------|
| Agent not configured | Warning | `AgentNotConfigured` |
| Missing source | Error | `MissingSource` |
| Drifted symlink | Warning | `DriftedSymlink` |
| Broken symlink | Error | `BrokenSymlink` |
| Destination collision | Error | `DestinationCollision` |
| MCP name collision | Error | `McpNameCollision` |
| Unsupported target | Warning | `UnsupportedTarget` |
| Missing plugin snapshot | Warning | `MissingPluginSnapshot` |
| Disabled MCP server | Info | `DisabledMcpServer` |
| Pending plugin approval | Warning | `PendingPluginApproval` |
| Unsupported plugin component | Warning | `UnsupportedPluginComponent` |
| Existing MCP server | Info/Warning | `ExistingMcpServer` |
| Disabled agent | Info | `DisabledAgent` |

### Relative-Path Portability

All paths in `ResolvedConfiguration` are relative to the project root, using forward slashes regardless of platform. The `portable_path()` function in `src/inspect.rs` handles this conversion.

### Schema Versioning

The `schema_version` field allows future breaking changes while maintaining backward compatibility for consumers. Current version is 1.

### Redaction Hooks

All sensitive values (secrets, credentials, tokens) are redacted before serialization:
- Environment variables: classified as `"environment-reference"` or `"redacted-literal"`
- Headers: same classification
- URLs: credentials in authority component redacted
- Command args: sensitive flags and values redacted

See ADR-002 for the complete redaction contract.

## Scope Notice

**AgentSync-managed effective configuration is NOT vendor runtime-effective configuration.** User, global, organization, and session controls may be outside project ownership. The `scope_notice` field always contains this disclaimer.

## Dependency Map

| Feature | Can Implement Now | Belongs to Future |
|---------|-------------------|-------------------|
| `inspect` command | ✅ Uses ResolvedConfiguration directly | - |
| Profiles (#690) | ✅ Can layer on ResolvedConfiguration | - |
| Native adapters (#693) | ❌ Requires capability adapter contract | Full translation |
| Imports (#696) | ✅ Can use ResolvedConfiguration for diffs | Provider-specific parsing |
| Policy packs (#694) | ✅ Can validate against ResolvedConfiguration | - |

## Security Review

- No sensitive environment or literal secret values are exposed in diagnostic material
- All redaction happens before serialization
- Plugin MCP contributions are checked against approval lists
- No network access during resolution
- No script execution during resolution

## Consequences

### Positive
- All inspection, profiling, and policy features share one deterministic model
- Clear separation between AgentSync-managed and vendor runtime configuration
- Versioned schema allows evolution without breaking consumers
- Provenance tracking enables audit trails

### Negative
- Consumers must handle all five item states
- Redaction means some values cannot be used directly for runtime configuration
- Scope limitations mean this cannot replace vendor runtime introspection

## References

- `src/inspect.rs` - Implementation
- `src/config.rs` - Configuration types
- `src/plugins.rs` - Plugin provenance
- `src/mcp.rs` - MCP formatting
- `src/commands/inspect.rs` - CLI command
- ADR-002 - Secret reference and redaction contract
