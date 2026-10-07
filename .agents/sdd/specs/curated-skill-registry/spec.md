# Curated Skill Registry Specification

## Purpose

Define a versioned, deterministic source of curated skills that preserves existing TOML
recommendation and installed-registry contracts while making provenance, integrity, licensing, and
fallback behavior verifiable.

## Requirements

### Requirement: Versioned Curated Registry

The registry MUST declare a supported schema version and, for every skill, canonical local ID,
provider ID, repository/path, pinned commit SHA, manifest expectations, file SHA-256 hashes, license
evidence, and validation metadata. Invalid or unsupported registry documents MUST fail explicitly.

#### Scenario: Valid registry loads offline

- GIVEN a versioned registry containing complete metadata for a curated skill
- WHEN the registry is loaded without network access
- THEN it MUST validate and resolve the skill from bundled or local content

#### Scenario: Invalid registry is rejected

- GIVEN a registry with an unsupported schema or missing required provenance
- WHEN it is loaded
- THEN loading MUST fail with a diagnostic naming the invalid field

### Requirement: Provenance and Integrity Validation

Each resolved skill MUST be attributable to its pinned repository/path and commit SHA. Before
installation, the system MUST validate the manifest, expected paths, and every declared file hash;
any mismatch MUST be non-destructive and MUST NOT install content.

#### Scenario: Pinned content passes validation

- GIVEN content fetched from the declared commit and matching all manifest and SHA-256 records
- WHEN validation runs
- THEN the content MUST be eligible for installation

#### Scenario: Hash or manifest mismatch blocks install

- GIVEN fetched content whose file hash or manifest differs from the registry
- WHEN validation runs
- THEN installation MUST be rejected and existing installed content MUST remain unchanged

### Requirement: Deterministic Resolution and Remote Fallback

Resolution MUST prefer bundled/packaged curated content, then fetch only the pinned upstream commit
when content is absent. Mutable remote HEAD resolution MUST NOT be used for curated entries. Remote
fallback MUST be explicit, observable, policy-valid, and disabled when provenance or integrity is
unverifiable.

#### Scenario: Local resolution works offline

- GIVEN a valid curated entry and an available local fixture
- WHEN installation is requested offline
- THEN the local fixture MUST be selected deterministically without network access

#### Scenario: Pinned remote fallback succeeds

- GIVEN no bundled content and a reachable archive for the declared commit
- WHEN fallback is enabled
- THEN the pinned archive MUST be fetched, validated, and installed

#### Scenario: Remote fallback is unavailable

- GIVEN no local content and network access is unavailable
- WHEN installation is requested
- THEN it MUST fail clearly without partial files or registry changes

### Requirement: License and Provenance Policy

Curated entries MUST include SPDX-identifiable license evidence and source provenance. Entries with
missing, incompatible, or unapproved licensing MUST be rejected or quarantined and MUST NOT be
installed or recommended. The registry MUST NOT copy protected catalog/content without permission.

#### Scenario: Approved license permits use

- GIVEN a curated entry with reviewed SPDX evidence and required attribution
- WHEN policy validation runs
- THEN the entry MUST remain eligible

#### Scenario: Missing or incompatible license blocks use

- GIVEN an upstream entry without acceptable license evidence
- WHEN policy validation runs
- THEN it MUST be excluded with an actionable policy diagnostic

### Requirement: TOML Compatibility and Migration

Migration MUST preserve `catalog.v1.toml` recommendation IDs, provider IDs, JSON/CLI output shape,
and installed `registry.json` semantics. Existing provider resolution MUST remain available behind a
compatibility switch while entries migrate incrementally.

#### Scenario: Existing catalog remains compatible

- GIVEN an existing TOML recommendation and no provider overlay
- WHEN suggestions run after migration
- THEN materially equivalent recommendations and stable output contracts MUST be returned

#### Scenario: Migration rollback preserves user state

- GIVEN curated validation is disabled through the compatibility switch
- WHEN a skill is installed through the legacy path
- THEN existing installed directories and `registry.json` semantics MUST remain unchanged

### Requirement: End-to-End Offline and Remote Verification

The test suite MUST include deterministic E2E fixtures for offline resolution, pinned remote
fallback, integrity/license failures, TOML compatibility, and migration. Network tests MUST be
explicitly gated and MUST never depend on mutable remote HEAD.

#### Scenario: Offline E2E is reproducible

- GIVEN the local fixture registry and network access disabled
- WHEN the E2E install flow runs twice
- THEN both runs MUST produce identical resolved content and outcomes

#### Scenario: Gated remote E2E verifies the pin

- GIVEN an explicitly enabled remote fixture for a fixed commit
- WHEN the E2E flow installs the skill
- THEN it MUST verify the commit, hashes, manifest, and resulting installed state
