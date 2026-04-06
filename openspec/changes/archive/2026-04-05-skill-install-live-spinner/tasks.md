# Tasks: Skill Install Live Spinner

## Phase 1: Output Mode and Summary Contracts

- [x] 1.1 Add RED cases in `tests/unit/suggest_install.rs` for `detect_suggest_install_output_mode(...)` covering `--json`, non-TTY human fallback, TTY live selection, and `TERM=dumb` fallback.
- [x] 1.2 Add RED cases in `tests/unit/suggest_install.rs` for human summary rendering from mixed-result and no-op `SuggestInstallJsonResponse` values, requiring explicit `Installed`, `Already installed`, and `Failed` counts.
- [x] 1.3 Refactor private helpers in `src/commands/skill.rs` so suggest-install output can distinguish `Json`, `HumanLine`, and `HumanLive` without changing direct `skill install|update|uninstall` paths.

## Phase 2: Human-Mode Suggest Install UX

- [x] 2.1 Implement a TTY-only live reporter in `src/commands/skill.rs` with a small tick loop/writer seam that renders explicit `resolving` / `installing` activity and prints terminal success, skip, and failure lines.
- [x] 2.2 Wire `run_suggest` in `src/commands/skill.rs` to select `HumanLive` only for `skill suggest --install` and `--install --all` human TTY runs, while preserving current non-TTY line mode and unchanged `--json` execution.
- [x] 2.3 Replace the single-line suggest-install completion sentence in `src/commands/skill.rs` with a shared multi-line human summary block that always shows installed/already-installed/failed counts and appends failure details when present.

## Phase 3: Verification Coverage

- [x] 3.1 Add unit coverage in `tests/unit/suggest_install.rs` for live reporter finalization through the injected writer seam, asserting newline cleanup and no partial spinner frame left behind.
- [x] 3.2 Update `tests/integration/skill_suggest.rs` for non-TTY `skill suggest --install --all` human output to assert ordered line updates, absence of cursor-control/spinner leakage, and the new summary block.
- [x] 3.3 Extend `tests/contracts/test_skill_suggest_output.rs` so `skill suggest --install --all --json` stays pure JSON with no progress preamble, spinner frames, or human summary headings/text.

## Phase 4: Cleanup and Scope Guardrails

- [x] 4.1 Remove superseded single-line summary helpers in `src/commands/skill.rs`, keep live UX command-layer-only, and verify no behavioral drift in `src/skills/suggest.rs` or other skill subcommands.
