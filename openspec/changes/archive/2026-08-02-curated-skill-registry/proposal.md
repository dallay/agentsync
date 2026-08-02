# Proposal: Curated, Verifiable Skill Registry

## Intent

Replace the fragile remote catalog/provider dependency with a versioned, curated registry inspired by the catalog model of `midudev/autoskills`, without copying its protected code or content. AgentSync should resolve curated skills from pinned upstream commits, record provenance and commit SHA, verify file hashes, validate manifests, and remain compatible with the existing TOML recommendation catalog.

## Scope

### In Scope
- Define a versioned registry format for curated skills, provenance, upstream commit SHA, per-file SHA-256 hashes, licenses, and validation metadata.
- Curate AgentSync-owned and upstream skills as registry entries while preserving `catalog.v1.toml` recommendation IDs and provider IDs.
- Add deterministic local/packaged registry resolution with integrity, manifest, compatibility, and license-policy validation.
- Define remote fallback behavior, migration from current provider resolution, and E2E fixtures/tests that do not depend on mutable remote HEAD.

### Out of Scope
- Copying `autoskills` source, text, assets, or protected catalog content.
- Rewriting the recommendation/detection model or changing the installed `registry.json` contract.
- Automatically relicensing upstream skills or redistributing content without confirming upstream terms.

## Capabilities

### New Capabilities
- `curated-skill-registry`: Versioned registry with provenance, pinned commits, hashes, validation, and deterministic resolution.
- `skill-provenance-and-license-policy`: License/provenance records and acceptance rules for curated and upstream content.

### Modified Capabilities
- `skill-recommendations`: Recommendations remain TOML-compatible while resolving curated entries before remote fallback.
- `skill-lifecycle`: Install/update verification consumes registry pins and hashes without changing installed-state semantics.

## Approach

Introduce a checked-in or packaged registry as the primary source. Each entry identifies canonical skill ID, source repository/path, commit SHA, expected manifest metadata, file hashes, license/spdx evidence, and registry schema version. Validate the registry before use; fetch only the pinned upstream archive when content is not bundled, then verify extracted paths and hashes before installation. Keep remote provider fallback opt-in/explicit, pinned where possible, observable, and disabled for policy-invalid or unverifiable entries. Migrate catalog entries incrementally, preserving TOML keys and output shape.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/skills/catalog.rs`, `catalog.v1.toml` | Modified | Preserve recommendations; add registry linkage and validation. |
| `src/skills/provider.rs`, `install.rs` | Modified | Curated-first resolution, pinned fetch, hash/license checks, fallback policy. |
| `src/skills/registry.rs` | Modified | Keep installed registry contract; add provenance only where compatible. |
| `tests/test_catalog_integration.rs`, E2E fixtures/workflows | Modified | Offline deterministic catalog tests and migration coverage. |
| `openspec/specs/` | Modified | Durable registry, provenance, and recommendation deltas. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `autoskills` is CC BY-NC 4.0 and cannot be treated as reusable catalog/content | High | Reimplement behavior independently; do not copy code/text/assets; document attribution only where legally required. |
| Upstream skill licenses are incompatible or missing | High | Require SPDX/license evidence and maintainer review; reject or quarantine entries lacking permission. |
| Pinned hashes drift from upstream | Med | Immutable commit pin, deterministic regeneration, CI validation, explicit refresh PRs. |
| Remote fallback reintroduces flaky E2E | Med | Make fixtures/local registry primary; gate network fallback and assert clear diagnostics. |

## Rollback Plan

Keep the existing TOML/provider path behind a compatibility switch during migration. If registry validation or installs regress, disable curated-first resolution, revert registry/catalog linkage, and retain existing installed `registry.json`; remove only the new registry artifacts after confirming no user state depends on them.

## Dependencies

- License review for `autoskills` (CC BY-NC 4.0) and every upstream skill.
- Stable upstream commit SHAs and reproducible hash-generation tooling.
- E2E access to local fixtures; network tests remain explicitly gated.

## Success Criteria

- [ ] Curated entries resolve deterministically from versioned metadata and pinned commits.
- [ ] Hash, manifest, provenance, compatibility, and license-policy failures are explicit and non-destructive.
- [ ] Existing TOML recommendations and JSON/CLI recommendation contracts remain compatible.
- [ ] E2E passes using local/pinned fixtures without mutable remote HEAD dependency.
- [ ] Migration and remote fallback behavior are documented and reversible.
