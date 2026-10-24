# ADR: AgentSync-managed resolved configuration and inspection

**Status:** Provisional — implemented for DALLAY-696; pending ratification through DALLAY-708.
**Date:** 2026-10-23

## Context

AgentSync parses project declarations, projects targets, materializes symlinks, composes MCP server
configuration, and tracks plugin contributions. These stages are distinct. A configuration report
must not conflate declaration with planned output or observed filesystem state. Vendor runtime
configuration can also include global, user, organization, or session overlays that AgentSync does
not own.

## Decision

`ResolvedConfiguration` is a deterministic projection of **AgentSync-managed** configuration for
one configured assistant. It is not an authoritative provider-runtime snapshot.

The JSON v1 envelope contains `schema_version`, `scope`, `agent`, `items`, `mcp`, `diagnostics`, and
`scope_notice`. It has no timestamp. Collections use stable ordering. Project-relative paths use `/`
separators. Each managed item carries declaring target, sync mode, source, destination, origin,
owner, provider-support status, projected state, observed state, and child observations where the
sync mode expands entries.

Item states have these meanings:

- `configured`: intent is declared, but a source is missing.
- `projected`: source exists; the destination is not observed as the planned output.
- `observed`: materialized output matches the projection.
- `conflicted`: observed destination differs from the projection or a declaration collision exists.
- `unsupported`: AgentSync has no projection for the capability. This does not assert that the
  provider itself lacks that capability.

`provider_support: "unknown"` is used for instruction, skill, and command targets unless a future
capability adapter can establish more. The inspector does not invent native support semantics.

### MCP precedence

1. Explicit `[mcp_servers.*]` declarations form the base set.
2. Plugin MCP contributions are namespaced as `plugin/{marketplace}/{plugin}/{server}` and included
   only when listed in `plugins.allowed_mcp`.
3. A plugin contribution with a name already declared explicitly is accepted by `apply` only when
   the server configurations are equal; a differing configuration is an error.
4. Disabled MCP servers are not generated.
5. The configured merge strategy determines how existing destination entries are merged or
   overwritten. The report reads existing server names to explain destination interactions but never
   emits unmanaged vendor values.

### Read-only and offline boundary

Inspection may read project config, target paths, MCP destination files, plugin lockfiles, and plugin
snapshots already present locally. It MUST NOT write files, restore or download snapshots, resolve
marketplace references, expand environment variables, launch provider CLIs, or start MCP servers.
An unavailable plugin snapshot produces an explicit diagnostic.

### Redaction

Environment and header values are never serialized, whether literal or `${ENV}` references. The
report contains key names and classifications only. Sensitive command arguments, URL userinfo, and
sensitive query values are redacted. Existing unmanaged MCP configuration contributes server names
only. The report does not read arbitrary instruction or skill file contents and is not a general
secret scanner.

### Exit behavior

Default mode returns `0` when it produces a report, including reports with diagnostics. `--strict`
returns `1` when any diagnostic has error severity. Clap usage errors retain exit code `2`; config
and runtime failures are non-zero.

## Consequences

- The model can be reused by later project profile and capability-adapter work.
- `status --json` remains unchanged; it consumes the same extracted materialization reader.
- Provider-native runtime settings remain explicitly unknown outside AgentSync's managed boundary.
- The exact state vocabulary, redaction matrix, and envelope remain provisional until DALLAY-708 and
  DALLAY-709 are ratified.
