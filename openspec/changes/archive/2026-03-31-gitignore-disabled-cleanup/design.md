# Design: Clean up managed .gitignore blocks when gitignore is disabled

## Technical Approach

Keep `.gitignore` reconciliation centered in the existing CLI apply flow. `src/main.rs` will switch from a single `if !no_gitignore && enabled` branch to an explicit three-way decision: skip when `--no-gitignore` is set, update/create the managed block when `[gitignore].enabled = true`, and remove any managed block when `[gitignore].enabled = false`. The cleanup path should live in `src/gitignore.rs` so marker parsing, dry-run messaging, idempotence, and write avoidance stay in one module.

## Architecture Decisions

### Decision: Put cleanup behavior in `src/gitignore.rs`

**Choice**: Add a dedicated public cleanup helper in `src/gitignore.rs` and call it from `src/main.rs`.
**Alternatives considered**: Inline file-removal logic in `src/main.rs`; make `remove_managed_section(...)` public and let callers assemble the rest.
**Rationale**: `src/main.rs` should remain orchestration-only. `src/gitignore.rs` already owns marker construction, file I/O, content rewriting, dry-run output, and idempotence concerns for `.gitignore`.

### Decision: Keep `remove_managed_section(...)` internal

**Choice**: Expose a higher-level helper such as `cleanup_gitignore(project_root, marker, dry_run)` while keeping `remove_managed_section(...)` private.
**Alternatives considered**: Publicly expose `remove_managed_section(...)` as the new contract.
**Rationale**: The internal helper is content-oriented and intentionally narrow. The new use case needs a full operational contract: read `.gitignore` if present, derive markers from the configured marker, detect whether cleanup changes content, avoid unnecessary writes, and print apply-style status. A public wrapper preserves encapsulation and avoids duplicating file-handling logic at call sites.

### Decision: Preserve `--no-gitignore` as the highest-priority bypass

**Choice**: Branch in `src/main.rs` in this order: `--no-gitignore` skip, enabled update, disabled cleanup.
**Alternatives considered**: Let disabled cleanup run even when `--no-gitignore` is set.
**Rationale**: The current product contract is that `--no-gitignore` disables all `.gitignore` reconciliation. The bug fix is only for the normal apply path and must not change that override.

## Data Flow

```text
agentsync apply
    -> linker.sync(...)
    -> gitignore branch in src/main.rs
         --no-gitignore? ----------> skip entirely
         enabled? -----------------> gitignore::update_gitignore(...)
         disabled? ----------------> gitignore::cleanup_gitignore(...)

cleanup_gitignore(...)
    -> read .gitignore if it exists
    -> build start/end markers from configured marker
    -> remove_managed_section(existing, start, end)
    -> if unchanged: report no cleanup needed, do not write
    -> if dry-run: report would remove managed section, do not write
    -> else: write cleaned content
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/main.rs` | Modify | Replace the single update-only condition with explicit skip/update/cleanup branching for `.gitignore`. |
| `src/gitignore.rs` | Modify | Add a public cleanup helper that wraps marker resolution, change detection, dry-run reporting, and no-op write avoidance around the existing private removal logic. |
| `src/commands/doctor.rs` | No functional change expected | Existing behavior already does not require a managed section when gitignore is disabled and the file is clean; only review messaging/tests for coherence. |
| `src/commands/doctor_tests.rs` | Modify | Add or adjust tests that lock in disabled-without-managed-section expectations if needed. |
| `tests/test_update_output.rs` and/or a new CLI integration test | Modify | Cover apply output and on-disk behavior for disabled cleanup, `--no-gitignore`, dry-run, and custom-marker scenarios. |

## Interfaces / Contracts

Planned public helper in `src/gitignore.rs`:

```rust
pub fn cleanup_gitignore(project_root: &Path, marker: &str, dry_run: bool) -> Result<()>
```

Expected contract:

- Uses `# START {marker}` / `# END {marker}` to identify the managed block.
- Removes only the configured managed block and preserves all other `.gitignore` lines.
- If `.gitignore` does not exist, or no matching managed block is present, returns success without writing.
- In dry-run mode, reports that cleanup would occur only when content would actually change.
- In normal mode, skips the write when cleaned content matches the existing file content.

`remove_managed_section(content, start_marker, end_marker)` remains private and continues to serve as the content transformation primitive used by both update replacement and disabled cleanup.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | Cleanup removes only the configured managed block and respects custom markers | Extend `src/gitignore.rs` tests around managed-section removal and the new cleanup helper. |
| Unit | Cleanup is idempotent and avoids writes when no matching block exists or content is unchanged | Add tests that verify unchanged files remain byte-for-byte identical and, where practical, preserve mtime. |
| CLI integration | Disabled config triggers cleanup on `apply` | Add a temp-repo test that seeds a stale managed block, runs `agentsync apply`, and asserts the block is removed while symlink sync still succeeds. |
| CLI integration | `--no-gitignore` still skips cleanup | Seed the same stale block, run `agentsync apply --no-gitignore`, and assert `.gitignore` is untouched. |
| CLI integration | Dry-run reports cleanup without writing | Run `agentsync apply --dry-run` with gitignore disabled and assert stdout mentions removal while file content stays unchanged. |
| Doctor review | Disabled + cleaned `.gitignore` yields no gitignore warning | Add a focused doctor test if needed to document that no managed section is required once management is disabled. |

## Migration / Rollout

No migration required. Repositories that disable gitignore management may see a one-time cleanup diff on the next normal `agentsync apply`; after that, subsequent applies should be no-ops unless the managed block reappears.

## Open Questions

- [ ] None blocking.
