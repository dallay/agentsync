## Exploration: claude-skill-adoption

### Current State

#### Init Flow (`src/init.rs`)

The `init` command has two modes:

1. **`init()` (plain)** — Creates `.agents/` directory, `.agents/skills/`,
   `.agents/agentsync.toml` (from `DEFAULT_CONFIG` template), and `.agents/AGENTS.md`. No scanning
   or detection of existing files.

2. **`init_wizard()` (--wizard)** — Calls `scan_agent_files()` which scans for 30+ agent file
   types (CLAUDE.md, .cursor/, .mcp.json, .github/copilot-instructions.md, etc.). It then offers
   interactive migration:
    - **Instruction files** (CLAUDE.md, copilot-instructions.md, etc.) → merged into
      `.agents/AGENTS.md`
    - **Directories** (.cursor/, .windsurf/, etc.) → copied as-is into `.agents/`
    - **MCP configs** (.mcp.json) → noted but not migrated
    - Optionally backs up originals to `.agents/backup/`

**Gap: The wizard does NOT scan for `.claude/skills/` or `.claude/commands/`.** The `AgentFileType`
enum has no variants for Claude skills or commands. The `scan_agent_files()` function checks for
`CLAUDE.md` (instructions) but ignores the `.claude/` subdirectory structure entirely.

#### Apply Flow (`src/linker.rs`, `src/main.rs`)

The `apply` command:

1. Finds `agentsync.toml` (searches `.agents/agentsync.toml` then root `agentsync.toml`)
2. Loads config, creates `Linker`
3. Iterates enabled agents → processes each target by `SyncType`:
    - `Symlink` — single file symlink (e.g., AGENTS.md → CLAUDE.md)
    - `SymlinkContents` — symlinks each item inside a source directory to destination
    - `NestedGlob` — walks a directory tree matching a glob pattern
    - `ModuleMap` — maps source files to specific module directories
4. Updates `.gitignore`
5. Syncs MCP configurations

**Gap: The default config template (`DEFAULT_CONFIG`) has NO Claude skills target.** It only
defines:

- `agents.claude.targets.instructions` → AGENTS.md → CLAUDE.md (symlink)
- `agents.codex.targets.skills` → skills → .codex/skills (symlink-contents)

There is no `agents.claude.targets.skills` entry. Users must manually add something like:

```toml
[agents.claude.targets.skills]
source = "skills"
destination = ".claude/skills"
type = "symlink-contents"
```

#### Skills System (`src/skills/`, `src/commands/skill.rs`)

The skills subsystem (`skill install/update/uninstall`) is about installing skills **from external
sources** (skills.sh, GitHub URLs, zip archives) into `.agents/skills/`. It has:

- A `registry.json` for tracking installed skills
- Manifest validation (SKILL.md frontmatter parsing)
- Transaction-safe install with backup/rollback

**This system is completely separate from the init/apply symlink pipeline.** It writes to
`.agents/skills/<skill-id>/` but doesn't create any config entries or symlink targets. The apply
step must already have a `symlink-contents` target configured to pick them up.

#### Dry-run Diagnostics

The `--dry-run` flag on `apply` shows what symlinks would be created/updated. For `SymlinkContents`,
if the source directory doesn't exist, it prints:

```text
! Source directory does not exist: <path>
```

and increments `skipped`. There are no warnings about "you have skills in .claude/skills/ that
aren't being managed" or "your skills source is empty."

#### Config Schema (`src/config.rs`)

The config supports any number of agents and targets. Skills are just another target with
`type = "symlink-contents"`. There's no special "skills" concept in the config — it's all generic
source→destination mapping.

### Affected Areas

- `src/init.rs` — `scan_agent_files()` needs new variants for Claude skills/commands;
  `AgentFileType` enum; migration logic for directory-type skill files
- `src/init.rs` — `DEFAULT_CONFIG` template needs Claude skills target
- `src/init.rs` — `init_wizard()` needs to generate config entries for discovered skill directories
- `src/linker.rs` — Potentially add diagnostic warnings when skill sources are empty
- `src/commands/status.rs` — Could report "unmanaged Claude skills detected" diagnostics
- `src/agent_ids.rs` — Already has `.claude/skills/` in gitignore patterns (line 97), confirming
  awareness of the path

### Approaches

1. **Detect-and-adopt in wizard** — Extend `scan_agent_files()` to detect `.claude/skills/` and
   `.claude/commands/`, copy them into `.agents/skills/` during migration, and generate
   `agentsync.toml` with a `[agents.claude.targets.skills]` entry.
    - Pros: Fully automated one-shot migration; fits existing wizard architecture; user sees skills
      in multi-select
    - Cons: Medium complexity; needs config generation (currently uses static `DEFAULT_CONFIG`
      template); must handle skill name collisions if skills from multiple agents overlap
    - Effort: Medium

2. **Template-only fix** — Add `[agents.claude.targets.skills]` to `DEFAULT_CONFIG` so new inits
   automatically get the skills target. For existing repos, document manual config addition.
    - Pros: Very simple; backward compatible; low risk
    - Cons: Doesn't solve the adoption problem for existing repos; no auto-detection; users still
      must manually copy skills
    - Effort: Low

3. **Dynamic config generation in wizard** — Instead of writing the static `DEFAULT_CONFIG`
   template, have the wizard build `agentsync.toml` dynamically based on what it discovers. For each
   detected agent type, generate the appropriate targets including skills.
    - Pros: Most comprehensive; config exactly matches the project's actual setup; can handle any
      combination of agents
    - Cons: Higher complexity; need to maintain a builder/generator alongside the static template;
      more test surface
    - Effort: High

### Recommendation

**Approach 1 (Detect-and-adopt in wizard)** is the right balance. Specifically:

1. **Add `AgentFileType::ClaudeSkills` and `AgentFileType::ClaudeCommands`** variants to the enum.
2. **Extend `scan_agent_files()`** to detect `.claude/skills/` and `.claude/commands/` directories.
3. **In the wizard migration logic**, when Claude skills are selected for migration:
    - Copy `.claude/skills/*` into `.agents/skills/` (merging with any existing skills there)
    - Copy `.claude/commands/*` into `.agents/commands/` (or a designated location)
4. **Update `DEFAULT_CONFIG`** to include `[agents.claude.targets.skills]` pointing `skills` →
   `.claude/skills` as `symlink-contents`. This benefits both new inits and wizard migrations.
5. **Add a diagnostic** in the apply flow (or `status`/`doctor`) that warns when `.claude/skills/`
   exists with content but isn't managed by any target.

Additionally, as a quick win, update `DEFAULT_CONFIG` (Approach 2) regardless — it costs nothing and
helps new projects immediately.

### Risks

- **Skill name collisions**: If `.claude/skills/foo/` and `.codex/skills/foo/` both exist, copying
  both into `.agents/skills/` would conflict. Mitigation: warn the user during wizard migration and
  let them choose.
- **Commands migration scope**: `.claude/commands/` is a different concept than skills. The issue
  mentions skills but commands may also need adoption. Need to clarify scope — the current issue (
  #256) focuses on skills, so commands can be a follow-up.
- **Breaking existing workflows**: Users who already have a working `.agents/skills/` directory and
  a custom `agentsync.toml` shouldn't have their setup disrupted by a re-init. The existing `!force`
  guard protects against this.
- **Wizard-only path**: The non-wizard `init` won't auto-detect. This is by design (non-wizard is
  for fresh projects), but should be documented clearly.
- **Registry.json sync**: Skills copied from `.claude/skills/` won't have `registry.json` entries
  since they weren't installed via `skill install`. The skill system and the symlink system are
  orthogonal — copied skills just need SKILL.md validation, not registry entries. But this should be
  explicitly documented.

### Ready for Proposal

Yes — the exploration has identified clear gaps and a recommended approach. The orchestrator should
proceed to the proposal phase with the "detect-and-adopt in wizard + update default template"
direction. Key decisions for the proposal:

1. Whether to include `.claude/commands/` in scope or just skills
2. Whether to add doctor/status warnings for unmanaged skill directories
3. Whether the config should be generated dynamically or remain a template with optional sections
   appended
