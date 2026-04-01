# Delta for Documentation

## ADDED Requirements

### Requirement: Dedicated Windows Symlink Setup Guide And Information Architecture

The documentation set MUST provide one dedicated, linkable Windows symlink setup guide as the
canonical destination for Windows-specific AgentSync setup guidance.

The guide MUST live in the guides documentation area and MUST be discoverable from the primary
documentation information architecture, including guide navigation or another equivalent
high-visibility guide index surface.

The dedicated guide MUST answer what a Windows user needs to do before AgentSync symlinks work
reliably, while deferring general team workflow policy and cross-platform process guidance to the
existing canonical workflow documentation.

#### Scenario: Windows reader needs a canonical setup destination

- GIVEN a Windows user needs platform-specific setup help for AgentSync symlinks
- WHEN the reader navigates the documentation or follows a cross-link from another docs surface
- THEN the documentation MUST provide one dedicated Windows symlink setup guide as the canonical
  destination
- AND the guide MUST be directly linkable
- AND the guide MUST focus on Windows environment readiness rather than reproducing the full team
  workflow narrative

#### Scenario: Guide is discoverable from primary docs navigation

- GIVEN a reader starts from a primary docs entry point such as the guides index or guide navigation
- WHEN the reader looks for setup help related to Windows symlink behavior
- THEN the dedicated Windows guide MUST be discoverable from that information architecture
- AND the navigation wording MUST make clear that the page is for Windows-specific symlink setup
  guidance

---

### Requirement: Documentation Explains Native Windows Symlink Prerequisites

The dedicated Windows guide MUST explain the native Windows prerequisites that affect whether
AgentSync-managed symlinks can be created reliably.

That guidance MUST describe prerequisite categories such as permission or policy requirements,
required shell or execution context expectations, and any setup conditions the reader should verify
before relying on native Windows symlink creation.

The documentation MUST describe these prerequisites as environment-readiness guidance and MUST NOT
imply that AgentSync changes its product defaults or workflow semantics on Windows.

#### Scenario: Native Windows user evaluates prerequisites before setup

- GIVEN a reader intends to use AgentSync from native Windows
- WHEN the reader opens the dedicated Windows guide
- THEN the guide MUST explain the prerequisites that affect native symlink creation
- AND the guide MUST identify what the reader should confirm before running AgentSync
- AND the guide MUST frame those prerequisites as Windows environment requirements rather than
  product configuration changes

#### Scenario: Native Windows guidance stays consistent with shared workflows

- GIVEN a team uses the documented default or opt-out gitignore workflow
- WHEN a Windows user reviews native prerequisite guidance
- THEN the documentation MUST keep the workflow policy consistent with the shared cross-platform
  docs
- AND the guide MUST avoid implying that Windows requires a separate workflow variant

---

### Requirement: Documentation Positions WSL As An Optional Lower-Friction Path

The dedicated Windows guide MUST describe WSL as an optional lower-friction setup path when native
Windows symlink prerequisites are difficult, unavailable, or undesirable for the reader.

The documentation SHOULD explain when a reader might prefer WSL and what expectation that choice
changes for local setup context.

The documentation MUST NOT imply that WSL is mandatory for Windows users or that it replaces the
shared team workflow guidance.

#### Scenario: Reader needs an alternative to native Windows setup friction

- GIVEN a Windows reader cannot easily satisfy native symlink prerequisites or wants a
  lower-friction development setup
- WHEN the reader reviews the Windows guide
- THEN the guide MUST present WSL as an available alternative
- AND the guide MUST explain when WSL may be the simpler option
- AND the guide MUST avoid presenting WSL as a required default for all Windows users

#### Scenario: WSL guidance remains scoped to setup positioning

- GIVEN a reader follows the WSL-oriented portion of the Windows guide
- WHEN the reader looks for workflow instructions beyond environment setup
- THEN the guide MUST point back to the canonical shared workflow documentation for team process
  details
- AND the WSL guidance MUST remain focused on setup positioning rather than duplicating the full
  workflow narrative

---

### Requirement: Documentation Includes Windows Verification And Recovery Guidance

The dedicated Windows guide MUST include practical verification guidance that helps a reader confirm
the selected setup path is ready for AgentSync symlink usage.

The guide MUST include recovery guidance for common setup failures within scope, such as unmet
prerequisites, permission-related failures, or a need to re-run setup and re-apply symlinks after
correcting the environment.

The verification and recovery guidance SHOULD help a reader distinguish between
environment-readiness problems and normal shared workflow expectations.

#### Scenario: Reader verifies a Windows setup path before relying on it

- GIVEN a Windows reader has completed either native prerequisite setup or a WSL-based setup path
- WHEN the reader consults the guide before continuing with AgentSync usage
- THEN the guide MUST describe how to verify that symlink setup is working as expected
- AND the verification steps MUST be actionable enough for the reader to confirm readiness

#### Scenario: Reader needs recovery steps after a setup failure

- GIVEN a Windows reader encounters a symlink-related setup problem that falls within the guide's
  scope
- WHEN the reader looks for recovery help in the dedicated guide
- THEN the guide MUST describe corrective actions or re-check steps for that class of failure
- AND the guide MUST explain when to retry AgentSync after fixing the environment
- AND the guide MUST keep the recovery scope focused on setup and readiness guidance rather than
  becoming a broad troubleshooting catalog

---

### Requirement: Documentation Defines Mixed-Platform Team Guidance For Windows Setup

The documentation MUST explain how the dedicated Windows guide fits into mixed-platform team usage
so maintainers and collaborators can keep platform-specific setup separate from shared repository
workflow policy.

That guidance MUST clarify that repository policy, gitignore decisions, maintainer responsibilities,
and collaborator responsibilities remain defined by the shared workflow documentation, while the
Windows guide covers only platform-specific readiness and setup concerns.

The documentation SHOULD help teams link Windows-specific onboarding needs without forcing
non-Windows readers through platform-specific instructions.

#### Scenario: Maintainer documents onboarding for a mixed-platform team

- GIVEN a maintainer supports a team that includes both Windows and non-Windows contributors
- WHEN the maintainer updates onboarding or references AgentSync workflow docs
- THEN the documentation MUST let the maintainer send Windows users to the dedicated Windows guide
  for setup prerequisites
- AND the maintainer MUST still be able to rely on the shared workflow guide as the source of truth
  for repository process

#### Scenario: Windows collaborator needs platform setup without a separate team policy

- GIVEN a Windows collaborator joins a repository that follows the shared AgentSync workflow
- WHEN the collaborator uses the dedicated Windows guide
- THEN the collaborator MUST receive Windows-specific setup guidance
- AND the guide MUST point back to the shared workflow documentation for
  maintainer-versus-collaborator process expectations
- AND the documentation MUST avoid creating a separate Windows-only repository policy

---

### Requirement: Supporting Documentation Surfaces Cross-Link To The Windows Guide

Primary supporting documentation surfaces that mention Windows, symlink setup, onboarding,
configuration caveats, or CLI behavior MUST cross-link to the dedicated Windows symlink setup guide
when platform-specific setup context is relevant.

At minimum, this cross-link coverage MUST include the main documentation entry surfaces, getting
started guidance, the canonical gitignore team workflow guide, relevant CLI and configuration
reference surfaces, the repository README, and the npm README.

Those surfaces SHOULD keep the Windows mention concise and use the dedicated guide as the stable
destination for deeper setup detail.

#### Scenario: Reader starts from a supporting docs or README surface

- GIVEN a reader starts from getting started, the canonical workflow guide, the CLI reference, the
  configuration reference, the repository README, or the npm README
- WHEN the page mentions Windows-specific symlink setup context
- THEN the page MUST include a cross-link to the dedicated Windows setup guide
- AND the local explanation MUST remain concise rather than becoming a full Windows setup page

#### Scenario: Reader discovers the Windows guide from a primary entry point

- GIVEN a reader begins at a primary docs entry point such as the docs home page or guide navigation
- WHEN the reader looks for Windows-specific setup help
- THEN the entry point MUST surface or route the reader to the dedicated Windows guide
- AND the route to that guide MUST be consistent with the broader documentation information
  architecture

## MODIFIED Requirements

### Requirement: Windows Notes Stay Minimal And Link-Oriented

Documentation for gitignore team workflows and other shared cross-platform surfaces MUST keep
Windows-specific guidance minimal within shared workflow content.

When Windows-specific caveats are necessary, the documentation SHOULD use short callouts or concise
summaries that link to the dedicated Windows symlink setup guide instead of duplicating the workflow
narrative or reproducing a second full set of Windows-specific workflow steps.

The shared workflow documentation MUST remain the source of truth for cross-platform repository
workflow policy, and the dedicated Windows guide MUST remain the source of truth for Windows setup
prerequisites, verification, and recovery guidance.

(Previously: Documentation for gitignore team workflows had to keep Windows-specific guidance
minimal within shared workflow content, use short callouts or links to targeted platform guidance
when needed, and remain cross-platform without creating a second Windows-specific workflow
narrative.)

#### Scenario: Shared workflow page links out instead of duplicating Windows setup

- GIVEN a Windows user reads the canonical workflow guide or another shared workflow-oriented docs
  surface
- WHEN the page reaches a Windows-specific caveat
- THEN the page MUST keep the Windows note brief
- AND the page MUST link to the dedicated Windows symlink setup guide for deeper setup detail
- AND the page MUST NOT restate the full Windows setup or duplicate the full team workflow narrative
  from the canonical workflow documentation

#### Scenario: Documentation remains maintainable across shared and platform-specific pages

- GIVEN maintainers update onboarding or workflow documentation over time
- WHEN they review Windows-related content across the docs set
- THEN shared pages MUST continue to carry only concise Windows notes
- AND the dedicated Windows guide MUST remain the canonical place for Windows setup specifics
- AND general workflow content MUST NOT be duplicated across both documentation areas
