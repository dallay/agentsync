# Task Breakdown: agents-skills Monorepo

| Field | Value |
|-------|-------|
| **Change ID** | agents-skills-monorepo |
| **Version** | 1.0 |
| **Status** | Completed |

## Task Overview

```text
Phase 1: Repo Setup          [5 tasks]  ──  No agentsync changes
Phase 2: Content Migration    [3 tasks]  ──  Skill files moved
Phase 3: Catalog Integration  [4 tasks]  ──  agentsync changes
Phase 4: CI & Quality Gates   [3 tasks]  ──  Both repos
Phase 5: Documentation        [3 tasks]  ──  Both repos
```

---

## Phase 1: Repository Setup

### TASK-01: Create GitHub repo `dallay/agents-skills`

**Type:** Setup
**Effort:** Small
**Dependencies:** None

Create the repo on GitHub with:
- Public visibility
- MIT license
- Main branch protection (require PR reviews, CI passing)
- Description: "Curated AI agent skills for AgentSync — install, contribute, and manage reusable skills"
- Topics: `ai-agents`, `skills`, `agentsync`, `ai-coding-assistants`

**Acceptance:** Repo exists and is accessible at `github.com/dallay/agents-skills`

---

### TASK-02: Create repo skeleton ✅

**Type:** Implementation
**Effort:** Medium
**Dependencies:** TASK-01

Create initial structure:

```text
agents-skills/
├── README.md
├── CONTRIBUTING.md
├── LICENSE (MIT)
├── .github/
│   ├── PULL_REQUEST_TEMPLATE.md
│   └── workflows/
│       └── validate-skills.yml
└── skills/
    └── .gitkeep
```

**README.md content:**
- Project description and purpose
- Quick install example: `agentsync skill install <name>`
- Link to full skill list
- Contributing badge/link
- Link to agentsync CLI

**CONTRIBUTING.md content:**
- Skill directory structure
- SKILL.md manifest format with example
- Naming conventions
- Quality expectations
- Testing locally instructions
- PR checklist

**Acceptance:** Skeleton repo with CI, README, and CONTRIBUTING merged to main

---

### TASK-03: Implement CI validation workflow ✅

**Type:** CI/CD
**Effort:** Medium
**Dependencies:** TASK-02

Create `.github/workflows/validate-skills.yml` that:
1. Triggers on push to main and PRs touching `skills/**`
2. Finds all `skills/*/SKILL.md` files
3. Validates frontmatter (name, version, description, triggers)
4. Verifies name matches directory name
5. Checks content is non-empty after frontmatter
6. Checks for duplicate skill directories
7. Reports clear error messages per file

**Options:**
- A) Shell script (zero dependencies, works immediately)
- B) Rust binary using agentsync's manifest parser (more thorough, requires build)
- C) Node.js script with gray-matter (middle ground)

**Recommendation:** Start with (A) for speed, upgrade to (B) when agentsync
exposes manifest validation as a library function.

**Acceptance:** CI runs on PR, catches invalid manifests, passes for valid ones

---

### TASK-04: Set up branch protection rules

**Type:** Setup
**Effort:** Small
**Dependencies:** TASK-03

Configure main branch protection:
- Require PR reviews (1 reviewer minimum)
- Require CI to pass (validate-skills workflow)
- Require branches to be up-to-date
- No force pushes to main

**Acceptance:** PRs to main require passing CI and review

---

### TASK-05: Add semantic PR title enforcement ✅

**Type:** CI/CD
**Effort:** Small
**Dependencies:** TASK-02

Add workflow to enforce conventional commit PR titles:
- `feat:` — new skill
- `fix:` — skill content fix
- `docs:` — README/CONTRIBUTING changes
- `chore:` — CI, tooling changes

**Acceptance:** PRs with non-semantic titles are blocked

---

## Phase 2: Content Migration

### TASK-06: Identify all dallay-owned skills and their current sources

**Type:** Research
**Effort:** Medium
**Dependencies:** None (can run in parallel with Phase 1)

For each skill in `catalog.v1.toml` where `provider_skill_id == local_skill_id`
(the built-in pattern), determine:
1. Where the skill content currently lives
2. Whether it has a SKILL.md with valid frontmatter
3. What the canonical content should be

Create a migration spreadsheet:

| Skill ID | Current Source | Has Valid Manifest | Action |
|----------|---------------|-------------------|--------|
| accessibility | skills.sh search | ? | Migrate |
| docker-expert | skills.sh search | ? | Migrate |
| ... | ... | ... | ... |

**Acceptance:** Complete inventory of all skills to migrate with source locations

---

### TASK-07: Migrate skill content to agents-skills repo

**Type:** Implementation
**Effort:** Large
**Dependencies:** TASK-02, TASK-06

For each dallay-owned skill:
1. Copy/create `skills/{skill-id}/SKILL.md` with valid frontmatter
2. Include any supporting resources (templates, scripts)
3. Ensure CI validation passes
4. One PR per batch (5-10 skills per PR for manageable review)

**Migration batches:**
- Batch 1: Core web skills (accessibility, best-practices, performance, seo, core-web-vitals)
- Batch 2: Frontend skills (frontend-design, webapp-testing, web-quality-audit)
- Batch 3: DevOps skills (docker-expert, github-actions, makefile, grafana-dashboards)
- Batch 4: Code quality skills (brainstorming, skill-creator, pr-creator, markdown-a11y)
- Batch 5: Language/DB skills (rust-async-patterns, sql-optimization-patterns, pinned-tag)

**Acceptance:** All dallay-owned skills in repo with passing CI

---

### TASK-08: Verify skill install works from agents-skills repo

**Type:** Testing
**Effort:** Small
**Dependencies:** TASK-07

Manually test:
```bash
# Direct install from the new repo
agentsync skill install accessibility --source https://github.com/dallay/agents-skills
```

Verify:
- ZIP download succeeds
- Correct subdirectory extracted
- SKILL.md validates
- Installed to `.agents/skills/accessibility/`

**Acceptance:** Manual install from agents-skills repo works end-to-end

---

## Phase 3: Catalog Integration (agentsync repo)

### TASK-09: Verify resolver behavior for `dallay/agents-skills/*` ✅

**Type:** Research / Implementation
**Effort:** Medium
**Dependencies:** TASK-07

Investigate `resolve_deterministic()` in `src/skills/provider.rs`:
1. What subpath does `dallay/agents-skills/docker-expert` resolve to?
2. Does the repo name `agents-skills` trigger the `skills/` prefix logic?
3. If not, either:
   a. Add `agents-skills` to the special repo name list, OR
   b. Use `dallay/agents-skills/skills/docker-expert` as provider_skill_id

**Write a unit test** that validates the expected resolution:

```rust
#[test]
fn test_resolve_dallay_agents_skills_deterministic() {
    let provider = SkillsShProvider;
    let info = provider.resolve("dallay/agents-skills/docker-expert").unwrap();
    assert!(info.download_url.contains("dallay/agents-skills/archive/HEAD.zip"));
    assert!(info.download_url.contains("#skills/docker-expert"));
}
```

**Acceptance:** Resolution path is verified and tested

---

### TASK-10: Update catalog.v1.toml with new provider_skill_ids ✅

**Type:** Implementation
**Effort:** Medium
**Dependencies:** TASK-09

Update all dallay-owned skill entries:

```diff
 [[skills]]
-provider_skill_id = "accessibility"
+provider_skill_id = "dallay/agents-skills/accessibility"
 local_skill_id = "accessibility"
 title = "Accessibility"
 summary = "WCAG 2.1 audit and improvement guidelines"
```

**Acceptance:**
- All dallay-owned skills use `dallay/agents-skills/*` format
- External skills remain unchanged
- `cargo test suggest_catalog` passes
- `agentsync skill suggest` produces same recommendations

---

### TASK-11: Add catalog integrity E2E test ✅

**Type:** Testing
**Effort:** Small
**Dependencies:** TASK-10

Add test (gated behind `RUN_E2E=1`) that validates all `dallay/agents-skills/*`
entries in the catalog point to existing skills in the remote repo.

**Acceptance:** Test passes when agents-skills repo has all referenced skills

---

### TASK-12: Release agentsync with updated catalog

**Type:** Release
**Effort:** Small
**Dependencies:** TASK-10, TASK-11

1. Ensure all tests pass (unit + integration)
2. Update CHANGELOG
3. Bump version
4. Release

**Acceptance:** New agentsync version installs dallay skills deterministically

---

## Phase 4: CI & Quality Gates

### TASK-13: Add optional repository_dispatch notification

**Type:** CI/CD
**Effort:** Small
**Dependencies:** TASK-03

In `agents-skills`, add workflow that sends `repository_dispatch` to `agentsync`
when skills are added/removed on main. This is advisory only — no auto-modification.

```yaml
# .github/workflows/notify-catalog.yml
on:
  push:
    branches: [main]
    paths: ['skills/**']
jobs:
  notify:
    runs-on: ubuntu-latest
    steps:
      - uses: peter-evans/repository-dispatch@v3
        with:
          token: ${{ secrets.AGENTSYNC_DISPATCH_TOKEN }}
          repository: dallay/agentsync
          event-type: skills-updated
          client-payload: '{"ref": "${{ github.sha }}"}'
```

**Acceptance:** Merge to agents-skills triggers a check workflow in agentsync

---

### TASK-14: Add skills count badge to agents-skills README

**Type:** Enhancement
**Effort:** Small
**Dependencies:** TASK-07

Add a dynamic badge showing skill count:
```markdown
![Skills](https://img.shields.io/badge/dynamic/json?url=...&label=skills&query=$.count)
```

Or a simple static badge updated by CI.

**Acceptance:** README shows current skill count

---

### TASK-15: Add link validation for SKILL.md content

**Type:** CI/CD
**Effort:** Small
**Dependencies:** TASK-03

Add markdown link checking to CI to catch broken URLs in skill content.

**Acceptance:** Broken links in SKILL.md files fail CI

---

## Phase 5: Documentation

### TASK-16: Update agentsync skills guide ✅

**Type:** Documentation
**Effort:** Medium
**Dependencies:** TASK-10

Update `website/docs/src/content/docs/guides/skills.mdx`:
- Add section about the agents-skills repo
- Link to CONTRIBUTING.md for skill authors
- Explain the dual-source model (owned vs external)

**Acceptance:** Skills guide references agents-skills repo

---

### TASK-17: Update agentsync README ✅

**Type:** Documentation
**Effort:** Small
**Dependencies:** TASK-10

Add to agentsync README:
- Link to agents-skills repo in the Skills section

**Acceptance:** README links to agents-skills (skills-count badge is a separate enhancement tracked by TASK-14)

---

### TASK-18: Add "Create a Skill" tutorial to docs site

**Type:** Documentation
**Effort:** Medium
**Dependencies:** TASK-16

New page in docs site: `/guides/creating-skills/`
- Step-by-step guide to creating a skill
- SKILL.md format reference
- How to test locally
- How to submit to agents-skills repo

**Acceptance:** Tutorial page built and linked from skills guide

---

## Execution Order (Dependency Graph)

```text
TASK-01 ─── TASK-02 ─── TASK-03 ─── TASK-04
                │            │        TASK-05
                │            │
                │            └─── TASK-13
                │                 TASK-14
                │                 TASK-15
                │
TASK-06 ───────┼─── TASK-07 ─── TASK-08
                                    │
                              TASK-09 ─── TASK-10 ─── TASK-11 ─── TASK-12
                                                │
                                          TASK-16 ─── TASK-18
                                          TASK-17
```

**Critical path:** TASK-01 → 02 → 03 → 07 → 09 → 10 → 12

**Estimated total effort:** ~2-3 days for one developer

## Risk Mitigation Checklist

- [ ] Resolver subpath verified (TASK-09) before catalog changes
- [ ] Skills.sh fallback remains functional during transition
- [ ] E2E test validates catalog integrity
- [ ] All existing `skill install` commands still work
- [ ] No breaking changes to registry.json format
