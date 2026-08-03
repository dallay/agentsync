# Proposal: CLI Output Extraction and Canonical Agent Documentation

## Intent

Address GitHub issues #494 and #500 together: separate reusable CLI presentation from `src/main.rs` without changing established output contracts, and eliminate supported-agent/MCP documentation drift by making native MCP metadata typed and canonical with fast CI validation.

## Scope

### In Scope
- Extract main-owned apply/clean/init presentation helpers into the output boundary while preserving exact line order, labels, spacing, ANSI behavior, and existing JSON contracts.
- Add exact renderer/output-contract tests, retaining status JSON ownership in `src/commands/status.rs` and leaving `src/init.rs` operation progress unchanged.
- Define one typed canonical registry for native MCP agents, including Claude Desktop, canonical IDs, names, paths, formats, global-path status, and documentation notes.
- Generate or validate marked documentation fragments in `README.md`, `npm/agentsync/README.md`, `website/docs/src/content/docs/guides/mcp.mdx`, and `openspec/specs/mcp-generation/spec.md`; add focused CI drift validation.

### Out of Scope
- Refactoring unrelated command orchestration, linker/config behavior, or `src/init.rs` progress output.
- Treating configurable-only agents as native MCP agents or expanding the native MCP support set.
- Changing MCP formats, paths, defaults, filtering semantics, or CLI output wording.

## Capabilities

### New Capabilities
- None. This change consolidates implementation and governance of existing behavior.

### Modified Capabilities
- `mcp-generation`: document the complete native MCP registry, including Claude Desktop and canonical IDs, without changing generation behavior.
- `documentation`: require governed native-agent/MCP listings and CI validation against the typed registry.

## Approach

Move pure renderers and emission helpers from `src/main.rs` to `src/output.rs` (or a narrowly scoped output submodule), preserving orchestration boundaries and contract tests. Extend `McpAgent` with typed documentation metadata while retaining formatter dispatch and OS-specific path resolution as code behavior. Use stable generated/validated Markdown markers and a deterministic validator in `scripts/` or Rust tooling, then run it as a fast CI job. Documentation must distinguish native MCP support from configurable sync support and use canonical IDs.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/main.rs`, `src/output.rs` | Modified | Extract presentation; preserve contracts |
| `src/mcp.rs`, `src/agent_ids.rs` | Modified | Canonical native MCP metadata and ID boundary |
| `README.md`, `npm/agentsync/README.md`, `website/docs/src/content/docs/guides/mcp.mdx` | Modified | Governed MCP listings |
| `openspec/specs/mcp-generation/spec.md`, `.github/workflows/ci.yml`, `scripts/` | Modified/New | Spec alignment and drift validation |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Implicit output changes | Med | Golden/exact renderer tests and narrow extraction |
| MCP/configurable agents conflated | Med | Explicit native-only registry and canonical IDs |
| Noisy generated docs | Low | Stable marked fragments and focused CI |

## Rollback Plan

Revert the change commit(s), restoring renderer locations and documentation sources; disable/remove the validator job if needed. No runtime data migration is required.

## Dependencies

- Existing MCP registry and CI workflow; no new runtime dependency required.

## Success Criteria

- [ ] Existing CLI output and status JSON contract tests pass unchanged.
- [ ] Native MCP docs agree with typed metadata, including Claude Desktop and canonical IDs.
- [ ] CI fails deterministically when governed MCP documentation drifts.
- [ ] No unrelated agent or MCP behavior changes are introduced.
