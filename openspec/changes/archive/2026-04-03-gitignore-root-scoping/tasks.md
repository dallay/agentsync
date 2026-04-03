# Tasks: Root-scope auto-generated .gitignore entries

## Phase 1: Foundation

- [x] 1.1 Inspect `src/config.rs::all_gitignore_entries()` and add a private helper that normalizes
  only auto-generated managed root-file entries by prefixing `/` when the entry is a concrete
  filename with no slash.
- [x] 1.2 Route managed destination entries in `src/config.rs` through the helper, including
  root-level destination files and their `.bak` companions, while continuing to skip `nested-glob`
  templates.
- [x] 1.3 Route built-in managed patterns in `src/config.rs` through the same helper so root files
  like `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.mcp.json`, `opencode.json`, and `WARP.md` become
  root-scoped, but slash-containing patterns remain unchanged.

## Phase 2: Shared behavior wiring

- [x] 2.1 Preserve manual `[gitignore].entries` in `src/config.rs` exactly as authored by keeping
  them outside the normalization path; verify mixed manual + managed entries still deduplicate and
  sort correctly.
- [x] 2.2 Confirm `src/gitignore.rs` and the apply flow continue to render the returned entries
  verbatim, updating code only if needed to keep rendering behavior unchanged with normalized
  inputs.
- [x] 2.3 Confirm `src/commands/doctor.rs` continues to source expected managed entries from
  `Config::all_gitignore_entries()` so doctor and apply share the same normalized set without
  duplicating logic.

## Phase 3: Tests and verification

- [x] 3.1 Extend `src/config.rs` tests for root-scoped managed destinations and backups, covering
  `AGENTS.md -> /AGENTS.md` and `AGENTS.md.bak -> /AGENTS.md.bak`.
- [x] 3.2 Extend `src/config.rs` tests for known managed patterns and non-regressions: root-level
  known files gain `/`, slash-containing patterns such as `.claude/commands/` and module-map
  expansions such as `src/api/CLAUDE.md` stay unchanged.
- [x] 3.3 Extend `src/config.rs` tests proving manual `[gitignore].entries` remain verbatim,
  including a bare `AGENTS.md` manual entry that must not be rewritten to `/AGENTS.md`.
- [x] 3.4 Add doctor-oriented coverage in `src/commands/doctor_tests.rs` showing normalized managed
  entries are accepted as expected output and legacy unscoped entries like `AGENTS.md` are treated
  as drift.
- [x] 3.5 Run focused validation with `cargo test --lib test_all_gitignore_entries` and
  `cargo test --lib doctor_tests`, then fix any expectation updates required by normalized managed
  output.
