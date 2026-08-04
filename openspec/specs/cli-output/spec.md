# CLI Output Presentation Specification

## Purpose

Preserve AgentSync's established human and machine-readable output while moving reusable
presentation concerns out of command orchestration.

## Requirements

### Requirement: Human Output Is Contract-Preserving

The CLI MUST preserve existing human-output line order, labels, spacing, blank lines, summaries,
and ANSI color behavior when presentation helpers are extracted.

#### Scenario: Colored terminal output remains unchanged

- GIVEN an apply or clean operation runs with a color-capable TTY
- WHEN the operation emits human output
- THEN the output MUST retain the existing labels, ordering, spacing, and ANSI coloring

#### Scenario: Color is disabled consistently

- GIVEN JSON mode, a non-TTY stdout, `NO_COLOR`, `CLICOLOR=0`, or `TERM=dumb`
- WHEN human output is rendered
- THEN no ANSI escape sequences MUST be emitted

### Requirement: Output Contracts Have Exact Tests

The project MUST maintain exact output-contract tests for extracted apply, clean, and summary
renderers, including colored and uncolored paths. Existing status JSON serialization MUST remain
owned by its command module and unchanged.

#### Scenario: Renderer regression is detected

- GIVEN a renderer's expected lines, whitespace, or labels change
- WHEN the focused output tests run
- THEN the test suite MUST fail with the differing output

#### Scenario: Status JSON remains compatible

- GIVEN `agentsync status --json` produces a status array
- WHEN the output implementation is reorganized
- THEN its JSON shape and non-zero problem exit behavior MUST remain unchanged

### Requirement: Diagnostic Logs and Functional Output Are Stream-Separated

The CLI MUST write all tracing log events to stderr and MUST write only functional output to
stdout. Functional output is the command's human-readable result or its `--json` machine-readable
result.

The CLI MUST NOT write any tracing event to stdout in any mode. The stdout JSON contract MUST remain
fully parseable: a command run with its functional `--json` flag MUST produce stdout that parses as
JSON without discarding any content.

#### Scenario: WARN event does not corrupt --json stdout

- GIVEN `agentsync skill suggest --json` emits a WARN event during execution
- WHEN the command completes
- THEN stdout MUST parse as a single JSON value
- AND the WARN event MUST appear only on stderr

#### Scenario: Human output stays on stdout unchanged

- GIVEN `agentsync apply` runs with a color-capable TTY
- WHEN the operation emits human output
- THEN stdout MUST retain the existing labels, ordering, spacing, and ANSI coloring
- AND no tracing event MAY be interleaved into stdout

#### Scenario: Functional --json and log-format json stay separate

- GIVEN `agentsync status --log-format json --json` runs
- WHEN the command completes
- THEN stdout MUST contain only the functional status array
- AND stderr MUST contain only JSON log events
