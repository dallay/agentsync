# Tasks: Curated, Verifiable Skill Registry

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | 650–950 |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | Unit 1 foundation/registry; Unit 2 provider/install; Unit 3 catalog compatibility/E2E/docs |
| Delivery strategy | auto-chain |
| Chain strategy | github-stacked-prs |

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: github-stacked-prs
400-line budget risk: High

Approved scope decision: Option A — metadata + pinned commit SHA + hashes; no vendored bundles.

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Foundation/registry | PR 1 | trunk=`main`; parent_branch=`main`; base=`main`; branch=`curated-skill-registry/unit-1`; position=1; issue/Linear=`pending`; tests and manifest/lock included |
| 2 | Provider/install | PR 2 | trunk=`main`; parent_branch=`curated-skill-registry/unit-1`; base=`curated-skill-registry/unit-1`; branch=`curated-skill-registry/unit-2`; position=2; issue/Linear=`pending`; depends on Unit 1 |
| 3 | Catalog compatibility/E2E/docs | PR 3 | trunk=`main`; parent_branch=`curated-skill-registry/unit-2`; base=`curated-skill-registry/unit-2`; branch=`curated-skill-registry/unit-3`; position=3; issue/Linear=`pending`; depends on Unit 2 |

## Phase 1: Foundation and TDD Contracts

- [x] 1.1 Add failing unit tests in `src/skills/registry.rs` for supported schema, required fields, IDs, full commit SHA, safe paths, and actionable invalid-field diagnostics.
- [x] 1.2 Add deterministic fixtures under `tests/fixtures/curated-skills/` for valid, altered-hash, invalid-license, invalid-manifest, and missing-content cases.
- [x] 1.3 Add failing tests for `RegistryDocument`, `RegistryEntry`, `SourcePin`, `FileHash`, `ManifestExpectation`, `LicenseEvidence`, and `ValidationMetadata` TOML parsing with deterministic `BTreeMap` ordering.
- [x] 1.4 Create `src/skills/registry.v1.toml` and generated `src/skills/registry.lock.toml` entries with provenance, SPDX evidence, pins, hashes, and manifest expectations.
- [x] 1.5 Implement typed loaders and validators in `src/skills/registry.rs`; make the Phase 1 tests pass without changing installed `registry.json` semantics.

## Phase 2: Integrity, Provider, and Install

- [x] 2.1 Add failing tests for normalized expected paths, `SKILL.md`, byte-level SHA-256 verification, license policy, and non-destructive rejection.
- [x] 2.2 Implement staging validation in `src/skills/install.rs`, including manifest/hash/license checks before atomic replacement or installed-registry writes.
- [x] 2.3 Add failing provider tests for local-first resolution, pinned archive URLs, explicit fallback, no mutable `HEAD`, and clear offline failure diagnostics.
- [x] 2.4 Implement `PinnedProvider`/common source resolution in `src/skills/provider.rs`, wiring registry entries to local fixtures or pinned upstream archives.
- [x] 2.5 Add integration tests for `catalog -> registry -> provider -> install`, rollback, provenance capture, and preservation of existing installed content.

## Phase 3: Compatibility and Maintainer Tooling

- [x] 3.1 Add failing compatibility tests in `tests/test_catalog_integration.rs` and `tests/test_catalog_integrity.rs` for recommendation IDs, provider IDs, aliases, and JSON/CLI output.
- [x] 3.2 Update `src/skills/catalog.rs` and `catalog.v1.toml` with optional registry linkage while preserving existing contracts and legacy switch behavior.
- [x] 3.3 Add failing CLI tests for registry validation/sync, then implement the maintainer command in `src/commands/skill.rs` with atomic manifest+lockfile refresh.

## Phase 4: CI, E2E, and Policy Documentation

- [x] 4.1 Add offline reproducibility E2E coverage and gated fixed-commit remote E2E coverage in `.github/workflows/catalog-e2e.yml`.
- [x] 4.2 Verify twice-run identical outcomes plus hash, manifest, license, rollback, and migration scenarios using local fixtures by default.
- [x] 4.3 Document provenance, SPDX/dual-license approval, attribution, protected-content restrictions, migration switch, and explicit remote refresh policy in the relevant `website/docs/src/content/docs/` guide.
- [x] 4.4 Run focused Rust tests, offline E2E, formatting, and strict clippy; record the open maintainer decisions on command location and dual-license redistribution.
