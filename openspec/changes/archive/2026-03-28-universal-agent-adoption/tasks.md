# Tasks: Universal Agent Adoption

## Phase 1: Foundation — New Enum Variants & Scan Entries

- [x] 1.1 Add `OpenCodeInstructions` variant to `AgentFileType` enum in `src/init.rs`. Add scan entry for `OPENCODE.md` file existence. Add to instruction_files merge filter. *(Spec: Scan Detects Missing Instruction Files — OPENCODE.md scenario; Design: §Interfaces/New AgentFileType Variants)*
- [x] 1.2 Add scan entry for `.windsurfrules` (variant exists, no scan code) and wire `AmpInstructions` scan for `AMPCODE.md`. Add both to instruction_files merge filter if missing. *(Spec: .windsurfrules + AMPCODE.md scenarios; Design: §Scan Order — Windsurf/Amp)*
- [x] 1.3 Add 8 skill variants (`CursorSkills`, `CodexSkills`, `GeminiSkills`, `OpenCodeSkills`, `RooSkills`, `FactorySkills`, `VibeSkills`, `AntigravitySkills`) to enum. Add scan entries with `has_content` directory checks. *(Spec: Scan Detects Agent Skill Directories — all scenarios; Design: §Scan Entry Pattern)*
- [x] 1.4 Add 3 command variants (`ClaudeCommands`, `GeminiCommands`, `OpenCodeCommands`) to enum. Add scan entries with `has_content` directory checks for `.claude/commands/`, `.gemini/commands/`, `.opencode/command/`. *(Spec: Scan Detects Command Directories; Design: §Scan Entry Pattern)*
- [x] 1.5 Add 10 MCP config variants (`CursorMcpConfig`, `CopilotMcpConfig`, `WindsurfMcpConfig`, `CodexConfig`, `RooMcpConfig`, `KiroMcpConfig`, `AmazonQMcpConfig`, `KilocodeMcpConfig`, `FactoryMcpConfig`, `OpenCodeConfig`) to enum. Add scan entries with file existence checks. *(Spec: Scan Detects Agent-Specific MCP Configs; Design: §Scan Entry Pattern — MCP)*

## Phase 2: Core Implementation — Migration Logic & Config

- [x] 2.1 Extend skills migration match arm: add all 8 new `*Skills` variants alongside `ClaudeSkills` using the same copy-contents + collision-detect pattern. *(Spec: Wizard Migrates Skills from All Agents; Design: §Migration Match Arm Patterns — skills)*
- [x] 2.2 Add commands migration match arm: `ClaudeCommands | GeminiCommands | OpenCodeCommands` → copy contents to `.agents/commands/` with collision warn+skip. *(Spec: Wizard Migrates Commands to Canonical Location; Design: §Migration Match Arm Patterns — commands)*
- [x] 2.3 Extend MCP note match arm: add all 10 new `*McpConfig`/`*Config` variants alongside existing `McpConfig | ZedSettings`. Print note with path + suggestion. *(Spec: MCP Config Note Handling; Design: §Migration Match Arm Patterns — MCP)*
- [x] 2.4 Update backup exclusion check: add all new MCP variants so they're skipped during backup. *(Design: §Backup exclusion update)*
- [x] 2.5 Create `.agents/commands/` directory in both `init()` and `init_wizard()` flows, alongside existing `.agents/skills/` creation. *(Spec: Init creates commands directory; Design: §File Changes — init())*
- [x] 2.6 Update `DEFAULT_CONFIG`: add `[agents.claude.targets.commands]` section. Add `[agents.gemini]` and `[agents.opencode]` agent sections with instructions, skills, and commands targets. *(Spec: DEFAULT_CONFIG requirements; Design: §New DEFAULT_CONFIG Sections)*

## Phase 3: Testing

- [x] 3.1 Add scan tests for instruction files: `test_scan_agent_files_finds_windsurf_rules`, `test_scan_agent_files_finds_opencode_instructions`, `test_scan_agent_files_finds_amp_instructions`. *(Spec: Scan Detects Missing Instruction Files scenarios)*
- [x] 3.2 Add scan tests for all 8 skill variants: `test_scan_agent_files_finds_{agent}_skills` (cursor, codex, gemini, opencode, roo, factory, vibe, antigravity). Include empty-dir-ignored test. *(Spec: Scan Detects Agent Skill Directories scenarios)*
- [x] 3.3 Add scan tests for 3 command variants: `test_scan_agent_files_finds_{agent}_commands` (claude, gemini, opencode). Include empty-dir-ignored test. *(Spec: Scan Detects Command Directories scenarios)*
- [x] 3.4 Add scan tests for MCP config variants: `test_scan_agent_files_finds_{agent}_mcp_config` for all 10 variants. *(Spec: Scan Detects Agent-Specific MCP Configs scenarios)*
- [x] 3.5 Add `DEFAULT_CONFIG` tests: verify `gemini` and `opencode` keys exist, verify `claude.targets.commands` exists with correct source/destination/type, verify TOML still parses into `Config`. *(Spec: DEFAULT_CONFIG scenarios)*
- [x] 3.6 Run `cargo test --all-features` and `cargo clippy --all-targets --all-features -- -D warnings` to verify no regressions. *(Spec: Acceptance Criteria #12)*
