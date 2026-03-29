# Tasks: Wizard Preserve Existing Skill Links

## Phase 1: Shared layout detection

- [x] 1.1 Create `src/skills_layout.rs` with a helper that inspects only `skills` targets, recognizes `SymlinkContents` configs whose destination is already a directory symlink to the expected `.agents/skills` source, and returns structured mismatch data plus remediation text.
- [x] 1.2 Register the new module in `src/lib.rs` and add focused unit coverage in `src/skills_layout.rs` or adjacent tests for recognized directory-symlink matches and ignored unrelated layouts.

## Phase 2: Wizard UX and config generation

- [x] 2.1 Refactor `src/init.rs` to derive per-agent skills choices from the shared helper, building pure planning data that recommends `SyncType::Symlink` for recognized directory symlinks and stays silent when no skills target is generated.
- [x] 2.2 Update the wizard flow in `src/init.rs` so each generated `skills` target prompts with explicit `symlink` vs `symlink-contents` choices, shows the recommendation, and preserves user overrides in the collected plan.
- [x] 2.3 Replace direct `DEFAULT_CONFIG` emission in `src/init.rs` with a targeted template renderer that updates only the selected skills `type` lines while preserving comments/order in the starter config.
- [x] 2.4 Add `src/init.rs` tests for recommendation planning, override handling, no-extra-prompt cases, and config rendering that changes only intended skills targets.

## Phase 3: Post-init validation and diagnostics

- [x] 3.1 Add a post-write validation summary in `src/init.rs` that reuses the shared helper to warn before exit when a generated skills target’s configured mode disagrees with the observed directory-symlink layout.
- [x] 3.2 Update `src/commands/doctor.rs` to report the recognized skills mode-semantic mismatch as a warning that names the target, cites `symlink-contents` versus directory symlink shape, and warns about apply churn.
- [x] 3.3 Update `src/commands/status.rs` to emit a hint-only message for the same recognized mismatch without marking the target broken or changing normal success semantics.
- [x] 3.4 Extend `src/commands/doctor_tests.rs` and `src/commands/status_tests.rs` for mismatch warning/hint coverage and clean `symlink`-match regressions.

## Phase 4: Regression coverage

- [x] 4.1 Extend `tests/test_agent_adoption.rs` with a fixture where `.claude/skills` already symlinks to `.agents/skills`, verifying wizard/default preservation keeps `type = "symlink"` for skills while existing command-target behavior remains unchanged.
- [x] 4.2 Add regression coverage in `src/init.rs` or adoption-style tests for the override path where a forced `symlink-contents` choice triggers the post-init warning before command exit.

## Phase 5: Documentation

- [x] 5.1 Update `website/docs/src/content/docs/reference/configuration.mdx` and `website/docs/src/content/docs/guides/skills.mdx` to describe `symlink` as the default skills mode, when `symlink-contents` is still valid, and how wizard preservation works.
- [x] 5.2 Update `website/docs/src/content/docs/reference/cli.mdx` to document the explicit wizard skills prompt, post-init validation summary, doctor warning, and status hint for recognized mismatches.
