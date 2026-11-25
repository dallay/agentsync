# Proposal: Static Security Audit Hardening (#688)

## Intent

Make `agentsync audit` a trustworthy R1 security control, not merely an implemented command. The
current audit path has gaps in secret redaction, bounded path traversal, scan-completion reporting,
rule coverage, plugin/MCP trust integration, and reproducibility. A CI consumer must be able to
distinguish a completed scan from an incomplete one without the audit leaking sensitive source
content or implying that an empty result guarantees safety.

## Scope

### In Scope

- Harden target resolution and bounded traversal: validate skill/plugin identifiers, enforce the
  authorized root (including root and nested symlinks), apply limits while reading, and account for
  unreadable, unsupported, or skipped content in explicit coverage and scan-error data.
- Correctly apply static rules to the primary skill content (`SKILL.md`), supported structured
  configuration (JSON, YAML, TOML), scripts, and relevant text files. Detect path references by
  resolving them against the authorized root rather than relying only on textual traversal patterns.
- Ensure diagnostics never copy untrusted source lines or expose secrets through human output, JSON,
  stderr, errors, or logs. Align the report, coverage, completion status, ordering, and fingerprint
  contract with shared diagnostics work in #695 and redaction decisions in #702; assess compatibility
  before changing the current JSON schema.
- Integrate plugin and MCP inspection with AgentSync's existing inventory, lock, snapshot,
  provenance, and approval model. Audit only already available local content; do not download,
  materialize, or execute plugin content or MCP servers.
- Reconcile the CLI argument contract and make incomplete scans distinguishable from findings and
  clean completed scans. Define stable exit behavior: `0` for a complete scan below the configured
  `--fail-on` threshold, `1` when findings reach that threshold, and `2` for an incomplete scan or
  operational error. Reconcile threshold names with the shared severity model and avoid wording that
  certifies audited content as safe.
- Add binary-level CLI and contract tests for redaction, containment, limits, coverage, supported
  formats, plugin/MCP trust, determinism, offline/read-only behavior, and exit codes; document the
  supported audit behavior and limitations.

### Out of Scope

- SARIF, SkillSpector or other mandatory external analyzers, dynamic execution, LLM-assisted
  analysis, and a general-purpose malware or prompt-injection detector.
- Replacing the plugin manager, creating a parallel trust/provenance resolver, or redesigning the
  entire diagnostics subsystem beyond the shared audit contract required here.
- Closing #688 based only on command availability or adding a claim that no findings means the
  audited content is safe.

## Approach

Evolve the existing Rust audit, diagnostics, CLI, and plugin code without a rewrite. Agree first on
the shared diagnostic/redaction contract with #695 and #702; then implement bounded target
resolution and coverage, close the rules and trust-integration gaps, and finish with the CLI
contract, documentation, and end-to-end acceptance tests. Keep these as independently reviewable
slices so the shared contract can be validated before downstream consumers depend on it. Treat
schema compatibility and any required versioning as a design decision informed by existing
consumers, not as an assumed breaking change.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/audit.rs` | Modified | Rule applicability, safe evidence, bounded scan results, ordering, and fingerprints. |
| `src/diagnostics.rs` | Modified | Shared coverage/completion and deterministic report contract, coordinated with #695. |
| `src/commands/audit.rs`, `src/main.rs` | Modified | Target/argument validation, output and exit-code behavior. |
| `src/plugins.rs` | Modified or reused | Offline inspection of locked snapshots, provenance, and MCP approvals. |
| `src/secrets.rs` | Reused or modified | Apply the established redaction policy to every audit output path. |
| `tests/` and `tests/contracts/` | Modified | Real-binary acceptance and machine-readable output regression coverage. |
| `website/docs/src/content/docs/` | Modified | Document audit options, exit statuses, coverage, and limitations. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Fixes miss credential forms or leak source text through an untested output channel | Medium | Use synthetic canaries and assert absence from stdout, stderr, JSON, errors, and logs for each rule path; never use raw source as evidence. |
| Symlink policy rejects legitimate managed content or permits an escape | Medium | Reuse AgentSync's trust boundary, state the policy explicitly, and test root symlinks and nested escapes without reading external targets. |
| Report/schema changes break existing automation | Medium | Search for consumers, coordinate with #695, version only if required, and retain a tested compatibility path where feasible. |
| Existing plugin inspection APIs are insufficient for offline audit | Medium | Confirm API readiness during design; extend/reuse those APIs rather than adding a second plugin resolver, and fail as incomplete when required trusted data is unavailable. |
| Broader content coverage increases false positives or scan cost | Medium | Keep rules deterministic and bounded, test supported formats and negative cases, and expose skipped content and limits through coverage. |

## Rollback Plan

Deliver the work in independently revertible slices. If a slice causes a regression, revert that slice
and its associated contract/test changes, preserving the existing CLI behavior where possible. Do
not roll back secret redaction or fail-closed handling of incomplete scans to restore compatibility;
instead, gate release or provide a versioned compatibility adapter. Keep schema changes additive
until consumer compatibility is established.

## Dependencies

- Coordinate the shared diagnostics/report contract with #695 and redaction policy with #702 before
  stabilizing audit output.
- Reuse the existing local plugin inventory, lock/snapshot, provenance, and MCP approval facilities;
  confirm during design that they expose sufficient read-only inspection APIs.
- Existing audit fixtures and test infrastructure in `tests/`; no external service or network access
  is required for implementation or verification.

## Success Criteria

- [ ] A synthetic secret canary is absent from human output, JSON, stderr, errors, and logs for every audit rule path.
- [ ] Invalid identifiers and out-of-root references are rejected; an escaping symlink is reported without reading its target, including when the audit root itself is a symlink.
- [ ] Read/depth/file limits are enforced during traversal, and reports account for scanned, skipped, and failed content; completed scans return `0` below the configured threshold, findings at/above it return `1`, and incomplete scans or operational errors return `2`.
- [ ] Integration fixtures prove required credential and fetch-and-execute rules inspect `SKILL.md`, supported configuration formats, and scripts, including valid quoted JSON keys.
- [ ] Plugin/MCP auditing uses existing trust and approval data offline and performs no download, materialization, command execution, or filesystem mutation.
- [ ] Equivalent scans produce sorted, stable findings and fingerprints; CLI tests cover completed scans, threshold behavior, incomplete scans, and JSON contract validation.
- [ ] User-facing output and documentation state that an empty result is not a safety guarantee; #688 remains open until the acceptance matrix is verified.