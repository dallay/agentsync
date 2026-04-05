# Proposal: Improve Skill Install UX Feedback

## Intent

Human users running `agentsync skill suggest --install`, especially `--install --all`, currently get
little or no real-time feedback while installs are resolving and executing. This change narrows in on
the CLI experience so users can see what is happening per skill as work progresses, without changing
the stable machine-readable `--json` contract.

## Scope

### In Scope
- Add human-readable live status output for recommendation-driven install flows only.
- Show per-skill progress updates for resolve/install/skip/fail states during `skill suggest --install`
  and `skill suggest --install --all`.
- Use lightweight colored/status messaging, with optional spinner-style activity only when stdout is a
  TTY and the implementation stays simple and safe.
- Preserve non-interactive safety by falling back to plain line-oriented output instead of relying on
  terminal control behavior.

### Out of Scope
- Any change to `--json` output shape, fields, or semantics.
- Broad redesign of other `skill` subcommands or the direct `skill install` command UX.
- Rich progress bars, concurrent installs, or terminal UI frameworks.
- Build, packaging, or docs-site changes.

## Approach

Introduce a small human-mode progress reporting layer around the existing recommendation install loop
so the CLI can emit start, in-progress, skip, success, and failure updates as each skill is handled.
Keep the existing install execution path and result contract intact, but let human output stream live
events while JSON mode continues to emit only the final structured payload. Gate any spinner behavior
behind TTY detection and keep non-TTY output deterministic and line-based.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/commands/skill.rs` | Modified | Branch human vs JSON install rendering and wire live feedback for `skill suggest` install flows. |
| `src/skills/suggest.rs` | Modified | Add progress event hooks/reporting around recommendation-driven install selection and execution. |
| `tests/` | Modified | Add focused CLI/integration coverage for human-mode live feedback, install-all progress, and non-interactive-safe output. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Live output breaks expected non-interactive usage or test determinism | Medium | Restrict spinner/control sequences to TTYs and keep non-TTY output plain, ordered, and stable. |
| Human-readable changes accidentally alter JSON behavior | Low | Keep JSON code path unchanged and verify existing JSON contract coverage still passes. |
| Extra status plumbing makes install flow harder to maintain | Medium | Use a narrow reporter abstraction or callback surface localized to suggestion install flows only. |

## Rollback Plan

Revert the progress-reporting changes in `src/commands/skill.rs`, `src/skills/suggest.rs`, and the
associated tests so recommendation installs return to final-summary-only human output. Because the
change is CLI-only and does not alter persisted data formats, rollback does not require migration.

## Dependencies

- Existing Clap-based `skill suggest` command structure and current recommendation install flow.
- Existing colored CLI output conventions already used in the Rust CLI.
- TTY detection already available from the standard library for interactive safety decisions.

## Success Criteria

- [ ] Human-mode `agentsync skill suggest --install --all` shows live per-skill feedback before final completion output.
- [ ] Already installed recommendations are clearly reported as skipped in real time instead of only appearing in the final summary.
- [ ] `--json` output remains unchanged for both read-only and install suggestion flows.
- [ ] Non-interactive runs remain safe, readable, and free from spinner/control-sequence noise.
