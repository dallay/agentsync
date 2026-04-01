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
- THEN the docs MUST explain that AgentSync-managed `.gitignore` entries include root-scoped patterns for those destinations
- AND the explanation MUST align with current gitignore-management behavior

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

### Requirement: Windows Notes Stay Minimal And Link-Oriented

Documentation for gitignore team workflows MUST keep Windows-specific guidance minimal within the shared workflow content.

When Windows-specific caveats are necessary, the documentation SHOULD use short callouts or links to targeted platform guidance instead of duplicating the full workflow narrative.

The shared workflow documentation MUST remain valid for cross-platform readers without creating a second Windows-specific set of workflow steps.

#### Scenario: Windows reader uses the canonical workflow guide

- GIVEN a Windows user reads the canonical workflow guide
- WHEN the guide reaches a platform-specific caveat
- THEN the guide MUST keep the Windows note brief
- AND the guide MUST link to more specific platform guidance when needed
- AND the guide MUST NOT repeat the full workflow in a separate Windows-only section

#### Scenario: Cross-platform docs remain maintainable

- GIVEN maintainers update gitignore team workflow documentation over time
- WHEN they review Windows-related content in the affected docs surfaces
- THEN the shared workflow explanation MUST remain the primary source of truth
- AND Windows-specific details MUST remain link-oriented rather than deeply duplicated
