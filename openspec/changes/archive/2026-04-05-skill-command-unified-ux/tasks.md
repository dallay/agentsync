# Tasks: Unified CLI UX for Skill Commands

## Phase 1: Foundation — Shared Formatting Module

- [x] 1.1 Create `src/commands/skill_fmt.rs` with `LabelKind` enum (`Info`, `Warning`, `Success`, `Failure`) and derive `Debug, Clone, Copy, PartialEq, Eq`
- [x] 1.2 Add `OutputMode` enum (`Json`, `Human { use_color: bool }`) to `skill_fmt.rs` with same derives
- [x] 1.3 Add `HumanFormatter` struct with `use_color: bool`, implement `format_label(symbol, label, kind) -> String` using `colored` crate and `format_heading(heading) -> String`
- [x] 1.4 Add `detect_output_mode(json, stdout_is_tty, no_color, clicolor, term) -> OutputMode` pure function and `output_mode(json) -> OutputMode` convenience wrapper reading real env
- [x] 1.5 Add `pub mod skill_fmt;` to `src/commands/mod.rs`

## Phase 2: Core Implementation — Wire Commands + Refactor

- [x] 2.1 In `src/commands/skill.rs` `run_install`: replace `println!("Installed {}")` success path with `OutputMode::Human` branch using `HumanFormatter::format_label("✔", "installed", LabelKind::Success)`; replace error `println!` with `format_label("✗", "failed", LabelKind::Failure)` + `Hint:` line; keep JSON path and `tracing::error!` unchanged
- [x] 2.2 In `src/commands/skill.rs` `run_update`: same pattern — success `✔ updated`, error `✗ failed` + `Hint:`, JSON unchanged
- [x] 2.3 In `src/commands/skill.rs` `run_uninstall`: same pattern — success `✔ uninstalled`, error `✗ failed` + `Hint:`, JSON unchanged
- [x] 2.4 Refactor `suggest --install` internals: remove `SuggestInstallHumanFormatter` and `SuggestInstallLabelKind`, import `skill_fmt::{HumanFormatter, LabelKind}` instead; update `SuggestInstallLineReporter` and `SuggestInstallLiveReporter` and all `render_suggest_install_*` functions to use shared types
- [x] 2.5 Refactor `detect_suggest_install_output_mode()` to delegate color detection to `skill_fmt::output_mode(json)` and locally split `HumanLine`/`HumanLive` based on TTY + TERM; remove duplicated env-reading logic

## Phase 3: Testing

- [x] 3.1 Add unit tests in `src/commands/skill_fmt.rs` `#[cfg(test)]`: `format_label` colored vs plain for each `LabelKind` (SC-021a, SC-021b, SC-021c), distinct colors per kind, `format_heading` bold vs plain
- [x] 3.2 Add unit tests for `detect_output_mode`: JSON priority (SC-021d), TTY+color (SC-021e), non-TTY (SC-021f), `NO_COLOR` (SC-020a), `CLICOLOR=0` (SC-020b), `TERM=dumb` (SC-020c), empty `NO_COLOR`
- [x] 3.3 Add human-output focused tests for `install`/`update`/`uninstall` verifying `✔ verb skill_id` on success and `✗ failed skill_id:` + `Hint:` on error (SC-015d–SC-015j, SC-022a)
- [x] 3.4 Run existing suggest-install tests — confirm zero regressions from rename refactor (SC-021g); run `cargo test --all-features`

## Phase 4: Cleanup

- [x] 4.1 Verify no `SuggestInstallHumanFormatter` or `SuggestInstallLabelKind` references remain; clippy error in `src/skills/suggest.rs` is pre-existing (not from this change)
- [x] 4.2 Verify JSON contract tests pass unchanged (SC-015k): `tests/contracts/test_install_output.rs`, `tests/test_update_output.rs`
