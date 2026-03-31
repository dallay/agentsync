# Proposal: Init Wizard Post-Migration Summary

## Intent

Improve the `agentsync init --wizard` post-migration summary so users understand what the wizard actually did, what it intentionally did not do, and which follow-up git and `agentsync apply` actions they still need to take.

Today the wizard migrates files into `.agents/`, writes configuration, and may back up originals, but it does not run `apply`, does not update `.gitignore`, and does not inspect repository git state. The current completion messaging can therefore leave users with an incomplete or misleading mental model of what remains before the repo is fully reconciled.

## Scope

### In Scope
- Update wizard completion output to summarize migrated content, config creation, optional backup results, and required manual follow-up actions.
- Explicitly explain that the wizard does **not** run `agentsync apply`, does **not** update `.gitignore`, and does **not** inspect or summarize git status.
- Add guardrails so wizard messaging makes only safe claims about follow-up work and avoids duplicate or conflicting generic footer output.

### Out of Scope
- Changing the default gitignore policy; `[gitignore].enabled = true` remains the product default.
- Making the wizard run `agentsync apply`, edit `.gitignore`, stage files, or inspect git state.
- Broader init flow redesign outside the post-migration summary and closely related completion messaging.

### Safe and Unsafe Claims

#### Safe claims the wizard MAY make
- It migrated or skipped specific files/directories into `.agents/`.
- It wrote or preserved `.agents/agentsync.toml`.
- It did or did not back up original source files.
- The user still needs to run `agentsync apply` to reconcile downstream target files.
- `apply` may update managed files such as agent targets and `.gitignore` according to configuration.
- The user should review resulting file changes with normal git workflows before committing.

#### Unsafe claims the wizard MUST NOT make
- That `apply` has already run.
- That `.gitignore` has already been updated or does not need changes.
- That git changes are clean, dirty, staged, unstaged, or safe to commit.
- That backups, migration, or config writes are the only repo changes requiring review.
- Any repo-specific git instruction derived from actual git state, since the wizard does not inspect it.

## Approach

Refine the wizard completion flow so it emits a purpose-built post-migration summary and next-steps section based only on facts known during the wizard run. The summary should clearly separate:
- what was migrated or skipped,
- whether config and backup actions occurred,
- what still requires `agentsync apply`, and
- what git review actions the user should take manually.

Implementation should centralize or coordinate completion messaging between `init_wizard()` and the generic init footer so users see one coherent set of next steps. The wizard-specific summary must avoid duplicate or conflicting generic footer output, especially where the generic footer could imply a simpler initialization path than the migration flow actually performed.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/init.rs` | Modified | Update wizard post-migration summary, next-step guidance, and any backup-related completion messaging. |
| `src/main.rs` | Modified | Coordinate or suppress generic init footer output so wizard runs do not print duplicate/conflicting completion guidance. |
| `tests/**/*.rs` | Modified | Add or update CLI/wizard output tests covering accurate follow-up messaging and non-claims. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Messaging over-promises actions the wizard did not perform | Med | Limit output to facts known during execution and codify safe/unsafe claims in tests. |
| Generic and wizard-specific completion text diverge | Med | Consolidate completion messaging responsibilities and add regression coverage for final output. |
| Output changes become brittle for tests or docs | Low | Assert key user-facing statements rather than overly rigid full-output snapshots. |

## Rollback Plan

Revert the wizard summary/footer changes and restore the previous completion messaging behavior. Because this change is UX/output-only, rollback does not require data migration; it only restores prior text and test expectations.

## Dependencies

- Existing init wizard migration behavior in `src/init.rs`
- Existing init completion footer in `src/main.rs`
- Current gitignore-management and skill-adoption expectations

## Success Criteria

- [ ] After `agentsync init --wizard`, users are explicitly told that the wizard did not run `agentsync apply` and that they still need to run it.
- [ ] Wizard output explains that `.gitignore` may still change during `apply` according to configuration, without claiming it was already updated.
- [ ] Wizard output tells users to review git changes manually, without claiming any actual git status or staging state.
- [ ] Completion output contains one coherent post-migration summary and does not include duplicate or conflicting generic footer guidance.
- [ ] Automated tests cover the safe/unsafe messaging boundaries for the wizard completion flow.
