# Design: Wizard Preserve Existing Skill Links

## Technical Approach

Keep the change centered on one shared skills-layout diagnosis that can be reused by the init wizard, post-init validation, doctor, and an optional status hint. The wizard remains the main place where users choose skills behavior, but it becomes explicit: only generated `skills` targets get a compact mode prompt, defaulting to `symlink` and preserving an already-correct destination directory symlink when one is detected.

Implementation stays within the current contract: `type = "symlink"` remains the default for skills, `type = "symlink-contents"` remains available, and `apply` semantics do not change. The new logic only detects when config mode and on-disk shape disagree and surfaces that early.

## Architecture Decisions

### Decision: Reuse one skills-layout detector across init and diagnostics

**Choice**: Add a small shared library helper that inspects a skills target destination and classifies only confidently recognized layouts, especially “destination is already a directory symlink to the expected source”.

**Alternatives considered**: Duplicating path checks inside `init.rs`, `doctor.rs`, and `status.rs`; teaching `Linker` to silently absorb mismatches.

**Rationale**: A single detector keeps wizard recommendations, validation output, and diagnostics consistent while avoiding broader sync-engine changes.

### Decision: Keep wizard explicit, but only for selected skills targets

**Choice**: Prompt once per generated `skills` target that is actually relevant to the current wizard run, using a compact two-option choice with a recommended default.

**Alternatives considered**: No prompt and rely only on defaults; global one-size-fits-all prompt for every agent; extra CLI flags.

**Rationale**: This makes skills mode visible without turning the wizard into a noisy questionnaire, and it matches the proposal’s “explicit but minimal” goal.

### Decision: Preserve the commented template by parameterizing rendering, not by serializing `Config`

**Choice**: Replace the direct `DEFAULT_CONFIG` write in wizard generation with a renderer that starts from the existing template and swaps only the per-agent skills `type` lines that the user chose.

**Alternatives considered**: Deriving `Serialize` for config structs and regenerating the entire file via `toml::to_string`; brittle ad-hoc string replacement across the whole template.

**Rationale**: The repository already treats `DEFAULT_CONFIG` as a curated, commented starter file. A targeted renderer preserves those comments and ordering while allowing only the few wizard-selected skills modes to vary.

### Decision: Treat mode-only mismatch as a warning/hint, not a broken-link error

**Choice**: `doctor` reports the mismatch as a warning that explains why `apply` can churn; `status` may emit a hint under an otherwise `OK` entry, without changing JSON shape or exit status.

**Alternatives considered**: Making `status` fail for a correct directory symlink; ignoring the mismatch until `apply` runs.

**Rationale**: The destination still points at the right source, so the link is healthy. The problem is semantic drift between config and layout, so warning-level output is the right signal.

## Data Flow

### Wizard planning and validation

```text
scan_agent_files()
  -> selected files to migrate
  -> relevant skills targets
  -> inspect existing destination layout against expected .agents/skills source
  -> prompt with recommended mode
  -> render config with chosen mode(s)
  -> validate written config against current on-disk layout
  -> print summary / warnings before backup prompt exits
```

### Shared mismatch detection

```text
target.destination
    |
    v
resolve expected source (.agents/skills)
    |
    v
read symlink_metadata(destination)
    |
    +--> destination is dir symlink resolving to expected source
    |         -> recognized layout: directory-symlink-to-source
    |
    +--> anything else
              -> unrecognized / no mismatch diagnosis
```

### Sequence

```text
Wizard/Doctor/Status -> shared skills-layout helper -> filesystem metadata/read_link
Wizard/Doctor/Status <- layout classification        <- resolved source comparison
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/init.rs` | Modify | Add per-skills-target mode planning, compact wizard prompt text, template rendering with chosen skills modes, and post-init validation summary. |
| `src/commands/doctor.rs` | Modify | Reuse shared skills-layout detection and print warning-level mismatch guidance. |
| `src/commands/status.rs` | Modify | Optionally print a hint for recognized mode-only mismatches while keeping normal success/problem semantics unchanged. |
| `src/lib.rs` | Modify | Register the shared helper module. |
| `src/skills_layout.rs` | Create | Shared recognition logic for “directory symlink already points at expected skills source” plus message formatting helpers. |
| `src/commands/doctor_tests.rs` | Modify | Add warning coverage for recognized mode-only mismatch and non-mismatch cases. |
| `src/commands/status_tests.rs` | Modify | Add hint-only coverage for recognized mismatch without making the entry problematic. |
| `src/init.rs` tests | Modify | Add pure-function coverage for skills-mode recommendation, config rendering, and post-init validation messaging. |
| `tests/test_agent_adoption.rs` | Modify | Add regression coverage for preserving an existing skills directory symlink layout during adoption/migration. |
| `website/docs/src/content/docs/reference/configuration.mdx` | Modify | Update skills examples and target-type guidance to show `symlink` as the default/recommended skills mode. |
| `website/docs/src/content/docs/reference/cli.mdx` | Modify | Document the explicit wizard choice, validation summary, doctor warning, and optional status hint behavior. |
| `website/docs/src/content/docs/guides/skills.mdx` | Modify | Update migration guidance, preservation behavior, and when `symlink-contents` is still appropriate. |

## Interfaces / Contracts

The shared helper should stay small and deterministic, for example:

```rust
pub enum SkillsLayoutMatch {
    DirectorySymlinkToExpectedSource,
}

pub struct SkillsModeMismatch {
    pub agent_name: String,
    pub target_name: String,
    pub destination: PathBuf,
    pub configured_mode: SyncType,
    pub detected_layout: SkillsLayoutMatch,
}

pub fn detect_skills_mode_mismatch(
    project_root: &Path,
    config_path: &Path,
    agent_name: &str,
    target_name: &str,
    target: &TargetConfig,
) -> Option<SkillsModeMismatch>;
```

Planned behavior:

- Only inspect targets that are semantically “skills” targets (`target_name == "skills"` and `target.source == "skills"`).
- Only diagnose shapes that can be recognized with high confidence.
- Initial mismatch rule: config says `SymlinkContents`, but destination is already a directory symlink resolving to the expected source directory.
- Message text should recommend changing `type` to `"symlink"` or rerunning `agentsync init --wizard`.

Wizard-specific helpers in `src/init.rs` should operate on pure data so tests do not need interactive terminal coverage:

```rust
struct SkillsWizardChoice {
    agent_name: &'static str,
    destination: &'static str,
    recommended_mode: SyncType,
    reason: Option<String>,
}

fn build_default_config_with_skills_modes(modes: &BTreeMap<&str, SyncType>) -> String;
fn collect_post_init_skills_warnings(...) -> Vec<String>;
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | Shared layout detection recognizes an existing directory symlink to the expected `.agents/skills` source and ignores unrelated layouts. | Add focused tests for path resolution and mismatch classification in the new helper / doctor tests. |
| Unit | Wizard recommendation logic stays quiet unless a relevant skills target is present and defaults to `symlink` when no recognized layout exists. | Extract recommendation/planning helpers from interactive code and test them directly in `src/init.rs` tests. |
| Unit | Config rendering changes only the intended skills target `type` lines. | Snapshot-like string assertions on the rendered default template in `src/init.rs` tests. |
| Integration | Post-init validation emits the expected warning when config says `symlink-contents` but destination is already a directory symlink to `.agents/skills`. | Cover through pure validation helpers and adoption-style filesystem fixtures. |
| Integration | `doctor` reports the mode-only mismatch with remediation guidance and does not warn for already-matching `symlink` configs. | Extend `src/commands/doctor_tests.rs`. |
| Integration | `status` keeps recognized mismatch entries non-problematic while surfacing a human hint if implemented. | Extend `src/commands/status_tests.rs`; keep JSON/problem logic unchanged. |
| Regression | Existing adoption flows for Claude/Gemini/Codex still produce directory symlinks for skills and `symlink-contents` for commands. | Extend `tests/test_agent_adoption.rs` with an existing-link preservation fixture instead of changing current assertions. |

Migration/regression cases that must be covered explicitly:

- Existing `.claude/skills -> ../.agents/skills` symlink + wizard-generated skills target preserves `type = "symlink"` by default.
- Existing config with `type = "symlink-contents"` + directory symlink destination triggers post-init validation and `doctor` warning.
- Healthy `type = "symlink"` + directory symlink destination remains clean.
- Repos without selected skills targets do not receive extra skills prompts.

## Migration / Rollout

No migration required.

Existing configs continue to work unchanged. The rollout effect is diagnostic-only for existing mismatches and wizard-only for new/adoption flows.

## Open Questions

- [ ] Whether the shared helper should live in a new `src/skills_layout.rs` module or inside an existing module like `init`/`doctor`; new module is preferred for reuse, but implementation can stay local if that proves smaller.
- [ ] Whether the `status` hint should be human-output only (preferred, to avoid JSON schema churn) or also added to JSON in a backward-compatible way.
