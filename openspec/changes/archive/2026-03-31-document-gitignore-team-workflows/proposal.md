# Proposal: Document Gitignore Team Workflows

## Intent

Issue DALLAY-219 needs documentation that explains AgentSync's `.gitignore` behavior in practical team terms, not just configuration reference terms. The current docs mention gitignore management, but they do not clearly walk readers through the two supported team workflows: the default managed-gitignore path with `[gitignore].enabled = true`, and the opt-out committed-symlink path with `[gitignore].enabled = false`. Recent behavior changes around root-scoped managed entries and stale managed-block cleanup when management is disabled also need to be reflected so the docs match current product behavior.

## Scope

### In Scope
- Add a primary guide that explains the two team workflows step by step: default managed-gitignore and opt-out committed-symlink.
- Update supporting docs surfaces to cross-link back to the primary guide and clarify current `.gitignore` behavior, including root-scoped managed entries, disabled cleanup, and `--no-gitignore` opt-out semantics.
- Keep the documentation aligned with the existing product default of `[gitignore].enabled = true` and frame the disabled mode as an intentional opt-out workflow rather than a new default policy.
- Minimize Windows-specific duplication by documenting the shared workflow once and reserving OS-specific notes for targeted callouts ahead of DALLAY-220.

### Out of Scope
- Changing AgentSync product behavior, defaults, or policy around `.gitignore` management.
- Expanding into a broad cross-platform docs reorganization or dedicated Windows workflow rewrite.
- Re-documenting all target types or all `agentsync apply` behavior beyond the parts needed to explain the two gitignore workflows.

## Approach

Create one canonical guide under `website/docs/src/content/docs/guides/` that answers the practical question, "How should a team use AgentSync with gitignore?" The guide should present a decision-oriented introduction, then provide two explicit step-by-step paths:

1. **Default managed-gitignore workflow**
   - keep `[gitignore].enabled = true`
   - explain that AgentSync writes a managed block in `.gitignore`
   - explain that managed entries now include root-scoped patterns for repository-root managed files
   - explain what teammates should run and what diffs they should expect

2. **Opt-out committed-symlink workflow**
   - set `[gitignore].enabled = false`
   - explain that existing AgentSync-managed blocks are cleaned up on apply
   - explain that cleanup is marker-aware and stale managed blocks are removed without changing unmanaged lines
   - explain when this mode is appropriate for teams intentionally committing managed destinations

Supporting reference pages should stay concise and defer workflow guidance to the new guide. The configuration reference should keep the technical definition of `[gitignore]` fields while adding a clearer summary of enabled vs disabled behavior. CLI docs should clarify `apply`/`--no-gitignore` behavior in workflow terms. Landing-page or getting-started surfaces should point readers to the new guide rather than duplicating the full explanation.

To reduce future churn before DALLAY-220, Windows-specific caveats should be expressed as small scoped notes or links instead of maintaining parallel workflow instructions.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `website/docs/src/content/docs/guides/` | New | Add a primary guide for AgentSync gitignore team workflows. |
| `website/docs/src/content/docs/reference/configuration.mdx` | Modified | Clarify `[gitignore].enabled` default, disabled cleanup behavior, and point to the workflow guide. |
| `website/docs/src/content/docs/reference/cli.mdx` | Modified | Clarify `apply` and `--no-gitignore` behavior in relation to the two workflows. |
| `website/docs/src/content/docs/guides/getting-started.mdx` and/or `website/docs/src/content/docs/index.mdx` | Modified | Add lightweight cross-links to the new workflow guide from entry-point docs. |
| `openspec/specs/gitignore-management/spec.md` | Referenced | Source of truth for disabled cleanup, marker-aware behavior, and default-preserving semantics that docs must reflect. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Docs accidentally imply `[gitignore].enabled = false` is a recommended new default | Medium | Repeatedly label managed mode as the default and disabled mode as an opt-out workflow. |
| Docs drift from recent implementation details such as root-scoped entries or stale-block cleanup | Medium | Anchor explanations to the current gitignore-management spec and recent behavior changes before writing copy. |
| Workflow guidance gets duplicated across multiple pages and becomes hard to maintain | Medium | Keep one canonical guide and use short summary cross-links elsewhere. |
| Windows notes sprawl into a second full set of workflow steps before DALLAY-220 | Low | Keep platform-specific content to brief callouts and defer broader Windows duplication work. |

## Rollback Plan

If the new documentation proves confusing or inaccurate, revert the new guide and the related cross-link/reference edits, restoring the prior documentation set. Because this is a docs-only change, rollback is limited to documentation files and does not require product or migration changes.

## Dependencies

- `openspec/specs/gitignore-management/spec.md` for current behavioral truth.
- Existing Starlight docs structure under `website/docs/src/content/docs/`.
- Documentation review/build validation via `pnpm run docs:build` during implementation.

## Success Criteria

- [ ] The docs include one primary guide that step-by-step explains both the default managed-gitignore workflow and the opt-out committed-symlink workflow.
- [ ] The guide explicitly states that `[gitignore].enabled = true` remains the default and that the disabled mode is an intentional opt-out for teams that commit managed destinations.
- [ ] Supporting docs surfaces cross-link to the primary guide instead of re-explaining the workflow in multiple places.
- [ ] The final docs accurately describe root-scoped managed entries, cleanup of stale managed blocks when gitignore management is disabled, and the effect of `--no-gitignore`.
- [ ] The rollout updates the relevant docs entry points without creating a parallel Windows-specific workflow narrative ahead of DALLAY-220.
