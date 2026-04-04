# Proposal: Fix suggest --install using wrong skill ID for provider resolution

## Intent

`skill suggest --install --all` fails for most catalog-defined skills because `install_selected_with()` resolves skills using the `local_skill_id` (e.g., `"accessibility"`) instead of the `provider_skill_id` (e.g., `"dallay/agents-skills/accessibility"`). The provider's slash-count routing sends all local IDs to the skills.sh search API fallback instead of deterministic GitHub URL construction, causing install failures or wrong skill downloads.

The root cause is that `CatalogSkillMetadata` drops the `provider_skill_id` when building suggestions, and `SkillSuggestion` has no field to carry it through to install time.

## Scope

### In Scope
- Add `provider_skill_id` field to `CatalogSkillMetadata`
- Add `provider_skill_id` field to `SkillSuggestion` and thread it through `new()`
- Use `provider_skill_id` in `install_selected_with()` for `provider.resolve()` calls
- Add `provider_skill_id` to `SuggestJsonRecommendation` JSON output
- Update unit test mocks to reflect the new field

### Out of Scope
- Changing the provider's `resolve()` slash-count routing logic
- Modifying the skill registry format (it correctly uses `local_skill_id` for folder names)
- Changing the interactive selection UI (it correctly displays `local_skill_id`)
- Refactoring the overall suggest/install pipeline architecture

## Approach

Thread the `provider_skill_id` from `CatalogSkillDefinition` through the recommendation pipeline:

1. **`CatalogSkillMetadata`** — add `provider_skill_id: String` field; populate from `definition.provider_skill_id` at lines 297 and 333 in `catalog.rs`
2. **`SkillSuggestion`** — add `provider_skill_id: String` field; populate from `metadata.provider_skill_id` in `new()`
3. **`install_selected_with()`** — change line 370 from `provider.resolve(&recommendation.skill_id)` to `provider.resolve(&recommendation.provider_skill_id)`
4. **`SuggestJsonRecommendation`** — add `provider_skill_id` field so JSON consumers can see the qualified ID
5. **Tests** — update mock provider `sources` maps to use `provider_skill_id` values for resolve lookups

The `skill_id` field remains the `local_skill_id` everywhere else: display, registry keys, folder names, installed-state lookups.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/skills/catalog.rs` | Modified | Add `provider_skill_id` to `CatalogSkillMetadata`; populate at lines 297 and 333 |
| `src/skills/suggest.rs` | Modified | Add `provider_skill_id` to `SkillSuggestion` and `SuggestJsonRecommendation`; use in `install_selected_with()` line 370 |
| `tests/` (suggest/install tests) | Modified | Update mock provider resolve maps to key by `provider_skill_id` |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| JSON output shape change (new `provider_skill_id` field in recommendations) | High (intentional) | Additive-only change; existing fields unchanged; update contract tests if present |
| Test mock breakage (resolve calls receive provider_skill_id instead of local_skill_id) | High (intentional) | Update mock provider `sources` maps to use provider_skill_id keys |
| Display regression (UI accidentally shows provider_skill_id instead of local_skill_id) | Low | `skill_id` field remains `local_skill_id`; only `provider.resolve()` call changes |

## Rollback Plan

Revert changes to the 3 files (`catalog.rs`, `suggest.rs`, test files). No data migration needed — the installed-skill registry uses `local_skill_id` which remains unchanged. The JSON output `provider_skill_id` field simply disappears on revert.

## Dependencies

- None. All changes are internal to the existing codebase with no external dependency additions.

## Success Criteria

- [ ] `skill suggest --install --all` successfully installs catalog-defined skills using deterministic GitHub URL construction (not skills.sh fallback)
- [ ] `provider.resolve()` receives the qualified `provider_skill_id` (e.g., `"dallay/agents-skills/accessibility"`)
- [ ] `skill_id` in display output, registry keys, and folder names remains the `local_skill_id`
- [ ] JSON output includes both `skill_id` (local) and `provider_skill_id` (qualified) in recommendation entries
- [ ] All existing tests pass after mock updates
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes clean
