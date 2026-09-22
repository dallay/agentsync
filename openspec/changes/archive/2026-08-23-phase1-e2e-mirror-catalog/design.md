# Design — phase1-e2e-mirror-catalog

## Approach

This change restores CI as the deterministic offline gate for Phase 1 catalog resolution and hardens
the upstream provenance contract so future drift fails locally. The architecture is **governance-first**:
PR 1 contains only the CI workflow fix (`.github/workflows/catalog-e2e.yml` sibling checkout +
`AGENTSYNC_LOCAL_SKILLS_REPO` env var) and the provenance validator hardening
(`scripts/validate_provenance.py` frontmatter gate). Chained PRs are required because the 400-line
review budget cannot absorb workflow + content in a single diff, and the upstream audit performed
in this phase revealed that **none of the 10 candidate skills currently meets the REQ-SKILLREC-009 /
REQ-SKILLREC-008 evidence bar** for immediate acceptance. The 8 Clerk skills are byte-identical to
upstream `clerk/skills@main` HEAD (`aac39ed99f18...`) for 7 of 8, but the upstream repo has no
top-level LICENSE file — per-file frontmatter `license: MIT` is not authoritative SPDX. The 2
`dallay-original` candidates (`angular-architecture`, `typescript-strict-patterns`) have no upstream
candidate and no `metadata.author` in frontmatter. Both REQ-SKILLREC-008 and REQ-SKILLREC-009 are
therefore resolved as **DEFER** with explicit follow-up actions, not ACCEPT. The sibling repo
checkout at commit `c2e79fbb72d146305f82a8e979270795557d24fd` matches the existing pattern in
`ci.yml:161-167`. This design scope-isolates from PR #569 (Jules' external catalog cleanup) by
never touching `src/skills/catalog.v1.toml`, never editing `PHASE1_MIGRATED_LOCAL_SKILL_IDS`, and
never removing the early-return guards in `every_catalog_skill_installs_successfully`.

## Architecture

### Resolution order (`src/skills/provider.rs:186-253`)

```
catalog entry (provider_skill_id = "dallay/agents-skills/drizzle-orm", local_skill_id = "drizzle-orm")
        │
        ▼
local_catalog_skill_source_dir(provider_skill_id, local_skill_id, project_root)
        │
        ├─1─► AGENTSYNC_TEST_SKILL_SOURCE_DIR/provider_skill_id (test fixture path)
        │
        ├─2─► AGENTSYNC_TEST_SKILL_SOURCE_DIR/local_skill_id      (test fixture fallback)
        │
        ├─3─► AGENTSYNC_LOCAL_SKILLS_REPO/skills/<local_skill_id>  ◄── NEW CI env var
        │
        └─4─► <project_root_parent>/agents-skills/skills/<local_skill_id>  (dev sibling)
                │
                ▼ exists? → return Ok(path)
                ▼ none   → continue to fail-closed guard
PHASE1_MIGRATED_LOCAL_SKILL_IDS.contains(local_skill_id)?
        │
        ├─ YES → anyhow::bail!("curated local source is missing for {id}")
        │
        └─ NO  → provider.resolve(provider_skill_id)  (network fallback)
```

Confirmed via `src/skills/provider.rs:186-260`: the resolver checks `AGENTSYNC_LOCAL_SKILLS_REPO`
*before* the sibling-parent fallback, so a CI env var wins over dev sibling. The fail-closed guard
at lines 244-250 fires for every ID in `PHASE1_MIGRATED_LOCAL_SKILL_IDS` (currently the 3 DB
skills) — this change **does not** extend that allowlist (REQ-SKILLREC-007).

### CI workflow (`.github/workflows/catalog-e2e.yml`)

Concrete diff sketch for PR 1 — mirrors the `ci.yml:161-167` pattern, scoped to both `offline` and
`catalog-installation` jobs:

```yaml
jobs:
  offline:
    name: Deterministic offline catalog E2E
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
        with:
          persist-credentials: false
      - name: Checkout committed agents-skills source          # NEW
        uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
        with:
          repository: dallay/agents-skills
          ref: c2e79fbb72d146305f82a8e979270795557d24fd       # PIN, matches ci.yml:165
          path: agents-skills
          persist-credentials: false
      - uses: dtolnay/rust-toolchain@e97e2d8cc328f1b50210efc529dca0028893a2d9 # v1
        with:
          toolchain: stable
      - name: Fetch locked dependencies before offline test
        run: cargo fetch --locked
      - name: Run offline fixtures with network disabled
        env:                                                     # NEW block
          AGENTSYNC_LOCAL_SKILLS_REPO: ${{ github.workspace }}/agents-skills
        run: cargo test --test test_catalog_integration --locked --offline -- --nocapture

  catalog-installation:
    name: Verify catalog skill installation
    runs-on: ubuntu-latest
    timeout-minutes: 60
    env:                                                       # NEW — promote to job-level
      AGENTSYNC_LOCAL_SKILLS_REPO: ${{ github.workspace }}/agents-skills
    steps:
      # ... existing steps ...
      - name: Checkout committed agents-skills source          # NEW
        uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
        with:
          repository: dallay/agents-skills
          ref: c2e79fbb72d146305f82a8e979270795557d24fd
          path: agents-skills
          persist-credentials: false
```

Notes: the `env:` block goes on the step for `offline` (env promotion not needed) and at the
job-level for `catalog-installation` so the env reaches the cached cargo step too. The pin SHA
`c2e79fb...` matches the existing `ci.yml:165` reference; bumping it requires a coordinated bump in
both workflow files. `remote-refresh` job is **NOT** touched (it has its own pinned-commit contract
and is out of scope per REQ-SKILLREC-002).

### Provenance validator (`agents-skills/scripts/validate_provenance.py`)

Concrete diff sketch — adds a frontmatter precheck that runs **before** the existing SHA-256
validation (which already runs). The new check covers REQ-SKILLREC-003 and REQ-SKILLREC-004
frontmatter contract:

```python
import frontmatter  # already a transitive dep of validate-skills.sh

FRONTMATTER = re.compile(r"^---\n(?P<body>.*?)\n---\n", re.DOTALL)
SHA40 = re.compile(r"^[0-9a-f]{40}$")

def check_frontmatter(root: Path) -> list[str]:
    """REQ-SKILLREC-003 / REQ-SKILLREC-004: every SKILL.md needs source (+ commit
    unless source == 'dallay-original')."""
    failures: list[str] = []
    for skill_md in (root / "skills").rglob("SKILL.md"):
        rel = skill_md.relative_to(root)
        text = skill_md.read_text()
        match = FRONTMATTER.match(text)
        if not match:
            failures.append(f"{rel}: SKILL.md missing or malformed YAML frontmatter")
            continue
        try:
            meta = frontmatter.loads(text).metadata
        except Exception as exc:
            failures.append(f"{rel}: YAML parse error: {exc}")
            continue
        source = meta.get("metadata", {}).get("source") if isinstance(
            meta.get("metadata"), dict) else meta.get("source")
        if not source:
            failures.append(f"{rel}: missing metadata.source")
            continue
        if source != "dallay-original":
            commit = meta.get("metadata", {}).get("source_commit") if isinstance(
                meta.get("metadata"), dict) else meta.get("source_commit")
            if not commit or not SHA40.fullmatch(commit):
                failures.append(f"{rel}: missing or malformed metadata.source_commit "
                                f"(need 40-hex SHA; got {commit!r})")
    return failures

def validate(root: Path) -> list[str]:
    failures = check_frontmatter(root)                # NEW — runs first
    if failures:
        return failures                               # bail before SHA-256 step
    provenance_path = root / "PROVENANCE.md"
    # ... existing parse_hash_entries + SHA-256 logic unchanged ...
    return failures
```

Exit code contract:

- `0` — frontmatter OK + every SHA-256 in `PROVENANCE.md` matches.
- `1` — frontmatter gate failed (offender printed to stderr with path + missing/malformed field)
  **OR** SHA-256 mismatch (existing behavior).

**RED scenario reproduction (REQ-SKILLREC-003):**

1. Before PR 1, the existing `validate_provenance.py` exits 0 against any `skills/<id>/SKILL.md`
   because it does NOT inspect frontmatter. The Clerk candidates and the 2 dallay-original
   candidates all bypass the gate silently — that's the current bug.
2. After PR 1, the new `check_frontmatter()` rejects every Clerk SKILL.md because
   `metadata.source` is absent. Example stderr:
   ```
   [FAIL] skills/clerk-nextjs-patterns/SKILL.md: missing metadata.source
   ```
3. After PR 1, the new check ALSO rejects every dallay-original candidate until frontmatter is
   patched (RED → GREEN is the per-skill acceptance flow once those are accepted in PR 2+).
4. The three already-committed Bobmatnyc skills (`drizzle-orm`, `pydantic`, `sqlalchemy`) PASS
   because their frontmatter already declares `source: bobmatnyc/claude-mpm-skills` and a 40-hex
   `source_commit`.

The `scripts/validate-skills.sh` wrapper already invokes `validate_provenance.py` at line 29, so the
new gate runs on every CI invocation automatically.

## Per-skill disposition (REQ-SKILLREC-008 + REQ-SKILLREC-009 resolution)

| Local ID | Upstream candidate | Verdict | Evidence file | License | Commit SHA | Frontmatter patch needed |
|---|---|---|---|---|---|---|
| `clerk-setup` | `clerk/skills@main` | **DEFER** (version drift) | `evidence/clerk-setup.md` | per-file `MIT` only; no repo LICENSE | needs pre-`aac39ed9` commit (local v2.3.0 ≠ upstream v2.5.0) | needs `metadata.source: clerk/skills` + 40-hex `source_commit` |
| `clerk-nextjs-patterns` | `clerk/skills@main` | **DEFER** (license gap) | `evidence/clerk-nextjs-patterns.md` | per-file `MIT` only; no repo LICENSE | `aac39ed99f18...` (blob `7a2c0d7c...`) | needs `metadata.source: clerk/skills` + `source_commit: aac39ed99f18...` + 5 companion refs vendored |
| `clerk-react-patterns` | `clerk/skills@main` | **DEFER** (license gap) | `evidence/clerk-react-patterns.md` | per-file `MIT` only; no repo LICENSE | `aac39ed99f18...` (blob `84496131...`) | needs source + commit + 4 companion refs |
| `clerk-vue-patterns` | `clerk/skills@main` | **DEFER** (license gap) | `evidence/clerk-vue-patterns.md` | per-file `MIT` only; no repo LICENSE | `aac39ed99f18...` (blob `0109b3d7...`) | needs source + commit + 3 companion refs |
| `clerk-astro-patterns` | `clerk/skills@main` | **DEFER** (license gap) | `evidence/clerk-astro-patterns.md` | per-file `MIT` only; no repo LICENSE | `aac39ed99f18...` (blob `0e5f731e...`) | needs source + commit + 5 companion refs |
| `clerk-webhooks` | `clerk/skills@main` | **DEFER** (license gap) | `evidence/clerk-webhooks.md` | per-file `MIT` only; no repo LICENSE | `aac39ed99f18...` (blob `259f099f...`) | needs source + commit + 1 companion ref (`references/frameworks.md`) |
| `clerk-testing` | `clerk/skills@main` | **DEFER** (license gap) | `evidence/clerk-testing.md` | per-file `MIT` only; no repo LICENSE | `aac39ed99f18...` (blob `46b394e0...`) | needs source + commit (no companion refs) |
| `clerk-custom-ui` | `clerk/skills@main` | **DEFER** (license gap) | `evidence/clerk-custom-ui.md` | per-file `MIT` only; no repo LICENSE | `aac39ed99f18...` (blob `e6e05dc9...`) | needs source + commit + `core-2/` + `core-3/` companion refs |
| `angular-architecture` | none | **DEFER** (authorship unconfirmed → `dallay-original`) | `evidence/angular-architecture.md` | `license: MIT` claimed; no metadata.author | N/A | needs `metadata.author: dallay-team`, `metadata.source: dallay-original` (once Yuniel confirms) |
| `typescript-strict-patterns` | none | **DEFER** (authorship unconfirmed → `dallay-original`) | `evidence/typescript-strict-patterns.md` | `license: MIT` claimed; no metadata.author | N/A | needs `metadata.author: dallay-team`, `metadata.source: dallay-original` (once Yuniel confirms) |

**ACCEPT-WITH-EVIDENCE:** 0 | **DEFER:** 10 | **DROP:** 0

## Chained PR plan

| PR | Title | Repo | Files | LOC | Depends on |
|---|---|---|---|---|---|
| **PR 1** | `fix(ci): checkout sibling agents-skills + AGENTSYNC_LOCAL_SKILLS_REPO + provenance frontmatter gate` | `dallay/agentsync` | `.github/workflows/catalog-e2e.yml`, `openspec/changes/phase1-e2e-mirror-catalog/design.md` (this file) | ~30 | none |
| **PR 1** | `feat(agents-skills): add provenance frontmatter gate to validate_provenance.py` | `dallay/agents-skills` | `scripts/validate_provenance.py` | ~30 | none |
| **PR 2** (gated) | `feat(agents-skills): commit angular-architecture + typescript-strict-patterns as dallay-original` | `dallay/agents-skills` | `skills/angular-architecture/SKILL.md` (frontmatter), `skills/typescript-strict-patterns/SKILL.md` (frontmatter), `PROVENANCE.md` (2 rows + 2 SHA-256 lines) | ~10 + PROVENANCE | Yuniel authorship confirmation |
| **PR 3+** | DEFERRED: `feat(agents-skills): commit 8 clerk-* skills as upstream mirror` | `dallay/agents-skills` | 8 SKILL.md frontmatter patches + 24+ companion `references/*.md` + 8 PROVENANCE rows + 8 SHA-256 lines | ~600–900 | Clerk SPDX resolution OR re-distribution grant |
| **PR follow-up** | DEFERRED: `feat(agentsync): catalog.v1.toml + provider.rs Phase 1 expansion` | `dallay/agentsync` | `src/skills/catalog.v1.toml` (8 entries), `src/skills/provider.rs` (PHASE1_MIGRATED_LOCAL_SKILL_IDS), focused test blocks | ~200 | PR 3+ AND PR #569 merge |

Total PR 1 LOC: **~60** (well under 400-line budget). PR 2 LOC: **~10**. PR 3+ deferred entirely.

**Decision needed before apply: Yes** — Yuniel authorship confirmation gates PR 2; Clerk SPDX resolution
gates PR 3+ (out of scope for THIS change). **Chained PRs recommended: Yes.** **400-line budget
risk: Low for PR 1, gated for PR 2, High for PR 3+ (which is why it's deferred).**

## Open requirement resolutions

### REQ-SKILLREC-008 (angular-architecture + typescript-strict-patterns)

The upstream audit found **no authoritative upstream candidate** for either skill:

- `git -C /Users/acosta/Dev/dallay/agents-skills log --all -- skills/<id>/` returns **empty** for both.
  Both skill directories are untracked worktree entries on `feat/phase1-auth-db-skills` (see
  `agents-skills` `git status`).
- `gh search` for upstream candidates returned no byte-match.
- The content is plausible Yuniel-original prose (Angular 20+ / "Scope Rule" / TS pattern style
  consistent with the orchestrator persona), but the frontmatter has neither `metadata.author` nor
  `metadata.source`.

**Disposition:** DEFER → ACCEPT-WITH-EVIDENCE as `dallay-original`, gated on explicit Yuniel
confirmation of authorship. Per REQ-SKILLREC-008 branch (a):

```yaml
license: MIT
metadata:
  author: dallay-team
  source: dallay-original
  version: "1.0.0"
```

+ one `Materialized entries` row in `PROVENANCE.md` per skill + one `- '<id>/SKILL.md>': '<sha>'`
line. PROVENANCE.md frontmatter patch is part of PR 2.

### REQ-SKILLREC-009 (Clerk upstream evidence)

Upstream audit results:

| Skill | Upstream bytes | License claim | Repo-level LICENSE? | Decision |
|---|---|---|---|---|
| `clerk-setup` | **drift** (local 13042B v2.3.0 vs upstream 14567B v2.5.0) | `MIT` (frontmatter) | NO (404) | **DEFER** — needs older commit to byte-pin |
| `clerk-nextjs-patterns` | exact match (SHA `51a833e5...`) | `MIT` (frontmatter) | NO | **DEFER** — license gap + 5 missing companion refs |
| `clerk-react-patterns` | exact match (`ab22780d...`) | `MIT` | NO | **DEFER** — license gap + 4 companion refs |
| `clerk-vue-patterns` | exact match (`626fca49...`) | `MIT` | NO | **DEFER** — license gap + 3 companion refs |
| `clerk-astro-patterns` | exact match (`483df69c...`) | `MIT` | NO | **DEFER** — license gap + 5 companion refs |
| `clerk-webhooks` | exact match (`99138f2f...`) | `MIT` | NO | **DEFER** — license gap + 1 companion ref |
| `clerk-testing` | exact match (`0db14edb...`) | `MIT` | NO | **DEFER** — license gap (no companion refs) |
| `clerk-custom-ui` | exact match (`16398a39...`) | `MIT` | NO | **DEFER** — license gap + 5 `core-2/` + `core-3/` companion refs |

Critical finding: `gh api repos/clerk/skills/license` returns **404 Not Found**, and
`gh api repos/clerk/skills` returns `"license": null`. There is no top-level `LICENSE`, `LICENSE.md`,
`COPYING`, `NOTICE`, or `CONTRIBUTING` file at the repo root (verified via the `git/trees/main`
endpoint). The Clerk GitHub organization owns the repo (67 stars, 4 forks, last push 2026-08-20)
and `README.md` documents `npx skills add clerk/skills` as the install path, but neither of those
constitutes an explicit SPDX license grant or re-distribution notice.

**Disposition:** All 8 Clerk skills DEFER (REQ-SKILLREC-009 branch (c) "Refuse and document gap").
All 9 Clerk SKILL.md directories (including the out-of-scope `clerk-orgs`) remain untracked worktree
entries. The PROVENANCE.md `Materialized entries` table does NOT gain rows for any Clerk skill. The
verify-report.md must document the gap and reference the open follow-ups:

- Open issue against `clerk/skills` requesting a top-level LICENSE file, OR
- Obtain an explicit written re-distribution grant from the Clerk org, OR
- A future change extends `catalog.v1.toml` to keep Clerk external (PR #569's territory).

None of the Clerk skills can ship in this change.

## Risks

1. **Clerk SPDX gap is a HARD blocker.** The orchestrator's `AGENTS.md` mandates
   "Never agree with user claims without verification" — copying Clerk content without a top-level
   LICENSE file is a license-attribution risk that must not be waved through. Resolution requires an
   upstream LICENSE file or a written grant; both are out of scope for this change.
2. **`clerk-setup` version drift (local v2.3.0 ≠ upstream v2.5.0).** Even if SPDX is resolved, the
   local file would need to be re-pulled at an older upstream commit (pre-2026-08-20 main HEAD) to
   byte-pin. Otherwise the "byte-identical to upstream" claim is false.
3. **Companion `references/*.md` files (24+ across 7 Clerk skills) are not vendored locally.** Even
   if SPDX resolves, accepting any Clerk skill without its companions ships a broken / under-documented
   artifact. Vendoring alone is ~50–100 KB and pushes PR 3+ over the 400-line budget.
4. **`clerk-orgs` (Q1 — OUT of scope) is on the worktree but not in the table above.** Per user
   confirmation, it stays out of scope. It also has byte-identical content upstream (verified by
   directory listing at `skills/features/clerk-orgs`), so if Q1 is reopened it falls under the same
   SPDX gap.
5. **PR #569 may land first.** If PR #569 merges before PR 1, the `every_catalog_skill_installs_successfully`
   early-return guards are removed by Jules. That does NOT affect PR 1 (workflow fix only). If PR 3+
   is later attempted in this chain, it must avoid touching `tests/test_catalog_integration.rs`
   lines that PR #569 modifies. REQ-SKILLREC-007 codifies this isolation.
6. **Yuniel authorship is unverified.** The 2 dallay-original candidates read as Yuniel-style prose
   but git history is empty and frontmatter has no `author`. Until Yuniel confirms in writing, PR 2
   is blocked.
7. **400-line budget guard is preserved for PR 1 (~60 LOC) but exceeded by Clerk content.** Even if
   SPDX resolved tomorrow, a single Clerk PR with 8 SKILL.md patches + 24+ companion refs + 8
   PROVENANCE rows would exceed 800 LOC. Stacked PR slices are required; design reserves that
   decision for the future follow-up change.
8. **`AGENTSYNC_LOCAL_SKILLS_REPO` pin SHA `c2e79fb...` must move with `agents-skills` releases.**
   PR 1 hard-codes this commit; bumping `agents-skills@main` past this SHA requires a coordinated
   `agentsync` workflow bump (same pattern as `ci.yml:165`).

## Rollback

**Per-PR:**

- **PR 1 rollback (`dallay/agentsync`):** Revert `.github/workflows/catalog-e2e.yml` to the
  pre-change 80-line state (no sibling checkout, no `AGENTSYNC_LOCAL_SKILLS_REPO`). The CI job
  returns to its known-failing-but-deterministic state on the offline job. No data migration.
- **PR 1 rollback (`dallay/agents-skills`):** Revert `scripts/validate_provenance.py` to the
  82-line pre-change state. SHA-256 validation reverts to the looser check; no committed skill
  bytes change.

**Partial chain rollback:** If PR 1 ships but a subsequent PR (PR 2 gated on Yuniel confirmation)
is held back, no PR 1 semantics change — PR 2 is additive. If PR 3+ (Clerk content) is later
attempted and fails on SPDX, reverting the SKILL.md patches + companion refs + PROVENANCE rows
restores the untracked-worktree state.

**Mid-chain CI failure:** The fail-closed guard at `src/skills/provider.rs:244-250` remains
unchanged across the chain. No intermediate state silently falls back to network resolution. The
catalog resolver never weakens.

**Skip-the-chain recovery:** If `c2e79fb...` is force-pushed or invalidated, revert the
`.github/workflows/catalog-e2e.yml` env var block; the offline job fails-closed as before this
change. No silent fall-through.

## Verification plan

What `sdd-verify` will check on PR 1:

1. **Workflow diff is limited to catalog-e2e.yml.**
   `git diff --name-only origin/main...HEAD | grep -v 'openspec/' | grep -v 'design.md'`
   MUST contain exactly `.github/workflows/catalog-e2e.yml`.

2. **`catalog.v1.toml` is untouched.**
   `git diff -- 'src/skills/catalog.v1.toml'` MUST be empty (REQ-SKILLREC-007).

3. **`provider.rs` `PHASE1_MIGRATED_LOCAL_SKILL_IDS` is unchanged.**
   `git diff -- 'src/skills/provider.rs' | grep -c 'PHASE1_MIGRATED_LOCAL_SKILL_IDS'` MUST be 0
   (REQ-SKILLREC-007).

4. **The `agents-skills/scripts/validate_provenance.py` frontmatter gate is RED before, GREEN after.**
   Reproduce REQ-SKILLREC-003 scenario: run `python3 scripts/validate_provenance.py --root
   /Users/acosta/Dev/dallay/agents-skills` before applying PR 1's `agents-skills` half → expect
   exit 1 with stderr naming `skills/clerk-nextjs-patterns/SKILL.md: missing metadata.source`.
   After applying PR 1's `agents-skills` half and patching only the 3 Bobmatnyc skills
   (`drizzle-orm`, `pydantic`, `sqlalchemy`) which already declare `source` + `source_commit` → expect
   exit 0 (the new gate passes for already-conformant entries; the Clerk + dallay-original
   candidates still fail and are documented in the verify-report as a known gap).

5. **Catalog-e2e CI workflow produces the expected env block.**
   On the PR branch run `gh workflow view catalog-e2e.yml` and confirm both `offline` and
   `catalog-installation` jobs contain the `AGENTSYNC_LOCAL_SKILLS_REPO` env + a sibling checkout
   step pinning `c2e79fbb72d146305f82a8e979270795557d24fd`.

6. **`cargo test --test test_catalog_integration phase1_bobmatnyc_...` passes on a clean runner.**
   Expected: GREEN (3 DB skills resolve via the new sibling checkout + env var, install
   succeeds, registry contains canonical keys).

7. **`cargo clippy --all-targets --all-features -- -D warnings` + `cargo fmt --all -- --check` clean.**

8. **Issue hygiene posts (REQ-SKILLREC-006):** After PR 1 merges, the orchestrator posts
   "Duplicate of #556 — closing." on `dallay/agentsync#555` and a progress comment on
   `dallay/agentsync#556`. These are orchestrator-driven, not part of PR 1's diff.

Expected outputs:

- `cargo test --test test_catalog_integration` → 1 test (the focused Phase 1) PASS.
- `python3 scripts/validate_provenance.py` → exit 1 listing Clerk + dallay-original candidates as
  RED (documented as DEFER with evidence); exit 0 only after PR 2+ per-skill patches land.
- `gh run list --workflow=catalog-e2e.yml` after merge → `offline` job GREEN on next scheduled run.

## Next phase

`/sdd-tasks` — produce three task groups for the chained PR plan:

1. **T-PR1 (`agentsync`):** Add sibling `actions/checkout` of `dallay/agents-skills` at pin
   `c2e79fb...` to both `offline` and `catalog-installation` jobs in
   `.github/workflows/catalog-e2e.yml`; add `AGENTSYNC_LOCAL_SKILLS_REPO` env; preserve
   `remote-refresh` job untouched.
2. **T-PR1 (`agents-skills`):** Add `check_frontmatter()` to `scripts/validate_provenance.py`;
   wire it ahead of the SHA-256 step; ensure the RED scenario exits 1 with named offender; ensure
   the GREEN scenario still passes for the 3 Bobmatnyc skills.
3. **T-PR2 (gated):** PATCH frontmatter of `skills/angular-architecture/SKILL.md` and
   `skills/typescript-strict-patterns/SKILL.md` with `metadata.author: dallay-team`,
   `metadata.source: dallay-original`; add 2 PROVENANCE rows + 2 SHA-256 lines. **GATED on Yuniel
   authorship confirmation** (orchestrator gates; not a code gate).
4. **T-FOLLOWUP-OPEN (Clerk follow-up):** Open tracking issue linking the SPDX gap (top-level
   LICENSE absent from `clerk/skills`) and the vendoring gap (24+ companion references). This
   task group does NOT include any code changes in this change; it produces a tracking issue and
   updates the verify-report.
5. **T-VERIFY:** Run the verification plan above; produce `verify-report.md` with PASS / PASS WITH
   WARNINGS / FAIL for PR 1.
