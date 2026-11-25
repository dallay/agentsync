# Acceptance QA Report: issue-688-static-security-audit-hardening

## Identity

- Change: `issue-688-static-security-audit-hardening`
- Mode: openspec
- QA phase: Final independent acceptance refresh against the latest verification publication and current source; no tests rerun
- Date: 2026-11-25
- State: `state.yaml` records `current_phase: qa`, `completed` includes `verify` and `qa`, and `next: verify`; implementation tasks are complete. Technical verification is `FAIL` because required TDD evidence is incomplete.
- Final report state: refreshed against current source and the latest 2026-11-24 `verify-report.md`, which records 30 summaries, 1,516 passed, 0 failed, 6 ignored, including 35 `audit_output_contract` tests. QA verdict is `PASS WITH WARNINGS` for the bounded, observed native-fixture acceptance scope only; this is not release or archive clearance and does not clear technical verification.

## Sources of Truth

- Proposal: `.agents/sdd/changes/issue-688-static-security-audit-hardening/proposal.md` (read).
- Specifications: `.agents/sdd/changes/issue-688-static-security-audit-hardening/specs/static-security-audit/spec.md` (all available delta specifications read; 24 Given/When/Then scenarios).
- Design: `.agents/sdd/changes/issue-688-static-security-audit-hardening/design.md` (read; schema-v2 compatibility and cross-issue decisions remain open).
- Tasks: `.agents/sdd/changes/issue-688-static-security-audit-hardening/tasks.md` (read; 18 implementation tasks checked; TDD deviations documented).
- Apply progress: `.agents/sdd/changes/issue-688-static-security-audit-hardening/apply-progress.md` (read; records the same 30-summary suite at 1,516 passed/0 failed/6 ignored, 35 `audit_output_contract` tests, and TDD deviations; the latest verification report is the source for the QA evidence summary).
- Technical verification: `.agents/sdd/changes/issue-688-static-security-audit-hardening/verify-report.md` (read; latest verdict `FAIL`, with a `CRITICAL` TDD-evidence finding; it records that QA was not refreshed during that verification run).
- State: `.agents/sdd/changes/issue-688-static-security-audit-hardening/state.yaml` (read).
- SDD config: `.agents/sdd/config.yaml` (read; no configured coverage threshold).
- Previous QA report: read before update; this refresh corrects its stale test count and preserves the spec's 24-scenario matrix (no supplemental scenario rows).
- Current source inspected: `src/plugins.rs` and `src/skills/validation.rs`. `PluginManager::inspect_status_report_read_only` rejects symlinked local-source path components with `ensure_local_source_has_no_symlinks`, caps selected targets at 1,000, validates the selected root, and hashes the tree before `discover_plugin`; the capability-relative hash walk enforces 10,000 entries, depth 64, 10,000 files, and 500 MiB, with no-follow opens. The recorded suite includes regressions for local-source ancestor rejection, symlinked skill discovery, no-follow/bounded plugin hashing, and bundle integrity through symlinks. Skill validation collects only `SKILL.md` contents returned by bounded traversal, and `BundleIntegrityRule` consumes those scanned manifests. Source inspection is context only; scenario PASS results rely on recorded runtime tests, not static inspection alone.

## Target and Environment

- Target: Native `agentsync audit` CLI; real-binary fixtures use `CARGO_BIN_EXE_agentsync` in `tests/contracts/test_audit_output.rs`, registered by `tests/all_tests.rs`.
- Environment: macOS / Unix host. Unix-gated root/nested symlink contract cases ran on this host.
- Credentials/permissions: Synthetic canaries and temporary local project/plugin fixtures; no real credentials or external service used.
- Latest full-suite evidence from `verify-report.md` (2026-11-24): `cargo test --all-features` exited 0; 30 test-result summaries, 1,516 passed, 0 failed, 6 ignored. The suite includes 35 `audit_output_contract` tests. This QA refresh uses the recorded run and does not rerun tests.
- Current relevant behavior in that evidence: skill-local structured MCP privilege detection has a real-binary contract test; split-argument MCP secret detection has a unit test; plugin hashing rejects symlink file reads and enforces entry/depth/file/byte limits; audit inventory caps selected targets before lock access. New recorded regressions cover read-only status rejecting symlinked local-source ancestors and skills directories, and bundle integrity ignoring `SKILL.md` manifests reached through symlinks. Source limits are 1,000 selected audit targets, 10,000 directory entries, depth 64, 10,000 files, and 500 MiB aggregate content. The split-argument assertion is not a separate real-binary output-channel case.
- Platform/build caveats: Windows cross-target compilation stopped before AgentSync compilation because Windows system headers/Vcpkg/MSVC prerequisites were unavailable; no Windows runtime test was possible. The docs build used Node `v26.13.1`, not the project target `24.14.0`; Astro built 45 pages, but the pnpm `prepare` hook failed to find the globally installed wrapper.
- Scope limits: Tests compare plugin fixture paths/bytes and a hook sentinel, but do not directly observe network attempts, arbitrary child/MCP process launches, or filesystem metadata such as timestamps and permissions. External schema-v2 consumer compatibility is not established.

## Capability Inventory

| Capability | Availability | Selected? | Rationale / rejection reason |
|---|---|---|---|
| Native CLI with real-binary fixture suite | Available | Yes | Narrow applicable target; latest full-suite evidence records 35 passing `audit_output_contract` tests. |
| Captured human/JSON stdout and stderr | Available | Yes | Contract tests assert redaction, parseable JSON, output separation, report shape and human/JSON parity. |
| Local filesystem fixtures and Unix symlinks | Available | Yes | Contract tests exercise selectors, references, limits, root/nested symlinks and local snapshot contents on macOS. |
| Plugin lock/provenance and MCP approval fixtures | Available | Yes | Local contract tests exercise locked snapshots, invalid/missing trust data and approval state. |
| Unit-level rule/hash/preflight tests in full suite | Available | Yes | Used as limited runtime evidence for split-argument secret parsing, Unix no-follow plugin hashing and bounded plugin preflight; not represented as a separate CLI output-channel test. |
| Current source inspection | Available | Yes, context only | Confirms current status ancestor checks, bounded hash-before-discovery flow, no-follow skill discovery, and bounded scanned-manifest use; static inspection is not used by itself to pass a scenario. |
| Network request interception | Unavailable | No | No direct network monitor/interceptor is evidenced; fixture behavior cannot prove absence of attempted requests. |
| Arbitrary process/MCP launch observation | Unavailable | No | No OS-level process observer is evidenced; the fixture sentinel does not cover arbitrary launches. |
| Filesystem metadata snapshot | Unavailable | No | Fixture snapshot checks names and bytes, not timestamps, permissions or other metadata. |
| Windows runtime/cross-target execution | Unavailable / externally blocked | No | Required native headers/Vcpkg/MSVC prerequisites were missing before crate compilation; no Windows runtime is available. |
| External schema consumer | Unavailable | No | No downstream consumer or finalized #695 shared-contract decision is evidenced. |
| Browser, accessibility, responsive or browser-locale checks | Rejected | No | Not applicable to this native CLI target. |
| Deterministic QA runner/FSM | Unavailable | No | No runner is configured; evidence is in `fallback` mode and is not deterministic-runner enforcement. |

## Scenario Matrix

All 24 delta-spec scenarios are mapped below. Evidence is the latest full-suite result recorded in `verify-report.md`; this QA publication did not rerun tests. There are 21 `PASS` results and three `NOT TESTED` results where the complete security guarantee requires an unavailable direct observer.

| ID | Capability | Acceptance scenario | Result | Evidence or reason |
|---|---|---|---|---|
| QA-01 | Captured CLI output | Credential canary is detected without disclosure in human/JSON output. | PASS | `audit_output_does_not_expose_source_lines` and `quoted_json_secret_keys_are_parsed_and_redacted` assert canary absence from captured output. |
| QA-02 | Captured CLI output | A command-rule source line containing a canary is reported without source-line evidence. | PASS | `audit_output_does_not_expose_source_lines` exercises the real binary in human and JSON modes with `RUST_LOG=trace`. |
| QA-03 | Target validation | Invalid skill selector is rejected without scanning outside the authorized skill root. | PASS | `invalid_skill_selector_does_not_scan_outside_the_skill_root` asserts incomplete outcome and canary absence. |
| QA-04 | Unix filesystem fixtures | Root or nested escaping symlink is not followed and produces incomplete status. | PASS | `symlink_skill_root_is_incomplete_and_not_followed` and `nested_symlink_is_skipped_and_incomplete_takes_precedence_over_threshold` pass on the Unix host. |
| QA-05 | Reference containment | Referenced path escaping the authorized root is reported without opening the outside target. | PASS | `markdown_reference_that_escapes_the_target_is_reported_without_reading_it` and `structured_file_reference_escape_is_reported_without_reading_the_target` pass. |
| QA-06 | Traversal fixtures | A traversal/read limit stops further reading and reports incomplete coverage. | PASS | `file_limit_produces_incomplete_coverage_without_reading_the_file` and traversal tests for depth, per-file/aggregate bytes, file count, and read failures pass in the recorded suite. |
| QA-07 | Bounded enumeration | Directory enumeration beyond its configured bound is incomplete. | PASS | `traversal_report_caps_directory_entry_enumeration` and `all_inventory_reports_incomplete_when_skill_entry_limit_is_exceeded` exercise the 10,000-entry cap. |
| QA-08 | Target resolution | Requested missing or unavailable target is incomplete, not a clean scan. | PASS | `missing_skill_is_reported_as_an_incomplete_scan` and `unavailable_plugin_source_or_invalid_provenance_is_incomplete` pass. |
| QA-09 | Content coverage | Unsupported/non-UTF-8 content is counted skipped and marks the scan incomplete. | PASS | `unsupported_non_utf8_files_are_skipped_and_mark_the_scan_incomplete` asserts status, skipped count, and stable error code. |
| QA-10 | CLI content fixtures | Markdown skill prose/fenced shell pattern is detected with sanitized evidence. | PASS | `skill_markdown_is_checked_for_credentials_and_fetch_patterns` passes. |
| QA-11 | Structured content fixtures | A quoted JSON sensitive key is parsed and its canary is not disclosed. | PASS | `quoted_json_secret_keys_are_parsed_and_redacted` passes. |
| QA-12 | CLI content fixtures | Supported JSON, YAML, TOML, Markdown, and script/text content is inspected and reflected in coverage. | PASS | `skill_markdown_is_checked_for_credentials_and_fetch_patterns`, `yaml_sensitive_keys_are_parsed_and_redacted`, `structured_configuration_is_checked_for_fetch_and_execute_patterns`, and `fetch_patterns_are_checked_in_all_declared_script_and_text_formats` pass. |
| QA-13 | Plugin trust fixtures | A locked local snapshot is audited with namespaced identity. | PASS | `locked_plugin_snapshot_is_audited_read_only_with_namespaced_mcp_state` passes; it asserts namespaced findings and compares fixture paths/bytes before and after. The latest suite also includes `status_report_rejects_symlinked_local_source_ancestors`, `status_report_rejects_symlinked_skills_before_discovery`, and `skill_discovery_rejects_a_symlinked_skills_directory`; current status source checks local-source components and rejects the link before discovery. |
| QA-14 | Plugin trust fixtures / network observer | Missing snapshot or invalid provenance is incomplete without materialization or network access. | NOT TESTED | `unavailable_plugin_source_or_invalid_provenance_is_incomplete` and `invalid_plugin_provenance_is_incomplete_and_safe` verify incomplete/safe results, but no direct network observer establishes that no request was attempted. |
| QA-15 | MCP approval fixtures / process observer | Pending MCP approval is represented without launching the server. | NOT TESTED | `locked_plugin_snapshot_is_audited_read_only_with_namespaced_mcp_state` exercises pending approval; no process observer establishes absence of arbitrary MCP/server launches. |
| QA-16 | Plugin hash/preflight tests | Entry, depth, file, byte, and selected-target limits stop processing and fail closed. | PASS | The recorded suite includes `plugin_hash_reader_rejects_symlink_files`, `plugin_hash_tree_caps_directory_entries`, `plugin_hash_tree_caps_directory_depth`, `plugin_source_hash_rejects_excessive_file_count`, `plugin_source_hash_rejects_excessive_content_size`, and `audit_snapshot_inventory_rejects_too_many_selected_plugins_before_lock_access`. Current source performs bounded capability-relative tree hashing before `discover_plugin`; it rejects symlinks and caps at 10,000 entries, depth 64, 10,000 files, 500 MiB, and 1,000 selected targets. |
| QA-17 | Deterministic CLI fixtures | Equivalent scans produce stable JSON, findings, and secret-independent fingerprints. | PASS | `repeated_audits_emit_identical_json`, `credential_fingerprints_are_sha256_and_ignore_the_secret_value`, and `all_audit_uses_the_same_skill_identity_as_direct_selection` pass. |
| QA-18 | JSON contract | Local versioned report identifies schema and contains coverage/error fields. | PASS | `complete_json_report_includes_versioned_coverage` asserts schema version 2 and required fields; external consumer compatibility is not established. |
| QA-19 | CLI exit behavior | Complete scan below threshold returns 0 and does not certify safety. | PASS | `empty_human_report_does_not_certify_safety` passes. |
| QA-20 | CLI exit behavior | Finding at configured threshold returns 1. | PASS | `findings_meeting_the_threshold_return_one` passes. |
| QA-21 | CLI exit behavior | Incomplete scan with findings returns 2, regardless of threshold. | PASS | `nested_symlink_is_skipped_and_incomplete_takes_precedence_over_threshold` passes. |
| QA-22 | CLI argument validation | Conflicting selectors are rejected before scanning. | PASS | `audit_target_selectors_are_mutually_exclusive` asserts empty stdout and a Clap conflict on stderr. |
| QA-23 | Network/process/filesystem observation | Local fixtures cause no network or command execution and leave contents and metadata unchanged. | NOT TESTED | `locked_plugin_snapshot_is_audited_read_only_with_namespaced_mcp_state` compares paths/bytes and checks a hook sentinel; this is not direct network/process observation and does not snapshot timestamps, permissions, or other metadata. |
| QA-24 | Captured JSON/stdout/stderr | JSON stdout is exactly one parseable report and stderr contains no source content. | PASS | JSON contract tests deserialize stdout; `audit_output_does_not_expose_source_lines` checks stdout/stderr. |

## Untested Scope

- Scope: Direct network-attempt observation; arbitrary child/MCP launch observation; filesystem timestamp/permission/metadata immutability; Windows behavior; external schema-v2 consumer compatibility. Split-argument MCP detection has unit evidence but no separate real-binary output-channel assertion for that exact argument form.
- Reason: Required observers, Windows native prerequisites, and an external consumer/#695 contract decision are unavailable. Passing source/tests cannot substitute for these missing observations.
- Re-run prerequisite: Add controlled network/process/metadata observation for applicable fixtures, provide a Windows runner with native toolchain prerequisites, exercise split-argument redaction at the binary output boundary, and obtain a representative downstream consumer plus the #695 compatibility decision. Re-run QA if implementation or observation coverage changes.

## Findings

| ID | Severity | Scenario / location | Evidence | Status |
|---|---|---|---|---|
| QA-F1 | P2 | Offline and no-process guarantees | No direct network interceptor or arbitrary process/MCP observer; current fixture sentinel is narrow. | NOT TESTED |
| QA-F2 | P2 | Filesystem metadata immutability | Fixture comparison captures paths and bytes, not timestamps, permissions or other metadata. | NOT TESTED |
| QA-F3 | P2 | Windows CLI and symlink behavior | Cross-target build stopped before crate compilation because native prerequisites were missing; no Windows runtime. | BLOCKED |
| QA-F4 | P2 | External JSON schema compatibility | Local schema-v2 assertions pass; downstream consumer and #695 compatibility decision unavailable. | NOT TESTED |
| QA-F5 | P2 | Split-argument MCP output-channel acceptance | Unit assertion passes for finding message/evidence; no separate real-binary output-channel fixture for this exact argument form. | PARTIAL EVIDENCE |
| QA-F6 | P2 | Documentation build environment | Node `v26.13.1` differs from project target `24.14.0`; pnpm `prepare` failed to find the global wrapper although Astro built 45 pages. | WARNING |

Technical verification is a separate gate, not a QA behavior result: the latest `verify-report.md` verdict is exactly **FAIL** because a **CRITICAL** historical TDD evidence gap remains. Persisted records do not establish complete RED→GREEN→REFACTOR evidence for changed behavior or baseline/post-refactor evidence for the validation migration. Passing final tests and user authorization to continue do not resolve that gap.

## Verdict

`PASS WITH WARNINGS`

### Rationale

The latest recorded full suite passed with 1,516 passed, 0 failed, and 6 ignored across 30 summaries, including 35 `audit_output_contract` tests. The 21 scenarios with observable runtime evidence pass, including redaction, containment, coverage/limits, supported formats, locked plugin identity, deterministic reports, local schema shape, and CLI outcomes; latest regressions cover read-only status rejecting symlinked local-source ancestors and symlinked skills before discovery, bounded hash preflight before plugin discovery, and bundle integrity consuming only bounded scanned manifests. Three scenarios remain `NOT TESTED` for direct network, arbitrary process, and filesystem-metadata observation. Windows is `BLOCKED`; external schema-v2 compatibility is unresolved. `PASS WITH WARNINGS` is bounded to the tested macOS/local-fixture scope, does not certify unobserved guarantees, and is not release or archive clearance; technical verification remains `FAIL` with a `CRITICAL` TDD-evidence finding.

## Limitations and Handoff

- QA does not fix code. This update changes only this report; no product code, tests or specs were modified.
- Evidence is the latest verification report's recorded run (1,516 passed, 0 failed, 6 ignored across 30 summaries; 35 `audit_output_contract` tests), not static inspection alone. This QA refresh did not rerun tests or duplicate the technical verification pass.
- Network attempts, arbitrary process/MCP launches, and filesystem metadata mutation were not directly observed. Windows behavior is blocked, external schema compatibility is untested, and the split-argument MCP path lacks a separate binary-level output-channel assertion.
- The docs build used Node `v26.13.1` (the `.nvmrc` value) versus the stated project target `24.14.0`; Astro built 45 pages, but pnpm's `prepare` hook failed to resolve the globally installed wrapper.
- Technical verification remains **FAIL** on the **CRITICAL** TDD evidence gap. Do not archive or represent this as a clean release gate until `sdd-verify` resolves or formally dispositions that finding and the applicable acceptance gaps are addressed.
- Follow-up: return to `sdd-verify` for TDD evidence remediation/disposition; rerun QA if implementation or acceptance evidence changes. Review Windows/schema/observer gaps before archive or release, and do not infer safety from an empty audit result.
