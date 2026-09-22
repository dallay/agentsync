# Issue 595 — Z-Code and MiniMax support

## Route
Delegated direct (ODD), not formal SDD. The requested outcome is bounded, but it crosses agent registration, target configuration, MCP generation, and tests.

## Acceptance criteria
- [x] `zcode` and `minimax` are accepted as canonical AgentSync agent IDs and aliases where applicable.
- [x] Generated/default configuration documents the support accurately.
- [x] Existing `.agents/skills` and root `AGENTS.md` remain the canonical sources; no redundant skill targets are added.
- [x] MiniMax uses the existing project `.mcp.json` MCP contract.
- [x] Z-Code MCP generation writes/merges `.zcode/config.json` using `mcp.servers` without discarding unrelated configuration keys.
- [x] Z-Code commands expose `foo.agent.md` as `.zcode/commands/foo.md` (or an explicitly tested equivalent), preserving the intended slash-command name.
- [x] Unsupported project custom-agent mappings are not invented.
- [x] Focused tests cover registration, command naming, MCP output/merge behavior, and config/template integrity.

## Evidence
- Issue: https://github.com/dallay/agentsync/issues/595
- Runtime analysis supplied by user: both runtimes natively consume `.agents/skills`; MiniMax consumes root `.mcp.json`; Z-Code needs native config mapping for reliable MCP and command-name normalization.

## Progress
- ODD-001: Issue comment posted in English — complete.
- ODD-002: Registry/config/MCP surfaces inspected — complete.
- ODD-003: Add failing tests — complete; tests cover registration, aliases, end-to-end Z-Code commands/status, command naming, and Z-Code MCP preservation/initialization.
- ODD-004: Implement minimal support — complete; agent registry, MCP formatters, Z-Code command normalization, apply/status/cleanup consistency, templates, and docs updated.
- ODD-005: Run focused verification and diff review — complete; focused tests, docs registry validation, cargo check, rustfmt, and diff checks pass.

## Next step
Ready for review. The implementation intentionally does not add redundant `.agents/skills` targets or project custom-agent mappings.
