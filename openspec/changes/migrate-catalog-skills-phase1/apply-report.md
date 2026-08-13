# Apply Report: migrate-catalog-skills-phase1

## Delivery

- Strategy: `single-pr` with explicit user-approved `size-exception`.
- Branch: `feat/migrate-catalog-skills-phase1`; base: `main`.
- Scope: only the three MIT-backed Bobmatnyc entries `drizzle-orm`, `pydantic`, and `sqlalchemy`.
- Sibling source: `agents-skills` commit `c2e79fbb72d146305f82a8e979270795557d24fd` (PR #18)
  contains the three migrated skill directories and `PROVENANCE.md`; CI checks out this committed
  revision into `agents-skills` for deterministic resolution.

## Completed

- Added the three canonical sibling skill directories under `agents-skills/skills/`, including all
  source-declared reference files and explicit full-source companions for the long Pydantic and
  SQLAlchemy documents. These directories are committed in `agents-skills` commit `70298da` (PR
  #18); the provenance record pins the upstream immutable source and target file hashes.
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
