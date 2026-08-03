# Delta for Documentation

## ADDED Requirements

### Requirement: Native MCP Documentation Has One Governed Source

Documentation MUST identify native MCP support separately from generic configurable-agent support.
The README, npm README, MCP guide, and MCP OpenSpec MUST use the canonical native MCP IDs and
metadata, including Claude Desktop. Documentation MUST describe Claude Desktop's global,
OS-dependent scope when supported and MUST NOT claim that all configurable agents are native MCP
agents.

#### Scenario: Reader sees a complete native MCP list

- GIVEN a reader checks any governed native-MCP documentation surface
- WHEN the supported agents are listed
- THEN the list MUST use canonical IDs and include Claude Desktop
- AND its entries MUST agree with the typed runtime metadata

#### Scenario: Reader understands Claude Desktop scope

- GIVEN Claude Desktop is enabled or documented as supported
- WHEN the reader reviews its MCP entry
- THEN the documentation MUST identify its global OS-dependent destination
- AND MUST preserve any default-disabled or opt-in distinction

### Requirement: Manual Documentation Synchronization Guidance Is Removed

The documentation MUST NOT instruct maintainers to manually cross-check or keep native MCP agent
lists synchronized. It MUST instead point to the automated generation or CI drift validation as
the governing maintenance mechanism.

#### Scenario: Maintainer updates native MCP metadata

- GIVEN a maintainer changes the canonical native MCP registry
- WHEN the maintainer follows repository documentation guidance
- THEN the guidance MUST direct them to the generator or validator and focused CI check
- AND MUST NOT require an undocumented manual comparison process

#### Scenario: Documentation drift is reported

- GIVEN a governed native-MCP fragment no longer matches the registry
- WHEN CI validation runs
- THEN the documentation workflow MUST report the drift clearly
- AND MUST fail before the change is accepted
