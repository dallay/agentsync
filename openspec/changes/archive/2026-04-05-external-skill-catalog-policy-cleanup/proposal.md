# Proposal: Clean up external skill catalog entries to enforce recommendation policy

## Intent

The embedded recommendation catalog currently includes external skill recommendations that do not meet the approved policy for external suggestions. External recommendations must point only to approved official tech skills, while third-party or community skills must not be recommended from external sources and should instead live under `dallay/agents-skills` if the team curates them.

This change narrows the catalog back to policy-compliant entries by removing the currently problematic recommendations `biome-linter`, `nodejs-backend-patterns`, `nodejs-best-practices`, and `typescript-advanced-types` from the embedded catalog where no approved local replacement exists yet, and by adding validation coverage so similar entries cannot re-enter silently.

## Scope

### In Scope
- Remove the four policy-violating skill definitions from the embedded catalog
- Remove or update technology mappings that currently reference those four skills so the catalog remains valid after cleanup
- Add embedded catalog validation coverage that fails when disallowed external recommendations are added again
- Document the policy intent in the proposal/spec flow so future catalog maintenance has an explicit guardrail

### Out of Scope
- Creating new curated replacements in `dallay/agents-skills`
- Expanding the approved external allowlist beyond what policy already permits
- Redesigning the overall recommendation architecture or provider overlay behavior
- Broad catalog curation beyond the four identified problematic entries

## Approach

Update `src/skills/catalog.v1.toml` as a narrow content cleanup: remove the four disallowed skill definitions and replace affected technology recommendation lists with only policy-compliant entries that already exist locally, or leave those technologies without those recommendations where no compliant replacement exists yet.

Then add a validation layer and regression tests around embedded catalog loading so the checked-in catalog fails fast if a future change introduces external recommendations that are not approved official tech skills. The validation should target the policy boundary directly instead of relying only on ad hoc count-based catalog tests.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/skills/catalog.v1.toml` | Modified | Remove the four problematic entries and clean up technology references for `node`, `typescript`, and `biome` |
| `src/skills/catalog.rs` | Modified | Enforce embedded catalog policy validation during normalization/loading |
| `tests/unit/suggest_catalog.rs` | Modified | Add regression coverage for policy-invalid external recommendations |
| `openspec/specs/skill-recommendations/spec.md` | Modified later in follow-on phase | Capture the policy rule and expected validation behavior in delta specs |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Recommendation output changes for Node, TypeScript, or Biome repositories | High | Keep the scope explicit, update specs/tests, and treat the removals as intentional policy cleanup |
| Validation is too strict and blocks legitimate approved official skills | Medium | Encode the rule around approved official skills only and cover both allowed and disallowed examples in tests |
| Hidden references to removed skills remain in technologies or combos | Medium | Add regression coverage that loads the full embedded catalog and fails on dangling or policy-invalid references |

## Rollback Plan

Revert the catalog cleanup and validation changes in `src/skills/catalog.v1.toml`, `src/skills/catalog.rs`, and `tests/unit/suggest_catalog.rs`. This restores the previous recommendation set immediately because the catalog is checked in and loaded at runtime without migrations.

## Dependencies

- Approved external skill policy as already decided by maintainers
- Existing local curated catalog entries in `dallay/agents-skills` only where they already exist today

## Success Criteria

- [ ] The embedded catalog no longer recommends `biome-linter`, `nodejs-backend-patterns`, `nodejs-best-practices`, or `typescript-advanced-types`
- [ ] Embedded catalog loading remains valid after removing those entries and updating affected references
- [ ] Automated validation fails if a new non-approved third-party/community external recommendation is added to the embedded catalog
- [ ] Automated validation still permits approved official external skill recommendations
- [ ] Recommendation behavior changes are limited to the intended policy cleanup scope
