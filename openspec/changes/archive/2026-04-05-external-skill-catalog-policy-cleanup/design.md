# Design: External skill catalog policy cleanup

## Technical Approach

Keep this change inside the embedded catalog loader path. The embedded fallback catalog already passes through `parse_embedded_catalog()` → `normalize_catalog()` in `src/skills/catalog.rs`; this is the narrowest place to enforce the approved external source policy before the fallback catalog becomes runtime data.

The implementation should add one embedded-only policy validation step after normal schema/reference normalization and before the `ResolvedSkillCatalog` is returned. That validator should inspect every recommended skill referenced by embedded technology and combo entries, allow all local curated skills under `dallay/agents-skills/*`, and allow only explicitly approved official external skills for non-local entries. Any embedded recommendation that points at a non-approved external source must fail catalog loading explicitly.

This keeps the change tightly scoped to:
- removing `biome-linter`, `nodejs-backend-patterns`, `nodejs-best-practices`, and `typescript-advanced-types` from `src/skills/catalog.v1.toml`
- updating the affected embedded technology mappings so they still validate
- adding regression coverage around embedded policy enforcement

## Architecture Decisions

### Decision: Enforce policy during embedded catalog normalization, not in suggestion generation

**Choice**: Validate the embedded external recommendation policy inside `normalize_catalog()` / `parse_embedded_catalog()` before `EmbeddedSkillCatalog::default()` can succeed.

**Alternatives considered**: Filter invalid skills later during `recommend_skills()`; validate only in tests; validate only at TOML edit time via external tooling.

**Rationale**: The spec requires the embedded fallback catalog to fail explicitly when it is policy-invalid. Late filtering would silently keep a bad checked-in catalog alive, and test-only enforcement would not protect runtime initialization.

### Decision: Make the policy validator embedded-only for this change

**Choice**: Apply the approved external source check only to the embedded baseline path and leave provider overlay validation semantics unchanged.

**Alternatives considered**: Apply the same rule to provider metadata immediately; redesign provider overlay behavior to share one strict policy gate.

**Rationale**: The active spec already requires provider metadata to remain an optional, lenient overlay where partially invalid provider entries are ignored while valid ones are preserved. Tightening provider behavior here would broaden scope beyond catalog cleanup and would risk breaking the required fallback behavior.

### Decision: Validate referenced recommendations, not just raw skill definitions

**Choice**: Run the policy check against every skill reference used by embedded `technologies[*].skills` and `combos[*].skills`, resolving each reference through the normalized skill-definition map.

**Alternatives considered**: Validate every external `[[skills]]` definition regardless of whether it is referenced; validate only technology entries.

**Rationale**: The policy is about what the embedded catalog recommends. Validating referenced entries matches the spec language directly, still catches dangling references through existing integrity checks, and naturally covers combos without changing the catalog model.

## Data Flow

### Embedded load with policy enforcement

```text
catalog.v1.toml
   │
   ▼
toml::from_str::<RawCatalogDocument>()
   │
   ▼
normalize_skill_definitions()
normalize_technologies()
normalize_combos()
   │
   ▼
validate_embedded_external_recommendation_policy()
   │
   ├── all referenced skills are local or approved official external
   │      ▼
   │   ResolvedSkillCatalog
   │
   └── any referenced skill is disallowed external
          ▼
       explicit catalog load error
```

### Sequence diagram: embedded baseline initialization

```text
SuggestionService -> load_catalog(): request catalog
load_catalog() -> parse_embedded_catalog(): parse embedded metadata
parse_embedded_catalog() -> normalize_catalog(): schema + integrity normalization
normalize_catalog() -> PolicyValidator: inspect referenced embedded skills
PolicyValidator --> normalize_catalog(): ok / policy error
normalize_catalog() --> parse_embedded_catalog(): resolved catalog / error
parse_embedded_catalog() --> load_catalog(): baseline / failure
load_catalog() --> SuggestionService: usable embedded catalog or explicit initialization error
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/skills/catalog.rs` | Modify | Introduce an embedded-only policy validation step after normalization, add a small approval predicate/allowlist for approved official external sources, and return an explicit embedded catalog error when a referenced recommendation violates policy. |
| `src/skills/catalog.v1.toml` | Modify | Remove the four disallowed `[[skills]]` entries and repoint affected technologies to existing policy-compliant local skills so the embedded catalog stays valid. |
| `tests/unit/suggest_catalog.rs` | Modify | Add focused regression tests for allowed embedded external recommendations, rejected disallowed embedded external recommendations, and cleaned baseline mappings. |
| `openspec/changes/external-skill-catalog-policy-cleanup/state.yaml` | Modify | Mark the design phase complete and point the workflow to `tasks`. |

## Interfaces / Contracts

### Embedded catalog policy hook

The existing loader contract stays intact; the change is an internal validation addition.

```rust
enum CatalogOrigin {
    Embedded,
    Provider,
}

fn validate_embedded_external_recommendation_policy(
    origin: CatalogOrigin,
    skill_definitions: &BTreeMap<String, CatalogSkillDefinition>,
    technologies: &BTreeMap<TechnologyId, CatalogTechnologyEntry>,
    combos: &BTreeMap<String, CatalogComboEntry>,
) -> Result<()>;
```

Expected behavior:
- `CatalogOrigin::Embedded`: enforce approved-source validation on referenced skills
- `CatalogOrigin::Provider`: no-op for this change

### Approved-source rule shape

The validator should treat recommendation references in three buckets:

```rust
enum EmbeddedRecommendationSource {
    LocalCurated,      // provider_skill_id starts with "dallay/agents-skills/"
    ApprovedExternal,  // explicit approved official external source/id
    DisallowedExternal,
}
```

For this change, the approval logic should stay simple and explicit:
- local curated entries remain allowed by prefix
- external entries are allowed only if they match the checked-in approved official source policy
- everything else is rejected for embedded recommendations

## Catalog Data Changes

The checked-in TOML cleanup is intentionally narrow:

1. Delete these `[[skills]]` definitions:
   - `wshobson/agents/nodejs-backend-patterns`
   - `sickn33/antigravity-awesome-skills/nodejs-best-practices`
   - `wshobson/agents/typescript-advanced-types`
   - `biomejs/biome` (`local_skill_id = "biome-linter"`)
2. Update affected technology mappings so they still satisfy `technology.skills` non-empty validation:
   - `node` → replace both removed references with `dallay/agents-skills/best-practices`
   - `typescript` → replace the removed reference with `dallay/agents-skills/best-practices`
   - `biome` → replace the removed reference with `dallay/agents-skills/prettier-formatting`
3. Rely on existing normalization to fail if any hidden technology or combo reference still points at one of the removed IDs.

These replacements are deliberately conservative: they keep the affected technology entries valid without introducing new curated skills or expanding the external approval set.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | Approved official external embedded recommendation is accepted | Add an inline `parse_embedded_catalog()` fixture in `tests/unit/suggest_catalog.rs` using one approved official external skill and assert the catalog loads successfully. |
| Unit | Disallowed external embedded recommendation fails explicitly | Add an inline fixture that references one of the blocked patterns (for example `wshobson/agents/nodejs-backend-patterns`) and assert the error message is a policy failure, not a silent skip. |
| Unit | Cleaned embedded baseline no longer contains removed skills | Load `EmbeddedSkillCatalog::default()` and assert the four removed local/provider IDs are absent from skill definitions and are not referenced by `node`, `typescript`, or `biome`. |
| Unit | Cleanup preserves valid technology mappings | Assert `node`, `typescript`, and `biome` still exist in the embedded catalog and each has a non-empty policy-compliant `skills` list after cleanup. |
| Regression | Provider overlay semantics stay unchanged | Keep existing provider overlay tests in `tests/unit/suggest_catalog.rs` green as the non-regression proof that lenient provider behavior was not altered. |

The new tests should be targeted and policy-based. Count-only assertions like `expanded_catalog_has_minimum_expected_counts()` are not enough to guard this requirement by themselves.

## Migration / Rollout

No migration required. The catalog is checked in, loaded at runtime, and this change only adjusts embedded metadata plus validation. Rollout is immediate once the updated catalog and tests land.

## Open Questions

None. The proposal and spec are narrow enough to implement directly.
