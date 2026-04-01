## Exploration: Directory-level skills symlink

### Current State

**Skills sync uses `symlink-contents` today.** When a target has `type = "symlink-contents"`, the
linker (`src/linker.rs:466-516`) iterates through every entry in the source directory and creates an
individual symlink for each one inside the destination directory. For skills, this means:

```
.agents/skills/pinned-tag/  →  .claude/skills/pinned-tag  (symlink)
.agents/skills/rust/        →  .claude/skills/rust         (symlink)
.agents/skills/registry.json →  .claude/skills/registry.json (symlink)
```

The destination directory (`.claude/skills/`) is created as a real directory by
`ensure_directory()` (line 487), then each child gets its own symlink.

**A `symlink` type already exists** (`SyncType::Symlink`) that creates a single symlink from source
to destination. It already handles directory sources — on Unix via `std::os::unix::fs::symlink` (
line 442) and on Windows via `std::os::windows::fs::symlink_dir` (line 448). Changing the type from
`symlink-contents` to `symlink` would produce:

```
.claude/skills  →  ../../.agents/skills  (single directory symlink)
```

**Default configs are defined in two places:**

1. `src/init.rs:70-73` — the default template emitted by `agentsync init`, which sets
   `type = "symlink-contents"` for skills across all agents (claude, codex, gemini, opencode, etc.)
2. Each project's `.agents/agentsync.toml` — the user-facing config

**Clean logic differs by type:**

- `SymlinkContents` clean (`linker.rs:860-893`): iterates entries inside dest dir, removes each
  symlink, then tries to `remove_dir` the now-empty directory
- `Symlink` clean (`linker.rs:894-905`): removes the single symlink at the destination path

### Affected Areas

- `src/init.rs:70-73,106-109,123-126,145-148` — default config template for all agents with skills
  targets; change `type = "symlink-contents"` to `type = "symlink"` for skills targets
- `src/linker.rs:204-215` — `process_target()` dispatch; no code change needed, already routes
  `Symlink` correctly
- `src/linker.rs:344-463` — `create_symlink()`; already handles directory sources (no change needed)
- `src/linker.rs:798-945` — `clean()`; `SyncType::Symlink` branch already handles removal (no change
  needed)
- `src/linker.rs:2027-2102` — unit tests for `symlink-contents` skills; need parallel tests for
  `symlink` directory behavior
- `tests/test_agent_adoption.rs` — integration tests that assert per-skill symlinks; must be updated
  for directory symlink
- `.agents/agentsync.toml:70-73` — this repo's own config; change skills target type
- `src/linker.rs:403-425` — backup logic for existing real directories at destination; already
  handles this via `fs::rename` to `.bak`

### Approaches

1. **Change default to `symlink` for skills targets** — Modify the init template and this repo's
   config to use `type = "symlink"` instead of `type = "symlink-contents"` for skills.
    - Pros: Simplest change; leverages existing `Symlink` code path; single symlink is cleaner,
      fewer filesystem entries, new skills in `.agents/skills/` appear instantly without re-running
      sync
    - Cons: Exposes `registry.json` and any non-skill files in `.agents/skills/` to the agent;
      existing projects using `symlink-contents` need manual config migration; `pattern` filter (
      e.g., `*.md`) no longer applies
    - Effort: Low

2. **New `symlink-directory` type** — Add a dedicated `SyncType` variant that always symlinks the
   source as a directory, with explicit semantics.
    - Pros: Clear intent; doesn't change `symlink` semantics for file sources; could add
      directory-specific validation
    - Cons: Over-engineered; `SyncType::Symlink` already handles directories; adds enum variant,
      serde, tests for no functional gain
    - Effort: Medium

3. **Keep `symlink-contents` but add a `link_directory` flag** — Add an optional boolean to
   `TargetConfig` that makes `symlink-contents` create a directory symlink instead of per-item
   links.
    - Pros: Backward-compatible config shape
    - Cons: Confusing — "symlink-contents" that doesn't symlink contents; adds complexity to an
      already-working code path
    - Effort: Medium

### Recommendation

**Approach 1: Change default to `symlink` for skills targets.** The existing `SyncType::Symlink`
code path already handles directory sources correctly on both Unix and Windows. No new sync type or
flag is needed. The change is:

1. Update init template in `src/init.rs` — change skills targets from `type = "symlink-contents"` to
   `type = "symlink"`
2. Update this repo's `.agents/agentsync.toml` — same change
3. Handle migration: when `clean()` runs on an old `symlink-contents` setup, the per-skill symlinks
   inside `.claude/skills/` get removed and the directory itself gets removed. Then `sync()` with
   the new `symlink` type creates the single directory symlink. The `--clean` flag already handles
   this transition.
4. Update tests

Users with existing configs keep `symlink-contents` until they choose to change — it's purely a
default change for new projects and explicit opt-in for existing ones.

### Risks

- **`registry.json` exposure**: A directory symlink exposes everything in `.agents/skills/`,
  including `registry.json`. This is likely acceptable since agents already see the skills directory
  contents, but worth noting.
- **Existing project migration**: Users who run `agentsync sync` after updating the binary but
  before updating their config will see no change (their config still says `symlink-contents`). Only
  `agentsync init` on new projects or manual config edits are affected. No silent breakage.
- **`pattern` filter becomes irrelevant**: With `symlink` type, the `pattern` field is ignored. Any
  config that relied on `pattern` to filter skills (e.g., `pattern = "*.md"`) would need to use
  `symlink-contents` explicitly. The default configs don't use `pattern` for skills, so this is low
  risk.
- **Windows directory symlinks require elevated privileges**: On Windows, `symlink_dir` may require
  admin/developer mode. This is an existing limitation of `SyncType::Symlink` for directory sources,
  not new to this change. The `create_symlink` method already uses `symlink_dir` for directories.
- **Clean then sync ordering**: If a user has `.claude/skills/` as a real directory with per-skill
  symlinks, running sync with the new `symlink` type will trigger the backup logic (line 403-425)
  which renames the existing directory to `.claude/skills.bak.<timestamp>`. This is safe but users
  should be aware.

### Ready for Proposal

Yes — the recommended approach (change default type from `symlink-contents` to `symlink` for skills
targets) is well-scoped and low-risk. The proposal should cover:

- Init template changes across all agents
- This repo's config update
- Test updates (unit + integration)
- Documentation of the `registry.json` exposure consideration
- Migration guidance for existing users (use `--clean` flag or manually update config)
