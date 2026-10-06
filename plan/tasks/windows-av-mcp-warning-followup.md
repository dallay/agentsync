# Windows library AV and legacy MCP warning follow-up

## Route

Delegated direct. The user authorized fixing only review findings that remain valid. No formal SDD cycle was requested.

## Current state

- Parent #632 remote: `9b2b871`; child #633 remote: `2f15fa3`. Child contains ordinary merge `53a5892`, tracker/docs updates, marker cleanup `984f896`, scoped error tracing `92162fe`, and RPI-060 marker/workflow commit `1e9de97`; follow-up API-level markers are now local.
- Latest check snapshot after `2f15fa3`: #632 has 41 pass / 1 pending / 3 fail; #633 has 36 pass / 4 pending / 5 fail (both Windows jobs, Security Audit, SonarCloud, Semgrep). Both PRs are `CHANGES_REQUESTED` and not drafts. Review threads and draft state must remain untouched.
- Windows job `112277715557` on run `37466196607` failed the serial library run with `STATUS_ACCESS_VIOLATION` (`0xc0000005`). The quarantine regression reports `ok`; output begins the next test `copy_backup_contents_refuses_existing_regular_file_without_modifying_it`.
- Workflow diagnostic commit `93bc518` added the exact Windows test in isolation and removed `continue-on-error` from the serial library step. Commit `040afa2` instrumented it; Windows logs isolated the AV to a null `lpbDaclDefaulted` argument in four `GetSecurityDescriptorDacl` calls. The fix was published in #633 `db7c0c5`, and the isolated Windows test passed there. The serial suite then exposed four distinct failures: the approved Windows early-reject case, an unsupported test-only staged-copy case, and two Z-Code orphan cases failing to read restored `foo.md`.
- The parent commits `bfa4e77` and `9b2b871` contain the pointer fix and early Windows `--keep-backups` rejection. Both Windows jobs for parent #632 `9b2b871` pass. Child #633 `2f15fa3` carries those commits through ordinary merge `53a5892`; its Windows handle-stage trace jobs are pending.
- Windows jobs on child `984f896` and `1e9de97` consistently fail the two Z-Code orphans with `restored=0, skipped=0, errors=1`. Tracing confirms `ERROR_ACCESS_DENIED` in `move_entry_no_replace`; markers in `1e9de97` show source open/metadata pass, then the first quarantine rename fails. On `2f15fa3`, the new marker says `ReOpenFile returned valid=false`, before `SetFileInformationByHandle` is reached. The error shown after that marker is not reliable because `eprintln!` may overwrite thread last-error; the earlier trace established error 5 at this boundary.
- MCP warning regression was observed RED (`skipped=1` with no `.mcp.json`), fixed and published as #633 commit `3be4b18`; focused test, format check, Clippy, and Linux pre-push passed. Tracker publication text was first corrected in `be91f03`; the current head/check refresh remains local.

## Tasks

- [x] RPI-058: all four `GetSecurityDescriptorDacl` callsites now pass a local defaulted output. The isolated Windows backup-copy test passed on #633 `db7c0c5`; temporary markers and workflow env have since been removed in the local child worktree. No DACL policy changed.
- [x] RPI-057: Windows `Linker::revert` rejects `--keep-backups` before output/mutation; docs explain the platform limit. Published parent-first in #632 `9b2b871`; both Windows jobs for that head pass, and implementation is merged into child `2f15fa3`.
- [x] Gate `restore_staged_publication_uses_no_replace_for_files_and_directories` to Unix because the Windows test-only helper correctly refuses inherited-DACL copies. Focused Linux test passes.
- [x] Result-counter assertions now precede the missing `foo.md` reads in both Z-Code orphan tests. Windows counters confirm `errors=1`, `skipped=0`, `restored=0`; eligibility is not the failing branch.
- [x] Test-scoped WARN tracing with `with_test_writer()` in `92162fe` localized the failure to the Windows no-replace move and reported `ERROR_ACCESS_DENIED` (5).
- [ ] `2f15fa3` markers show `ReOpenFile` returns `INVALID_HANDLE_VALUE`; `SetFileInformationByHandle` is not reached. Its subsequent `last_os_error()` may be clobbered by diagnostic output, so error 6 is unreliable; the earlier uninstrumented run reported error 5. Local follow-up now captures last-error immediately and probes the same handle/share/flags without `FILE_TRAVERSE`, logging validity/error and closing any probe handle. Local fmt, revert module tests, and Clippy pass. Await Windows results before changing production rights or rename behavior.
- [x] The Unix-only staged-copy test gate is in place. Local `cargo test --lib linker::revert::tests` passes all 43 tests, and Clippy passes.
- [x] Push the child’s ordinary merge and focused test updates after local checks. Published as child #633 `984f896`, `92162fe`, and `2f15fa3` with normal fast-forwards; no rebase or force push.
- [x] Add a regression for `warn_if_legacy_mcp_is_unowned`: active MCP plus a supported configured agent but no existing resolved config entry leaves `skipped` unchanged; creating `.mcp.json` increments it once. RED/GREEN confirmed locally; implementation published in #633 `3be4b18`.
- [x] Correct stale publication/status prose in `plan/tasks/revert-command.md`: historical `a9faa7c` push is distinguished from current ancestry; both PRs are correctly marked `CHANGES_REQUESTED` and not drafts. Current refs/check snapshot: #632 `9b2b871`, #633 `2f15fa3`.
- [x] Preserve already-satisfied review items and prior policy decisions: the ENOTSUP ACL handling, incomplete WalkDir status, Z-Code matcher reuse, staged permission paths, `$HOME` exception, journal rebase recovery, and Windows DACL guide are already present. Keep no in-place fallback after Unix `chown` failure and keep unsupported Unix ACL replacement fail-closed/documented; do not add a Linux-only gate where tests use explicit `AGENTSYNC_DATA_DIR` isolation.
- [x] Targeted MCP warning regression is GREEN locally (the test was observed RED before the guard). The remaining acceptance gate is Windows CI for the serial revert suite; do not claim Windows GREEN without its runner.

## Acceptance

- The isolated test either passes or provides a focused reproducible failure; the serial diagnostic no longer masks its exit status.
- The MCP warning test distinguishes absent from present resolved destinations.
- The task tracker reflects verified current publication state.
- No unrelated review findings are changed; no GitHub thread is replied to/resolved and no draft state is changed.
