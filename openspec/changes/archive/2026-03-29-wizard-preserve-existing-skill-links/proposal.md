# Proposal: Wizard Preserve Existing Skill Links

## Intent

Reduce migration risk for repositories that already have agent skill links by making `init --wizard`
and pre-apply diagnostics aware of the on-disk skills layout. The change should help users keep
working setups intact, avoid `apply` churn when config mode and disk shape disagree, and clarify the
current recommended skills strategy without re-opening the already-fixed default of
`type = "symlink"` for skills.

## Scope

### In Scope

- Add an explicit wizard prompt for each generated skills target that explains `symlink` vs
  `symlink-contents`, shows the recommended choice, and defaults to the current safe recommendation.
- Detect existing destination state during wizard setup and preserve the effective mode when the
  destination already points at the canonical skills source, especially for existing directory
  symlinks.
- Add a post-init validation summary plus `doctor`/targeted `status` diagnostics for mode-semantic
  mismatches so users see migration risks before `apply` mutates anything.
- Update CLI/config/skills documentation to describe the current skills default, preservation
  behavior, and the new diagnostics.

### Out of Scope

- Changing the underlying behavior of `SyncType::Symlink` or `SyncType::SymlinkContents`, or
  introducing a new sync type.
- Reverting or reworking the current default skills target behavior on `main`; `type = "symlink"`
  remains the default contract.
- Adding `init --wizard --preserve-existing-links` in this change; inference-backed wizard defaults
  cover the migration-safe path with less CLI surface and lower maintenance risk.
- Broad `status` output redesign beyond a focused hint for detected mode-semantic mismatches.

## Approach

Keep the change minimal and migration-safe by reusing one shared notion of “mode-semantic mismatch”
across init-time validation and diagnostics. In `src/init.rs`, detect whether an existing skills
destination is already a directory symlink to the canonical `.agents/skills` source and
recommend/persist `symlink` for that agent, while still allowing the user to override the choice in
the wizard. After the wizard writes config, run a validation summary that highlights any remaining
mismatch before the first `apply`.

In diagnostics, teach `doctor` to flag the important mismatch case called out in exploration: config
says `symlink-contents` while the destination is already a directory symlink to the expected source.
Add a focused `status` hint only if it can reuse the same check without changing normal
success/problem semantics. Update docs in the same change so implementation, generated config
guidance, and reference material stay aligned.

## Affected Areas

| Area                                                        | Impact   | Description                                                                                       |
|-------------------------------------------------------------|----------|---------------------------------------------------------------------------------------------------|
| `src/init.rs`                                               | Modified | Wizard prompt text, recommendation inference, config generation, and post-init validation summary |
| `src/commands/doctor.rs`                                    | Modified | Warn about skills mode-semantic mismatches before `apply` churn                                   |
| `src/commands/status.rs`                                    | Modified | Add a narrow hint for detected mode-only mismatches if reuse stays simple                         |
| `src/linker.rs`                                             | Modified | Small guard or shared helper use only if needed to avoid churn from recognized mismatch states    |
| `tests/test_agent_adoption.rs`                              | Modified | Cover wizard preservation and migration-safe defaults                                             |
| `src/commands/doctor_tests.rs`                              | Modified | Cover mode-semantic mismatch detection                                                            |
| `src/commands/status_tests.rs`                              | Modified | Cover status hint behavior if included                                                            |
| `website/docs/src/content/docs/reference/configuration.mdx` | Modified | Correct skills mode guidance and examples                                                         |
| `website/docs/src/content/docs/reference/cli.mdx`           | Modified | Document wizard prompt/validation behavior                                                        |
| `website/docs/src/content/docs/guides/skills.mdx`           | Modified | Explain recommended skills strategy and preservation workflow                                     |

## Risks

| Risk                                                           | Likelihood | Mitigation                                                                                                        |
|----------------------------------------------------------------|------------|-------------------------------------------------------------------------------------------------------------------|
| Wizard prompts become noisy across multiple agents             | Medium     | Keep the prompt compact, show a recommendation, and only ask for skills targets that are actually generated       |
| Diagnostics misclassify user-managed directories as mismatches | Medium     | Limit the first implementation to confidently recognized shapes, especially directory-symlink-to-canonical-source |
| Partial fix leaves docs and UX inconsistent                    | Medium     | Ship docs updates in the same change and test wizard output/diagnostics together                                  |
| Touching status broadens user-visible behavior unnecessarily   | Low        | Keep status support to a hint-only reuse of the doctor check, or drop it if complexity grows                      |

## Rollback Plan

Revert the wizard prompt/validation changes and the new mismatch diagnostics, restoring prior init
and doctor/status behavior while keeping the existing `type = "symlink"` default for skills intact.
If any targeted linker guard is added, remove it in the same rollback so sync behavior returns to
the pre-change baseline.

## Dependencies

- Existing skills-target defaults on `main` from #261 remain the baseline and MUST stay unchanged.
- Tests covering init, doctor, status, and skills adoption need updates alongside docs to keep
  behavior aligned.

## Success Criteria

- [ ] `agentsync init --wizard` explicitly asks how each skills target should sync, recommends the
  migration-safe choice, and preserves existing directory-link setups by default.
- [ ] After wizard completion, users see a validation summary before first `apply` when config mode
  and on-disk skills shape disagree.
- [ ] `agentsync doctor` reports the known mode-semantic mismatch that currently appears healthy but
  causes `apply` churn; `status` provides a focused hint only if included without added noise.
- [ ] Documentation consistently describes skills targets as defaulting to `symlink`, explains when
  `symlink-contents` is still valid, and documents preservation/mismatch guidance.
