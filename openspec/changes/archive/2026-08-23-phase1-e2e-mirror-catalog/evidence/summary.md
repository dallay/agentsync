# Audit Summary

## Per-skill verdict

| Local ID | Verdict | Upstream | License | SHA | Notes |
|---|---|---|---|---|---|
| `clerk-setup` | **DEFER** | `clerk/skills@main` exists; bytes diverge (local v2.3.0 vs upstream v2.5.0) | Per-file `license: MIT` only; **no repo-level LICENSE** | local `2357d8a7...` ≠ upstream `737561a9...` | License-evidence gap + version drift. Need older upstream commit to byte-pin. |
| `clerk-nextjs-patterns` | **DEFER** | `clerk/skills@main` exact match | Per-file `license: MIT` only; **no repo-level LICENSE** | local `51a833e5...` = upstream `51a833e5...` | Bytes clean. Need SPDX (LICENSE file) + vendoring 5 missing companion refs. |
| `clerk-react-patterns` | **DEFER** | `clerk/skills@main` exact match | Per-file `license: MIT` only; **no repo-level LICENSE** | local `ab22780d...` = upstream `ab22780d...` | Bytes clean. Need SPDX + vendoring 4 missing companion refs. |
| `clerk-vue-patterns` | **DEFER** | `clerk/skills@main` exact match | Per-file `license: MIT` only; **no repo-level LICENSE** | local `626fca49...` = upstream `626fca49...` | Bytes clean. Need SPDX + vendoring 3 missing companion refs. |
| `clerk-astro-patterns` | **DEFER** | `clerk/skills@main` exact match | Per-file `license: MIT` only; **no repo-level LICENSE** | local `483df69c...` = upstream `483df69c...` | Bytes clean. Need SPDX + vendoring 5 missing companion refs. |
| `clerk-webhooks` | **DEFER** | `clerk/skills@main` exact match | Per-file `license: MIT` only; **no repo-level LICENSE** | local `99138f2f...` = upstream `99138f2f...` | Bytes clean. Need SPDX + vendoring `references/frameworks.md`. |
| `clerk-testing` | **DEFER** | `clerk/skills@main` exact match | Per-file `license: MIT` only; **no repo-level LICENSE** | local `0db14edb...` = upstream `0db14edb...` | Bytes clean. Need SPDX. No companion refs to vendor. |
| `clerk-custom-ui` | **DEFER** | `clerk/skills@main` exact match | Per-file `license: MIT` only; **no repo-level LICENSE** | local `16398a39...` = upstream `16398a39...` | Bytes clean. Need SPDX + 5 missing companion refs in `core-2/` + `core-3/` (may not exist upstream). |
| `angular-architecture` | **DEFER** (→ `dallay-original`) | No upstream candidate (untracked, no git history) | `license: MIT` claimed; no `metadata.author`, no `metadata.source` | local `bf3930a0...` | Can ship as `dallay-original` if Yuniel confirms authorship. Pending user gate. |
| `typescript-strict-patterns` | **DEFER** (→ `dallay-original`) | No upstream candidate (untracked, no git history) | `license: MIT` claimed; no `metadata.author`, no `metadata.source` | local `edc05081...` | Can ship as `dallay-original` if Yuniel confirms authorship. Pending user gate. |

## Counts

- **ACCEPT-WITH-EVIDENCE**: 0
- **DEFER**: 10 (all 10 skills)
- **DROP**: 0

## Disposition for the design

**All 10 candidate skills DEFER.** Two different gating questions are at play:

1. **Clerk (8 skills):** Bytes are byte-identical to upstream `clerk/skills@main` HEAD (`aac39ed99f18...`) for 7 of 8, with only `clerk-setup` drifted (older version). The blocker is **license evidence**: the upstream repo has no top-level `LICENSE`, `LICENSE.md`, `COPYING`, or `NOTICE` file, and `gh api repos/clerk/skills` returns `license: null`. Per-file `license: MIT` declarations in YAML frontmatter are not authoritative SPDX. REQ-SKILLREC-009 branch (a) "Commit with evidence" requires authoritative MIT license + maintainer permission at an immutable commit; that bar is NOT met. Branch (c) "Refuse and document gap" applies: all 9 Clerk skills (including the 9th, `clerk-orgs`, which is OUT of scope per Q1) remain untracked worktree entries pending upstream SPDX resolution or an explicit Clerk re-distribution grant.

2. **dallay-original (2 skills):** No upstream candidate, no git history, frontmatter lacks `metadata.author` and `metadata.source`. REQ-SKILLREC-008 branch (a) allows shipping as `dallay-original` if the author is confirmed. The orchestrator persona and skill style strongly suggest Yuniel authorship, but this is unverified — explicit user confirmation is required before commit.

The chained PR plan therefore collapses to:

- **PR 1 (the only commit in this change):** CI workflow fix + `validate_provenance.py` frontmatter gate (REQ-SKILLREC-002, REQ-SKILLREC-003, REQ-SKILLREC-004 partial, REQ-SKILLREC-006, REQ-SKILLREC-007). Roughly 50–80 LOC. Green CI on the existing 3 DB skills.
- **PR 2 (dallay-original, gated):** Optional, only if Yuniel confirms authorship of `angular-architecture` and `typescript-strict-patterns`. Patches frontmatter, adds 2 PROVENANCE rows + SHA-256 lines, ~10 LOC.
- **PR 3+ (Clerk content):** DEFERRED to a follow-up change. Documented as a gap in the verify-report. Triggered by either (a) Clerk adding a top-level LICENSE to `clerk/skills`, or (b) a written re-distribution grant. The follow-up will also need to vendor 24+ companion `references/*.md` files across 7 of the 8 skills, which alone is well over the 400-line PR budget.

**Decision needed before apply: Yes** (Yuniel authorship confirmation + Clerk SPDX path).
**Chained PRs recommended: Yes** (PR 1 is governance; PR 2 is gated dallay-original; PR 3+ is Clerk after upstream resolution).
**400-line budget risk: Low for PR 1**; High if Clerk content is forced into this change.
