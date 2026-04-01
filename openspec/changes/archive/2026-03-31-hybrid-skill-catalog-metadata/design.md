# Design: Hybrid Skill Catalog Metadata

## Technical Approach

This change keeps repository technology detection in Rust and moves only recommendation metadata
into a declarative embedded catalog. The embedded fallback becomes a checked-in TOML document loaded
with `include_str!`, normalized into runtime catalog types, and then optionally overlaid by provider
metadata using stable entry IDs.

The new catalog shape is intentionally technology-centric and combo-centric rather than
rule-centric. It should read like a declarative equivalent of `SKILLS_MAP` / `COMBO_SKILLS_MAP`:
technology entries declare which skills are recommended for a detected technology, and combo entries
declare which skills are recommended for a named set of required technologies. Runtime detection
stays in `src/skills/detect.rs`; catalog files do not become the source of truth for detection logic
in phase 1.

To keep current install, registry, and JSON contracts stable, recommendation metadata may carry
canonical provider IDs such as `antfu/skills/vue`, but suggestion output and installed-state lookup
continue using the current local `skill_id`/slug model. The loader therefore needs an explicit
normalization step that separates canonical recommendation identity from the local runtime/install
key.

## Architecture Decisions

### Decision: Use a technology/combo catalog shape instead of per-rule rows

**Choice**: Represent the embedded fallback and provider overlay as catalog documents with
`technologies` entries and `combos` entries, each carrying stable IDs and a `skills` list.

**Alternatives considered**: Keep the current flat `rules` list; keep hardcoded Rust maps; introduce
a fully generic boolean rule DSL now.

**Rationale**: The product goal is easier catalog authoring. A map-like shape makes additions such
as `vue -> antfu/skills/vue` obvious, keeps multiple recommendations per technology easy to review,
and mirrors how maintainers think about curated recommendations. It also gives combos a first-class
schema slot without forcing phase-1 runtime support.

### Decision: Keep detection logic authoritative in Rust

**Choice**: `src/skills/detect.rs` remains the only active detection engine. Catalog metadata may
reserve optional future `detect` fields on technology entries, but phase 1 does not execute or
depend on them.

**Alternatives considered**: Move detection markers into metadata now; derive detections from
catalog files; let provider metadata change supported detections.

**Rationale**: The existing detector is already deterministic, tested, and separate from
recommendation generation. Keeping detection in Rust avoids expanding scope into file-system marker
semantics while still leaving room for future authoring hints or documentation in catalog files.

### Decision: Keep hybrid behavior as embedded baseline plus provider overlay

**Choice**: Always load the embedded catalog first, then merge provider metadata by stable catalog
entry IDs. Missing, unreadable, or top-level-invalid provider metadata falls back to embedded only;
partially invalid provider metadata contributes only its valid entries.

**Alternatives considered**: Provider replaces the whole catalog; provider-only catalogs; fail
closed whenever provider metadata is imperfect.

**Rationale**: The product requires offline fallback and predictable behavior. Overlay semantics
preserve baseline recommendations while still allowing provider extension and targeted overrides.

### Decision: Use canonical provider IDs in recommendation metadata, but preserve local runtime keys

**Choice**: The `skills` references inside technology/combo entries should point at canonical
provider IDs when known (for example `antfu/skills/vue`). A separate normalized skill-definition
layer must carry the local phase-1 `skill_id` used for CLI output, install resolution input,
`.agents/skills/{skill_id}`, and registry keys.

**Alternatives considered**: Keep catalog `skills` values as local slugs only; replace all
runtime/install/storage keys with canonical IDs immediately; infer local IDs purely from the
basename of canonical IDs.

**Rationale**: Canonical IDs are the right authoring identity for future provider-backed catalogs
and make opinionated mappings portable across providers. However, current code explicitly rejects
slash-delimited install IDs in `src/commands/skill.rs`, installs into `.agents/skills/{skill_id}` in
`src/skills/install.rs`, validates manifest names as slugs in `src/skills/manifest.rs`, and stores
installed state by local key in `src/skills/registry.rs`. The runtime must therefore preserve a
local alias and MUST NOT rely on basename inference alone because canonical IDs can collide.

### Decision: Persist combo entries in schema now, but gate evaluation

**Choice**: Add first-class `combos` entries to the schema now, validate and merge them now, and
treat combo evaluation as inactive by default in phase 1. Runtime evaluation can later be enabled
behind an explicit code gate or follow-on change without reshaping the catalog again.

**Alternatives considered**: Omit combos entirely until evaluation exists; implement live combo
matching now; overload technology entries to encode combos indirectly.

**Rationale**: This preserves schema stability and lets catalog authors define future opinionated
mappings early, while keeping the current implementation aligned with the phase-1 requirement that
simple technology-driven matching remains authoritative.

## Data Flow

### Catalog load and merge flow

```text
catalog.v1.toml --include_str!--> RawCatalogDocument
                                      |
                                      v
                        normalize_catalog("embedded")
                                      |
                                      v
                        Resolved baseline catalog
                                      |
                     provider.recommendation_catalog()?
                     /                |                \
                    /                 |                 \
               none/error      invalid top-level      metadata
                  |                  |                  |
                  v                  v                  v
          use baseline only   use baseline only   normalize_catalog("provider")
                                                        |
                                                        v
                                      keep valid entries, warn on invalid ones
                                                        |
                                                        v
                                     overlay_catalogs(baseline, provider)
                                                        |
                                                        v
                                            Resolved hybrid catalog
```

### Suggestion flow in phase 1

```text
RepoDetector (Rust) -> [TechnologyDetection]
                           |
                           v
      lookup technology entries by detected technology id
                           |
                           v
        expand referenced skills into recommendation candidates
                           |
                           +--> combo entries loaded but not evaluated by default
                           |
                           v
       dedupe by local skill_id, annotate installed state, emit output
```

### Sequence diagram: baseline and overlay loading

```text
SuggestionService -> CatalogLoader: load_catalog(provider)
CatalogLoader -> EmbeddedCatalog: parse embedded TOML
EmbeddedCatalog --> CatalogLoader: normalized baseline
CatalogLoader -> Provider: recommendation_catalog()
Provider --> CatalogLoader: optional metadata / error / none
CatalogLoader -> ProviderCatalog: normalize valid entries only
ProviderCatalog --> CatalogLoader: normalized overlay + warnings
CatalogLoader -> CatalogLoader: overlay by stable ids
CatalogLoader --> SuggestionService: resolved catalog
SuggestionService -> RepoDetector: detect(project_root)
RepoDetector --> SuggestionService: detections
SuggestionService -> SuggestEngine: recommend from detections + catalog
SuggestEngine --> SuggestionService: deduped recommendations
```

### Sequence diagram: recommendation identity handling

```text
Catalog entry.skills -> canonical provider skill ids
Normalizer -> SkillDefinitions: resolve metadata for each canonical id
SkillDefinitions --> Normalizer: canonical id + local skill_id alias
SuggestEngine -> InstalledState: lookup by local skill_id
SuggestEngine -> Provider.resolve(): use local skill_id in phase 1 flows
SuggestEngine --> JSON/CLI: emit local skill_id only
```

## File Changes

| File                                           | Action     | Description                                                                                                                                                  |
|------------------------------------------------|------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `src/skills/catalog.v1.toml`                   | Create     | Declarative embedded fallback catalog with `technologies`, `combos`, and skill-definition metadata.                                                          |
| `src/skills/catalog.rs`                        | Modify     | Replace hardcoded embedded data with TOML loading, normalization, overlay merge logic, technology/combo expansion, and local-vs-canonical identity handling. |
| `src/skills/provider.rs`                       | Modify     | Update provider metadata structs to the new technology/combo catalog shape and preserve optional overlay behavior.                                           |
| `src/skills/suggest.rs`                        | Modify     | Consume normalized catalog recommendations while keeping output, dedupe, installed-state lookup, and install selection keyed by local `skill_id`.            |
| `src/commands/skill.rs`                        | Referenced | Continues validating direct install IDs as single path-safe segments; no phase-1 change to accept canonical slash-delimited IDs.                             |
| `src/skills/install.rs`                        | Referenced | Continues installing into `.agents/skills/{skill_id}` using the local alias.                                                                                 |
| `src/skills/registry.rs`                       | Referenced | Continues storing installed state keyed by the local alias.                                                                                                  |
| `src/skills/manifest.rs`                       | Referenced | Continues validating manifest `name` as a slug distinct from canonical provider IDs.                                                                         |
| `tests/unit/suggest_catalog.rs`                | Modify     | Update tests for embedded parsing, overlay semantics, technology/combo normalization, canonical/local identity handling, and deferred combo evaluation.      |
| `tests/integration/skill_suggest.rs`           | Modify     | Verify baseline-only and provider-overlay suggest flows preserve output/install compatibility.                                                               |
| `tests/contracts/test_skill_suggest_output.rs` | Modify     | Protect the current JSON contract and baseline recommendations for supported technologies.                                                                   |

## Interfaces / Contracts

### Declarative catalog schema

The catalog should read as recommendation metadata, not as detection logic:

```toml
version = "v1"

[[skills]]
provider_skill_id = "antfu/skills/vue"
local_skill_id = "vue"
title = "Vue"
summary = "Opinionated Vue workflow guidance."

[[skills]]
provider_skill_id = "frontend-design"
local_skill_id = "frontend-design"
title = "Frontend Design"
summary = "Create polished, production-grade frontend interfaces."

[[technologies]]
id = "vue"
name = "Vue"
skills = ["antfu/skills/vue"]

[technologies.detect]
# reserved for future metadata only; ignored by runtime in phase 1
markers = ["package.json", "vite.config.ts"]

[[technologies]]
id = "astro"
name = "Astro"
min_confidence = "medium"
reason_template = "Recommended because {technology} was detected from {evidence}."
skills = [
  "frontend-design",
  "accessibility",
  "performance",
  "core-web-vitals",
  "seo",
]

[[combos]]
id = "astro-github-actions"
name = "Astro + GitHub Actions"
requires = ["astro", "github_actions"]
skills = ["github-actions"]
enabled = false # optional runtime gate for deferred evaluation
```

### Normalized runtime model

```rust
pub struct CatalogSkillDefinition {
    pub provider_skill_id: String,
    pub local_skill_id: String,
    pub title: String,
    pub summary: String,
}

pub struct CatalogTechnologyEntry {
    pub id: TechnologyId,
    pub name: String,
    pub detect: Option<toml::Value>,
    pub skills: Vec<String>,
    pub min_confidence: DetectionConfidence,
    pub reason_template: String,
}

pub struct CatalogComboEntry {
    pub id: String,
    pub name: String,
    pub requires: Vec<TechnologyId>,
    pub skills: Vec<String>,
    pub enabled: bool,
    pub reason_template: Option<String>,
}

pub struct ResolvedSkillCatalog {
    pub source_name: String,
    pub metadata_version: String,
    pub skill_definitions: BTreeMap<String, CatalogSkillDefinition>, // keyed by provider_skill_id
    pub technologies: BTreeMap<TechnologyId, CatalogTechnologyEntry>,
    pub combos: BTreeMap<String, CatalogComboEntry>,
}
```

### Merge semantics

```text
Skill-definition overlay key: provider_skill_id
Technology overlay key:       technology.id
Combo overlay key:            combo.id

Precedence:
1. Embedded baseline always loads first.
2. Provider metadata may add new skills, technologies, and combos.
3. Provider metadata may override matching entries with the same stable key.
4. Provider metadata may not delete embedded entries in phase 1.
5. Invalid provider entries are skipped individually; valid entries still merge.
6. Technology/combo entries that reference unknown skills are skipped with warnings.
7. Combo entries may be present in the resolved catalog even when runtime evaluation is disabled.
8. If provider metadata is absent or unusable at the top level, embedded remains authoritative.
```

### Validation rules

Validation should enforce:

- `version` MUST match the supported schema version.
- every `skills.provider_skill_id` and `skills.local_skill_id` MUST be non-empty.
- every `skills.local_skill_id` MUST remain a single path-safe segment compatible with current
  install/storage code.
- canonical `provider_skill_id` values MAY contain `/` because they are metadata, not filesystem
  keys.
- every technology `id` used by active recommendation matching MUST map to a known `TechnologyId`
  enum value.
- every technology entry MUST include `id`, `name`, and at least one referenced skill.
- every combo entry MUST include `id`, `name`, `requires`, and at least one referenced skill.
- every referenced skill in `technologies.skills` or `combos.skills` MUST resolve to a known skill
  definition.
- duplicate `local_skill_id` aliases across different canonical provider IDs MUST be rejected unless
  explicitly allowed by a future migration, because phase-1 installed-state lookup keys are local.
- `detect` metadata MUST be ignored by runtime matching in phase 1.
- combo entries MAY be loaded now even if evaluation is disabled.

## Testing Strategy

| Layer       | What to Test                         | Approach                                                                                                                                                                       |
|-------------|--------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Unit        | Embedded catalog parse and normalize | Load the checked-in TOML file and assert the current v1 mappings normalize into expected technology entries, combo entries, and skill definitions.                             |
| Unit        | Canonical/local identity separation  | Assert canonical `provider_skill_id` values can contain `/` while local `skill_id` aliases remain slug-safe and are used for recommendation output and installed-state lookup. |
| Unit        | Overlay precedence                   | Verify provider skill-definition, technology, and combo entries add or override by stable IDs without deleting embedded entries.                                               |
| Unit        | Partial-invalid provider metadata    | Confirm invalid provider entries are skipped individually while valid entries still overlay the baseline.                                                                      |
| Unit        | Deferred combo evaluation            | Confirm combos can normalize and merge successfully while suggestion output remains unchanged when combo evaluation is disabled.                                               |
| Unit        | Recommendation compatibility         | Assert Rust, Docker, Astro, GitHub Actions, Make, Node/TypeScript, and Python detections still yield materially equivalent phase-1 recommendations and dedupe behavior.        |
| Integration | Suggest with embedded fallback only  | Run suggest flows without provider metadata and verify stable detections, recommendations, and installed annotations.                                                          |
| Integration | Suggest with provider overlay        | Use fake provider metadata to verify added/overridden technology mappings while preserving local install/storage behavior.                                                     |
| Contract    | JSON output stability                | Keep `detections`, `recommendations`, `summary`, and recommendation `skill_id` contract unchanged for phase 1.                                                                 |

## Migration / Rollout

1. Create `src/skills/catalog.v1.toml` by transcribing the current hardcoded baseline into
   technology-centric entries, plus explicit skill-definition records for every referenced skill.
2. Introduce raw and normalized catalog types in `src/skills/catalog.rs`, including the
   canonical-to-local identity bridge.
3. Switch `load_catalog()` from replace-or-fallback behavior to embedded-baseline-plus-overlay
   behavior.
4. Keep suggestion output, installed-state lookup, and installation keyed by local `skill_id`
   aliases.
5. Load and validate combo entries now, but keep combo evaluation disabled by default until a later
   change explicitly enables it.
6. Add compatibility tests before or with the migration to catch any user-visible drift.

No separate data migration is required in phase 1 because installed registries and on-disk skill
directories continue using the existing local keys.

## Open Questions

- [ ] Should the user-visible `catalog_source` remain a single label such as `embedded:v1` /
  `skills.sh:2026.03`, or should merged catalogs report a composite source label?
- [ ] Should embedded v1 skill definitions require explicit `local_skill_id` for every canonical
  provider ID, or may canonical-only entries be allowed only when
  `provider_skill_id == local_skill_id`?
- [ ] Should combo evaluation be gated by a compile-time constant, a runtime feature flag, or simply
  deferred by not calling the combo evaluator yet in phase 1?
