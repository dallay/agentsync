# Plugin Materialization

## Requirement: Locked project sources

AgentSync MUST require a valid project plugin lockfile before applying enabled selections. The lock
MUST contain an immutable Git commit or local content revision and content hashes.

### Scenario: Offline apply

- GIVEN a selected plugin with a valid local lock entry
- WHEN `agentsync apply` runs without network access
- THEN AgentSync verifies the source and materializes the locked content
- AND it MUST NOT resolve a new reference

### Scenario: Drift is rejected

- GIVEN a source or installed skill whose content differs from the lock
- WHEN apply or status runs
- THEN AgentSync reports drift and MUST NOT replace unmanaged content

## Requirement: Safe supported materialization

AgentSync MUST materialize only conventional skills and standard `.mcp.json` declarations. It MUST
reject unsupported lifecycle components and MUST NOT execute plugin content.

### Scenario: Skill fan-out

- GIVEN a locked plugin containing `skills/review/SKILL.md`
- WHEN apply succeeds
- THEN `.agents/skills/review/` contains the validated skill and references
- AND configured agent targets receive it through the existing linker

### Scenario: MCP fan-out

- GIVEN a locked plugin containing a valid root `.mcp.json`
- WHEN apply succeeds
- THEN the server is namespaced and generated through the existing agent formatters
- AND no MCP command is started

### Scenario: Unsupported component

- GIVEN a plugin containing hooks, agents, commands, apps, or LSP components
- WHEN add or update is requested
- THEN the operation fails explicitly before materialization
