# Delta for Skill Adoption

## ADDED Requirements

### Requirement: Wizard-Written AGENTS Includes Managed Agent Config Layout Section

When `agentsync init --wizard` writes `.agents/AGENTS.md`, the generated file MUST include exactly
one managed section titled `Agent config layout`.

The managed section MUST be placed prominently near the top of the file, after the file's opening
title or introductory context when present, and before the remainder of the wizard-written
instruction body.

The managed section MUST use stable begin/end markers so a later forced wizard rewrite can replace
the generated section instead of duplicating it.

The managed section MUST explain, in actionable language for a fresh agent, that `.agents/` is the
canonical source for the generated instructions, skills, and commands layout.

#### Scenario: Fresh wizard output includes one managed explainer block near the top

- GIVEN a project with no existing `.agents/AGENTS.md`
- WHEN the user runs `agentsync init --wizard`
- THEN the written `.agents/AGENTS.md` MUST contain exactly one managed `Agent config layout`
  section
- AND that section MUST appear after the file title or opening introduction
- AND that section MUST appear before later wizard-written instruction content

#### Scenario: Wizard output with migrated instruction content keeps explainer block prominent

- GIVEN the wizard is writing `.agents/AGENTS.md` with migrated instruction content from discovered
  agent files
- WHEN the final file is rendered
- THEN the managed `Agent config layout` section MUST remain near the top of the file
- AND the migrated instruction content MUST remain after that managed section

#### Scenario: Forced rewrite replaces existing managed block instead of duplicating it

- GIVEN `.agents/AGENTS.md` already contains the managed `Agent config layout` section written by a
  previous wizard run
- WHEN the user runs `agentsync init --wizard --force`
- THEN the resulting `.agents/AGENTS.md` MUST still contain exactly one managed
  `Agent config layout` section
- AND the regenerated section MUST replace the earlier managed block rather than append a second
  copy

### Requirement: Agent Config Layout Section Reflects Wizard-Generated Destinations And Sync Semantics

The managed `Agent config layout` section MUST be derived from the wizard-generated configuration
that is being written for the project.

The section MUST describe the actual configured canonical source content and the actual enabled
default destinations for instructions, skills, and commands that the wizard generated.

The section MUST NOT invent destinations, agent targets, or sync behavior that are absent from the
generated configuration.

When a described target uses `symlink`, the wording MUST indicate that the destination reflects the
canonical source directly.

When a described target uses `symlink-contents`, the wording MUST indicate that the destination is
populated from the canonical source by sync and that adding, removing, or renaming entries requires
rerunning `agentsync apply`.

#### Scenario: Default wizard layout lists generated instruction, skills, and commands destinations

- GIVEN the wizard generates the default target layout
- WHEN `.agents/AGENTS.md` is written
- THEN the managed `Agent config layout` section MUST describe `.agents/AGENTS.md` as the canonical
  instructions source
- AND the section MUST include the generated instruction destinations `CLAUDE.md`,
  `.github/copilot-instructions.md`, `GEMINI.md`, `OPENCODE.md`, and `AGENTS.md`
- AND the section MUST include the generated skills destinations `.claude/skills`, `.codex/skills`,
  `.gemini/skills`, and `.opencode/skills`
- AND the section MUST include the generated commands destinations `.claude/commands`,
  `.gemini/commands`, and `.opencode/command`

#### Scenario: Skills wording changes with selected sync mode

- GIVEN the wizard generates skills targets where one enabled default target uses `symlink`
- AND another enabled default target uses `symlink-contents`
- WHEN `.agents/AGENTS.md` is written
- THEN the `Agent config layout` section MUST describe the `symlink` skills destination as
  reflecting the canonical skills source directly
- AND the section MUST describe the `symlink-contents` skills destination as requiring
  `agentsync apply` to propagate added, removed, or renamed skill entries

#### Scenario: Layout block omits targets that are not present in generated config

- GIVEN the wizard-generated configuration does not include a default destination for a particular
  instructions, skills, or commands target
- WHEN `.agents/AGENTS.md` is written
- THEN the managed `Agent config layout` section MUST NOT claim that missing destination exists
- AND the section MUST describe only the enabled default targets present in the generated
  configuration

### Requirement: Wizard AGENTS Layout Generation Preserves Existing Non-Forced Behavior And Excludes Apply Mutation

If `.agents/AGENTS.md` already exists and `--force` is not used, `agentsync init --wizard` MUST
preserve the existing file unchanged.

In that case, the wizard MUST NOT inject, prepend, append, or partially replace a managed
`Agent config layout` section.

This change is wizard-only in v1. `agentsync apply` MUST NOT create, refresh, or mutate the managed
`Agent config layout` section in `.agents/AGENTS.md`.

#### Scenario: Existing AGENTS file is preserved without force

- GIVEN `.agents/AGENTS.md` already exists before the wizard runs
- WHEN the user runs `agentsync init --wizard` without `--force`
- THEN `.agents/AGENTS.md` MUST remain byte-for-byte unchanged
- AND no managed `Agent config layout` section SHALL be inserted or updated as part of that run

#### Scenario: Forced rewrite stays idempotent across repeated runs

- GIVEN the wizard can write `.agents/AGENTS.md`
- WHEN the user runs `agentsync init --wizard --force` multiple times without changing the generated
  configuration
- THEN each run MUST produce the same single managed `Agent config layout` section
- AND repeated forced runs MUST NOT accumulate duplicate markers or duplicate explainer content

#### Scenario: Apply does not own AGENTS layout regeneration

- GIVEN `.agents/AGENTS.md` contains a wizard-generated managed `Agent config layout` section
- WHEN the user later runs `agentsync apply`
- THEN `agentsync apply` MUST NOT rewrite that managed section
- AND `agentsync apply` MUST NOT add a new `Agent config layout` section if one is missing
