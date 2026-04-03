# Proposal: agents-skills Monorepo

| Field         | Value                                        |
|---------------|----------------------------------------------|
| **Change ID** | agents-skills-monorepo                       |
| **Author**    | dallay                                       |
| **Status**    | Draft                                        |
| **Created**   | 2026-04-03                                   |
| **Related**   | agentsync CLI, skills suggest/install system |

## Intent

Create a dedicated `dallay/agents-skills` repository as the canonical home for all
dallay-owned AI agent skills. This decouples skill content from the CLI binary,
enables independent versioning, community contributions, and CI validation of skill
manifests.

## Problem Statement

Today, ~11 built-in skills (accessibility, docker-expert, rust-async-patterns, etc.)
have no canonical source repository. They are resolved at install time via the
skills.sh search API, which:

1. **Adds latency** — every install requires an API search round-trip
2. **Creates fragility** — if skills.sh is down or returns unexpected results, install fails
3. **Blocks contributions** — there is no clear place for the community to submit new skills
4. **Prevents CI validation** — skill manifests are not validated until a user installs them
5. **Hinders discoverability** — skills are scattered across unknown repos with no central index

## Proposed Solution

### Hybrid monorepo approach

```text
dallay/agents-skills/          <-- NEW REPO: dallay-owned skill content
dallay/agentsync/              <-- EXISTING: CLI + embedded catalog (catalog.v1.toml)
External repos (angular/skills, vercel-labs/next-skills, ...)  <-- UNCHANGED
```

**Scope of `agents-skills`:**

- All skills where dallay is the author/maintainer
- Community-contributed skills accepted via PR
- CI pipeline for manifest validation and quality gates

**NOT in scope:**

- Skills maintained by external organizations (Angular, Vercel, Cloudflare, etc.)
- The catalog itself (stays embedded in agentsync binary)
- The provider/resolver architecture (stays in agentsync)

## Success Criteria

1. All dallay-owned skills have a canonical URL of the form
   `dallay/agents-skills/skills/{skill-id}`
2. `agentsync skill install {skill-id}` resolves dallay-owned skills deterministically
   (no search API needed)
3. Every skill in the repo passes manifest validation in CI
4. The catalog in `agentsync` references `dallay/agents-skills` for owned skills
5. Community can submit new skills via PR with clear contributing guidelines

## Risks

| Risk                                                          | Impact | Mitigation                                                         |
|---------------------------------------------------------------|--------|--------------------------------------------------------------------|
| Maintenance burden of 30+ skills                              | Medium | Start with existing built-ins only, grow organically               |
| Catalog drift (skill renamed/deleted but catalog not updated) | High   | CI check in agentsync that validates all dallay/* skill URLs exist |
| Breaking existing installs during migration                   | High   | Keep skills.sh fallback during transition period                   |
| Community PRs with low-quality skills                         | Medium | Clear CONTRIBUTING.md + CI manifest validation                     |

## Alternatives Considered

| Alternative                      | Why rejected                                                           |
|----------------------------------|------------------------------------------------------------------------|
| Git submodule in agentsync       | DX nightmare: clone friction, CI complexity, pin hell, coordinated PRs |
| All skills in agentsync repo     | Mixes concerns, bloats binary repo, different release cycles           |
| Keep status quo (skills.sh only) | No CI, no community hub, fragile resolution, no offline support        |
| Fork all external skills too     | Maintenance hell, licensing issues, stale content                      |

## Approach

1. Create `dallay/agents-skills` with standard repo structure
2. Migrate existing built-in skills from their current locations
3. Update `catalog.v1.toml` in agentsync to use deterministic `dallay/agents-skills/*` paths
4. Add CI to both repos for validation
5. Update documentation
