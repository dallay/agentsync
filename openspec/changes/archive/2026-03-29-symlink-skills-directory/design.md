# Design: Directory-level skills symlink

## Technical Approach

Change the default sync type for skills targets from `symlink-contents` to `symlink` across all agent init templates and this repo's config. No new code paths, types, or flags — `SyncType::Symlink` already handles directory sources correctly on both Unix and Windows. The change is purely config-level: update TOML strings in `src/init.rs` and `.agents/agentsync.toml`, then update tests to assert a single directory symlink instead of per-entry symlinks.

This maps directly to the proposal's approach (Option 1 from exploration).

## Architecture Decisions

### Decision: Reuse existing `SyncType::Symlink` instead of a new variant

**Choice**: Use the existing `Symlink` sync type for directory sources
**Alternatives considered**: New `SyncType::SymlinkDirectory` variant; `link_directory` boolean flag on `SymlinkContents`
**Rationale**: `create_symlink()` (`src/linker.rs:344-463`) already handles directory sources — on Unix it calls `std::os::unix::fs::symlink` (line 442) which works for directories, and on Windows it dispatches to `std::os::windows::fs::symlink_dir` (line 448) when the source is a directory. Adding a new variant or flag would be pure overhead with no functional benefit. The `process_target()` dispatch (line 204-208) routes `SyncType::Symlink` correctly without any changes.

### Decision: Config-only change, no migration tooling

**Choice**: Change defaults for new projects; existing projects keep `symlink-contents` until manual update
**Alternatives considered**: Auto-migration on sync; deprecation warnings for `symlink-contents`
**Rationale**: `symlink-contents` remains valid for targets that use `pattern` filtering (commands, prompts, agents). Auto-migration would be risky and unnecessary — users can opt in by editing their config or re-running `agentsync init`. Running `agentsync sync --clean` handles the transition cleanly via existing logic.

### Decision: Accept `registry.json` exposure

**Choice**: Allow `registry.json` and other non-skill files in `.agents/skills/` to be visible through the directory symlink
**Alternatives considered**: Adding a `.gitignore`-style filter for directory symlinks; documenting as a blocker
**Rationale**: Agents already read the skills directory contents. `registry.json` is non-sensitive metadata. The simplicity of a single directory symlink outweighs the minor exposure.

## Data Flow

No change to data flow. The existing path through the linker remains identical:

```
agentsync sync
    │
    ▼
Linker::sync() → process_target()
    │
    ├── SyncType::Symlink ──→ create_symlink()
    │   (now used for skills)   │
    │                           ├── dest is symlink? → check target, skip or update
    │                           ├── dest is real dir? → backup to .bak.<timestamp>
    │                           └── create symlink (unix: symlink, windows: symlink_dir)
    │
    └── SyncType::SymlinkContents ──→ create_symlinks_for_contents()
        (still used for commands, prompts, agents)
```

The only difference: skills targets now take the `Symlink` branch instead of `SymlinkContents`.

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/init.rs:73` | Modify | Claude skills target: `type = "symlink-contents"` → `type = "symlink"` |
| `src/init.rs:109` | Modify | Codex skills target: `type = "symlink-contents"` → `type = "symlink"` |
| `src/init.rs:126` | Modify | Gemini skills target: `type = "symlink-contents"` → `type = "symlink"` |
| `src/init.rs:148` | Modify | OpenCode skills target: `type = "symlink-contents"` → `type = "symlink"` |
| `.agents/agentsync.toml:73` | Modify | Repo's OpenCode skills: `type = "symlink-contents"` → `type = "symlink"` |
| `.agents/agentsync.toml:96` | Modify | Repo's Copilot skills: `type = "symlink-contents"` → `type = "symlink"` |
| `src/linker.rs` (tests) | Modify | Add `test_sync_symlink_directory_for_skills` unit test |
| `tests/test_agent_adoption.rs` | Modify | Update 5 test functions: change skills targets from `symlink-contents` to `symlink`, update assertions from per-entry checks to directory symlink checks |

## Interfaces / Contracts

No new interfaces. The only contract change is in the TOML config schema, where skills targets use a different value for an existing field:

```toml
# Before
[agents.claude.targets.skills]
source = "skills"
destination = ".claude/skills"
type = "symlink-contents"

# After
[agents.claude.targets.skills]
source = "skills"
destination = ".claude/skills"
type = "symlink"
```

The `SyncType` enum, `TargetConfig` struct, and all Rust APIs remain unchanged.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | Directory symlink creation for skills source | New test `test_sync_symlink_directory_for_skills` in `src/linker.rs`: create a `.agents/skills/` dir with multiple skill subdirectories, configure `type = "symlink"`, sync, assert dest is a single symlink pointing to source dir (not individual entries inside a real dir) |
| Unit | Clean removes directory symlink | Verify existing `SyncType::Symlink` clean test covers this, or add assertion that cleaning a directory symlink works (line 894-905 already handles it) |
| Integration | Claude adoption with directory symlink | Update `test_adoption_claude_with_skills_and_commands` (line 117-163): change skills target to `"symlink"`, replace `assert_symlink_points_to(root, ".claude/skills/debugging", ...)` with `assert_symlink_points_to(root, ".claude/skills", "skills")` — a single directory symlink |
| Integration | Gemini adoption with directory symlink | Update `test_adoption_gemini_with_skills_and_commands` (line 169-261): same pattern |
| Integration | Codex adoption with directory symlink | Update `test_adoption_codex_skills_only` (line 266-298): same pattern |
| Integration | Multi-agent adoption | Update `test_adoption_multi_agent_shared_skills` (line 303-424): change all three agents' skills targets, update all per-skill assertions to directory-level assertions |
| Integration | Dry-run | Update `test_adoption_dry_run_no_side_effects` (line 430-469): change skills target to `"symlink"` |

### Test assertion pattern change

```rust
// Before: assert individual symlinks inside a real directory
assert_symlink_points_to(root, ".claude/skills/debugging", "debugging");
assert_symlink_points_to(root, ".claude/skills/testing", "testing");

// After: assert the directory itself is a symlink to the source
assert_symlink_points_to(root, ".claude/skills", "skills");
// Then verify contents are accessible through the symlink
assert!(root.join(".claude/skills/debugging").exists());
assert!(root.join(".claude/skills/testing").exists());
```

## Migration / Rollout

### New projects
`agentsync init` emits `type = "symlink"` for skills targets. No action needed.

### Existing projects
- Config still says `symlink-contents` → behavior is unchanged, no breakage
- User updates config to `symlink` → next `agentsync sync` triggers backup logic:
  1. `create_symlink()` sees `.claude/skills/` is a real directory (not a symlink)
  2. Renames to `.claude/skills.bak.<timestamp>` (line 412-423)
  3. Creates single directory symlink `.claude/skills → ../../.agents/skills`
- User runs `agentsync sync --clean` first → cleaner transition:
  1. `clean()` with old `SymlinkContents` config removes per-entry symlinks + empty dir (line 860-892)
  2. User updates config to `symlink`
  3. `sync()` creates the directory symlink fresh

### This repo
Update `.agents/agentsync.toml`, run `pnpm run agents:sync:clean` to transition.

## Open Questions

- [x] Does `create_symlink` handle directory sources? → **Yes**, confirmed at lines 441-452
- [x] Does clean handle `Symlink` type for directories? → **Yes**, `fs::remove_file` on a directory symlink works on both Unix and Windows (line 900)
- [x] Does backup logic handle existing real directories at dest? → **Yes**, `fs::rename` at line 417
- [ ] Should we add a note in CLI `--help` or docs about the `symlink` vs `symlink-contents` distinction for skills? → Non-blocking, can be a follow-up
