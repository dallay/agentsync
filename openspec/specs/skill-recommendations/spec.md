# Skill Recommendations Specification

## Purpose

Define how AgentSync detects repository technologies, generates opinionated skill recommendations,
reports them in human and JSON forms, and optionally installs recommended skills without introducing
a parallel install or registry system.

## Requirements

### Requirement: Local Repository Technology Detection

The system MUST detect supported repository technologies from local repository contents only.

The detector MUST support the initial v1 ecosystems of Rust, TypeScript/Node, Astro, GitHub Actions,
Docker, Make, and Python.

The detector MUST return a normalized detection entry for each detected technology containing:

- a stable technology identifier,
- a confidence value of `high`, `medium`, or `low`, and
- one or more evidence items describing the local file markers that caused the detection.

The detector MUST be deterministic for the same repository contents.

The detector MUST NOT report a technology when none of that technology's supported local markers are
present.

#### Scenario: Detect multiple supported ecosystems with evidence

- GIVEN a repository containing `Cargo.toml`, `package.json`, `website/docs/astro.config.mjs`,
  `.github/workflows/ci.yml`, `Dockerfile`, `Makefile`, and `pyproject.toml`
- WHEN the user runs the suggestion flow for that repository
- THEN the detection result MUST include `rust`, `node_typescript`, `astro`, `github_actions`,
  `docker`, `make`, and `python`
- AND each detected technology MUST include at least one evidence item naming a matching local
  marker
- AND each detected technology MUST include a confidence value of `high`, `medium`, or `low`

#### Scenario: Omit unsupported or absent technologies

- GIVEN a repository containing only `Cargo.toml`
- WHEN the user runs the suggestion flow for that repository
- THEN the detection result MUST include `rust`
- AND the detection result MUST NOT include `python`
- AND the detection result MUST NOT include `docker`

#### Scenario: Canonical marker yields high-confidence detection

- GIVEN a repository containing `Cargo.toml`
- WHEN the user runs the suggestion flow for that repository
- THEN the `rust` detection MUST have confidence `high`
- AND the evidence list MUST include `Cargo.toml`

---

### Requirement: Embedded Declarative Recommendation Catalog

The system MUST define the embedded fallback recommendation catalog in checked-in declarative
metadata rather than in hardcoded Rust recommendation tables.

The embedded catalog MUST be loadable without network or provider access and MUST remain available
as the baseline recommendation source for every suggestion flow.

If the embedded metadata cannot be parsed or validated, the system MUST fail the
recommendation-loading step explicitly rather than silently proceeding with an empty or partial
fallback catalog.

#### Scenario: Embedded metadata supplies the baseline catalog

- GIVEN the checked-in embedded recommendation metadata is present and valid
- AND no usable provider recommendation metadata is available
- WHEN the user runs the suggestion flow
- THEN the system MUST load recommendations from the embedded metadata
- AND the resulting recommendations MUST be produced without provider access

#### Scenario: Invalid embedded metadata fails explicitly

- GIVEN the checked-in embedded recommendation metadata is malformed or fails schema validation
- WHEN the system initializes recommendation loading
- THEN the system MUST report an explicit recommendation catalog loading error
- AND the system MUST NOT silently continue with an empty or truncated fallback catalog

---

### Requirement: Explicit Technology Recommendation Entries

The recommendation catalog MUST define technology recommendation entries explicitly rather than
deriving them from ad hoc rule tables.

Each technology recommendation entry MUST include:

- `id`,
- `name`, and
- `skills`.

`id` MUST be the stable catalog key that aligns with the detected technology identifier used by the
Rust detector in phase 1.

`skills` MUST support one or more opinionated skill identifiers for that technology.

The phase-1 schema MAY include additional technology metadata fields for future compatibility, but
those extra fields MUST NOT be required for recommendation loading or Rust detection in this phase.

#### Scenario: One technology maps to one opinionated skill

- GIVEN the catalog defines a technology entry with `id = "make"`, `name = "Make"`, and
  `skills = ["makefile"]`
- WHEN the suggestion flow evaluates a repository whose Rust detector reports `make`
- THEN the system MUST treat that technology entry as a valid recommendation source
- AND the recommendations MUST include `makefile`

Phase-1 note: illustrative future technologies such as `vue` remain out of runtime detection scope
until the Rust `TechnologyId` enum and detector explicitly add support for them.

#### Scenario: One technology maps to multiple opinionated skills

- GIVEN the catalog defines a technology entry whose `skills` list contains multiple skill
  identifiers
- WHEN the suggestion flow evaluates a repository matching that technology entry
- THEN the system MUST be able to recommend each listed skill
- AND the recommendation model MUST preserve them as distinct recommendations unless later
  deduplicated by identical skill identifier

---

### Requirement: Explicit Combo Recommendation Entries

The recommendation catalog MUST define combo recommendation entries explicitly so future
multi-technology recommendations can be represented without changing the catalog shape.

Each combo recommendation entry MUST include:

- `id`,
- `name`,
- `requires`, and
- `skills`.

`requires` MUST support multiple technology identifiers.

`skills` MUST support one or more opinionated skill identifiers recommended when the combo applies.

In phase 1, the schema MUST accept and preserve valid combo entries even if active combo evaluation
is deferred by design.

#### Scenario: Combo entry captures a future multi-technology recommendation

- GIVEN the catalog defines a combo entry with a stable `id`, a human-readable `name`, multiple
  `requires` technologies, and one or more `skills`
- WHEN the catalog is loaded and validated in phase 1
- THEN the combo entry MUST be accepted as valid catalog data
- AND the catalog shape MUST preserve that combo entry for future recommendation behavior

#### Scenario: Invalid combo entry is rejected explicitly

- GIVEN the catalog defines a combo entry missing `id`, `name`, `requires`, or `skills`
- WHEN the system validates that catalog entry
- THEN the system MUST reject that combo entry as invalid
- AND the rest of the catalog MUST continue following the embedded-versus-provider fallback rules

---

### Requirement: Provider Metadata Overlay and Safe Fallback

The system MUST treat provider recommendation metadata as optional overlay input on top of the
embedded fallback catalog.

If provider recommendation metadata is unavailable, unreadable, or invalid at the top level, the
system MUST ignore the provider metadata and MUST continue using the embedded fallback catalog only.

If provider recommendation metadata is partially valid, the system MUST keep every valid provider
entry that passes validation, MUST ignore only the invalid provider entries, and MUST preserve the
embedded fallback behavior for all unaffected technology and combo entries.

#### Scenario: Missing provider metadata falls back safely

- GIVEN the embedded recommendation metadata is valid
- AND the provider does not return recommendation metadata
- WHEN the user runs the suggestion flow
- THEN the system MUST use the embedded fallback catalog
- AND the suggestion flow MUST still complete successfully

#### Scenario: Partially invalid provider metadata keeps valid overlay entries

- GIVEN the embedded recommendation metadata is valid
- AND the provider returns recommendation metadata containing both valid and invalid entries
- WHEN the system merges provider metadata with the embedded fallback catalog
- THEN the valid provider entries MUST participate in recommendation generation
- AND the invalid provider entries MUST be ignored
- AND embedded recommendations not superseded by valid provider entries MUST remain available

---

### Requirement: Hybrid Catalog Merge Semantics

The system MUST merge embedded and provider recommendation metadata deterministically using stable
catalog keys.

Technology entry overlay MUST be keyed by technology `id`.

Combo entry overlay MUST be keyed by combo `id`.

When a provider technology or combo entry does not match an embedded entry key, the provider entry
MUST extend the merged catalog.

When a provider technology or combo entry matches an embedded entry key, the provider entry MUST
override that matching embedded entry for recommendation generation.

This phase MUST NOT require provider-driven deletion or disable semantics for embedded technology or
combo entries.

#### Scenario: Provider extends the fallback catalog with a new technology entry

- GIVEN a valid phase-1 baseline catalog input contains no technology entry keyed `make`
- AND the provider metadata contains a valid technology entry keyed `make`
- WHEN the system merges the catalogs
- THEN the merged catalog MUST include the provider technology entry `make`
- AND existing embedded entries MUST remain present

#### Scenario: Provider overrides a matching embedded combo entry

- GIVEN the embedded fallback catalog contains a combo entry with stable key `rust-docker`
- AND the provider metadata contains a valid combo entry with the same stable key `rust-docker`
- WHEN the system merges the catalogs
- THEN the provider combo entry MUST take precedence for that key
- AND the embedded combo entry for that same key MUST NOT also produce a second conflicting instance

---

### Requirement: Compatibility for Existing Supported Technologies

For repositories whose detections are limited to the current supported technologies of `rust`,
`node_typescript`, `astro`, `github_actions`, `docker`, `make`, and `python`, the embedded
declarative catalog MUST preserve the pre-migration recommendation behavior unless a valid provider
overlay explicitly changes it.

In the absence of a valid provider override, this migration MUST preserve the same JSON shape, CLI
suggestion shape, and materially equivalent recommendation results for those supported technologies.

The catalog MUST also support technologies whose recommendation set contains multiple opinionated
skills, even when an existing supported technology currently maps to only one baseline skill.

#### Scenario: Embedded declarative migration preserves current baseline behavior

- GIVEN a repository whose detections map only to the current supported technologies
- AND no usable provider recommendation metadata is available
- WHEN the user runs the suggestion flow after the declarative catalog migration
- THEN the recommendation results MUST remain materially equivalent to the pre-migration embedded
  baseline
- AND the JSON and CLI output structures MUST remain unchanged

#### Scenario: Provider override changes only the targeted supported technology mapping

- GIVEN a repository with a detected supported technology
- AND the provider supplies a valid override only for that technology's catalog entry
- WHEN the user runs the suggestion flow
- THEN the targeted recommendation outcome MAY differ according to the provider override
- AND recommendations for other supported technologies MUST continue using the embedded fallback
  behavior unless separately overridden

---

### Requirement: Recommendation Schema Is Future-Compatible but Phase-1 Minimal

The catalog schema MUST distinguish between phase-1 required recommendation fields and
future-compatible metadata hooks.

In phase 1:

- technology entries MUST require only `id`, `name`, and `skills`,
- combo entries MUST require only `id`, `name`, `requires`, and `skills`, and
- recommendation evaluation MUST continue consuming technology detections produced in Rust.

The schema MAY include additional metadata adjacent to those entries for future detection,
confidence, labels, evidence hints, or other recommendation annotations, but phase 1 MUST NOT
require those fields to exist and MUST NOT move repository detection out of Rust.

#### Scenario: Future metadata hooks do not block phase-1 loading

- GIVEN a valid catalog whose technology and combo entries contain the phase-1 required fields
- AND optional future-oriented metadata fields are absent
- WHEN the system loads the catalog in phase 1
- THEN the catalog MUST still be considered valid
- AND recommendation loading MUST succeed without those future fields

#### Scenario: Detection remains Rust-owned despite adjacent metadata hooks

- GIVEN the catalog includes optional future-compatible metadata adjacent to a technology entry
- WHEN the user runs the suggestion flow in phase 1
- THEN repository technology detection MUST still come from Rust detection logic
- AND the optional metadata MUST NOT become a required source of truth for detection in this phase

---

### Requirement: Detection and Recommendation Are Separate Behaviors

The system MUST keep repository technology detection separate from recommendation generation.

Repository technology detection MUST remain the source of truth in Rust and recommendation
generation MUST consume detection results rather than re-scanning repository contents or deriving
technologies from recommendation metadata.

Catalog technology identifiers MUST align with the technology identifiers produced by Rust detection
so recommendation lookup remains deterministic.

Embedded and provider recommendation metadata MUST influence only recommendation mapping behavior
and MUST NOT change what technologies are detected from a repository.

The system MUST be able to report detections even when zero skill recommendations are produced.

#### Scenario: Provider metadata does not alter detections

- GIVEN a repository with local markers for `rust` only
- AND provider metadata contains recommendation entries mentioning additional technologies
- WHEN the user runs the suggestion flow
- THEN the detection result MUST still include only the technologies supported by the repository's
  local markers
- AND provider metadata MUST affect only the recommendation mapping for those detections

#### Scenario: Detections remain visible without usable recommendation metadata

- GIVEN a repository with a detected supported technology
- AND no embedded or provider entry yields a recommendation for that detected technology
- WHEN the user runs the suggestion flow
- THEN the output MUST still include the detection entry
- AND the recommendations collection MUST be empty

---

### Requirement: Recommendation Generation Includes Reasons

The system MUST generate recommendations from detected technologies using the merged hybrid catalog
and MUST keep recommendation generation independent from installation execution.

Each recommendation MUST include:

- a `skill_id`,
- one or more matched technology identifiers,
- one or more human-readable reasons explaining why the skill was suggested for this repository, and
- installed-state awareness as defined elsewhere in the specification.

The catalog model MUST support multiple distinct recommendations for the same detected technology.

When the same `skill_id` is recommended by more than one matched technology entry, combo entry, or
detected technology contribution, the system MUST emit one deduplicated recommendation entry whose
matched technologies and reasons cover all contributing matches.

#### Scenario: One technology yields multiple recommendations

- GIVEN a repository with one detected technology
- AND the merged hybrid catalog contains a technology entry for that technology with multiple listed
  skills
- WHEN the user runs the suggestion flow
- THEN the recommendations collection MUST include each distinct recommended skill
- AND each recommendation MUST include a reason for why it was suggested

#### Scenario: Duplicate skill IDs are deduplicated across technology and combo contributions

- GIVEN a repository whose detected technologies match multiple merged catalog contributions that
  all recommend the same `skill_id`
- WHEN the user runs the suggestion flow
- THEN that `skill_id` MUST appear only once in the recommendations collection
- AND the recommendation MUST include every contributing matched technology
- AND the recommendation MUST include the combined reasons for that skill

---

### Requirement: Installed-State Awareness

The system MUST determine whether each recommended skill is already installed by consulting the
existing installed-skill state.

Each recommendation MUST indicate whether the skill is already installed.

Read-only suggestion output MUST include already installed recommendations rather than silently
hiding them.

Install flows MUST NOT reinstall a skill that is already installed.

#### Scenario: Suggest marks installed recommendations

- GIVEN a repository with a recommended skill `docker-expert`
- AND `docker-expert` is already installed in the repository's existing skill registry/state
- WHEN the user runs the suggestion flow
- THEN the `docker-expert` recommendation MUST be present
- AND the recommendation MUST indicate `installed = true`

#### Scenario: Install flow skips already installed recommendations

- GIVEN a repository with three recommended skills
- AND one of those skills is already installed
- WHEN the user runs a recommendation-driven install flow
- THEN the already installed skill MUST NOT be reinstalled
- AND only the not-yet-installed recommended skills MUST be passed to installation execution

---

### Requirement: Read-Only Suggest Is Non-Destructive By Default

The phase 1 suggestion command MUST be read-only by default.

Running the read-only suggestion command MUST NOT modify repository files, installed-skill registry
contents, or installed skill directories.

The read-only suggestion command MUST succeed whether or not any detections or recommendations are
present.

#### Scenario: Suggest performs no filesystem or registry changes

- GIVEN a repository with detectable technologies and recommended skills
- WHEN the user runs the read-only suggestion command
- THEN no skill files SHALL be installed, updated, or removed
- AND the installed-skill registry/state SHALL remain unchanged
- AND the command MUST only report detections and recommendations

#### Scenario: Suggest succeeds with no recommendations

- GIVEN a repository with no supported v1 technology markers
- WHEN the user runs the read-only suggestion command
- THEN the command MUST exit successfully
- AND the output MUST indicate that no technologies and no recommendations were found

---

### Requirement: Suggest JSON Output Contract

When the user requests JSON output for the read-only suggestion command, the system MUST continue
emitting the same top-level contract with `detections`, `recommendations`, and `summary`.

Each recommendation entry in JSON MUST continue exposing `skill_id`, `matched_technologies`,
`reasons`, and `installed` as the stable phase-1 fields.

Migrating the catalog to explicit technology/combo metadata and provider overlay MUST NOT by itself
require new top-level JSON fields.

`summary.detected_count` MUST equal the number of detection entries.

`summary.recommended_count` MUST equal the number of recommendation entries.

`summary.installable_count` MUST equal the number of recommendation entries where `installed` is
`false`.

```json
{
  "detections": [
    {
      "technology": "string",
      "confidence": "high|medium|low",
      "evidence": ["string"]
    }
  ],
  "recommendations": [
    {
      "skill_id": "string",
      "matched_technologies": ["string"],
      "reasons": ["string"],
      "installed": true
    }
  ],
  "summary": {
    "detected_count": 0,
    "recommended_count": 0,
    "installable_count": 0
  }
}
```

#### Scenario: Hybrid catalog keeps the existing JSON contract stable

- GIVEN a repository with detections and recommendations produced from the merged hybrid catalog
- WHEN the user runs the read-only suggestion command with JSON output enabled
- THEN the JSON output MUST still contain `detections`, `recommendations`, and `summary`
- AND each recommendation entry MUST still contain `skill_id`, `matched_technologies`, `reasons`,
  and `installed`
- AND internal catalog-source changes MUST NOT require a different top-level JSON structure

#### Scenario: Multiple recommended skills preserve the stable recommendation shape

- GIVEN a recommendation catalog entry that lists multiple skills for one detected technology
- WHEN the user runs the read-only suggestion command with JSON output enabled
- THEN each resulting recommendation MUST still use the existing recommendation object shape
- AND supporting multiple recommended skills MUST NOT require a new top-level JSON contract

---

### Requirement: Guided Recommendation Install

The phase 2 guided installation flow MUST allow the user to review and choose from the repository's
recommended skills before installation execution begins.

In an interactive terminal, the guided installation flow MUST present only recommendation-driven
choices and MUST allow the user to install a selected subset of not-yet-installed recommended
skills.

If no interactive terminal is available and the user has not provided an explicit non-interactive
install choice, the guided installation flow MUST fail without installing anything and MUST instruct
the user to use a supported non-interactive path.

#### Scenario: Interactive guided install installs a selected subset

- GIVEN a repository with three recommended skills that are not installed
- AND the command is running in an interactive terminal
- WHEN the user chooses two of the three recommended skills in the guided install flow
- THEN exactly those two selected skills MUST be installed
- AND the unselected recommended skill MUST remain uninstalled

#### Scenario: Non-interactive guided install without explicit choice is rejected

- GIVEN a repository with recommended skills
- AND the command is not running in an interactive terminal
- WHEN the user invokes the guided install flow without an explicit non-interactive selection path
- THEN the command MUST fail without installing any skills
- AND the output MUST tell the user how to run a supported non-interactive install path

---

### Requirement: Install-All Recommended Skills

The phase 2 install-all flow MUST install every recommended skill that is not already installed.

The install-all flow MUST be explicit and MUST NOT be the default behavior of the read-only
suggestion command.

If zero installable recommendations exist, the install-all flow MUST complete without error and MUST
NOT modify installed state.

#### Scenario: Install-all installs every pending recommendation

- GIVEN a repository with four recommended skills
- AND one of those skills is already installed
- WHEN the user invokes the explicit install-all recommendation flow
- THEN the three not-yet-installed recommended skills MUST be installed
- AND the already installed skill MUST be skipped

#### Scenario: Install-all is a no-op when nothing is installable

- GIVEN a repository where every recommended skill is already installed
- WHEN the user invokes the explicit install-all recommendation flow
- THEN the command MUST complete successfully
- AND no additional installation work SHALL occur
- AND installed state MUST remain unchanged

---

### Requirement: Recommendation Installs Reuse Existing Lifecycle and Registry Flows

Recommendation-driven installs MUST reuse the same install execution and installed-state persistence
behavior as the existing skill installation flow.

The recommendation feature MUST NOT introduce a separate installer, a separate installed-state
store, or a separate success/error contract for installation execution.

When recommendation-driven installation succeeds, installed skills MUST be observable through the
same installed-skill registry/state used by direct skill installation.

#### Scenario: Guided install persists through the existing installed-state system

- GIVEN a repository with a recommended skill that is not installed
- WHEN the user installs that skill through a recommendation-driven flow
- THEN the installed skill MUST appear in the same installed-skill registry/state that direct skill
  installation uses
- AND subsequent read-only suggestion output MUST mark that skill as installed

#### Scenario: Recommendation-driven install surfaces existing installation failure semantics

- GIVEN a repository with a recommended skill whose direct installation would fail
- WHEN the user attempts to install that skill through a recommendation-driven flow
- THEN the recommendation-driven flow MUST report the installation failure
- AND the failure semantics MUST match the existing skill installation behavior for that same
  failure
