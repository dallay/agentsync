# Acceptance QA Report: migrate-catalog-skills-phase1

## Identity

- Change: `migrate-catalog-skills-phase1`
- Mode: `openspec`
- QA phase: `qa`
- Date: `2026-08-13`

## Sources of Truth

- Exploration: `openspec/changes/migrate-catalog-skills-phase1/exploration.md`
- Proposal: `openspec/changes/migrate-catalog-skills-phase1/proposal.md`
- Specification: `openspec/changes/migrate-catalog-skills-phase1/specs/skill-recommendations/spec.md`
- Design: `openspec/changes/migrate-catalog-skills-phase1/design.md`
- Tasks: `openspec/changes/migrate-catalog-skills-phase1/tasks.md`
- Apply handoff: `openspec/changes/migrate-catalog-skills-phase1/apply-report.md`
- Technical verification: `openspec/changes/migrate-catalog-skills-phase1/verify-report.md` (`PASS WITH WARNINGS`)
- State: `openspec/changes/migrate-catalog-skills-phase1/state.yaml`
- Policy: `openspec/config.yaml`

## Target and Environment

- Target: No launchable application or operator acceptance target was supplied. The inspected surfaces were the local `agentsync` checkout and the sibling `agents-skills` checkout; the Rust test binaries were used only as technical evidence, not as a product-acceptance target.
- `agentsync`: `/Users/acosta/Dev/dallay/agentsync`, branch `feat/migrate-catalog-skills-phase1`, with six tracked source/test files modified and OpenSpec artifacts untracked.
- `agents-skills`: `/Users/acosta/Dev/dallay/agents-skills`, branch `feat/phase1-auth-db-skills`, `HEAD 17db5a3`; the three migrated directories and `PROVENANCE.md` are untracked, alongside nine Clerk and two unrelated candidate directories.
- Environment: macOS/Darwin, local filesystem, Rust/Cargo toolchain, Python 3.14 validator environment.
- Credentials/permissions: local repository access only; no product credentials, deployed endpoint, or external acceptance permission was available.
- Refresh focus: the three generated entrypoint SHA-256 values in `agents-skills/PROVENANCE.md` were corrected and recomputed successfully during this QA refresh.
- Limitations: There is no supported black-box acceptance harness, deployed target, browser surface, API target, or committed sibling revision for this QA run. Local tests and static/worktree inspection provide technical evidence but cannot establish user/operator acceptance under the QA policy.

## Capability Inventory

| Capability | Availability | Selected? | Rationale / rejection reason |
|---|---|---:|---|
| Local filesystem and Git inspection | available | selected | Required to inspect both repositories, uncommitted diffs, source isolation, and provenance state. Evidence is not product acceptance by itself. |
| Rust/Cargo focused test runner | available | selected | Executed focused resolver, catalog, caller, install, and harness checks as technical evidence only. No black-box product target was attached. |
| Offline/local-source execution | available | selected | `CARGO_NET_OFFLINE=true` and local sibling/override paths exercised the technical local-resolution paths. |
| Temporary install and `registry.json` inspection | available | selected | Focused integration test observed copied files and local registry keys in temporary directories; treated as technical evidence only. |
| Pinned `skills-ref` validation | available | selected | Validated the three migrated sibling directories individually. |
| Target repository validator | available | selected | Ran the documented whole-repository validator; its failures were limited to pre-existing Clerk candidates. |
| Browser/Playwright/Chrome | available | rejected | The target is a Rust CLI/catalog flow with no browser UI target. |
| API/client acceptance calls | unavailable | rejected | No API service or endpoint was supplied. |
| Manual operator CLI session against a real project | unavailable | rejected | No executable acceptance target/project invocation was supplied. |
| Accessibility, responsive, and locale checks | unavailable | rejected | No UI or localization surface is in scope for this CLI change. |
| Credential/authorization testing | unavailable | rejected | No authenticated target or permission model was supplied. |
| Interrupted/repeated exploratory acceptance session | unavailable | rejected | No launchable target or supported black-box harness was available. |

## Scenario Matrix

Every acceptance scenario is `NOT TESTED`: the commands below produced technical evidence, but there was no application-under-test or black-box acceptance target. Static inspection and local test binaries are not promoted to product `PASS` results.

| ID | Capability | Acceptance scenario | Result | Evidence or reason |
|---|---|---|---|---|
| QA-01 | Offline/local-source execution | Happy path: each migrated skill resolves from the local sibling `agents-skills` checkout without external resolution. | NOT TESTED | Technical checks passed, including `phase1_catalog_source_uses_sibling_agents_skills_checkout` and the focused three-skill install test; no launchable operator target was available. |
| QA-02 | Offline/local-source execution | Happy path/boundary: `AGENTSYNC_LOCAL_SKILLS_REPO/skills/{local_skill_id}` takes precedence and resolves each migrated skill locally. | NOT TESTED | Technical check `phase1_catalog_source_uses_agentsync_local_skills_repo_before_provider` passed; no product acceptance target or network-observation harness was available. |
| QA-03 | Negative/security boundary | Missing curated content fails closed and does not fall back to a mutable external source. | NOT TESTED | Technical check `phase1_catalog_source_fails_closed_when_curated_content_is_missing` passed; no black-box install target was available to observe the operator-facing error. |
| QA-04 | Persistence/install evidence | Offline installation of `drizzle-orm`, `pydantic`, and `sqlalchemy` copies `SKILL.md`, required companions, and records local IDs in `registry.json`. | NOT TESTED | Current refresh passed the focused integration test for all three entries and checked companion files plus local registry keys; this is technical test-harness evidence, not product acceptance. |
| QA-05 | Catalog identity/metadata | Recommendations retain local IDs, titles, summaries, install identities, and metadata while provider IDs become `dallay/agents-skills/{local_skill_id}` and stale sources are removed. | NOT TESTED | Catalog-boundary test passed; no user-facing recommendation target was available. |
| QA-06 | Caller propagation | Direct install/update and suggestion install propagate the real project root so sibling resolution is reachable. | NOT TESTED | Direct and suggestion project-root regression tests passed; update propagation was inspected in the diff, but no black-box CLI session was available. |
| QA-07 | Unauthorized/security boundary | A missing approved local source cannot silently resolve through provider/network fallback, while unrelated catalog entries retain their existing fallback behavior. | NOT TESTED | Technical fail-closed and unrelated-fallback tests passed; no network interception or operator target was available. |
| QA-08 | Scope/exclusion | Unrelated skills and blocked Clerk files are not remapped or included in the three-skill migration. | NOT TESTED | Git/diff inspection found no Clerk catalog remap and the target worktree still contains the nine Clerk plus two unrelated untracked candidates; static evidence cannot produce acceptance `PASS`. |
| QA-09 | State transition | Repeated, interrupted, or partially completed installation leaves a safe, predictable state and retry behavior. | NOT TESTED | No dedicated black-box repeated/interrupted acceptance scenario or target was available. Existing technical retry/install tests are part of verification evidence only. |
| QA-10 | Harness preservation | The full-catalog E2E retains `#[ignore]`, its guard, and the explicit early return while unrelated external entries remain broken. | NOT TESTED | Current `RUN_E2E=1 cargo test ... --ignored --nocapture` emitted `Skipping full catalog installation E2E: Phase 1 focused coverage is scoped to the three migrated Bobmatnyc skills` and returned `ok`; this validates harness preservation, not product acceptance or full-catalog health. |
| QA-11 | Browser | Browser installation/recommendation behavior. | NOT TESTED | No browser target; CLI change has no browser surface. |
| QA-12 | Accessibility | Keyboard/screen-reader/contrast behavior. | NOT TESTED | No UI target or accessibility surface. |
| QA-13 | Responsive | Narrow/wide viewport behavior. | NOT TESTED | No UI target or responsive surface. |
| QA-14 | Internationalization | Locale/translation behavior for recommendations and install errors. | NOT TESTED | No locale-enabled target or acceptance requirement was supplied. |
| QA-15 | Exploratory/manual | End-to-end operator workflow across both repositories with a real project root. | NOT TESTED | No launchable target, installed release binary workflow, or acceptance harness was supplied. |

## Exact Checks Run

The following commands were run during this QA refresh. Their results are technical evidence only and
do not change the scenario results above:

- SHA-256 recomputation for the three materialized entrypoints, with the recorded values checked against
  `agents-skills/PROVENANCE.md` — all matched:
  - `drizzle-orm/SKILL.md`: `31aab8f3fff9dc3b4dd0ac593f33d6b5e6583885db5a373dbcfd805b3732714f`
  - `pydantic/SKILL.md`: `6769a7817671c8673e94221357d2e2058867594a006fdc05b3d551850cf4ff99`
  - `sqlalchemy/SKILL.md`: `35e1956c80ec9b8644d6b67de13909696ee83191534a51bf0d7dbfdfca65df4b`
- `PYTHONPATH=.tools/skills-ref/lib/python3.14/site-packages python3.14 .tools/skills-ref/bin/skills-ref validate skills/drizzle-orm` — `Valid skill`.
- Same pinned `skills-ref validate` command for `skills/pydantic` — `Valid skill`.
- Same pinned `skills-ref validate` command for `skills/sqlalchemy` — `Valid skill`.
- `python3 scripts/validate_skills.py` in `agents-skills` — exit 1 as expected; exactly nine pre-existing Clerk directories failed the exact `Use when` activation-cue check, and no migrated database skill was reported.
- `CARGO_NET_OFFLINE=true cargo test --all-features --test test_catalog_integration phase1_bobmatnyc_catalog_entries_install_offline_and_register_local_ids` — `1 passed`.
- `CARGO_NET_OFFLINE=true cargo test --all-features --test all_tests 'unit::provider::'` — `16 passed`, including local override precedence, sibling lookup, fail-closed behavior, and unrelated fallback.
- `CARGO_NET_OFFLINE=true cargo test --all-features --test all_tests phase1_bobmatnyc_entries_use_curated_sources_and_preserve_boundaries` — `1 passed`.
- `CARGO_NET_OFFLINE=true cargo test --all-features --test all_tests unit::suggest_install::` — `10 passed`.
- `CARGO_NET_OFFLINE=true cargo test --all-features commands::skill::tests::direct_catalog_install_resolution_uses_the_sibling_agents_skills_checkout` — `1 passed`.
- `CARGO_NET_OFFLINE=true cargo test --all-features commands::skill::tests::suggestion_catalog_install_resolution_uses_the_project_root` — `1 passed`.
- `CARGO_NET_OFFLINE=true RUN_E2E=1 cargo test --all-features --test test_catalog_integration every_catalog_skill_installs_successfully -- --ignored --nocapture` — intentional early-return message emitted; harness test returned `ok`; no full-catalog pass claimed.
- `git status --short --branch`, `git diff --name-status`, `git diff --stat`, and `git diff --check` in `agentsync`, plus `git status --short --branch` in `agents-skills` — confirmed the current uncommitted state, no commit/push, six modified `agentsync` source/test files, untracked OpenSpec artifacts, and untracked sibling content/provenance.

## Untested Scope

- Scope: Product/user/operator acceptance of the three skill migrations, including a real CLI invocation, network-denial observation, repeat/interruption behavior, and release-like installation from a committed sibling revision.
- Reason: No target, deployed artifact, black-box acceptance harness, credentials, or committed `agents-skills` revision was supplied. The repository-level Rust tests are technical verification evidence, not a substitute for product acceptance in this QA phase.
- Re-run prerequisite: Provide a launchable CLI acceptance target or approved black-box harness, and a clean/committed sibling snapshot containing only the gated migrated content. The corrected provenance hashes are now verified. Then rerun `sdd-qa` before archive.

## Findings

| ID | Severity | Scenario / location | Evidence | Status |
|---|---|---|---|---|
| QA-F-001 | P2 | Release readiness: `agents-skills` migrated content and `PROVENANCE.md` | Target branch `feat/phase1-auth-db-skills` has the three migrated directories and provenance record untracked; no immutable sibling revision exists yet. | Open warning; does not invalidate the technical three-skill test evidence. |
| QA-F-002 | P2 | Release readiness: `agents-skills/PROVENANCE.md` materialized file hashes | Current SHA-256 values for `drizzle-orm/SKILL.md`, `pydantic/SKILL.md`, and `sqlalchemy/SKILL.md` match the corrected values recorded in `PROVENANCE.md`. | Resolved during this QA refresh; final committed bytes still require a post-commit provenance recheck. |
| QA-F-003 | P2 | Target repository validation: pre-existing Clerk candidates | Whole-repository validator reports nine Clerk directories missing the exact `Use when` cue. The migrated Bobmatnyc directories are valid individually and are not in the failures. | Open pre-existing warning; Clerk remains blocked and out of scope. |
| QA-F-004 | P2 | Full-catalog E2E | Full-catalog test remains ignored and returns early because unrelated external entries remain broken. | Accepted scope warning; preserve until a later catalog-health change. |
| QA-F-005 | P2 | Source isolation | The sibling worktree also contains untracked Clerk, `angular-architecture`, and `typescript-strict-patterns` candidates. | Open warning; provenance excludes them, but the source snapshot is not release-isolated. |
| QA-F-006 | P3 | Coverage boundary: install-all provider/local identity | No new dedicated three-recommendation `install-all` test was added in this change; existing install-all technical coverage passed. | Coverage suggestion; not an observed acceptance failure. |

No unresolved `CRITICAL`, `P0`, or `P1` product findings were observed. The missing acceptance target is a QA testability limitation, not evidence of a product defect.

**CRITICAL/P0/P1 findings:** None.

## Verdict

`NOT TESTED`

### Rationale

The technical evidence is consistent with the verify report: the three migrated entries resolve locally, fail closed when absent, install with companions and local registry keys, preserve catalog identity, propagate project roots, exclude Clerk/unrelated mappings, and retain the full-catalog early return in the available Rust test harness. However, this repository run had no application-under-test, launchable operator target, or supported black-box acceptance harness. Per the QA gate, those technical checks cannot be represented as product acceptance `PASS`; the release-readiness warnings also remain open.

## Limitations and Handoff

- QA did not modify source code, skill content, provenance, or registry files; only this report and the phase state handoff are produced.
- This report does not claim product acceptance for the AgentSync harness or the full catalog.
- Implementation/release handoff: commit and isolate the three sibling skill directories plus provenance, keep the corrected hashes synchronized with final committed bytes, keep Clerk and unrelated candidates excluded, and preserve the full-catalog early return.
- Archive is not recommended yet. Rerun `sdd-qa` with a launchable acceptance target and committed sibling snapshot; only then evaluate `sdd-archive`.
