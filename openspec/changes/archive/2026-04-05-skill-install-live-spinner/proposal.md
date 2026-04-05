# Proposal: Skill Install Live Spinner

## Intent

Improve the human experience of recommendation-driven installs in phase 1 by making `agentsync skill suggest --install` and `agentsync skill suggest --install --all` feel alive in a real terminal without changing machine contracts. Today the flow is correct but visually flat; this change adds lightweight live TTY feedback plus a clearer final summary so humans can scan progress and outcome faster while preserving readable non-TTY output and stable JSON.

## Scope

### In Scope
- Add terminal-aware live progress presentation for human-mode recommendation installs when stdout is a TTY and `--json` is not enabled.
- Add a more visual final summary block for recommendation-driven installs that makes installed, skipped, and failed counts easy to scan.
- Preserve plain, line-oriented, readable output for non-TTY human runs.
- Reuse the existing recommendation-install event flow and result model for `agentsync skill suggest --install` and `agentsync skill suggest --install --all` only.

### Out of Scope
- Any JSON contract, field, ordering, or shape changes for suggestion or recommendation-install responses.
- Expanding live presentation to direct `agentsync skill install`, `update`, or `uninstall` flows.
- Broad CLI theming, full-screen UI, alternate screen usage, or rich TUI interaction.
- Changes to recommendation detection, catalog generation, provider behavior, or install lifecycle semantics.

## Approach

Introduce a narrow terminal-aware presentation layer on top of the existing `SuggestInstallProgressReporter` events instead of changing install execution semantics. In TTY human mode, the CLI should render a lightweight single-activity spinner/live status presentation during resolve/install work and then print a more visual final summary block; in non-TTY human mode, it should keep emitting stable plain text lines with no cursor-control dependency; in JSON mode, it should continue emitting final JSON only. The implementation should stay scoped to the suggest-install command path and continue reusing the existing install-selected/install-all workflow and result aggregation.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/commands/skill.rs` | Modified | Add terminal-aware presenter selection, TTY live activity rendering, and richer final human summary formatting for suggest-install flows. |
| `src/skills/suggest.rs` | Modified | Continue exposing/reusing install progress events and final result aggregation without changing install semantics or JSON payload shape. |
| `tests/integration/skill_suggest.rs` | Modified | Extend human-output coverage for TTY/non-TTY recommendation-install behavior and final summary readability. |
| `tests/contracts/test_skill_suggest_output.rs` | Modified | Preserve JSON contract guarantees and verify no human progress output leaks into JSON mode. |
| `tests/unit/` | Modified | Add focused presenter/output-mode tests for terminal-aware live activity and summary rendering decisions. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Live terminal rendering becomes noisy or brittle across shells/CI | Medium | Limit live behavior to TTY-only human mode, keep non-TTY fallback line-oriented, and avoid full-screen terminal control. |
| Spinner/progress rendering accidentally leaks into JSON mode | Low | Keep JSON mode on a separate output path and add contract coverage that stdout remains valid JSON only. |
| Visual summary obscures underlying status meaning | Low | Ensure every visual element still includes explicit text labels for installed, skipped, and failed outcomes. |

## Rollback Plan

Revert the suggest-install presenter changes in `src/commands/skill.rs` and restore the existing line-reporter/final-summary behavior while keeping install execution untouched in `src/skills/suggest.rs`. If regressions appear, disable the TTY live presenter path and fall back to plain line-oriented reporting for all human-mode recommendation installs.

## Dependencies

- Existing suggest-install progress events and result aggregation in `src/skills/suggest.rs`
- Current terminal detection and human-vs-JSON branching in `src/commands/skill.rs`
- Test coverage for JSON contracts and non-TTY readability

## Success Criteria

- [ ] In TTY human mode, `agentsync skill suggest --install` and `agentsync skill suggest --install --all` show lightweight live progress that is easier to scan than plain lines.
- [ ] In non-TTY human mode, recommendation installs remain readable, line-oriented, and independent of cursor movement or spinner-only meaning.
- [ ] In JSON mode, recommendation-install stdout remains valid JSON with the existing response contract and no interleaved human progress output.
- [ ] Final human summaries clearly show installed, already-installed, and failed outcomes without changing install semantics.
