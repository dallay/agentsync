# Apply Report: migrate-catalog-skills-phase1

## Delivery

- Strategy: `single-pr` with explicit user-approved `size-exception`.
- Branch: `feat/migrate-catalog-skills-phase1`; base: `main`.
- Scope: only the three MIT-backed Bobmatnyc entries `drizzle-orm`, `pydantic`, and `sqlalchemy`.
- Sibling source: `agents-skills` merge commit `be4570aa6f23931f51692661d163ddd663b57488` (PR #18)
  contains the three migrated skill directories and `PROVENANCE.md`; CI checks out this committed
  revision into `agents-skills` for deterministic resolution.

## Completed

- Added the three canonical sibling skill directories under `agents-skills/skills/`, including all
  source-declared reference files and explicit full-source companions for the long Pydantic and
  SQLAlchemy documents. These directories are committed in the `agents-skills` PR #18 merge; the
  provenance record pins the upstream immutable source and target file hashes.
- Added `agents-skills/PROVENANCE.md` with immutable repository commit, source paths, Git blob/tree
  identities, attribution, MIT license evidence, materialized file inventory, and companion status.
- Remapped only the three catalog definitions and their Drizzle/Pydantic/SQLAlchemy technology
  references to `dallay/agents-skills/<local-id>`, removing mutable external install sources. The
  Wispbit SQLAlchemy recommendation and all Clerk mappings remain external.
- Added fail-closed behavior for missing curated `dallay/agents-skills/*` content, while preserving
  local override precedence.
- Threaded the real project root through direct/update and suggestion install resolution.
- Added focused resolver, catalog-boundary, caller-propagation, offline install, companion, and
  registry-key tests.
- Preserved the ignored full-catalog E2E early return because unrelated external entries remain
  broken.

## QA Remediation Handoff (2026-08-14)

- Added `tests/acceptance/phase1_catalog.sh`, a narrow black-box harness that launches an external
  `agentsync` executable, copies only `drizzle-orm`, `pydantic`, and `sqlalchemy` into temporary
  source fixtures, and asserts local installation, companions, registry keys, override precedence,
  fail-closed missing-source behavior, and direct/suggestion project-root propagation.
- Documented the explicit target as
  `make acceptance-phase1 AGENTSYNC_BIN=target/release/agentsync AGENTSYNC_SOURCE_REPO=../agents-skills`.
  The harness fails clearly when the release-like binary is absent and never calls Rust modules
  directly. The full-catalog early return remains untouched.
- Refreshed the eight stale materialized hashes in the merged `agents-skills` Phase 1 provenance
  record and added `scripts/validate_provenance.py`, wired into `scripts/validate-skills.sh`, so
  future materialized-byte drift is caught by a focused check. Run
  `AGENTSYNC_SOURCE_REPO=../agents-skills tests/acceptance/test_phase1_provenance.sh` to invoke it.
- TDD evidence: the missing-binary harness contract failed before the harness existed and passed
  after implementation; provenance validation first failed on the eight stale hashes and passed
  after the refresh.
- No production source, catalog, registry manifest, full-catalog test, or unrelated untracked
  sibling candidates were changed. QA remains `NOT TESTED`; `sdd-qa` must rerun against the
  documented external target before any acceptance or archive decision.

## Verification

- Target sibling validator: pinned `skills-ref validate` passed for all three migrated directories
  (`PYTHONPATH=.tools/skills-ref/lib/python3.14/site-packages python3.14 .tools/skills-ref/bin/skills-ref validate ...`).
- Repository-wide target validator remains expected to report pre-existing Clerk activation-cue
  failures; those files were not modified.
- Focused Rust tests are recorded in the orchestrator return summary.

## Risks

- The explicit size exception accepts a high forecasted review workload in one PR; no chain was used.
- Full catalog E2E is still intentionally not green and remains ignored/early-returned.
- Curated entrypoints normalize unsupported upstream frontmatter fields and preserve the full source
  body in companions; future registry/hash hardening should pin the resulting sibling commit.
- `agents-skills` still contains unrelated pre-existing untracked Clerk, Angular, and TypeScript files;
  they are intentionally excluded from this change.
