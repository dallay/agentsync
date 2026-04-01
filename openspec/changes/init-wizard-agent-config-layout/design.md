# Design: Init Wizard Agent Config Layout

## Technical Approach

Add a wizard-only renderer in `src/init.rs` that builds a managed `## Agent config layout` section for `.agents/AGENTS.md` from the same final config that `agentsync init --wizard` is about to write. The renderer will summarize canonical instruction, skills, and commands targets for the default wizard layout, reflect the selected skills sync mode per agent, and inject the section near the top of the generated AGENTS content with explicit markers so forced reruns can replace the block idempotently.

This stays within the approved v1 scope:

- only `init --wizard` writes the layout section;
- existing `.agents/AGENTS.md` remains untouched when it already exists and `--force` is not used;
- `agentsync apply` is unchanged and remains explicitly deferred.

The design is based on the current wizard flow in `src/init.rs`, the main `skill-adoption` spec, and the approved proposal. No delta specs exist yet for this change, so this design intentionally scopes itself to the approved proposal plus current main-spec behavior.

## Architecture Decisions

### Decision: Derive the explainer from the final rendered wizard config

**Choice**: Generate layout prose from the exact config text produced by `build_default_config_with_skills_modes()` (or an equivalent parsed `Config` derived from that output) rather than from hardcoded markdown.

**Alternatives considered**:
- Hardcode a static section inside `DEFAULT_AGENTS_MD`.
- Derive content directly from scan results alone.

**Rationale**: The final rendered config already captures the only wizard-time variability relevant to this change: enabled default targets and per-agent skills sync mode. Using that rendered config keeps prose aligned with actual destinations and sync semantics, especially for `symlink` versus `symlink-contents` skills targets.

### Decision: Keep layout generation in `src/init.rs` for v1

**Choice**: Implement renderer and block-insertion helpers in `src/init.rs`, near the existing wizard AGENTS/config generation path.

**Alternatives considered**:
- Add shared AGENTS block generation to `src/config.rs`.
- Generalize a reusable AGENTS block manager for both wizard and `apply`.

**Rationale**: The feature is explicitly wizard-only in v1. `src/init.rs` already owns `DEFAULT_AGENTS_MD`, merged migrated content, `skills_modes`, and overwrite semantics. Keeping the new logic co-located minimizes scope and avoids accidentally expanding behavior into `apply`.

### Decision: Use unique HTML comment markers around the managed block

**Choice**: Wrap the generated section in a dedicated marker pair such as:

```md
<!-- agentsync:agent-config-layout:start -->
...
<!-- agentsync:agent-config-layout:end -->
```

**Alternatives considered**:
- No markers; always prepend fresh content.
- Reuse `.gitignore`-style marker text.
- Track the section structurally by heading text alone.

**Rationale**: HTML comment markers are invisible in normal rendering, unlikely to collide with user prose, and simple to replace idempotently during `--force` rewrites. Heading-only detection is brittle, and blind prepending would duplicate content.

### Decision: Insert the layout section after the file's opening title/introduction, before migrated body content

**Choice**: Treat the layout section as a top-of-file explainer but place it after the opening document header block when one exists.

**Alternatives considered**:
- Prepend at byte 0.
- Append at end of file.
- Insert only in the default template, not in migrated content.

**Rationale**: Current wizard output may be either the default AGENTS template or migrated instruction content that already starts with an H1. Byte-0 prepending risks awkward double-H1 structure. Appending hides the explainer. Inserting after the leading title/introduction keeps the section prominent while still leaving migrated content below it.

## Data Flow

### Sequence

```text
scan_agent_files()
    -> user selects files to migrate
    -> build_skills_wizard_choices()
    -> user selects per-agent skills modes
    -> build_default_config_with_skills_modes()
    -> parse rendered config / inspect final target set
    -> render_agent_config_layout_section()
    -> merge layout block into AGENTS content
    -> write .agents/AGENTS.md (only if wizard would already write it)
```

### Detailed flow

1. `init_wizard()` gathers `skills_modes` from the explicit wizard prompts already implemented in `build_skills_wizard_choices()` and `resolve_skills_mode_selection()`.
2. The wizard renders final config text with `build_default_config_with_skills_modes(&skills_modes)`.
3. The new design adds a helper to derive a layout summary from that rendered config:
   - include only enabled default targets the wizard can explain safely;
   - instructions: summarize `source = "AGENTS.md"` + `type = "symlink"` destinations;
   - skills: summarize `source = "skills"` destinations and chosen sync mode per agent;
   - commands: summarize `source = "commands"` + `type = "symlink-contents"` destinations.
4. The helper renders a managed markdown section that:
   - states `.agents/` is canonical;
   - lists downstream instruction destinations;
   - describes skills semantics accurately per selected mode;
   - explains that commands are linked per-entry and therefore reconciled on future `agentsync apply` runs;
   - preserves the intentional OpenCode singular destination `.opencode/command`.
5. The wizard merges that managed block into the AGENTS body it already plans to write:
   - if migrated instruction content exists, inject the block near the top of that content;
   - otherwise inject it into `DEFAULT_AGENTS_MD`.
6. Write behavior stays gated by existing overwrite rules:
   - if `.agents/AGENTS.md` exists and `force == false`, do not mutate it at all;
   - if the wizard is writing the file (`!exists || force`), replace any existing marked block and write the updated content.

### Injection algorithm

```text
base_content
    -> strip/replace existing managed block if present
    -> find insertion point after opening header block
    -> insert managed block + blank-line normalization
    -> return final AGENTS.md text
```

Suggested header-block heuristic for v1:

- if content starts with an H1, keep that H1 first;
- also keep any immediately following blank lines and opening blockquote/introduction paragraph attached to that H1;
- insert `## Agent config layout` after that opening block;
- leave all migrated/default body content below unchanged.

This places the explainer above migrated body sections without replacing the document's original title.

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/init.rs` | Modify | Add layout-section rendering, marker replacement/insertion helpers, and wire them into the wizard AGENTS write path. |
| `src/init.rs` | Modify | Extend unit tests for rendering accuracy, insertion placement, idempotent replacement, and preserve-without-force behavior. |
| `tests/test_agent_adoption.rs` | Optional modify | Add a higher-level coverage case only if a non-interactive helper path is worth exercising from outside `src/init.rs`; otherwise keep coverage local to `src/init.rs`. |
| `openspec/specs/skill-adoption/spec.md` | Deferred follow-up | Main spec likely needs new scenarios for the managed layout section, but that is not part of this design artifact itself. |

## Interfaces / Contracts

No public CLI contract changes are planned. The change is internal to wizard-time AGENTS rendering.

Likely new private helpers in `src/init.rs`:

```rust
struct AgentLayoutFacts {
    instructions: Vec<InstructionTargetLayout>,
    skills: Vec<SkillsTargetLayout>,
    commands: Vec<CommandTargetLayout>,
}

fn build_wizard_layout_facts(rendered_config: &str) -> Result<AgentLayoutFacts>;

fn render_agent_config_layout_section(facts: &AgentLayoutFacts) -> String;

fn upsert_agent_config_layout_block(base_content: &str, layout_block: &str) -> String;

fn find_agents_layout_insertion_offset(content: &str) -> usize;
```

Representative rendering contract:

- `build_wizard_layout_facts` only includes targets matching the canonical wizard-owned cases:
  - instructions sourced from `AGENTS.md`;
  - skills sourced from `skills`;
  - commands sourced from `commands`.
- targets using other sync types (`nested-glob`, `module-map`) or non-default custom sources are omitted rather than guessed about.
- skills copy uses agent-specific sync mode language:
  - `symlink`: downstream directory reflects `.agents/skills` immediately;
  - `symlink-contents`: downstream directory is populated per item when `agentsync apply` runs.

## Marker Strategy / Idempotency

- Markers are wizard-specific and local to `.agents/AGENTS.md`.
- When force-writing AGENTS, insertion first removes any existing managed layout block delimited by the marker pair, then inserts a fresh block once.
- If the file contains no markers, insertion adds the block once at the computed placement offset.
- If `.agents/AGENTS.md` exists and `--force` is not used, the wizard does not attempt partial replacement; it preserves the file byte-for-byte.

This yields three stable outcomes:

1. **Fresh wizard write**: block added once.
2. **Forced wizard rerun**: existing block replaced, not duplicated.
3. **Existing file without force**: no change at all.

## Heading / Placement Strategy

- The managed section heading should be `## Agent config layout`.
- The file's existing top-level title remains the document H1.
- For default template output, the block lands after:
  - `# AI Agent Instructions`
  - the existing introductory blockquote
- For migrated content with a leading H1 (for example merged `# Instructions from ...`), the block lands immediately after that opening H1/introduction region.
- For content without a recognizable opening heading, the block is prepended at the start as a fallback.

This satisfies the “near the top” expectation while avoiding a second H1 above user-migrated content.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | Layout facts generation from rendered wizard config | Add `src/init.rs` tests that parse rendered config for default and overridden skills modes and assert listed destinations/sync-language are correct. |
| Unit | Managed block rendering | Assert the rendered section includes marker comments, `## Agent config layout`, canonical-source wording, `.opencode/command`, and mode-specific wording for `symlink` vs `symlink-contents`. |
| Unit | Placement/insertion | Feed default-template content and migrated-content fixtures into insertion helper; assert block appears after the opening title/introduction and before the main migrated body. |
| Unit | Idempotent replacement | Start from content that already contains the managed block; rerender and assert exactly one marker pair remains and content is refreshed in place. |
| Unit | Preserve without force | Extend wizard/AGENTS tests in `src/init.rs` to assert an existing `.agents/AGENTS.md` is unchanged when `force` is false, even if the new layout feature exists. |
| Integration | Wizard-adoption behavior around AGENTS ownership | Optional higher-level test if a non-interactive helper can be exercised; otherwise rely on focused `src/init.rs` coverage because the wizard itself is interactive. |

## Migration / Rollout

No migration required.

This is a wizard-only write-path enhancement. Existing projects are unaffected unless they rerun `agentsync init --wizard` and allow AGENTS regeneration. Existing `.agents/AGENTS.md` files remain unchanged unless `--force` is used.

## Limitations and Deferred Items

- `agentsync apply` does not own or refresh the layout block in v1.
- The renderer intentionally explains only default wizard-owned instructions/skills/commands targets; it does not attempt to document arbitrary custom target layouts.
- Later manual edits to `.agents/agentsync.toml` can make the layout explainer stale until a future change extends regeneration ownership beyond the wizard.
- Non-default `source_dir` or future target types are out of scope for this change.
- Main spec updates for this exact layout section are still needed as a follow-up to keep OpenSpec artifacts fully aligned.

## Open Questions

- [ ] Should the layout section mention disabled default agents explicitly, or only list enabled downstream targets?
- [ ] Should forced reruns preserve any user edits made outside the managed block when the wizard rewrites migrated content, or is full-file rewrite acceptable under current `--force` semantics?
- [ ] Should the wording mention the root `AGENTS.md` target explicitly as a downstream symlink destination, or keep focus on agent-specific files only?
