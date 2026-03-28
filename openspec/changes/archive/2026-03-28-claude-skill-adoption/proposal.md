# Proposal: Claude Skill Adoption

## Intent

When users run `agentsync init` (plain or wizard), the generated `agentsync.toml` only maps `AGENTS.md → CLAUDE.md` for Claude. There is no `[agents.claude.targets.skills]` entry, so skills placed in `.agents/skills/` are never symlinked to `.claude/skills/`. Additionally, the wizard's `scan_agent_files()` doesn't detect existing `.claude/skills/` directories, meaning users who already have Claude skills get no migration path — they must manually copy files and edit config.

This creates a gap between the documented "single source of truth" promise and the actual out-of-the-box experience for Claude skill users (GitHub #256).

## Scope

### In Scope
- Add `[agents.claude.targets.skills]` to `DEFAULT_CONFIG` template (`skills` → `.claude/skills`, `symlink-contents`)
- Add `AgentFileType::ClaudeSkills` variant to `scan_agent_files()` for `.claude/skills/` detection
- Extend wizard migration to copy discovered `.claude/skills/*` into `.agents/skills/` and append the skills target to generated config
- Add a lightweight diagnostic in `apply` (or `status`) that warns when `.claude/skills/` exists with content but no target manages it
- Unit and integration tests for all new behavior

### Out of Scope
- `.claude/commands/` detection and migration (follow-up change)
- Dynamic config generation / config builder replacing the static template
- Changes to the `skill install` registry system (`registry.json`)
- Skills from other agents (Codex already has a skills target in the template)

## Approach

Combine Approach 1 (detect-and-adopt in wizard) with Approach 2 (template update) from the exploration:

1. **Update `DEFAULT_CONFIG`** — Add a `[agents.claude.targets.skills]` entry with `source = "skills"`, `destination = ".claude/skills"`, `type = "symlink-contents"`. This is zero-risk and immediately benefits new projects.

2. **Extend `AgentFileType` enum** — Add `ClaudeSkills` variant. Update `scan_agent_files()` to check for `.claude/skills/` as a directory. Categorize it alongside other directory-type agent files (like `CursorDirectory`, `WindsurfDirectory`).

3. **Wizard migration for skills** — When the wizard discovers `.claude/skills/` and the user selects it for migration:
   - Copy each skill subdirectory from `.claude/skills/*` into `.agents/skills/`, warning on name collisions
   - The updated `DEFAULT_CONFIG` template already includes the skills target, so no config append logic is needed for new inits
   - For re-inits where config already exists, skip config modification (the `!force` guard already handles this)

4. **Apply-time diagnostic** — In the `apply` flow, after processing targets, check if `.claude/skills/` exists and contains files but no enabled target maps to it. Print a warning: `⚠ .claude/skills/ has content but is not managed by any target. Run 'agentsync init --wizard' to adopt.`

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/init.rs` — `DEFAULT_CONFIG` | Modified | Add `[agents.claude.targets.skills]` section |
| `src/init.rs` — `AgentFileType` enum | Modified | Add `ClaudeSkills` variant |
| `src/init.rs` — `scan_agent_files()` | Modified | Detect `.claude/skills/` directory |
| `src/init.rs` — wizard migration logic | Modified | Handle directory-type skill migration with collision detection |
| `src/linker.rs` or `src/commands/apply.rs` | Modified | Add unmanaged-skills diagnostic warning |
| `src/agent_ids.rs` | None | Already has `.claude/skills/` in gitignore patterns (line 97) — no changes needed |
| `tests/` | New | Tests for scan detection, migration, config template, diagnostic |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Skill name collisions during migration (`.claude/skills/foo` vs existing `.agents/skills/foo`) | Medium | Warn user during wizard and skip conflicting skills with a message |
| Existing users re-running `init` get unexpected config changes | Low | The `!force` guard prevents overwriting existing `agentsync.toml`; template change only affects fresh inits |
| Apply diagnostic false positive if user intentionally has unmanaged `.claude/skills/` | Low | Make the warning informational (not an error); suppress if a target already covers `.claude/skills` |
| `DEFAULT_CONFIG` template tests break due to new section | Low | Update existing template-parsing tests to expect the new skills target |

## Rollback Plan

All changes are additive and backward-compatible:

1. **Template change**: Remove the `[agents.claude.targets.skills]` block from `DEFAULT_CONFIG`. Existing configs are not modified by this change, so users who already generated configs keep them as-is.
2. **Enum/scan change**: Remove the `ClaudeSkills` variant and its scan entry. The wizard simply stops detecting `.claude/skills/`.
3. **Diagnostic**: Remove the warning check from the apply path.

No data migration or schema changes are involved. A single revert commit undoes everything.

## Dependencies

- None. All changes are within the existing codebase with no new external crates or dependencies.

## Success Criteria

- [ ] `agentsync init` on a fresh project generates config with `[agents.claude.targets.skills]`
- [ ] `agentsync apply` on a fresh project symlinks `.agents/skills/*` → `.claude/skills/*`
- [ ] `agentsync init --wizard` in a repo with `.claude/skills/` detects and offers to migrate those skills
- [ ] Wizard migration copies skills into `.agents/skills/` with collision warnings
- [ ] `agentsync apply` warns when `.claude/skills/` has unmanaged content
- [ ] All existing tests pass without modification (no regressions)
- [ ] New tests cover: template parsing, skill detection, migration flow, diagnostic warning
