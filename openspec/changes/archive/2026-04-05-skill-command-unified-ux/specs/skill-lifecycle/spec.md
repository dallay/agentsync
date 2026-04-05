# Delta for Skill Lifecycle

## MODIFIED Requirements

### Requirement: REQ-015 — CLI Human Output Format

The CLI MUST render human-readable output for `install`, `update`, and `uninstall` commands using
the same colored status-line style used by `suggest --install`.

(Previously: Human output used bare `println!("Installed {skill_id}")`, `println!("Updated {skill_id}")`,
`println!("Uninstalled {skill_id}")` for success, and `error!()` + `println!("Hint: ...")` for errors —
no Unicode symbols, no color, no TTY awareness.)

#### Success output

On success, the CLI MUST print a single status line in the format:

```
{symbol} {verb} {skill_id}
```

| Command     | Symbol | Verb          |
|-------------|--------|---------------|
| `install`   | `✔`    | `installed`   |
| `update`    | `✔`    | `updated`     |
| `uninstall` | `✔`    | `uninstalled` |

When color is enabled, the status line MUST be rendered green and bold.
When color is disabled, the status line MUST be rendered as plain text with the Unicode symbol
preserved (no ANSI escape codes).

#### Error output

On error, the CLI MUST print two lines:

1. A failure status line: `✗ failed {skill_id}: {error_message}`
2. A hint line: `Hint: {remediation_message}`

When color is enabled, the failure status line MUST be rendered red and bold.
When color is disabled, both lines MUST be rendered as plain text with the Unicode symbol preserved.

The hint line MUST NOT be colored (same as current behavior of `suggest --install` hint lines).

#### JSON output unchanged

JSON output (`--json`) MUST remain byte-identical to current behavior for all commands. No symbols,
no color, same schema, same field names, same serialization.

#### Scenario: SC-015d — Install success in TTY with color

- GIVEN stdout is a TTY
- AND `NO_COLOR` is not set
- AND `CLICOLOR` is not `0`
- AND `TERM` is not `dumb`
- WHEN `agentsync skill install my-skill` succeeds
- THEN stdout MUST contain `✔ installed my-skill`
- AND the output MUST include ANSI green+bold escape codes around the status text

#### Scenario: SC-015e — Install success in non-TTY (piped)

- GIVEN stdout is NOT a TTY (e.g., piped to a file or another process)
- WHEN `agentsync skill install my-skill` succeeds
- THEN stdout MUST contain `✔ installed my-skill`
- AND the output MUST NOT contain any ANSI escape codes

#### Scenario: SC-015f — Update success in TTY with color

- GIVEN stdout is a TTY with color enabled
- WHEN `agentsync skill update my-skill` succeeds
- THEN stdout MUST contain `✔ updated my-skill`
- AND the output MUST include ANSI green+bold escape codes

#### Scenario: SC-015g — Uninstall success in TTY with color

- GIVEN stdout is a TTY with color enabled
- WHEN `agentsync skill uninstall my-skill` succeeds
- THEN stdout MUST contain `✔ uninstalled my-skill`
- AND the output MUST include ANSI green+bold escape codes

#### Scenario: SC-015h — Install error in TTY with color

- GIVEN stdout is a TTY with color enabled
- WHEN `agentsync skill install bad-skill` fails with error message `"source not found"`
- THEN stdout MUST contain `✗ failed bad-skill: source not found`
- AND the failure line MUST include ANSI red+bold escape codes
- AND stdout MUST contain a second line starting with `Hint:`

#### Scenario: SC-015i — Install error in non-TTY

- GIVEN stdout is NOT a TTY
- WHEN `agentsync skill install bad-skill` fails
- THEN stdout MUST contain `✗ failed bad-skill:` followed by the error message
- AND stdout MUST contain `Hint:` on a subsequent line
- AND the output MUST NOT contain any ANSI escape codes

#### Scenario: SC-015j — Uninstall not-found error in TTY

- GIVEN stdout is a TTY with color enabled
- AND skill `missing-skill` is not installed
- WHEN `agentsync skill uninstall missing-skill` is invoked
- THEN stdout MUST contain `✗ failed missing-skill:` followed by the not-found message
- AND stdout MUST contain `Hint:` with remediation mentioning `list`

#### Scenario: SC-015k — JSON output unchanged after refactor

- GIVEN `--json` flag is set
- WHEN any of `install`, `update`, or `uninstall` succeeds or fails
- THEN the JSON output MUST match the existing schema exactly (fields, types, values)
- AND the output MUST NOT contain Unicode symbols or ANSI escape codes

---

## ADDED Requirements

### Requirement: REQ-020 — TTY-Aware Color Gating

All human-mode output paths for `install`, `update`, and `uninstall` commands MUST determine
whether to use color using the same detection logic as `suggest --install`.

The color gating algorithm MUST evaluate the following inputs in order:

1. If `--json` is set, output mode is JSON — no color, no symbols.
2. Otherwise, detect color eligibility:
   - `stdout.is_terminal()` MUST return `true`
   - `NO_COLOR` environment variable MUST NOT be set to a non-empty value
   - `CLICOLOR` environment variable MUST NOT be set to `"0"`
   - `TERM` environment variable MUST NOT be `"dumb"` (case-insensitive)
3. Color is enabled only if ALL four conditions in step 2 are satisfied.

When color is disabled, Unicode symbols (`✔`, `✗`) MUST still be printed. Only ANSI escape
sequences MUST be suppressed.

The detection function MUST be reusable across all skill commands — it SHALL NOT be duplicated
per command.

#### Scenario: SC-020a — NO_COLOR suppresses color

- GIVEN `NO_COLOR=1` is set in the environment
- AND stdout is a TTY
- WHEN `agentsync skill install my-skill` succeeds
- THEN stdout MUST contain `✔ installed my-skill`
- AND the output MUST NOT contain ANSI escape codes

#### Scenario: SC-020b — CLICOLOR=0 suppresses color

- GIVEN `CLICOLOR=0` is set in the environment
- AND stdout is a TTY
- WHEN `agentsync skill update my-skill` succeeds
- THEN stdout MUST contain `✔ updated my-skill`
- AND the output MUST NOT contain ANSI escape codes

#### Scenario: SC-020c — TERM=dumb suppresses color

- GIVEN `TERM=dumb` is set in the environment
- AND stdout is a TTY
- WHEN `agentsync skill uninstall my-skill` succeeds
- THEN stdout MUST contain `✔ uninstalled my-skill`
- AND the output MUST NOT contain ANSI escape codes

#### Scenario: SC-020d — Color enabled when all conditions met

- GIVEN `NO_COLOR` is not set
- AND `CLICOLOR` is not set
- AND `TERM=xterm-256color`
- AND stdout is a TTY
- WHEN `agentsync skill install my-skill` succeeds
- THEN stdout MUST contain ANSI green+bold escape codes in the status line

#### Scenario: SC-020e — Non-TTY always suppresses color

- GIVEN stdout is NOT a TTY (piped)
- AND `NO_COLOR` is not set, `CLICOLOR` is not set, `TERM=xterm-256color`
- WHEN `agentsync skill install my-skill` succeeds
- THEN stdout MUST contain `✔ installed my-skill`
- AND the output MUST NOT contain ANSI escape codes

---

### Requirement: REQ-021 — Shared Formatting Abstractions

The colored status-line formatting logic MUST be extracted into command-agnostic types reusable
across all skill subcommands.

The shared module MUST provide:

1. **`StatusLabelKind`** — an enum with variants: `Info`, `Warning`, `Success`, `Failure`.
2. **`SkillHumanFormatter`** — a struct parameterized by `use_color: bool` that provides:
   - `format_label(symbol, label, kind) -> String` — returns the symbol+label with ANSI color
     when `use_color` is true, or plain text when false.
   - `format_heading(heading) -> String` — returns the heading bold when `use_color` is true,
     or plain text when false.
3. **`OutputMode`** — an enum with variants: `Json`, `Human { use_color: bool }`. This is the
   simplified mode for single-operation commands (no `HumanLive` variant needed).
4. **`detect_output_mode(json, stdout_is_tty, no_color, clicolor, term) -> OutputMode`** — the
   reusable detection function.

The `suggest --install` flow MUST be refactored to use these shared types. The existing
`SuggestInstallHumanFormatter` and `SuggestInstallLabelKind` types MUST be removed and all
references MUST point to the shared types.

The `SuggestInstallOutputMode` MAY retain its `HumanLive` variant as a local extension that wraps
the shared `OutputMode`, since spinners are specific to the suggest-install flow.

No `SuggestInstall*`-prefixed types SHOULD remain for functionality that is shared across commands.

#### Scenario: SC-021a — Formatter produces colored output when use_color is true

- GIVEN a `SkillHumanFormatter` with `use_color = true`
- WHEN `format_label("✔", "installed", StatusLabelKind::Success)` is called
- THEN the result MUST contain ANSI green+bold escape codes wrapping `✔ installed`

#### Scenario: SC-021b — Formatter produces plain output when use_color is false

- GIVEN a `SkillHumanFormatter` with `use_color = false`
- WHEN `format_label("✔", "installed", StatusLabelKind::Success)` is called
- THEN the result MUST be exactly `✔ installed` with no ANSI escape codes

#### Scenario: SC-021c — Formatter applies correct color per kind

- GIVEN a `SkillHumanFormatter` with `use_color = true`
- WHEN `format_label` is called with each `StatusLabelKind` variant
- THEN `Info` MUST produce cyan+bold, `Warning` MUST produce yellow+bold, `Success` MUST produce
  green+bold, `Failure` MUST produce red+bold

#### Scenario: SC-021d — detect_output_mode returns Json when json flag is true

- GIVEN `json = true`
- WHEN `detect_output_mode(true, true, None, None, Some("xterm"))` is called
- THEN the result MUST be `OutputMode::Json`

#### Scenario: SC-021e — detect_output_mode returns Human with color for normal TTY

- GIVEN `json = false`, stdout is TTY, no env overrides
- WHEN `detect_output_mode(false, true, None, None, Some("xterm-256color"))` is called
- THEN the result MUST be `OutputMode::Human { use_color: true }`

#### Scenario: SC-021f — detect_output_mode returns Human without color for non-TTY

- GIVEN `json = false`, stdout is NOT a TTY
- WHEN `detect_output_mode(false, false, None, None, Some("xterm-256color"))` is called
- THEN the result MUST be `OutputMode::Human { use_color: false }`

#### Scenario: SC-021g — suggest --install still works identically after refactor

- GIVEN the suggest-install flow uses the shared `SkillHumanFormatter` and `StatusLabelKind`
- WHEN `agentsync skill suggest --install` is invoked
- THEN the visual output MUST be identical to the pre-refactor behavior
- AND all existing suggest-install tests MUST pass without modification

---

### Requirement: REQ-022 — Hint Line Formatting Consistency

Error output for `install`, `update`, and `uninstall` commands MUST use a consistent two-line
format for human-mode errors.

Line 1 MUST be a failure status line rendered via the shared formatter:
`{formatted ✗ failed} {skill_id}: {error_message}`

Line 2 MUST be a plain-text hint line: `Hint: {remediation_message}`

The hint line MUST NOT be colored. The remediation message MUST be produced by the existing
`remediation_for_error()` function (or command-specific remediation logic for uninstall).

The `tracing::error!()` call for structured logging MUST be preserved — it is not part of user
output but is emitted to stderr for diagnostic purposes.

#### Scenario: SC-022a — Error output has two lines

- GIVEN a human-mode error from any skill command
- WHEN the error is rendered
- THEN stdout MUST contain exactly two relevant lines:
  1. A line matching `✗ failed {skill_id}: {message}`
  2. A line matching `Hint: {remediation}`

#### Scenario: SC-022b — Hint line is never colored

- GIVEN stdout is a TTY with color enabled
- WHEN a skill command fails
- THEN the `Hint:` line MUST NOT contain ANSI escape codes

#### Scenario: SC-022c — Uninstall not-found uses specific remediation

- GIVEN skill `missing` is not installed
- WHEN `agentsync skill uninstall missing` fails in human mode
- THEN the hint line MUST contain text about using `list` to verify installed skills
