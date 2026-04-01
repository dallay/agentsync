# Gitignore Management Specification

## Purpose

Defines how AgentSync reconciles the managed `.gitignore` section during `apply`, including enabled,
disabled, dry-run, and opt-out behavior, so repository state matches the configured gitignore policy
without disturbing unmanaged lines.

## Requirements

### Requirement: Apply Removes Managed Gitignore Block When Management Is Disabled

When `[gitignore].enabled = false`, `agentsync apply` MUST reconcile `.gitignore` by removing an
existing AgentSync-managed block identified by the configured marker.

The cleanup MUST remove only the matching managed block and MUST preserve unmanaged content before
and after that block.

The cleanup MUST be idempotent. If no matching managed block exists, `agentsync apply` MUST leave
`.gitignore` unchanged and MUST NOT introduce a replacement block while management remains disabled.

The product default for gitignore management MUST remain unchanged; this requirement applies only
when the effective configuration disables gitignore management.

#### Scenario: Apply removes stale managed block when disabled

- GIVEN a repository `.gitignore` contains unmanaged lines and an AgentSync-managed block using the
  effective marker
- AND the effective configuration sets `[gitignore].enabled = false`
- WHEN the user runs `agentsync apply`
- THEN the managed block MUST be removed from `.gitignore`
- AND the unmanaged lines before and after the removed block MUST be preserved
- AND no new managed `.gitignore` block SHALL be written

#### Scenario: Repeat apply is idempotent after cleanup

- GIVEN `[gitignore].enabled = false`
- AND a prior `agentsync apply` has already removed the matching managed `.gitignore` block
- WHEN the user runs `agentsync apply` again
- THEN `.gitignore` MUST remain unchanged
- AND the command MUST NOT recreate a managed block
- AND the command MUST NOT rewrite `.gitignore` solely to express a no-op cleanup state

#### Scenario: Cleanup respects custom markers

- GIVEN a repository `.gitignore` contains an AgentSync-managed block delimited by a custom
  configured marker
- AND `[gitignore].enabled = false`
- WHEN the user runs `agentsync apply`
- THEN AgentSync MUST remove the block matching that custom marker
- AND AgentSync MUST NOT remove text associated with any different marker value

---

### Requirement: Dry-Run Reports Disabled Gitignore Cleanup Without Writing

When `[gitignore].enabled = false` and a matching managed `.gitignore` block exists,
`agentsync apply --dry-run` MUST report that the managed `.gitignore` block would be removed.

Dry-run MUST NOT modify `.gitignore` or any other file.

If no matching managed block exists, dry-run MUST NOT report a cleanup that would not occur.

#### Scenario: Dry-run reports pending cleanup

- GIVEN a repository `.gitignore` contains a matching AgentSync-managed block
- AND `[gitignore].enabled = false`
- WHEN the user runs `agentsync apply --dry-run`
- THEN the output MUST report that the managed `.gitignore` block would be removed
- AND the `.gitignore` file on disk MUST remain unchanged

#### Scenario: Dry-run with no matching managed block is a no-op

- GIVEN a repository `.gitignore` does not contain a managed block matching the effective marker
- AND `[gitignore].enabled = false`
- WHEN the user runs `agentsync apply --dry-run`
- THEN the output MUST NOT claim that `.gitignore` would be cleaned up
- AND `.gitignore` MUST remain unchanged

---

### Requirement: No-Gitignore Flag Strictly Opts Out Of Gitignore Reconciliation

When the user passes `--no-gitignore`, that flag MUST take precedence over gitignore enablement or
disablement settings.

With `--no-gitignore`, `agentsync apply` MUST NOT create, update, remove, or otherwise rewrite a
managed `.gitignore` block.

This strict opt-out MUST apply equally to normal apply and dry-run behavior.

#### Scenario: Disabled cleanup is skipped when no-gitignore is set

- GIVEN a repository `.gitignore` contains a stale AgentSync-managed block
- AND `[gitignore].enabled = false`
- WHEN the user runs `agentsync apply --no-gitignore`
- THEN `.gitignore` MUST remain unchanged
- AND AgentSync MUST NOT remove the managed block

#### Scenario: Dry-run with no-gitignore does not report gitignore cleanup

- GIVEN a repository `.gitignore` contains a stale AgentSync-managed block
- AND `[gitignore].enabled = false`
- WHEN the user runs `agentsync apply --dry-run --no-gitignore`
- THEN the output MUST NOT report a `.gitignore` cleanup action
- AND `.gitignore` MUST remain unchanged

---

### Requirement: Diagnostics Remain Aligned With Disabled Gitignore Policy

When gitignore management is disabled, AgentSync diagnostics such as `doctor` MUST treat the absence
of an AgentSync-managed `.gitignore` block as the expected healthy state.

Diagnostics MUST NOT require a managed `.gitignore` block to exist when
`[gitignore].enabled = false`.

This change MUST NOT alter the product default; it only clarifies that disabled gitignore management
and successful cleanup are consistent with a healthy repository state.

#### Scenario: Doctor accepts cleaned state when gitignore is disabled

- GIVEN `[gitignore].enabled = false`
- AND `.gitignore` does not contain a managed block matching the effective marker because cleanup
  has already occurred
- WHEN the user runs `agentsync doctor`
- THEN doctor MUST treat the missing managed `.gitignore` block as expected for the configuration
- AND doctor MUST NOT instruct the user to restore a managed `.gitignore` block while management
  remains disabled
