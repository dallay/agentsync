# Tasks: Claude Skill Adoption

## Phase 1: Infrastructure / Foundation

- [x] 1.1 Add `ClaudeSkills` variant to `AgentFileType` enum in `src/init.rs`. Include it in any
  exhaustive match arms (display, backup eligibility). *(Design §Interfaces — New AgentFileType
  Variant; Spec §Scan Detects Claude Skills Directory)*
- [x] 1.2 Add `[agents.claude.targets.skills]` block to `DEFAULT_CONFIG` in `src/init.rs` —
  `source = "skills"`, `destination = ".claude/skills"`, `type = "symlink-contents"` — after the
  existing `[agents.claude.targets.instructions]` entry. *(Design §Init Flow; Spec §Default Config
  Includes Claude Skills Target)*

## Phase 2: Core Implementation

- [x] 2.1 Extend `scan_agent_files()` in `src/init.rs` to detect `.claude/skills/` — check exists,
  is_dir, and has at least one child entry. Push `DiscoveredFile` with `file_type: ClaudeSkills` and
  a display name like "Claude Code skills (.claude/skills/)". *(Design §Wizard Detection Flow; Spec
  scenarios: finds with content, ignores empty, ignores absent, detects alongside CLAUDE.md)*
- [x] 2.2 Add `AgentFileType::ClaudeSkills` match arm in the wizard migration loop in `src/init.rs`.
  For each subdirectory in `.claude/skills/`: if dest exists in `.agents/skills/`, skip + warn; else
  `copy_dir_all` + print success. Track `files_actually_migrated` / `files_skipped`. *(Design
  §Wizard Migration Match Arm; Spec §Wizard Migrates Claude Skills — all 6 scenarios)*
- [x] 2.3 Add `check_unmanaged_claude_skills()` function in `src/commands/doctor.rs`. Check
  `.claude/skills/` exists, is non-empty, and no enabled target has
  `destination = ".claude/skills"`. Print warning with suggestion to run `init --wizard`. Wire into
  `run_doctor()` after existing checks. *(Design §Doctor Diagnostic Flow; Spec §Apply-Time
  Diagnostic — placed in doctor per design decision)*

## Phase 3: Testing

- [x] 3.1 Add unit test in `src/init.rs` tests: parse `DEFAULT_CONFIG`, assert `claude` agent has
  `skills` target with correct source/destination/sync_type. *(Spec scenarios: Fresh init config
  parseable, Fresh init generates config)*
- [x] 3.2 Add unit tests for `scan_agent_files()`: (a) detects `.claude/skills/` with content, (b)
  ignores empty dir, (c) ignores absent dir, (d) detects alongside `CLAUDE.md`. Use
  `tempfile::TempDir`. *(Spec §Scan scenarios)*
- [x] 3.3 Add unit tests for wizard skill migration: (a) copies skills into `.agents/skills/`, (b)
  skips collisions with warning, (c) handles mixed content (subdirs + loose files). *(Spec §Wizard
  migration scenarios)*
- [x] 3.4 Add tests in `src/commands/doctor_tests.rs` (or equivalent): (a) warns when
  `.claude/skills/` has content and no target manages it, (b) suppresses warning when target
  exists, (c) no warning when dir absent/empty. *(Spec §Diagnostic scenarios)*
- [x] 3.5 Run `cargo test --all-features` to verify no regressions in existing tests. *(Spec
  §Acceptance Criteria #5)*

## Phase 4: Cleanup

- [x] 4.1 Run `cargo fmt --all` and `cargo clippy --all-targets --all-features -- -D warnings` to
  ensure code passes CI checks.
