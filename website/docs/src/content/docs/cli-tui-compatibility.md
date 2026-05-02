---
title: CLI and TUI compatibility contract
---

# CLI and TUI compatibility contract

AgentSync can improve its human interface without breaking existing CLI users when every output change follows this contract.

## Stable machine output

Commands that support `--json` must treat `--json` as the highest-priority output mode. When `--json` is set:

- stdout must contain machine-readable JSON only.
- Human headings, colors, spinners, progress frames, and hints must not be mixed into stdout.
- Existing JSON field names and meanings should remain stable unless a schema migration is explicitly documented.
- Error exits should preserve the command's existing exit-code behavior and return structured JSON whenever the command already supports JSON errors.

## Human output mode

Human output is optimized for people and may use headings, labels, Unicode status symbols, hints, and summaries. Human output must remain readable when copied from logs or piped into another command.

Use the shared formatter and output-mode detection rules instead of ad hoc color or TTY checks.

## TTY and color rules

Color is enabled only when stdout is a TTY and no supported color-disabling override is active.

The current color-disabling overrides are:

- `NO_COLOR` with any non-empty value.
- `CLICOLOR=0`.
- `TERM=dumb`, matched case-insensitively.

When stdout is not a TTY, AgentSync should continue in plain human output unless the user requested `--json`.

## CI and piped output

CI and piped output should be line-oriented and deterministic. Commands must not require cursor control, alternate-screen mode, or carriage-return animation when stdout is not an interactive TTY.

Progress-heavy flows may keep a live renderer for safe TTY contexts, but they must provide a plain line renderer for non-TTY contexts.

## Exit codes

Output rendering must not change command success or failure semantics. A formatting-only change should preserve existing exit codes for success, validation failures, partial failures, and unsupported terminal states.

## Non-interactive fallbacks

AgentSync supports non-interactive fallbacks for interactive flows. Interactive flows must detect unsupported non-interactive contexts before prompting. In non-interactive contexts, a command should either:

- use an explicit non-interactive path, such as an `--all` or dry-run mode;
- fall back to the existing safe renderer; or
- exit with a clear diagnostic and remediation.

A command must not hang waiting for input that cannot arrive.

## Full-screen TUI compatibility

A full-screen TUI is allowed only when it is explicitly opt-in or when AgentSync can prove the terminal context is safe and interactive. Until a future release changes this contract deliberately, full-screen TUI behavior must not replace the existing default wizard behavior.

Any full-screen TUI must:

- initialize only after confirming the required TTY capabilities;
- clean up terminal state on success, error, and cancellation;
- provide cancellation semantics that preserve existing generated files unless the user confirms changes;
- fall back to the existing wizard or exit with a clear message when initialization fails; and
- keep `--json` and non-interactive command paths free from full-screen rendering.
