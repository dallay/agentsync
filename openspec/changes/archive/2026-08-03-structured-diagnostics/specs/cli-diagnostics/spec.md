# CLI Diagnostics Specification

## Purpose

Structured diagnostics: events to stderr (human/JSON), global log flags, spans with context,
enriched error context, no secrets, CI-parseable. Fixes logs corrupting machine-readable stdout.

## Requirements

### Requirement: Global Logging Flags

The CLI MUST accept global flags `--log-format <human|json>` (default `human`) and
`--log-level <trace|debug|info|warn|error>` on every subcommand. The functional per-command `--json`
flag MUST remain independent of `--log-format`.

#### Scenario: Default human format

- GIVEN no `--log-format` is provided
- WHEN any command runs
- THEN events SHALL render human-readable on stderr

### Requirement: JSON Event Shape

Events MUST go to stderr in both formats and every command mode. With `--log-format json`, each
event MUST be a single-line JSON object with at least `timestamp`, `level`, `target`, and `fields`
(incl. `message`), plus `span` inside a span, so stderr is line-parseable; JSON MUST NOT contain
ANSI escapes.

#### Scenario: Span fields appear in JSON events

- GIVEN an INFO event logged inside an apply span with `agent_id` and `target` fields
- WHEN rendered with `--log-format json`
- THEN the JSON object SHALL include `span` with those field values

### Requirement: Spans Around Core Operations

The CLI MUST instrument `apply`, `clean`, `mcp`, and `skill` with spans carrying `operation`,
record `outcome` (ok|created|updated|removed|skipped|error) on the span, and include `agent_id`,
`target`, `path`, `skill_id` on agent/target spans where applicable.

#### Scenario: Apply target span carries context

- GIVEN `agentsync apply` syncs agent `claude` target `agents`
- WHEN the sync runs with `--log-format json`
- THEN span events SHALL include `operation` and `agent_id`; target events SHALL include `target`
  and `path`

#### Scenario: Skill install span carries skill_id

- GIVEN `agentsync skill install some-skill` runs
- WHEN the operation runs
- THEN the span SHALL include `operation` and `skill_id`

#### Scenario: Failed target records error outcome

- GIVEN an apply target fails during processing
- WHEN the span closes
- THEN the span SHALL record `outcome = "error"`

### Requirement: Error Context in Critical Failures

Apply target failure logs MUST include the target's `agent_id` and `path`. MCP sync failure logs
MUST include the agent and its config path.

#### Scenario: Target failure logs agent and path

- GIVEN a target fails during apply
- WHEN the error event is emitted
- THEN it MUST include `agent_id` and `path`

#### Scenario: MCP sync failure logs agent and config path

- GIVEN MCP config generation fails for an agent
- WHEN the error event is emitted
- THEN it MUST include the agent and its config path

### Requirement: Secrets Are Never Logged

The CLI MUST NOT log MCP `env` or HTTP `headers` at any level. URLs MUST be logged redacted —
credentials, query strings, and tokens stripped before emission; `path`/`url` SHALL be sanitized.

#### Scenario: MCP env with bearer token is not logged

- GIVEN an MCP config has `env` with a bearer token
- WHEN MCP config generation runs at any log level
- THEN no log event SHALL contain the token value or the `env` block

#### Scenario: URL token query is redacted

- GIVEN a skill URL contains a `?token=...` query parameter
- WHEN the URL is logged
- THEN the event SHALL show the URL without query or credentials

### Requirement: CI-Friendly Logging

Logging MUST respect `RUST_LOG`; `--log-level` MUST override it. Diagnostics MUST be deterministic
(stable field names, one event per line). Human events SHOULD NOT emit ANSI when stderr is not a
TTY.

#### Scenario: RUST_LOG raises verbosity

- GIVEN `RUST_LOG=debug` and no `--log-level`
- WHEN a command runs
- THEN DEBUG events SHALL appear on stderr

#### Scenario: --log-level overrides RUST_LOG

- GIVEN `RUST_LOG=debug` and `--log-level warn`
- WHEN a command runs
- THEN only WARN and ERROR events SHALL appear

#### Scenario: Piped stderr has no ANSI in human mode

- GIVEN stderr is a pipe with `--log-format human`
- WHEN a command runs
- THEN stderr SHALL contain no ANSI escape sequences
