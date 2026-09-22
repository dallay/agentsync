# Exploration — phase1-e2e-mirror-catalog

## Problem statement

The scheduled CI job **"Deterministic offline catalog E2E"** in `.github/workflows/catalog-e2e.yml`
fails because `tests/test_catalog_integration.rs::phase1_bobmatnyc_catalog_entries_install_offline_and_register_local_ids`
panics at line 106 with `curated local source is missing for drizzle-orm (dallay/agents-skills/drizzle-orm);
refusing external fallback`. The test expects a sibling `agents-skills` checkout visible at runtime, but
the workflow job has no `actions/checkout` step for the sibling repo and no `AGENTSYNC_LOCAL_SKILLS_REPO`
override, so `local_catalog_skill_source_dir()` always returns `None` and the fail-closed guard at
`src/skills/provider.rs:244-250` trips. The companion `verify-report.md` for the previous SDD
(`migrate-catalog-skills-phase1`, archived 2026-08-23) reported PASS for this exact test on a local dev
workstation where `/Users/acosta/Dev/dallay/agents-skills/` happens to live next to the agentsync checkout;
that result did not translate to CI.

## Current state — agentsync

- **Catalog & resolver contract**:
  - `src/skills/catalog.v1.toml:466-470` declares the Phase 1 entry
    `provider_skill_id = "dallay/agents-skills/drizzle-orm"` / `local_skill_id = "drizzle-orm"`.
  - `src/skills/catalog.v1.toml:864-871` declares the matching `pydantic` and `sqlalchemy` entries.
  - `src/skills/catalog.rs:16` defines `LOCAL_EMBEDDED_SKILL_PREFIX = "dallay/agents-skills/"`, which is
    whitelisted as a local-curated recommendation source (lines 702-746).
  - `src/skills/catalog.rs:53-67` lists `clerk/skills/clerk-*` IDs in `APPROVED_EMBEDDED_EXTERNAL_SKILL_IDS`
    — Clerk still has an external allowlist; nothing under `dallay/agents-skills/clerk-*` exists yet.
  - `src/skills/provider.rs:173` hard-codes `PHASE1_MIGRATED_LOCAL_SKILL_IDS = ["drizzle-orm", "pydantic", "sqlalchemy"]`
    (note: defined in `provider.rs`, not `registry.rs` as the brief assumed). Lines 244-250 raise the
    "refusing external fallback" panic when the source is missing for one of those IDs.
  - `src/skills/provider.rs:186-225` `local_catalog_skill_source_dir` resolves the source in this order:
    `AGENTSYNC_TEST_SKILL_SOURCE_DIR` (with provider_skill_id then local_skill_id) →
    `AGENTSYNC_LOCAL_SKILLS_REPO/skills/<local_id>` → `<project_root_parent>/agents-skills/skills/<local_id>`.
- **Test surface**:
  - `tests/test_catalog_integration.rs:66-131` `phase1_bobmatnyc_catalog_entries_install_offline_and_register_local_ids`
    asserts, for each of `drizzle-orm`, `pydantic`, `sqlalchemy`: catalog definition present, install
    source resolves to a directory, SKILL.md + companion reference files exist, and `registry.json`
    contains a canonical key.
  - `tests/test_catalog_integration.rs:133-219` `every_catalog_skill_installs_successfully` is `#[ignore]` AND
    contains an explicit early return (line 146) that prints the known-issue skip message; the previous SDD
    left both barriers in place.
- **Workflow**:
  - `.github/workflows/catalog-e2e.yml:16-29` `offline` job runs
    `cargo test --test test_catalog_integration --locked --offline -- --nocapture` with **no sibling checkout**
    and **no `AGENTSYNC_LOCAL_SKILLS_REPO` env var**. This is the actual failure surface.
  - `.github/workflows/catalog-e2e.yml:31-64` `catalog-installation` job also has no sibling checkout and
    no `AGENTSYNC_LOCAL_SKILLS_REPO`; it runs `RUN_E2E=1 cargo test -- --ignored --nocapture`.
  - `.github/workflows/ci.yml:150` and `sonarcloud.yml:42` already export
    `AGENTSYNC_LOCAL_SKILLS_REPO: ${{ github.workspace }}/agents-skills` (used by other tests, not catalog-e2e).
  - `README.md:119` documents the dev override: `export AGENTSYNC_LOCAL_SKILLS_REPO="$(pwd)/../agents-skills"`.
- **Sibling resolution mechanism**:
  - Three modes are supported: test-source dir, `AGENTSYNC_LOCAL_SKILLS_REPO`, and sibling
    `../agents-skills`. All three resolve a `skills/<local_id>` directory; none clone or fetch the
    sibling automatically.
  - `src/skills/registry.v1.toml` and `registry.lock.toml` already exist with two curated entries
    (`accessibility`, `docker-expert`) pointing at commit `17db5a394732ae569392cf871c52d7df88de3a2d`. They are
    a separate path used by `PinnedProvider`; the `phase1_bobmatnyc_*` test does NOT exercise them.
- **Existing change artifact**: `openspec/changes/migrate-catalog-skills-phase1/` (not archived yet;
    same content exists at `openspec/changes/archive/2026-08-23-migrate-catalog-skills-phase1/`). The
    archived `verify-report.md` says "PASS WITH WARNINGS" and the previous `qa-report.md` (passing)
    cover the same Phase 1 subset on a local workstation only.

## Current state — agents-skills

**Branch state** (`feat/phase1-auth-db-skills`, last commit `c2e79fb fix(skills): harden curated database guidance`):

- 3 already-committed skills on the branch (drizzle-orm, pydantic, sqlalchemy) with `metadata.source`
  and `metadata.source_commit` pointing at `bobmatnyc/claude-mpm-skills@718070a7d622921b01687799a1f9613f36c6f615`.
  Each has `references/` populated; PROVENANCE.md lists 10 SHA-256 entries and an explicit Materialized table.
- Untracked worktree additions: `scripts/validate_provenance.py` (new), modifications to
  `PROVENANCE.md` and `scripts/validate-skills.sh`, and 11 candidate skill dirs (9 clerk + angular-architecture
  + typescript-strict-patterns).

**Provenance audit of the 9 uncommitted Clerk skills + 2 extras** (per
`/Users/acosta/Dev/dallay/agents-skills/skills/<id>/SKILL.md`):

| Skill | Bytes | Frontmatter | `metadata.source` | `metadata.source_commit` | License | `metadata.author` | Companion refs | Quality (subjective) | Verdict |
|---|---:|---|---|---|---|---|---|---|---|
| `clerk-astro-patterns` | 3305 | ✅ | ❌ missing | ❌ missing | `MIT` (claimed) | `clerk` | refers to `references/*.md` (none present) | production-ready prose, but vendor-frontmatter only | **needs provenance patched or rejected** |
| `clerk-custom-ui` | 6494 | ✅ | ❌ missing | ❌ missing | `MIT` (claimed) | `clerk` | refers to `core-2/`, `core-3/` (none present) | production-ready prose, but vendor-frontmatter only | **needs provenance patched or rejected** |
| `clerk-nextjs-patterns` | 8302 | ✅ | ❌ missing | ❌ missing | `MIT` (claimed) | `clerk` | refers to `references/*.md` (none present) | production-ready prose | **needs provenance patched or rejected** |
| `clerk-orgs` | 19230 | ✅ | ❌ missing | ❌ missing | `MIT` (claimed) | `clerk` | refers to `references/*.md` (none present) | production-ready prose, comprehensive | **needs provenance patched or rejected** |
| `clerk-react-patterns` | 4162 | ✅ | ❌ missing | ❌ missing | `MIT` (claimed) | `clerk` | refers to `references/*.md` (none present) | production-ready prose | **needs provenance patched or rejected** |
| `clerk-setup` | 13042 | ✅ | ❌ missing | ❌ missing | `MIT` (claimed) | `clerk` | none | production-ready, references itself | **needs provenance patched or rejected** |
| `clerk-testing` | 1931 | ✅ | ❌ missing | ❌ missing | `MIT` (claimed) | `clerk` | none | short but coherent | **needs provenance patched or rejected** |
| `clerk-vue-patterns` | 2833 | ✅ | ❌ missing | ❌ missing | `MIT` (claimed) | `clerk` | refers to `references/*.md` (none present) | production-ready prose | **needs provenance patched or rejected** |
| `clerk-webhooks` | 13562 | ✅ | ❌ missing | ❌ missing | `MIT` (claimed) | `clerk` | refers to `references/frameworks.md` (none present) | production-ready prose | **needs provenance patched or rejected** |
| `angular-architecture` | 7577 | ✅ | ❌ missing | ❌ missing | `MIT` (claimed) | not set | none | original-feeling content (Yuniel's voice); no source provenance at all | **needs author/source attribution before commit** |
| `typescript-strict-patterns` | 5273 | ✅ | ❌ missing | ❌ missing | `MIT` (claimed) | not set | none | original-feeling content (Yuniel's voice); no source provenance at all | **needs author/source attribution before commit** |

**Validation tooling** (`/Users/acosta/Dev/dallay/agents-skills/scripts/`):

- `validate-skills.sh` — shells out to a `skills-ref` binary (`scripts/install-skills.sh`) to validate each
  `skills/*/SKILL.md`, then runs `validate_provenance.py` and `validate_skills.py`. NOT source-provenance-aware
  — it only checks manifest shape, body length, and `Use when` cue in the description.
- `validate_provenance.py` — parses the `^- '<path>': '<sha256>'` lines in `PROVENANCE.md`, recomputes SHA-256
  for each `skills/<path>`, and reports mismatches. Will fail if `PROVENANCE.md` adds entries pointing at
  the Clerk/Angular/TypeScript files without their bytes existing yet, but is otherwise content-agnostic.
- Neither script validates that `metadata.source` / `metadata.source_commit` are present in the SKILL.md
  frontmatter. This check would need to be added before any Clerk commit.

**PROVENANCE.md current state**: 52 lines, explicitly excludes Clerk/Angular/TypeScript: *"no Clerk,
Angular, or TypeScript candidate is included in this provenance record or migration scope"*. Any change
to add Clerk would require a parallel `Materialized entries` row, an immutable commit pin, MIT evidence
record, and file hashes for the new entries.

## Failure root cause

`tests/test_catalog_integration.rs::phase1_bobmatnyc_catalog_entries_install_offline_and_register_local_ids`
calls `resolve_catalog_install_source(catalog, provider, "dallay/agents-skills/drizzle-orm", "drizzle-orm",
Some(project_root))` (`tests/test_catalog_integration.rs:99-106`). Inside
`src/skills/provider.rs:227-253`, the sibling path is checked at lines 211-219
(`<project_root_parent>/agents-skills/skills/<id>`). On the CI runner that path does not exist because
`.github/workflows/catalog-e2e.yml:16-29` never checks out a sibling repo and never sets
`AGENTSYNC_LOCAL_SKILLS_REPO`. Resolution falls through to the panic guard at
`src/skills/provider.rs:244-250` because `drizzle-orm` is in `PHASE1_MIGRATED_LOCAL_SKILL_IDS` (line 173).
The local dev workstation path is masked because the previous SDD's verification was run with
`AGENTSYNC_SOURCE_REPO=../agents-skills` (acceptance harness) but the CI workflow file was never updated
to mirror that.

## Issue/PR landscape

- **Issue #556 / DALLAY-581** (`dallay/agentsync`, OPEN, `enhancement`/`chore`/`jules` labels):
  - Title: *"Catalog cleanup: 56 broken upstream skill entries"*.
  - Body describes 56 broken upstream entries across Clerk, Cloudflare, Deno, Flutter, .NET, Vercel, etc.
  - First comment by `@yacosta738` (2026-08-12): proposes the curated-repo migration strategy with four
    phases. Phase 1 lists exactly **8** clerk skills (`clerk-setup`, `clerk-nextjs-patterns`,
    `clerk-react-patterns`, `clerk-vue-patterns`, `clerk-astro-patterns`, `clerk-webhooks`,
    `clerk-testing`, `clerk-custom-ui`) plus drizzle-orm, pydantic, sqlalchemy = **11 total**.
  - Success criteria (verbatim): "Phase 1 complete (11 skills migrated); Catalog updated to point to
    agents-skills; Test `every_catalog_skill_installs_successfully` passes; Remove early-return from test;
    Close this issue."
  - Jules bot left a "Ready for review!" comment linking to PR #569.
- **Issue #555 / DALLAY-580**: empty body except the Linear linkback comment — confirmed duplicate / placeholder.
- **PR #569** (`dallay/agentsync`, OPEN, `CHANGES_REQUESTED`, `+81 / -311`, base=main, head=
  `fix/catalog-cleanup-broken-upstream-skills-14159881112161993279`, opened 2026-08-18 by
  `google-labs-jules`): *"Catalog cleanup: audit and resolve broken upstream skill entries"*. CodeRabbit
  walkthrough describes catalog integrity cleanup (remap/remove/skip) and re-enabling the full-catalog
  E2E. **Not** the same work as the new Phase 1 mirror — it operates on the external catalog entries
  rather than on the local mirror.
- **Other open PRs**: only `app/renovate` bot PRs (#571 rust docker tag, #570 iconify-json, #568
  rust-toolchain, #566 github-actions, #564 astro) plus #552 release-please. None conflict with this change.

## Proposed scope (high level only — no design decisions yet)

In `agents-skills`: (1) decide whether the 11 uncommitted skill dirs get committed in this change or
remain out-of-scope. (2) If in-scope, patch every committed SKILL.md frontmatter to add
`metadata.source: clerk/skills` (or original upstream) and `metadata.source_commit: <immutable SHA>`,
plus ship companion `references/` files the body actually links to. (3) Extend `PROVENANCE.md` with one
`Materialized entries` row and SHA-256 lines per new skill. (4) Confirm whether the two extras
(`angular-architecture`, `typescript-strict-patterns`) get author/source attribution or are removed
from the worktree entirely.

In `agents-sync`: (1) **fix the CI workflow** — either add a sibling `actions/checkout` of
`dallay/agents-skills` plus `AGENTSYNC_LOCAL_SKILLS_REPO` env var on the `offline` and
`catalog-installation` jobs, OR replace the catalog-e2e trigger with a local-fixture version. (2) Only if
Phase 1 catalog expansion is approved: extend `catalog.v1.toml` with the new provider IDs, add
`PHASE1_MIGRATED_LOCAL_SKILL_IDS` entries, and add focused integration tests per new skill (the current
catalog-e2e test only asserts the 3 already-migrated skills). (3) Decide whether to populate
`registry.v1.toml`/`registry.lock.toml` for the new skills under the verified curated install path or
keep them on the legacy sibling resolution path.

## Risks and open questions

- **Skill count inconsistency**: issue #556 Phase 1 says **8** clerk skills; the worktree on
  `feat/phase1-auth-db-skills` has **9** clerk skills (extra: `clerk-orgs`). Orchestrator's prompt says
  "11 Phase 1 skills declared in issue #556 (8 clerk + drizzle + pydantic + sqlalchemy)" but also
  lists `clerk-orgs` on disk. Need explicit clarification on whether Phase 1 includes `clerk-orgs`
  (effective total: 9 clerk + 3 db = 12).
- **Provenance gap on all 11 candidates**: zero Clerk skills and neither extra carry `metadata.source`
  or `metadata.source_commit`. The previous SDD's `verify-report.md` and `PROVENANCE.md` both flagged
  this as a hard blocker for Clerk. No upstream immutable commit is currently pinned for any of the
  9 clerk candidates. Migrating any of them without authoritative MIT evidence or maintainer
  permission is a licensing risk.
- **Missing companion references**: every Clerk SKILL.md body references `references/*.md` (or
  `core-2/`, `core-3/`) files that do not exist on disk. Migrating SKILL.md-only would leave skills
  functionally incomplete and would also require vendoring 8+ companion files per skill (the upstream
  Clerk repo at `clerk/skills` has them under `skills/core/`, `skills/features/`,
  `skills/frameworks/`).
- **CI failure may be unrelated to migration**: the deterministic failure is a missing
  `AGENTSYNC_LOCAL_SKILLS_REPO` / sibling checkout in `.github/workflows/catalog-e2e.yml`. The previous
  SDD's "PASS" was local-only and never exercised this workflow. Migrating 11 more skills would not by
  itself fix the failing `offline` job — that needs a workflow fix.
- **PR #569 collision risk**: PR #569 (open, CHANGES_REQUESTED, +81/-311, addresses the broader 56-entry
  audit) may touch the same `catalog.v1.toml` lines and test files. If merged before this change, the
  baseline shifts; if merged after, expect conflicts.
- **Registry manifest extension**: `registry.v1.toml`/`registry.lock.toml` currently only cover 2
  skills. Adding verified-curated entries for any new skill requires pinned commit, subpath, manifest
  metadata, SHA-256 hash, SPDX evidence, and approved validation — significantly more per-skill work
  than the legacy sibling resolution path.
- **400-line PR budget**: per the review workload guard, a change touching 11 catalog entries, 11
  workflow steps, 11 integration test cases, plus 11 PROVENANCE.md hash lines will likely exceed 400
  LOC. Forecast must decide between chained PRs (recommended) or a size exception before apply.

## Next recommended step

Move to **proposal phase** — present the orchestrator with the two-axis decision
(CI-workflow-only vs. CI-workflow-plus-catalog-expansion; 8-clerk vs. 9-clerk scope) and the
licensing/provenance gate on Clerk before any spec design begins.
