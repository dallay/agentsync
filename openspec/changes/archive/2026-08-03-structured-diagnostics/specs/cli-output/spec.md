# Delta for CLI Output

## ADDED Requirements

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
