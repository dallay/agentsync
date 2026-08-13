# Design: Phase 1 Catalog Skill Migration

## Technical Approach

Treat nine Clerk skills plus `drizzle-orm`, `pydantic`, and `sqlalchemy` as candidates. Remap only
entries passing source, companion, provenance, and license gates. Preserve `local_skill_id`, title,
summary, install folder, and installed-registry key. This is not a registry redesign or full-catalog
health claim.

## Architecture Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Source of truth | `agents-skills/skills/<local_skill_id>/` | Matches the sibling repository convention and the resolver’s existing `skills/<id>` lookup. |
| Catalog identity | `dallay/agents-skills/<local_skill_id>`; remove migrated `install_source` | Existing embedded policy already permits this prefix; removing stale `HEAD` fragments prevents path drift. |
| Resolver safety | Local candidates win; Phase 1 candidates fail closed when absent; unrelated externals retain current fallback | Missing curated content must not silently become mutable `archive/HEAD.zip`. |
| Registry scope | Do not change `registry.v1.toml` or `registry.lock.toml` | Hash/approval metadata and Clerk license evidence are not complete; verified-registry hardening is a later change. |
| Content | Copy source text verbatim and add required companions, without rewriting `SKILL.md` | Preserves upstream attribution and behavior; missing references must be visible rather than hidden. |

## Data Flow

```text
catalog recommendation -> qualified local provider ID
  -> test source -> AGENTSYNC_LOCAL_SKILLS_REPO -> sibling ../agents-skills
  -> install_from_dir (recursive copy + SKILL.md validation) -> registry.json
```

For a migrated candidate, no local source means an actionable resolver error. Other external
entries retain existing `install_source`/provider behavior.

## Catalog Mapping

For every approved candidate, change the provider ID to `dallay/agents-skills/{local_skill_id}` and
remove its external `install_source`. Update the Clerk technology entry; Vue, React, Astro, and
Next.js Clerk combos; and Drizzle, Pydantic, and SQLAlchemy technologies. Keep the base
`clerk/skills/clerk` router, Wispbit SQLAlchemy entry, other Clerk framework entries, and unrelated
external mappings unchanged.

## Interfaces / Contracts

`resolve_catalog_install_source()` keeps its precedence but must receive the real project root from
direct and suggestion-install callers (current callers pass `None`, so sibling lookup is skipped):

```text
local override or sibling directory -> return directory
missing Phase 1 local source        -> return error
other catalog entry                 -> existing install_source/provider fallback
```

The local ID remains the installer argument and destination; the qualified provider ID is used
only for resolution.

## Provenance, Attribution, and Companions

Create a sibling provenance record (not a skill file) with source repository/path, immutable commit,
Git blob/file identity, attribution, license evidence or permission, and companion audit per entry.
Use the recorded Clerk and Bob Matsuoka data from `exploration.md`. Clerk’s frontmatter/README MIT
claim is insufficient; without authoritative evidence or permission, leave all nine unmigrated.
Materialize DB entries from their observed `toolchains/...` paths under canonical `skills/` names.
Body-linked `references/`, `core-*`, templates, and other companions must exist and survive recursive
installation; Pydantic has no known companions.

## File Changes

| File | Action | Description |
|---|---|---|
| `src/skills/catalog.v1.toml` | Modify | Approved IDs, sources, and affected mappings only. |
| `src/skills/provider.rs` | Modify | Pass-through local lookup plus narrow Phase 1 fail-closed guard/tests. |
| `src/commands/skill.rs` | Modify | Thread project root into direct and suggestion resolution. |
| `tests/unit/suggest_catalog.rs`, `tests/unit/provider.rs` | Modify | Identity, boundary, precedence, and missing-source regressions. |
| `tests/test_catalog_integration.rs` | Modify | Explicit Phase 1 offline subset test; retain full-catalog early return. |
| `../agents-skills/skills/...`, provenance record | Prepare in sibling commit | Add only gated skills, companions, attribution, and validator-clean manifests. |

## Testing Strategy

| Layer | Coverage | Approach |
|---|---|---|
| Unit | Catalog identity/boundaries | Assert approved canonical IDs and that base Clerk, Wispbit, and other external IDs remain external. |
| Resolver | Sibling and override precedence | Use isolated layouts; assert local paths and no provider call. Exercise `AGENTSYNC_LOCAL_SKILLS_REPO` in a subprocess to avoid environment races. |
| Focused integration | Every approved entry | An explicit 12-entry (or smaller gated subset) installs offline, checks `SKILL.md`, companions, folder, and canonical `registry.json` key. Missing entries fail, never omit. |
| Sibling validation | Manifest/source quality | Run `skills-ref validate` per migrated directory and record results; audit relative links separately. |

## Migration / Rollout

First commit/validate sibling content, record provenance/license decisions, then apply catalog/caller
changes and focused tests. No data migration is required. Rollback reverts catalog, callers, and
tests; installed directories and `registry.json` are not edited.

## Open Questions

- [ ] Obtain authoritative Clerk MIT evidence or maintainer permission.
- [ ] Add/commit missing Clerk companions and the three DB skill directories.
- [ ] Decide in a later change when approved entries move from legacy local resolution to verified registry pins.
