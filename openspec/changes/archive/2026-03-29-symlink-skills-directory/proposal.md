# Proposal: Directory-level skills symlink

## Intent

Skills sync currently uses `symlink-contents`, creating individual symlinks per skill entry inside a
real destination directory. This means adding a new skill in `.agents/skills/` requires re-running
`agentsync sync` before the agent sees it. Switching to a single directory symlink (
`type = "symlink"`) makes new skills appear instantly, eliminates stale-entry drift, and simplifies
the filesystem layout — all using code paths that already exist.

**Linear issue**: DALLAY-197

## Scope

### In Scope

- Update the `agentsync init` default template to emit `type = "symlink"` for all skills targets (
  claude, codex, gemini, opencode)
- Update this repo's `.agents/agentsync.toml` skills targets (opencode, copilot) from
  `symlink-contents` to `symlink`
- Update unit tests in `src/linker.rs` that assert `symlink-contents` behavior for skills
- Update integration tests in `tests/test_agent_adoption.rs` that assert per-skill symlinks

### Out of Scope

- Changing `symlink-contents` for `commands`/`prompts`/`agents` targets (those benefit from pattern
  filtering)
- Deprecating or removing `symlink-contents` type (still valid for filtered content)
- Auto-migration of existing user configs (users opt in by editing their config or re-running
  `init`)
- Windows-specific symlink privilege handling (pre-existing limitation, unchanged)

## Approach

Change the default sync type from `symlink-contents` to `symlink` for skills targets only. No new
code paths, types, or flags needed — `SyncType::Symlink` already handles directory sources correctly
on both Unix (`std::os::unix::fs::symlink`) and Windows (`std::os::windows::fs::symlink_dir`).

**Changes:**

1. **`src/init.rs`** — In the default config template, change `type = "symlink-contents"` to
   `type = "symlink"` for skills targets across all agents (lines 70-73, 106-109, 123-126, 145-148)
2. **`.agents/agentsync.toml`** — Change skills targets for opencode (line 70-73) and copilot (line
   93-96) from `symlink-contents` to `symlink`
3. **Tests** — Update assertions that expect per-entry symlinks inside a real skills directory to
   instead expect a single directory symlink

**Migration path for existing users:** Running `agentsync sync --clean` removes the old per-entry
symlinks and the real directory, then creates the single directory symlink. Users who don't update
their config continue using `symlink-contents` with no change in behavior.

## Affected Areas

| Area                           | Impact   | Description                                                   |
|--------------------------------|----------|---------------------------------------------------------------|
| `src/init.rs:70-73`            | Modified | Claude skills target: `symlink-contents` → `symlink`          |
| `src/init.rs:106-109`          | Modified | Codex skills target: `symlink-contents` → `symlink`           |
| `src/init.rs:123-126`          | Modified | Gemini skills target: `symlink-contents` → `symlink`          |
| `src/init.rs:145-148`          | Modified | OpenCode skills target: `symlink-contents` → `symlink`        |
| `.agents/agentsync.toml:70-73` | Modified | Repo's OpenCode skills target: `symlink-contents` → `symlink` |
| `.agents/agentsync.toml:93-96` | Modified | Repo's Copilot skills target: `symlink-contents` → `symlink`  |
| `src/linker.rs` (unit tests)   | Modified | Update/add tests for directory symlink on skills              |
| `tests/test_agent_adoption.rs` | Modified | Update integration assertions for directory symlink           |

## Risks

| Risk                                               | Likelihood | Mitigation                                                                             |
|----------------------------------------------------|------------|----------------------------------------------------------------------------------------|
| `registry.json` exposed via directory symlink      | Low        | Agents already read skills dir contents; `registry.json` is non-sensitive metadata     |
| `pattern` filter ignored with `symlink` type       | Low        | Default configs don't use `pattern` on skills targets; documented in Out of Scope      |
| Existing users see no change without config update | Low        | Intentional — no silent migration; users opt in via config edit or `init`              |
| Windows `symlink_dir` requires elevated privileges | Low        | Pre-existing limitation of `SyncType::Symlink`; unchanged by this proposal             |
| `--clean` renames existing real dir to `.bak`      | Low        | Backup logic already handles this safely; timestamp-suffixed backup prevents data loss |

## Rollback Plan

1. Revert the config template changes in `src/init.rs` (restore `type = "symlink-contents"` for
   skills targets)
2. Revert `.agents/agentsync.toml` skills targets back to `symlink-contents`
3. Run `agentsync sync --clean` to remove directory symlinks and recreate per-entry symlinks
4. Revert test changes

No data loss risk — the rollback is a config-only revert followed by a clean sync.

## Dependencies

- None. All required code paths (`SyncType::Symlink` for directories, backup logic, clean logic)
  already exist.

## Success Criteria

- [ ] Creating a skill dir in `.agents/skills/` makes it visible in `.claude/skills/` (and other
  agent dirs) without re-running sync
- [ ] Renaming or deleting a skill dir in `.agents/skills/` is immediately reflected in agent dirs
  without re-running sync
- [ ] `agentsync sync --clean` correctly transitions from old `symlink-contents` layout to new
  `symlink` layout
- [ ] `agentsync sync --dry-run` clearly reports the symlink strategy being used
- [ ] All existing tests pass after updating assertions
- [ ] `agentsync init` on a fresh project produces `type = "symlink"` for skills targets
