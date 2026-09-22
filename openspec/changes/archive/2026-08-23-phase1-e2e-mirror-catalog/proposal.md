# Proposal — phase1-e2e-mirror-catalog

## Why

The scheduled CI job **"Deterministic offline catalog E2E"** in `.github/workflows/catalog-e2e.yml`
fails on every run because the `offline` and `catalog-installation` jobs never check out the sibling
`dallay/agents-skills` repo and never export `AGENTSYNC_LOCAL_SKILLS_REPO`. As a result,
`local_catalog_skill_source_dir()` returns `None` on the runner, and
`tests/test_catalog_integration.rs::phase1_bobmatnyc_catalog_entries_install_offline_and_register_local_ids`
trips the fail-closed guard at `src/skills/provider.rs:244-250` with
*"curated local source is missing for drizzle-orm; refusing external fallback"*.

The previous SDD (`migrate-catalog-skills-phase1`, archived 2026-08-23) reported PASS WITH WARNINGS,
but that PASS came from a local dev workstation where `/Users/acosta/Dev/dallay/agents-skills/`
happens to live next to the agentsync checkout. The acceptance harness happened to set
`AGENTSYNC_SOURCE_REPO=../agents-skills`, so the missing-CI-workflow-fix regression slipped through
the verification gate. The companion `ci.yml:150` + `ci.yml:161-167` already exports
`AGENTSYNC_LOCAL_SKILLS_REPO` and checks out `dallay/agents-skills` at commit
`c2e79fbb72d146305f82a8e979270795557d24fd` — `catalog-e2e.yml` simply does not match.

CI is the only deterministic guard preventing reinstall of broken or revoked external skills.
Fixing the offline gate is a *governance* fix as much as a CI fix: every Phase 1 migration without
it is unverifiable end-to-end on a clean runner, so the same drift that produced issue #556
(DALLAY-581) can recur silently.

The user has accepted "Camino Lento" — ~1 week of curated work — to extend the Phase 1 mirror from
3 Bobmatnyc DB skills (already done, archived) to the full **11 skills** declared in issue #556
(8 clerk + drizzle-orm + pydantic + sqlalchemy), plus audit-and-migrate the two extras on the
working branch (`angular-architecture`, `typescript-strict-patterns`) if and only if they have a
verifiable upstream source or are declared original. Every migrated skill lands with
`metadata.source` + `metadata.source_commit` (immutable SHA) or `metadata.author = "dallay-team"`
+ `metadata.source = "dallay-original"` with explicit PROVENANCE.md justification. Provenance
enforcement is tightened in `validate_provenance.py` so the next drift is caught locally.

PR #569 (Jules, OPEN, +81/-311, CodeRabbit CHANGES_REQUESTED) operates on the **external** catalog
entries; it does not touch the curated mirror. Catalog.v1.toml lines and any Phase 1 catalog
expansion must be scope-isolated to avoid blocking this change behind #569. Issue #555 is an empty
placeholder duplicating #556 and must be closed as part of this change.

## What changes

### agents-sync CI workflow (governance-critical, single PR)

| File | Change |
|---|---|
| `.github/workflows/catalog-e2e.yml` | Add `actions/checkout` of `dallay/agents-skills` (pinned to the latest validated commit, currently `c2e79fbb72d146305f82a8e979270795557d24fd`) at path `agents-skills` on both `offline` and `catalog-installation` jobs. Add `AGENTSYNC_LOCAL_SKILLS_REPO: ${{ github.workspace }}/agents-skills` env on both jobs. Pin SHA, not branch. |

### agents-sync catalog + tests

| File | Change | Notes |
|---|---|---|
| `src/skills/catalog.v1.toml` | Skip if **fully covered by PR #569**; otherwise add `dallay/agents-skills/clerk-*` (8 ids per issue #556) entries preserving local IDs. | Only touch if PR #569 leaves the curated slots untouched. |
| `src/skills/provider.rs` | Add new local IDs to `PHASE1_MIGRATED_LOCAL_SKILL_IDS` (line 173) after the catalog wires them. | Mirrors the existing fail-closed contract; do not weaken it. |
| `tests/test_catalog_integration.rs` | Re-enable `every_catalog_skill_installs_successfully` only if the migrated set covers the whole catalog surface; otherwise extend the focused test with one assertion block per new local ID and keep the early-return guard. | TDD-first: RED assertion for each new skill, GREEN after fixture present. |
| `tests/test_bug.rs`, `tests/unit/suggest_catalog.rs`, `tests/unit/provider.rs` | Add focused coverage for new IDs (catalog entry, sibling resolution, install, registry key). | Mirrors archived Phase 1 test shape. |

### agents-skills content (curated migration)

11 candidates from issue #556 + 2 audited extras from the worktree. Per-skill disposition chosen by
spec phase after each is verified individually.

| Local ID | Source / Status | Disposition | Frontmatter shape |
|---|---|---|---|
| `drizzle-orm` | bobmatnyc/claude-mpm-skills @ 718070a7 | Already committed (last migration). | `metadata.source` + `source_commit` ✅ |
| `pydantic` | bobmatnyc/claude-mpm-skills @ 718070a7 | Already committed. | same ✅ |
| `sqlalchemy` | bobmatnyc/claude-mpm-skills @ 718070a7 | Already committed. | same ✅ |
| `clerk-setup` | clerk/skills at pinned immutable SHA | New commit + companion refs vendored. | upstream + source_commit |
| `clerk-nextjs-patterns` | clerk/skills at pinned SHA | New commit + companions. | upstream + source_commit |
| `clerk-react-patterns` | clerk/skills at pinned SHA | New commit + companions. | upstream + source_commit |
| `clerk-vue-patterns` | clerk/skills at pinned SHA | New commit + companions. | upstream + source_commit |
| `clerk-astro-patterns` | clerk/skills at pinned SHA | New commit + companions. | upstream + source_commit |
| `clerk-webhooks` | clerk/skills at pinned SHA | New commit + companions (`references/frameworks.md`). | upstream + source_commit |
| `clerk-testing` | clerk/skills at pinned SHA | New commit + (no body-linked companions). | upstream + source_commit |
| `clerk-custom-ui` | clerk/skills at pinned SHA | New commit + `core-2/` + `core-3/` companions vendored. | upstream + source_commit |
| `clerk-orgs` ⚠ | TBD | **Scope question — see Open Questions.** Likely deferred to a follow-up because issue #556 lists 8, not 9. | — |
| `angular-architecture` ⚠ | Yuniel-original? | Audit: declare `metadata.author = "dallay-team"`, `metadata.source = "dallay-original"` if no upstream; otherwise refuse. | author + dallay-original **or** upstream + commit |
| `typescript-strict-patterns` ⚠ | Yuniel-original? | Same audit gate as `angular-architecture`. | same |

⚠ = requires gating decision in spec phase.

### agents-skills governance

| File | Change |
|---|---|
| `PROVENANCE.md` | Add a `Materialized entries` row and per-file SHA-256 entries for each new committed skill. Refresh the 10 existing SHA-256 entries if any touch them (e.g., frontmatter normalization). Add Clerk license evidence row and immutable commit pin. |
| `scripts/validate_provenance.py` | Add a frontmatter check that rejects any `skills/*/SKILL.md` lacking `metadata.source` and (when source ≠ `dallay-original`) `metadata.source_commit`. Fail loudly; non-zero exit. |
| `scripts/validate-skills.sh` | No contract change; the new frontmatter check runs as part of the existing wrapper. |

### Issue hygiene

| Issue / PR | Action |
|---|---|
| `dallay/agentsync#555` | Comment "Duplicate of #556 — closing." and close. |
| `dallay/agentsync#556` | Add a progress comment linking to this change's verify-report and noting #555 was closed as duplicate. |
| `dallay/agentsync#569` | Do **not** block on it. Verify PR scope first; if it touches the curated slots, scope-isolate this change to avoid blocking. |
| `dallay/agents-skills` | Open a PR per chained work unit (see Acceptance Criteria § 3). |

## Out of scope

- Phase 2/3/4 skills from issue #556 (Cloudflare, Deno, Flutter, ML/data).
- Full PR #569 review (Jules' external-catalog cleanup) — tracked separately.
- Re-enabling `every_catalog_skill_installs_successfully` for non-curated entries.
- Registry manifest (`src/skills/registry.v1.toml` / `registry.lock.toml`) extension — the legacy
  sibling resolution path is sufficient for Phase 1.
- Content edits to existing unrelated skills on the branch.
- Production code changes outside the catalog resolver and CI workflow fix.

## Impact

- **Downstream users**: every recommended skill from the 11+2 set installs offline from a
  deterministic local mirror; no network dependency on third-party repo stability.
- **CI**: `catalog-e2e.yml` offline job becomes green; the fail-closed contract at
  `provider.rs:244-250` is preserved (does not weaken into silent network fallback).
- **License / provenance**: every shipped skill carries either an immutable upstream commit + MIT
  evidence (Clerk, Bobmatnyc) or a declared original authorship (Angular/TypeScript, if audited).
  No third-party content enters the catalog without an auditable SHA.
- **Risk**: medium-high — a Clerk license-evidence gap remains a hard blocker; see Open Questions.

## Acceptance criteria

1. `.github/workflows/catalog-e2e.yml` runs `phase1_bobmatnyc_catalog_entries_install_offline_and_register_local_ids`
   green on a fresh runner with no local overrides — sibling checkout + `AGENTSYNC_LOCAL_SKILLS_REPO`
   are sufficient.
2. Every catalog skill from the **8 clerk + drizzle-orm + pydantic + sqlalchemy** set installs
   successfully from the local mirror via `provider.resolve()` returning a directory source.
3. `PROVENANCE.md` contains a `Materialized entries` row and SHA-256 line per newly committed
   skill; `python3 scripts/validate_provenance.py` exits 0 against the pinned commit.
4. `validate_provenance.py` rejects any `skills/<id>/SKILL.md` that lacks `metadata.source` (and,
   unless `source = "dallay-original"`, `metadata.source_commit`) — demonstrated by a RED test.
5. `cargo test --test test_catalog_integration` passes 100% — the focused subset test plus any new
   per-skill assertion blocks.
6. `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check`
   clean (project's pre-push contract).
7. `dallay/agentsync#555` is closed with a "Duplicate of #556" comment.
8. `dallay/agentsync#556` has a progress comment linking to the verify-report.
9. Either PR #569 is merged first and the catalog.v1.toml edits are skipped here, or PR #569 is
   scope-isolated so neither change blocks the other.
10. **Delivery strategy**: forecast expects > 400 changed lines (catalog + 11×PROVENANCE rows +
    11×frontmatter patches + workflow fix + new tests). Forecast must resolve to either
    chained/stacked PRs or an explicit `size:exception` decision before `sdd-apply` starts.

## Open questions

1. **clerk-orgs scope**: issue #556's body lists **8** clerk skills; the worktree on
   `feat/phase1-auth-db-skills` holds **9** (extra: `clerk-orgs`). The orchestrator's prompt lists
   8 clerk. Is `clerk-orgs` in or out? Default assumption: **out**, deferred to a follow-up.
2. **PR #569 merge order**: scope-isolate the catalog.v1.toml edits here (preferred: don't touch
   `catalog.v1.toml` at all and rely on PR #569 to land); or wait for #569 merge? Default:
   scope-isolate, do NOT touch catalog.v1.toml.
3. **`angular-architecture` / `typescript-strict-patterns` disposition**: Yuniel's voice suggests
   original authorship. If no upstream exists at an immutable SHA, declare
   `author = "dallay-team"`, `source = "dallay-original"`. Confirm with Yuniel before committing
   either one (license claim is currently unverifiable).
4. **Clerk upstream pinning**: is an authoritative MIT license + maintainer permission documented
   for `clerk/skills` at a specific immutable commit? If not, Clerk entries may shift from
   "migrate-with-evidence" to "keep external but rebuild the catalog slot to non-Clerk source".
5. **Delivery strategy resolution**: confirm before apply whether the change resolves to chained
   PRs (recommended: PR 1 = CI workflow fix alone; PR 2 = agents-skills content per skill group)
   or to a `size:exception` single PR.

## Next phase

`/sdd-spec` must produce per-skill delta specs and confirm the disposition for `clerk-orgs`,
`angular-architecture`, `typescript-strict-patterns`, and the Clerk upstream pinning question
**before** any implementation lands in `sdd-apply`.

## Review workload forecast

| Field | Value |
|---|---|
| Estimated changed lines | 1100–1600 (workflow fix + 11 PROVENANCE rows + 11 SKILL.md patches + 11 test blocks + catalog edits if isolated) |
| 400-line budget risk | **High** |
| Chained PRs recommended | **Yes** |
| Decision needed before apply | **Yes** — chained PRs vs `size:exception` (Open Question 5) |

Decision needed before apply: Yes
Chained PRs recommended: Yes
400-line budget risk: High

## Rollback plan

Revert the `actions/checkout` sibling step and `AGENTSYNC_LOCAL_SKILLS_REPO` env (workflow file),
revert catalog additions and `PHASE1_MIGRATED_LOCAL_SKILL_IDS` extension (provider.rs + tests),
and revert the contents of any merged `dallay/agents-skills` PR by reverting to the previously
pinned commit. The fail-closed guard at `provider.rs:244-250` stays in place at every step, so no
intermediate state silently falls back to network resolution. Do not touch installed user
directories or `registry.json`.

## Dependencies

- A pinned immutable commit on `dallay/agents-skills` containing the new committed skills.
- Authoritative MIT license + maintainer evidence for `clerk/skills` at the chosen commit, **or**
  a documented decision to leave Clerk external in this phase.
- Confirmation on `angular-architecture` / `typescript-strict-patterns` authorship before either
  is committed.
- PR #569 status: scope-isolate or wait-for-merge (Open Question 2).
