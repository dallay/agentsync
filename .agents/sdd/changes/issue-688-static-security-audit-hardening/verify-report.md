## Verification Report

**Change**: `issue-688-static-security-audit-hardening`
**Spec version**: N/A
**Audit report schema**: 2
**Verification run**: 2026-11-24 original verification; 2026-11-25 review-reconciliation refresh
**Latest full-suite log**: `/Users/acosta/.junie/sessions/session-261124-131238-1x3l/task-261125-103704-dw7n/terminal-output/terminal-output-7988393195676727869.txt`
**Original full-suite log**: `/Users/acosta/.junie/sessions/session-261124-131238-1x3l/task-261124-160019-1ks7/terminal-output/terminal-output-5235162038257154938.txt`
**Log note**: The 2026-11-25 suite used an isolated target directory to rebuild tests against the reconciled checkout; this was verification evidence, not a new SDD or independent QA run.

**Status**: partial
**Verdict**: FAIL

### Executive Summary

All 18 implementation tasks are checked. The delta specification has 24 Given/When/Then scenarios:
21 have passing runtime evidence and three remain partial because direct network-attempt, arbitrary
MCP-process, and filesystem-metadata observation is unavailable. The original fresh `cargo test --all-features`
run exited 0 with 1,516 passed, 0 failed, and 6 ignored across 30 test-result summaries, including
35 `audit_output_contract` tests. A subsequent 2026-11-25 `CARGO_TARGET_DIR=target/reconcile-pr-758 cargo test --all-features`
run from the reconciled checkout exited 0 with all suites passing. An intermediate shared-target
run reused a test binary with the removed temporary worktree embedded in `CARGO_MANIFEST_DIR`; it
was not used as code-failure evidence. Fresh native all-target check, formatting, strict Clippy, and
`git diff --check` exited 0. The supplied docs result is exit 0 with 45 Astro pages; its pnpm
`prepare` hook warned that the globally installed wrapper was unavailable. The run used Node
`v26.13.1`, matching `.nvmrc` but differing from the project guidelines' `24.14.0` target. Current
source and passing tests confirm read-only status rejects symlinked local-source ancestors and
caps inventory before discovery; plugin hashing is capability-relative, no-follow, and bounded by
entry/depth/file/byte limits; skill discovery rejects symlinks; skill-local MCP privilege findings
and shared secret classification are present. Bundle integrity consumes only bounded-traversal
`SKILL.md` manifests. The Windows target remains blocked in native dependencies before AgentSync
compilation. At this report's 2026-11-24 publication, the independent QA report was `PASS WITH
WARNINGS` and cited prior 1,515-pass evidence; QA was not rerun or changed by this verification.
The subsequent `qa-report.md` refresh on 2026-11-25 uses 1,516-pass evidence from the verification
run without rerunning tests. Verification remains **FAIL** because persisted records do not
establish complete RED→GREEN→REFACTOR evidence or
validation-migration baseline/post-refactor evidence. No TDD-compliance or full-compliance claim is
made; external JSON compatibility also remains an open release risk.

### Completeness

| Metric | Value |
|--------|-------|
| Implementation tasks total | 18 |
| Implementation tasks complete | 18 |
| Implementation tasks incomplete | 0 |

`tasks.md` contains 18 checked implementation tasks and no unchecked implementation tasks.
At this report's 2026-11-24 publication, `apply-progress.md` recorded all five slices complete,
1,516 passing tests, and draft PR creation pending; `qa-report.md` cited its prior 1,515-pass
publication. These are historical handoff details. The 2026-11-25 QA refresh used the recorded
1,516-pass evidence without rerunning tests; its separate verdict remains `PASS WITH WARNINGS`.
GitHub currently reports PR #758 open, no longer draft, with `CHANGES_REQUESTED` at head `ec2324d`.
The local branch now matches that head, with 12 review-change files remaining uncommitted and
unpublished. The current PR snapshot has 11 open review threads, four with outdated anchors; no
GitHub replies or resolutions were made. The PR body still states the Windows cross-target gap.
`state.yaml` records `current_phase: qa`, both `verify` and `qa` completed, and `next: verify`.
This refresh preserves this report's technical verdict as `FAIL`.

### Build, Tests & Artifact Checks

**Build**: Fresh native all-target check passed; the latest Windows cross-target result is blocked
before AgentSync compilation by unavailable Windows native prerequisites.

- `cargo check --all-targets --all-features` — freshly rerun, exit 0. Cargo reports the existing
  redundant `package.homepage` warning.
- `cargo clippy --all-targets --all-features -- -D warnings` — freshly rerun, exit 0. Cargo reports
  the existing redundant `package.homepage` warning. The test run also reports the unused `renamore`
  dependency warning.
- `cargo check --target x86_64-pc-windows-msvc --all-targets --all-features` — latest recorded attempt,
  exit 101 before compiling AgentSync. `libz-sys` cannot find `sys/types.h`; `aws-lc-sys` cannot find
  `windows.h` and reports unavailable Vcpkg/MSVC prerequisites. This is an environment block, not a
  Rust crate compile result; Windows runtime behavior remains unverified.
- `pnpm run docs:build` — latest supplied run exited 0; Astro built 45 pages. The pnpm
  `prepare`-hook wrapper lookup produced a warning because the globally installed
  `@dallay/agentsync` wrapper was unavailable; the Astro build still completed. The build used Node
  `v26.13.1` and pnpm `12.21.0`. The current `.nvmrc` also reads `26.13.1`; this conflicts with the
  project guidelines' target `24.14.0`, so the runtime-source discrepancy remains unresolved.

**Tests**: `cargo test --all-features` — freshly rerun, exit 0; 30 test-result summaries total 1,516
passed, 0 failed, and 6 ignored, including 35 `audit_output_contract` tests (parsed from the direct
rerun log linked above). The passing suite includes
`mcp_server_audit_detects_secrets_in_separate_arguments`,
`plugin_hash_reader_rejects_symlink_files`, `plugin_hash_tree_caps_directory_entries`,
`plugin_hash_tree_caps_directory_depth`, `skill_discovery_rejects_a_symlinked_skills_directory`,
`plugin_source_hash_rejects_excessive_file_count`,
`plugin_source_hash_rejects_excessive_content_size`,
`audit_snapshot_inventory_rejects_too_many_selected_plugins_before_lock_access`,
`skill_local_mcp_privilege_is_reported_from_structured_declaration`,
`status_report_rejects_symlinked_local_source_ancestors`,
`status_report_rejects_symlinked_skills_before_discovery`, and
`bundle_integrity_does_not_read_skills_through_symlinks`. The current source review confirms
read-only status validates every existing local-source ancestor and hashes the bounded plugin tree
before plugin discovery; `discover_skills` traverses capability-relative directories without
following symlinks. `BundleIntegrityRule` consumes only the `SKILL.md` files returned by bounded
traversal. The selected audit inventory is capped before lock access. Cargo reported the redundant
`package.homepage` and unused `renamore` dependency warnings. At the time of this 2026-11-24
verification, QA had not yet been refreshed and cited 1,515 passes. The later 2026-11-25 QA refresh
uses the recorded 1,516-pass evidence and did not rerun tests; its separate verdict remains
`PASS WITH WARNINGS`.

A later fresh run from the reconciled checkout used
`CARGO_TARGET_DIR=target/reconcile-pr-758 cargo test --all-features` and exited 0 with all suites
passing. Its full output is linked in the report metadata above; this refresh does not recalculate
or replace the earlier 1,516-pass aggregate.

**Artifact checks**:

| Check | Result |
|-------|--------|
| `cargo fmt --all -- --check` | Pass in this refresh, exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | Pass in this refresh, exit 0; existing manifest warning |
| `cargo check --all-targets --all-features` | Pass in this refresh, exit 0; existing manifest warning |
| `git diff --check` | Pass after report refresh, exit 0 |
| `pnpm run docs:build` | Latest supplied run exited 0; 45 pages generated; prepare-hook wrapper warning |
| Windows MSVC cross-target check | Latest recorded attempt blocked before crate compilation by missing Windows headers/Vcpkg/MSVC prerequisites |

**Coverage**: Not configured in `.agents/sdd/config.yaml`; no coverage threshold was run.

**Deterministic runner**: `fallback` — `.agents/sdd/quality-runner.json` is absent. The checks above
are executed command evidence, not deterministic-runner enforcement. No project
`.agents/skill-registry.md` exists; `skill_resolution: none`.

### Spec Compliance Matrix

| Requirement | Scenario | Evidence / validation | Result |
|-------------|----------|-----------------------|--------|
| Source secret safety | Credential canary is detected without disclosure | `quoted_json_secret_keys_are_parsed_and_redacted`, `audit_output_does_not_expose_source_lines`; human/JSON stdout and stderr are captured. `mcp_server_audit_detects_secrets_in_separate_arguments` verifies split-argument detection and safe diagnostic text/evidence at unit level. | ✅ COMPLIANT |
| Source secret safety | Source line matches a command rule and contains a canary | `audit_output_does_not_expose_source_lines` runs the real binary with a canary in a fetch-and-execute line and checks human/JSON output. | ✅ COMPLIANT |
| Authorized target boundary | Invalid skill selector is rejected | `invalid_skill_selector_does_not_scan_outside_the_skill_root`; incomplete status and canary absence asserted. | ✅ COMPLIANT |
| Authorized target boundary | Root or nested symlink destination is not followed | `symlink_skill_root_is_incomplete_and_not_followed`, `nested_symlink_is_skipped_and_incomplete_takes_precedence_over_threshold`; Unix-gated tests pass on this host. | ✅ COMPLIANT |
| Authorized target boundary | Referenced path escape is reported without opening the target | `markdown_reference_that_escapes_the_target_is_reported_without_reading_it`, `structured_file_reference_escape_is_reported_without_reading_the_target`. | ✅ COMPLIANT |
| Scan bounds and completeness | Traversal limit stops additional reading and reports incomplete coverage | `file_limit_produces_incomplete_coverage_without_reading_the_file` plus `src/diagnostics.rs` tests for depth, file, per-file/aggregate byte limits and read errors. | ✅ COMPLIANT |
| Scan bounds and completeness | Directory entry enumeration over the bound is incomplete | `traversal_report_caps_directory_entry_enumeration`, `all_inventory_reports_incomplete_when_skill_entry_limit_is_exceeded`; both cover 10,001 entries and `max-entries-exceeded`. | ✅ COMPLIANT |
| Scan bounds and completeness | Requested target unavailable is incomplete, not clean | `missing_skill_is_reported_as_an_incomplete_scan`, `unavailable_plugin_source_or_invalid_provenance_is_incomplete`. | ✅ COMPLIANT |
| Scan bounds and completeness | Unsupported/non-UTF-8 content is counted skipped and incomplete | `unsupported_non_utf8_files_are_skipped_and_mark_the_scan_incomplete`. | ✅ COMPLIANT |
| Supported formats and rules | Markdown skill prose/fenced shell pattern is detected safely | `skill_markdown_is_checked_for_credentials_and_fetch_patterns`. | ✅ COMPLIANT |
| Supported formats and rules | Quoted JSON credential key is parsed and redacted | `quoted_json_secret_keys_are_parsed_and_redacted`. | ✅ COMPLIANT |
| Supported formats and rules | Supported formats are inspected and coverage reflects scanned content | `skill_markdown_is_checked_for_credentials_and_fetch_patterns`, `quoted_json_secret_keys_are_parsed_and_redacted`, `yaml_sensitive_keys_are_parsed_and_redacted`, `env_files_are_checked_for_credentials_and_redacted`, `structured_configuration_is_checked_for_fetch_and_execute_patterns`, and `fetch_patterns_are_checked_in_all_declared_script_and_text_formats`. | ✅ COMPLIANT |
| Plugin/MCP trust and offline behavior | Locked local snapshot is audited with namespaced identity | `locked_plugin_snapshot_is_audited_read_only_with_namespaced_mcp_state` asserts identity/local snapshot behavior; `skill_discovery_rejects_a_symlinked_skills_directory`, `status_report_rejects_symlinked_skills_before_discovery`, and `status_report_rejects_symlinked_local_source_ancestors` cover the latest no-follow guards. | ✅ COMPLIANT |
| Plugin/MCP trust and offline behavior | Missing snapshot or invalid provenance is incomplete without materialization or network access | `unavailable_plugin_source_or_invalid_provenance_is_incomplete`, `invalid_plugin_provenance_is_incomplete_and_safe`; no direct network-attempt observer is available, so the no-network assertion remains partial. | ⚠️ PARTIAL |
| Plugin/MCP trust and offline behavior | Pending MCP approval is represented without launching the server | `locked_plugin_snapshot_is_audited_read_only_with_namespaced_mcp_state` and `approved_plugin_mcp_declaration_is_not_reported_as_pending`; no process observer establishes absence of arbitrary MCP launches. | ⚠️ PARTIAL |
| Plugin/MCP trust and offline behavior | Plugin preflight stops at entry/depth/file/byte/selected-target caps | `plugin_hash_tree_caps_directory_entries`, `plugin_hash_tree_caps_directory_depth`, `plugin_source_hash_rejects_excessive_file_count`, `plugin_source_hash_rejects_excessive_content_size`, and `audit_snapshot_inventory_rejects_too_many_selected_plugins_before_lock_access`; errors are returned before plugin discovery. | ✅ COMPLIANT |
| Shared deterministic report | Equivalent scans have stable JSON/findings/fingerprints | `repeated_audits_emit_identical_json`, `credential_fingerprints_are_sha256_and_ignore_the_secret_value`, `all_audit_uses_the_same_skill_identity_as_direct_selection`. | ✅ COMPLIANT |
| Shared deterministic report | Versioned schema contains status, coverage and scan errors | `complete_json_report_includes_versioned_coverage`; schema version 2 and required fields asserted. | ✅ COMPLIANT |
| CLI outcome contract | Complete scan below threshold returns 0 and does not certify safety | `empty_human_report_does_not_certify_safety`; clean and default-threshold paths pass. | ✅ COMPLIANT |
| CLI outcome contract | Finding at configured threshold returns 1 | `findings_meeting_the_threshold_return_one`. | ✅ COMPLIANT |
| CLI outcome contract | Incomplete scan with findings returns 2 regardless of threshold | `nested_symlink_is_skipped_and_incomplete_takes_precedence_over_threshold`. | ✅ COMPLIANT |
| CLI outcome contract | Conflicting selectors are rejected before scanning | `audit_target_selectors_are_mutually_exclusive`; stdout is empty and Clap reports the conflict. | ✅ COMPLIANT |
| Local/read-only operation | Local fixtures cause no network/command execution or mutation, including metadata | Locked-snapshot test compares paths and bytes and checks a hook sentinel; there is no direct network/process observer or filesystem metadata snapshot. | ⚠️ PARTIAL |
| Output channels | JSON stdout is one parseable report and stderr has no source content | JSON contract tests deserialize stdout; `audit_output_does_not_expose_source_lines` checks stdout/stderr. | ✅ COMPLIANT |

**Compliance summary**: 21/24 declared Given/When/Then scenarios have passing runtime evidence; 3/24
are partial because network attempts, arbitrary MCP process launches, and filesystem metadata
immutability were not directly observed. Split-argument MCP detection passes at unit level; there is
no separate real-binary fixture for that exact argument form. Passing runtime tests do not establish
TDD ordering; see the separate TDD audit.

### Correctness (Static — Structural Evidence)

| Requirement | Status | Notes |
|------------|--------|-------|
| Diagnostics never serialize source excerpts or literal secrets | ✅ Implemented | Rule-controlled messages/evidence and safe relative paths in `src/audit.rs`; real-binary canary coverage passes. The split `--key value` parser is present and its unit test passes. |
| Selectors and traversal stay within authorized roots | ✅ Implemented | Selector validation in `src/commands/audit.rs`; symlink metadata, capability-relative no-follow traversal, and `FollowSymlinks::No` file opens in `src/diagnostics.rs`; file references resolve within the authorized root. Plugin hash reads also use capability-relative directory handles and no-follow file opens in `src/plugins.rs`. |
| Limits, coverage, scan errors and incomplete outcomes | ✅ Implemented | `BoundedTraversal` uses `DEFAULT_MAX_TRAVERSAL_ENTRIES = 10,000`; `--all` inventory and plugin preflight enforce entry caps. Plugin preflight bounds selected targets at 1,000 and source trees at 10,000 entries, 10,000 files, depth 64, and 500 MiB aggregate bytes. Read-only status validates local-source ancestors and hashes the bounded plugin tree before discovery (`status_report_rejects_symlinked_local_source_ancestors`). The `MaxEntriesExceeded` to `traversal-entries` validation mapping is present, but no dedicated validation-diagnostic assertion was found. |
| Structured content and deterministic findings | ✅ Implemented | JSON/YAML/TOML parsing, SHA-256 fingerprints, ordering and deduplication are present; declared text/script extension coverage has passing contract tests. Bundle integrity consumes only `SKILL.md` entries returned by bounded traversal; `bundle_integrity_does_not_read_skills_through_symlinks` passes. |
| Plugin/MCP trust is local and structured | ✅ Implemented | `PluginManager::inspect_audit_snapshots_read_only` verifies selection/lock/source/provenance; status hashes through a bounded descriptor-relative/no-follow walk before discovery. `discover_skills` uses capability directory handles and rejects symlinks (`status_report_rejects_symlinked_skills_before_discovery`); audit uses structured server declarations and approval state. `McpPrivilegeRule` applies to skill-local declarations, and split secret arguments are classified. Direct network/process observers remain unavailable. |
| Versioned shared JSON report and CLI outcomes | ✅ Implemented | Scanned reports use schema version 2 and the `0`/`1`/`2` mapping, with local schema contract tests. External compatibility is not established. |
| Human/JSON report parity | ✅ Implemented | Human and JSON render from the shared `DiagnosticReport`; `human_report_includes_the_same_scan_coverage_as_json` covers target and coverage output. |
| English and Spanish CLI documentation | ✅ Implemented | Both references document audit limits and report behavior; `pnpm run docs:build` built 45 pages. |

### Coherence (Design)

| Decision | Followed? | Notes |
|----------|-----------|-------|
| Use shared diagnostic and traversal contract | ✅ Yes | Audit uses `DiagnosticReport`, shared coverage/errors and bounded traversal; schema version 2 is explicit while legacy validation reports can omit scan fields. Bundle integrity uses only manifests returned by bounded traversal. |
| Enforce bounded traversal and no-follow symlink policy | ✅ Yes | Sorted traversal, byte/file/depth/entry bounds, `open_dir_nofollow`, file open with `FollowSymlinks::No`, and explicit incomplete outcomes are implemented. |
| Never retain untrusted source as diagnostic evidence | ✅ Yes | Findings use controlled rule messages/evidence; contract tests exercise canary absence. |
| Parse supported formats and explicitly inspect skill content | ✅ Yes | Parser-backed JSON/YAML/TOML rules and Markdown/script handlers are present. |
| Reuse shared secret classification/redaction | ✅ Yes | `CredentialLiteralRule` and plugin validation reuse `src/secrets.rs`; `shared_secret_classifier_covers_credentials_and_preserves_header_references` passes. |
| Reuse plugin trust state through read-only inspection | ✅ Yes | Existing lock/provenance/approval paths produce already-present local roots; status preflights bounded hashes before discovery; audit does not use restore/materialization. |
| Normalize reports and fingerprints | ✅ Yes | Shared report sorts/deduplicates; fingerprints use SHA-256 and target/path/location identity. |
| Separate incomplete status from finding threshold | ✅ Yes | CLI checks incomplete status before threshold; `main.rs` maps audit outcomes to process exit codes. |
| Match design file-change table | ✅ Yes | The implementation includes the planned audit, diagnostics, command, plugin, validation, policy/explorer, test, and English/Spanish docs changes. |
| Confirm external report compatibility with #695 consumers | ⚠️ Cannot verify | Design open question #1 remains unchecked; external consumers and the #695 compatibility decision are unavailable. The shared secrets decision with #702 is also unchecked. |

### TDD Compliance Audit

| Metric | Status |
|--------|--------|
| TDD applicability per task | Mandatory for behavior-changing audit/traversal/plugin tasks; not applicable to documentation-only tasks. |
| RED→GREEN→REFACTOR evidence for behavior changes | ⚠️ Partial / not fully verified |
| Baseline and post-refactor evidence for behavior-preserving validation migration | ⚠️ Cannot verify from persisted apply evidence |
| Required TDD evidence recorded before completion | ❌ No complete evidence |

`apply-progress.md` states that the fail-closed plugin branch, empty-result human text, final coverage
aggregation, YAML parsing, initial Markdown-reference detection, repeated-JSON comparison, shared
secret-classifier integration, human target rendering, split MCP arguments, and directory-entry
limits lack retained pre-fix RED evidence. The latest code-review hardening (descriptor-relative
plugin hash reads, entry/depth/target bounds, and the restored skill-local structured MCP rule) has
passing regression tests in the fresh suite, but the terminal log records only final results and
does not establish test-before-implementation sequencing. The no-follow open test also passed on its
first run because the underlying capability library already rejected that Unix symlink; this is
characterization evidence, not a RED run. The available records do not establish RED→GREEN→REFACTOR
for every changed behavior or baseline/post-refactor evidence for the behavior-preserving
skill-validation migration. User authorization to continue and passing current tests do not
establish TDD compliance. Test history is insufficient to reconstruct the required ordering for all
behavioral changes. **No TDD-compliance claim is made.**

### Issues Found

**CRITICAL**

- **TDD evidence is incomplete for changed business behavior.** The apply record explicitly lists
  behavior implemented before its specific tests, including plugin fail-closed handling, empty-result
  messaging, coverage aggregation, YAML/Markdown-reference behavior, repeated-JSON comparison,
  shared-secret-classifier integration, human target rendering, split MCP arguments, and entry-limit
  handling. No complete RED→GREEN→REFACTOR evidence is persisted, and the behavior-preserving
  skill-validation migration has no baseline/post-refactor record. The final hash-tree hardening,
  restored skill-local MCP rule, split-argument handling, and entry/depth/selected-target bounds are
  covered by passing tests, but no persisted evidence establishes their RED-before-implementation
  sequence. User authorization and passing tests do not satisfy the required TDD gate. Do not claim
  TDD compliance or archive as a clean pass.

**WARNING**

- **Windows behavior is unverified.** The latest cross-target build stopped in `libz-sys`, `zstd-sys`,
  and `aws-lc-sys` before compiling AgentSync because Windows headers/Vcpkg/MSVC prerequisites were
  absent.
- **Docs setup emitted a prepare-hook wrapper warning despite a successful docs build, and the Node
  sources disagree.** Astro built 45 pages on Node `v26.13.1`, which matches the current `.nvmrc`;
  the project guidelines state a `24.14.0` target. Pnpm could not resolve the globally installed
  wrapper path during `prepare`.
- **External JSON compatibility and cross-issue decisions remain release risks.** Schema v2 passes
  local assertions, but the #695/#702 design questions remain unchecked and external consumers are
  unknown. All seven proposal success-criteria checkboxes remain unchecked despite the runtime
  evidence recorded here; the proposal artifact was not changed in this verification.
- **Acceptance QA was not rerun during this 2026-11-24 verification.** The `qa-report.md` publication
  was subsequently refreshed on 2026-11-25 using the 1,516-pass evidence recorded here, without
  rerunning tests. Its `PASS WITH WARNINGS` verdict and the three observer gaps (network, arbitrary
  process, metadata) remain separate from this report's technical verdict.
- **Some new behavior has focused rather than end-to-end coverage.** Split `--key value` detection
  passes its unit test but has no separate real-binary assertion for that exact argument form across
  output channels. The `MaxEntriesExceeded` to `traversal-entries` validation mapping is present in
  source but no dedicated validation-diagnostic assertion was found.
- **Worktree baseline is unavailable.** A status snapshot during verification showed staged and
  unstaged changes across implementation, SDD artifacts, tests, and docs; their origin cannot be
  determined. The docs install briefly advanced `pnpm-lock.yaml` wrapper versions; that incidental
  change was restored to the existing index version. This verification updated only
  `verify-report.md`; the later 2026-11-25 QA refresh is documented separately in `qa-report.md`.

**SUGGESTION**

- Keep #688 open; resolve the historical TDD gate, obtain the #695/#702 decisions and Windows
  evidence, and rerun acceptance QA if implementation or observer coverage changes. Do not archive
  while the TDD finding remains unresolved or undispositioned.

### Verdict

**FAIL** — 21/24 delta-spec scenarios have complete runtime evidence; three remain partial for
network, arbitrary MCP process, and metadata observation. The fresh serial Rust suite passed with
1,516 passed, 0 failed, and 6 ignored across 30 summaries, including 35 audit contract tests; the
default-parallel attempt had one isolated `dev_bench` broken-pipe failure. Native check, format,
strict Clippy, diff check, and the supplied docs build also exited 0; Astro built 45 pages despite
the prepare-hook warning. The historical TDD CRITICAL remains unresolved; Windows compilation and
external schema compatibility remain unverified. At this report's publication, independent QA
remained `PASS WITH WARNINGS` at its then-current 1,515-pass evidence; the separate 2026-11-25 QA
refresh used the 1,516-pass evidence without rerunning tests. Do not treat this result as release or
archive clearance.