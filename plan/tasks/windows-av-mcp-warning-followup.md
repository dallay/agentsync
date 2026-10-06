# Windows library AV and legacy MCP warning follow-up

## Route

Delegated direct. The user authorized fixing only review findings that remain valid. No formal SDD cycle was requested.

## Current state

- Parent #632: `d52c6e7`; child #633: `db7c0c5` (parent is an ancestor through the normal stack merge).
- Latest check snapshot after `db7c0c5`: #632 has 40 pass / 5 fail; #633 has 1 pass / 39 pending, including both Windows test jobs. Both PRs are `CHANGES_REQUESTED` and not drafts. Review threads and draft state must remain untouched.
- Windows job `112277715557` on run `37466196607` failed the serial library run with `STATUS_ACCESS_VIOLATION` (`0xc0000005`). The quarantine regression reports `ok`; output begins the next test `copy_backup_contents_refuses_existing_regular_file_without_modifying_it`.
- Workflow diagnostic commit `93bc518` added that exact Windows test in isolation and removed `continue-on-error` from the serial library step. The isolated test itself reproduced the same AV in job `112322951556` on run `37479309135`; this rules out the preceding quarantine test as the primary cause, but does not identify the exact call. Temporary env-gated markers are now published in `040afa2`; current Windows jobs were pending at the latest check snapshot.
- MCP warning regression was observed RED (`skipped=1` with no `.mcp.json`), fixed and published as #633 commit `3be4b18`; focused test, format check, Clippy, and the full Linux pre-push suite passed. The publication tracker correction is now present locally and awaits its work-unit commit.

## Tasks

- [ ] Instrumentation in #633 `040afa2` showed Windows jobs `112351226699` and `112351266976` reproducibly AV inside `GetSecurityDescriptorDacl` after SDDL conversion succeeds. All four callsites in `src/linker/revert.rs` passed null for `lpbDaclDefaulted`; Microsoft documents it as an output pointer, while sibling calls in `src/mcp.rs` use a local BOOL. The candidate fix is published in #633 `db7c0c5`, supplying a local output at all four callsites. Await exact-test GREEN before removing markers; do not change DACL policy.
- [x] Add a regression for `warn_if_legacy_mcp_is_unowned`: active MCP plus a supported configured agent but no existing resolved config entry leaves `skipped` unchanged; creating `.mcp.json` increments it once. RED/GREEN confirmed locally; implementation published in #633 `3be4b18`.
- [x] Correct stale publication/status prose in `plan/tasks/revert-command.md`: current refs #632/#633 are recorded; the historical push of `a9faa7c` is distinguished from current ancestry; both PRs are correctly marked `CHANGES_REQUESTED` and not drafts.
- [x] Preserve already-satisfied review items and prior policy decisions: the ENOTSUP ACL handling, incomplete WalkDir status, Z-Code matcher reuse, staged permission paths, `$HOME` exception, journal rebase recovery, and Windows DACL guide are already present. Keep no in-place fallback after Unix `chown` failure and keep unsupported Unix ACL replacement fail-closed/documented; do not add a Linux-only gate where tests use explicit `AGENTSYNC_DATA_DIR` isolation.
- [ ] Validate the targeted MCP unit test locally and obtain Windows CI evidence for the isolated and serial library tests. Do not claim Windows GREEN without its runner.

## Acceptance

- The isolated test either passes or provides a focused reproducible failure; the serial diagnostic no longer masks its exit status.
- The MCP warning test distinguishes absent from present resolved destinations.
- The task tracker reflects verified current publication state.
- No unrelated review findings are changed; no GitHub thread is replied to/resolved and no draft state is changed.
