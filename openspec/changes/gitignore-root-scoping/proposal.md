# Proposal: Root-scope auto-generated .gitignore entries

## Intent

Issue DALLAY-221 addresses a correctness bug in generated `.gitignore` content. Auto-generated entries such as `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.mcp.json`, `opencode.json`, and `WARP.md` are currently emitted without root scoping, so Git treats them as matches anywhere in the repository tree rather than only at the repository root. This can accidentally ignore canonical nested files such as `.agents/AGENTS.md`, creating confusing behavior and hiding tracked or intended files from normal workflows.

## Scope

### In Scope\n\n- Normalize auto-generated `.gitignore` entries that represent concrete repository-root files so they are written as root-scoped patterns.
- Preserve existing behavior for user-supplied `[gitignore].entries`, including patterns intentionally meant to match anywhere in the tree.
- Update validation coverage for generated `.gitignore` output so nested canonical files are no longer ignored by the managed defaults.

### Out of Scope\n\n- Changing the product default for `[gitignore].enabled`; it remains `true`.
- Reinterpreting, rewriting, or auto-normalizing user-authored `[gitignore].entries`.
- Broad redesign of `.gitignore` generation beyond the root-scoping fix for managed entries.

## Approach

Adjust the managed-entry assembly path so only auto-generated concrete root-level file entries are normalized before rendering. The preferred implementation is to apply normalization in the config layer where managed entries are assembled, prefixing `/` when an auto-generated entry lacks any slash and represents a concrete repository-root path. Rendering in `src/gitignore.rs::update_gitignore()` should remain largely unchanged, continuing to write the final entry set verbatim.

This keeps the fix narrowly scoped: managed defaults become safer and more precise, while user-provided patterns retain their current semantics.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/config.rs` | Modified | Normalize managed auto-generated gitignore entries during assembly. |
| `src/gitignore.rs` | Modified | Preserve rendering contract while consuming normalized managed entries. |
| `tests/` | Modified | Add or update coverage for root-scoped generated entries and nested-file safety. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Root-scoping logic accidentally changes semantics for user-supplied entries | Low | Limit normalization strictly to auto-generated managed entries and leave `[gitignore].entries` untouched. |
| An incomplete allowlist of managed root files leaves some generated entries unscoped | Medium | Derive normalization from the managed-entry source of truth and add tests covering known generated filenames. |
| Existing repositories may notice diffs in generated `.gitignore` output | Medium | Keep the change limited to concrete managed root entries and document the migration as a safe corrective diff. |

## Rollback Plan

Revert the managed-entry normalization change and restore the previous generated `.gitignore` behavior if unexpected matching regressions appear. Because the fix is isolated to proposal-targeted modules and output generation, rollback is low risk and can be performed by reverting the implementation commit and regenerating managed gitignore content.

## Dependencies

- Existing managed `.gitignore` entry generation in `src/config.rs::all_gitignore_entries()`.
- Existing rendering flow in `src/gitignore.rs::update_gitignore()`.

## Success Criteria

- [ ] Auto-generated concrete root-level files are emitted as root-scoped `.gitignore` patterns.
- [ ] Managed defaults no longer ignore nested canonical files such as `.agents/AGENTS.md`.
- [ ] User-supplied `[gitignore].entries` preserve their current matching behavior without automatic normalization.
- [ ] Test coverage demonstrates both the corrected root-scoped output and the non-regression for user-defined entries.

## Acceptance Criteria

- Generated `.gitignore` output prefixes `/` for affected managed root-file entries that previously matched anywhere in the repository.
- Managed output continues to include the same default entries, with only the intended root-scoping correction.
- Nested files with the same basename as managed root entries remain visible to Git unless separately ignored by explicit user rules.
- No proposal-driven change alters the default enablement policy for gitignore management.

## Rollout Notes

This should roll out as a normal backward-compatible bug fix. Repositories using managed gitignore generation will see a small one-time diff in generated output for corrected entries, but the behavioral change is intentionally safer because it reduces accidental ignores outside the repository root.

## Non-Goals and Risk Notes

Do not treat this work as a general gitignore policy redesign. The main operational risk is overreaching normalization; implementation should stay tightly focused on managed root-file entries only.
