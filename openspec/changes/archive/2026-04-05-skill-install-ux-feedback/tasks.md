# Tasks: Improve Skill Install UX Feedback

## Phase 1: Progress Reporting Foundation

- [x] 1.1 Add `SuggestInstallPhase`, `SuggestInstallProgressEvent`, and an internal reporter interface in `src/skills/suggest.rs` with a no-op implementation that preserves current public install helpers.
- [x] 1.2 Refactor the recommendation install loop in `src/skills/suggest.rs` into an internal `*_with_reporter` path that emits ordered events for resolve, install, skip-already-installed, success, and failure while keeping `SuggestInstallJsonResponse` unchanged.

## Phase 2: Human CLI Output Wiring

- [x] 2.1 Add a small output-mode helper in `src/commands/skill.rs` that distinguishes human vs `--json`, checks `stdout.is_terminal()`, and disables ANSI color when `NO_COLOR` or `CLICOLOR=0` is set.
- [x] 2.2 Implement a line-oriented suggest-install reporter in `src/commands/skill.rs` that renders explicit labels like `resolving`, `installing`, `installed`, `already installed`, and `failed` without cursor control.
- [x] 2.3 Wire `skill suggest --install` and `skill suggest --install --all` in `src/commands/skill.rs` to use the human reporter only outside `--json`, including batch framing and final “nothing installable” / completion summary messaging.

## Phase 3: Focused Verification

- [x] 3.1 Extend `tests/unit/suggest_install.rs` with reporter-recording tests that assert exact event order and statuses for success, skip-after-registry-recheck, resolve failure, and install failure.
- [x] 3.2 Add unit coverage in `src/commands/skill.rs` or its adjacent test module for output-mode detection across TTY, non-TTY, `NO_COLOR`, `CLICOLOR=0`, and `--json` cases.
- [x] 3.3 Update `tests/integration/skill_suggest.rs` to assert non-TTY human `--install --all` output is stable, line-oriented, contains skip/failure/success messages, and contains no ANSI escape sequences.
- [x] 3.4 Update `tests/contracts/test_skill_suggest_output.rs` to confirm `skill suggest --install --all --json` and guided `--install --json` still emit only the existing final JSON contract.

## Phase 4: End-to-End CLI UX Coverage

- [x] 4.1 Extend `tests/e2e/scenarios/04-suggest-install-guided.sh` to capture the pseudo-TTY transcript and assert selected skills show per-skill status lines before the final summary.
- [x] 4.2 Extend `tests/e2e/scenarios/05-suggest-install-all.sh` to verify first-run install-all progress, repeat-run `already installed` skips, and clear `nothing installable` messaging.
