# Static Security Audit Specification

## Purpose

Define the observable security and CI contract for `agentsync audit`. The command performs
deterministic, local, read-only analysis and reports both findings and whether the requested scan
completed. An empty result is not a guarantee that content is safe.

## Requirements

### Requirement: Audit output never exposes source secrets

The audit MUST treat scanned content as untrusted. Human output, JSON, stderr, operational errors,
and logs MUST NOT contain source excerpts or literal secret values. Finding messages and evidence
MUST be controlled by the rule, and paths MUST be relative to an authorized target root.

#### Scenario: Credential canary is detected without disclosure

- GIVEN a supported file contains a synthetic credential canary that matches an enabled rule
- WHEN the file is audited in human and JSON modes
- THEN a finding MAY identify the rule and safe relative location, but the canary and source excerpt
  MUST be absent from stdout, stderr, serialized diagnostics, and captured logs

#### Scenario: Source line matches a command rule

- GIVEN a source line matches a fetch-and-execute or MCP privilege rule and also contains a canary
- WHEN the audit reports the finding
- THEN evidence MUST use rule-controlled text and MUST NOT include any part of the source line

### Requirement: Audit targets remain inside an authorized boundary

The command MUST validate skill and plugin selectors before path resolution. It MUST NOT read a
target outside the authorized skill or plugin roots. A root or nested symlink MUST NOT be followed
unless the existing AgentSync trust resolver explicitly authorizes that target. An escaping or
untrusted symlink MUST be reported as skipped/incomplete without reading its destination.
File-reference rules MUST resolve references against the authorized root rather than infer an
escape from text patterns alone.

#### Scenario: Invalid skill selector is rejected

- GIVEN `--skill` contains an absolute path, separator, traversal component, or invalid identifier
- WHEN the selector is resolved
- THEN no path outside the authorized skill root is accessed and the command reports an incomplete
  or operational result rather than an empty successful scan

#### Scenario: Symlink destination escapes the target

- GIVEN a selected root or a discovered file is a symlink to a location outside the authorized root
- WHEN the audit traverses the target
- THEN it does not read the destination, records a safe scan error, and marks the report incomplete

#### Scenario: Referenced path escapes the target

- GIVEN scanned content references a file path that resolves outside the authorized root
- WHEN the file-reference rule evaluates it
- THEN a safe finding is reported without opening the outside target

### Requirement: Scan bounds and completeness are explicit

Traversal MUST enforce directory-entry, file-count, depth, per-file byte, and aggregate byte limits
while reading or enumerating. The report MUST distinguish complete from incomplete scans and include
coverage counts for discovered, scanned, and skipped targets/files plus bytes scanned. Required read
failures, limit hits, missing requested targets, unsupported required content, and untrusted links
MUST be represented by stable, controlled scan errors; they MUST NOT become a clean empty result.
Scan errors MUST NOT include raw OS/parser errors or source content.

#### Scenario: A traversal limit is reached

- GIVEN the selected target exceeds a configured traversal or read limit
- WHEN the audit reaches that limit
- THEN it stops reading beyond the limit, records coverage and a stable limit error, and marks the
  report incomplete

#### Scenario: Directory entry enumeration exceeds its bound

- GIVEN a selected target or `--all` inventory contains more than the maximum number of directory
  entries
- WHEN the audit enumerates that directory
- THEN it stops collecting entries at the bound, records a `max-entries-exceeded` scan error, and
  marks the report incomplete

#### Scenario: A requested target is unavailable

- GIVEN a requested skill or trusted plugin snapshot does not exist or cannot be read
- WHEN the audit resolves the target
- THEN it reports an incomplete scan and MUST NOT print a clean-scan message

#### Scenario: Unsupported content is encountered

- GIVEN the traversal discovers content that the enabled rules cannot inspect
- WHEN the report is constructed
- THEN the content is counted as skipped and the scan is incomplete unless the shared policy
  explicitly classifies that content as excluded and non-required

### Requirement: Rules cover supported skill and configuration content

Credential and fetch-and-execute rules MUST inspect `SKILL.md`, supported text/script files, and
supported JSON, YAML, and TOML configuration. Structured formats MUST be parsed according to their
syntax, including quoted JSON keys. File-reference checks MUST use resolved path components and the
authorized-root policy. Unsupported or non-UTF-8 content MUST be accounted for as skipped, not
reported as scanned.

#### Scenario: Markdown skill instructions contain a credential or execution pattern

- GIVEN `SKILL.md` contains a credential-like value or a fetch-and-execute pattern in prose or a
  fenced shell example
- WHEN the skill is audited
- THEN the applicable rule reports a sanitized finding with a stable location

#### Scenario: JSON uses a quoted sensitive key

- GIVEN a valid JSON file contains a quoted credential key and a synthetic canary value
- WHEN credential rules inspect the parsed document
- THEN the rule detects the key/value without exposing the value in any output

#### Scenario: Multiple formats are analyzed

- GIVEN a selected skill contains supported JSON, YAML, TOML, Markdown, and script files
- WHEN the audit completes
- THEN each supported file is evaluated by applicable rules and coverage reflects the files actually
  scanned

### Requirement: Plugin and MCP audit reuse AgentSync trust state offline

Plugin targets MUST be resolved through existing inventory, lock, snapshot, provenance, and approval
facilities. The audit MUST inspect only already-present local content and MUST NOT download,
materialize, restore, execute plugin content, or launch MCP servers. Plugin trust preflight MUST cap
selected targets, directory entries, depth, file count, and bytes before reading manifests or hashing
content. MCP findings MUST be derived from structured server declarations and approval state, not
broad keyword matching across arbitrary TOML lines.

#### Scenario: A locked local plugin snapshot is audited

- GIVEN a plugin is selected and its locally available snapshot matches the trusted inventory and
  lock state
- WHEN the audit runs
- THEN only that authorized snapshot is inspected and its namespace identity is retained in findings

#### Scenario: Plugin snapshot or provenance is unavailable

- GIVEN the selected plugin has no local snapshot or its provenance/lock data is invalid
- WHEN the audit runs
- THEN no materialization or network access occurs and the report is incomplete

#### Scenario: MCP server is pending approval

- GIVEN an audited plugin declares an MCP server that is not approved
- WHEN structured MCP rules evaluate the declaration
- THEN the result reflects the pending approval state without launching the server

#### Scenario: Plugin preflight limit is exceeded

- GIVEN a selected local snapshot exceeds a plugin entry, depth, file-count, or byte limit
- WHEN the audit verifies the snapshot before rule analysis
- THEN it stops at the applicable bound, reports a controlled incomplete result, and does not launch
  processes or inspect content beyond the trust failure

### Requirement: Results are versioned, deterministic, and shared

Audit JSON MUST use the shared diagnostic report contract, expose scan status, coverage, findings,
summary, and scan errors, and carry an explicit schema version. Findings MUST be sorted by stable
target identity, normalized relative path, source location, rule identifier, and fingerprint;
duplicate findings MUST be removed deterministically. Fingerprints MUST use a stable digest over
non-sensitive rule/location identity and MUST NOT depend on secret values, raw source, absolute
paths, filesystem enumeration order, or whether the same target was selected directly or through
`--all`.

#### Scenario: Equivalent scans produce identical JSON findings

- GIVEN the same authorized content and rule set are audited more than once
- WHEN JSON results are compared
- THEN findings have the same order and fingerprints, independent of directory enumeration order and
  path separator conventions

#### Scenario: Shared report schema changes

- GIVEN the audit report is serialized for an automated consumer
- WHEN the report is emitted
- THEN its versioned schema identifies the shared contract and includes enough coverage/error
  information to distinguish incomplete analysis from zero findings

### Requirement: CLI outcomes distinguish findings from incomplete scans

`--all`, `--skill`, and `--plugin` selectors MUST be mutually exclusive and validated. `--fail-on`
MUST use the shared `Info`/`Warning`/`Error` severity contract. The command MUST return `0` for a
complete scan below threshold, `1` when findings meet the configured threshold, and `2` for an
incomplete scan or operational error; incompleteness MUST take precedence. Human and JSON output
MUST describe the same report. An empty human result MUST state that no findings were detected by
enabled rules and that this does not guarantee safety.

#### Scenario: Complete scan has no threshold finding

- GIVEN the scan completes with no findings at or above the configured threshold
- WHEN the audit command exits
- THEN it returns `0` and does not imply the audited content is safe

#### Scenario: Finding reaches the configured threshold

- GIVEN the scan completes with a finding at or above `--fail-on`
- WHEN the audit command exits
- THEN it returns `1`

#### Scenario: Scan is incomplete and also has findings

- GIVEN the scan has findings but traversal or target resolution is incomplete
- WHEN the audit command exits
- THEN it returns `2` regardless of the configured finding threshold

#### Scenario: Selector options conflict

- GIVEN more than one of `--all`, `--skill`, or `--plugin` is provided
- WHEN Clap parses the command
- THEN it rejects the invocation before scanning any target

### Requirement: Audit is local and read-only

The audit MUST NOT make network requests, execute scripts or shell commands, launch MCP servers,
resolve secrets, materialize plugins, or modify project files. All output for JSON mode MUST remain
machine-readable on stdout; logs and operational diagnostics MUST be emitted only to stderr.

#### Scenario: Audit runs with local fixtures

- GIVEN a project contains skills and local plugin fixtures
- WHEN the audit command runs
- THEN it performs no network or command execution and leaves fixture contents and metadata
  unchanged

#### Scenario: JSON output is captured

- GIVEN the command runs with JSON output enabled
- WHEN stdout and stderr are captured separately
- THEN stdout contains exactly one parseable report object and stderr contains no source content