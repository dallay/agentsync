# Proposal: Repository-owned plugin materialization

## Intent

Provide a deterministic, vendor-neutral path from a selected marketplace plugin to AgentSync's
canonical skills and MCP configuration without relying on Claude or Codex user caches.

## Scope

In scope: typed marketplace/plugin selections, immutable project lockfile and provenance, local and
pinned GitHub sources, conventional `skills/<id>/SKILL.md` bundles, root `.mcp.json` declarations,
safe apply/update/remove/drift behavior, and Claude/Codex/Gemini/OpenCode fan-out.

Out of scope: vendor cache installation or enablement, hooks, scripts, binaries, LSPs, apps, and
execution of MCP servers.

## Compatibility

Existing skill registry metadata, installed-state JSON, symlink targets, and explicit
`[mcp_servers.*]` configuration remain supported. Plugin provenance is additive and the curated
maintainer registry is not reused as the project plugin lockfile.
