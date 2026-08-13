# Delta for Skill Recommendations

## MODIFIED Requirements

### Requirement: Install Resolution Uses Provider Skill ID

`install_selected_with()` MUST pass `recommendation.provider_skill_id` to `provider.resolve()`, not
`recommendation.skill_id`. Approved Phase 1 entries MUST use committed validated local sources; mutable
`archive/HEAD.zip` MUST NOT satisfy approval. The local ID MUST remain the registry key, install
parameter, folder name, result, and display value.

(Previously: provider IDs were used for resolution and local IDs remained installation identities.)

#### Scenario: Qualified ID reaches resolution

- GIVEN local ID `accessibility` and provider ID `dallay/agents-skills/accessibility`
- WHEN installation resolves the recommendation
- THEN `provider.resolve()` MUST receive the provider ID, never the local ID

#### Scenario: Provider routing remains qualified

- GIVEN a `SkillsShProvider` and provider ID `dallay/agents-skills/docker-expert`
- WHEN `resolve()` receives that ID
- THEN deterministic routing MUST target that qualified skill

#### Scenario: Local ID controls installation state

- GIVEN local ID `accessibility` and successful resolution
- WHEN installation completes
- THEN install parameter, registry key, and folder MUST use `accessibility`

#### Scenario: Install-all preserves both IDs

- GIVEN three uninstalled recommendations with distinct provider and local IDs
- WHEN install-all runs
- THEN resolves MUST use provider IDs and registry entries MUST use local IDs

## ADDED Requirements

### Requirement: Approved Local Curated Source Resolution

Each Phase 1 entry MUST have a committed validated source in `agents-skills`. Resolution MAY use
sibling checkout or `AGENTSYNC_LOCAL_SKILLS_REPO`, but MUST NOT use network or mutable archives. Missing
sources MUST block or leave entries unmigrated; none may be invented.

#### Scenario: Approved source resolves offline

- GIVEN an approved entry with a committed curated source
- WHEN installation runs without network access
- THEN it MUST resolve locally and not contact an external provider

#### Scenario: Missing source blocks

- GIVEN a candidate has no committed validated local source
- WHEN migration or focused validation runs
- THEN it MUST be reported blocked or unmigrated

### Requirement: Catalog Source Updates Preserve Local IDs

Approved definitions MUST set `provider_skill_id` to `dallay/agents-skills/{local_skill_id}`, remove
stale `install_source`, and preserve local ID, title, and summary. The base `clerk` router and Wispbit
SQLAlchemy entry MUST remain external and outside Phase 1.

#### Scenario: Remap preserves metadata

- GIVEN an approved definition with local ID, title, and summary
- WHEN its source is migrated
- THEN provider identity MAY change while local metadata and state semantics MUST remain unchanged

### Requirement: Companion and Provenance Gates

Migrated skills MUST include `SKILL.md` and all required or referenced companions at expected paths.
Approval metadata MUST record source path, immutable commit/file identity, attribution, license evidence
or permission, and companion status. Missing companions or authoritative evidence MUST block approval.

#### Scenario: Complete attributable source is eligible

- GIVEN companions, immutable identity, attribution, and authoritative license evidence exist
- WHEN approval is evaluated
- THEN the entry MAY enter focused validation

#### Scenario: Incomplete or unsupported source blocks

- GIVEN a companion is absent or MIT appears only in frontmatter/README
- WHEN approval is evaluated
- THEN the blocker MUST be recorded and the entry MUST remain unmigrated

### Requirement: Focused Phase 1 Installation Validation

Focused validation MUST install every approved entry offline through the existing lifecycle and verify
`SKILL.md`, the local directory, and its local ID in `registry.json`. It MUST exclude unapproved entries
and MUST NOT pass by omission or external fallback.

#### Scenario: Approved subset installs and registers

- GIVEN approved entries pass source, companion, and provenance gates
- WHEN focused validation runs without network access
- THEN every entry MUST install and its local ID MUST be recorded in `registry.json`

#### Scenario: Incomplete subset fails

- GIVEN an expected entry lacks a source or required content
- WHEN focused validation runs
- THEN it MUST identify the entry and blocker and MUST NOT pass by omission

### Requirement: Full-Catalog E2E Early Return Is Preserved

The full-catalog E2E MUST retain its known-issue early return while unrelated external failures remain.
Focused validation MUST be separate and MUST NOT claim full-catalog green status.

#### Scenario: Focused success remains scoped

- GIVEN Phase 1 focused validation passes while unrelated entries remain broken
- WHEN catalog integration tests run
- THEN the early return MUST remain
- AND results MUST distinguish Phase 1 from full-catalog coverage
