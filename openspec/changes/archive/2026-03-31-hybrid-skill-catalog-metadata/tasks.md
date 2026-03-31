# Tasks: Hybrid Skill Catalog Metadata

Deferred in this change: active combo evaluation in `src/skills/suggest.rs` and any canonical install/storage migration in `src/commands/skill.rs`, `src/skills/install.rs`, `src/skills/registry.rs`, and `src/skills/manifest.rs`.

## Phase 1: Embedded Catalog Schema Foundation

- [x] 1.1 Create `src/skills/catalog.v1.toml` with v1 `skills`, `technologies`, and `combos` entries, preserving current baseline mappings for Rust, Node/TypeScript, Astro, GitHub Actions, Docker, Make, and Python.
- [x] 1.2 Update raw catalog/provider document structs in `src/skills/catalog.rs` and `src/skills/provider.rs` to match the new shape: technology `{id,name,skills}` entries and combo `{id,name,requires,skills}` entries plus optional future metadata.
- [x] 1.3 Replace hardcoded embedded catalog construction in `src/skills/catalog.rs` with `include_str!` TOML loading so malformed embedded metadata fails recommendation loading explicitly.

## Phase 2: Normalize, Validate, and Overlay

- [x] 2.1 Add normalized runtime catalog types in `src/skills/catalog.rs` for skill definitions, technology entries, combo entries, and a resolved catalog keyed by provider skill ID, technology ID, and combo ID.
- [x] 2.2 Implement shared normalization/validation in `src/skills/catalog.rs`: require schema version, enforce non-empty IDs, require known skill references, keep `detect` metadata inert in phase 1, and reject duplicate local aliases.
- [x] 2.3 Implement embedded-baseline-plus-provider-overlay merge in `src/skills/catalog.rs`, letting valid provider skills/technologies/combos add or override by stable key while invalid provider entries are skipped individually and never delete baseline entries.

## Phase 3: Recommendation Compatibility Wiring

- [x] 3.1 Update `src/skills/suggest.rs` to map Rust detections to catalog technology entries, expand referenced skills, dedupe by local `skill_id`, and preserve existing reasons, installed-state lookup, and JSON output fields.
- [x] 3.2 Add the canonical-to-local compatibility bridge in `src/skills/catalog.rs`/`src/skills/suggest.rs` so catalog `skills` may use provider IDs while phase-1 output and install/storage flows remain keyed by local aliases.
- [x] 3.3 Load, validate, and merge combo entries in `src/skills/catalog.rs`, but leave combo evaluation disabled/deferred in `src/skills/suggest.rs` for this phase.

## Phase 4: Verification and Documentation

- [x] 4.1 Extend `tests/unit/suggest_catalog.rs` for embedded parse/validation failures, technology/combo normalization, canonical-vs-local mapping, overlay precedence, partial-invalid provider metadata, and deferred combo handling.
- [x] 4.2 Update `tests/integration/skill_suggest.rs` to verify embedded-only and provider-overlay suggestion flows preserve local install compatibility and supported-technology recommendations.
- [x] 4.3 Update `tests/contracts/test_skill_suggest_output.rs` to lock the existing `detections`/`recommendations`/`summary` JSON contract and materially equivalent baseline recommendations when no valid override exists.
- [x] 4.4 Update `website/docs/src/content/docs/guides/skills.mdx` and related CLI docs to describe the embedded fallback catalog, provider overlay behavior, and deferred combo/canonical-ID migration scope.
