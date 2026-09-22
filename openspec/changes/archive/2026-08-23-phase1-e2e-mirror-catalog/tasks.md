# Tasks — phase1-e2e-mirror-catalog

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~70 (PR1a: ~60 workflow + test; PR1b: ~10 PROVENANCE note) |
| 400-line budget risk | Low |
| Chained PRs recommended | Yes |
| Suggested split | PR1a (`agentsync`) → PR1b (`agents-skills`) → tracking issue |
| Delivery strategy | ask-on-risk (resolved to chained, no size exception) |
| Chain strategy | github-stacked-prs |
| Decision needed before apply | No — chained PRs, all gates resolved (Clerk/dallay-original deferred) |

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: github-stacked-prs
400-line budget risk: Low

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| PR1a | Restore CI as deterministic offline gate for Phase 1 catalog resolution (workflow + RED test) | PR1a (`dallay/agentsync`) | base: `main`; no `catalog.v1.toml` or `provider.rs` edits |
| PR1b | Document that 10 deferred candidates remain untracked pending SPDX / authorship resolution | PR1b (`dallay/agents-skills`) | base: PR1a branch (stacked); PROVENANCE.md note only, no skill commits |
| Issue | Tracking issue for Clerk SPDX + dallay-original authorship gaps | Issue (no PR) | `dallay/agents-skills` repo |

## Delivery plan

**Chained PRs + tracking issue**:

- **PR1a (`dallay/agentsync`)** — Workflow fix + regression test (Group 1 + Group 2 + Group 5 partial + Group 6.1-6.2/6.5-6.6). base: `main`. Files: `.github/workflows/catalog-e2e.yml`, `tests/test_catalog_integration.rs`. No content commits. Satisfies **REQ-SKILLREC-002**, **REQ-SKILLREC-007**.
- **PR1b (`dallay/agents-skills`)** — PROVENANCE.md note about deferred candidates (Group 3). base: PR1a branch (stacked). Files: `PROVENANCE.md`. No skill commits. Satisfies **REQ-SKILLREC-004** (deferred-section only).
- **Issue (no PR)** — SPDX / authorship tracking issue in `dallay/agents-skills` (Group 4.3). Closes #555 (Group 4.1-4.2), updates #556 (Group 4.4). References **REQ-SKILLREC-008** and **REQ-SKILLREC-009**.

**Cross-cutting**: **REQ-SKILLREC-005** (chained PR strategy), **REQ-SKILLREC-006** (issue hygiene), **REQ-SKILLREC-007** (PR #569 scope isolation — never touch `catalog.v1.toml`).

---

## Task groups

### Group 1 — Regression test (TDD-RED)
**Goal**: Add a failing assertion to the existing focused Phase 1 test that captures the workflow env-var contract so future regressions are caught at unit-test level, not just CI. Satisfies **REQ-SKILLREC-001** (Bobmatnyc scenario), **REQ-SKILLREC-002** (env-var contract).

- [ ] 1.1 Read `tests/test_catalog_integration.rs` lines 60-110 to confirm the current shape of `phase1_bobmatnyc_catalog_entries_install_offline_and_register_local_ids` and the `resolve_catalog_install_source` call at line 99-106.
- [ ] 1.2 Add RED assertion: when `AGENTSYNC_LOCAL_SKILLS_REPO` is unset AND a sibling `../agents-skills` exists, the resolver still finds `drizzle-orm`, `pydantic`, `sqlalchemy`. Confirm RED via `cargo test --test test_catalog_integration phase1_bobmatnyc` on the local dev workstation.
- [ ] 1.3 Add RED assertion: when `AGENTSYNC_LOCAL_SKILLS_REPO` is set to a custom path containing `skills/`, the resolver uses it (not the sibling). Confirm RED via the same test invocation with the env var set.

### Group 2 — Workflow fix (TDD-GREEN)
**Goal**: Make the regression test pass on a clean runner by configuring the workflow correctly. Pin to commit `c2e79fbb72d146305f82a8e979270795557d24fd` — matches the existing `ci.yml:165` reference. Satisfies **REQ-SKILLREC-002**.

- [ ] 2.1 Read `.github/workflows/catalog-e2e.yml` and `.github/workflows/ci.yml:150-167` to confirm the existing checkout pattern (job structure, env promotion style, step ordering).
- [ ] 2.2 Edit `.github/workflows/catalog-e2e.yml`: add `actions/checkout` of `dallay/agents-skills@c2e79fbb72d146305f82a8e979270795557d24fd` to path `agents-skills` on both `offline` and `catalog-installation` jobs. Add `AGENTSYNC_LOCAL_SKILLS_REPO: ${{ github.workspace }}/agents-skills` env on both jobs. Do NOT touch `remote-refresh` (out of scope per **REQ-SKILLREC-002**).
- [ ] 2.3 Confirm GREEN locally: re-run `cargo test --test test_catalog_integration phase1_bobmatnyc` with `AGENTSYNC_LOCAL_SKILLS_REPO` set to the local `agents-skills` checkout — should pass.

### Group 3 — agents-skills PROVENANCE note
**Goal**: Document that the 10 deferred candidates (8 Clerk + 2 dallay-original) remain untracked pending upstream SPDX / authorship resolution. No skill commits. Satisfies **REQ-SKILLREC-004** (deferred-section only — no SHA-256 rows added).

- [ ] 3.1 Edit `dallay/agents-skills/PROVENANCE.md`: add a section after the existing "Materialized entries" titled "Deferred candidates pending upstream SPDX or authorship confirmation" listing all 10 skills with one-line blocker each, referencing `openspec/changes/phase1-e2e-mirror-catalog/evidence/*.md` as the audit record.
- [ ] 3.2 Verify no new commits to `skills/` directory: `git -C /Users/acosta/Dev/dallay/agents-skills status` shows the 10 untracked skills remain untracked. Confirm RED contract: `validate_provenance.py` (unchanged, existing 82-line version) still exits 0 — the new PROVENANCE note is a comment, not a hash entry.

### Group 4 — Issue hygiene
**Goal**: Close the duplicate, open the tracking issue, update the parent issue. Satisfies **REQ-SKILLREC-006**.

- [ ] 4.1 Comment on `dallay/agentsync#555` with text exactly: "Duplicate of #556 — closing. See `phase1-e2e-mirror-catalog` design for current status."
- [ ] 4.2 Close `dallay/agentsync#555` with reason "duplicate".
- [ ] 4.3 Open new issue in `dallay/agents-skills` titled: "Block Phase 1 migration of Clerk + dallay-original candidates — SPDX / authorship gap". Body must include: (a) list of all 10 skills with per-skill blocker (Clerk 8: missing repo LICENSE file at `clerk/skills`; dallay-original 2: unverified authorship), (b) link to `openspec/changes/phase1-e2e-mirror-catalog/evidence/summary.md`, (c) acceptance criteria for unblock (Clerk: SPDX identifier OR maintainer grant; dallay-original: authorship confirmation).
- [ ] 4.4 Comment on `dallay/agentsync#556` with progress: "Phase 1 partial migration lands in PR1a (workflow fix) + PR1b (PROVENANCE note). Clerk + dallay-original blocked by upstream SPDX / authorship. Tracking issue: <link to 4.3>."

### Group 5 — Verification (PR1a ready gate)
**Goal**: Run the full quality gate before declaring PR1a ready. Satisfies **REQ-SKILLREC-005**, **REQ-SKILLREC-007**.

- [ ] 5.1 `cargo fmt --all -- --check` — clean.
- [ ] 5.2 `cargo clippy --all-targets --all-features -- -D warnings` — clean.
- [ ] 5.3 `cargo test --test test_catalog_integration` — 100% green locally with `AGENTSYNC_LOCAL_SKILLS_REPO` set.
- [ ] 5.4 `cargo test --all-features` — full suite green.
- [ ] 5.5 Verify `git diff origin/main...HEAD -- 'src/skills/catalog.v1.toml'` is empty (**REQ-SKILLREC-007**).
- [ ] 5.6 Verify `git diff origin/main...HEAD -- 'src/skills/provider.rs' | grep -c PHASE1_MIGRATED_LOCAL_SKILL_IDS` is `0` (**REQ-SKILLREC-007** — fail-closed guard unchanged).

### Group 6 — Apply & merge
**Goal**: Open the chained PRs, verify CI green on PR1a, merge in order. **REQ-SKILLREC-005**.

- [ ] 6.1 Create worktree `phase1-e2e-mirror-pr1a` from `dallay/agentsync` `main`.
- [ ] 6.2 Commit + push + open PR1a on `dallay/agentsync` (workflow + test). Title: `fix(ci): checkout sibling agents-skills in catalog-e2e offline jobs`. Body references fixes #556, links `openspec/changes/phase1-e2e-mirror-catalog/design.md`, explicitly notes `catalog.v1.toml` untouched per **REQ-SKILLREC-007**.
- [ ] 6.3 Create worktree `phase1-e2e-mirror-pr1b` from `dallay/agents-skills` `main`.
- [ ] 6.4 Commit + push + open PR1b on `dallay/agents-skills` (PROVENANCE.md only). Title: `docs(provenance): note deferred Phase 1 candidates pending SPDX / authorship`. Base branch: PR1a branch (stacked). Body links `evidence/summary.md`.
- [ ] 6.5 Wait for CI green on PR1a, request review.
- [ ] 6.6 After PR1a merges, verify `catalog-e2e.yml` shows green on `main` (next scheduled run or workflow_dispatch).
- [ ] 6.7 PR1b can merge independently once stacked base is merged.

---

## Out-of-scope tasks (NOT in this change, tracked elsewhere)

- Future change **"phase1-clerk-spdx-resolution"** — gated on Clerk SPDX or maintainer grant. Opens when tracking issue from Task 4.3 is closed. Will commit 8 Clerk SKILL.md frontmatter patches + 24+ companion refs + PROVENANCE rows.
- Future change **"phase1-dallay-original-resolution"** — gated on authorship confirmation for `angular-architecture` + `typescript-strict-patterns`. Will patch frontmatter and add 2 PROVENANCE rows.
- Future change **"phase1-clerk-orgs-scope"** — gate decision on whether `clerk-orgs` belongs in Phase 1 (Q1 resolved: out).
- PR #569 review/merge — independent track, owned by Jules. Scope-isolated per **REQ-SKILLREC-007**.
- Future `agents-skills/scripts/validate_provenance.py` frontmatter hardening (REQ-SKILLREC-003) — deferred because the 10 candidates remain untracked and the existing SHA-256 check is sufficient for the 3 committed Bobmatnyc skills.

## Forecast

- Total LOC: ~60 (workflow + test) + ~10 (PROVENANCE note) + 0 skill commits
- Tasks: 22 checkbox items across 6 groups (Groups 1-6)
- PRs: 2 chained (PR1a agentsync + PR1b agents-skills) + 1 tracking issue
- Risk: low (no catalog content touches, no `provider.rs` changes, no skill commits)
- Critical path: PR1a workflow → verify CI green on main → PR1b PROVENANCE note → close #555
