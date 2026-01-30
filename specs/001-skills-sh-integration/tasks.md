---

description: "Task list for Skills.sh Integration feature"

---

# Tasks: Skills.sh Integration

**Input**: Design documents from `specs/001-skills-sh-integration/`

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure required by the implementation plan

- [x] T001 Create repository directories `src/skills/`, `src/commands/`, `tests/integration/`, `tests/fixtures/` (project structure) with README in `specs/001-skills-sh-integration/`
- [x] T002 Initialize `Cargo.toml` with dependencies: `reqwest`, `tokio`, `clap`, `serde`, `serde_json`, `anyhow`, `thiserror`, `tracing` (update `Cargo.toml`)
- [x] T003 [P] Add formatting and lint config: create `.rustfmt.toml` and `rust-toolchain.toml` in repo root

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core library and CLI plumbing that MUST be complete before any user story work

- [x] T004a Unit test for `Provider` trait in `tests/unit/provider.rs`
- [x] T004 Implement `Provider` trait and core types in `src/skills/provider.rs`
- [x] T005a Unit test for manifest parsing/validation in `tests/unit/manifest.rs`
- [x] T005 Implement manifest parsing/validation + integrate in `src/skills/manifest.rs`
- [x] T006a Unit test for registry management in `tests/unit/registry.rs`
- [x] T006 Implement local registry management for `.agents/skills/registry.json` in `src/skills/registry.rs`
- [x] T007a Unit test for installation skeleton in `tests/unit/install.rs`
- [x] T007 Implement installation skeleton (download/extract/write) in `src/skills/install.rs`
- [x] T008 Add CLI command skeleton for skill operations in `src/commands/skill.rs` and wire into `src/main.rs`
- [x] T009 [P] Add idempotency and rollback helpers in `src/skills/transaction.rs` (ensure failed installs leave repo clean)
- [x] T010 Create integration test harness scaffolding in `tests/integration/mod.rs` and add `tests/fixtures/` layout

**Checkpoint**: After Phase 2, the CLI and library must compile and basic unit/integration test harness must run via `cargo test`

---

## Phase 3: User Story 1 - Install skill from skills.sh (Priority: P1) 🎯 MVP

**Goal**: Allow a repository maintainer to run `agentsync skill install <skill-id>` and have skill files (manifest, assets) appear under `.agents/skills/<skill-id>/` and an entry added to the registry

**Independent Test**: Run `cargo test -- tests/integration/skill_install.rs` or run the built CLI `agentsync skill install rust-async-patterns` against the fixture and verify `.agents/skills/<skill-id>/SKILL.md` exists and `.agents/skills/registry.json` contains the entry

### Tests (requested by spec)

- [x] T011 [P] [US1] Create integration test `tests/integration/skill_install.rs` that runs `agentsync skill install sample-skill` against a local fixture and asserts files and registry entry
- [x] T012 [P] [US1] Add fixture skill under `tests/fixtures/sample-skill/SKILL.md` and `tests/fixtures/sample-skill/assets/icon.png`
- [x] T013 [P] [US1] Implement HTTP fetch and download logic in `src/skills/install.rs` using `reqwest` and `tokio`

- [x] T014 [US1] Implement archive/extraction and safe write to `.agents/skills/<skill-id>/` in `src/skills/install.rs` (prevent path traversal)
- [x] T015 [US1] Combine manifest validation (schema/frontmatter) and wiring into overall install flow, implemented in `src/skills/manifest.rs`/`src/skills/install.rs`
- [x] T016 [US1] Implement registry update logic to write `.agents/skills/registry.json` in `src/skills/registry.rs`
- [x] T017 [US1] Implement CLI `agentsync skill install <skill-id>` end-to-end in `src/commands/skill.rs` and ensure `--json` output option produces machine-readable result 
- [x] T017a [US1] Define JSON output schema and add contract tests asserting schema stability in `tests/contracts/test_install_output.rs`
- [x] T018 [US1] Add user-facing error handling and clear remediation messages in `src/commands/skill.rs` and `src/skills/install.rs` (Standardized error codes, machine- and human-friendly remediation messages; contract tests pass)

**Checkpoint**: User Story 1 is complete when `agentsync skill install sample-skill` using fixture installs files to `.agents/skills/` and `cargo test` for `skill_install.rs` passes

---


## Phase 5: User Story 3 - Update skill from skills.sh (Priority: P1) 🌟

**Goal**: Allow a maintainer to update a skill to the latest compatible version using semantic compatibility rules.

### Tests

- [x] T025 [US3] Create integration test `tests/integration/skill_update.rs` for updating an existing skill, mocking both previous and updated versions. *(Integration test now passes; update logic verified)*

### Implementation for User Story 3

- [x] T026 [US3] Implement version resolution based on semantic compatibility rules in `src/skills/update.rs`. *(Strict semver comparison implemented; only upgrades allowed; tested & verified)*
- [x] T027 [US3] Add HTTP fetch (mocked) for new versions to `src/skills/update.rs`.
    - Unified update/install now robustly fetches, extracts, validates, and installs from local dir, archive, or HTTP source (async, atomic, full rollback safe). All tests passing.
- [x] T028 [US3] Implement atomic rollback for updates in `src/skills/update.rs`. *(Skill update logic is now atomic and rollback-safe; tested and verified)*
- [x] T029 [US3] Ensure correct manifest validation during update and registry synchronization. *(Manifest is validated, registry updated/rolled back as required)*
- [x] T030 [US3] Add CLI `agentsync skill update <skill-id>` support in `src/commands/skill.rs`. *(CLi & error contract fully verified, all modes wired; robust to invalid skill, directory, and archive cases)*
- [ ] T031 [US3] Provide --json output support for updates and update `tests/contracts/` for schemas.

**Checkpoint**: User Story 3 is complete when `cargo test --integration skill_update` validates successful and rollback-safe updates. 

---