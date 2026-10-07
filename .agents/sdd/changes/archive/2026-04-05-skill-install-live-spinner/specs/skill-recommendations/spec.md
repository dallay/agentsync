# Delta for Skill Recommendations

## MODIFIED Requirements

### Requirement: Terminal-Aware Recommendation Install Presentation

Human-readable recommendation install output for `agentsync skill suggest --install` and
`agentsync skill suggest --install --all` MUST adapt to the execution environment.

When stdout is a terminal and `--json` is not enabled, the system MAY use a lightweight
spinner/live-activity presentation while recommendation-driven install work is in progress.

Any TTY live presentation MUST remain text-explicit enough that humans can understand what work is
being performed without relying on animation alone.

When stdout is not a terminal, terminal capabilities are unavailable, or the environment is
otherwise non-interactive, the system MUST emit readable, stable, line-oriented output whose meaning
does not depend on cursor movement, spinner animation, or other terminal control sequences.

(Previously: TTY human output only needed to use status-oriented presentation, while non-TTY output
had to stay readable and independent of terminal control behavior.)

#### Scenario: TTY human output may use live activity during recommendation installs

- GIVEN recommendation-driven install output is running in a terminal
- AND the user did not request `--json`
- WHEN the system reports in-progress install activity for `agentsync skill suggest --install` or
  `agentsync skill suggest --install --all`
- THEN the output MAY use a lightweight spinner or live activity presentation while work is running
- AND the text shown to the user MUST still identify the active install state in human-readable
  terms

#### Scenario: Non-TTY output stays stable and line-oriented

- GIVEN recommendation-driven install output is running without a TTY
- AND the user did not request `--json`
- WHEN the system reports per-skill install activity
- THEN each update MUST be emitted as stable readable lines in order
- AND the output MUST NOT rely on cursor movement, spinner-only frames, or similar terminal control
  behavior to communicate install state

## ADDED Requirements

### Requirement: Recommendation Install Final Human Summary Clarity

After a human-readable recommendation-driven install flow completes, the system MUST emit a final
summary that is easier to scan visually than plain prose while still stating outcomes explicitly.

The final human summary MUST explicitly report counts for:

- installed skills,
- already-installed skills, and
- failed skills.

The summary MAY use layout, icons, color, or emphasis to improve scanability in human mode, but the
meaning of each count MUST remain understandable from the text itself.

This requirement applies to both `agentsync skill suggest --install` and
`agentsync skill suggest --install --all` in human-readable mode only.

#### Scenario: Human summary shows explicit outcome counts after mixed results

- GIVEN a human-readable recommendation-driven install run completes
- AND the run includes at least one installed skill, one already-installed skill, and one failed
  skill
- WHEN the final summary is printed
- THEN the summary MUST explicitly show the installed count
- AND the summary MUST explicitly show the already-installed count
- AND the summary MUST explicitly show the failed count
- AND any visual styling MUST NOT be the only way those outcomes are communicated

#### Scenario: JSON output contract remains unchanged despite visual human summary

- GIVEN a repository with recommended skills
- WHEN the user runs a recommendation-driven install flow with `--json`
- THEN stdout MUST remain valid JSON for the existing recommendation-install response contract
- AND the command MUST NOT emit the human visual summary or any other human-only progress output
