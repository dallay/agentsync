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
