# Design: Universal Agent Adoption

## Technical Approach

Expand the existing `scan_agent_files()` / wizard migration pipeline in `src/init.rs` to detect and
migrate skills, commands, instruction files, and MCP configs across all known agents — not just
Claude. The approach is strictly additive: new `AgentFileType` variants, new scan entries, new
migration match arms, new `DEFAULT_CONFIG` sections. No existing behavior changes; no new modules or
abstractions.

Maps directly to the proposal's 6 deliverables:

1. Expanded scan with new enum variants
2. Skills normalization (all agents → `.agents/skills/`)
3. Commands migration (new `.agents/commands/` canonical location)
4. Instruction file scan fixes (`.windsurfrules`, `OPENCODE.md`, `AMPCODE.md`)
5. MCP config detection (note-only, no parsing)
6. `DEFAULT_CONFIG` update (commands target + gemini/opencode sections)

## Architecture Decisions

### Decision: Keep Explicit AgentFileType Variants (No Generic Wrapper)

**Choice**: Add ~20 explicit variants like `CursorSkills`, `GeminiCommands`, `RooMcpConfig`
**Alternatives considered**: Generic variants like `AgentSkills(String)` with agent name parameter
**Rationale**: The existing codebase uses explicit variants throughout — in `scan_agent_files()`, in
migration match arms, in backup exclusion checks, and in instruction file filtering. Generic
variants would require runtime string matching in every match arm, introduce a new category of
bugs (typo in agent name string), and break the exhaustive match pattern the compiler enforces.
With ~20 new variants the enum grows but remains manageable. The compiler guarantees every variant
is handled in every match block.

### Decision: Extract Skills Before Parent Dir Copy (No Deduplication Needed)

**Choice**: Scan for skills subdirectories as separate `AgentFileType` variants (e.g.,
`CursorSkills` for `.cursor/skills/`). These are independent from the parent directory variant (
e.g., `CursorDirectory` for `.cursor/`). During migration, skills variants copy contents to
`.agents/skills/`. The parent directory copy (`CursorDirectory`) continues to copy `.cursor/`
as-is — including the `skills/` subdirectory inside it.
**Alternatives considered**: (a) Skip `skills/` subdirectory during parent dir copy, (b) Remove
parent dir copy entirely when skills are extracted
**Rationale**: The parent directory copy preserves the full original structure in `.agents/.cursor/`
for reference/backup. The skills extraction into `.agents/skills/` is the canonical managed
location. Duplication is acceptable because: (1) the `.agents/.cursor/` copy is inert (not synced
back), (2) skills in `.agents/skills/` are the ones actually managed by `symlink-contents`, and (3)
avoiding duplication would require modifying `copy_dir_all()` with skip lists, adding complexity for
no functional benefit. Users can delete the `.agents/.cursor/` backup after confirming migration.

### Decision: Commands Use `symlink-contents` Type

**Choice**: `.agents/commands/` is the canonical source, synced to agent-specific locations via
`type = "symlink-contents"` targets (same as skills).
**Alternatives considered**: (a) `type = "symlink"` (links the whole directory), (b) New sync type
**Rationale**: `symlink-contents` creates individual symlinks per file inside the destination, which
is exactly how `.claude/commands/` and `.gemini/commands/` work — each command is an independent
markdown file. This matches the existing skills pattern perfectly. No new sync type needed.

### Decision: Flat `.agents/commands/` Directory (No Agent Namespacing)

**Choice**: All commands from all agents merge into a single `.agents/commands/` directory.
**Alternatives considered**: Namespaced directories like `.agents/commands/claude/`,
`.agents/commands/gemini/`
**Rationale**: Commands are markdown files with a largely interchangeable format. The canonical
directory should be agent-agnostic (same philosophy as `.agents/skills/`). If two agents have a
command with the same filename, the collision handler (warn + skip) prevents overwrites. Users can
rename to resolve conflicts. Agent-specific distribution happens via `symlink-contents` targets in
`agentsync.toml`.

### Decision: MCP Configs Note-Only in Phase 1

**Choice**: Detect agent-specific MCP configs and print an informational note; do not parse or
import.
**Alternatives considered**: Parse JSON/TOML MCP configs and generate `[mcp_servers]` entries
**Rationale**: MCP config formats vary wildly across agents (JSON with different schemas for
Claude/Cursor/Roo/etc., TOML for Codex, nested JSON for Kiro). The existing MCP generation system (
`src/mcp.rs`) is already mature and handles all 7 MCP-native agents. Parsing 8+ formats for import
would be high-effort, error-prone, and is explicitly deferred to Phase 2.

### Decision: Static DEFAULT_CONFIG Extension (No Dynamic Builder)

**Choice**: Extend the existing static `DEFAULT_CONFIG` string with new sections.
**Alternatives considered**: Dynamic config builder that generates TOML based on discovered agents
**Rationale**: Dynamic generation is Phase 2. For Phase 1, adding a few sections to the static
template is minimal risk and maintains the existing pattern. The template serves as documentation
and a starting point; users customize it after init.

## Data Flow

### Wizard Migration Flow (Multi-Agent)

```
agentsync init --wizard
        │
        ▼
scan_agent_files(project_root)
        │
        ├── Check instruction files: CLAUDE.md, GEMINI.md, OPENCODE.md, AMPCODE.md,
        │   .windsurfrules, .clinerules, CRUSH.md, WARP.md, copilot-instructions.md, AGENTS.md
        │
        ├── Check skill directories: .claude/skills/, .cursor/skills/, .codex/skills/,
        │   .gemini/skills/, .opencode/skills/, .roo/skills/, .factory/skills/,
        │   .vibe/skills/, .agent/skills/  (each with has_content check)
        │
        ├── Check command directories: .claude/commands/, .gemini/commands/,
        │   .opencode/command/  (each with has_content check)
        │
        ├── Check MCP configs: .cursor/mcp.json, .windsurf/mcp_config.json,
        │   .codex/config.toml, .roo/mcp.json, .kiro/settings/mcp.json,
        │   .amazonq/mcp.json, .kilocode/mcp.json, .factory/mcp.json,
        │   opencode.json, .vscode/mcp.json
        │
        └── Check directories + single-file configs (existing behavior)
        │
        ▼
User selects files to migrate (MultiSelect)
        │
        ▼
Migration Loop (for each selected file)
        │
        ├─ Instruction files ──────→ Merge into .agents/AGENTS.md
        │
        ├─ *Skills variants ───────→ Copy contents → .agents/skills/
        │                             (warn + skip on collision)
        │
        ├─ *Commands variants ─────→ Copy contents → .agents/commands/
        │                             (warn + skip on collision)
        │
        ├─ *McpConfig variants ────→ Print note: "configure in [mcp_servers]"
        │
        ├─ Directory variants ─────→ Copy as-is → .agents/{path}
        │
        └─ Single-file configs ────→ Copy file → .agents/{path}
        │
        ▼
Write DEFAULT_CONFIG → .agents/agentsync.toml
        │
        ▼
Offer backup of original files
```

### Scan Order Within `scan_agent_files()`

The scan order matters for user display. Group by agent, with artifacts ordered: instructions →
skills → commands → MCP config → directory.

```
Native MCP agents (existing order preserved):
  Claude:   CLAUDE.md → .claude/skills/ → .claude/commands/
  Copilot:  copilot-instructions.md
            .vscode/mcp.json  (NEW)
  Cursor:   .cursor/ (dir) → .cursor/skills/ (NEW) → .cursor/mcp.json (NEW)
  Gemini:   GEMINI.md → .gemini/skills/ (NEW) → .gemini/commands/ (NEW)
  OpenCode: OPENCODE.md (NEW) → .opencode/skills/ (NEW) → .opencode/command/ (NEW)
            opencode.json (NEW)
  Codex:    AGENTS.md → .codex/skills/ (NEW) → .codex/config.toml (NEW)
  .mcp.json (existing)

Configurable agents (existing order preserved, new entries added after each):
  Windsurf: .windsurfrules (NEW scan) → .windsurf/ (dir) → .windsurf/mcp_config.json (NEW)
  Amp:      AMPCODE.md (NEW scan)
  Roo:      .roo/rules/ → .roo/skills/ (NEW) → .roo/mcp.json (NEW)
  Kiro:     .kiro/steering/ → .kiro/settings/mcp.json (NEW)
  Factory:  .factory/ (dir) → .factory/skills/ (NEW) → .factory/mcp.json (NEW)
  Vibe:     .vibe/ (dir) → .vibe/skills/ (NEW)
  Antigravity: .agent/rules/ → .agent/skills/ (NEW)
  Amazon Q: .amazonq/rules/ → .amazonq/mcp.json (NEW)
  Kilocode: .kilocode/ (dir) → .kilocode/mcp.json (NEW)
  (all others: unchanged)
```

## File Changes

| File                                     | Action | Description                                                                                                                                                                      |
|------------------------------------------|--------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `src/init.rs` — `AgentFileType` enum     | Modify | Add ~20 new variants: 8 skills, 3 commands, 10 MCP configs, 2 instructions                                                                                                       |
| `src/init.rs` — `scan_agent_files()`     | Modify | Add ~25 new scan entries following existing pattern (existence + has_content checks)                                                                                             |
| `src/init.rs` — instruction_files filter | Modify | Add `OpenCodeInstructions` and `AmpInstructions` to the instruction merge filter (AmpInstructions variant exists but is not in the filter; add it. OpenCodeInstructions is new.) |
| `src/init.rs` — migration match arms     | Modify | Add match arms for new skills, commands, and MCP variants                                                                                                                        |
| `src/init.rs` — `init()`                 | Modify | Create `.agents/commands/` directory alongside `.agents/skills/`                                                                                                                 |
| `src/init.rs` — `init_wizard()`          | Modify | Create `.agents/commands/` directory in wizard flow                                                                                                                              |
| `src/init.rs` — `DEFAULT_CONFIG`         | Modify | Add `[agents.claude.targets.commands]`, `[agents.gemini]`, `[agents.opencode]` sections                                                                                          |
| `src/init.rs` — backup exclusion         | Modify | Add new MCP variants to the "skip files that weren't actually migrated" check                                                                                                    |
| `src/init.rs` — tests                    | Modify | Add ~25 new tests for scan entries, ~3 for migration, ~2 for DEFAULT_CONFIG                                                                                                      |

## Interfaces / Contracts

### New `AgentFileType` Variants

```rust
enum AgentFileType {
    // === EXISTING (unchanged) ===
    ClaudeInstructions,
    CopilotInstructions,
    RootAgentsFile,
    WindsurfRules,
    ClineRules,
    CrushInstructions,
    AmpInstructions,       // existing but not scanned — will wire up
    // ... all other existing variants ...
    ClaudeSkills,
    CursorDirectory,
    WindsurfDirectory,
    McpConfig,
    Other,

    // === NEW: Instructions ===
    OpenCodeInstructions,  // OPENCODE.md

    // === NEW: Skills ===
    CursorSkills,          // .cursor/skills/
    CodexSkills,           // .codex/skills/
    GeminiSkills,          // .gemini/skills/
    OpenCodeSkills,        // .opencode/skills/
    RooSkills,             // .roo/skills/
    FactorySkills,         // .factory/skills/
    VibeSkills,            // .vibe/skills/
    AntigravitySkills,     // .agent/skills/

    // === NEW: Commands ===
    ClaudeCommands,        // .claude/commands/
    GeminiCommands,        // .gemini/commands/
    OpenCodeCommands,      // .opencode/command/  (note: singular)

    // === NEW: MCP Configs ===
    CursorMcpConfig,       // .cursor/mcp.json
    CopilotMcpConfig,      // .vscode/mcp.json
    WindsurfMcpConfig,     // .windsurf/mcp_config.json
    CodexConfig,           // .codex/config.toml
    RooMcpConfig,          // .roo/mcp.json
    KiroMcpConfig,         // .kiro/settings/mcp.json
    AmazonQMcpConfig,      // .amazonq/mcp.json
    KilocodeMcpConfig,     // .kilocode/mcp.json
    FactoryMcpConfig,      // .factory/mcp.json
    OpenCodeConfig,        // opencode.json
}
```

### New DEFAULT_CONFIG Sections

```toml
# After [agents.claude.targets.skills]:
[agents.claude.targets.commands]
source = "commands"
destination = ".claude/commands"
type = "symlink-contents"

# New agent section:
[agents.gemini]
enabled = true
description = "Gemini CLI - Google's AI coding assistant"

[agents.gemini.targets.instructions]
source = "AGENTS.md"
destination = "GEMINI.md"
type = "symlink"

[agents.gemini.targets.skills]
source = "skills"
destination = ".gemini/skills"
type = "symlink-contents"

[agents.gemini.targets.commands]
source = "commands"
destination = ".gemini/commands"
type = "symlink-contents"

# New agent section:
[agents.opencode]
enabled = true
description = "OpenCode - Open-source AI coding assistant"

[agents.opencode.targets.instructions]
source = "AGENTS.md"
destination = "OPENCODE.md"
type = "symlink"

[agents.opencode.targets.skills]
source = "skills"
destination = ".opencode/skills"
type = "symlink-contents"

[agents.opencode.targets.commands]
source = "commands"
destination = ".opencode/command"
type = "symlink-contents"
```

### Migration Match Arm Patterns

Skills migration (all `*Skills` variants share the same pattern as `ClaudeSkills`):

```rust
AgentFileType::ClaudeSkills
| AgentFileType::CursorSkills
| AgentFileType::CodexSkills
| AgentFileType::GeminiSkills
| AgentFileType::OpenCodeSkills
| AgentFileType::RooSkills
| AgentFileType::FactorySkills
| AgentFileType::VibeSkills
| AgentFileType::AntigravitySkills => {
    // existing ClaudeSkills logic (iterate, check collision, copy)
}
```

Commands migration (new arm, identical pattern but targets `commands_dir`):

```rust
AgentFileType::ClaudeCommands
| AgentFileType::GeminiCommands
| AgentFileType::OpenCodeCommands => {
    if src_path.exists() && src_path.is_dir() {
        for entry in fs::read_dir(&src_path)? {
            let entry = entry?;
            let entry_path = entry.path();
            let cmd_name = entry.file_name();
            let dest_cmd = commands_dir.join(&cmd_name);
            if dest_cmd.exists() {
                // warn + skip (same as skills)
            } else if entry_path.is_dir() {
                copy_dir_all(&entry_path, &dest_cmd)?;
            } else {
                fs::copy(&entry_path, &dest_cmd)?;
            }
        }
    }
}
```

MCP config note (extend existing `McpConfig | ZedSettings` arm):

```rust
AgentFileType::McpConfig
| AgentFileType::ZedSettings
| AgentFileType::CursorMcpConfig
| AgentFileType::CopilotMcpConfig
| AgentFileType::WindsurfMcpConfig
| AgentFileType::CodexConfig
| AgentFileType::RooMcpConfig
| AgentFileType::KiroMcpConfig
| AgentFileType::AmazonQMcpConfig
| AgentFileType::KilocodeMcpConfig
| AgentFileType::FactoryMcpConfig
| AgentFileType::OpenCodeConfig => {
    println!("  {} Note: {} detected. You can configure MCP servers in agentsync.toml",
        "ℹ".blue(), file.path.display());
    files_skipped += 1;
}
```

Instruction file filter update:

```rust
// Add to the instruction_files filter:
| AgentFileType::OpenCodeInstructions
| AgentFileType::AmpInstructions  // already in enum, now added to filter
```

Backup exclusion update (add all new MCP variants alongside existing `McpConfig | ZedSettings`):

```rust
if matches!(
    file.file_type,
    AgentFileType::McpConfig
        | AgentFileType::ZedSettings
        | AgentFileType::CursorMcpConfig
        | AgentFileType::CopilotMcpConfig
        | AgentFileType::WindsurfMcpConfig
        | AgentFileType::CodexConfig
        | AgentFileType::RooMcpConfig
        | AgentFileType::KiroMcpConfig
        | AgentFileType::AmazonQMcpConfig
        | AgentFileType::KilocodeMcpConfig
        | AgentFileType::FactoryMcpConfig
        | AgentFileType::OpenCodeConfig
        | AgentFileType::Other
) {
    continue; // Skip files that weren't actually migrated
}
```

### Scan Entry Pattern (for new entries)

All new scan entries follow the existing pattern. For skill/command directories with `has_content`
check:

```rust
// Example: Cursor skills
let cursor_skills_path = project_root.join(".cursor").join("skills");
if cursor_skills_path.exists() && cursor_skills_path.is_dir() {
    let has_content = fs::read_dir(&cursor_skills_path)
        .ok()
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);
    if has_content {
        discovered.push(DiscoveredFile {
            path: ".cursor/skills".into(),
            file_type: AgentFileType::CursorSkills,
            display_name: "Cursor skills (.cursor/skills/)".to_string(),
        });
    }
}
```

For MCP config files (simple existence check):

```rust
// Example: Cursor MCP config
let cursor_mcp_path = project_root.join(".cursor").join("mcp.json");
if cursor_mcp_path.exists() {
    discovered.push(DiscoveredFile {
        path: ".cursor/mcp.json".into(),
        file_type: AgentFileType::CursorMcpConfig,
        display_name: ".cursor/mcp.json (Cursor MCP configuration)".to_string(),
    });
}
```

## Collision Handling Strategy

### Skills Collisions

When multiple agents have a skill with the same directory name (e.g., `.claude/skills/my-skill/` and
`.cursor/skills/my-skill/`):

1. **First writer wins** — skills are processed in scan order (Claude first as the existing
   behavior)
2. **Subsequent collisions** — warn with `⚠` and skip:
   `"Skipped: skill 'my-skill' already exists in .agents/skills/"`
3. **No overwrite** — existing content is always preserved
4. **Resolution** — users rename one of the conflicting skills before re-running wizard

This matches the existing `ClaudeSkills` collision behavior exactly (lines 740-746 of `init.rs`).

### Commands Collisions

Same strategy as skills:

1. First writer wins
2. Warn + skip on collision
3. No overwrite

### Instruction File Merging

No collision — all instruction files are merged into a single `AGENTS.md` with section separators (
`---`) and headers showing the source file. This is existing behavior; adding `OpenCodeInstructions`
and `AmpInstructions` to the filter extends it naturally.

## Testing Strategy

| Layer | What to Test                                   | Approach                                                                                                                                            |
|-------|------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------|
| Unit  | Each new scan entry detects its file/directory | `test_scan_agent_files_finds_{variant}` pattern — create the file in a TempDir, call `scan_agent_files()`, assert the expected variant is found     |
| Unit  | Empty skill/command dirs are ignored           | Create empty directory, verify not in discovered list                                                                                               |
| Unit  | Skills migration from multiple agents          | Create skills in `.cursor/skills/` and `.roo/skills/`, simulate migration, verify contents in `.agents/skills/`                                     |
| Unit  | Commands migration                             | Create commands in `.claude/commands/` and `.gemini/commands/`, simulate migration, verify contents in `.agents/commands/`                          |
| Unit  | Skill collision detection                      | Pre-populate `.agents/skills/shared-name/`, attempt migration from two agents, verify skip + original preserved                                     |
| Unit  | Command collision detection                    | Same pattern as skill collision                                                                                                                     |
| Unit  | `DEFAULT_CONFIG` validity                      | Existing `test_default_config_is_valid_toml` — already catches any TOML syntax errors in the updated template                                       |
| Unit  | `DEFAULT_CONFIG` new sections                  | Assert `config.agents.contains_key("gemini")`, `config.agents.contains_key("opencode")`, `config.agents["claude"].targets.contains_key("commands")` |
| Unit  | `init()` creates commands directory            | Call `init()`, assert `.agents/commands/` exists                                                                                                    |
| Unit  | Instruction filter includes new variants       | Create `OPENCODE.md` + `AMPCODE.md` + `CLAUDE.md`, scan, verify all are in instruction_files filter                                                 |

**Test count estimate**: ~25 new scan tests + ~5 migration tests + ~3 config tests = ~33 new tests.

## Migration / Rollout

No migration required. All changes are additive to `src/init.rs`:

- New enum variants don't affect existing serialization (enum is not serialized)
- New scan entries only add to the discovered list; existing entries unchanged
- New match arms handle new variants; existing arms unchanged
- `DEFAULT_CONFIG` additions are valid TOML that extend the existing template
- `init()` creating `.agents/commands/` is harmless if directory already exists

**Backward compatibility**: A project initialized with the old wizard and re-run with the new wizard
will:

1. Detect additional files (skills, commands, MCP configs from non-Claude agents)
2. Offer to migrate them — user can accept or decline
3. Not touch any existing `.agents/` content (the `exists && !force` guards remain)

## Open Questions

- [x] ~~Skills extraction vs parent dir copy duplication~~ → Resolved: accept duplication (see
  Architecture Decisions)
- [x] ~~AgentFileType variant approach~~ → Resolved: keep explicit variants
- [x] ~~Commands sync type~~ → Resolved: `symlink-contents`
- [x] ~~Commands directory structure~~ → Resolved: flat `.agents/commands/`
- [ ] Should `AmpInstructions` scan check for `AMPCODE.md` or `.ampcode`? The
  `agent_convention_filename("amp")` returns `"AMPCODE.md"`. Need to verify this is the correct
  filename in the wild.
