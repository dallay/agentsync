# Design: Skill Install Live Spinner

## Technical Approach

Phase 1 stays entirely in the CLI command layer. `src/commands/skill.rs` already decides between human and JSON output and already consumes `SuggestInstallProgressEvent` values emitted by `SuggestionService::install_selected_with_reporter`. The design extends that command-layer presentation choice into three explicit human/machine paths:

- `Json`: unchanged final JSON only.
- `HumanLine`: today's stable line-oriented reporter for non-TTY runs.
- `HumanLive`: a lightweight TTY-only live reporter for `skill suggest --install` and `--install --all`.

The install workflow, provider resolution, result aggregation, and JSON response shape in `src/skills/suggest.rs` stay unchanged. The command layer will adapt existing progress events into either plain lines or a live single-activity spinner/status line, then print a richer final summary block in human mode.

## Architecture Decisions

### Decision: Keep live UX entirely at the command layer

**Choice**: Build the spinner/live reporter in `src/commands/skill.rs` as another `SuggestInstallProgressReporter` implementation that consumes the existing event stream.

**Alternatives considered**:
- Add terminal rendering responsibilities into `src/skills/suggest.rs`
- Change the progress event model to emit UI-specific frames or counters

**Rationale**: `src/skills/suggest.rs` is currently the execution/result layer; `src/commands/skill.rs` is already the presentation boundary. Keeping live behavior in the command layer preserves separation of concerns and avoids changing install semantics or the JSON contract.

### Decision: Use a tiny background tick loop, not a new spinner dependency

**Choice**: Implement the live reporter with `std` primitives (`thread`, channel/shared state, `\r`, stdout flush) instead of adding a crate like `indicatif`.

**Alternatives considered**:
- Add a third-party progress/spinner dependency
- Redraw only when events arrive, with no tick loop

**Rationale**: The repo does not currently depend on a spinner crate, and the change is intentionally narrow. Event-only redraws would not feel alive during long installs because there is often a gap between `Installing` and the terminal result event. A tiny internal tick loop gives a real spinner while staying lightweight and dependency-free.

### Decision: Preserve the existing non-TTY line reporter as the fallback human contract

**Choice**: Continue using stable one-line updates for non-TTY human runs and whenever live terminal capabilities are not available.

**Alternatives considered**:
- Use live rendering for all human runs
- Replace non-TTY output with a buffered summary-only mode

**Rationale**: The spec explicitly requires readable, ordered, line-oriented output without cursor-control dependence outside TTY mode. The current line reporter already satisfies that requirement and should remain the safe fallback.

### Decision: Render a multi-line final summary block for both human modes

**Choice**: Replace the current single completion sentence with a structured human summary block that always includes explicit counts for installed, already-installed, and failed results, with optional failure details.

**Alternatives considered**:
- Keep the one-line completion sentence only
- Show a rich summary only in TTY live mode

**Rationale**: The new requirement is about scanability, not animation only. A shared final summary block keeps the human contract aligned across live and non-TTY modes while still allowing TTY color/emphasis.

## Data Flow

### Runtime flow

```text
run_suggest()
  │
  ├─ suggest_install_output_mode(...) ──→ Json | HumanLine | HumanLive
  │
  ├─ optional interactive selection (only --install without --all)
  │
  ├─ HumanLive/HumanLine:
  │    create reporter implementing SuggestInstallProgressReporter
  │
  ├─ SuggestionService::install_selected_with_reporter(..., reporter, ...)
  │    │
  │    ├─ emit Resolving(skill)
  │    ├─ emit Installing(skill)
  │    ├─ emit Installed(skill)
  │    ├─ emit SkippedAlreadyInstalled(skill)
  │    └─ emit Failed(skill, phase, message)
  │
  ├─ reporter renders live line or plain lines
  │
  └─ command prints final human summary block or final JSON
```

### TTY live sequence

```text
SuggestionService            Live reporter                 stdout
       │                           │                         │
       │ Resolving(skill-a)        │                         │
       ├──────────────────────────>│ set state=Resolving     │
       │                           ├────────────────────────>│ \r⠋ resolving skill-a
       │                           │ tick loop advances      │
       │                           ├────────────────────────>│ \r⠙ resolving skill-a
       │ Installing(skill-a)       │                         │
       ├──────────────────────────>│ set state=Installing    │
       │                           ├────────────────────────>│ \r⠹ installing skill-a
       │ Installed(skill-a)        │                         │
       ├──────────────────────────>│ stop live line, print   │
       │                           ├────────────────────────>│ \r✔ installed skill-a\n
       │ next events...            │                         │
       │                           │                         │
       │ finished response         │ finalize/clear line     │
       │                           ├────────────────────────>│ \nRecommendation install summary...
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/commands/skill.rs` | Modify | Extend output mode detection to distinguish JSON, non-TTY line mode, and TTY live mode; add live reporter implementation; replace single-line completion sentence with structured final summary rendering. |
| `tests/integration/skill_suggest.rs` | Modify | Update non-TTY human assertions to match the new summary block while preserving ordered line updates and no ANSI/cursor-control leakage. |
| `tests/contracts/test_skill_suggest_output.rs` | Modify | Keep JSON contract coverage and add assertions that no human summary/progress text leaks into `--json` suggest-install flows. |

## Interfaces / Contracts

No changes are planned to the service-side install event contract in `src/skills/suggest.rs`:

```rust
pub trait SuggestInstallProgressReporter {
    fn on_event(&mut self, event: SuggestInstallProgressEvent);
}
```

The command layer will likely evolve its private output-mode model into something equivalent to:

```rust
enum SuggestInstallOutputMode {
    Json,
    HumanLine { use_color: bool },
    HumanLive { use_color: bool },
}
```

The human summary renderer should return a deterministic string/block derived from `SuggestInstallJsonResponse`, for example:

```text
Recommendation install summary
  Installed: 1
  Already installed: 1
  Failed: 1
```

If failures exist, the block may append detail lines such as:

```text
  Failure details:
    - rust-async-patterns (install): <message>
```

Important contract notes:

- Human-only live rendering remains private to the command layer.
- `SuggestInstallJsonResponse` shape, field names, and ordering expectations remain unchanged.
- Direct `skill install`, `skill update`, and `skill uninstall` stay untouched in this phase.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | Output-mode selection chooses `Json`, `HumanLine`, or `HumanLive` correctly based on `--json`, TTY, and terminal-capability inputs. | Extend existing `src/commands/skill.rs` tests around `detect_suggest_install_output_mode(...)`. |
| Unit | Final summary block always includes explicit installed/already-installed/failed counts and uses the expected empty/no-op wording. | Replace/add string-render tests next to the current summary helper tests in `src/commands/skill.rs`. |
| Unit | Live reporter state transitions/finalization are correct without requiring a real PTY. | Keep the live renderer behind testable helpers (recording sink or injected writer/state) and assert it emits a final newline, success/failure terminal lines, and does not leave a partial spinner frame behind. |
| Integration | Non-TTY `skill suggest --install --all` remains line-oriented, readable, ordered, and free of ANSI/cursor-control sequences. | Update `tests/integration/skill_suggest.rs` to assert existing per-skill lines plus the new multi-line summary block. |
| Contract | JSON suggest-install output remains pure JSON with no batch-start line, progress lines, spinner frames, or human summary block. | Extend `tests/contracts/test_skill_suggest_output.rs` with negative assertions against human-only summary headings/text. |

## Migration / Rollout

No migration required.

Rollout is implicit and low risk because the change is limited to the suggest-install human presentation path. If regressions are found, the live reporter selection can be disabled and all human runs can fall back to `HumanLine` without changing service behavior or JSON output.

## Open Questions

- [ ] Should live mode be disabled when `TERM=dumb`, even if stdout reports as a TTY? The design assumes yes for safety.
- [ ] Should the final summary block include per-failure detail lines in phase 1, or should phase 1 stop at explicit counts only? Counts are required either way.
