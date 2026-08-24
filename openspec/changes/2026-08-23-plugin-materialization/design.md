# Design: Repository-owned plugin materialization

## Configuration

`[plugins]` is disabled by default for backwards compatibility. Marketplaces are named source
declarations and selections identify the plugin to resolve. Mutable references are accepted only
by explicit add/update operations.

The project lockfile is `.agents/plugins.lock.toml` by default. It uses schema `v1`, stable TOML
ordering, full Git commit SHAs or `local:<tree-sha256>` revisions, plugin tree hashes, per-skill
hashes, MCP names, and source provenance.

## Apply data flow

```text
agentsync.toml selection -> plugins.lock.toml -> source verification
  -> marketplace manifest -> plugin component validation
  -> skills materialized under .agents/skills
  -> existing linker fan-out and MCP formatters
```

Apply is offline and fails closed when the lockfile, source, component set, or content hash does
not match. Add/update may resolve a GitHub reference and writes the lock atomically.
For Git sources, add/update also materialize a project-owned snapshot under
`.agents/.agentsync-plugin-sources`; apply/status/dry-run never download a source.

## Supported components

The first adapter accepts a vendor marketplace manifest at `.agents/plugins/marketplace.json` or
`.claude-plugin/marketplace.json`, a local plugin source, conventional skill directories, and a
root `.mcp.json` with `mcpServers`. AgentSync rejects plugin-level agents, commands, hooks, LSPs,
apps, and vendor-specific MCP fields instead of silently flattening them.

Plugin MCP names are namespaced as `plugin/<marketplace>/<plugin>/<server>`. They are merged with
explicit project servers only after collision checks and are never executed.

## Atomicity and safety

Skill copies reject symlinks and unsafe IDs/paths. Existing unmanaged skills are never replaced.
Plugin-owned replacements require matching registry provenance. Lockfile and config writes use
same-directory temporary files and atomic replacement. No vendor CLI, lifecycle hook, executable,
LSP, or MCP process is started.
