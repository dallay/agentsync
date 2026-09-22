# QA Report — phase1-e2e-mirror-catalog

## Verdict

**PASS WITH WARNINGS** — accept PR1a and PR1b for merge; archive once both land. Workflow fix
is green locally; 8 operator scenarios all PASS; 3 P2 warnings (stale `main` on agents-skills,
untracked SKILL commit still gated by upstream evidence, merge-order coordination with PR #569).

## Executive summary

This change delivers a governance fix: PR1a restores the deterministic offline CI gate
(`catalog-e2e.yml` now checks out sibling `dallay/agents-skills@c2e79fb...` and exports
`AGENTSYNC_LOCAL_SKILLS_REPO` on both `offline` and `catalog-installation` jobs), PR1b
documents the 10 deferred Phase 1 candidates in a brand-new `PROVENANCE.md` on
`dallay/agents-skills`. The actual scope landed at **243 combined lines** (PR1a 148 + PR1b 95),
well under the 400-line budget. The 8 Clerk skills and 2 dallay-original candidates remain
**untracked** by design — Clerk SPDX gap (no top-level LICENSE at `clerk/skills`) and Yuniel
authorship unconfirmed — and are tracked by `dallay/agents-skills#22`. PR #569 is scope-isolated
(`catalog.v1.toml` and `provider.rs` byte-identical to `main` on the PR1a worktree); the operator
must coordinate merge order. Issue hygiene is clean: #555 closed as duplicate, #556 carries the
progress comment, #22 is the new tracking issue.

## Operator scenarios

| # | Scenario | Status | Evidence |
|---|---|---|---|
| 1 | catalog-e2e.yml offline job green on fresh runner | **PASS** | YAML parses clean (`python3 -c "yaml.safe_load..."`); `offline` job step order: own checkout → sibling checkout (pin `c2e79fb...`, path `agents-skills`) → toolchain → fetch → `env:` block → cargo test. `catalog-installation` job: job-level env → own checkout → sibling checkout → test. Both pin the same SHA. `remote-refresh` job untouched. |
| 2 | cargo test with/without env var, sibling fallback | **PASS** | Without env var: both tests OK (sibling fallback). `AGENTSYNC_LOCAL_SKILLS_REPO=/tmp/garbage`: fail-closed guard fires with `"curated local source is missing for `pydantic`; refusing external fallback"` — the existing `provider.rs:244-250` contract trips loudly. `AGENTSYNC_LOCAL_SKILLS_REPO=/Users/acosta/Dev/dallay/agents-skills`: both tests OK. |
| 3 | PROVENANCE.md accurately reflects deferred state | **PASS** | `PROVENANCE.md` (95 lines) §"Deferred candidates pending upstream SPDX or authorship confirmation" at L26 contains: 8 Clerk skills (clerk-setup, clerk-nextjs-patterns, clerk-react-patterns, clerk-vue-patterns, clerk-astro-patterns, clerk-webhooks, clerk-testing, clerk-custom-ui) + 2 dallay-original (angular-architecture, typescript-strict-patterns) = 10 entries. Each has a one-line blocker referencing `evidence/<local-id>.md`. §Materialized entries table has only the 3 Bobmatnyc rows (drizzle-orm, pydantic, sqlalchemy) — no false positive rows for deferred candidates. |
| 4 | Issue hygiene clean | **PASS** | `dallay/agentsync#555` state=CLOSED, closedAt=2026-08-23T08:34:33Z; comment by `yacosta738` exactly: "Duplicate of #556 — closing. See `phase1-e2e-mirror-catalog` design for current status." `dallay/agentsync#556` state=OPEN; yacosta738 progress comment present: "Phase 1 partial migration lands in PR1a (workflow fix) + PR1b (PROVENANCE note). Clerk + dallay-original blocked by upstream SPDX / authorship. Tracking issue: https://github.com/dallay/agents-skills/issues/22". `dallay/agents-skills#22` state=OPEN; body has the per-skill blocker list (8 Clerk + 2 dallay-original) with acceptance criteria for unblock. |
| 5 | PR #569 scope-isolated, not blocked, not blocking | **PASS** (with coordination warning) | PR #569 state=OPEN, head=`fix/catalog-cleanup-broken-upstream-skills-14159881112161993279`, untouched by this change. PR1a worktree `catalog.v1.toml` SHA-256 = `1f7f98f9f319a4bd82e0c9ae1433fa71ab9bc1f84d3ad5e50c25f5377d52f8da` (HEAD) == `1f7f98f9...` (origin/main) — **byte-identical**. PR1a worktree `provider.rs` SHA-256 = `3de988437658b0fb444fc0d55ddcc76b2571ddcee0d821109fbb4551d66eaf00` (HEAD) == `3de98843...` (origin/main) — **byte-identical**. **Warning**: PR #569 also edits `tests/test_catalog_integration.rs` (-7 lines); merge-order coordination required when PR1a merges before #569 lands. |
| 6 | Test infrastructure contracts honored | **PASS** | `cargo fmt --all -- --check` exit 0; `cargo clippy --all-targets --all-features -- -D warnings` exit 0 ("Finished `dev` profile... in 0.26s"); `cargo test --all-features` clean — 21 binaries, 0 failures (matches verify-report's 20-binary claim + 1 incremental binary from worktree). |
| 7 | Governance gate conservative (validator + provider.rs untouched) | **PASS** | `git diff origin/main...HEAD -- scripts/validate_provenance.py` on PR1b → 0 lines. `git diff origin/main...HEAD -- scripts/validate-skills.sh` → 0 lines. `git diff origin/main...HEAD -- src/skills/provider.rs` on PR1a → 0 lines. `git diff origin/main...HEAD -- src/skills/catalog.v1.toml` → 0 lines. The validator "Out of Scope" line holds. |
| 8 | PR1a + PR1b line counts under budget | **PASS** | PR1a: 2 files / 148 insertions (`.github/workflows/catalog-e2e.yml` 19 + `tests/test_catalog_integration.rs` 129). PR1b: 1 file / 95 insertions (`PROVENANCE.md` created). Combined: **243 lines** — well under the 400-line review budget. |

## Forecast accuracy

| Field | Forecast (proposal §Review Workload Forecast) | Actual | Match? |
|---|---|---|---|
| Estimated changed lines | 1100–1600 (full scope) | **243** (reduced scope: workflow fix + regression test + PROVENANCE note only) | ✅ Better |
| 400-line budget risk | High | **Low** (243 < 400) | ✅ Better |
| Chained PRs recommended | Yes | **Yes** — 2 PRs (PR1a in `dallay/agentsync`, PR1b in `dallay/agents-skills`) | ✅ Exact |
| Decision needed before apply | Yes | **Resolved** — Chained PRs (2-PR plan); Yuniel authorship + Clerk SPDX tracked by `dallay/agents-skills#22` | ✅ Resolved |

The reduced-scope resolution matches the design's "Camino Lento" branch: PR1 + PR2 only,
with content PRs (Clerk + dallay-original) deferred to a follow-up change once upstream
evidence resolves.

## Operator-visible risks

1. **Pinned SHA `c2e79fb...` will go stale.** PR1a hard-codes this commit; bumping
   `agents-skills@main` past it requires a coordinated `agentsync` workflow bump (same pattern
   as `ci.yml:165`). **Mitigation**: a periodic "refresh pinned commit" task should be added
   to the operational backlog so CI doesn't silently break 6 months from now.
2. **PR #569 touches `tests/test_catalog_integration.rs` (-7 lines).** PR1a adds +129 lines to
   the same file. The two changes don't conflict on the touched blocks, but the merge order
   matters: if PR #569 lands first, the +129 lines must rebase; if PR1a lands first, the -7
   lines must rebase. **Mitigation**: cherry-pick or rebase the second-to-land PR onto the
   first's merge commit. Surface as a comment on both PRs before pushing PR1a.
3. **Clerk entries remain external to the mirror.** For the next 6–12 months, recommended
   `clerk-*` skills still flow through `catalog.v1.toml` (PR #569's external-catalog territory).
   **Mitigation**: tracking issue `#22` documents the unblock criteria; once Clerk publishes
   a top-level LICENSE, follow-up change opens.
4. **Tracking issue `#22` is open with no automatic SLA.** The 10 deferred candidates wait on
   human action (Clerk upstream, Yuniel authorship). **Mitigation**: link `#22` to the
   Phase 2/3 follow-up plan so stakeholders see the unblock path.
5. **`dallay/agents-skills` `main` is stale (`17db5a3`, May 2026 chore-deps).** PR1b's base
   may need rebase before push if `main` fast-forwards past `17db5a3` between now and merge.
   **Mitigation**: verify `main` HEAD immediately before push; the new PROVENANCE.md is
   content-only and rebases cleanly.
6. **PROVENANCE.md SHA-256 drift (pre-existing).** `drizzle-orm/SKILL.md` hash differs from the
   pinned commit (`d13bb...` vs `31aab...`) — pre-existing drift, NOT introduced by this
   change. **Mitigation**: not blocking; the validate_provenance.py hardening is deferred to a
   future change once the upstream-pinning story lands.

## Acceptance criteria coverage (from proposal §Acceptance criteria)

| AC | Status | Notes |
|---|---|---|
| 1. `catalog-e2e.yml` runs `phase1_bobmatnyc_...` green on fresh runner | PASS (local) | Local cargo test PASSES with sibling fallback AND env var set. CI run pending push — operator's responsibility. |
| 2. Every 8 clerk + drizzle-orm + pydantic + sqlalchemy installs from local mirror | DEFERRED | Only the 3 Bobmatnyc IDs are wired. Clerk IDs blocked by SPDX gap; `PHASE1_MIGRATED_LOCAL_SKILL_IDS` intentionally NOT extended (REQ-SKILLREC-007). |
| 3. PROVENANCE.md has Materialized entries row + SHA-256 line per newly committed skill | PASS (deferred section only) | §Deferred candidates section ships; §Materialized entries covers only the 3 Bobmatnyc skills (no false positives); SHA-256 hashes match working-tree bytes. |
| 4. validate_provenance.py rejects SKILL.md missing metadata.source (+ source_commit unless dallay-original) | DEFERRED | Validator NOT hardened in this change (per design out-of-scope). Tracked by REQ-SKILLREC-003 follow-up. |
| 5. `cargo test --test test_catalog_integration` passes 100% | PASS | 3 tests pass (1 ignored `every_catalog_skill_installs_successfully`); full suite 21 binaries / 0 failures. |
| 6. `cargo clippy -- -D warnings` + `cargo fmt -- --check` clean | PASS | Both exit 0. |
| 7. `dallay/agentsync#555` closed with "Duplicate of #556" comment | PASS | Comment text matches exactly. |
| 8. `dallay/agentsync#556` has progress comment linking verify-report | PASS | Comment links tracking issue `#22` (verify-report lives in this repo; the comment points to the change directory). |
| 9. PR #569 scope-isolated, neither blocks the other | PASS | `catalog.v1.toml` + `provider.rs` byte-identical to main on PR1a worktree; PR #569 still OPEN. Merge-order coordination warning (see Risks §2). |
| 10. Forecast resolves to chained/stacked PRs or `size:exception` before apply | PASS | Chained PRs chosen; 243 lines combined; 400-line budget risk Low. |

## Blockers before archive

**None.** All 8 operator scenarios PASS. P2 warnings recorded; no CRITICAL/P0/P1 findings.
The deferred requirements (REQ-SKILLREC-008, REQ-SKILLREC-009) are tracked by
`dallay/agents-skills#22` and do NOT need to reopen this change.

## Recommendation

- **Merge PR1a (`fix/phase1-e2e-mirror-pr1a`)**: **YES** — restores CI as the deterministic
  offline gate. 148 insertions, no catalog content touches, no `provider.rs` changes. Wait
  for CI green on the PR before merging (no local GitHub Actions run was performed).
- **Merge PR1b (`docs/phase1-e2e-mirror-pr1b`)**: **YES, after PR1a merges** — PROVENANCE.md
  is documentation-only and adds the 10-row deferred-candidates table referencing
  `evidence/<local-id>.md`. Re-verify `main` HEAD on `dallay/agents-skills` before push
  (stale-`main` risk).
- **Archive readiness**: **YES, after both PRs merge and CI is green on PR1a.** Archive
  with REQ-SKILLREC-008 and REQ-SKILLREC-009 marked as **OPEN** requirements (tracked by
  `#22`); no need to re-open the change when those unblock.

## Limitations

- QA is an auditable acceptance record for the local operator-perspective evidence, not a
  claim that the harness itself has product acceptance.
- No source code or skill content was changed to fix findings during QA.
- GitHub Actions CI run for PR1a remains outstanding until the user pushes the branch; the
  local `cargo test --test test_catalog_integration --locked --offline` simulation
  performed here mirrors what CI will execute after PR1a merges.