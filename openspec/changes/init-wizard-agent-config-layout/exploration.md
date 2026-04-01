## Exploration: init-wizard-agent-config-layout

### Current State

`src/init.rs` owns both the default `AGENTS.md` template (`DEFAULT_AGENTS_MD`) and the `init_wizard()` flow that migrates instructions, skills, and commands into `.agents/`. Today the wizard either writes merged instruction content directly into `.agents/AGENTS.md` or falls back to the plain default template; it does not add any generated layout explainer.

The generated config already contains enough structure for a wizard-default explainer: `DEFAULT_CONFIG` defines canonical sources (`AGENTS.md`, `skills`, `commands`), destinations per agent, and sync type semantics. Wizard-only variation currently affects skills via `build_default_config_with_skills_modes()`, while instructions and commands stay fixed to `symlink` and `symlink-contents` respectively.

Current overwrite semantics are important: if `.agents/AGENTS.md` already exists and `--force` is not used, `init_wizard()` preserves it and does not offer backup. That behavior already matches the requested v1 constraint and should remain unchanged.

The main gap is discoverability for fresh agents: nothing in the generated file says `.agents/` is canonical, which downstream files are symlinks, or why skills propagate differently from commands. The issue body asks for config-derived content and hints at marker-based idempotence, but no AGENTS-specific managed marker exists today.

### Affected Areas

- `src/init.rs` — owns `DEFAULT_AGENTS_MD`, wizard migration, config rendering, and the only safe v1 insertion point.
- `src/config.rs` — provides the parsed model (`Config`, `TargetConfig`, `SyncType`, `source_dir()`) needed if generated text is derived from config rather than hardcoded paths.
- `openspec/specs/skill-adoption/spec.md` — already defines wizard canonical-source behavior and is the closest existing spec domain for new wizard/AGENTS requirements.
- `tests` in `src/init.rs` (and possibly `tests/test_agent_adoption.rs`) — current coverage checks default AGENTS sections and wizard behavior, but not generated layout content, placement, or preservation.

### Approaches

1. **Hardcode the layout block in the default template only** — add a static section to `DEFAULT_AGENTS_MD` and maybe prepend it to migrated content.
   - Pros: Smallest code change.
   - Cons: Drifts from actual generated config, misses wizard-selected skills mode, and is brittle for future agent/target changes.
   - Effort: Low

2. **Render a wizard-only managed layout block from the generated config** — after wizard builds the final config text, derive an "Agent config layout" section from enabled targets and inject/update it inside `.agents/AGENTS.md` only when the wizard is writing that file.
   - Pros: Matches requested v1 scope, stays accurate for wizard-default output, can reflect selected skills mode, and avoids touching `apply`.
   - Cons: Needs clear placement rules and managed-marker rules for idempotence.
   - Effort: Medium

3. **Generalize layout generation for both wizard and apply** — create shared rendering/injection logic and invoke it from wizard now and apply later.
   - Pros: Best long-term model; one renderer for all entry points.
   - Cons: Broader than the requested safe scope and risks changing apply semantics in v1.
   - Effort: Medium/High

### Recommendation

Use **Approach 2** with a narrow contract under change slug `init-wizard-agent-config-layout`:

1. Keep the feature **wizard-only** in v1. Generate the layout section during `init --wizard` only, after the final config content is known.
2. Derive the section from the same config the wizard writes, not from unrelated repo state. For v1, that means:
   - instructions: list enabled `symlink` targets whose source is the canonical AGENTS source;
   - skills: describe the selected sync mode per generated skills target, emphasizing "instant" propagation only for directory-symlink targets;
   - commands: describe configured command destinations and the need to rerun `agentsync apply` for `symlink-contents` targets.
3. Insert the content as a **managed subsection** inside `.agents/AGENTS.md` with explicit begin/end markers so future wizard reruns with `--force` can replace just that block instead of duplicating it.
4. Preserve existing semantics when `.agents/AGENTS.md` already exists and `--force` is false: do not inject, rewrite, or partially mutate the file.
5. Place the generated section after the file's primary title/opening context rather than blindly before byte 0; this avoids the double-H1 confusion called out in the issue while still making the section prominent.

This keeps scope aligned with the issue and prior orchestration guidance, while leaving room for a later follow-up to reuse the same renderer in `apply`.

### Risks

- **Header placement ambiguity**: the issue says “at the top,” but current wizard output may be migrated user content with its own H1. Blind prepending can create awkward or misleading structure.
- **Custom `source_dir` drift**: the wizard writes `source_dir = "."`, so deriving from generated config is accurate for the default path; later manual edits to `source_dir` can make the explainer stale unless apply eventually owns regeneration.
- **Custom target semantics**: future or user-edited targets (`nested-glob`, `module-map`, nonstandard command sources, disabled agents) should not be overexplained by v1. The renderer should scope itself to the canonical instructions/skills/commands cases it can describe safely.
- **Marker interaction**: AGENTS-specific markers do not exist today. The implementation must define markers that are unique, idempotent, and unlikely to collide with user-authored content.
- **Skills asymmetry**: the issue text assumes skills are "instant" everywhere, but the wizard can emit `symlink-contents` for skills. Generated prose must reflect the chosen mode instead of hardcoding that claim.
- **OpenCode path confusion**: `.opencode/command` is intentionally singular in `DEFAULT_CONFIG`; the generated text should preserve that exact path and likely annotate that it is intentional.

### Ready for Proposal

Yes — proposed next change: `init-wizard-agent-config-layout`. Keep it scoped to wizard-time generation of a managed AGENTS layout section, preservation of existing `.agents/AGENTS.md` without `--force`, and tests for config-derived accuracy plus marker idempotence.
