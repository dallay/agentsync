# Proposal: Clean up managed .gitignore blocks when gitignore is disabled

## Intent

Issue DALLAY-218 addresses a reconciliation bug in `agentsync apply`. Repositories that previously
used AgentSync-managed `.gitignore` entries can later set `[gitignore].enabled = false`, but `apply`
currently skips `.gitignore` entirely in that configuration, leaving the old managed block behind.
This creates stale managed state that no longer reflects the config and makes disabling gitignore
management incomplete.

## Scope

### In Scope

- Update `agentsync apply` so a previously managed `.gitignore` block is removed when
  `[gitignore].enabled = false`.
- Preserve the current `--no-gitignore` behavior so the CLI still skips all `.gitignore`
  reconciliation when that flag is passed.
- Define dry-run and custom-marker expectations for the disabled-cleanup path and cover them with
  tests.

### Out of Scope

- Changing the product default for `[gitignore].enabled`; it remains `true`.
- Redesigning general `.gitignore` generation, marker formats, or doctor policy beyond what is
  needed for cleanup reconciliation.
- Removing unmanaged `.gitignore` content or attempting broader normalization/migration of
  user-authored entries.

## Approach

Adjust the apply flow so `.gitignore` reconciliation has three explicit modes: skip entirely when
`--no-gitignore` is set, update/create the managed block when gitignore management is enabled, and
remove the existing managed block when gitignore management is disabled. The cleanup path should
reuse the existing managed-section removal logic in `src/gitignore.rs` so removal semantics stay
consistent with normal replacement behavior.

Dry-run behavior should mirror existing apply semantics: when cleanup would occur, AgentSync reports
that it would remove the managed `.gitignore` section but does not modify the file. Custom marker
behavior should remain fully respected by using the configured marker when locating the managed
block to remove.

No default-enablement change is intended. This is a one-time corrective reconciliation path for
repositories that transition from enabled to disabled.

## Affected Areas

| Area                     | Impact   | Description                                                                                                                |
|--------------------------|----------|----------------------------------------------------------------------------------------------------------------------------|
| `src/main.rs`            | Modified | Route apply through explicit update vs cleanup vs skip `.gitignore` behavior.                                              |
| `src/gitignore.rs`       | Modified | Expose or add a helper that removes a managed section from `.gitignore`, respecting dry-run and configured markers.        |
| `src/commands/doctor.rs` | Reviewed | Confirm doctor expectations remain coherent when gitignore management is disabled and cleanup has removed the stale block. |
| `tests/`                 | Modified | Add coverage for disabled cleanup, `--no-gitignore` preservation, dry-run reporting, and custom-marker cleanup.            |

## Risks

| Risk                                                                     | Likelihood | Mitigation                                                                                      |
|--------------------------------------------------------------------------|------------|-------------------------------------------------------------------------------------------------|
| Cleanup path removes the wrong section                                   | Low        | Match only the configured start/end markers and reuse existing managed-section removal logic.   |
| Disabled cleanup accidentally runs during `--no-gitignore`               | Low        | Keep `--no-gitignore` as the highest-priority skip path and add explicit tests.                 |
| Cleanup rewrites `.gitignore` unnecessarily when no managed block exists | Medium     | Make the cleanup path idempotent and avoid writes when the file content would remain unchanged. |

## Rollback Plan

Revert the apply-flow change and any new cleanup helper so disabled configs return to the prior
behavior of leaving `.gitignore` untouched. Because the change is isolated to reconciliation logic,
rollback can be performed by reverting the implementation commit and rerunning `agentsync apply`
with gitignore enabled if a managed block must be restored.

## Dependencies

- Existing apply decision logic in `src/main.rs`.
- Existing managed-section parsing/removal behavior in `src/gitignore.rs::remove_managed_section`.
- Existing gitignore marker configuration in `Config::gitignore.marker`.

## Success Criteria

- [ ] When a repo contains an AgentSync-managed `.gitignore` block and
  `[gitignore].enabled = false`, `agentsync apply` removes that managed block while preserving
  surrounding unmanaged content.
- [ ] When `agentsync apply --no-gitignore` is used, AgentSync leaves `.gitignore` untouched even if
  gitignore management is disabled and a stale managed block exists.
- [ ] Dry-run reports the disabled cleanup action without modifying `.gitignore`.
- [ ] Cleanup respects custom markers and remains idempotent when no matching managed block exists.

## Acceptance Criteria

- `agentsync apply` reconciles stale managed `.gitignore` content when gitignore management is
  disabled instead of silently skipping the file.
- The cleanup path removes only the managed section identified by the configured marker and
  preserves all non-managed `.gitignore` lines.
- Passing `--no-gitignore` keeps current behavior unchanged by bypassing both update and cleanup
  logic.
- Dry-run produces an accurate “would remove managed section” style result and does not write
  `.gitignore`.
- Custom marker repositories clean up the matching managed block using that custom marker value.
- Doctor expectations remain aligned with current policy: no new requirement to keep a managed
  `.gitignore` section when gitignore management is disabled, and no default-behavior change is
  introduced.

## Rollout Notes

This should ship as a backward-compatible bug fix. Repositories that disable gitignore management
may see a one-time `.gitignore` diff on the next `agentsync apply`, removing stale AgentSync-managed
markers and entries while leaving user-managed ignore rules intact.
