# Windows library AV and legacy MCP warning follow-up

## Route

Delegated direct. The user authorized fixing only review findings that remain valid. No formal SDD cycle was requested.

## Current state

- Parent #632 remote: `9b2b871`; child #633 remote: `db7c0c5`. Local child branch `fix/revert-stack-mcp-parent-sync` has the ordinary parent merge `53a5892` and tracker commit `be91f03`; the diagnostic cleanup and focused tests are validated locally and pending commit/push.
- Latest check snapshot: #632 has 22 pass / 19 pending / 3 fail (Codecov patch, SonarCloud, Semgrep); #633 has 41 pass / 4 fail (SonarCloud, two Windows jobs, Semgrep). Both PRs are `CHANGES_REQUESTED` and not drafts. Review threads and draft state must remain untouched.
- Windows job `112277715557` on run `37466196607` failed the serial library run with `STATUS_ACCESS_VIOLATION` (`0xc0000005`). The quarantine regression reports `ok`; output begins the next test `copy_backup_contents_refuses_existing_regular_file_without_modifying_it`.
- Workflow diagnostic commit `93bc518` added the exact Windows test in isolation and removed `continue-on-error` from the serial library step. Commit `040afa2` instrumented it; Windows logs isolated the AV to a null `lpbDaclDefaulted` argument in four `GetSecurityDescriptorDacl` calls. The fix was published in #633 `db7c0c5`, and the isolated Windows test passed there. The serial suite then exposed four distinct failures: the approved Windows early-reject case, an unsupported test-only staged-copy case, and two Z-Code orphan cases failing to read restored `foo.md`.
- The parent commits `bfa4e77` and `9b2b871` now contain the pointer fix and early Windows `--keep-backups` rejection. Parent #632 `9b2b871` is pushed. The child merge `53a5892` carries those fixes locally through a normal merge; it is not yet pushed.
- MCP warning regression was observed RED (`skipped=1` with no `.mcp.json`), fixed and published as #633 commit `3be4b18`; focused test, format check, Clippy, and Linux pre-push passed. Tracker publication text was corrected in `be91f03` locally.

## Tasks

- [x] RPI-058: all four `GetSecurityDescriptorDacl` callsites now pass a local defaulted output. The isolated Windows backup-copy test passed on #633 `db7c0c5`; temporary markers and workflow env have since been removed in the local child worktree. No DACL policy changed.
- [x] RPI-057 implementation: Windows `Linker::revert` now rejects `--keep-backups` before output/mutation, and the CLI/troubleshooting docs explain the platform limit. Published parent-first in #632 `9b2b871`, included in local child merge `53a5892`; final Windows regression evidence is pending.
- [x] Gate `restore_staged_publication_uses_no_replace_for_files_and_directories` to Unix because the Windows test-only helper correctly refuses inherited-DACL copies. Focused Linux test passes.
- [x] Result-counter assertions now precede the missing `foo.md` reads in both Z-Code orphan tests. The Unix-only staged-copy test gate is in place. Local `cargo test --lib linker::revert::tests` passes all 43 tests, and Clippy passes; rerun on Windows to see the counters before considering any production matcher or backup-move change.
- [ ] Push the child’s ordinary merge and focused test updates after local checks. Keep current thread/draft constraints; no rebase or force push.
- [x] Add a regression for `warn_if_legacy_mcp_is_unowned`: active MCP plus a supported configured agent but no existing resolved config entry leaves `skipped` unchanged; creating `.mcp.json` increments it once. RED/GREEN confirmed locally; implementation published in #633 `3be4b18`.
- [x] Correct stale publication/status prose in `plan/tasks/revert-command.md`: historical `a9faa7c` push is distinguished from current ancestry; both PRs are correctly marked `CHANGES_REQUESTED` and not drafts. The local snapshot now records #632 `9b2b871` and #633 remote `db7c0c5`.
- [x] Preserve already-satisfied review items and prior policy decisions: the ENOTSUP ACL handling, incomplete WalkDir status, Z-Code matcher reuse, staged permission paths, `$HOME` exception, journal rebase recovery, and Windows DACL guide are already present. Keep no in-place fallback after Unix `chown` failure and keep unsupported Unix ACL replacement fail-closed/documented; do not add a Linux-only gate where tests use explicit `AGENTSYNC_DATA_DIR` isolation.
- [x] Targeted MCP warning regression is GREEN locally (the test was observed RED before the guard). The remaining acceptance gate is Windows CI for the serial revert suite; do not claim Windows GREEN without its runner.

## Acceptance

- The isolated test either passes or provides a focused reproducible failure; the serial diagnostic no longer masks its exit status.
- The MCP warning test distinguishes absent from present resolved destinations.
- The task tracker reflects verified current publication state.
- No unrelated review findings are changed; no GitHub thread is replied to/resolved and no draft state is changed.
