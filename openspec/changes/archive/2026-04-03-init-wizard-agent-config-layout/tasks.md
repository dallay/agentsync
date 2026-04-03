# Tasks: Init Wizard Agent Config Layout

## Phase 1: Layout Facts and Rendering Foundation

- [x] 1.1 Add private layout data structures and config-derived extraction helpers in `src/init.rs`
  that inspect the final `build_default_config_with_skills_modes()` output and collect only
  wizard-owned instructions, skills, and commands targets.
- [x] 1.2 Add a renderer in `src/init.rs` for the managed `## Agent config layout` block, including
  stable HTML comment markers, canonical `.agents/` wording, listed destinations,
  `.opencode/command`, and sync-type-specific language.
- [x] 1.3 Add insertion/upsert helpers in `src/init.rs` that remove an existing managed block, find
  the post-header insertion point, and normalize spacing for both default-template and
  migrated-content AGENTS bodies.

## Phase 2: Wizard Write-Path Integration

- [x] 2.1 Update the wizard AGENTS generation path in `src/init.rs` so migrated content and
  `DEFAULT_AGENTS_MD` both pass through the managed-block upsert step before any write occurs.
- [x] 2.2 Keep existing overwrite semantics in `src/init.rs`: if `.agents/AGENTS.md` exists and
  `force` is false, preserve it byte-for-byte and skip any partial block injection or replacement.
- [x] 2.3 Ensure the layout block is generated from the same final rendered config the wizard
  writes, so omitted targets stay omitted and selected skills modes drive the explainer text.

## Phase 3: Focused Unit Test Coverage

- [x] 3.1 Extend `src/init.rs` tests for layout-facts extraction from rendered config, covering
  default destinations plus a mixed skills-mode fixture (`symlink` and `symlink-contents`).
- [x] 3.2 Add `src/init.rs` rendering tests that assert exactly one marker pair, the
  `## Agent config layout` heading, canonical-source wording, expected default destinations,
  `.opencode/command`, and apply-required wording for `symlink-contents` skills targets.
- [x] 3.3 Add `src/init.rs` insertion tests using two fixtures: the default `DEFAULT_AGENTS_MD`
  shape and migrated content starting with `# Instructions from ...`; assert the block lands after
  the opening title/introduction and before later body content.
- [x] 3.4 Add `src/init.rs` idempotency/preservation tests covering forced rerun replacement with no
  duplicate block and non-force behavior where an existing `.agents/AGENTS.md` remains unchanged
  byte-for-byte.

## Phase 4: Validation and Regression Checks

- [x] 4.1 Run targeted Rust tests for `src/init.rs` wizard coverage (including the new placement,
  mode-language, idempotency, and preserve-without-force scenarios) and fix any failures.
- [x] 4.2 Run a broader regression command for init behavior (`cargo test` subset for
  init/agent-adoption paths, plus `cargo fmt --all -- --check` if code changed) to confirm the
  wizard-only change does not affect deferred `apply` ownership.
