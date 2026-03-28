# Tasks: Directory-level skills symlink

## Phase 1: Config Changes

- [x] 1.1 In `src/init.rs`, change `type = "symlink-contents"` to `type = "symlink"` for all four skills targets: claude (line 73), codex (line 109), gemini (line 126), opencode (line 148). Leave `commands` targets unchanged. Verify: `cargo test --lib test_sync_creates_symlink` still passes; grep `init.rs` for `symlink-contents` — only `commands` targets should remain.

- [x] 1.2 In `.agents/agentsync.toml`, change `type = "symlink-contents"` to `type = "symlink"` for opencode skills (line 73) and copilot skills (line 96). Leave `commands`, `agents`, and `prompts` targets unchanged. Verify: `pnpm run agents:sync:clean` runs without error.

## Phase 2: Unit Tests

- [x] 2.1 Add `test_sync_symlink_directory_for_skills` in `src/linker.rs` (after existing `test_sync_symlink_contents` at line 2062). Create `.agents/skills/` with two skill subdirectories, configure `type = "symlink"`, sync, assert: destination is a symlink (not a real dir), destination resolves to source, skill subdirectories are accessible through the symlink. Follow the pattern from `test_sync_creates_symlink` (line 1467). Verify: `cargo test --lib test_sync_symlink_directory_for_skills`.

## Phase 3: Integration Tests

- [x] 3.1 In `tests/test_agent_adoption.rs`, update `test_adoption_claude_with_skills_and_commands` (line 117): change skills config from `"symlink-contents"` to `"symlink"`. Replace per-skill assertions (`assert_symlink_points_to` for `.claude/skills/debugging` and `.claude/skills/testing`) with: assert `.claude/skills` itself is a symlink to `skills`, then assert `.claude/skills/debugging/SKILL.md` and `.claude/skills/testing/SKILL.md` exist. Keep commands assertions unchanged.

- [x] 3.2 Update `test_adoption_gemini_with_skills_and_commands` (line 169): same pattern — change skills config to `"symlink"`, assert `.gemini/skills` is a directory symlink, verify `.gemini/skills/code-review/SKILL.md` accessible. Keep commands assertion unchanged.

- [x] 3.3 Update `test_adoption_codex_with_skills` (line 244): change skills config to `"symlink"`, assert `.codex/skills` is a directory symlink, verify `.codex/skills/linting/SKILL.md` accessible.

- [x] 3.4 Update `test_adoption_multi_agent_claude_gemini_codex` (line 299): change all three agents' skills configs to `"symlink"`. Replace all per-skill `assert_symlink_points_to` calls (lines 408-422) with directory-level assertions: `.claude/skills`, `.gemini/skills`, `.codex/skills` are each symlinks to `skills`. Then assert individual skill dirs exist through the symlinks.

- [x] 3.5 Update `test_adoption_dry_run_no_side_effects` (line 430): change skills config to `"symlink"`. Update the non-existence check from `.claude/skills/my-skill` to `.claude/skills` (the directory symlink itself should not exist after dry-run).

## Phase 4: Verification

- [x] 4.1 Run `cargo test --test test_agent_adoption` to verify all integration tests pass. Then run `cargo test --lib test_sync_symlink` to verify all symlink unit tests pass. Finally run `cargo clippy --all-targets --all-features -- -D warnings` for lint.
