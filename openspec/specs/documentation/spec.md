# Documentation Specification

## Purpose

Defines how AgentSync documentation explains gitignore-related team workflows so readers can adopt the default managed-gitignore path or the intentional committed-symlink opt-out path without changing product defaults or relying on duplicated guidance across docs surfaces.

## Requirements

### Requirement: Canonical Gitignore Team Workflow Guide

The documentation set MUST provide one canonical guide for gitignore-related team workflows.

The canonical guide MUST explain, in a decision-oriented and step-by-step format, how a team adopts AgentSync, what maintainers do, and what collaborators do for both the default managed-gitignore workflow and the intentional committed-symlink workflow.

Supporting pages SHOULD summarize the workflows briefly but MUST defer detailed workflow instructions to the canonical guide.

#### Scenario: Reader needs one primary workflow guide

- GIVEN a reader wants to understand how a team should use AgentSync with gitignore
- WHEN the reader navigates the documentation
- THEN the documentation MUST present one primary guide dedicated to gitignore team workflows
- AND the guide MUST include step-by-step instructions for both supported workflows
- AND the guide MUST distinguish maintainer adoption steps from collaborator expectations

#### Scenario: Supporting page avoids duplicating the full workflow

- GIVEN a reader starts from a supporting page such as getting started, configuration, CLI reference, README, or npm README
- WHEN the page discusses gitignore-related workflow choices
- THEN the page MUST provide a concise summary or decision cue
- AND the page MUST link to the canonical workflow guide for full instructions
- AND the page MUST NOT become a second full copy of the workflow guide

---

### Requirement: Documentation Preserves Managed-Gitignore As The Default Workflow

The documentation MUST state that `[gitignore].enabled = true` remains the default behavior.

The documentation MUST explain that, in the default workflow, AgentSync manages a marker-delimited block in `.gitignore` and that managed entries include root-scoped patterns when managed destinations live at the repository root.

The documentation MUST describe the default workflow as the recommended starting point for teams unless they intentionally choose to commit managed destinations.

#### Scenario: Reader evaluates the default workflow

- GIVEN a reader is adopting AgentSync without a special need to commit managed destinations
- WHEN the reader follows the workflow documentation
- THEN the docs MUST identify managed gitignore mode as the default workflow
- AND the docs MUST state that `[gitignore].enabled = true` is the default configuration
- AND the docs MUST explain that AgentSync writes and updates a managed block in `.gitignore`

#### Scenario: Reader needs accurate details about managed entries

- GIVEN a repository has managed destinations at the repository root
- WHEN the reader reviews the default workflow documentation
- THEN the docs MUST describe that AgentSync-managed `.gitignore` entries include root-scoped patterns for those destinations
- AND this guidance MUST align with current gitignore-management behavior

---

### Requirement: Documentation Explains Committed-Symlink Mode As An Opt-Out Workflow

The documentation MUST explain that setting `[gitignore].enabled = false` is an intentional opt-out workflow for teams that choose to commit AgentSync-managed destinations.

The documentation MUST NOT describe committed-symlink mode as the default or implied recommendation.

The documentation MUST explain that, when this mode is active, `agentsync apply` removes an existing AgentSync-managed `.gitignore` block using marker-aware cleanup and preserves unmanaged `.gitignore` lines.

#### Scenario: Reader evaluates the opt-out workflow

- GIVEN a team intends to commit AgentSync-managed destinations to version control
- WHEN the reader reviews the workflow guide
- THEN the docs MUST present `[gitignore].enabled = false` as an opt-out workflow for that team choice
- AND the docs MUST explain when this workflow is appropriate
- AND the docs MUST avoid implying that this workflow replaces the default

#### Scenario: Reader needs accurate cleanup behavior

- GIVEN a repository previously used managed gitignore mode
- AND the team switches to `[gitignore].enabled = false`
- WHEN the reader reviews the workflow documentation
- THEN the docs MUST explain that `agentsync apply` removes the matching AgentSync-managed block from `.gitignore`
- AND the docs MUST explain that the cleanup is marker-aware
- AND the docs MUST explain that unmanaged `.gitignore` content is preserved

---

### Requirement: Documentation Covers Migration And Staging Guidance After Init Or Apply

The documentation MUST explain what maintainers should check, stage, or expect after `agentsync init` and after `agentsync apply` for each workflow.

The documentation MUST describe expected repository diffs in practical terms, including `.gitignore` updates in the default workflow and committed managed-destination changes in the opt-out workflow.

The documentation MUST explain the effect of `agentsync apply --no-gitignore` in workflow terms as a strict opt-out from gitignore reconciliation for that invocation.

#### Scenario: Maintainer reviews diffs after adopting default workflow

- GIVEN a maintainer has run `agentsync init` or updated config for the default managed-gitignore workflow
- WHEN the maintainer reads the workflow guide before staging changes
- THEN the docs MUST explain that `.gitignore` changes are expected after `agentsync apply`
- AND the docs MUST identify which repository changes should be reviewed and staged for the default workflow
- AND the docs MUST describe the expected diff at a practical level rather than only as raw configuration reference

#### Scenario: Maintainer reviews diffs after opting out of gitignore management

- GIVEN a maintainer has configured `[gitignore].enabled = false`
- WHEN the maintainer reads the workflow guide after running `agentsync apply`
- THEN the docs MUST explain that stale AgentSync-managed `.gitignore` blocks may be removed
- AND the docs MUST explain that managed destination changes may now be committed as part of the repository workflow
- AND the docs MUST describe `--no-gitignore` as skipping `.gitignore` reconciliation for that command run

---

### Requirement: Documentation Defines Collaborator Expectations And Prepare-Hook Guidance

The documentation MUST explain what collaborators are expected to do after pulling repository changes for each workflow.

The documentation MUST describe prepare-hook guidance in a way that helps teams understand when collaborators can rely on automated setup and when they still need to run AgentSync explicitly.

The documentation SHOULD clarify expected collaborator outcomes such as refreshed symlinks, updated managed files, or no-op runs, without requiring readers to infer workflow behavior from implementation details.

#### Scenario: Collaborator joins a repository using the default workflow

- GIVEN a collaborator pulls changes in a repository that uses the default managed-gitignore workflow
- WHEN the collaborator consults the workflow documentation
- THEN the docs MUST explain what command or hook-driven setup the collaborator is expected to run or rely on
- AND the docs MUST explain the expected local outcome after that setup
- AND the docs MUST make clear that the repository remains in the default managed-gitignore mode

#### Scenario: Team relies on prepare-hook automation

- GIVEN a team uses package-manager prepare hooks or equivalent onboarding automation around AgentSync
- WHEN a reader reviews collaborator guidance
- THEN the docs MUST explain how the prepare-hook workflow fits into collaborator setup expectations
- AND the docs MUST clarify any remaining cases where a collaborator should run AgentSync manually
- AND the guidance MUST remain compatible with both documented team workflows

---

### Requirement: Supporting Documentation Surfaces Remain Consistent And Cross-Linked

Getting-started, configuration, CLI, README, and npm README documentation that mention gitignore-related workflow behavior MUST remain consistent with the canonical guide.

Those surfaces MUST use terminology that preserves the default managed-gitignore mode, accurately describes the committed-symlink opt-out mode, and aligns with current `--no-gitignore` semantics.

When those surfaces mention gitignore workflow decisions, they MUST cross-link to the canonical guide.

#### Scenario: Reader compares guide and configuration reference

- GIVEN a reader moves between the canonical guide and the configuration reference
- WHEN the reader compares the description of `[gitignore].enabled`
- THEN both pages MUST agree that enabled mode is the default
- AND both pages MUST agree that disabled mode is an intentional opt-out
- AND the configuration reference MUST link to the canonical guide for workflow-oriented instructions

#### Scenario: Reader compares guide and CLI or README surfaces

- GIVEN a reader starts from CLI docs, the repository README, or the npm README
- WHEN the reader looks for gitignore workflow guidance
- THEN those surfaces MUST use behavior descriptions that are consistent with the canonical guide
- AND they MUST link to the canonical guide when deeper workflow explanation is needed
- AND they MUST accurately describe `agentsync apply` and `--no-gitignore` in workflow terms

---

### Requirement: Dedicated Windows Symlink Setup Guide And Information Architecture

The documentation set MUST provide one dedicated, linkable Windows symlink setup guide as the canonical destination for Windows-specific AgentSync setup guidance.

The guide MUST live in the guides documentation area and MUST be discoverable from the primary documentation information architecture, including guide navigation or another equivalent high-visibility guide index surface.

The dedicated guide MUST answer what a Windows user needs to do before AgentSync symlinks work reliably, while deferring general team workflow policy and cross-platform process guidance to the existing canonical workflow documentation.

#### Scenario: Windows reader needs a canonical setup destination

- GIVEN a Windows user needs platform-specific setup help for AgentSync symlinks
- WHEN the reader navigates the documentation or follows a cross-link from another docs surface
- THEN the documentation MUST provide one dedicated Windows symlink setup guide as the canonical destination
- AND the guide MUST be directly linkable
- AND the guide MUST focus on Windows environment readiness rather than reproducing the full team workflow narrative

#### Scenario: Guide is discoverable from primary docs navigation

- GIVEN a reader starts from a primary docs entry point such as the guides index or guide navigation
- WHEN the reader looks for setup help related to Windows symlink behavior
- THEN the dedicated Windows guide MUST be discoverable from that information architecture
- AND the navigation wording MUST make clear that the page is for Windows-specific symlink setup guidance

---

### Requirement: Documentation Explains Native Windows Symlink Prerequisites

The dedicated Windows guide MUST explain the native Windows prerequisites that affect whether AgentSync-managed symlinks can be created reliably.

That guidance MUST describe prerequisite categories such as permission or policy requirements, required shell or execution context expectations, and any setup conditions the reader should verify before relying on native Windows symlink creation.

The documentation MUST describe these prerequisites as environment-readiness guidance and MUST NOT imply that AgentSync changes its product defaults or workflow semantics on Windows.

#### Scenario: Native Windows user evaluates prerequisites before setup

- GIVEN a reader intends to use AgentSync from native Windows
- WHEN the reader opens the dedicated Windows guide
- THEN the guide MUST explain the prerequisites that affect native symlink creation
- AND the guide MUST identify what the reader should confirm before running AgentSync
- AND the guide MUST frame those prerequisites as Windows environment requirements rather than product configuration changes

#### Scenario: Native Windows guidance stays consistent with shared workflows

- GIVEN a team uses the documented default or opt-out gitignore workflow
- WHEN a Windows user reviews native prerequisite guidance
- THEN the documentation MUST keep the workflow policy consistent with the shared cross-platform docs
- AND the guide MUST avoid implying that Windows requires a separate workflow variant

---

### Requirement: Documentation Positions WSL As An Optional Lower-Friction Path

The dedicated Windows guide MUST describe WSL as an optional lower-friction setup path when native Windows symlink prerequisites are difficult, unavailable, or undesirable for the reader.

The documentation SHOULD explain when a reader might prefer WSL and what expectation that choice changes for local setup context.

The documentation MUST NOT imply that WSL is mandatory for Windows users or that it replaces the shared team workflow guidance.

#### Scenario: Reader needs an alternative to native Windows setup friction

- GIVEN a Windows reader cannot easily satisfy native symlink prerequisites or wants a lower-friction development setup
- WHEN the reader reviews the Windows guide
- THEN the guide MUST present WSL as an available alternative
- AND the guide MUST explain when WSL may be the simpler option
- AND the guide MUST avoid presenting WSL as a required default for all Windows users

#### Scenario: WSL guidance remains scoped to setup positioning

- GIVEN a reader follows the WSL-oriented portion of the Windows guide
- WHEN the reader looks for workflow instructions beyond environment setup
- THEN the guide MUST point back to the canonical shared workflow documentation for team process details
- AND the WSL guidance MUST remain focused on setup positioning rather than duplicating the full workflow narrative

---

### Requirement: Documentation Includes Windows Verification And Recovery Guidance

The dedicated Windows guide MUST include practical verification guidance that helps a reader confirm the selected setup path is ready for AgentSync symlink usage.

The guide MUST include recovery guidance for common setup failures within scope, such as unmet prerequisites, permission-related failures, or a need to re-run setup and re-apply symlinks after correcting the environment.

The verification and recovery guidance SHOULD help a reader distinguish between environment-readiness problems and normal shared workflow expectations.

#### Scenario: Reader verifies a Windows setup path before relying on it

- GIVEN a Windows reader has completed either native prerequisite setup or a WSL-based setup path
- WHEN the reader consults the guide before continuing with AgentSync usage
- THEN the guide MUST describe how to verify that symlink setup is working as expected
- AND the verification steps MUST be actionable enough for the reader to confirm readiness

#### Scenario: Reader needs recovery steps after a setup failure

- GIVEN a Windows reader encounters a symlink-related setup problem that falls within the guide's scope
- WHEN the reader looks for recovery help in the dedicated guide
- THEN the guide MUST describe corrective actions or re-check steps for that class of failure
- AND the guide MUST explain when to retry AgentSync after fixing the environment
- AND the guide MUST keep the recovery scope focused on setup and readiness guidance rather than becoming a broad troubleshooting catalog

---

### Requirement: Documentation Defines Mixed-Platform Team Guidance For Windows Setup

The documentation MUST explain how the dedicated Windows guide fits into mixed-platform team usage so maintainers and collaborators can keep platform-specific setup separate from shared repository workflow policy.

That guidance MUST clarify that repository policy, gitignore decisions, maintainer responsibilities, and collaborator responsibilities remain defined by the shared workflow documentation, while the Windows guide covers only platform-specific readiness and setup concerns.

The documentation SHOULD help teams link Windows-specific onboarding needs without forcing non-Windows readers through platform-specific instructions.

#### Scenario: Maintainer documents onboarding for a mixed-platform team

- GIVEN a maintainer supports a team that includes both Windows and non-Windows contributors
- WHEN the maintainer updates onboarding or references AgentSync workflow docs
- THEN the documentation MUST let the maintainer send Windows users to the dedicated Windows guide for setup prerequisites
- AND the maintainer MUST still be able to rely on the shared workflow guide as the source of truth for repository process

#### Scenario: Windows collaborator needs platform setup without a separate team policy

- GIVEN a Windows collaborator joins a repository that follows the shared AgentSync workflow
- WHEN the collaborator uses the dedicated Windows guide
- THEN the collaborator MUST receive Windows-specific setup guidance
- AND the guide MUST point back to the shared workflow documentation for maintainer-versus-collaborator process expectations
- AND the documentation MUST avoid creating a separate Windows-only repository policy

---

### Requirement: Supporting Documentation Surfaces Cross-Link To The Windows Guide

Primary supporting documentation surfaces that mention Windows, symlink setup, onboarding, configuration caveats, or CLI behavior MUST cross-link to the dedicated Windows symlink setup guide when platform-specific setup context is relevant.

At minimum, this cross-link coverage MUST include the main documentation entry surfaces, getting started guidance, the canonical gitignore team workflow guide, relevant CLI and configuration reference surfaces, the repository README, and the npm README.

Those surfaces SHOULD keep the Windows mention concise and use the dedicated guide as the stable destination for deeper setup detail.

#### Scenario: Reader starts from a supporting docs or README surface

- GIVEN a reader starts from getting started, the canonical workflow guide, the CLI reference, the configuration reference, the repository README, or the npm README
- WHEN the page mentions Windows-specific symlink setup context
- THEN the page MUST include a cross-link to the dedicated Windows setup guide
- AND the local explanation MUST remain concise rather than becoming a full Windows setup page

#### Scenario: Reader discovers the Windows guide from a primary entry point

- GIVEN a reader begins at a primary docs entry point such as the docs home page or guide navigation
- WHEN the reader looks for Windows-specific setup help
- THEN the entry point MUST surface or route the reader to the dedicated Windows guide
- AND the route to that guide MUST be consistent with the broader documentation information architecture

---

### Requirement: Windows Notes Stay Minimal And Link-Oriented

Documentation for gitignore team workflows and other shared cross-platform surfaces MUST keep Windows-specific guidance minimal within shared workflow content.

When Windows-specific caveats are necessary, the documentation SHOULD use short callouts or concise summaries that link to the dedicated Windows symlink setup guide instead of duplicating the workflow narrative or reproducing a second full set of Windows-specific workflow steps.

The shared workflow documentation MUST remain the source of truth for cross-platform repository workflow policy, and the dedicated Windows guide MUST remain the source of truth for Windows setup prerequisites, verification, and recovery guidance.

#### Scenario: Shared workflow page links out instead of duplicating Windows setup

- GIVEN a Windows user reads the canonical workflow guide or another shared workflow-oriented docs surface
- WHEN the page reaches a Windows-specific caveat
- THEN the page MUST keep the Windows note brief
- AND the page MUST link to the dedicated Windows symlink setup guide for deeper setup detail
- AND the page MUST NOT restate the full Windows setup or duplicate the full team workflow narrative from the canonical workflow documentation

#### Scenario: Documentation remains maintainable across shared and platform-specific pages

- GIVEN maintainers update gitignore team workflow documentation over time
- WHEN they review Windows-related content across the docs set
- THEN shared pages MUST continue to carry only concise Windows notes
- AND the dedicated Windows guide MUST remain the canonical place for Windows setup specifics
- AND general workflow content MUST NOT be duplicated across both documentation areas
