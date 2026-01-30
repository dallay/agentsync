# AgentSync Constitution
<!--
  Sync Impact Report
  - Version change: none → 1.0.0 (initialized constitution)
  - Modified principles: (created)
    - I. Code Quality (NON-NEGOTIABLE)
    - II. Test-First & Test Standards (NON-NEGOTIABLE)
    - III. User Experience Consistency
    - IV. Performance Requirements & Budgets
    - V. Observability, Versioning & Release Discipline
  - Added sections: Non-Functional Constraints; Development Workflow & Quality Gates
  - Removed sections: none
  - Templates requiring updates: ✅ .specify/templates/plan-template.md
                                ✅ .specify/templates/tasks-template.md
                                ✅ .specify/templates/spec-template.md
  - Follow-up TODOs: RATIFICATION_DATE unknown (TODO)
-->

## Core Principles

### I. Code Quality (NON-NEGOTIABLE)

Code MUST be statically typed where the language supports it (TypeScript/Rust); every commit that
changes production code MUST pass the configured linters and formatter. All code introduced to the
repository MUST include clear in-code documentation for public functions and modules, and any API
surface MUST include type signatures and example usage. Commits that add TODOs, FIXMEs, or
commented-out dead code MUST include an explicit migration ticket linking to a follow-up task and a
deadline; such temporary markers are only allowed with a tracking issue. Rationale: consistent
formatting, linting, and typing reduce review friction, prevent classes of bugs earlier, and allow
automation (format-on-save, static analysis) to enforce baseline quality.

### II. Test-First & Test Standards (NON-NEGOTIABLE)

All production behavior MUST be accompanied by automated tests that express the intended
behavior. Tests for a given change MUST be written before the implementation (red → green →
refactor). Test categories and expectations:
- Unit tests: isolate single functions/modules and run in <100ms where practical.
- Integration tests: exercise interactions between modules or services using lightweight fixtures.
- Contract tests: when libraries or services expose public contracts, a contract test MUST exist.

Merge policy: no PR touching production code MAY be merged unless the CI run completes and all
tests pass. Projects SHOULD set a minimum coverage threshold (recommended baseline: 80%) for core
libraries; if a lower threshold is accepted, the PR MUST include a documented risk justification.
Rationale: tests written first clarify requirements, create an executable specification, and
prevent regressions throughout the lifecycle.

### III. User Experience Consistency

User-facing surfaces (CLI, web UI, API responses, error messages) MUST follow the project's UX
patterns and documentation. For CLIs: commands MUST be discoverable via `--help`, output MUST
support a machine-readable mode (JSON) alongside human-readable defaults, and error messages MUST
include actionable remediation steps and stable error codes. For web/UI: component libraries and
tokens MUST be reused; new components MUST be documented with examples and accessibility notes.
Every user-facing change MUST include acceptance criteria and at least one end-to-end test or UX
review checklist entry. Rationale: consistent UX reduces cognitive load for users and maintainers
and makes automated testing and observability effective.

### IV. Performance Requirements & Budgets

Every feature that could affect latency, throughput, memory, or cost MUST declare measurable
performance goals in the implementation plan (see Constitution Check in the plan template). Typical
requirements include p95/p99 latency targets, maximum memory footprint, and acceptable CPU usage.
Performance rules:
- Benchmarks or performance tests MUST be included for features that affect hot paths.
- Code changes MUST not regress established performance budgets; CI MUST run lightweight
  performance/regression checks for critical paths where feasible.
- If a change increases resource usage, the PR MUST document the trade-off and provide an
  optimization plan or a flag to opt-in the behavior.

Rationale: quantifiable budgets make performance visible and enforceable; explicit expectations
prevent silent regressions that are expensive to diagnose in production.

### V. Observability, Versioning & Release Discipline

All production services and libraries MUST emit structured logs and expose metrics for key
business and system signals. Tracing is REQUIRED for cross-service request paths where latency and
error attribution matter. Versioning policy:
- Follow semantic versioning for public packages/APIs (MAJOR.MINOR.PATCH).
- Breaking changes MUST be communicated via a changelog entry and migration guidance; a
  migration plan or deprecation window MUST accompany PRs that introduce breaking changes.
- Releases MUST include a short changelog and an explicit rollout plan for production-impacting
  changes.

Rationale: observability and disciplined versioning reduce blast radius, speed incident response,
and make upgrades predictable for downstream consumers.

## Non-Functional Constraints

The project enforces these cross-cutting constraints:
- Security: secrets MUST never be committed; credential scans MUST run in CI; security-sensitive
  changes MUST include a short threat model and testing notes.
- Compatibility: supported runtime/platform versions MUST be declared in the plan (e.g., Rust
  version, Node.js LTS). Changes that drop support for a platform MUST be treated as a breaking
  change and follow the versioning policy above.
- Licensing & Third-Party: any new dependency MUST be approved by maintainers and documented
  with license and triage notes.
- Resource budgets: production services SHOULD list expected cost impact and resource budgets in
  the plan.

## Development Workflow & Quality Gates

- Pull Requests touching production code MUST include: a short description, linked issue/spec,
  tests, and performance/UX notes if applicable.
- Every PR MUST have at least one approving review from a maintainer familiar with the area.
- CI gates: linting, unit tests, and relevant integration/contract tests MUST pass before merge.
- Emergency fixes to production MAY be merged with expedited review but MUST be followed by a
  retrospective and a corrective PR that restores normal guards.
- Regular compliance reviews (quarterly) WILL verify adherence to this constitution and report
  findings to the maintainers.

## Governance

Amendments to this constitution MUST follow this process:

1. Propose amendment as a Pull Request against this file with: rationale, a diff, and a migration
   plan for any required operational changes.
2. The PR MUST include an explicit version bump following semantic versioning rules described
   below.
3. Approval: at least two maintainers or a maintainer + a domain owner (e.g., security owner) MUST
   approve the change. For governance changes that materially alter developer workflows, a
   majority of core maintainers MUST approve.

Versioning policy for this constitution:
- MAJOR: breaking governance changes (removing or redefining non-negotiable principles).
- MINOR: adding principles or materially expanding guidance.
- PATCH: clarifications, wording fixes, and non-semantic refinements.

Compliance & reviews:
- Every PR that implements features MUST reference the relevant principles and explain how the
  change complies (or why an exception is required).
- The project WILL run a constitution compliance review at least once per quarter and publish a
  short status report in the repository (e.g., docs/GOVERNANCE.md or a maintainer issue).

Use the templates in `.specify/templates/` when preparing plans/specs/tasks to ensure gates are
applied consistently.

**Version**: 1.0.1 | **Ratified**: 2026-01-20: original adoption date unknown | **Last Amended**: 2026-01-27
