# Delta for Skill Recommendations

## ADDED Requirements

### Requirement: Embedded Catalog External Recommendation Policy Compliance

The embedded fallback recommendation catalog MUST allow externally sourced recommendation entries
only when the referenced skill is an approved official technology skill.

The embedded fallback recommendation catalog MUST NOT recommend third-party or community external
skills directly from external sources.

If maintainers want to recommend a third-party or community skill, that skill MUST first exist as a
curated local skill under `dallay/agents-skills`. If no curated local replacement exists, the
embedded catalog MUST omit that recommendation rather than keep the disallowed external reference.

When policy cleanup removes a disallowed external skill from a technology mapping, every affected
technology entry MUST remain valid after cleanup and MUST NOT retain dangling references to removed
skills.

#### Scenario: Approved official external recommendation remains valid

- GIVEN the embedded catalog contains a technology recommendation that references an approved
  official external skill
- WHEN the system validates and loads the embedded catalog
- THEN the catalog MUST accept that recommendation
- AND the affected technology mapping MUST remain available for recommendation generation

#### Scenario: Third-party external recommendation is rejected

- GIVEN the embedded catalog contains a technology recommendation that references a third-party or
  community external skill
- AND no curated replacement exists under `dallay/agents-skills`
- WHEN the system validates the embedded catalog
- THEN the system MUST fail catalog loading explicitly
- AND the invalid recommendation MUST NOT be used for suggestion generation

#### Scenario: Cleanup preserves valid technology mappings

- GIVEN the embedded catalog previously referenced disallowed external skills for `node`,
  `typescript`, or `biome`
- WHEN those disallowed skills are removed without a policy-compliant local replacement
- THEN every affected technology entry MUST still pass catalog validation
- AND the cleaned catalog MUST contain no references to removed or unknown skills

---

## MODIFIED Requirements

### Requirement: Embedded Declarative Recommendation Catalog

(Previously: the embedded catalog was required to fail explicitly when metadata could not be parsed
or failed schema validation, but the spec did not require policy validation for disallowed external
recommendations.)

The system MUST define the embedded fallback recommendation catalog in checked-in declarative
metadata rather than in hardcoded Rust recommendation tables.

The embedded catalog MUST be loadable without network or provider access and MUST remain available
as the baseline recommendation source for every suggestion flow.

If the embedded metadata cannot be parsed, fails schema or integrity validation, or violates the
external recommendation policy, the system MUST fail the recommendation-loading step explicitly
rather than silently proceeding with an empty, partial, or policy-invalid fallback catalog.

#### Scenario: Embedded metadata supplies the baseline catalog

- GIVEN the checked-in embedded recommendation metadata is present and valid
- AND the catalog satisfies schema, reference integrity, and external recommendation policy
- AND no usable provider recommendation metadata is available
- WHEN the user runs the suggestion flow
- THEN the system MUST load recommendations from the embedded metadata
- AND the resulting recommendations MUST be produced without provider access

#### Scenario: Policy-invalid embedded metadata fails explicitly

- GIVEN the checked-in embedded recommendation metadata contains a disallowed external
  recommendation
- WHEN the system initializes recommendation loading
- THEN the system MUST report an explicit recommendation catalog loading error
- AND the system MUST NOT silently continue with an empty, truncated, or policy-invalid fallback
  catalog
