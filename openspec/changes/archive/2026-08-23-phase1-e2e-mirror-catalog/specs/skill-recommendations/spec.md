# Delta Spec — phase1-e2e-mirror-catalog

## Change Context

The scheduled CI job **"Deterministic offline catalog E2E"** in `.github/workflows/catalog-e2e.yml`
fails because the `offline` and `catalog-installation` jobs never check out sibling
`dallay/agents-skills` and never export `AGENTSYNC_LOCAL_SKILLS_REPO`, so
`tests/test_catalog_integration.rs::phase1_bobmatnyc_catalog_entries_install_offline_and_register_local_ids`
trips the fail-closed guard at `src/skills/provider.rs:244-250`. The previous SDD's
`verify-report.md` passed only because the local workstation happened to have the sibling checkout;
CI never exercised that path. This delta restores CI as the deterministic guard, hardens the
upstream provenance gate on every newly committed skill, and tightens `validate_provenance.py` so
the next drift fails locally. Scope is fixed by user confirmation: `clerk-orgs` is deferred, no
edits touch `src/skills/catalog.v1.toml` (PR #569's territory), `angular-architecture` /
`typescript-strict-patterns` disposition awaits design, and Clerk upstream license evidence awaits
design. Delivery is **chained PRs**: PR 1 = workflow fix alone; PR 2+ = content groups per skill.

## Modified Requirements

### REQ-SKILLREC-001 — Approved Local Curated Source Resolution (Phase 1 expanded)

**Source**: skill-recommendations §Approved Local Curated Source Resolution (archived delta
`2026-08-23-migrate-catalog-skills-phase1`).
**Capability**: Resolution
**Change**: MODIFIED
**Description**: `local_catalog_skill_source_dir()` MUST resolve every Phase 1 local skill ID from
the sibling `dallay/agents-skills` checkout when `AGENTSYNC_LOCAL_SKILLS_REPO` is set or when a
sibling checkout is present at `<project_root_parent>/agents-skills`. The Phase 1 ID families MUST
include the three Bobmatnyc DB skills (`drizzle-orm`, `pydantic`, `sqlalchemy`) AND the eight Clerk
skills declared in issue #556 (`clerk-setup`, `clerk-nextjs-patterns`, `clerk-react-patterns`,
`clerk-vue-patterns`, `clerk-astro-patterns`, `clerk-webhooks`, `clerk-testing`, `clerk-custom-ui`).
Resolution MUST NOT use network or mutable archives; missing sources MUST fail closed for every
Phase 1 ID, not just the three already wired. `clerk-orgs` is explicitly out of scope for this
change.
(Previously: covered only `drizzle-orm`, `pydantic`, `sqlalchemy`; no CI enforcement existed.)

#### Scenario: Bobmatnyc DB family resolves offline in CI

- **WHEN** `AGENTSYNC_LOCAL_SKILLS_REPO` points at a sibling checkout containing
  `skills/{drizzle-orm,pydantic,sqlalchemy}/SKILL.md`
- **THEN** `local_catalog_skill_source_dir()` MUST return the matching directory for each ID
- **AND** `phase1_bobmatnyc_catalog_entries_install_offline_and_register_local_ids` MUST pass with
  no network access

#### Scenario: Clerk auth family resolves offline in CI

- **WHEN** `AGENTSYNC_LOCAL_SKILLS_REPO` points at a sibling checkout containing
  `skills/{clerk-setup,clerk-nextjs-patterns,clerk-react-patterns,clerk-vue-patterns,
  clerk-astro-patterns,clerk-webhooks,clerk-testing,clerk-custom-ui}/SKILL.md`
- **THEN** `local_catalog_skill_source_dir()` MUST return the matching directory for each of the
  eight IDs
- **AND** `provider.resolve()` MUST NOT be called for any of them

#### Scenario: Missing source fails closed for every Phase 1 ID

- **WHEN** `AGENTSYNC_LOCAL_SKILLS_REPO` is unset AND no sibling checkout exists
- **THEN** `resolve_catalog_install_source()` MUST return an explicit error naming the missing ID
- **AND** the error MUST be raised for any of the eight Clerk IDs the same way it is raised for
  `drizzle-orm` today
- **AND** the error message MUST NOT silently fall back to network resolution

## Added Requirements

### REQ-SKILLREC-002 — CI Workflow Sibling Skills Checkout

**Capability**: CI Gate
**Description**: `.github/workflows/catalog-e2e.yml` MUST check out `dallay/agents-skills` and
export `AGENTSYNC_LOCAL_SKILLS_REPO` on every CI job that exercises Phase 1 catalog resolution.
The `offline` and `catalog-installation` jobs MUST both include an `actions/checkout` step pinning
`dallay/agents-skills` to a specific immutable commit SHA (the latest validated commit, currently
`c2e79fbb72d146305f82a8e979270795557d24fd`), placed at `${{ github.workspace }}/agents-skills`,
and MUST set `AGENTSYNC_LOCAL_SKILLS_REPO: ${{ github.workspace }}/agents-skills`. Branches MUST
NOT be used as the pin source. `remote-refresh` is intentionally out of scope (it has its own
pinned-commit contract).

#### Scenario: Offline job succeeds on a clean runner with sibling checkout

- **WHEN** the `offline` job runs on `ubuntu-latest` with the sibling checkout and env var present
- **THEN** `cargo test --test test_catalog_integration --locked --offline -- --nocapture` MUST pass
- **AND** the test MUST NOT depend on any pre-existing local skill fixture outside the sibling
  checkout

#### Scenario: Missing AGENTSYNC_LOCAL_SKILLS_REPO fails loudly

- **WHEN** the sibling checkout step is omitted or `AGENTSYNC_LOCAL_SKILLS_REPO` is unset
- **THEN** the `offline` and `catalog-installation` jobs MUST fail with a visible error
- **AND** the failure MUST be attributable to the missing env var, not a generic network or
  fixture error
- **AND** the failure message MUST name `AGENTSYNC_LOCAL_SKILLS_REPO` so operators can remediate

### REQ-SKILLREC-003 — Provenance Frontmatter Gate

**Capability**: Provenance
**Description**: `scripts/validate_provenance.py` MUST reject every `skills/<id>/SKILL.md` whose
frontmatter is missing `metadata.source`. When `metadata.source` is present and not equal to
`dallay-original`, the script MUST also require `metadata.source_commit` to be a 40-character
hexadecimal SHA. The check MUST fail loudly (non-zero exit, named offender) before any SHA-256
hash recomputation runs. The script MUST be the single gate invoked from `scripts/validate-skills.sh`
so the new rule runs on every CI invocation.

#### Scenario: RED — missing metadata.source fails validation

- **WHEN** a `skills/<id>/SKILL.md` exists with frontmatter that omits `metadata.source`
- **THEN** `python3 scripts/validate_provenance.py --root .` MUST exit non-zero
- **AND** the stderr MUST name the offending `<id>` and the missing `metadata.source` field

#### Scenario: RED — missing source_commit for non-original source

- **WHEN** a SKILL.md sets `metadata.source = "clerk/skills"` but omits `metadata.source_commit`
- **THEN** the validator MUST exit non-zero
- **AND** the stderr MUST name the offending file and the missing `source_commit` field

#### Scenario: GREEN — dallay-original source is exempt from source_commit

- **WHEN** a SKILL.md sets `metadata.source = "dallay-original"` and omits `metadata.source_commit`
- **THEN** the validator MUST accept the entry
- **AND** the SHA-256 hash check MUST still run for the file body

#### Scenario: GREEN — upstream source with immutable SHA passes

- **WHEN** a SKILL.md sets `metadata.source = "clerk/skills"` and
  `metadata.source_commit = "abcdef0123456789abcdef0123456789abcdef01"` (40 hex chars)
- **THEN** the validator MUST accept the entry

### REQ-SKILLREC-004 — PROVENANCE.md Completeness on Materialization

**Capability**: Provenance
**Description**: `PROVENANCE.md` MUST contain one `Materialized entries` table row and one
`- '<path>': '<sha256>'` line per newly committed skill (Phase 1 expansion + any later addition).
When Clerk upstream evidence is confirmed by design phase, PROVENANCE.md MUST also include a
license-evidence row in `## Source and license evidence` naming the immutable commit, the SPDX
identifier, and the URL of the license file at that commit. Hashes MUST be refreshed whenever the
materialized bytes change. The record MUST exclude entries for skills whose status is still blocked.

#### Scenario: New Clerk skill gets a materialized row + SHA-256 line

- **WHEN** a Clerk skill is committed and its frontmatter passes REQ-SKILLREC-003
- **THEN** PROVENANCE.md MUST contain a `Materialized entries` row for `<local-id>`
- **AND** MUST contain a `- '<local-id>/SKILL.md>': '<sha256>'` line whose digest matches
  `shasum -a 256 skills/<local-id>/SKILL.md`

#### Scenario: Companion references get SHA-256 lines

- **WHEN** a committed skill declares body-linked companions under `references/`
- **THEN** PROVENANCE.md MUST contain one SHA-256 line per companion file
- **AND** `validate_provenance.py` MUST exit 0 against the pinned commit

#### Scenario: Clerk license evidence row appears when design confirms

- **WHEN** design phase confirms Clerk MIT license + maintainer permission at an immutable commit
- **THEN** PROVENANCE.md MUST include a license-evidence row naming the commit, SPDX ID, and
  license-file URL
- **AND** the row MUST live under `## Source and license evidence`

### REQ-SKILLREC-005 — Chained PR Delivery Strategy

**Capability**: Governance
**Description**: `sdd-apply` MUST deliver this change as chained PRs because the 400-line PR
review budget cannot absorb workflow + 11 PROVENANCE rows + 11 SKILL.md frontmatter patches +
integration test blocks + validator hardening in a single PR. The forecast `Decision needed before
apply: Yes`, `Chained PRs recommended: Yes`, `400-line budget risk: High` (from proposal §Review
Workload Forecast) is resolved to chained. **PR 1 = CI workflow fix alone** (catalog-e2e.yml
sibling checkout + env var + the new validate_provenance.py RED-then-GREEN test, no skill
commits). **PR 2+ = content groups** in `dallay/agents-skills` (DB family in one PR, Clerk family
in chained sub-PRs once REQ-SKILLREC-009 design gate resolves, optionally `angular-architecture` /
`typescript-strict-patterns` only if REQ-SKILLREC-008 design gate resolves). Each PR MUST have a
clear start, clear finish, autonomous verification, and a single rollback unit. No PR may bundle
workflow changes with content commits.

#### Scenario: PR 1 contains only workflow + validator hardening

- **WHEN** `sdd-apply` opens PR 1
- **THEN** the diff MUST be limited to `.github/workflows/catalog-e2e.yml`,
  `agents-skills/scripts/validate_provenance.py`, and the RED-then-GREEN test
- **AND** the diff MUST NOT touch any `skills/*/SKILL.md` or `PROVENANCE.md`

#### Scenario: Subsequent PRs are content-only and grouped

- **WHEN** `sdd-apply` opens PR 2 and beyond
- **THEN** each PR MUST touch only `agents-skills/skills/<group>/*` and the matching
  `PROVENANCE.md` rows
- **AND** each PR MUST pass the new validator and any per-skill focused tests before merge

### REQ-SKILLREC-006 — Issue Hygiene

**Capability**: Governance
**Description**: After the workflow PR (PR 1) merges, the orchestrator MUST post a duplicate-close
comment on `dallay/agentsync#555` and a progress comment on `dallay/agentsync#556` linking to this
change's verify-report. The `dallay/agentsync#555` close comment text MUST be exactly: "Duplicate
of #556 — closing." The `dallay/agentsync#556` progress comment MUST reference Phase 1 status,
note the workflow fix landing, and link to the verify-report artifact. Neither action is gated on
the full Phase 1 content landing; the workflow fix is sufficient to mark meaningful progress.

#### Scenario: Issue #555 is closed as duplicate

- **WHEN** PR 1 (workflow fix) merges
- **THEN** a comment with text "Duplicate of #556 — closing." MUST be posted on issue #555
- **AND** the issue MUST transition to `closed`

#### Scenario: Issue #556 receives a progress comment

- **WHEN** PR 1 merges
- **THEN** a comment MUST be posted on issue #556 summarizing: workflow fix landed, remaining
  content groups queued behind chained PRs, and a link to this change's verify-report
- **AND** the issue MUST remain `open` until the success criteria in its body are fully met

### REQ-SKILLREC-007 — Scope Isolation from PR #569

**Capability**: Governance
**Description**: This change MUST NOT edit `src/skills/catalog.v1.toml`. Catalog additions, removals,
or remaps for Clerk / external entries are PR #569's territory. This change is limited to: (a) the
CI workflow file, (b) the provenance validator and its test, (c) the PROVENANCE.md Materialized
table (lives in `agents-skills`, not `agents-sync`), and (d) Phase 1 SKILL.md frontmatter patches +
companion vendoring inside `dallay/agents-skills`. The fail-closed guard at
`src/skills/provider.rs:244-250` MUST remain unchanged in this change (no edits to
`PHASE1_MIGRATED_LOCAL_SKILL_IDS`).

#### Scenario: catalog.v1.toml has zero diff lines

- **WHEN** any PR in the chained sequence opens
- **THEN** `git diff -- 'src/skills/catalog.v1.toml'` MUST be empty
- **AND** `cargo test --test test_catalog_integration phase1_bobmatnyc_...` MUST still pass using
  the existing three DB entries

#### Scenario: provider.rs fail-closed contract is preserved

- **WHEN** any PR in the chained sequence opens
- **THEN** `PHASE1_MIGRATED_LOCAL_SKILL_IDS` MUST contain exactly `["drizzle-orm", "pydantic",
  "sqlalchemy"]`
- **AND** the fail-closed guard at `src/skills/provider.rs:244-250` MUST be unchanged

## Removed Requirements

None. The archived Phase1 delta requirements (Approved Local Curated Source Resolution, Catalog
Source Updates Preserve Local IDs, Companion and Provenance Gates, Focused Phase 1 Installation
Validation, Full-Catalog E2E Early Return Is Preserved) remain valid and are extended rather than
removed. Their archive into `openspec/specs/skill-recommendations/spec.md` will happen in the
sdd-archive phase for this change.

## Open Requirements

### REQ-SKILLREC-008 — Disposition of `angular-architecture` and `typescript-strict-patterns`

**Capability**: Governance
**Status**: Awaiting design confirmation
**Description**: If design confirms that neither skill has an authoritative upstream source at an
immutable commit SHA, the apply phase MUST commit each one with
`metadata.author = "dallay-team"` and `metadata.source = "dallay-original"`, MUST add a
`Materialized entries` row + SHA-256 lines to `PROVENANCE.md` naming them as `dallay-original`,
and MUST extend the focused integration test with one assertion block per skill using the
sibling-resolution path. If design confirms an authoritative upstream exists at an immutable
SHA, the apply phase MUST instead set `metadata.source = <upstream>` and
`metadata.source_commit = <40-hex SHA>`, plus a license-evidence row in PROVENANCE.md. If design
cannot reach either conclusion, the apply phase MUST refuse to commit either skill and MUST
document the gap in this change's verify-report.

**Design question**: Does `angular-architecture` and/or `typescript-strict-patterns` have an
authoritative upstream commit, or are they declared `dallay-original`? If upstream, what is the
immutable commit SHA and license evidence?

### REQ-SKILLREC-009 — Clerk Upstream License Evidence Resolution

**Capability**: Provenance
**Status**: Awaiting design confirmation
**Description**: The apply phase MUST follow exactly one of three branches based on the design
phase's finding on Clerk upstream license + maintainer permission evidence at an immutable commit
on `clerk/skills`:

- **(a) Commit with evidence.** Design confirms authoritative MIT license + maintainer permission
  at an immutable commit SHA. Apply patches all eight Clerk SKILL.md frontmatters with
  `metadata.source = "clerk/skills"` + `metadata.source_commit = <SHA>`, vendors required companion
  references, extends PROVENANCE.md with `Materialized entries` rows + SHA-256 lines + a Clerk
  license-evidence row, extends the focused integration test with one assertion block per Clerk
  ID.
- **(b) Keep external.** Design confirms Clerk evidence but does not want to commit to the mirror
  in this change. Out of scope here; the design phase opens a follow-up issue documenting the
  catalog.v1.toml mapping handoff to PR #569's territory.
- **(c) Refuse and document gap.** Design cannot reach either conclusion. Apply MUST NOT commit
  any Clerk SKILL.md, MUST keep all nine Clerk skills as untracked worktree entries (matching the
  current state), MUST extend PROVENANCE.md with an explicit "Clerk blocked pending upstream
  evidence" note, and MUST document the gap in the verify-report. The PR #1 workflow change and
  REQ-SKILLREC-003 / REQ-SKILLREC-004 hardening still ship.

**Design question**: Is there authoritative MIT license + maintainer permission evidence for
`clerk/skills` at an immutable commit, and what is that commit SHA?

## Cross-References

- Proposal: `openspec/changes/phase1-e2e-mirror-catalog/proposal.md`
- Exploration: `openspec/changes/phase1-e2e-mirror-catalog/exploration.md`
- Archived reference: `openspec/changes/archive/2026-08-23-migrate-catalog-skills-phase1/`
- Issue #556: https://github.com/dallay/agentsync/issues/556
- Issue #555: https://github.com/dallay/agentsync/issues/555
- PR #569: https://github.com/dallay/agentsync/pull/569
- CI workflow: `.github/workflows/catalog-e2e.yml`
- Resolver: `src/skills/provider.rs:173-260`
- Focused test: `tests/test_catalog_integration.rs:66-131`
- Validator: `agents-skills/scripts/validate_provenance.py`
- Provenance record: `agents-skills/PROVENANCE.md`

## Open Questions

- **Q1 — clerk-orgs scope**: RESOLVED — out of scope for this change; deferred to a follow-up.
  REQ-SKILLREC-001 lists the eight Clerk IDs from issue #556 only.
- **Q2 — PR #569 merge order**: RESOLVED — scope-isolate; do NOT touch
  `src/skills/catalog.v1.toml` in this change. Rely on PR #569 to land external cleanup
  separately. Codified in REQ-SKILLREC-007.
- **Q3 — angular-architecture / typescript-strict-patterns disposition**: DEFERRED TO DESIGN —
  codify as `## Open Requirements` REQ-SKILLREC-008 with two resolution branches and a refusal
  fallback.
- **Q4 — Clerk upstream license + maintainer permission evidence**: DEFERRED TO DESIGN — codify
  as `## Open Requirements` REQ-SKILLREC-009 with three resolution branches (commit with
  evidence / keep external / refuse and document).
- **Q5 — Delivery strategy**: RESOLVED — chained PRs. PR 1 = workflow fix alone; PR 2+ = content
  groups per skill family. Codified in REQ-SKILLREC-005.