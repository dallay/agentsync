# Design: Init Wizard Post-Migration Summary

## Technical Approach

Add a wizard-only completion renderer in `src/init.rs` and invoke it at the very end of `init_wizard()`, after the backup prompt and any backup moves have finished. The renderer will consume only facts gathered during the wizard run, print a final post-migration summary plus next steps, and make no claims about `apply`, `.gitignore`, or git state beyond what the proposal marks as safe.

`src/main.rs` will keep the shared success banner, but it will stop printing the generic init footer for wizard runs so users see one coherent completion message.

## Architecture Decisions

### Decision: Keep wizard summary composition inside `init_wizard()`

**Choice**: Compose and print the final wizard-specific summary in `src/init.rs`, immediately after backup handling is finalized.
**Alternatives considered**: Build the summary in `main.rs`; print part before backup and part after backup.
**Rationale**: Backup outcome is only known inside `init_wizard()`. Keeping summary ownership there avoids leaking wizard-only state upward and prevents incomplete or premature messaging.

### Decision: Model only wizard-known facts with a private summary state type

**Choice**: Introduce a small private struct/enum set in `src/init.rs` to track summary facts such as migrated counts, whether `AGENTS.md` or config was created/preserved, and backup outcome.
**Alternatives considered**: Recompute facts from the filesystem at the end; inspect git state; infer `apply`/`.gitignore` effects from config defaults.
**Rationale**: The proposal explicitly forbids unsafe claims. Persisting facts as they happen is simpler, deterministic, and avoids accidental overreach.

### Decision: Represent backup as an explicit outcome enum

**Choice**: Track backup with states like `NotOffered`, `Declined`, and `Completed { moved_count }` (optionally with a reason for not offering it).
**Alternatives considered**: Use a boolean like `did_backup`; infer outcome from whether `.agents/backup` exists.
**Rationale**: A boolean cannot distinguish “not offered”, “declined”, and “completed”. The final summary needs those distinctions to describe original-file handling accurately.

### Decision: Suppress generic footer in `main.rs` for wizard mode

**Choice**: Gate the existing generic “Next steps” footer so it prints only for non-wizard init runs.
**Alternatives considered**: Keep both footers; try to make generic footer wording broad enough for both paths.
**Rationale**: The proposal requires one coherent post-migration summary. The generic footer currently implies the simpler non-migration flow and would duplicate/conflict with wizard-specific guidance.

## Data Flow

```text
main.rs
  └─ init --wizard
      └─ init::init_wizard(project_root, force)
           ├─ discover files
           ├─ collect migration facts while copying/creating/skipping
           ├─ finalize backup decision and backup result
           └─ render wizard final summary from collected facts

main.rs
  └─ prints success banner
      └─ generic footer only when wizard == false
```

### Summary rendering sequence

```text
migration steps complete
      ↓
config write/preserve recorded
      ↓
backup prompt answered
      ↓
backup moves counted
      ↓
wizard final summary printed
      ↓
main.rs prints no generic wizard footer
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/init.rs` | Modify | Add private summary facts/backup outcome types, collect facts during wizard flow, and print the final wizard-only summary/next steps after backup handling. |
| `src/main.rs` | Modify | Prevent the generic init footer from printing for wizard mode while preserving the shared success banner. |
| `src/init.rs` tests | Modify | Add focused tests for summary text generation and backup outcome wording using stable substring assertions. |

## Interfaces / Contracts

No public API changes are required. The design can stay behind private helpers in `src/init.rs`.

Illustrative internal shape:

```rust
enum BackupOutcome {
    NotOffered,
    Declined,
    Completed { moved_count: usize },
}

struct WizardSummaryFacts {
    instruction_files_merged: usize,
    migrated_count: usize,
    skipped_count: usize,
    wrote_agents_md: bool,
    wrote_config: bool,
    backup: BackupOutcome,
}
```

The final renderer should produce wording only from these facts, plus fixed safe guidance:
- `agentsync apply` has not run yet
- `apply` may update managed files such as agent targets and `.gitignore`
- review resulting changes with normal git workflows

It must not mention actual git cleanliness/staging or claim `.gitignore` is already updated.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | Summary wording for each backup outcome and safe/unsafe claims | Extract summary-building logic into a small helper that returns plain text/lines; assert presence of required phrases and absence of forbidden claims instead of snapshotting full terminal output. |
| Unit | Footer suppression decision | Add a focused test for the branch/formatter decision that wizard mode does not emit the generic footer text from `main.rs`. |
| Integration | End-of-flow regression coverage | If practical, add one narrow CLI output test around completion messaging boundaries; keep assertions to a few key substrings so color, spacing, and unrelated copy changes do not break tests. |

## Migration / Rollout

No migration required. This is a localized CLI output change.

## Open Questions

- [ ] No delta spec files exist yet for this change; if spec artifacts are added later, confirm the final wording still matches them exactly.
