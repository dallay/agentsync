# Proposal: Universal Agent Adoption

## Intent

AgentSync's init wizard currently treats Claude as a first-class citizen for skills/commands migration while ignoring equivalent artifacts from ~15 other agents. Users with Cursor skills, Gemini commands, OpenCode skills, or agent-specific MCP configs get an incomplete adoption experience — those artifacts are either silently lost or left unmanaged. Additionally, several instruction files (`.windsurfrules`, `OPENCODE.md`, `AMPCODE.md`) are declared in the enum but never scanned, and the static `DEFAULT_CONFIG` doesn't reflect which agents were actually discovered.

This change (Phase 1) closes the detection and migration gaps so that `agentsync init --wizard` produces a comprehensive adoption regardless of which agents the project uses.

## Scope

### In Scope

1. **Expand `scan_agent_files()` detection** — Add new `AgentFileType` variants and scan entries for all known skill directories (~8), command directories (3), and agent-specific MCP config files (~10).
2. **Skills normalization** — All discovered agent skill directories (`.cursor/skills/`, `.codex/skills/`, `.gemini/skills/`, `.opencode/skills/`, `.roo/skills/`, `.factory/skills/`, `.vibe/skills/`, `.agent/skills/`) are migrated into `.agents/skills/` using the same copy-contents pattern as `ClaudeSkills`. Collisions warn and skip.
3. **Commands migration** — Create `.agents/commands/` as the canonical location. Copy command files from `.claude/commands/`, `.gemini/commands/`, `.opencode/command/` into it during wizard migration.
4. **Fix instruction file scan gaps** — Wire up detection for `.windsurfrules`, `OPENCODE.md`, `AMPCODE.md`, and any other files that `agent_convention_filename()` returns but `scan_agent_files()` doesn't check.
5. **MCP config detection** — Add scan entries for agent-specific MCP configs (`.cursor/mcp.json`, `.windsurf/mcp_config.json`, `.codex/config.toml`, `.roo/mcp.json`, `.kiro/settings/mcp.json`, `.amazonq/mcp.json`, `.kilocode/mcp.json`, `.factory/mcp.json`, `opencode.json`). Migration action: note with a message suggesting manual migration to `[mcp_servers]`.
6. **Update `DEFAULT_CONFIG`** — Add a `[agents.claude.targets.commands]` entry. Add placeholder sections for `gemini` and `opencode` agents (instructions + skills targets).

### Out of Scope

- **Dynamic config generation** — `DEFAULT_CONFIG` remains a static template; building config from discovered agents is Phase 2.
- **MCP config parsing/import** — Phase 1 only detects and notes; no parsing of JSON/TOML MCP configs into `[mcp_servers]`.
- **Agent-specific skill format conversion** — Skills are copied as-is; no transformation between formats.
- **Cursor rules parsing** — `.cursor/rules/*.mdc` files stay within the `.cursor/` directory copy; no individual extraction.

## Approach

### 1. New `AgentFileType` variants

Add variants to the existing enum in `src/init.rs`:

- **Skills**: `CursorSkills`, `CodexSkills`, `GeminiSkills`, `OpenCodeSkills`, `RooSkills`, `FactorySkills`, `VibeSkills`, `AntigravitySkills`
- **Commands**: `ClaudeCommands`, `GeminiCommands`, `OpenCodeCommands`
- **MCP configs**: `CursorMcpConfig`, `WindsurfMcpConfig`, `CodexConfig`, `RooMcpConfig`, `KiroMcpConfig`, `AmazonQMcpConfig`, `KilocodeMcpConfig`, `FactoryMcpConfig`, `OpenCodeConfig`
- **Instructions**: `OpenCodeInstructions`, (wire existing `WindsurfRules`, `AmpInstructions` to actual scan code)

### 2. Expand `scan_agent_files()`

For each new variant, add a directory/file existence check following the existing pattern (check path exists, check non-empty for dirs). Group the additions logically: skills block, commands block, MCP block, instruction fixes.

### 3. Extend migration match arms

In the wizard's migration logic, add match arms:
- All `*Skills` variants → copy contents into `.agents/skills/` (reuse `ClaudeSkills` migration pattern with collision detection)
- All `*Commands` variants → copy contents into `.agents/commands/` (new directory, same copy pattern)
- All `*McpConfig` variants → note only, print message suggesting `[mcp_servers]` in `agentsync.toml`
- New instruction variants → merge into `AGENTS.md` (existing pattern)

### 4. Create `.agents/commands/` in init

Add `commands` directory creation alongside the existing `skills` directory creation in `init()`.

### 5. Update `DEFAULT_CONFIG`

Add after `[agents.claude.targets.skills]`:
```toml
[agents.claude.targets.commands]
source = "commands"
destination = ".claude/commands"
type = "symlink-contents"
```

Add new agent sections for `gemini` and `opencode` with instructions and skills targets.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/init.rs` — `AgentFileType` enum | Modified | ~20 new variants for skills, commands, MCP configs |
| `src/init.rs` — `scan_agent_files()` | Modified | ~25 new scan entries with existence checks |
| `src/init.rs` — wizard migration match arms | Modified | New match arms for skills/commands/MCP migration |
| `src/init.rs` — `init()` | Modified | Create `.agents/commands/` directory |
| `src/init.rs` — `DEFAULT_CONFIG` | Modified | Add commands target for Claude; add gemini, opencode agent sections |
| `src/init.rs` — tests | New | ~25 new `test_scan_agent_files_finds_*` tests |
| `openspec/specs/skill-adoption/spec.md` | Affected | Existing spec covers Claude skills only; this change generalizes the pattern. No conflict — we extend, not break. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Skills extracted from dirs that are also copied as-is (e.g., `.cursor/skills/` inside `.cursor/` copy) causes duplication | Medium | Extract skills BEFORE parent directory copy; or exclude `skills/` subdirectory during parent dir copy |
| Commands format divergence across agents loses agent-specific metadata | Low | Copy as-is without transformation; `symlink-contents` distributes back faithfully |
| Large number of new scan entries inflates `scan_agent_files()` complexity | Medium | Group checks into helper functions by category (skills, commands, MCP); keep each check simple |
| `.windsurfrules` scan fix may change wizard behavior for existing users | Low | Additive — file is now detected where it was previously missed; no existing behavior removed |
| `DEFAULT_CONFIG` changes break TOML parsing | Low | Existing `test_default_config_is_valid_toml` test catches this; extend for new targets |

## Rollback Plan

All changes are additive to `src/init.rs` (new enum variants, new scan entries, new match arms). To roll back:

1. Revert the commit — no migration path or data format changes to undo.
2. Users who already ran the wizard with the new code will have `.agents/skills/` and `.agents/commands/` populated with extra files, but these are inert copies — they don't break anything if the new config targets are removed.
3. `DEFAULT_CONFIG` rollback: revert to previous static template. Existing user configs in `.agents/agentsync.toml` are not auto-modified, so no user-facing breakage.

## Dependencies

- None. All changes are self-contained within `src/init.rs` and its test module.
- The existing `skill-adoption` spec (Claude-specific) serves as the pattern to follow; no conflict.

## Success Criteria

- [ ] `scan_agent_files()` detects skill directories for all 8 agents listed (Cursor, Codex, Gemini, OpenCode, Roo, Factory, Vibe, Antigravity) when they exist with content
- [ ] `scan_agent_files()` detects command directories for Claude, Gemini, and OpenCode when they exist with content
- [ ] `scan_agent_files()` detects agent-specific MCP configs for all known agents
- [ ] `.windsurfrules`, `OPENCODE.md`, and `AMPCODE.md` are detected when present
- [ ] Wizard migrates discovered skills into `.agents/skills/` with collision detection (warn + skip)
- [ ] Wizard migrates discovered commands into `.agents/commands/`
- [ ] Wizard notes discovered MCP configs with a migration suggestion message
- [ ] `init()` creates `.agents/commands/` directory alongside `.agents/skills/`
- [ ] `DEFAULT_CONFIG` includes `[agents.claude.targets.commands]` and sections for `gemini` and `opencode`
- [ ] `DEFAULT_CONFIG` remains valid TOML that parses into `Config`
- [ ] All existing tests pass (no regressions)
- [ ] New tests cover each new scan entry, migration path, and collision scenario
