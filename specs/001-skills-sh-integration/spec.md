# Feature Specification: Skills.sh Integration

**Feature Branch**: `001-skills-sh-integration`  
**Created**: 2026-01-27  
**Status**: Draft  
**Input**: User description: "integrate support for installing/adding skills from the skills.sh ecosystem into the AgentSync tool, supporting only skills.sh as the source, and following the skills standards -> https://agentskills.io/specification"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Install skill from skills.sh (Priority: P1)

As a repository maintainer, I want to install a skill from the skills.sh ecosystem using AgentSync
so that the skill's assets and metadata are synchronized into my project with minimal manual steps.

**Why this priority**: Installing third-party skills is the core capability; early adopters expect a
simple install flow.

**Independent Test**: Run the AgentSync CLI command to install a known skills.sh skill and verify
that the expected files (skill manifest, assets, and any generated links) appear in the project and
that the CLI exits with success.

**Acceptance Scenarios**:

1. **Given** a repository with AgentSync initialized, **When** the user runs the `agentsync
   skill install <skill-id>` command for a skills.sh-hosted skill, **Then** the skill files are
   downloaded/linked into `.agents/skills/<skill-id>/` and a record is added to the agentsync
   configuration.
2. **Given** an already-installed skill, **When** the user runs `agentsync skill update <skill-id>`,
   **Then** AgentSync updates the skill to the latest compatible version following semantic
   compatibility rules.

---


### Edge Cases

- What if a skill references unsupported asset types? AgentSync should reject with a clear error
  and a remediation suggestion.
- What if skills.sh is offline or returns malformed manifests? AgentSync should fail the
  operation with a descriptive error and no partial state left behind.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: AgentSync MUST provide a CLI command `agentsync skill install <skill-id>` that
  installs a skill from the default provider (skills.sh) into the repository's `.agents/skills/`
  directory.
- **FR-002**: AgentSync MUST support `agentsync skill update <skill-id>` to fetch and apply
  updates in a controlled manner (follow semantic compatibility rules provided by the skill
  manifest).
- **FR-003**: AgentSync MUST maintain a local registry file (e.g., `.agents/skills/registry.json`)
  recording installed skills, provider metadata, installed versions, and source URLs.
- **FR-005**: AgentSync MUST validate skill manifests against the Agent Skills Specification
  (https://agentskills.io/specification) and reject non-conforming skills with a helpful error.

### Non-Functional Requirements (MANDATORY)

- **NFR-001**: Operations that modify the repository MUST be idempotent; failed installs must
  leave the repository in a clean pre-operation state.
- **NFR-002**: Install/update operations MUST provide clear progress output and machine-readable
  output mode (JSON) for automation.
- **NFR-004**: Security: AgentSync MUST not execute arbitrary code from downloaded skills during
  installation; any executable assets MUST be flagged and require explicit user consent.

*Assumptions*:

- The skills.sh ecosystem uses a manifest format compatible with or mappable to the Agent Skills
  Specification linked by the user.

### Key Entities *(include if feature involves data)*

- **Skill**: identifier, name, version, manifest, assets (files), dependencies.
- **Registry Entry**: skill-id, installed-version, install-date, manifest-hash.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A maintainer can install a public skills.sh skill into a fresh repository using a
  single AgentSync command with success on the first attempt in over 95% of trials during tests.
- **SC-003**: The CLI returns machine-readable JSON on install/update operations when requested
  and contains `skill_id`, `installed_version`, `provider`, and `status` fields.
- **SC-004**: Manifest validation rejects non-conforming manifests and provides clear error
  messages with remediation steps; false acceptance rate is 0% in test fixtures.

## Edge Cases

- Network failures during install must roll back partial changes and present a clear retry path.
- Conflicting assets (file path collisions) must be detected and require explicit user resolution.

---

## Notes & Next Steps

- Implementation focuses solely on skills.sh as the provider. No plugin/discovery or multi-provider support planned at this stage.
- Add unit and integration tests that exercise the install/update flows.
