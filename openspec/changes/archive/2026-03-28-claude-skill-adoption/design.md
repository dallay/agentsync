# Design: Claude Skill Adoption

## Technical Approach

Extend the existing init/wizard/apply pipeline to treat Claude skills as a first-class concern. The
change adds a Claude skills target to the default config template, teaches the wizard scanner to
detect `.claude/skills/`, adds a skill-directory migration category in the wizard that copies skill
subdirectories into `.agents/skills/` with collision handling, and adds an unmanaged-skills
diagnostic to the `doctor` command.

This maps directly to the proposal's four deliverables:

1. `DEFAULT_CONFIG` template update (immediate benefit for new projects)
2. `AgentFileType::ClaudeSkills` + scanner extension (detection)
3. Wizard migration with collision handling (adoption)
4. Doctor diagnostic for unmanaged skills (ongoing guidance)

## Architecture Decisions

### Decision: Diagnostic in `doctor` command, not in `apply`

**Choice**: Add the "unmanaged `.claude/skills/`" warning to `src/commands/doctor.rs`, not to the
`apply` flow in `main.rs`.
**Alternatives considered**: Adding the check inline in `main.rs` after `linker.sync()` returns;
adding it to `Linker::sync()` itself.
**Rationale**: The `doctor` command already has the infrastructure for iterating agents, checking
paths, and reporting issues with severity levels. The apply flow is focused on symlink creation and
should remain fast and deterministic. A warning in `apply` would fire on every run, which could
become noise for users who intentionally manage `.claude/skills/` outside agentsync. The `doctor`
pattern lets users run diagnostics on-demand. The proposal says "apply (or status)" — `doctor`
serves this role better than either.

### Decision: Static template update, not dynamic config generation

**Choice**: Add the `[agents.claude.targets.skills]` block directly to the `DEFAULT_CONFIG` string
constant.
**Alternatives considered**: Dynamic config builder that generates TOML based on discovered files;
template with conditional sections.
**Rationale**: The existing codebase uses a single static `DEFAULT_CONFIG` constant written verbatim
to disk (init.rs:173, init.rs:829). Dynamic generation would be a larger architectural change that
isn't justified by adding one target. The static approach is zero-risk, matches the existing
pattern (Codex already has a skills target at init.rs:96-99), and benefits both `init` and
`init --wizard` paths. The `!force` guard (init.rs:166, init.rs:822) ensures existing configs are
never overwritten.

### Decision: New skill-directory migration category in wizard

**Choice**: Add a third migration category alongside "instruction files" and "directories" — a "
skill-directory" category that copies individual skill subdirectories into `.agents/skills/` rather
than copying the parent directory as-is.
**Alternatives considered**: Reusing the existing directory copy logic (which would copy
`.claude/skills/` → `.agents/.claude/skills/`); copying the entire `.claude/skills/` directory to
`.agents/skills/`.
**Rationale**: The existing directory migration copies to `.agents/<original-path>` (e.g.,
`.cursor` → `.agents/.cursor`). For skills, we want to merge into the shared `.agents/skills/`
directory because that's where agentsync's symlink-contents target reads from. Copying individual
subdirectories (e.g., `.claude/skills/my-skill/` → `.agents/skills/my-skill/`) enables per-skill
collision detection and matches the skill install system's convention of one directory per skill.

### Decision: Skip-on-collision strategy

**Choice**: When a skill subdirectory already exists in `.agents/skills/`, skip it and print a
warning. Do not overwrite.
**Alternatives considered**: Prompt user per-collision; overwrite with backup; rename with suffix.
**Rationale**: The wizard already follows a non-destructive pattern (backup is offered after
migration, not inline). Skip-and-warn is the safest default — it preserves existing skills that may
have been installed via `skill install` or copied from another agent. The user can manually resolve
conflicts. This matches the existing `!force` guard pattern.

### Decision: Doctor check uses config-aware target scanning

**Choice**: The doctor diagnostic checks whether any enabled target's destination covers
`.claude/skills/`, rather than hardcoding a check.
**Alternatives considered**: Hardcoding `.claude/skills/` path check; checking only the `claude`
agent.
**Rationale**: Users might name their agent section `claude-code` or have a custom target that maps
to `.claude/skills/`. By iterating all enabled agents' targets and checking if any destination
matches `.claude/skills`, the diagnostic works regardless of naming conventions. This also avoids
false positives if the user already has a properly configured skills target.

## Data Flow

### Init Flow (Deliverable 1)

```
init(project_root, force)
  │
  ├─ Create .agents/, .agents/skills/
  ├─ Write DEFAULT_CONFIG (now includes [agents.claude.targets.skills])
  └─ Write AGENTS.md
```

No behavioral change — just the template content changes.

### Wizard Detection + Migration Flow (Deliverables 2 & 3)

```
init_wizard(project_root, force)
  │
  ├─ scan_agent_files(project_root)
  │    ├─ Existing checks (CLAUDE.md, .cursor/, .mcp.json, etc.)
  │    └─ NEW: Check .claude/skills/ exists && is_dir && has children
  │         └─ Push DiscoveredFile { path: ".claude/skills", type: ClaudeSkills }
  │
  ├─ User selects files to migrate (multi-select)
  │
  ├─ Migration loop (for each selected file):
  │    ├─ Instruction files → merge into AGENTS.md (existing)
  │    ├─ Directories → copy to .agents/<path> (existing)
  │    └─ NEW: ClaudeSkills → copy each child dir to .agents/skills/<name>
  │         │
  │         ├─ For each subdirectory in .claude/skills/:
  │         │    ├─ dest = .agents/skills/<subdir-name>
  │         │    ├─ IF dest exists:
  │         │    │    └─ SKIP + print "⚠ Skipped: skill <name> already exists"
  │         │    └─ ELSE:
  │         │         └─ copy_dir_all(src, dest) + print "✔ Copied"
  │         │
  │         └─ Increment files_actually_migrated per successful copy
  │
  ├─ Write DEFAULT_CONFIG (includes skills target) or skip if exists
  └─ Offer backup of originals
```

### Doctor Diagnostic Flow (Deliverable 4)

```
run_doctor(project_root)
  │
  ├─ Load config, create Linker (existing)
  ├─ Existing checks (sources, conflicts, MCP, gitignore)
  │
  └─ NEW: Unmanaged skill directory check
       │
       ├─ claude_skills_dir = project_root.join(".claude/skills")
       ├─ IF !claude_skills_dir.exists() || !claude_skills_dir.is_dir():
       │    └─ Skip (nothing to warn about)
       │
       ├─ IF directory is empty (no children):
       │    └─ Skip
       │
       ├─ Check if any enabled target maps destination to ".claude/skills":
       │    ├─ For each (agent, target) in config.agents:
       │    │    └─ IF target.destination == ".claude/skills"
       │    │         && target.sync_type == SymlinkContents
       │    │         && agent.enabled:
       │    │              └─ managed = true; break
       │    │
       │    ├─ IF managed: Skip (already handled)
       │    └─ IF !managed:
       │         └─ Print "⚠ .claude/skills/ has content but is not managed
       │                    by any target. Run 'agentsync init --wizard' to adopt."
       │         └─ issues += 1
       └─ End
```

## File Changes

| File                                      | Action | Description                                                                                                |
|-------------------------------------------|--------|------------------------------------------------------------------------------------------------------------|
| `src/init.rs` — `DEFAULT_CONFIG`          | Modify | Add `[agents.claude.targets.skills]` block after the existing `[agents.claude.targets.instructions]` block |
| `src/init.rs` — `AgentFileType` enum      | Modify | Add `ClaudeSkills` variant                                                                                 |
| `src/init.rs` — `scan_agent_files()`      | Modify | Add detection for `.claude/skills/` directory (with non-empty check)                                       |
| `src/init.rs` — wizard migration match    | Modify | Add `AgentFileType::ClaudeSkills` arm with per-subdirectory copy + collision handling                      |
| `src/init.rs` — wizard backup match       | Modify | Include `ClaudeSkills` in the set of file types eligible for backup                                        |
| `src/commands/doctor.rs` — `run_doctor()` | Modify | Add unmanaged `.claude/skills/` diagnostic check after existing checks                                     |
| `src/commands/doctor_tests.rs`            | Modify | Add tests for the new diagnostic                                                                           |
| `tests/` (integration)                    | Modify | Add tests for scan detection, migration, template parsing                                                  |

## Interfaces / Contracts

### New `AgentFileType` Variant

```rust
#[derive(Debug, Clone, PartialEq)]
enum AgentFileType {
    // ... existing variants ...
    
    // Skill directories (contents merged into .agents/skills/ on migration)
    ClaudeSkills,
    
    // ... existing variants ...
}
```

### New `DEFAULT_CONFIG` Section

The following block is inserted after `[agents.claude.targets.instructions]` (after line 68):

```toml
[agents.claude.targets.skills]
source = "skills"
destination = ".claude/skills"
type = "symlink-contents"
```

### Wizard Migration Match Arm

```rust
// Skill directories — copy contents into .agents/skills/
AgentFileType::ClaudeSkills => {
    if src_path.exists() && src_path.is_dir() {
        for entry in fs::read_dir(&src_path)? {
            let entry = entry?;
            let entry_path = entry.path();
            if !entry_path.is_dir() {
                continue; // Skip non-directory entries (skills are directories)
            }
            let skill_name = entry.file_name();
            let dest_skill = skills_dir.join(&skill_name);
            if dest_skill.exists() {
                println!(
                    "  {} Skipped: skill '{}' already exists in .agents/skills/",
                    "⚠".yellow(),
                    skill_name.to_string_lossy()
                );
                files_skipped += 1;
            } else {
                copy_dir_all(&entry_path, &dest_skill)?;
                println!(
                    "  {} Copied skill: {} → .agents/skills/{}",
                    "✔".green(),
                    entry_path.display(),
                    skill_name.to_string_lossy()
                );
                files_actually_migrated += 1;
            }
        }
    }
}
```

### Doctor Diagnostic Function

```rust
/// Check if .claude/skills/ has content but is not managed by any target.
fn check_unmanaged_claude_skills(
    project_root: &Path,
    config: &agentsync::config::Config,
) -> Option<String> {
    let claude_skills = project_root.join(".claude").join("skills");
    if !claude_skills.exists() || !claude_skills.is_dir() {
        return None;
    }
    
    // Check if directory has any children
    let has_content = fs::read_dir(&claude_skills)
        .ok()
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);
    if !has_content {
        return None;
    }
    
    // Check if any enabled target manages .claude/skills
    let is_managed = config.agents.values().any(|agent| {
        agent.enabled
            && agent.targets.values().any(|target| {
                target.destination == ".claude/skills"
                    && target.sync_type == agentsync::config::SyncType::SymlinkContents
            })
    });
    
    if is_managed {
        None
    } else {
        Some(
            ".claude/skills/ has content but is not managed by any target. \
             Run 'agentsync init --wizard' to adopt."
                .to_string(),
        )
    }
}
```

## Testing Strategy

| Layer       | What to Test                                         | Approach                                                                                                                                                |
|-------------|------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------|
| Unit        | `DEFAULT_CONFIG` parses with new skills target       | Extend `test_default_config_contains_expected_agents` to assert `config.agents["claude"].targets["skills"]` exists with correct source/destination/type |
| Unit        | `scan_agent_files()` detects `.claude/skills/`       | Create temp dir with `.claude/skills/my-skill/SKILL.md`, assert `ClaudeSkills` discovered                                                               |
| Unit        | `scan_agent_files()` ignores empty `.claude/skills/` | Create temp dir with empty `.claude/skills/`, assert not discovered                                                                                     |
| Unit        | Wizard migration copies skills                       | Set up `.claude/skills/a/`, `.claude/skills/b/`, run migration logic, assert both copied to `.agents/skills/`                                           |
| Unit        | Wizard migration skips collisions                    | Set up `.claude/skills/a/` and pre-existing `.agents/skills/a/`, run migration, assert original preserved                                               |
| Unit        | Doctor detects unmanaged `.claude/skills/`           | Create `.claude/skills/foo/`, load config without skills target, assert warning returned                                                                |
| Unit        | Doctor suppresses warning when target exists         | Load config with `[agents.claude.targets.skills]`, assert no warning                                                                                    |
| Integration | Full wizard flow with Claude skills                  | Create project with `.claude/skills/`, run `scan_agent_files`, verify detection; simulate migration; verify `.agents/skills/` populated                 |

## Migration / Rollout

No data migration required. All changes are additive:

- **New projects** (`agentsync init`): Get the skills target automatically via `DEFAULT_CONFIG`.
  Zero user action needed.
- **Existing projects**: Config is not modified (protected by `!force` guard). Users can either:
    1. Run `agentsync init --wizard` to detect and adopt Claude skills
    2. Manually add `[agents.claude.targets.skills]` to their `agentsync.toml`
    3. Run `agentsync doctor` to see if they have unmanaged skills
- **Backward compatibility**: The new `AgentFileType::ClaudeSkills` variant only affects the wizard
  path. The `#[allow(dead_code)]` annotation on the enum already accommodates future variants.
  Existing configs and workflows are completely unaffected.

## Open Questions

- [x] ~~Whether to include `.claude/commands/` in scope~~ — **Out of scope per proposal** (follow-up
  change)
- [x] ~~Where to put the diagnostic~~ — **`doctor` command** (decided above)
- [ ] Should `scan_agent_files()` require `.claude/skills/` to be non-empty before reporting it as
  discovered? A completely empty directory is likely a leftover from a previous `agentsync apply`
  and not user-created content worth migrating. **Recommendation: yes, require at least one child
  entry.**
