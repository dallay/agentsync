## Exploration: hybrid skill catalog metadata

### Current State

Repository technology detection is already cleanly separated in Rust via `src/skills/detect.rs`, and
recommendation generation already crosses a catalog boundary in `src/skills/catalog.rs` +
`src/skills/suggest.rs`. Today the embedded fallback is not declarative:
`EmbeddedSkillCatalog::default()` hardcodes `skills` and `rules` in Rust, while the provider seam is
`Provider::recommendation_catalog()` returning `ProviderCatalogMetadata` from
`src/skills/provider.rs`.

The current provider-backed behavior is effectively **replace-or-fallback**, not hybrid overlay.
`load_catalog()` uses the provider catalog only when `recommendation_catalog()` returns usable
metadata; otherwise it falls back to the embedded catalog. Within provider metadata, invalid rules
are skipped, but if no usable rules remain the whole provider catalog is discarded.

Current skill IDs are still optimized for local filesystem-safe slugs, not canonical provider
identifiers. Evidence:

- `validate_skill_id()` in `src/commands/skill.rs` rejects `/` and `\\`.
- `install_from_dir()` installs into `.agents/skills/{skill_id}`, so canonical IDs like
  `antfu/skills/vue` would create nested paths.
- registry keys are stored by the passed `skill_id` in `src/skills/registry.rs`.
- `SkillsShProvider.resolve()` already accepts either exact canonical matches or basename matches,
  so provider lookup is more flexible than local install/storage.

For embedded declarative metadata, the repo already ships both `toml` and `serde_yaml`, but TOML is
the better fit: it is already heavily used across the repo, supports comments, has stricter
structure than YAML, and works well with `include_str!` + one-time deserialization into the same
in-memory catalog structs.

The current model already supports multiple recommendations per technology because several rules can
target the same technology (`Astro` maps to multiple skills). It does **not** yet model richer
combinations such as “Kotlin + Coroutines” because each rule is just a flat
`technologies: Vec<TechnologyId>` list matched as independent any-of triggers.

### Affected Areas

- `src/skills/catalog.rs` — primary place to replace hardcoded Rust catalog data with declarative
  embedded metadata and hybrid merge logic.
- `src/skills/provider.rs` — current provider catalog schema; likely needs explicit
  overlay/precedence semantics and possibly canonical/provider ID fields.
- `src/skills/suggest.rs` — suggestion model currently assumes one `skill_id` string serves
  recommendation, install lookup, and installed-state matching.
- `src/commands/skill.rs` — direct install validation currently rejects canonical IDs with `/`.
- `src/skills/install.rs` — local install directory layout uses raw `skill_id` as a path segment.
- `src/skills/registry.rs` — installed-state keys are coupled to the current short ID model.
- `src/skills/manifest.rs` — manifest `name` validation remains slug-based and should stay decoupled
  from provider canonical IDs.
- `tests/unit/suggest_catalog.rs` / `tests/contracts/test_skill_suggest_output.rs` /
  `tests/integration/skill_suggest.rs` — existing behavior locks in current fallback, JSON, and
  supported-technology expectations that the migration should preserve.

### Approaches

1. **Declarative embedded catalog with provider overlay** — Move the embedded catalog into a
   checked-in TOML metadata file, load it locally in Rust, and merge optional provider metadata on
   top with explicit precedence rules.
    - Pros: Preserves offline fallback, keeps detection in Rust, minimizes churn to suggest/install
      flows, preserves current supported technologies, and creates the right seam for future
      provider extension/override.
    - Cons: Requires defining merge semantics now; canonical skill IDs still need a separate
      install/storage strategy.
    - Effort: Medium

2. **Full canonical-ID/provider-first redesign** — Redefine recommendation identities, install IDs,
   registry keys, and direct install validation at the same time, then make provider metadata the
   primary source.
    - Pros: Solves canonical provider IDs end-to-end immediately and leaves less legacy coupling
      behind.
    - Cons: Higher risk, touches install/update/uninstall semantics, and is larger than needed for
      v1 hybrid metadata.
    - Effort: High

### Recommendation

Use **declarative embedded catalog with provider overlay** for v1 of this change.

Recommended shape:

- Keep **technology detection in Rust** exactly where it is.
- Replace the hardcoded embedded catalog with a checked-in declarative file such as
  `src/skills/catalog.v1.toml`, loaded with `include_str!` and deserialized once into raw metadata
  structs.
- Keep the embedded metadata as the **authoritative fallback baseline**.
- Change provider behavior from “full replacement when valid” to **hybrid overlay**:
    - provider missing/unreachable/invalid schema → use embedded only;
    - provider partially invalid → ignore only invalid provider entries/rules and keep embedded
      baseline;
    - provider valid → extend with new skills/rules and override only fields/rules that explicitly
      declare the same stable key.
- Preserve current behavior by encoding the **existing embedded skills/rules unchanged** in the
  first declarative file so current JSON/contracts remain stable for Rust, Docker, Astro, GitHub
  Actions, Make, Node/TypeScript, and Python.
- For canonical skills.sh support, split identity concerns in the metadata model instead of
  overloading one string:
    - `skill_id`: stable recommendation/install key used locally today;
    - `provider_skill_id` (or `canonical_skill_id`): optional canonical external identifier like
      `antfu/skills/vue` used for provider resolution;
    - later, if desired, these can converge, but the current codebase is not ready for canonical IDs
      as raw local path keys.
- For future combo rules, avoid overbuilding v1 but leave room for a richer rule schema by evolving
  rules from today's implicit any-of to something like `match.any_of` / `match.all_of` /
  `match.not`, while only implementing today's simple rules now.

### Risks

- If `skill_id` is changed to canonical IDs too early, install paths, registry keys, and direct
  install validation will break because `/` is currently forbidden or path-significant.
- Overlay semantics can become ambiguous unless the metadata schema explicitly defines whether a
  provider rule extends, overrides, or disables an embedded rule.
- Partial provider data can silently skew recommendations unless invalid entries are logged and
  fallback decisions are deterministic.
- A basename-only fallback for canonical IDs could create collisions (`owner-a/vue` vs
  `owner-b/vue`) if local keys are not modeled explicitly.
- Adding combo-rule schema now without an immediate need could complicate v1 validation and tests.

### Ready for Proposal

Yes — the codebase already has the right detector/catalog split for this change. The proposal should
define the declarative embedded file schema, hybrid overlay/fallback rules, the distinction between
local skill keys and canonical provider IDs, and a narrow v1 migration that preserves the current
recommendation outputs while reserving a later path for combo rules.
