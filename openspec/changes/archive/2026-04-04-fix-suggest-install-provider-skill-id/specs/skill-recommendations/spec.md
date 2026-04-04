# Delta for Skill Recommendations

## MODIFIED Requirements

### Requirement: Catalog Skill Metadata Carries Provider Skill ID

(Previously: `CatalogSkillMetadata` contained only `skill_id`, `title`, and `summary`, dropping the
qualified `provider_skill_id` when building recommendations from `CatalogSkillDefinition`.)

`CatalogSkillMetadata` MUST include a `provider_skill_id` field containing the fully qualified
provider skill identifier (e.g., `"dallay/agents-skills/accessibility"`).

The `skill_id` field MUST remain the `local_skill_id` (e.g., `"accessibility"`) for display,
registry lookups, and folder naming.

The `provider_skill_id` MUST be populated from the corresponding `CatalogSkillDefinition` whenever
a `CatalogSkillMetadata` is constructed.

#### Scenario: CatalogSkillMetadata includes both local and provider IDs

- GIVEN a catalog definition with `provider_skill_id = "dallay/agents-skills/accessibility"` and
  `local_skill_id = "accessibility"`
- WHEN the system constructs a `CatalogSkillMetadata` from that definition
- THEN `metadata.skill_id` MUST equal `"accessibility"`
- AND `metadata.provider_skill_id` MUST equal `"dallay/agents-skills/accessibility"`

#### Scenario: Existing skill_id usage is unaffected

- GIVEN a `CatalogSkillMetadata` with `skill_id = "docker-expert"` and
  `provider_skill_id = "dallay/agents-skills/docker-expert"`
- WHEN the system uses `skill_id` for display output, registry key lookups, or installed-skill
  folder names
- THEN the value used MUST be `"docker-expert"` (the local ID)
- AND the `provider_skill_id` MUST NOT be used for those purposes

---

### Requirement: Skill Suggestion Carries Provider Skill ID

(Previously: `SkillSuggestion` had no `provider_skill_id` field; only `skill_id` was available,
which contained the local ID.)

`SkillSuggestion` MUST include a `provider_skill_id` field that is threaded from
`CatalogSkillMetadata.provider_skill_id` at construction time.

The `provider_skill_id` MUST be available on every `SkillSuggestion` instance for use during
installation resolution.

#### Scenario: SkillSuggestion carries provider_skill_id from metadata

- GIVEN a `CatalogSkillMetadata` with `provider_skill_id = "dallay/agents-skills/makefile"`
- WHEN the system creates a `SkillSuggestion` from that metadata
- THEN `suggestion.provider_skill_id` MUST equal `"dallay/agents-skills/makefile"`
- AND `suggestion.skill_id` MUST equal `"makefile"`

#### Scenario: Deduplicated suggestions preserve provider_skill_id

- GIVEN two catalog technology entries that both recommend the same `local_skill_id`
- WHEN the recommendation engine deduplicates them into a single `SkillSuggestion`
- THEN the resulting suggestion MUST retain the `provider_skill_id` from the first insertion
- AND the `provider_skill_id` MUST NOT be empty

---

### Requirement: Install Resolution Uses Provider Skill ID

(Previously: `install_selected_with()` called `provider.resolve(&recommendation.skill_id)` using
the local ID, causing the provider's slash-count routing to send all catalog skills to the
skills.sh search API fallback instead of deterministic GitHub URL construction.)

`install_selected_with()` MUST pass `recommendation.provider_skill_id` to `provider.resolve()`
instead of `recommendation.skill_id`.

The `skill_id` (local ID) MUST continue to be used for:
- installed-state registry lookups and persistence,
- install function `skill_id` parameter (folder naming),
- result reporting (`SuggestInstallResult.skill_id`), and
- display output.

#### Scenario: Install resolves using qualified provider skill ID

- GIVEN a recommendation with `skill_id = "accessibility"` and
  `provider_skill_id = "dallay/agents-skills/accessibility"`
- WHEN `install_selected_with()` resolves that recommendation
- THEN the system MUST call `provider.resolve("dallay/agents-skills/accessibility")`
- AND the system MUST NOT call `provider.resolve("accessibility")`

#### Scenario: Provider deterministic resolution receives qualified ID

- GIVEN a `SkillsShProvider` and a recommendation with
  `provider_skill_id = "dallay/agents-skills/docker-expert"`
- WHEN the provider's `resolve()` method receives the `provider_skill_id`
- THEN the slash-count routing MUST select the deterministic GitHub URL construction path
- AND the resulting download URL MUST point to the correct GitHub archive location

#### Scenario: Local skill_id is still used for folder naming and registry

- GIVEN a recommendation with `skill_id = "accessibility"` and
  `provider_skill_id = "dallay/agents-skills/accessibility"`
- WHEN the install function is invoked after successful resolution
- THEN the `skill_id` parameter passed to the install function MUST be `"accessibility"`
- AND the installed-skill registry MUST record the skill under key `"accessibility"`
- AND the skill folder MUST be created at `.agents/skills/accessibility/`

#### Scenario: Install-all flow uses provider_skill_id for every recommendation

- GIVEN a repository with three recommended skills, each having distinct `provider_skill_id` values
- AND none of the skills are currently installed
- WHEN the user invokes the install-all recommendation flow
- THEN each `provider.resolve()` call MUST receive the respective `provider_skill_id`
- AND the installed-skill registry MUST use the respective `skill_id` (local) for each entry

---

## ADDED Requirements

### Requirement: JSON Output Includes Provider Skill ID

`SuggestJsonRecommendation` SHOULD include a `provider_skill_id` field in its serialized JSON
output so that JSON consumers can access the fully qualified provider identifier alongside the
local `skill_id`.

The `provider_skill_id` field is additive; existing JSON fields (`skill_id`,
`matched_technologies`, `reasons`, `installed`) MUST remain unchanged.

#### Scenario: JSON recommendation includes provider_skill_id

- GIVEN a recommendation with `skill_id = "makefile"` and
  `provider_skill_id = "dallay/agents-skills/makefile"`
- WHEN the system serializes the suggestion to JSON output
- THEN the JSON object MUST include `"skill_id": "makefile"`
- AND the JSON object MUST include `"provider_skill_id": "dallay/agents-skills/makefile"`
- AND `"matched_technologies"`, `"reasons"`, and `"installed"` MUST still be present

#### Scenario: JSON shape remains backward compatible

- GIVEN a JSON consumer that reads `skill_id`, `matched_technologies`, `reasons`, and `installed`
- WHEN the system adds the `provider_skill_id` field to the recommendation JSON
- THEN the existing fields MUST NOT change position, type, or semantics
- AND the new field MUST be additive only

---

### Requirement: Test Mocks Must Reflect Provider Skill ID Resolution

Unit test mock providers MUST key their `resolve()` source maps by `provider_skill_id` values
rather than `local_skill_id` values, reflecting the changed calling convention in
`install_selected_with()`.

#### Scenario: Mock provider resolves by provider_skill_id

- GIVEN a unit test mock provider whose source map is keyed by `provider_skill_id`
  (e.g., `"dallay/agents-skills/rust-async-patterns"`)
- WHEN `install_selected_with()` calls `provider.resolve()` with the recommendation's
  `provider_skill_id`
- THEN the mock MUST successfully resolve the skill source
- AND the install flow MUST complete without "missing local source" errors

#### Scenario: Mock provider fails for unrecognized IDs

- GIVEN a unit test mock provider whose source map contains only `provider_skill_id` keys
- WHEN `provider.resolve()` receives a bare `local_skill_id` (e.g., `"rust-async-patterns"`)
- THEN the mock MUST return an error
- AND this validates that the calling code is correctly passing `provider_skill_id`
