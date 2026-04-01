## Exploration: universal-agent-adoption

### Current State

#### What the Wizard Already Does (`src/init.rs`)

The `init_wizard()` calls `scan_agent_files()` which detects **30+ agent file types** via the
`AgentFileType` enum. On migration, it groups files into four categories:

1. **Instruction files** (merged into `.agents/AGENTS.md`): `ClaudeInstructions`, `RootAgentsFile`,
   `CopilotInstructions`, `WindsurfRules`, `ClineRules`, `CrushInstructions`, `AmpInstructions`,
   `GooseHints`, `WarpInstructions`, `GeminiInstructions`
2. **Skill directories** (contents copied into `.agents/skills/`): `ClaudeSkills` — this is the ONLY
   skill type currently handled
3. **Directories** (copied as-is into `.agents/`): `CursorDirectory`, `WindsurfDirectory`,
   `AntigravityRules`, `AmazonQRules`, `OpenHandsMicroagents`, `JunieDirectory`, `AugmentRules`,
   `KilocodeDirectory`, `QwenDirectory`, `RooRules`, `TraeRules`, `KiroSteering`,
   `FactoryDirectory`, `VibeDirectory`, `JetBrainsRules`
4. **Noted but not migrated**: `McpConfig`, `ZedSettings`
5. **Single-file configs** (copied into `.agents/`): `AiderConfig`, `FirebenderConfig`,
   `FirebaseRules`

After migration, the wizard writes the **static `DEFAULT_CONFIG`** template regardless of what was
discovered. Config is not customized to the detected files.

#### Agent Identification (`src/agent_ids.rs`)

Two registries of canonical agent IDs:

- **MCP-native agents** (7): claude, copilot, codex, gemini, vscode, cursor, opencode
- **Configurable agents** (20+): windsurf, cline, crush, amp, antigravity, amazonq, aider, firebase,
  openhands, junie, augment, kilocode, goose, qwen, roo, zed, trae, warp, kiro, firebender, factory,
  vibe, jetbrains, pi, jules

Each agent has:

- `known_ignore_patterns()` — gitignore entries (includes skills/, commands/, mcp.json paths)
- `agent_convention_filename()` — instruction file name (only for ~10 agents)

#### MCP System (`src/mcp.rs`)

Comprehensive MCP config generation for 7 agents. Each has a dedicated formatter producing
agent-specific file formats. Supports merge and overwrite strategies. The wizard currently only *
*notes** `.mcp.json` existence — it does NOT import MCP server definitions into `agentsync.toml`.

#### Config Generation (`src/init.rs`)

`DEFAULT_CONFIG` is a **static string literal** — always the same regardless of what was discovered.
It defines targets for: claude (instructions + skills), copilot (instructions), cursor (no targets),
codex (skills), root (agents). No dynamic config building exists.

### Agent File Inventory: What Exists vs What's Detected vs What's Migrated Properly

| Agent           | Artifact Type | File/Path                         | Detected?                                      | Migration Category                 | Gap                                                  |
|-----------------|---------------|-----------------------------------|------------------------------------------------|------------------------------------|------------------------------------------------------|
| **Claude**      | Instructions  | `CLAUDE.md`                       | Yes                                            | Merged into AGENTS.md              | None                                                 |
|                 | Skills        | `.claude/skills/`                 | Yes                                            | Contents → `.agents/skills/`       | None (just added)                                    |
|                 | Commands      | `.claude/commands/*.md`           | **No**                                         | —                                  | Not detected or migrated                             |
|                 | MCP config    | `.mcp.json`                       | Yes                                            | Noted only                         | Not imported into toml                               |
| **Copilot**     | Instructions  | `.github/copilot-instructions.md` | Yes                                            | Merged into AGENTS.md              | None                                                 |
|                 | MCP config    | `.vscode/mcp.json`                | **No**                                         | —                                  | Not detected                                         |
| **Cursor**      | Directory     | `.cursor/`                        | Yes                                            | Copied as-is to `.agents/.cursor/` | Loses structure context                              |
|                 | Rules         | `.cursor/rules/*.mdc`             | **No** (only dir)                              | —                                  | Sub-files not parsed                                 |
|                 | Tools         | `.cursor/tools/`                  | **No**                                         | —                                  | Not detected                                         |
|                 | Skills        | `.cursor/skills/`                 | **No**                                         | —                                  | Not detected                                         |
|                 | MCP config    | `.cursor/mcp.json`                | **No** (only dir)                              | —                                  | Not extracted from dir                               |
| **Windsurf**    | Rules         | `.windsurfrules`                  | **No** (scan checks enum but file not scanned) | —                                  | `WindsurfRules` enum variant exists but no scan code |
|                 | Directory     | `.windsurf/`                      | Yes                                            | Copied as-is                       | Loses structure context                              |
|                 | MCP config    | `.windsurf/mcp_config.json`       | **No**                                         | —                                  | Not extracted from dir                               |
| **Codex**       | Instructions  | `AGENTS.md`                       | Yes (as RootAgentsFile)                        | Merged into AGENTS.md              | Shared with root                                     |
|                 | Skills        | `.codex/skills/`                  | **No**                                         | —                                  | Not detected; only in DEFAULT_CONFIG                 |
|                 | MCP config    | `.codex/config.toml`              | **No**                                         | —                                  | Not detected                                         |
| **Gemini**      | Instructions  | `GEMINI.md`                       | Yes                                            | Merged into AGENTS.md              | None                                                 |
|                 | Commands      | `.gemini/commands/`               | **No**                                         | —                                  | Not detected                                         |
|                 | Skills        | `.gemini/skills/`                 | **No**                                         | —                                  | Not detected                                         |
|                 | Settings      | `.gemini/settings.json`           | **No**                                         | —                                  | Not detected                                         |
| **OpenCode**    | Instructions  | `OPENCODE.md`                     | **No**                                         | —                                  | Not detected                                         |
|                 | Commands      | `.opencode/command/`              | **No**                                         | —                                  | Not detected                                         |
|                 | Skills        | `.opencode/skills/`               | **No**                                         | —                                  | Not detected                                         |
|                 | MCP config    | `opencode.json`                   | **No**                                         | —                                  | Not detected                                         |
| **Roo**         | Rules         | `.roo/rules/`                     | Yes                                            | Copied as-is                       | OK                                                   |
|                 | Skills        | `.roo/skills/`                    | **No**                                         | —                                  | Not detected                                         |
|                 | MCP config    | `.roo/mcp.json`                   | **No**                                         | —                                  | Not detected                                         |
| **Kiro**        | Steering      | `.kiro/steering/`                 | Yes                                            | Copied as-is                       | OK                                                   |
|                 | MCP config    | `.kiro/settings/mcp.json`         | **No**                                         | —                                  | Not detected                                         |
| **Factory**     | Directory     | `.factory/`                       | Yes                                            | Copied as-is                       | OK                                                   |
|                 | Skills        | `.factory/skills/`                | **No**                                         | —                                  | Not detected                                         |
|                 | MCP config    | `.factory/mcp.json`               | **No**                                         | —                                  | Not detected                                         |
| **Vibe**        | Directory     | `.vibe/`                          | Yes                                            | Copied as-is                       | OK                                                   |
|                 | Skills        | `.vibe/skills/`                   | **No**                                         | —                                  | Not detected                                         |
| **Antigravity** | Rules         | `.agent/rules/`                   | Yes                                            | Copied as-is                       | OK                                                   |
|                 | Skills        | `.agent/skills/`                  | **No**                                         | —                                  | Known in gitignore patterns                          |
| **Amazon Q**    | Rules         | `.amazonq/rules/`                 | Yes                                            | Copied as-is                       | OK                                                   |
|                 | MCP config    | `.amazonq/mcp.json`               | **No**                                         | —                                  | Not detected                                         |
| **Cline**       | Rules         | `.clinerules`                     | Yes                                            | Noted/merged                       | OK                                                   |
| **Kilocode**    | Directory     | `.kilocode/`                      | Yes                                            | Copied as-is                       | OK                                                   |
|                 | MCP config    | `.kilocode/mcp.json`              | **No**                                         | —                                  | Not detected                                         |

### Gap Summary

#### 1. Skills Not Detected (except Claude)

The following agents have skills directories listed in `known_ignore_patterns()` but NOT detected by
`scan_agent_files()`:

- `.cursor/skills/`, `.codex/skills/`, `.gemini/skills/`, `.opencode/skills/`, `.roo/skills/`,
  `.factory/skills/`, `.vibe/skills/`, `.agent/skills/` (Antigravity)

**Impact**: Users with skills in these directories lose them on adoption.

#### 2. Commands Not Detected

Known command directories (from `known_ignore_patterns()`):

- `.claude/commands/`, `.gemini/commands/`, `.opencode/command/`

**Impact**: These are markdown files that act as slash commands. No detection, no migration, no
canonical location in `.agents/`.

#### 3. MCP Configs Not Imported

The wizard notes `.mcp.json` but doesn't parse it. Agent-specific MCP configs (`.cursor/mcp.json`,
`.windsurf/mcp_config.json`, `.codex/config.toml`, `.gemini/settings.json`, `.roo/mcp.json`,
`.kiro/settings/mcp.json`, `.amazonq/mcp.json`, `.kilocode/mcp.json`, `.factory/mcp.json`,
`opencode.json`) are completely invisible.

**Impact**: Users must manually recreate their MCP server definitions in `agentsync.toml`.

#### 4. Static Config Generation

`DEFAULT_CONFIG` always writes the same template. If a user has Cursor + Windsurf + Roo but not
Claude, they still get Claude targets and miss targets for their actual agents.

**Impact**: The generated config doesn't match the project's reality. Users must manually edit.

#### 5. WindsurfRules Scan Gap

`AgentFileType::WindsurfRules` enum variant exists (for `.windsurfrules` the file), and it's in the
merge match arms, but `scan_agent_files()` never scans for this file — only `.windsurf/` directory
is checked.

#### 6. Several Agents Missing from Scan

`OpenCode` (OPENCODE.md, opencode.json, `.opencode/`), `Amp` (AMPCODE.md), several others with
convention filenames are not scanned at all.

### Approaches

#### 1. **Incremental: Expand scan + skill normalization

** — Add missing scan entries for skills/commands/MCP across all agents. Normalize skills into
`.agents/skills/` (like ClaudeSkills). Keep static DEFAULT_CONFIG.

- Pros: Minimal architecture change; additive; low risk; each agent can be added independently
- Cons: Static config still won't match detected agents; MCP import requires parsing multiple
  formats; commands concept needs new canonical location
- Effort: Medium

#### 2. **Dynamic config generation** — Replace static

`DEFAULT_CONFIG` with a builder that generates
`agentsync.toml` based on discovered files. Each detected agent gets appropriate targets
auto-configured.

- Pros: Config perfectly matches the project; no manual editing needed; true "universal adoption";
  the static template becomes a fallback for empty projects
- Cons: Significant new code (config builder); must maintain parity with what the static template
  provides; more test surface; must handle agent target types (instructions, skills, commands)
  generically
- Effort: High

#### 3. **Full universal adoption

** — Combine approaches 1 and 2, plus: parse MCP configs and import servers into
`[mcp_servers]`; create
`.agents/commands/` as canonical command location; normalize skills from all agents.

- Pros: Complete solution; users go from "any agent setup" to "fully managed" in one wizard run;
  commands and MCP are first-class
- Cons: Very large scope; MCP parsing is complex (JSON, TOML, different key names per agent);
  commands concept needs design work; risk of over-engineering
- Effort: Very High

### Recommendation

**Phased approach: Start with Approach 1, then add dynamic config (Approach 2) as Phase 2.**

**Phase 1** (this change):

1. **Expand `scan_agent_files()`** to detect ALL known skill directories, command directories, and
   agent-specific MCP configs. Add new `AgentFileType` variants: `CursorSkills`, `CodexSkills`,
   `GeminiSkills`, `GeminiCommands`, `OpenCodeSkills`, `OpenCodeCommands`, `RooSkills`,
   `FactorySkills`, `VibeSkills`, `AntigravitySkills`, `ClaudeCommands`, and similar.
2. **Normalize skill migration** — All agent skill directories get their contents merged into
   `.agents/skills/` (same as `ClaudeSkills` handling). Add prefix or namespace if collision
   detection shows overlap (e.g., warn and skip).
3. **Add commands migration** — Create `.agents/commands/` as canonical location. Copy
   `.claude/commands/*.md`, `.gemini/commands/`, `.opencode/command/` contents there. Add a new
   target type or use `symlink-contents` to sync back.
4. **Scan for missing instruction files** — Add detection for `.windsurfrules`, `OPENCODE.md`,
   `AMPCODE.md`, and any other convention filenames that `agent_convention_filename()` knows about
   but `scan_agent_files()` doesn't check.
5. **Detect agent-specific MCP configs** — Add scan entries. For Phase 1, just **note** them (like
   current `.mcp.json` handling) with a message suggesting manual migration to `[mcp_servers]`.

**Phase 2** (follow-up change):

1. **Dynamic config generation** — Build `agentsync.toml` content based on discoveries. Only add
   `[agents.X]` sections for agents actually found.
2. **MCP import** — Parse discovered MCP configs and generate `[mcp_servers]` entries in the
   generated config.

### Key Architectural Decisions Needed

1. **Commands canonical location**: Should commands live in `.agents/commands/` as a flat directory?
   Or namespaced by agent (`.agents/commands/claude/`, `.agents/commands/gemini/`)? Commands are
   agent-specific (Claude's slash commands are markdown files with a specific format). *
   *Recommendation**: `.agents/commands/` flat, since the format is markdown and largely
   interchangeable.

2. **Skills namespace collisions**: When merging skills from multiple agents (`.claude/skills/foo/`
   and `.cursor/skills/foo/`), what happens? **Recommendation**: Warn and skip duplicates (existing
   behavior for ClaudeSkills), since skills should be agent-agnostic in the canonical location.

3. **Config targets for commands**: Should `DEFAULT_CONFIG` include
   `[agents.claude.targets.commands]` etc.? **Recommendation**: Yes, if commands directory exists.
   Target: `source = "commands"`, `destination = ".claude/commands"`, `type = "symlink-contents"`.

4. **MCP import depth**: Should Phase 1 attempt to parse and import MCP servers, or just note them?
   **Recommendation**: Note only in Phase 1. MCP configs vary widely in format (JSON with different
   schemas per agent, TOML for Codex) and the existing MCP generation system is already mature —
   users can define servers in `agentsync.toml` and let the system generate all agent configs.

5. **Which agents get targets in DEFAULT_CONFIG**: Currently only claude, copilot, cursor, codex,
   root. Should we add gemini, opencode, windsurf, etc.? **Recommendation**: Yes — add all
   MCP-native agents (gemini, opencode) and the most popular configurable agents (windsurf, cursor
   rules). Phase 2's dynamic config makes this less critical.

### Risks

- **Scope creep**: 30+ agents × 4 artifact types (instructions, skills, commands, MCP) = potentially
  120+ scan entries. Must be selective — only add entries for artifacts we know exist in the wild.
- **Breaking existing wizard behavior**: The current directory copy-as-is behavior (e.g.,
  `.cursor/` → `.agents/.cursor/`) means skills inside `.cursor/skills/` are already captured.
  Changing to extract skills separately could conflict. **Mitigation**: Extract skills BEFORE
  copying the parent directory, or skip skills subdirs during parent dir copy.
- **Commands format divergence**: Claude commands are `*.md` files with frontmatter. Gemini commands
  may differ. OpenCode uses `.opencode/command/` (singular). Normalizing may lose agent-specific
  metadata. **Mitigation**: Copy as-is; let `symlink-contents` handle the distribution.
- **MCP config parsing complexity**: Each agent stores MCP config differently. Full import would
  require parsers for 8+ formats. **Mitigation**: Phase 1 just notes them; Phase 2 tackles import.
- **Test surface**: Each new scan entry needs tests. The existing test pattern (
  `test_scan_agent_files_finds_*`) is clear but adding 20+ tests is significant.
- **Config generation correctness**: Dynamic config must produce valid TOML that parses into
  `Config`. The existing `test_default_config_is_valid_toml()` test pattern should be extended to
  cover generated configs.

### Ready for Proposal

Yes — the exploration provides a clear gap analysis and phased approach. The orchestrator should
proceed to the proposal phase with:

- Phase 1 scope: expanded scan, skills/commands normalization, instruction file gap fixes
- Phase 2 scope: dynamic config generation, MCP import
- Key decisions on commands location and namespace collisions documented above
