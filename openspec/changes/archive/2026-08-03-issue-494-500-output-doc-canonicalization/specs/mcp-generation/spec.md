# Delta for MCP Configuration Generation

## ADDED Requirements

### Requirement: Canonical Typed Native MCP Metadata

The system MUST expose one typed canonical registry for native MCP agents. Each registry entry
MUST define the canonical ID, display name, documented destination, format classification, global
path status, and documentation notes. The registry MUST include Claude Desktop and MUST distinguish
native MCP support from configurable-only agent support. Runtime generation behavior, defaults,
filters, aliases, and OS-specific path resolution MUST NOT change solely because metadata is
centralized.

#### Scenario: Registry represents every native MCP agent

- GIVEN the native MCP registry is enumerated
- WHEN documentation metadata is requested
- THEN Claude Desktop and every currently supported native MCP agent MUST appear exactly once
- AND each entry MUST have non-empty canonical ID, name, format, and path metadata

#### Scenario: Shared and global destinations remain explicit

- GIVEN Copilot and VS Code share `.vscode/mcp.json`
- AND Claude Desktop uses an OS-dependent global destination
- WHEN metadata is rendered
- THEN the shared path and global-path distinction MUST be retained

#### Scenario: Configurable-only agents are not mislabeled

- GIVEN an agent is supported for generic synchronization but not native MCP generation
- WHEN supported-agent documentation is produced
- THEN it MUST NOT be listed as a native MCP agent

### Requirement: MCP Documentation Drift Is Validated

The project MUST generate or deterministically validate marked native-MCP documentation fragments in
the repository README, npm README, MCP guide, and MCP OpenSpec. CI MUST run this check as a focused
validation and fail when a governed fragment differs from the canonical registry. Manual
keep-in-sync instructions MUST be removed.

#### Scenario: Documentation matches metadata

- GIVEN all governed documentation fragments were produced from the canonical registry
- WHEN the documentation validation runs
- THEN CI MUST pass

#### Scenario: Drift blocks CI

- GIVEN a governed MCP ID, name, destination, format, or Claude Desktop row is stale or missing
- WHEN the validation runs
- THEN CI MUST fail and identify the drifted documentation fragment
