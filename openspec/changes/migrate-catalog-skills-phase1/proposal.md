# Proposal: Phase 1 Catalog Skill Migration

## Intent

Replace brittle external resolution for broken catalog entries with deterministic local-sibling
resolution from `/Users/acosta/Dev/dallay/agents-skills`, preserving `local_skill_id`, `title`,
and `summary`. This is not a catalog redesign or a full-health claim.

## Scope

### In Scope
- Migrate these Clerk entries when provenance gates pass: `clerk-setup`, `clerk-custom-ui`,
  `clerk-nextjs-patterns`, `clerk-react-patterns`, `clerk-vue-patterns`, `clerk-astro-patterns`,
  `clerk-orgs`, `clerk-webhooks`, `clerk-testing`.
- Migrate `drizzle-orm`, `pydantic`, and `sqlalchemy` only after verifying the recorded source paths
  and MIT evidence at commit `718070a7d622921b01687799a1f9613f36c6f615`.
- Remap accepted definitions to `dallay/agents-skills/{local_skill_id}`, update affected mappings,
  and test sibling plus `AGENTSYNC_LOCAL_SKILLS_REPO` paths.
- For Clerk, record repo/path, commit/blob, attribution, MIT evidence or permission, and companion
  status. If evidence is unavailable, block that entry.

### Out of Scope
- Re-enabling or claiming the full catalog E2E; remaining external failures stay out of scope.
- **Base `clerk` router recommendation: exclude from Phase 1.** No verified sibling source is
  present; retain its external mapping and revisit separately. The Wispbit SQLAlchemy entry remains.
- Registry redesign, unrelated sibling files, production code, or skill-content edits.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `skill-recommendations`: selected catalog entries use local curated resolution without changing
  recommendation IDs, metadata, or installed-state semantics.

## Approach

Validate a committed sibling revision and manifests first. Audit Clerk `references/`,
`templates/`, and eval companions; migrate each with provenance or block dependent entries. Remap
approved entries and add focused subset install/resolution coverage. Keep the full-catalog skip.

## Affected Areas

| Area | Impact | Description |
|---|---|---|
| `src/skills/catalog.v1.toml` | Modified | Approved Phase 1 definitions and mappings. |
| `tests/test_catalog_integration.rs`, `tests/unit/suggest_catalog.rs`, `tests/unit/suggest_install.rs` | Modified | Focused local tests only. |
| `../agents-skills` | Dependency | Must provide committed, validated, attributable content. |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Clerk MIT claim lacks authoritative evidence | High | Require evidence/permission; otherwise retain external mapping. |
| Missing companions make skills incomplete | Med | Inventory and migrate with provenance, or block. |
| External failures obscure test status | High | Report focused coverage only; keep full-catalog skip. |

## Rollback Plan

Revert catalog/mapping and focused-test changes, restoring prior provider IDs and install sources.
Do not alter installed directories or `registry.json`.

## Dependencies

- Committed sibling revision; Clerk license evidence/permission; verified DB paths and MIT records.

## Success Criteria

- [ ] Approved entries install locally without network access.
- [ ] Local IDs, title/summary metadata, and registry semantics remain unchanged.
- [ ] Clerk provenance, attribution, license, and companion decisions are recorded or blocked.
- [ ] DB entries map only after exact-path and MIT verification.
- [ ] Full catalog E2E remains explicitly out of scope.
