# Tasks: Clean up managed .gitignore blocks when gitignore is disabled

## Phase 1: RED - Lock in failing behavior

- [x] 1.1 Add unit tests in `src/gitignore.rs` for `cleanup_gitignore(...)` covering stale-block
  removal, custom-marker targeting, dry-run no-write, and no-op/idempotent behavior when no matching
  block exists.
- [x] 1.2 Add CLI regression tests in `tests/test_bug.rs` or `tests/test_update_output.rs` for
  `agentsync apply` with `[gitignore].enabled = false`: normal cleanup removes only the managed
  block; repeat apply leaves `.gitignore` byte-for-byte unchanged.
- [x] 1.3 Add CLI regression tests in `tests/test_bug.rs` or `tests/test_update_output.rs` for
  `agentsync apply --dry-run`, `--no-gitignore`, and `--dry-run --no-gitignore`, asserting stdout
  reports cleanup only when applicable and `.gitignore` is untouched otherwise.
- [x] 1.4 Add/adjust a focused doctor expectation test in `src/commands/doctor_tests.rs` proving
  disabled gitignore management treats a cleaned or missing managed block as healthy and does not
  request restoration.

## Phase 2: GREEN - Implement cleanup behavior

- [x] 2.1 In `src/gitignore.rs`, add public `cleanup_gitignore(project_root, marker, dry_run)` that
  reads `.gitignore`, resolves configured markers, reuses `remove_managed_section(...)`, and returns
  success when the file is absent.
- [x] 2.2 In `src/gitignore.rs`, make cleanup output apply-style status for real and dry-run
  cleanup, and skip writes when content is unchanged so mtime/content stay stable for no-op runs.
- [x] 2.3 In `src/main.rs`, replace the current update-only branch with explicit `.gitignore` flow:
  `--no-gitignore` skip, enabled -> `update_gitignore(...)`, disabled -> `cleanup_gitignore(...)`.

## Phase 3: REFACTOR - Align diagnostics and shared behavior

- [x] 3.1 Review `src/commands/doctor.rs` and make the smallest adjustment needed so disabled
  configs with no managed block remain non-issues while existing enabled-state audits stay
  unchanged.
- [x] 3.2 Refactor any duplicated marker/change-detection logic between update and cleanup paths in
  `src/gitignore.rs` only if needed to keep both paths consistent and idempotent.

## Phase 4: Verification

- [x] 4.1 Run targeted checks for touched areas: `cargo test --lib gitignore`, `cargo test doctor`,
  and the specific CLI test file(s) updated for disabled cleanup scenarios.
- [x] 4.2 If targeted tests pass, run `cargo test --all-features` to confirm the cleanup path does
  not regress broader apply or doctor behavior.
