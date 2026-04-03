# Specification: agents-skills Monorepo

| Field | Value |
|-------|-------|
| **Change ID** | agents-skills-monorepo |
| **Version** | 1.0 |
| **Status** | Draft |

## Requirements

### REQ-01: Repository Structure

The `dallay/agents-skills` repository MUST follow this structure:

```text
agents-skills/
├── README.md                      # Overview, badges, quick install
├── CONTRIBUTING.md                # How to add/update skills
├── LICENSE                        # MIT (consistent with agentsync)
├── skills/
│   ├── accessibility/
│   │   ├── SKILL.md               # Manifest + instructions (frontmatter required)
│   │   └── ... (optional resources)
│   ├── best-practices/
│   │   └── SKILL.md
│   ├── brainstorming/
│   │   └── SKILL.md
│   ├── core-web-vitals/
│   │   └── SKILL.md
│   ├── docker-expert/
│   │   └── SKILL.md
│   ├── frontend-design/
│   │   └── SKILL.md
│   ├── github-actions/
│   │   └── SKILL.md
│   ├── grafana-dashboards/
│   │   └── SKILL.md
│   ├── makefile/
│   │   └── SKILL.md
│   ├── markdown-a11y/
│   │   └── SKILL.md
│   ├── performance/
│   │   └── SKILL.md
│   ├── pinned-tag/
│   │   └── SKILL.md
│   ├── pr-creator/
│   │   └── SKILL.md
│   ├── rust-async-patterns/
│   │   └── SKILL.md
│   ├── seo/
│   │   └── SKILL.md
│   ├── skill-creator/
│   │   └── SKILL.md
│   ├── sql-optimization-patterns/
│   │   └── SKILL.md
│   ├── web-quality-audit/
│   │   └── SKILL.md
│   └── webapp-testing/
│       └── SKILL.md
└── .github/
    └── workflows/
        ├── validate-skills.yml    # CI: manifest parsing, lint
        └── notify-catalog.yml     # Optional: repository_dispatch to agentsync
```

### REQ-02: SKILL.md Manifest Format

Every skill MUST have a `SKILL.md` file with valid YAML frontmatter:

```yaml
---
name: skill-name          # Required. kebab-case, matches directory name
version: 1.0.0            # Optional. Semver format
description: >            # Required. Short description
  One-line description of what this skill does.
triggers:                 # Required. List of trigger phrases
  - "trigger phrase 1"
  - "trigger phrase 2"
---

# Skill Title

Skill instructions content here...
```

**Validation rules:**
- `name` MUST be kebab-case and MUST match the parent directory name
- `name` MUST NOT contain path separators, dots, or special characters
- `version`, if present, MUST be valid semver
- `description` MUST be non-empty
- `triggers` MUST be a non-empty array of strings
- Content after frontmatter MUST be non-empty

### REQ-03: Deterministic Resolution

When the CLI resolves a skill with `provider_skill_id` matching pattern
`dallay/agents-skills/{skill-name}`, it MUST construct the download URL
deterministically as:

```text
https://github.com/dallay/agents-skills/archive/HEAD.zip#skills/{skill-name}
```

No search API call is needed for this pattern.

### REQ-04: Catalog Integration

The `catalog.v1.toml` in agentsync MUST be updated so that all dallay-owned skills
use the deterministic `provider_skill_id` format:

```toml
[[skills]]
provider_skill_id = "dallay/agents-skills/accessibility"
local_skill_id = "accessibility"
title = "Accessibility"
summary = "WCAG 2.1 audit and improvement guidelines"
```

### REQ-05: CI Validation Pipeline

The `agents-skills` repo MUST have a CI pipeline that:

1. Discovers all `skills/*/SKILL.md` files
2. Parses and validates each manifest (frontmatter schema)
3. Verifies `name` matches directory name
4. Checks for empty content after frontmatter
5. Fails the build on any validation error
6. Runs on every PR and push to main

### REQ-06: Catalog Integrity Check (agentsync side)

The agentsync repo SHOULD have a CI check or test that:

1. Reads all `provider_skill_id` entries from `catalog.v1.toml` that match `dallay/agents-skills/*`
2. Verifies the corresponding `skills/{skill-name}/SKILL.md` exists in the agents-skills repo
3. Reports drift (missing skills, renamed skills) as test failures

This MAY be an E2E test gated behind `RUN_E2E=1`.

### REQ-07: Backward Compatibility

During the transition period:

1. The skills.sh search fallback MUST remain functional for skills not yet migrated
2. Users who previously installed skills via the old resolution path MUST NOT be broken
3. The `registry.json` format MUST NOT change

### REQ-08: Contributing Guidelines

The `CONTRIBUTING.md` MUST document:

1. How to create a new skill (directory structure, manifest format)
2. Quality expectations (useful content, proper triggers, no placeholder text)
3. Naming conventions (kebab-case, descriptive, technology-specific)
4. PR process (what reviewers look for)
5. How to test a skill locally before submitting

## Scenarios

### SC-01: Install a dallay-owned skill

```text
GIVEN the catalog maps "accessibility" to provider_skill_id "dallay/agents-skills/accessibility"
WHEN  the user runs: agentsync skill install accessibility
THEN  the CLI resolves to https://github.com/dallay/agents-skills/archive/HEAD.zip#skills/accessibility
AND   downloads, extracts, validates SKILL.md, and installs to .agents/skills/accessibility/
AND   NO search API call is made
```

### SC-02: Install an externally-owned skill

```text
GIVEN the catalog maps "angular-developer" to provider_skill_id "angular/skills/angular-developer"
WHEN  the user runs: agentsync skill install angular-developer
THEN  the CLI resolves to https://github.com/angular/skills/archive/HEAD.zip#skills/angular-developer
AND   the external resolution path is unchanged
```

### SC-03: Suggest detects and recommends dallay skills

```text
GIVEN a project with Dockerfiles detected
WHEN  the user runs: agentsync skill suggest
THEN  "docker-expert" appears in recommendations
AND   its source shows "dallay/agents-skills"
```

### SC-04: Community submits a new skill via PR

```text
GIVEN a contributor creates skills/terraform/SKILL.md with valid manifest
WHEN  they open a PR to dallay/agents-skills
THEN  CI validates the manifest automatically
AND   reviewers can merge when quality gates pass
AND   a follow-up PR to agentsync adds the catalog entry
```

### SC-05: Catalog drift detection

```text
GIVEN "dallay/agents-skills/old-skill" is in catalog.v1.toml
BUT   skills/old-skill/ was deleted from agents-skills repo
WHEN  the agentsync CI integrity check runs
THEN  the check FAILS with a clear message about the missing skill
```
