# Design: Static Security Audit Hardening (#688)

## Technical Approach

Harden the existing Rust audit path in place, keeping rule detection, filesystem inspection, report
construction, and CLI presentation as separate stages. The audit command will resolve an explicit
set of authorized targets, inspect them with the shared bounded traversal, apply deterministic
rules, and return a shared diagnostic report that records both findings and scan completeness. The
CLI will render that report and map its outcome to the documented exit contract without terminating
the process from library code.

Coordinate the shared report and traversal changes with #695 and the secret-handling policy with
#702 before freezing serialized fields. `BoundedTraversal` and `DiagnosticReport` are already used
by `skills/validation.rs`, so shared changes must preserve or deliberately migrate that consumer in
the same slice. Reuse the current `PluginManager` lock/snapshot and MCP approval logic through a
read-only library facade; do not resolve plugin directories independently in the CLI.

Complex flow:

```text
agentsync audit
  │
  ├─ CLI validates selector and resolves project root
  ▼
AuditTargetResolver ──→ authorized local targets / target errors
  │
  ├─ plugin target ──→ PluginManager read-only locked-snapshot inspection
  ▼
BoundedTraversal ──→ coverage + safe scan errors
  │
  ▼
Static rules ──→ sanitized findings ──→ canonical DiagnosticReport
                                         │
                                         ├─ human or JSON stdout
                                         └─ exit: 0 complete / 1 threshold / 2 incomplete
```

The audit remains offline and read-only: it does not restore/materialize plugins, download
snapshots, resolve secrets, execute scripts, or launch MCP servers. Unsupported content is counted
as skipped; if that content is required by the selected target/policy, or another required input is
unavailable, the scan is incomplete rather than silently treated as clean.

## Architecture Decisions

### Decision: Keep one shared diagnostic and traversal contract

**Choice**: Add a shared typed traversal result with coverage and safe scan errors in
`src/diagnostics.rs`, and migrate affected traversal consumers to it. Preserve the existing
`DiagnosticReport` v1 serialization during the first traversal slice. Merge scan status and
coverage into the audit report using schema v2 while keeping legacy validation reports on v1 through
omitted optional fields. External consumer compatibility with #695 remains unconfirmed, so the v2
audit contract is not considered frozen.

**Alternatives considered**: Add audit-private traversal and report types; keep independent
`AuditReport` and `DiagnosticReport` JSON envelopes.

**Rationale**: The current shared traversal and diagnostic abstractions are already used by both
`src/audit.rs` and `src/skills/validation.rs`. A private implementation would duplicate boundary
and coverage behavior. Optional fields preserve the existing validation serialization, while audit
emits schema v2 to distinguish incomplete scans. External JSON consumers may still depend on the old
audit-only envelope, so compatibility must be confirmed before treating the v2 report as stable.

### Decision: Make traversal return explicit, bounded scan results

**Choice**: Evolve `BoundedTraversal` from a callback-only `Result<(), TraversalError>` API to a
result that includes discovered/scanned/skipped counts, bytes read, and structured scan errors.
Enforce a 10,000-entry cap during enumeration and file-count, depth, per-file, and aggregate-byte
limits during traversal. Use capability-bound directory handles and `FollowSymlinks::No` for file
opens, reject directory symlinks, sort bounded directory entries before processing, and keep paths
relative to the selected authorized root. Represent limit, unreadable, unsupported, symlink, and
root-resolution outcomes with stable error codes and controlled messages—not raw source text or
platform error strings.

**Alternatives considered**: Keep emitting one traversal error and infer coverage from successful
callbacks; add a separate audit-only walker.

**Rationale**: The existing implementation checks declared metadata size before an unbounded whole
file read, stops at the first error, and silently skips symlinks. Those behaviors cannot support
accurate coverage or fail-closed CI results. A single shared outcome lets validation and audit
report incomplete work consistently. The audit rejects selected-root and nested symlinks rather than
inferring trust from their resolved destinations; no symlink target is opened.

### Decision: Treat source content as tainted and never serialize it as evidence

**Choice**: Detection rules may inspect source bytes in memory, but every `Diagnostic.message`,
`evidence`, scan error, and log field will be generated from rule-controlled text, stable codes,
locations, and safe identifiers. Do not include source-line excerpts, literal values, raw parser
errors, absolute paths, or environment-resolved secrets. Reuse `src/secrets.rs` classification and
redaction policy where a value must be classified; prefer not to put the value in a diagnostic at
all.

**Alternatives considered**: Truncate source lines; apply `redact_evidence` after constructing a
diagnostic; add more secret regexes to the audit module.

**Rationale**: Truncation and pattern-based redaction cannot guarantee removal of every credential
form. The existing `secrets` module provides sensitive-key classification and redaction helpers,
but output safety is strongest when untrusted source values never enter the report. Synthetic
canaries will be asserted absent from stdout, stderr, JSON, errors, and captured logs.

### Decision: Parse supported formats and apply rules to skill content explicitly

**Choice**: Keep rules modular through `ValidationRule`, but dispatch by recognized file type and
content role. Parse JSON, YAML, and TOML with the dependencies already present; inspect structured
key/value pairs rather than relying on one line-oriented extractor. Apply credential and
fetch-and-execute checks to `SKILL.md` content and supported text/script inputs, including shell
examples in Markdown code blocks where applicable. Resolve extracted file references against the
authorized root using path components and symlink metadata; report escapes without opening an
outside target. Unsupported/binary or non-UTF-8 content is counted as skipped, not silently
reported as scanned.

**Alternatives considered**: Extend only the current extension allowlists and regex patterns; add a
general-purpose shell or malware analyzer.

**Rationale**: `SKILL.md` is the primary instruction artifact, and the current extension filters
exclude it from credential and fetch-and-execute analysis. Existing JSON/YAML/TOML parsers avoid
adding dependencies and correctly handle quoted keys. A limited static parser preserves the R1
scope while making supported coverage explicit; execution and external analyzers remain out of
scope.

### Decision: Reuse plugin trust state through a read-only library facade

**Choice**: Add or expose a narrowly scoped `PluginManager` read-only audit inspection API in
`src/plugins.rs`. It will resolve selected plugins through the existing config, lockfile, snapshot,
and provenance checks; return only already-present local roots plus namespaced identity and MCP
approval state; and expose no materialization/fetch operation. The audit command can then scan
those roots and call structured MCP rules on the discovered server declarations. Missing or
invalid locked data becomes a scan error/incomplete result; pending approval is surfaced as a
finding/state, not treated as an approved server.

**Alternatives considered**: Concatenate `--plugin` to `.agents/plugins`; duplicate lockfile and
provenance resolution in `commands/audit.rs`; make the whole plugin manager public to the CLI.

**Rationale**: `PluginManager::inspect_status_report_read_only` already verifies selected lock
entries and local snapshots without materialization, but is `pub(crate)` and therefore unavailable
to the binary crate's command adapter. A small public inspection facade preserves trust semantics
without exposing implementation internals or adding a second resolver. The current report provides
`Allowed`/`Pending` MCP status that can feed the existing structured `audit_mcp_server` rule.

### Decision: Normalize reports and fingerprints before presentation

**Choice**: Use the shared report's ordered findings, summary, scan status, coverage, and scan
errors as the sole source for both output formats. Sort findings by target, normalized `/` relative
path, span, rule ID, and fingerprint; deduplicate by fingerprint. Generate fingerprints with the
already available SHA-256 dependency over stable rule ID, target identity, normalized relative
path, and a rule-controlled location/context discriminator; never hash secret values or raw source
lines.

**Alternatives considered**: Rely on filesystem enumeration order; retain `DefaultHasher` in both
modules; fingerprint full source content.

**Rationale**: `read_dir()` ordering is unspecified and `DefaultHasher` is not a cross-version
stable contract. Sorting and a fixed digest make results reproducible across repeated scans and
platform path separators without embedding sensitive content. Target identity must be independent
of whether an item is reached through `--all` or a specific selector.

### Decision: Make scan outcome distinct from finding threshold

**Choice**: Introduce an audit outcome that distinguishes complete scans from scans with findings
and incomplete/operational scans. The CLI adapter maps complete scans below threshold to `0`,
findings at or above `--fail-on` to `1`, and incomplete scans/operational errors to `2`, with
incompleteness taking precedence. Keep `--fail-on warning|error` aligned with the shared
`Info`/`Warning`/`Error` model unless #695 settles a different common threshold contract. Remove
wording that implies an empty result certifies safety.

**Alternatives considered**: Keep returning an empty report when a target is missing; use a warning
on stderr while returning success; call `std::process::exit` inside `run_audit`.

**Rationale**: The existing command treats missing targets as clean, suppresses traversal errors,
and calls `process::exit(1)` directly for threshold failures. A typed outcome keeps core logic
testable and lets `main.rs` map the CLI contract consistently. The empty human result should state
that no findings were detected by enabled rules and that this is not a safety guarantee.

## Data Flow

1. `main.rs` parses one audit selector and resolves the project root. Clap makes `--all`, `--skill`,
   and `--plugin` mutually exclusive; the command-specific `project_root` field is removed if the
   top-level `Commands::Audit` root remains authoritative.
2. `AuditTargetResolver` validates identifiers before joining paths. Skills resolve below the
   canonical skills root. Plugin selections resolve through `PluginManager` and the lock/snapshot
   inspection facade, not by accepting an arbitrary path.
3. The resolver returns target identities and authorized roots. Each root is checked without
   following a symlink by default; nested entries are checked against the same containment policy.
   Escaping/untrusted links are reported without opening their targets.
4. `BoundedTraversal` processes sorted entries and reads at most the configured per-file/aggregate
   limits. It updates coverage for every discovered, scanned, skipped, or failed item.
5. Rules consume bounded bytes. Format parsers produce structured values/references; each finding
   is constructed from controlled rule text, safe relative path/span, and a deterministic
   fingerprint. No source excerpt is retained.
6. The shared report aggregates and sorts findings, counts severity and coverage, and sets
   `complete` only when all required targets/content were examined under the declared policy.
7. The CLI serializes JSON only to stdout in JSON mode; human mode renders the same report. It
   returns the outcome to `main.rs`, which maps it to the process exit code. Audit-specific
   operational errors must also map to `2` before reaching the generic non-audit error handler.
   Logging remains on stderr and must not contain source content or unsanitized paths.

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/diagnostics.rs` | Modify | Add shared scan status, coverage, safe scan-error types and report fields; evolve bounded traversal to enforce read-time limits, deterministic order, symlink checks, and explicit outcomes; coordinate the contract with #695. |
| `src/audit.rs` | Modify | Add target-aware scan orchestration, parser-backed rule applicability, path-reference checks, safe diagnostics, structured MCP auditing, stable fingerprints, sorting, and deduplication; remove duplicate report behavior when shared contract is ready. |
| `src/commands/audit.rs` | Modify | Validate selectors, use the resolver/facade, render completed/incomplete reports, implement threshold behavior, and return a typed exit outcome instead of exiting internally. |
| `src/main.rs` | Modify | Keep one authoritative project-root argument and map audit outcomes to exit codes `0`, `1`, and `2` while preserving other command behavior. |
| `src/plugins.rs` | Modify | Expose a narrow read-only audit inspection API over existing locked snapshots and MCP approvals, without restore/materialization or network access. |
| `src/secrets.rs` | Reuse | Use the established sensitive-key, value-classification, and environment-reference helpers from both audit and plugin validation. |
| `src/skills/validation.rs` | Modify if needed | Migrate to the shared traversal/report API and preserve explicit validation behavior when traversal results become structured. |
| `src/policy.rs` | Modify | Initialize the optional diagnostic target identity as absent for non-audit policy findings. |
| `src/explorer.rs` | Modify | Initialize the optional diagnostic target identity as absent in the existing diagnostic conversion fixture. |
| `tests/contracts/test_audit_output.rs` | Create | Execute the real binary with temporary projects and assert CLI, JSON, redaction, coverage, exit codes, offline behavior, and deterministic output. |
| `tests/all_tests.rs` | Modify | Register the audit contract test module if it follows the existing `contracts/` harness pattern. |
| `src/audit.rs`, `src/diagnostics.rs`, `src/plugins.rs` tests | Modify | Cover rule/parser behavior, read-time bounds, traversal edge cases, fingerprint stability, plugin approvals, and absence of source content in diagnostics. |
| `website/docs/src/content/docs/reference/cli.mdx` | Modify | Document audit selection, formats, supported coverage/limits, threshold semantics, exit codes, and non-guarantee of empty results. |
| `website/docs/src/content/docs/es/reference/cli.mdx` | Modify | Keep the Spanish CLI reference in sync with the audit contract. |
| `.agents/sdd/changes/issue-688-static-security-audit-hardening/apply-progress.md` | Create | Record completed implementation slices, actual validation commands, TDD deviations, and remaining release gates. |

No new dependency is expected: JSON/YAML/TOML parsers and SHA-256 are already present in
`Cargo.toml`.

## Interfaces / Contracts

The first traversal slice adds an internal result without changing the existing serialized
`DiagnosticReport` v1. The walker reports file-level coverage and stable, source-free errors; the
audit adapter will attach target identity when this result is merged into the public report after
the #695 compatibility review:

```rust
pub struct TraversalReport {
    pub status: ScanStatus,
    pub coverage: TraversalCoverage,
    pub errors: Vec<TraversalError>,
}

pub struct TraversalCoverage {
    pub files_discovered: usize,
    pub files_scanned: usize,
    pub files_skipped: usize,
    pub bytes_scanned: u64,
}

pub struct ScanError {
    pub code: ScanErrorCode,
    pub source_path: Option<String>, // normalized path relative to the selected root
}
```

The final serialized report target shape, subject to the shared #695 contract and schema
compatibility review, is:

```rust
pub enum ScanStatus {
    Complete,
    Incomplete,
}

pub struct ScanCoverage {
    pub targets_discovered: usize,
    pub targets_scanned: usize,
    pub files_discovered: usize,
    pub files_scanned: usize,
    pub files_skipped: usize,
    pub bytes_scanned: u64,
}

pub struct ScanError {
    pub code: ScanErrorCode,
    pub target: String,
    pub source_path: Option<String>, // normalized path relative to authorized root
    pub message: String,             // controlled text; no OS error or source excerpt
}

pub struct DiagnosticReport {
    pub schema_version: u32,
    pub status: ScanStatus,
    pub findings: Vec<Diagnostic>,
    pub summary: SeveritySummary,
    pub coverage: ScanCoverage,
    pub scan_errors: Vec<ScanError>,
}

pub enum AuditOutcome {
    Complete { report: DiagnosticReport, exit_code: u8 },
    Incomplete { report: DiagnosticReport }, // always maps to exit code 2
}
```

Contract invariants:

- `status == Complete` only when every required target was scanned or explicitly declared
  unsupported according to the agreed completeness policy; required read/resolve failures set
  `Incomplete`.
- A missing requested target, invalid target resolution, traversal limit hit, unavailable required
  plugin snapshot, or containment violation cannot be serialized as a clean empty scan.
- JSON stdout contains exactly one report object. Human output and stderr diagnostics use the same
  safe rule messages and relative paths; neither contains original source values.
- A finding's fingerprint is prefixed `sha256:` and depends only on stable target/rule/location
  identity, not secret content or the invocation route (`--all` versus a single target).
- Finding severity is evaluated using shared `Severity`; scan incompleteness always maps to exit `2`
  regardless of `--fail-on`.
- Audit-specific operational errors that prevent report construction also map to exit `2`; generic
  error-to-exit behavior for other commands remains unchanged.

The existing JSON envelope uses `diagnostics`, `total`, and `counts`, while shared
`DiagnosticReport` uses `findings` and `summary`. A consumer search inside this repository found no
other `AuditReport` consumers, but external CLI consumers are unknown. Treat the v1-to-shared-shape
decision as a release-contract gate with #695; bump the schema version for an incompatible default
JSON change and retain an adapter only if compatibility requirements justify it.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|--------------|----------|
| Unit | Structured credential detection, quoted JSON keys, Markdown/script coverage, fetch-and-execute detections, path-reference resolution, safe messages/evidence, deduplication, ordering, SHA-256 stability | Add focused tests in `src/audit.rs`; use synthetic canaries and assert exact absence from every diagnostic field. |
| Unit | Bounded traversal counts, read-time byte/file/depth limits, root/nested symlinks, unreadable entries, stable errors and relative paths | Extend `src/diagnostics.rs` tests with `tempfile`; verify an external symlink target is not opened and incomplete coverage is reported. |
| Unit / Integration | Plugin inventory, locked snapshot, provenance mismatch, missing snapshot and allowed/pending MCP state | Reuse existing plugin fixtures and read-only inspection tests; prove no fetch/materialization path is called. |
| CLI contract | Selector conflicts and invalid IDs; missing targets; human and JSON report; `--fail-on warning|error`; exit codes `0/1/2`; JSON schema and stable ordering | Add `tests/contracts/test_audit_output.rs` using `CARGO_BIN_EXE_agentsync`, `TempDir`, `Command`, and `serde_json`, then register it in `tests/all_tests.rs`. |
| Security regression | Synthetic secret absent from stdout, stderr, JSON, errors and logs; outside-root symlink target contents absent; no filesystem mutation or command execution | Capture both output streams, snapshot fixture metadata before/after, use a unique canary and harmless sentinel command, and test root plus nested symlinks on supported platforms. |
| Regression | Shared skill validation behavior after changes to `DiagnosticReport` or `BoundedTraversal` | Run existing skill validation and plugin tests, then `cargo test --all-features`; run formatting and clippy for the final implementation slices. |

Network absence should be guaranteed structurally by the read-only API and verified with plugin
fixtures that omit cached snapshots: audit must report the missing data and must not invoke a fetcher
or alter the project. Symlink tests may need platform-specific setup where creating symlinks is
restricted.

## Migration / Rollout

No data migration or network service change is required. Roll out in independently reviewable slices
as proposed: (1) shared report/redaction/traversal contract with #695/#702, (2) rule correctness and
format coverage, (3) trust-aware plugin/MCP target resolution, and (4) CLI exit/output contract,
docs, and real-binary acceptance tests. Do not make the hardened command a release gate until the
JSON compatibility decision and symlink policy are settled and the acceptance matrix passes.

If the output envelope changes incompatibly, version it explicitly and either retain a tested v1
adapter for a defined compatibility window or document the breaking change before release. Never
roll back source redaction or fail-closed handling of incomplete scans to preserve v1 behavior.

## Open Questions

- [ ] With #695, confirm whether external consumers can accept the schema-v2 audit report or require a compatibility adapter. The implementation uses the shared `DiagnosticReport` type, but external consumer compatibility remains unverified.
- [ ] With #702, confirm that `src/secrets.rs` is the canonical classifier/redaction contract and whether any existing validation output contract also needs hardening. Audit and plugin validation now delegate to the shared helpers; cross-issue approval remains unconfirmed.
- [x] Symlink policy: reject selected-root and nested symlinks without following them; no managed-link exception is enabled for audit. An external target is never opened.
- [x] Plugin selection: `--plugin` requires one explicitly selected namespaced locked plugin; `--all` inspects every selected, locally available, lock/provenance-verified snapshot.
- [x] Unsupported and non-UTF-8 content is counted as skipped and makes the scan incomplete with exit code `2`.
- [x] Audit-specific operational errors use exit code `2`; non-audit command error handling remains unchanged.