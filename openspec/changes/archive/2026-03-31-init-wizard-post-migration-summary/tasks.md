# Tasks: Init Wizard Post-Migration Summary

## Phase 1: Foundation

- [x] 1.1 In `src/init.rs`, add a private wizard-summary facts type and `BackupOutcome` enum that capture only wizard-known data: migrated/skipped counts, AGENTS/config write-or-preserve state, canonical `.agents/` ownership, and backup result.
- [x] 1.2 In `src/init.rs`, extract a pure summary-rendering helper that returns text/lines for the final wizard summary and next steps from those facts, with no filesystem or git inspection.

## Phase 2: TDD for Summary Rules

- [x] 2.1 In `src/init.rs` tests, write RED unit tests for the renderer that assert required substrings for canonical `.agents/` messaging, `agentsync apply` as the next step, collaborator/gitignore guidance, and manual git review language.
- [x] 2.2 In `src/init.rs` tests, write RED unit tests for backup outcomes (`Completed`, `Declined`, `NotOffered`) and for forbidden claims: no “apply already ran”, no “.gitignore already updated”, and no reported git cleanliness/staging state.

## Phase 3: Wizard Flow Integration

- [x] 3.1 In `src/init.rs`, collect/update summary facts during `init_wizard()` migration work: instruction merges, copied/skipped items, and whether `.agents/AGENTS.md` / `.agents/agentsync.toml` were created or preserved.
- [x] 3.2 In `src/init.rs`, thread backup handling through the new `BackupOutcome` enum and render the wizard-only final summary after the backup prompt/moves are fully resolved.
- [x] 3.3 In `src/init.rs`, ensure the rendered text describes `.agents/` as the canonical source of truth and frames downstream targets as future reconciliation done by `agentsync apply`.

## Phase 4: Footer Coordination

- [x] 4.1 In `src/main.rs`, gate the generic init “Next steps” footer so the shared success banner still prints, but wizard runs do not emit duplicate or conflicting footer guidance.
- [x] 4.2 In `src/main.rs` tests or a nearby focused helper test, add a RED/GREEN assertion that wizard mode suppresses the generic footer branch while non-wizard init still keeps it.

## Phase 5: Regression Verification

- [x] 5.1 Add one narrow integration regression test in `tests/` or the existing init-related test module that exercises completion output boundaries for `init --wizard` using stable substring assertions instead of full-output snapshots.
- [x] 5.2 Run targeted Rust tests for the new renderer/footer coverage first, then run the relevant broader init/adoption test command to confirm the summary change does not regress existing migration behavior.
