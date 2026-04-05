# Delta for Skill Recommendations

## ADDED Requirements

### Requirement: Terminal-Aware Recommendation Install Presentation

Human-readable recommendation install output for `agentsync skill suggest --install` and
`agentsync skill suggest --install --all` MUST adapt to the execution environment.

When stdout is a terminal and `--json` is not enabled, the system SHOULD use
colored/status-oriented presentation to make in-progress install state easy for humans to scan.

When stdout is not a terminal, terminal capabilities are unavailable, or the environment is otherwise
non-interactive, the system MUST emit readable plain line-oriented output whose meaning does not
depend on cursor movement, spinner animation, or other terminal control sequences.

#### Scenario: TTY human output uses status-oriented presentation

- GIVEN recommendation-driven install output is running in a terminal
- AND the user did not request `--json`
- WHEN the system reports per-skill install activity
- THEN the output SHOULD use status-oriented presentation that makes in-progress, skipped, success,
  and failure states visually distinct
- AND the meaning of each status update MUST still be readable from the text itself

#### Scenario: Non-TTY output stays readable without terminal control behavior

- GIVEN recommendation-driven install output is running without a TTY
- AND the user did not request `--json`
- WHEN the system reports per-skill install activity
- THEN each update MUST be emitted as plain readable text in stable order
- AND the output MUST NOT rely on cursor movement, spinner-only frames, or similar terminal control
  behavior to communicate install state

---

### Requirement: Recommendation Install JSON Output Contract Stability

When the user requests `--json` for a recommendation-driven install flow, the system MUST preserve
the existing JSON output contract, field names, field meanings, and overall response shape.

JSON mode MUST NOT interleave human-oriented progress lines, colored text, spinner frames, or other
non-JSON output while work is happening.

#### Scenario: Install-all JSON output remains final structured JSON only

- GIVEN a repository with recommended skills
- WHEN the user runs `agentsync skill suggest --install --all --json`
- THEN stdout MUST remain valid JSON for the existing recommendation-install response contract
- AND the command MUST NOT emit human progress lines before or between JSON content

#### Scenario: Guided install JSON output suppresses live human progress rendering

- GIVEN the command is running in an interactive terminal
- AND the user requests `--json`
- WHEN the user completes the guided recommendation selection flow
- THEN the install execution output MUST remain restricted to the existing JSON contract
- AND any human-readable live progress presentation MUST be suppressed

---

## MODIFIED Requirements

### Requirement: Guided Recommendation Install

(Previously: the guided install flow required review and subset selection before installation, but it
did not require clear per-skill feedback while the chosen installs were actively being processed.)

The phase 2 guided installation flow MUST allow the user to review and choose from the repository's
recommended skills before installation execution begins.

In an interactive terminal, the guided installation flow MUST present only recommendation-driven
choices and MUST allow the user to install a selected subset of not-yet-installed recommended
skills.

After selection is complete and before final summary output, the human-readable guided install flow
MUST emit clear per-skill status updates while each selected recommendation is being processed.

Per-skill updates MUST make it clear when a selected skill is being processed, skipped because it is
already installed, installed successfully, or failed.

If no interactive terminal is available and the user has not provided an explicit non-interactive
install choice, the guided installation flow MUST fail without installing anything and MUST instruct
the user to use a supported non-interactive path.

#### Scenario: Interactive guided install reports live progress for selected skills

- GIVEN a repository with three recommended skills that are not installed
- AND the command is running in an interactive terminal
- WHEN the user chooses two of the three recommended skills in the guided install flow
- THEN exactly those two selected skills MUST be installed
- AND the human-readable output MUST emit per-skill progress updates for those selected skills before
  the final summary is printed

#### Scenario: Guided install reports immediate skip for already installed selection

- GIVEN a repository with a recommended skill that is already installed
- AND the command is running in an interactive terminal
- WHEN the user includes that skill in the guided install selection
- THEN the system MUST NOT reinstall that skill
- AND the human-readable output MUST explicitly report that skill as skipped while the install run is
  in progress

#### Scenario: Non-interactive guided install without explicit choice is rejected

- GIVEN a repository with recommended skills
- AND the command is not running in an interactive terminal
- WHEN the user invokes the guided install flow without an explicit non-interactive selection path
- THEN the command MUST fail without installing any skills
- AND the output MUST tell the user how to run a supported non-interactive install path

---

### Requirement: Install-All Recommended Skills

(Previously: the install-all flow required installation of every pending recommendation, but it did
not require visible per-skill progress updates during execution.)

The phase 2 install-all flow MUST install every recommended skill that is not already installed.

The install-all flow MUST be explicit and MUST NOT be the default behavior of the read-only
suggestion command.

During human-readable execution, the install-all flow MUST emit clear per-skill status updates while
work is happening so the user can see progress before completion.

If zero installable recommendations exist, the install-all flow MUST complete without error and MUST
NOT modify installed state.

#### Scenario: Install-all reports progress across pending recommendations

- GIVEN a repository with four recommended skills
- AND one of those skills is already installed
- WHEN the user invokes the explicit install-all recommendation flow
- THEN the three not-yet-installed recommended skills MUST be installed
- AND the already installed skill MUST be skipped
- AND the human-readable output MUST report each processed skill's status before the final summary

#### Scenario: Install-all is a no-op when nothing is installable

- GIVEN a repository where every recommended skill is already installed
- WHEN the user invokes the explicit install-all recommendation flow
- THEN the command MUST complete successfully
- AND no additional installation work SHALL occur
- AND installed state MUST remain unchanged
- AND the output MUST clearly indicate that there was nothing installable to do

---

### Requirement: Recommendation Installs Reuse Existing Lifecycle and Registry Flows

(Previously: recommendation-driven installs were required to surface the same installation failure
semantics as direct installs, but the spec did not require batch human output to keep individual
failures visible without losing the overall summary.)

Recommendation-driven installs MUST reuse the same install execution and installed-state persistence
behavior as the existing skill installation flow.

The recommendation feature MUST NOT introduce a separate installer, a separate installed-state
store, or a separate success/error contract for installation execution.

When recommendation-driven installation succeeds, installed skills MUST be observable through the
same installed-skill registry/state used by direct skill installation.

When one or more recommendation-driven installs fail in a multi-skill run, the human-readable output
MUST keep each failed skill visible with its failure status and MUST still emit the overall summary
for the completed run.

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

#### Scenario: Mixed install-all results keep failures visible and summary intact

- GIVEN a repository with multiple recommended skills selected for install-all
- AND one skill succeeds while another fails during installation
- WHEN the install-all run completes
- THEN the human-readable output MUST show the failed skill with a failure status and error context
- AND the human-readable output MUST still show the successful skill outcomes
- AND the final overall summary MUST remain visible after those per-skill results
