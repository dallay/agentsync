# Tasks: Autodetect Skill Suggestions

## Phase 1: Read-only suggest foundation

- [x] 1.1 Update `src/skills/mod.rs` and create `src/skills/detect.rs` with deterministic marker scanning, ignored-directory pruning, evidence records, and confidence scoring for Rust, Node/TypeScript, Astro, GitHub Actions, Docker, Make, and Python.
- [x] 1.2 Create `src/skills/catalog.rs` with the embedded v1 catalog, recommendation rules, deterministic deduplication/order, and a catalog trait that keeps future provider-backed metadata separate from install logic.
- [x] 1.3 Extend `src/skills/registry.rs` and `src/skills/provider.rs` with read-only installed-state helpers and the minimal metadata seam needed by the catalog boundary, without changing `registry.json` schema or current install behavior.
- [x] 1.4 Create `src/skills/suggest.rs` to turn detections into recommendation results, merge reasons/matched technologies, annotate installed state, compute summary counts, and guarantee read-only behavior when no install flag is used.

## Phase 2: Suggest CLI and recommendation-driven install

- [x] 2.1 Update `src/commands/skill.rs` and `src/main.rs` to add `agentsync skill suggest` install modes with `--json`, `--install`, and `--install --all`, including non-TTY validation for guided installs.
- [x] 2.2 Implement human and JSON output in `src/commands/skill.rs`/`src/skills/suggest.rs` to keep the read-only contract intact and extend install-mode responses with selection/results metadata plus structured error payloads.
- [x] 2.3 Add recommendation-driven install planning in `src/skills/suggest.rs` that skips already-installed skills, validates selected recommendation ids, and delegates each new install to existing `install::blocking_fetch_and_install_skill`.
- [x] 2.4 Reuse `dialoguer` patterns from `src/init.rs` for `--install`, keeping prompt wiring in `src/commands/skill.rs` thin and the selection/install plan logic in `src/skills/suggest.rs` testable without a TTY.

## Phase 3: Tests and regression coverage

- [x] 3.1 Create `tests/unit/suggest_detector.rs`, `tests/unit/suggest_catalog.rs`, and `tests/unit/mod.rs`, then update `tests/all_tests.rs` so marker detection, ignored paths, deduplication, installed-state annotation, and empty-result behavior run under `cargo test --test all_tests`.
- [x] 3.2 Create `tests/integration/skill_suggest.rs` and update `tests/integration/mod.rs` to cover read-only suggest output, no filesystem mutation, installed-skill skipping, invalid non-TTY guided usage, and `--install --all` delegation through existing lifecycle behavior.
- [x] 3.3 Create `tests/contracts/test_skill_suggest_output.rs` for JSON success/error contracts, including empty arrays, summary counts, stable enums, install-mode response fields, and non-TTY remediation guidance.

## Phase 4: Documentation and final validation

- [x] 4.1 Update `website/docs/src/content/docs/guides/skills.mdx` and `website/docs/src/content/docs/reference/cli.mdx` with `skill suggest` usage, phase-1 read-only semantics, phase-2 install modes, and JSON-output examples.
- [x] 4.2 Validate incrementally with `cargo test --test all_tests`, `cargo test --test test_skill_suggest_output`, `cargo clippy --all-targets --all-features -- -D warnings`, and `pnpm run docs:build`; fix any contract/help-text drift before handoff.
