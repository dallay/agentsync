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

### Requirement: Detection and Recommendation Are Separate Behaviors

The system MUST keep repository technology detection separate from recommendation generation.

Recommendation generation MUST consume detection results rather than re-scanning the repository
independently.

The system MUST be able to report detections even when zero skill recommendations are produced.

#### Scenario: Detections are reported when no catalog match exists

- GIVEN a repository with a detected supported technology
- AND the recommendation policy produces no skill recommendations for that detected technology
- WHEN the user runs the suggestion flow
- THEN the output MUST still include the detection entry
- AND the recommendations collection MUST be empty

#### Scenario: Unsupported repository produces no detections and no recommendations

- GIVEN a repository with none of the supported v1 technology markers
- WHEN the user runs the suggestion flow
- THEN the detections collection MUST be empty
- AND the recommendations collection MUST be empty

---

### Requirement: Recommendation Generation Includes Reasons

The system MUST generate recommendations from detected technologies using a recommendation policy
that is independent from installation execution.

Each recommendation MUST include:

- a skill identifier,
- one or more matched technology identifiers, and
- one or more human-readable reasons explaining why the skill was suggested for this repository.

When the same skill is recommended by more than one detected technology, the system MUST emit one
deduplicated recommendation entry whose matched technologies and reasons cover all contributing
detections.

#### Scenario: Recommendation includes matched technologies and reasons

- GIVEN a repository with detected `rust` and `docker` technologies
- WHEN the user runs the suggestion flow
- THEN each recommendation MUST include a skill identifier
- AND each recommendation MUST include at least one matched technology identifier from the
  detections
- AND each recommendation MUST include at least one human-readable reason tied to the repository
  evidence or detected technology

#### Scenario: Duplicate recommendations are merged

- GIVEN a repository whose detected technologies map to the same recommended skill more than once
- WHEN the user runs the suggestion flow
- THEN that skill MUST appear only once in the recommendations collection
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

When the user requests JSON output for the read-only suggestion command, the system MUST emit a JSON
object with this top-level shape:

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

The JSON output MUST always include the `detections`, `recommendations`, and `summary` fields, even
when the arrays are empty.

`summary.detected_count` MUST equal the number of detection entries.

`summary.recommended_count` MUST equal the number of recommendation entries.

`summary.installable_count` MUST equal the number of recommendation entries where `installed` is
`false`.

#### Scenario: JSON output includes all required fields

- GIVEN a repository with one detected technology and one recommended skill that is not installed
- WHEN the user runs the read-only suggestion command with JSON output enabled
- THEN the command MUST emit a JSON object containing `detections`, `recommendations`, and `summary`
- AND the detection entry MUST contain `technology`, `confidence`, and `evidence`
- AND the recommendation entry MUST contain `skill_id`, `matched_technologies`, `reasons`, and
  `installed`
- AND `summary.detected_count`, `summary.recommended_count`, and `summary.installable_count` MUST
  each equal `1`

#### Scenario: JSON output is empty but well-formed when nothing is detected

- GIVEN a repository with no supported v1 technology markers
- WHEN the user runs the read-only suggestion command with JSON output enabled
- THEN `detections` MUST be an empty array
- AND `recommendations` MUST be an empty array
- AND `summary.detected_count` MUST equal `0`
- AND `summary.recommended_count` MUST equal `0`
- AND `summary.installable_count` MUST equal `0`

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
