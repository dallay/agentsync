# Acceptance QA Report: migrate-catalog-skills-phase1

## Identity

- Change: `migrate-catalog-skills-phase1`
- Mode: `openspec`
- QA phase: `qa` (rerun after approved remediation)
- Date: `2026-08-14`

## Sources of Truth and Technical Verification Handoff

- Proposal: `openspec/changes/migrate-catalog-skills-phase1/proposal.md`
- Specification: `openspec/changes/migrate-catalog-skills-phase1/specs/skill-recommendations/spec.md`
- Design: `openspec/changes/migrate-catalog-skills-phase1/design.md`
- Tasks: `openspec/changes/migrate-catalog-skills-phase1/tasks.md`
- Apply handoff: `openspec/changes/migrate-catalog-skills-phase1/apply-report.md`
- Technical verification: `openspec/changes/migrate-catalog-skills-phase1/verify-report.md`
- State: `openspec/changes/migrate-catalog-skills-phase1/state.yaml`
- Policy: `openspec/config.yaml`
- Previous QA: this report replaces the stale prior `NOT TESTED` claims.

The `sdd-verify` handoff is `PASS WITH WARNINGS`. It reports that the release build, focused
tests, provenance refresh, and the new external acceptance target are technically executable. This
QA rerun independently exercised the documented external target and does not reinterpret static
inspection or in-process Rust tests as acceptance evidence.

## Target, Environment, Permissions, and Limitations

- Target: `/Users/acosta/Dev/dallay/agentsync/target/release/agentsync`, rebuilt locally with
  `cargo build --release` and launched by the shell harness as a separate process.
- AgentSync source reality: local `HEAD` is `8408753`; its tree matches `origin/main` at merged
  commit `7465d920b6c801b17c232e551a954041890c60f9`.
- Curated source: working `/Users/acosta/Dev/dallay/agents-skills` checkout. The three migrated
  skill directories match the supplied `agents-skills` merge commit `be4570aa...`; only the
  remediation provenance/wrapper files are dirty. The harness copies only those three directories
  into isolated sibling, override, and empty-source fixtures.
- Environment: macOS/Darwin; local release binary, Bash, Python 3, and temporary filesystem
  project roots. The harness sets a temporary `HOME`, disables update checks, and uses local source
  fixtures.
- Permissions: local read/execute and filesystem-write permissions for temporary fixtures; no
  deployed endpoint, product credentials, external operator account, or authorization target.
- Limitations: the acceptance target does not intercept packets, simulate an interrupted process,
  or assert retry/resume behavior. The full-catalog E2E remains intentionally early-returned and is
  not a catalog-health result. The remediation is intentionally uncommitted, so final committed
  provenance bytes require a later recheck before archive.

## Capability Inventory

| Capability | Availability | Selected? | Rationale / rejection reason |
|---|---|---:|---|
| External release-like CLI process | available | selected | `make acceptance-phase1` launches the executable externally and observes exit status, JSON, stderr, files, and registry state. |
| Isolated filesystem fixtures | available | selected | The harness creates temporary sibling, override, missing-source, command-CWD, and project-root fixtures and copies only the approved three skills. |
| Shell/JSON/registry assertions | available | selected | The harness asserts install results, provider identity, local IDs, companions, source markers, and absence of unintended paths. |
| Provenance validator | available | selected | Both the acceptance wrapper and direct Python validator recompute all ten current materialized hashes. |
| Release build | available | selected | Rebuilt the executable used by the external acceptance run. |
| Rust/Cargo technical tests | available | rejected for acceptance | Already owned by `sdd-verify`; in-process tests are not used to manufacture acceptance `PASS` results here. |
| Full-catalog integration boundary | available | selected for scope evidence | The ignored test was run only to confirm the intentional early-return message, not to claim catalog health. |
| Network interception | unavailable | rejected | No packet-blocking/proxy capability is provided by the repository harness. |
| Interrupted/retry operator flow | unavailable | rejected | The documented harness has no process interruption or resume scenario. |
| Browser/Playwright/Chrome | available | rejected | This is a CLI-only change with no browser surface. |
| Accessibility/responsive checks | unavailable | rejected | No UI, viewport, or screen-reader surface is in scope. |
| Locale/internationalization checks | unavailable | rejected | No locale-enabled target or requirement is supplied. |
| Credentials/authorization checks | unavailable | rejected | No authenticated target or permission model is supplied. |
| Manual release-operator session | unavailable | rejected | The approved target is the reproducible external harness, not a deployed operator environment. |

## Evidence Commands and Results

These are the exact commands run for this QA rerun:

| Command | Result |
|---|---|
| `cargo build --release` | PASS — release profile finished successfully. |
| `make acceptance-phase1 AGENTSYNC_BIN=target/release/agentsync AGENTSYNC_SOURCE_REPO=../agents-skills` | PASS — external harness completed sibling installs, override precedence, missing-source fail-closed, and suggestion project-root checks. |
| `tests/acceptance/test_phase1_catalog_harness.sh` | PASS — missing-binary contract rejected the absent executable. |
| `AGENTSYNC_SOURCE_REPO=../agents-skills tests/acceptance/test_phase1_provenance.sh` | PASS — `[OK] validated materialized hashes`. |
| `python3 scripts/validate_provenance.py --root .` from `agents-skills` | PASS — `[OK] validated materialized hashes`. |
| `bash -n tests/acceptance/*.sh && git diff --check` | PASS — no output/errors. |
| `git diff --check` from `agents-skills` | PASS — no output/errors. |
| `CARGO_NET_OFFLINE=true RUN_E2E=1 cargo test --all-features --test test_catalog_integration every_catalog_skill_installs_successfully -- --ignored --nocapture` | PASS only for the intentional early return: `Skipping full catalog installation E2E: Phase 1 focused coverage is scoped to the three migrated Bobmatnyc skills`. No full-catalog health claim. |
| `git diff --exit-code be4570aa6f23931f51692661d163ddd663b57488 -- skills/drizzle-orm skills/pydantic skills/sqlalchemy` from `agents-skills` | PASS — migrated skill content matches the supplied merge commit. |

The post-run worktree still shows only the intended remediation files plus the pre-existing
untracked Clerk/Angular/TypeScript candidate paths. The harness did not modify or copy those
unrelated candidates into any acceptance fixture.

## Scenario Matrix

Every scenario has one allowed result. `PASS` below is reserved for behavior observed by the
external harness; static inspection and technical-only checks are not promoted to acceptance.

| ID | Capability | Acceptance scenario | Result | Evidence or reason |
|---|---|---|---|---|
| QA-01 | External CLI / sibling fixture | Each approved Phase 1 entry (`drizzle-orm`, `pydantic`, `sqlalchemy`) installs through the external CLI from the sibling fixture. | PASS | `make acceptance-phase1 ...` ran all three installs, asserted JSON `status=installed`, checked installed `SKILL.md`, and verified local registry entries. |
| QA-02 | Persistence / companions | Installed state uses each local ID for the destination folder and `registry.json`, and required companions survive recursive installation. | PASS | The harness asserted all three local registry keys plus Drizzle's four references, Pydantic's `references/full-source.md`, and SQLAlchemy's two required references. |
| QA-03 | Override precedence | A valid `AGENTSYNC_LOCAL_SKILLS_REPO` source takes precedence over a sibling source. | PASS | The external override install contained the override marker and did not contain the sibling marker. |
| QA-04 | Negative / fail-closed boundary | An empty curated source fails closed without mutable external fallback or an installed directory. | PASS | The external process failed, stderr contained `refusing external fallback`, and the expected `pydantic` directory was absent. |
| QA-05 | Suggestion / project-root propagation | Suggestion installation uses the supplied project root, preserves qualified provider identity, and installs the local ID. | PASS | The harness created a `pyproject.toml`, observed `pydantic` with provider ID `dallay/agents-skills/pydantic`, installed it under the supplied project root, checked its companion and registry key, and confirmed no install in the command CWD. |
| QA-06 | Suggestion negative/boundary | The Phase 1 suggestion fixture does not attempt unrelated skill installs or report failed installs. | PASS | The harness asserted result IDs were limited to the explicitly allowed preinstalled generic skills plus `pydantic`, and asserted no result had `status=failed`. |
| QA-07 | Repeated / interrupted / retry | A partial or interrupted installation can be retried with a safe operator-visible state. | NOT TESTED | The documented harness uses fresh temporary roots and has no interruption or resume assertion. Rerun prerequisite: add a dedicated external process-kill and retry fixture. |
| QA-08 | Network boundary | Network traffic is intercepted/denied while local resolution succeeds and external fallback is proven absent. | NOT TESTED | The harness uses local fixtures and disables update checks but does not intercept packets. Rerun prerequisite: add a network-denial or proxy-observation capability. |
| QA-09 | Authorization/security | Unauthorized access or permissions cannot bypass curated-source gating. | NOT TESTED | No authenticated or multi-user target exists; QA-04 covers the observable missing-source gate only. Rerun prerequisite: provide a permissioned operator target if this surface becomes applicable. |
| QA-10 | Full-catalog boundary | Full-catalog execution reports health while unrelated external entries remain unresolved. | NOT TESTED | Intentionally out of scope; the ignored test early-returns. The command confirms scope preservation, not full-catalog health. Rerun prerequisite: a later catalog-health change that removes the early return and supplies all required sources. |
| QA-11 | Browser/accessibility/responsive | Browser, keyboard, screen-reader, viewport, and responsive behavior. | NOT TESTED | Not applicable to this CLI; no browser or UI surface exists. |
| QA-12 | Internationalization | Locale-specific recommendation and installation behavior. | NOT TESTED | No locale-enabled target or requirement exists. |
| QA-13 | Exploratory/manual operator flow | A human release operator completes the workflow against a deployed or approved release environment. | NOT TESTED | The approved evidence surface is the reproducible external harness; no deployed target or operator credentials were supplied. |

## Untested Scope

- Scope: interruption/retry semantics, packet-level network interception, authenticated permission
  behavior, full-catalog health, manual release operation, and UI/locale categories that do not
  apply to this CLI.
- Reason: these behaviors are not asserted by the documented Phase 1 black-box target, are
  explicitly out of scope, or have no applicable target/capability. The harness-covered Phase 1
  install, persistence, companion, override, fail-closed, and project-root scenarios do have
  observable evidence.
- Rerun prerequisites: extend the external harness with a controlled interruption/retry scenario
  and network-denial observation if those acceptance risks become required; provide a permissioned
  target for authorization checks; and remove the full-catalog early return only in a later catalog
  health change.
- Archive prerequisite: persist the intended remediation and re-run provenance validation against
  the final committed sibling bytes before treating the source as immutable release input.

## Findings

| ID | Severity | Scenario / location | Evidence | Status |
|---|---|---|---|---|
| QA-F-001 | P2 | `agents-skills/PROVENANCE.md` and remediation tooling | All ten hashes pass against the current working-tree bytes, but the refreshed provenance and validator wiring are intentionally uncommitted. | Open warning; recheck final committed bytes before archive/release. |
| QA-F-002 | P2 | Full-catalog integration | The ignored full-catalog test intentionally early-returns while unrelated external entries remain unresolved. | Accepted scope warning; no full-catalog health claim. |
| QA-F-003 | P2 | Acceptance coverage boundary | The external target does not cover interruption/retry or packet-level network interception. | Accepted non-blocking warning; add capabilities if those risks become acceptance requirements. |
| QA-F-004 | P3 | Local `agents-skills` worktree hygiene | Pre-existing Clerk, Angular, and TypeScript candidates remain untracked. The harness copies only the three approved Phase 1 directories and the post-run status confirms they remain untouched. | Preserved/excluded; not a Phase 1 behavior failure. |
| QA-F-005 | P3 | Repository-wide sibling validation | The broad wrapper enumerates unrelated candidates and is not used as the focused acceptance result; the provenance-specific validator and migrated-skill checks pass. | Environment limitation; keep unrelated candidates out of this change. |

No `CRITICAL`, `P0`, or `P1` findings were observed. No harness-covered scenario failed or was
blocked.

## Verdict

`PASS WITH WARNINGS`

### Rationale

The approved external acceptance target now exists and was run against the rebuilt release-like
executable. It observed the CLI as an external process and passed the Phase 1 behaviors it claims:
all three local installs, companions and registry state, override precedence, missing-source
fail-closed behavior, and direct/suggestion project-root and provider/local-ID propagation. The
refreshed working-tree provenance also validates all ten materialized hashes.

The verdict is deliberately scoped. It does not claim full-catalog health, interruption/retry
coverage, packet-level network interception, authorization behavior, or UI acceptance. It also does
not claim that the harness itself is a product acceptance surface. Those limitations are recorded
as non-blocking warnings, and there are no unresolved `CRITICAL`, `P0`, or `P1` findings.

## Archive Readiness and Implementation Handoff

- QA gate: **policy-allowed `PASS WITH WARNINGS`**. `verify-report.md` and this report exist, and
  no unresolved `CRITICAL`/`P0`/`P1` finding remains.
- Archive action: **not run**. The user explicitly requested no archive, and the remediation files
  remain intentionally uncommitted. Before archive, persist only the intended changes and rerun
  provenance against the final committed bytes.
- Handoff: preserve the full-catalog early return and the unrelated untracked candidates; do not
  broaden Phase 1 to Clerk or claim full-catalog green status. Extend the harness only if retry,
  network-interception, or authorization behavior becomes a required acceptance contract.
- QA did not modify production source, catalog data, registry manifests, skill content, commit
  history, or remediated candidate files.

## Limitations

- QA is an auditable acceptance record for the documented target, not a claim that the harness itself
  has product acceptance.
- No source code or skill content was changed to fix findings during QA.
