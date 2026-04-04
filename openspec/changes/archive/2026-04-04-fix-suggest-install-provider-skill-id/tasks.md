# Tasks: Fix suggest --install using wrong skill ID for provider resolution

## Phase 1: Infrastructure (struct changes)

- [x] 1.1 Add `provider_skill_id: String` field to `CatalogSkillMetadata` in `src/skills/catalog.rs` (~line 20)
- [x] 1.2 Add `provider_skill_id: String` field to `SkillSuggestion` in `src/skills/suggest.rs` (~line 92)
- [x] 1.3 Add `provider_skill_id: String` field to `SuggestJsonRecommendation` in `src/skills/suggest.rs` (~line 189)

## Phase 2: Implementation (wiring & population)

- [x] 2.1 Populate `provider_skill_id` from `CatalogSkillDefinition` in `recommend_skills()` at the technology-match branch (~line 297 in `catalog.rs`)
- [x] 2.2 Populate `provider_skill_id` from `CatalogSkillDefinition` in `recommend_skills()` at the explicit-recommendation branch (~line 333 in `catalog.rs`)
- [x] 2.3 Populate `provider_skill_id` in `rebuild_local_skill_index()` (~line 731 in `catalog.rs`) for consistency
- [x] 2.4 Thread `provider_skill_id` from `CatalogSkillMetadata` into `SkillSuggestion::new()` (~line 104 in `suggest.rs`)
- [x] 2.5 Populate `provider_skill_id` in `to_json_response()` when building `SuggestJsonRecommendation` (~line 450 in `suggest.rs`)
- [x] 2.6 Change `install_selected_with()` line ~370 from `provider.resolve(&recommendation.skill_id)` to `provider.resolve(&recommendation.provider_skill_id)`
- [x] 2.7 Fix all remaining compiler errors from the new field (struct literals, pattern matches, test constructors)

## Phase 3: Testing

- [x] 3.1 Update `LocalSkillProvider` mock in `tests/unit/suggest_install.rs` to key `sources` map by `provider_skill_id` values (e.g., `"dallay/agents-skills/rust-async-patterns"`)
- [x] 3.2 Update `PartiallyFailingProvider` mock in `tests/unit/suggest_install.rs` to key `sources` by `provider_skill_id`
- [x] 3.3 Update all `SkillSuggestion` and `CatalogSkillMetadata` constructors in test code to include `provider_skill_id`
- [x] 3.4 Run `cargo test --all-features` — all tests must pass
- [x] 3.5 Run `cargo clippy --all-targets --all-features -- -D warnings` — must pass clean
