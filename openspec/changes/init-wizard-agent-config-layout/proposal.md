# Proposal: Init Wizard Agent Config Layout

## Intent

Address issue #269 by making `agentsync init --wizard` generate a clear "Agent config layout"
explainer inside `.agents/AGENTS.md` so newly initialized projects understand that `.agents/` is the
canonical source and how the default agent targets are populated.

## Scope

### In Scope

- Generate and write/prepend a managed "Agent config layout" section when the wizard writes
  `.agents/AGENTS.md`.
- Derive the generated section from the wizard's final default target layout strongly enough to stay
  accurate for enabled default instruction, skills, and commands targets, including the selected
  skills sync mode.
- Use explicit begin/end markers so the generated block is idempotent and replaceable on future
  forced wizard reruns rather than duplicated.
- Preserve existing behavior when `.agents/AGENTS.md` already exists and `--force` is not used: do
  not mutate, inject into, or partially rewrite the file.
- Add tests covering generated content accuracy, marker-managed/idempotent behavior, and no-change
  behavior for existing `.agents/AGENTS.md` without `--force`.

### Out of Scope

- Any `agentsync apply` mutation or regeneration of `.agents/AGENTS.md`; that follow-up is
  explicitly deferred.
- Support for arbitrary custom target layouts beyond the wizard's default enabled targets.
- Reworking unrelated wizard migration, scan, or apply semantics.

## Approach

Extend the wizard-only `.agents/AGENTS.md` rendering path so, after the final config is known, it
generates a managed explainer block from the same target configuration the wizard is about to write.
The block should describe the canonical instructions, skills, and commands destinations using
wording that matches the chosen sync semantics, then insert that block prominently near the top of
the generated file without breaking the rest of the wizard-produced content. Marker management
should make the block safe to refresh during `--force` flows while leaving existing user-managed
files untouched when force is not enabled.

## Affected Areas

| Area                                                 | Impact   | Description                                                                                   |
|------------------------------------------------------|----------|-----------------------------------------------------------------------------------------------|
| `src/init.rs`                                        | Modified | Wizard AGENTS rendering, generated layout block insertion, and force/preservation behavior.   |
| `src/config.rs`                                      | Modified | Config-derived layout description helpers or accessors, if needed for target-aware rendering. |
| `openspec/specs/skill-adoption/spec.md`              | Modified | Add wizard AGENTS layout requirements and scenarios.                                          |
| `src/init.rs` tests and/or `tests/*` wizard coverage | Modified | Add coverage for generated block content, markers, idempotence, and non-force preservation.   |

## Risks

| Risk                                                                         | Likelihood | Mitigation                                                                                     |
|------------------------------------------------------------------------------|------------|------------------------------------------------------------------------------------------------|
| Generated prose drifts from actual default targets or sync semantics         | Medium     | Generate from the same final wizard config rather than hardcoded text.                         |
| New managed section creates awkward document structure or duplicate headings | Medium     | Define stable placement rules near the top of the file and validate with tests.                |
| Marker block collides with user-authored content on reruns                   | Low        | Use unique begin/end markers and only replace the managed block during force-enabled rewrites. |
| Users expect ongoing regeneration after manual config edits                  | Medium     | State in scope and specs that v1 is wizard-only and explicitly defer `apply` ownership.        |

## Rollback Plan

Remove the wizard-only generated layout block logic and restore the prior `.agents/AGENTS.md` write
path, leaving the wizard to write only migrated/default AGENTS content. Revert accompanying spec and
test updates with the code change.

## Dependencies

- Existing exploration artifact: `openspec/changes/init-wizard-agent-config-layout/exploration.md`
- Current wizard/default config model in `src/init.rs` and `src/config.rs`

## Success Criteria

- [ ] A fresh `agentsync init --wizard` that writes `.agents/AGENTS.md` includes a generated
  managed "Agent config layout" section near the top of the file.
- [ ] The generated section accurately reflects enabled default targets and the selected skills sync
  mode rather than relying on static assumptions.
- [ ] Rewriting the wizard output with `--force` updates the managed block idempotently instead of
  duplicating it.
- [ ] Running the wizard when `.agents/AGENTS.md` already exists without `--force` preserves the
  existing file unchanged.
- [ ] Proposal-level non-goals remain true in implementation: no `agentsync apply` mutation is
  introduced in v1.
