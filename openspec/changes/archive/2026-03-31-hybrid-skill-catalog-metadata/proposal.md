# Proposal: Hybrid Skill Catalog Metadata

## Intent

AgentSync's repository technology suggestion feature is currently extensible in code structure but
not in catalog authoring: repository detection is cleanly separated in Rust, while the embedded
recommendation catalog is still hardcoded in Rust values. That makes recommendation changes harder
to review, slower to extend, and awkward to evolve toward richer provider-backed metadata.

This change will keep repository technology detection in Rust while moving the embedded
recommendation catalog into declarative metadata and redefining catalog loading as a hybrid model:
an embedded declarative fallback baseline plus optional provider metadata overlay. The goal is to
preserve current user-visible recommendations for already supported technologies while making the
catalog easier to extend and leaving a clear future path for canonical provider identifiers and
combo-rule matching.

## Desired Outcome

- Recommendation mappings can be updated through declarative metadata instead of hardcoded Rust
  tables.
- The system uses a hybrid model: embedded declarative metadata is always available as the
  offline/local fallback, and provider metadata can optionally extend or override it.
- Existing supported technologies continue to produce materially the same detections and
  recommendations.
- The metadata model can represent multiple opinionated skills per technology.
- The model reserves room for future combo rules without implementing combo matching in this phase.
- Canonical skills.sh identifiers are introduced only where low-risk for phase 1.

## Scope

### In Scope

- Replace the embedded recommendation catalog with a checked-in declarative metadata file loaded by
  Rust at runtime via embedded source inclusion.
- Define a hybrid catalog model where embedded metadata is the baseline fallback and provider
  metadata is an optional overlay.
- Define deterministic merge behavior for missing, invalid, partially valid, and fully valid
  provider metadata.
- Preserve current recommendation output semantics for the existing v1 supported technologies: Rust,
  TypeScript/Node, Astro, GitHub Actions, Docker, Make, and Python.
- Support multiple recommendation rules per detected technology in declarative metadata.
- Extend metadata schema to carry an optional canonical/provider skill identifier such as
  `antfu/skills/vue` without requiring local install/storage to use that identifier yet.
- Identify affected Rust modules and contracts that specs/design must cover.

### Out of Scope

- Rewriting repository technology detection logic.
- Replacing existing install/update/uninstall storage semantics with canonical slash-delimited IDs.
- Changing local manifest `name` validation semantics.
- Implementing combo-rule evaluation such as `all_of`, `not`, or weighted matching.
- Expanding supported technologies beyond the current v1 set.
- Changing default user-visible recommendation output unless required for forward-compatible
  metadata representation.

## Approach

Use a declarative embedded catalog with provider overlay.

1. **Keep detection in Rust**
    - `src/skills/detect.rs` remains the source of truth for repository technology detection.
    - Recommendation generation continues consuming detections instead of re-scanning repositories.

2. **Move embedded catalog to declarative metadata**
    - Encode the current embedded skills and rules in a checked-in metadata file (TOML preferred).
    - Load it through `include_str!` and deserialize into catalog structs used by suggestion logic.
    - Keep the initial embedded metadata semantically equivalent to today's hardcoded catalog to
      minimize contract churn.

3. **Adopt a hybrid loading model**
    - Embedded metadata is always the baseline and fallback.
    - Provider metadata is optional and additive/overriding, not full replacement.
    - Expected behavior:
        - provider unavailable/missing/invalid at top level -> use embedded baseline only;
        - provider partially invalid -> ignore invalid provider entries, keep valid ones, preserve
          embedded baseline;
        - provider valid -> merge with embedded baseline using explicit precedence rules.

4. **Define stable merge semantics**
    - Skills and rules need stable identifiers/keys in metadata so overlay behavior is
      deterministic.
    - Provider metadata may:
        - add new skills/rules;
        - override explicitly matching embedded entries;
        - otherwise leave embedded entries unchanged.
    - This phase will not introduce provider-driven deletion/disable behavior unless clearly
      justified in specs/design.

5. **Handle canonical skill identifiers conservatively**
    - **Phase 1:** keep current local `skill_id` behavior as the recommendation/install key exposed
      by existing flows.
    - Add an optional metadata field for canonical/provider identity (for example
      `canonical_skill_id` or `provider_skill_id`) so provider-backed lookups can reference
      `antfu/skills/vue`.
    - **Later phase:** evaluate whether local install paths, registry keys, validation, and CLI UX
      can safely migrate to canonical IDs end-to-end.

6. **Prepare but do not implement combo rules**
    - Shape the metadata/rule schema so a later version can express richer match logic.
    - Phase 1 continues using today's simple per-technology recommendation behavior.

## Affected Areas

| Area                                                     | Impact              | Description                                                                                                                          |
|----------------------------------------------------------|---------------------|--------------------------------------------------------------------------------------------------------------------------------------|
| `src/skills/catalog.rs`                                  | Modified            | Load declarative embedded metadata, validate it, and apply hybrid merge behavior.                                                    |
| `src/skills/provider.rs`                                 | Modified            | Clarify provider catalog metadata schema and overlay semantics.                                                                      |
| `src/skills/suggest.rs`                                  | Modified            | Consume hybrid catalog results while preserving current recommendation output behavior.                                              |
| `src/skills/detect.rs`                                   | Referenced          | Detection stays in Rust and remains separate from recommendation mapping.                                                            |
| `src/commands/skill.rs`                                  | Referenced/Modified | May need limited adjustment if canonical provider IDs are surfaced in metadata handling, while preserving current local ID behavior. |
| `src/skills/install.rs`                                  | Referenced          | Local install path behavior remains unchanged in phase 1 and must not assume slash-delimited IDs.                                    |
| `src/skills/registry.rs`                                 | Referenced          | Installed-state keys remain aligned to current local `skill_id` semantics in phase 1.                                                |
| `src/skills/manifest.rs`                                 | Referenced          | Manifest naming remains decoupled from provider canonical identifiers.                                                               |
| `tests/unit/suggest_catalog.rs`                          | Modified            | Validate metadata loading, fallback behavior, and overlay semantics.                                                                 |
| `tests/contracts/test_skill_suggest_output.rs`           | Modified            | Protect current JSON/output contract for existing technologies.                                                                      |
| `tests/integration/skill_suggest.rs`                     | Modified            | Verify end-to-end suggestion behavior with embedded fallback and provider metadata.                                                  |
| `src/skills/*.toml` or equivalent embedded metadata path | New                 | Declarative embedded recommendation catalog source.                                                                                  |

## Risks

| Risk                                                                         | Likelihood | Mitigation                                                                                           |
|------------------------------------------------------------------------------|------------|------------------------------------------------------------------------------------------------------|
| Canonical IDs break install paths or registry semantics if adopted too early | High       | Keep canonical/provider IDs as metadata-only in phase 1; preserve current local `skill_id` behavior. |
| Provider overlay semantics become ambiguous                                  | Medium     | Define stable keys and explicit precedence rules in specs/design before implementation.              |
| Partial provider metadata causes silent recommendation drift                 | Medium     | Require deterministic fallback/skip behavior and coverage for partial-invalid cases.                 |
| Declarative catalog migration changes existing outputs unintentionally       | Medium     | Encode current embedded rules first and protect behavior with unit, integration, and contract tests. |
| Future combo-rule needs distort phase 1 scope                                | Low        | Reserve schema space only; defer match-engine changes to later work.                                 |

## Rollout

1. Land embedded declarative catalog and loader behind the current recommendation behavior.
2. Preserve current outputs for supported technologies using compatibility tests.
3. Introduce hybrid provider overlay semantics with deterministic fallback handling.
4. Carry optional canonical/provider skill identifiers in metadata without changing local
   install/storage semantics.
5. Reassess canonical end-to-end migration and combo-rule support in follow-on changes after phase 1
   stabilizes.

## Rollback Plan

If the declarative catalog or overlay semantics produce unstable or regressive recommendation
behavior, revert catalog loading to the current embedded Rust catalog implementation and disable
provider overlay logic while retaining the existing detection flow. Because phase 1 preserves
current local install and registry semantics, rollback should be limited to catalog loading and
recommendation mapping code plus associated metadata/tests.

## Dependencies

- Existing exploration artifact:
  `openspec/changes/2026-03-31-hybrid-skill-catalog-metadata/exploration.md`
- Existing main spec: `openspec/specs/skill-recommendations/spec.md`
- Current provider seam and installed-state behavior in the Rust skill recommendation pipeline

## Success Criteria

- [ ] Proposal gives specs/design a clear phase-1 direction: declarative embedded catalog plus
  optional provider overlay.
- [ ] Phase 1 explicitly preserves current user-visible recommendation behavior for existing
  supported technologies as much as possible.
- [ ] Phase 1 explicitly limits canonical skills.sh IDs to metadata/provider identity unless a later
  change broadens local install/storage semantics.
- [ ] Proposal identifies affected modules, fallback behavior, merge semantics, and rollout
  constraints needed for implementation.
- [ ] Proposal reserves a future path for combo rules without committing phase-1 implementation
  scope to them.
