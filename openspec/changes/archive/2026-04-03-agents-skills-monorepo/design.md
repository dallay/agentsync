# Technical Design: agents-skills Monorepo

| Field         | Value                  |
|---------------|------------------------|
| **Change ID** | agents-skills-monorepo |
| **Version**   | 1.0                    |
| **Status**    | Draft                  |

## System Architecture

### Current State

```text
┌──────────────────────────────────────────────────┐
│                  agentsync CLI                    │
│                                                   │
│  ┌─────────────────┐   ┌──────────────────────┐  │
│  │ catalog.v1.toml │   │  SkillsShProvider    │  │
│  │ (embedded)      │──>│  - resolve(id)       │  │
│  │                 │   │  - search API call   │  │
│  │ 120+ skills     │   │  - GitHub ZIP URL    │  │
│  │  40+ techs      │   └──────────┬───────────┘  │
│  │  12 combos      │              │               │
│  └─────────────────┘              │               │
│                                   ▼               │
│                          ┌────────────────┐       │
│                          │ GitHub Repos   │       │
│                          │ (scattered)    │       │
│                          │ - angular/*    │       │
│                          │ - vercel-*     │       │
│                          │ - ??? (search) │       │
│                          └────────────────┘       │
└──────────────────────────────────────────────────┘
```

**Problems with current state:**

- Built-in skills have no canonical repo (resolved via search API)
- Search API is a single point of failure
- No CI validation of skill content
- No clear contribution path

### Target State

```text
┌──────────────────────────────────────────────────────────────┐
│                       agentsync CLI                          │
│                                                              │
│  ┌─────────────────┐     ┌──────────────────────────────┐   │
│  │ catalog.v1.toml │     │  SkillsShProvider            │   │
│  │ (embedded)      │────>│                              │   │
│  │                 │     │  resolve(id):                │   │
│  │ Skills point to:│     │    IF id has 2+ slashes      │   │
│  │ - dallay/*  ────│─┐   │      → deterministic URL     │   │
│  │ - angular/* ────│─│──>│    ELSE                      │   │
│  │ - vercel/*  ────│─│   │      → search API fallback   │   │
│  └─────────────────┘ │   └──────────────┬───────────────┘   │
│                      │                  │                    │
└──────────────────────│──────────────────│────────────────────┘
                       │                  │
                       ▼                  ▼
        ┌──────────────────┐   ┌──────────────────┐
        │ dallay/           │   │ External repos    │
        │ agents-skills     │   │                   │
        │                   │   │ angular/skills    │
        │ skills/           │   │ vercel-labs/*     │
        │ ├── accessibility │   │ cloudflare/*      │
        │ ├── docker-expert │   │ expo/skills       │
        │ ├── rust-async    │   │ anthropics/*      │
        │ ├── github-actions│   │ ...               │
        │ └── ...           │   │                   │
        │                   │   │                   │
        │ CI: validate all  │   │ Owned by orgs     │
        │ SKILL.md manifests│   │ (not our concern) │
        └──────────────────┘   └──────────────────┘
```

### Resolution Flow (Target)

```text
User: agentsync skill install docker-expert

1. CLI looks up "docker-expert" in catalog.v1.toml
   → provider_skill_id = "dallay/agents-skills/docker-expert"

2. Provider.resolve("dallay/agents-skills/docker-expert")
   → ID has 2+ slashes → deterministic path
   → owner = "dallay", repo = "agents-skills"
   → repo name is NOT "skills"/"agent-skills"/"agentic-skills"
      → subpath = "skills/docker-expert" (convention: skills/ prefix)

3. Construct URL:
   https://github.com/dallay/agents-skills/archive/HEAD.zip#skills/docker-expert

4. Download ZIP, extract skills/docker-expert/, validate SKILL.md, install

NO search API call. Fully deterministic.
```

### Suggest Flow (Unchanged)

```text
User: agentsync skill suggest

1. Load embedded catalog.v1.toml
2. CatalogDrivenDetector scans project files
3. Match detections → technology entries → skill recommendations
4. Annotate with install state from registry.json
5. Display results

The catalog entries now point to dallay/agents-skills/* for owned skills.
No change to the suggest flow itself — only the catalog data changes.
```

## Design Decisions

### DD-01: Separate repos, no submodule

**Decision:** `dallay/agents-skills` is an independent repo with no git link to `dallay/agentsync`.

**Rationale:**

- Different release cycles (catalog vs content)
- Zero clone friction for agentsync contributors
- No CI coordination needed for content-only updates
- Community can contribute skills without touching the CLI repo

### DD-02: Repo naming convention for deterministic resolution

**Decision:** Name the repo `agents-skills` (not `skills` or `agentsync-skills`).

**Rationale:**

- Generic enough for broader ecosystem use
- The existing resolver checks for repos named exactly `skills`, `agent-skills`,
  or `agentic-skills` and adds a `skills/` prefix to the subpath.
- `agents-skills` does NOT match those special names, so we need to understand
  the resolver behavior:

**RESOLVED — Resolver behavior verified and implemented:**

`resolve_deterministic()` in `provider.rs` checks `SKILLS_REPO_NAMES`:

```rust
const SKILLS_REPO_NAMES: &[&str] = &["skills", "agent-skills", "agentic-skills", "agents-skills"];
```

For `dallay/agents-skills/docker-expert`:

- owner = `dallay`
- repo = `agents-skills` (matches `SKILLS_REPO_NAMES`)
- skill = `docker-expert`
- subpath = `skills/docker-expert` (prefix added automatically)
- URL = `https://github.com/dallay/agents-skills/archive/HEAD.zip#skills/docker-expert`

This was implemented by adding `"agents-skills"` to the `SKILLS_REPO_NAMES` constant.

### DD-03: Skills directory convention

**Decision:** All skills live under `skills/{skill-id}/` in the repo root.

**Rationale:**

- Matches the convention used by angular/skills, expo/skills, etc.
- The `#skills/{skill-id}` fragment in the ZIP URL maps directly
- Clean separation from repo metadata (README, CI, etc.)

### DD-04: CI validation as quality gate

**Decision:** GitHub Actions workflow validates all SKILL.md manifests on every PR.

**Rationale:**

- Catches broken manifests before merge
- Ensures naming consistency (dir name == manifest name)
- Low maintenance — can reuse agentsync's manifest parser logic
- Builds trust for community contributions

### DD-05: No catalog auto-sync

**Decision:** Catalog updates in agentsync require manual PRs. No automated sync.

**Rationale:**

- Adding a skill to the catalog is a deliberate decision (includes detection rules)
- Auto-sync would require cross-repo workflow permissions (security concern)
- The two repos have different approval flows
- Manual PRs are clear and auditable

### DD-06: Optional repository_dispatch notification

**Decision:** The `agents-skills` CI MAY send a `repository_dispatch` event to
`agentsync` when a skill is added/removed, as a reminder to update the catalog.

**Rationale:**

- Low-cost notification that does not auto-modify anything
- Can trigger a "catalog check" workflow in agentsync
- Purely advisory — does not block anything

## CI/CD Design

### agents-skills CI Pipeline

```yaml
# .github/workflows/validate-skills.yml
name: Validate Skills

on:
  push:
    branches: [ main ]
    paths: [ 'skills/**' ]
  pull_request:
    paths: [ 'skills/**' ]

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Find all SKILL.md files
        id: find
        run: |
          find skills -name "SKILL.md" -type f > skill_files.txt
          echo "count=$(wc -l < skill_files.txt | tr -d ' ')" >> $GITHUB_OUTPUT

      - name: Validate manifests
        run: |
          # Option A: Use agentsync binary
          # cargo install agentsync && agentsync doctor --skills-dir skills/

          # Option B: Simple shell validation (no dependency on agentsync)
          errors=0
          while IFS= read -r file; do
            dir=$(dirname "$file")
            skill_name=$(basename "$dir")

            # Check frontmatter exists
            if ! head -1 "$file" | grep -q '^---$'; then
              echo "ERROR: $file missing frontmatter"
              errors=$((errors + 1))
              continue
            fi

            # Extract name from frontmatter
            manifest_name=$(sed -n '/^---$/,/^---$/p' "$file" | grep '^name:' | awk '{print $2}')

            # Verify name matches directory
            if [ "$manifest_name" != "$skill_name" ]; then
              echo "ERROR: $file name '$manifest_name' != dir '$skill_name'"
              errors=$((errors + 1))
            fi

            # Check content after frontmatter is non-empty
            content=$(sed '1,/^---$/d' "$file" | sed '1,/^---$/d' | tr -d '[:space:]')
            if [ -z "$content" ]; then
              echo "ERROR: $file has no content after frontmatter"
              errors=$((errors + 1))
            fi

            echo "OK: $file ($skill_name)"
          done < skill_files.txt

          if [ $errors -gt 0 ]; then
            echo "FAILED: $errors validation errors"
            exit 1
          fi
          echo "All skills validated successfully"

      - name: Check for duplicate skill names
        run: |
          dupes=$(find skills -maxdepth 1 -mindepth 1 -type d -exec basename {} \; | sort | uniq -d)
          if [ -n "$dupes" ]; then
            echo "ERROR: Duplicate skill directories: $dupes"
            exit 1
          fi
```

### agentsync Catalog Integrity Check

```rust
// tests/test_catalog_integrity.rs (gated behind RUN_E2E=1)

#[test]
#[ignore] // Run with: RUN_E2E=1 cargo test catalog_integrity
fn test_dallay_skill_urls_are_reachable() {
    if std::env::var("RUN_E2E").is_err() { return; }

    let catalog = load_embedded_catalog();
    let client = reqwest::blocking::Client::new();

    for skill in &catalog.skills {
        if skill.provider_skill_id.starts_with("dallay/agents-skills/") {
            let skill_name = skill.provider_skill_id
                .strip_prefix("dallay/agents-skills/")
                .unwrap();
            let url = format!(
                "https://api.github.com/repos/dallay/agents-skills/contents/skills/{}",
                skill_name
            );
            let resp = client.get(&url)
                .header("User-Agent", "agentsync-test")
                .send()
                .unwrap();
            assert!(
                resp.status().is_success(),
                "Skill {} not found at {}",
                skill.provider_skill_id, url
            );
        }
    }
}
```

## Data Flow Diagram

```text
                    ┌─────────────┐
                    │ Contributor  │
                    └──────┬──────┘
                           │
                    PR: add skills/terraform/SKILL.md
                           │
                           ▼
              ┌────────────────────────┐
              │  dallay/agents-skills  │
              │                        │
              │  CI: validate-skills   │──── PASS? ──── Merge
              │  - parse frontmatter   │
              │  - name == dir name    │
              │  - content non-empty   │
              └────────────┬───────────┘
                           │
                  (optional repository_dispatch)
                           │
                           ▼
              ┌────────────────────────┐
              │  dallay/agentsync      │
              │                        │
              │  Manual PR:            │
              │  - Add [[skills]]      │
              │    entry to catalog    │
              │  - Add [[technologies]]│
              │    detection rules     │
              │                        │
              │  CI: integrity check   │──── All dallay/* skills reachable? ──── PASS
              └────────────────────────┘
```

## Migration Strategy

### Phase 1: Create repo and migrate content

1. Create `dallay/agents-skills` repo on GitHub
2. Set up README, CONTRIBUTING, LICENSE (MIT)
3. Copy existing built-in skill content into `skills/` directories
4. Set up CI validation workflow
5. Verify all manifests pass validation

### Phase 2: Update catalog references

1. Update `catalog.v1.toml` entries for dallay-owned skills:
    - Change `provider_skill_id` from simple names to `dallay/agents-skills/{name}`
2. Verify deterministic resolution works (may need resolver adjustment — see DD-02)
3. Add E2E catalog integrity test
4. Release agentsync with updated catalog

### Phase 3: Documentation and community onboarding

1. Update agentsync docs (skills guide) to reference agents-skills repo
2. Add "Contributing a Skill" section to agents-skills README
3. Add link from agentsync README to agents-skills repo
4. Announce community contribution path

## Open Questions

1. ~~**Resolver subpath behavior**~~ — **RESOLVED.** `agents-skills` was added to
   `SKILLS_REPO_NAMES` in `provider.rs`. `dallay/agents-skills/docker-expert` resolves
   to `#skills/docker-expert` deterministically.

2. **Skill content sources** — Where do the current built-in skills actually live?
   Some may be on skills.sh, some may be in the user's own `.agents/skills/`. We need
   to identify the canonical source for each before migrating.

3. **Versioning strategy** — Should the repo use tags/releases? Or is HEAD always the
   latest? Given that `HEAD.zip` is the current resolution pattern, we should document
   this explicitly.

4. **Notification mechanism** — Is `repository_dispatch` worth the setup cost, or is
   a manual checklist sufficient for catalog sync reminders?
