# Implementation Plan: Static Security Audit Hardening (#688)

## Delivery Summary

- Change: `issue-688-static-security-audit-hardening`
- Related issues: #688, #695, #702
- Expected scope: 5 dependency-ordered implementation slices delivered together in one PR;
  5,967 changed lines at final preflight (5,251 additions and 716 deletions), including tests and
  SDD/CLI documentation.
- Release condition: do not close #688 or claim the audit is a security guarantee until all slices
  and acceptance checks pass.

## Review Workload Forecast

- Forecast: High
- Decision needed before apply: No — user explicitly requested one PR containing the full scope.
- Chained PRs recommended: No — superseded by the user's latest delivery instruction.
- 400-line budget risk: High
- Proposed strategy: one PR using the explicitly approved `size-exception`; retain five functional
  implementation/verification slices internally, then deliver them together. The expected total
  exceeds the 400-line review budget and must be called out in the PR description.
- Delivery strategy approval: User changed the earlier stacked-PR decision and explicitly requested
  one PR with the complete scope; this is approval for the `size-exception` strategy.
- Trunk: `main`
- Parent branch: `main`
- Base: `main`
- Branch: `feat/issue-688-scan-contract`
- Position: 1/1 (single PR; five internal implementation slices)
- Issue references: #688, #695, #702
- Existing S1 code diff: 599 changed lines (514 additions, 85 deletions); included in the approved
  single-PR exception. Total scope now substantially exceeds the original 1,000–1,400-line estimate.
- Next action: complete broad verification, resolve compatibility/review findings, then open the
  single PR against `main` if all release gates pass.

## Phase 1: Secure Scan Contract and Traversal

### Slice S1 — Explicit bounded scan results

**User/operator result:** traversal reports what it inspected, what it skipped, and why, with hard
read-time limits; errors cannot masquerade as an empty successful scan.

**Review boundary:** shared diagnostics and bounded traversal only, including the skill-validation
consumer migration needed to preserve its existing behavior. This is an internal review slice, not
a separate PR; its 599-line diff is covered by the approved single-PR size exception.

- [x] 1.1 Document the internal traversal result contract—status, coverage, stable safe error codes,
  and relative paths—and preserve serialized `DiagnosticReport` v1 until the #695 compatibility
  decision is available.
- [x] 1.2 Add TDD coverage for traversal coverage counts, sorted enumeration, directory-entry/depth/
  file/per-file and aggregate read limits, read failures, and deterministic safe errors.
- [x] 1.3 Implement bounded traversal that enforces entry and byte limits during enumeration/reads,
  opens files and directories without following symlinks, reports skipped/untrusted links, and keeps
  paths normalized relative to the authorized root.
- [x] 1.4 Migrate affected skill-validation callers to the shared traversal/report API without
  changing their documented validation behavior; run their existing tests before and after the
  refactor.

## Phase 2: Safe Skill Audit and Rule Correctness

### Slice S2 — Fail-closed, secret-safe skill audit

**User/operator result:** auditing a selected skill cannot escape its authorized root or leak source
content; incomplete scans produce a distinct report and exit status.

**Review boundary:** skill target resolution, safe finding construction, report aggregation, and
CLI integration with real-binary tests. Untrusted plugin selection must fail closed until S4
provides the trust-aware resolver. This is an internal review slice in the approved single PR.

- [x] 2.1 Add contract tests for invalid/missing skill IDs, root and nested symlinks, entry/read
  limits, synthetic-secret redaction in stdout/stderr/JSON/logs, and exit status `2` for incomplete
  scans.
- [x] 2.2 Validate selectors and resolve skill roots within the authorized project boundary; ensure
  untrusted roots and links are never read and plugin paths are not resolved by direct concatenation.
- [x] 2.3 Integrate audit with shared scan results; remove source-line evidence and emit controlled
  findings, coverage, and scan errors using the shared report contract.
- [x] 2.4 Map complete/below-threshold, threshold findings, and incomplete/operational outcomes to
  exit codes `0`, `1`, and `2`; remove safety-certifying empty-result text and duplicate root CLI
  argument declarations.

### Slice S3 — Supported content coverage and deterministic rules

**User/operator result:** the enabled static rules examine the primary skill instructions and
supported structured content, resolve real file references, and emit reproducible findings.

**Review boundary:** parsers/rules and their unit/CLI tests; no general-purpose shell or malware
analysis. This is an internal review slice in the approved single PR.

- [x] 3.1 Add tests for credential and fetch-and-execute detection in `SKILL.md`, Markdown
  shell examples, scripts, JSON with quoted keys, YAML, and TOML, including negative cases and
  source-canary absence.
- [x] 3.2 Implement parser-backed rule dispatch for supported formats using existing dependencies;
  count binary/non-UTF-8/unsupported inputs as skipped and incomplete under the required-content
  policy.
- [x] 3.3 Add tests and implementation for path-reference resolution, traversal escapes, duplicate
  finding removal, canonical ordering, and stable SHA-256 fingerprints independent of secret values
  and absolute paths. Cross-selector target identity is validated in S4 after `--all` resolution.

## Phase 3: Trusted Plugin and MCP Audit

### Slice S4 — Offline, trust-aware plugin and MCP inspection

**User/operator result:** selected plugin content is audited only from an already-present snapshot
whose inventory, lock, provenance, and approval state is validated by AgentSync's existing trust
facilities.

**Review boundary:** narrow read-only plugin inspection facade, audit integration, and local fixture
tests; no restore/materialization/network operation. This is an internal review slice in the
approved single PR.

- [x] 4.1 Add tests for valid local snapshots, missing snapshots, invalid lock/provenance, plugin
  target/entry/depth/file/byte bounds, skill-local MCP declarations, `--all` inventory selection,
  namespaced identities, and approved/pending MCP declarations; assert no network, materialization,
  command execution, or filesystem mutation, including consistent fingerprints when a skill is
  selected directly or through `--all`.
- [x] 4.2 Expose a narrow read-only audit inspection API over existing plugin trust checks; return
  only authorized local roots and structured identity/approval state, with bounded discovery and
  descriptor-relative no-follow hashing.
- [x] 4.3 Connect resolved plugin and MCP declarations to bounded traversal and structured audit
  rules; report unavailable trust data as incomplete and pending approvals without launching MCP.

## Phase 4: CLI Acceptance, Documentation, and Release Evidence

### Slice S5 — Public contract and acceptance coverage

**User/operator result:** CLI help, human/JSON output, documentation, and binary-level tests agree
with the final versioned contract and explain its limits.

**Review boundary:** final contract fixtures, English and Spanish CLI reference, and end-to-end
regression checks. This is an internal review slice in the approved single PR.

- [x] 5.1 Add real-binary acceptance tests for mutually exclusive selectors, human/JSON output,
  schema version/shape, threshold behavior, incomplete outcomes, determinism, and separate stdout /
  stderr.
- [x] 5.2 Verify offline/read-only guarantees with local fixtures, sentinel content, and before/after
  filesystem state; verify all secret canaries are absent from every captured output channel.
- [x] 5.3 Document selection, supported content, traversal-entry/file/depth/byte limits, coverage,
  `--fail-on`, exit statuses, offline
  behavior, and the non-guarantee of empty results in English and Spanish CLI references.
- [x] 5.4 Run targeted and broad Rust validation, format and clippy checks; record actual commands,
  exit codes, and outcomes for SDD verification and review handoff.

## Verification Commands

- TDD fast feedback: `cargo test <targeted_test_name>` or `cargo test --test <integration_file>`.
- Broad tests: `cargo test --all-features`.
- Build: `cargo check --all-targets --all-features`.
- Formatting: `cargo fmt --all -- --check`.
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`.
- Docs, if changed: `pnpm run docs:build`.

## Delivery and Acceptance Gates

- The review forecast is High and the user explicitly approved one over-budget PR containing all
  five slices. Keep each slice independently testable and review its evidence before proceeding; do
  not create stacked PRs.
- Single-PR metadata: strategy `size-exception`, trunk `main`, parent `main`, base `main`, branch
  `feat/issue-688-scan-contract`, position 1/1, issue references #688, #695, and #702.
- Confirm the shared JSON contract/version with #695 and the redaction policy/API with #702 before
  treating those interfaces as frozen.
- TDD deviation (user-authorized to continue): the plugin fail-closed branch, empty-result human
  message, and final coverage aggregation were implemented before their specific tests were added;
  later tests cover the behavior, but no retroactive RED evidence is claimed. Subsequent behavior
  changes must use observed RED → GREEN evidence.
- Additional S3 TDD deviation: YAML parsing, initial Markdown-reference detection, and the repeated
  JSON comparison received characterization tests after their code was already present; those
  tests pass but do not constitute retroactive RED evidence.
- TDD evidence limitation: terminal records do not establish a RED→GREEN cycle for every changed
  behavior; no complete TDD-compliance claim is made. The user authorized continuing with this
  deviation documented.
- Release acceptance is independent of task completion: missing/nonpassing technical verification
  or acceptance QA keeps #688 open.
