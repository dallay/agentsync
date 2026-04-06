# Tasks: External skill catalog policy cleanup

## Phase 1: Embedded catalog foundation

- [x] 1.1 In `src/skills/catalog.rs`, identify the strict embedded load path (`parse_embedded_catalog()` → `normalize_catalog()`) and thread an embedded-only policy validation hook after technology/combo normalization and before returning `ResolvedSkillCatalog`.
- [x] 1.2 In `src/skills/catalog.rs`, add a small helper/predicate for embedded recommendation source classification: allow `dallay/agents-skills/*`, allow the approved official external set, reject all other external provider skill IDs.
- [x] 1.3 In `src/skills/catalog.rs`, validate every referenced skill in embedded `technologies[*].skills` and `combos[*].skills`, returning an explicit catalog-loading error when a referenced recommendation is disallowed.

## Phase 2: Catalog data cleanup

- [x] 2.1 In `src/skills/catalog.v1.toml`, remove the embedded `[[skills]]` entries for `biome-linter`, `nodejs-backend-patterns`, `nodejs-best-practices`, and `typescript-advanced-types`.
- [x] 2.2 In `src/skills/catalog.v1.toml`, update `node` technology skills to a policy-compliant non-empty list using `dallay/agents-skills/best-practices` and remove references to the deleted Node external skills.
- [x] 2.3 In `src/skills/catalog.v1.toml`, update `typescript` and `biome` technology skills to policy-compliant non-empty lists using existing local replacements (`dallay/agents-skills/best-practices` and `dallay/agents-skills/prettier-formatting`).
- [x] 2.4 In `src/skills/catalog.v1.toml`, check `combos[*].skills` and any remaining technology mappings for hidden references to the removed provider IDs so normalization only sees valid, non-dangling recommendations.

## Phase 3: Focused regression coverage

- [x] 3.1 In `tests/unit/suggest_catalog.rs`, add a fixture-based test proving an approved official external embedded recommendation still loads successfully through `parse_embedded_catalog()`.
- [x] 3.2 In `tests/unit/suggest_catalog.rs`, add a fixture-based test proving a disallowed third-party embedded recommendation fails explicitly with a policy-validation error.
- [x] 3.3 In `tests/unit/suggest_catalog.rs`, add baseline assertions that `EmbeddedSkillCatalog::default()` no longer exposes or references the four removed skills and that `node`, `typescript`, and `biome` still have non-empty valid mappings.
- [x] 3.4 In `tests/unit/suggest_catalog.rs`, keep provider overlay behavior covered by asserting existing lenient overlay tests remain unchanged and focused on provider non-regression.
