# Tasks: Phase 1 Catalog Skill Migration

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | 450–700 (gated sibling content, companions, provenance, Rust tests, catalog) |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | PR 1 → sibling gates/content/provenance; PR 2 → resolver/callers/catalog/focused tests |
| Delivery strategy | single-pr with explicit size exception |
| Chain strategy | single-pr |

Decision needed before apply: No — explicit user-approved size exception
Chained PRs recommended: Yes
Chain strategy: single-pr
400-line budget risk: High
Size exception: Approved by user for the coherent three-skill Phase 1 unit; branch `feat/migrate-catalog-skills-phase1` targets `main`.

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Gate and prepare only validated sibling sources | PR 1 | Exclude untracked Clerk, `angular-architecture`, and `typescript-strict-patterns`; validate both repos. |
| 2 | Apply AgentSync resolution/catalog changes and focused tests | PR 2 | Depends on Unit 1; base/branch boundary awaits chain-strategy approval. |

## Phase 1: Gates and RED Tests

- [x] 1.1 **RED:** Add resolver tests in `tests/unit/provider.rs` for override/sibling precedence, missing Phase 1 source fail-closed behavior, and no external fallback.
- [x] 1.2 **RED:** Add caller-propagation tests in the command/provider test modules proving direct and suggestion installs pass the project root, not `None`.
- [x] 1.3 **RED:** Add catalog assertions in `tests/unit/suggest_catalog.rs` for canonical local IDs and unchanged external base `clerk` router, Wispbit SQLAlchemy, and unrelated entries.
- [x] 1.4 **RED:** Add an explicit offline subset test in `tests/test_catalog_integration.rs` that cannot pass by omission or external fallback.
- [x] 1.5 Run `../agents-skills/scripts/validate-skills.py` (and its documented wrapper) on isolated candidates; exclude unrelated untracked skills and record Clerk blockers.
- [x] 1.6 Audit Bobmatnyc paths/companions and Clerk `references/`, `templates/`, and eval companions; leave all Clerk definitions/mappings unchanged unless authoritative license evidence/permission and companions exist.

## Phase 2: Gated Sources and Core Implementation

- [x] 2.1 Create `../agents-skills/PROVENANCE.md` with repo/path, immutable commit/blob IDs, attribution, authoritative license evidence, and companion status; record Clerk as blocked when gates fail.
- [x] 2.2 Prepare only validator-clean, committed Bobmatnyc `drizzle-orm`, `pydantic`, and `sqlalchemy` content under `../agents-skills/skills/`; do not commit unrelated files.
- [x] 2.3 Implement sibling local resolver caller propagation in `src/commands/skill.rs`, then make the RED caller tests pass.
- [x] 2.4 Implement the narrow Phase 1 fail-closed guard in `src/skills/provider.rs`, preserving local precedence and unrelated external fallback.
- [x] 2.5 Apply catalog mapping in `src/skills/catalog.v1.toml` only for approved entries: qualified IDs, removed stale sources, affected mappings, and preserved metadata; never map blocked Clerk entries.

## Phase 3: Integration and Verification

- [x] 3.1 Make the focused test install every approved entry offline and verify directory, `SKILL.md`, required companions, and local ID in `registry.json`.
- [x] 3.2 Preserve the known full-catalog early return, `#[ignore]`, and `RUN_E2E` gate in `tests/test_catalog_integration.rs`; distinguish subset results from full-catalog status.
- [x] 3.3 Run both-repo validation against the committed sibling revision: target validator/companion audit plus AgentSync focused/unit tests and formatting checks.
- [x] 3.4 Verify `src/skills/registry.v1.toml`, `src/skills/registry.lock.toml`, unrelated sibling skills, and installed-state semantics remain unchanged.

## QA Remediation

- [x] 4.1 Add a reproducible external-CLI acceptance harness with isolated sibling, override, and missing-source fixtures for the three migrated skills.
- [x] 4.2 Exercise direct and suggestion installs through an explicit project root, assert companions and local registry keys, and preserve the full-catalog early return by keeping the harness Phase 1-only.
- [x] 4.3 Refresh the eight stale materialized SHA-256 entries in `../agents-skills/PROVENANCE.md` and add focused provenance validation to the sibling validation wrapper.
- [x] 4.4 Document the acceptance command and hand off results without changing production source or promoting QA to PASS.
