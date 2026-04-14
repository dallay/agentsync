# Tasks: Fix Sync-Type-Aware Status For Symlink-Contents Targets

## Phase 1: Foundation

- [x] 1.1 Modify `src/linker.rs` to expose a reusable helper that derives expected `symlink-contents` child entries using apply rules (`pattern`, source existence, AGENTS compression).
- [x] 1.2 Refactor `src/commands/status.rs` to introduce the normalized target validation model (`sync_type`, destination kind, issues, managed children) used by both text and JSON output.

## Phase 2: Core Implementation

- [x] 2.1 Replace the destination-path-only collector in `src/commands/status.rs` with sync-type-aware validation for `symlink` and `symlink-contents`, preserving missing-source semantics and exit-code behavior.
- [x] 2.2 Extend `src/commands/status.rs` rendering/serialization so `symlink-contents` reports valid directory containers, the empty-valid case (`0 managed entries expected`), and child-level drift reasons.
- [x] 2.3 Update `src/commands/status.rs` handling for existing expanded targets (`module-map`) so the new validator stays compatible with current status coverage while keeping the narrowest behavior change.
- [x] 2.4 Update `src/init.rs` wizard/layout copy to describe `.agents/commands/` as the canonical source and agent command destinations as populated container directories, not destination symlinks.

## Phase 3: Regression Tests

- [x] 3.1 RED: add tests in `src/commands/status_tests.rs` for the spec scenarios where an empty existing `symlink-contents` source plus empty destination directory is valid in text and JSON status output.
- [x] 3.2 RED: add tests in `src/commands/status_tests.rs` for missing expected child, wrong child type, wrong child target, invalid destination type, and unchanged healthy/wrong-target `symlink` behavior.
- [x] 3.3 GREEN/REFACTOR: add or update integration-style status coverage in `src/commands/status_tests.rs` (or the closest existing harness) so `linker.sync()` fixtures verify healthy populated containers and skills-mode hints remain non-fatal.

## Phase 4: Documentation Cleanup And Alignment

- [x] 4.1 Update `README.md` to replace stale `.agents/command/` references, fix commands tree/examples, and explain sync-type-aware status for command destinations.
- [x] 4.2 Update `website/docs/src/content/docs/reference/configuration.mdx` so examples use `source = "commands"`, distinguish `.agents/commands/` from `.claude/commands`, `.gemini/commands`, and `.opencode/command/`, and document the empty-valid container case.
- [x] 4.3 Update `website/docs/src/content/docs/reference/cli.mdx` to document `status` validation by sync type, directory-container child checks, additive JSON details, and sweep related copy for lingering command/commands inconsistencies.
