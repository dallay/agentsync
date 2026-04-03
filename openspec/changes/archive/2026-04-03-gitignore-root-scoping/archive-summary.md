# Archive Summary: gitignore-root-scoping

## Change

Root-scope auto-generated `.gitignore` entries so managed patterns for concrete repository-root
files (e.g., `AGENTS.md`, `CLAUDE.md`, `.mcp.json`) use leading `/` to prevent accidental matching
of nested files like `.agents/AGENTS.md`.

## Dates

- **Started**: 2026-03-31
- **Archived**: 2026-04-03

## Verdict

**PASS WITH WARNINGS** — All 10 tasks complete, 24 tests passing, 11/13 spec scenarios covered by
automated tests, 2/13 verified manually via `git check-ignore`.

## Artifacts

| Artifact           | Description                                                       |
|--------------------|-------------------------------------------------------------------|
| `proposal.md`      | Change proposal with intent, scope, approach, and risks           |
| `specs/`           | Delta spec for gitignore-management (synced to main specs)        |
| `design.md`        | Technical design: normalize in config.rs, preserve manual entries |
| `tasks.md`         | 10 implementation tasks across 3 phases, all complete             |
| `verify-report.md` | Verification report with compliance matrix and test evidence      |

## Spec Sync

- `openspec/changes/gitignore-root-scoping/specs/gitignore-management/spec.md` →
  `openspec/specs/gitignore-management/spec.md` (replaced)

## Key Implementation

- Added `normalize_managed_gitignore_entry()` private helper in `src/config.rs`
- Routed auto-generated managed destinations, backups, and known patterns through the helper
- Manual `[gitignore].entries` remain verbatim and untouched
- Doctor audits inherit the same normalized set via shared `all_gitignore_entries()`

## Warnings Carried Forward

- Two spec scenarios (nested file non-matching) verified manually only — consider adding a
  `git check-ignore` regression test in a future change
- Unrelated `README.md` modification noted during verify — excluded from this change scope
