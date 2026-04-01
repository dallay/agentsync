# Delta for Skill Adoption

## ADDED Requirements

### Requirement: Wizard Makes Skills Link Strategy Explicit

For each generated skills target, `agentsync init --wizard` MUST present the skills link strategy as
an explicit user choice between `symlink` and `symlink-contents`.

The wizard MUST explain the observable difference between the two modes and MUST show a recommended
choice for the target before the user confirms it.

When the existing destination for that skills target is already a directory symlink to the canonical
`.agents/skills` source, the wizard MUST recommend `symlink` and MUST keep that recommendation as
the default selection.

The wizard MUST allow the user to override the recommendation before config is written.

#### Scenario: Wizard recommends preserving an existing directory symlink

- GIVEN `agentsync init --wizard` is generating a skills target for an agent
- AND the agent's skills destination already exists as a directory symlink to the canonical
  `.agents/skills` source
- WHEN the wizard asks how that skills target should sync
- THEN the wizard MUST show both `symlink` and `symlink-contents` as explicit choices
- AND the wizard MUST recommend `symlink`
- AND accepting the default MUST write `type = "symlink"` for that target

#### Scenario: User overrides the recommended skills mode

- GIVEN `agentsync init --wizard` is generating a skills target for an agent
- WHEN the wizard shows the explicit skills link strategy choices and recommendation
- AND the user selects the non-recommended mode
- THEN the generated config MUST use the mode chosen by the user for that target

### Requirement: Post-Init Validation Warns About Skills Mode Mismatches

After `agentsync init --wizard` writes config, the system MUST run a validation summary before the
command finishes.

If a generated skills target's configured mode does not match the on-disk destination shape in a way
that would cause different sync semantics, the validation summary MUST warn about that target before
the user runs `agentsync apply`.

The warning MUST identify the affected target and MUST describe the mismatch in user-facing terms.

#### Scenario: Wizard validation warns after a mode override creates mismatch

- GIVEN an agent's skills destination already exists as a directory symlink to the canonical
  `.agents/skills` source
- AND the user finishes `agentsync init --wizard` with that target configured as `symlink-contents`
- WHEN the wizard prints its post-init validation summary
- THEN the summary MUST warn that the configured mode does not match the current destination shape
- AND the warning MUST appear before the command exits

#### Scenario: Wizard validation stays quiet for matching directory symlink mode

- GIVEN an agent's skills destination already exists as a directory symlink to the canonical
  `.agents/skills` source
- AND the user finishes `agentsync init --wizard` with that target configured as `symlink`
- WHEN the wizard prints its post-init validation summary
- THEN the summary MUST NOT warn about a mode-semantic mismatch for that target

### Requirement: Doctor Clearly Reports Skills Mode-Semantic Mismatches

`agentsync doctor` MUST detect the case where a skills target is configured as `symlink-contents`
while the destination already exists as a directory symlink to the canonical source for that target.

When that mismatch is detected, `agentsync doctor` MUST report it clearly as a mode-semantic
mismatch rather than as a healthy target.

The diagnostic MUST identify the affected target, MUST describe the configured mode and observed
destination shape, and MUST warn that applying the config can cause avoidable churn.

#### Scenario: Doctor reports directory-symlink versus symlink-contents mismatch

- GIVEN a project with a skills target configured as `symlink-contents`
- AND that target's destination already exists as a directory symlink to the canonical source
- WHEN the user runs `agentsync doctor`
- THEN the output MUST report a mode-semantic mismatch for that target
- AND the output MUST mention both `symlink-contents` and the existing directory symlink shape
- AND the output MUST warn before the user runs `agentsync apply`

#### Scenario: Doctor does not report mismatch for matching directory symlink mode

- GIVEN a project with a skills target configured as `symlink`
- AND that target's destination already exists as a directory symlink to the canonical source
- WHEN the user runs `agentsync doctor`
- THEN the output MUST NOT report a mode-semantic mismatch for that target

### Requirement: Status Gives a Focused Hint for Recognized Skills Mode Mismatches

When `agentsync status` inspects a skills target whose destination resolves to the canonical source
but whose configured mode does not match the observed destination shape, it MUST include a focused
hint about the mode-semantic mismatch.

The hint MUST identify the affected target without requiring the command to treat the destination as
broken.

#### Scenario: Status hints on recognized mode mismatch

- GIVEN a project with a skills target configured as `symlink-contents`
- AND that target's destination already exists as a directory symlink to the canonical source
- WHEN the user runs `agentsync status`
- THEN the output MUST include a hint that the target's configured mode does not match the current
  destination shape
- AND the hint MUST identify the affected skills target

### Requirement: Skills Documentation Matches Shipped Link Behavior

The published documentation for skills configuration and wizard behavior MUST describe skills
targets as defaulting to `symlink`.

The documentation MUST explain when `symlink-contents` remains a valid choice, MUST describe the
wizard's recommendation and preservation behavior for existing correct directory symlinks, and MUST
describe how users can detect mode-semantic mismatches before apply.

#### Scenario: Reference docs describe current default and preservation guidance

- GIVEN the published configuration and skills guidance for AgentSync
- WHEN a user reads the skills sync documentation
- THEN the documentation MUST describe `symlink` as the default skills mode
- AND the documentation MUST explain that existing correct directory symlinks are preserved by
  default in the wizard

#### Scenario: CLI docs describe validation and diagnostics

- GIVEN the published CLI documentation for init and diagnostics
- WHEN a user reads about `agentsync init --wizard` and `agentsync doctor`
- THEN the documentation MUST describe the post-init validation summary for mode mismatches
- AND the documentation MUST describe how `doctor` reports the recognized mismatch case before apply
