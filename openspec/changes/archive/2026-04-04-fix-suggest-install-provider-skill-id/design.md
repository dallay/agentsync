# Design: Fix suggest --install using wrong skill ID for provider resolution

## Technical Approach

Thread the `provider_skill_id` from `CatalogSkillDefinition` through the recommendation pipeline so that `install_selected_with()` resolves skills using the qualified three-segment ID (e.g., `"dallay/agents-skills/accessibility"`) instead of the local alias (e.g., `"accessibility"`). This ensures `SkillsShProvider::resolve()` takes the deterministic path (slash-count ≥ 2) rather than falling back to the skills.sh search API.

The fix is purely additive: a new `provider_skill_id` field flows alongside the existing `skill_id` (which remains the `local_skill_id` for display, registry, and folder naming). Only the `provider.resolve()` call site changes which ID it passes.

## Architecture Decisions

### Decision: Add `provider_skill_id` to intermediate structs rather than looking it up at install time

**Choice**: Thread `provider_skill_id` through `CatalogSkillMetadata` → `SkillSuggestion` → install call site as a carried field.

**Alternatives considered**: Look up the `provider_skill_id` from the catalog at install time by reverse-mapping from `local_skill_id` → `CatalogSkillDefinition`. This would avoid adding fields but requires passing the catalog into `install_selected_with()`.

**Rationale**: The suggestion pipeline already constructs `CatalogSkillMetadata` from `CatalogSkillDefinition` (which has both IDs). Carrying the field forward is simpler, avoids changing the `install_selected_with()` signature to accept a catalog reference, and keeps the data self-contained in the recommendation. The catalog may not even be available at install time in all call paths.

### Decision: Keep `skill_id` as `local_skill_id` everywhere except `provider.resolve()`

**Choice**: The existing `skill_id` field on `SkillSuggestion`, `SuggestJsonRecommendation`, and `SuggestInstallResult` remains the `local_skill_id`. Only the `provider.resolve()` call switches to `provider_skill_id`.

**Alternatives considered**: Rename `skill_id` to `local_skill_id` across all structs for clarity.

**Rationale**: Renaming would be a breaking change to JSON output and would touch many more files. The `skill_id` field is already understood as the local/display ID by consumers. Adding `provider_skill_id` as a new field is additive-only and non-breaking.

### Decision: Expose `provider_skill_id` in JSON output

**Choice**: Add `provider_skill_id` to `SuggestJsonRecommendation` so JSON consumers can see the qualified ID.

**Alternatives considered**: Keep it internal-only (not serialized).

**Rationale**: External tooling may need the qualified ID for its own resolution logic. Since the field is additive, it doesn't break existing consumers. The proposal explicitly requires this.

## Data Flow

### Current (broken) flow

```
CatalogSkillDefinition          CatalogSkillMetadata           SkillSuggestion
┌─────────────────────┐         ┌──────────────────┐           ┌──────────────┐
│ provider_skill_id ──┼── X ──> │ skill_id (local) │ ────────> │ skill_id     │──> provider.resolve("accessibility")
│ local_skill_id   ───┼────────>│ title            │           │ title        │         │
│ title            ───┼────────>│ summary          │           │ summary      │         ▼
│ summary          ───┼────────>│                  │           │ ...          │    slash_count = 0
└─────────────────────┘         └──────────────────┘           └──────────────┘    → search API fallback
                                                                                   → FAILS ✗
```

### Fixed flow

```
CatalogSkillDefinition          CatalogSkillMetadata           SkillSuggestion
┌─────────────────────┐         ┌──────────────────────┐       ┌────────────────────┐
│ provider_skill_id ──┼────────>│ provider_skill_id ───┼──────>│ provider_skill_id  │──> provider.resolve("dallay/agents-skills/accessibility")
│ local_skill_id   ───┼────────>│ skill_id (local)     │──────>│ skill_id (local)   │         │
│ title            ───┼────────>│ title                │──────>│ title              │         ▼
│ summary          ───┼────────>│ summary              │──────>│ summary            │    slash_count = 2
└─────────────────────┘         └──────────────────────┘       │ ...                │    → deterministic resolve
                                                               └────────────────────┘    → OK ✓
```

### Sequence Diagram: Corrected install flow

```
  User                SuggestionService        recommend_skills()      SkillSuggestion       Provider::resolve()      SkillsShProvider
   │                        │                        │                       │                       │                       │
   │  suggest --install     │                        │                       │                       │                       │
   ├───────────────────────>│                        │                       │                       │                       │
   │                        │  recommend_skills()    │                       │                       │                       │
   │                        ├───────────────────────>│                       │                       │                       │
   │                        │                        │                       │                       │                       │
   │                        │                        │  CatalogSkillMetadata │                       │                       │
   │                        │                        │  { provider_skill_id: │                       │                       │
   │                        │                        │    "dallay/agents-    │                       │                       │
   │                        │                        │     skills/access.."  │                       │                       │
   │                        │                        │    skill_id: "access.."}                      │                       │
   │                        │                        │                       │                       │                       │
   │                        │                        │  SkillSuggestion::new()                       │                       │
   │                        │                        ├──────────────────────>│                       │                       │
   │                        │                        │                       │                       │                       │
   │                        │  Vec<SkillSuggestion>  │  provider_skill_id    │                       │                       │
   │                        │<───────────────────────┤  carried in struct    │                       │                       │
   │                        │                        │                       │                       │                       │
   │                        │  install_selected_with()                       │                       │                       │
   │                        │  for each recommendation:                      │                       │                       │
   │                        │                        │                       │                       │                       │
   │                        │  provider.resolve(recommendation.provider_skill_id)                    │                       │
   │                        ├───────────────────────────────────────────────────────────────────────>│                       │
   │                        │                        │                       │                       │  slash_count >= 2     │
   │                        │                        │                       │                       ├──────────────────────>│
   │                        │                        │                       │                       │  resolve_deterministic│
   │                        │                        │                       │                       │<─────────────────────┤
   │                        │                        │                       │                       │  SkillInstallInfo     │
   │                        │<──────────────────────────────────────────────────────────────────────┤                       │
   │                        │                        │                       │                       │                       │
   │                        │  install_fn(skill_id, url, target)             │                       │                       │
   │                        │  (skill_id = local for folder naming)          │                       │                       │
   │                        │                        │                       │                       │                       │
   │  SuggestInstallJson    │                        │                       │                       │                       │
   │<───────────────────────┤                        │                       │                       │                       │
   │                        │                        │                       │                       │                       │
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/skills/catalog.rs` | Modify | Add `provider_skill_id: String` to `CatalogSkillMetadata` (line 20); populate in `recommend_skills()` at lines 297 and 333; populate in `rebuild_local_skill_index()` at line 731 |
| `src/skills/suggest.rs` | Modify | Add `provider_skill_id: String` to `SkillSuggestion` (line 92); populate in `SkillSuggestion::new()` (line 104); add to `SuggestJsonRecommendation` (line 189); populate in `to_json_response()` (line 450); change `provider.resolve()` call at line 370 to use `recommendation.provider_skill_id` |
| `tests/unit/suggest_install.rs` | Modify | Update `LocalSkillProvider::new()` and `PartiallyFailingProvider` to key `sources` map by `provider_skill_id` values (e.g., `"test/skills/rust-async-patterns"`) so `resolve()` receives the qualified ID |

## Interfaces / Contracts

### Modified: `CatalogSkillMetadata`

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSkillMetadata {
    pub provider_skill_id: String, // NEW — qualified provider ID (e.g., "dallay/agents-skills/accessibility")
    pub skill_id: String,          // unchanged — local alias (e.g., "accessibility")
    pub title: String,
    pub summary: String,
}
```

### Modified: `SkillSuggestion`

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSuggestion {
    pub skill_id: String,            // unchanged — local alias
    pub provider_skill_id: String,   // NEW — qualified provider ID
    pub title: String,
    pub summary: String,
    pub reasons: Vec<String>,
    pub matched_technologies: Vec<TechnologyId>,
    pub installed: bool,
    pub installed_version: Option<String>,
    pub catalog_source: String,
}
```

### Modified: `SuggestJsonRecommendation`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SuggestJsonRecommendation {
    pub skill_id: String,            // unchanged — local alias
    pub provider_skill_id: String,   // NEW — qualified provider ID
    pub matched_technologies: Vec<TechnologyId>,
    pub reasons: Vec<String>,
    pub installed: bool,
}
```

### Key call site change in `install_selected_with()`

```rust
// BEFORE (line 370):
match provider.resolve(&recommendation.skill_id) {

// AFTER:
match provider.resolve(&recommendation.provider_skill_id) {
```

### JSON output change (additive only)

```json
{
  "recommendations": [
    {
      "skill_id": "accessibility",
      "provider_skill_id": "dallay/agents-skills/accessibility",
      "matched_technologies": ["node_typescript"],
      "reasons": ["..."],
      "installed": false
    }
  ]
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `provider.resolve()` receives `provider_skill_id` not `local_skill_id` | Update `LocalSkillProvider` and `PartiallyFailingProvider` in `tests/unit/suggest_install.rs` to key sources by provider-style IDs (e.g., `"test/skills/rust-async-patterns"`). Existing tests will fail if resolve receives wrong ID, validating the fix. |
| Unit | `SkillSuggestion` carries `provider_skill_id` | Assert `provider_skill_id` field is populated correctly after `recommend_skills()` |
| Unit | JSON output includes `provider_skill_id` | Assert serialized `SuggestJsonRecommendation` contains the new field |
| Integration | End-to-end suggest+install flow | Existing `guided_install_only_installs_selected_recommendations` and `install_all_skips_already_installed_recommendations` tests validate the full pipeline after mock updates |

## Migration / Rollout

No migration required. The change is additive:
- No persisted data format changes (registry uses `local_skill_id` which is unchanged)
- JSON output gains a new field (`provider_skill_id`) — additive, non-breaking
- No feature flags needed

## Open Questions

None. The proposal fully specifies the approach and the codebase analysis confirms feasibility.
