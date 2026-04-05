# Design: Improve Skill Install UX Feedback

## Technical Approach

This change adds a narrow progress-event layer to recommendation-driven installs only (`agentsync skill suggest --install` and `--install --all`). The install engine in `src/skills/suggest.rs` will emit structured per-skill lifecycle events while reusing the existing resolve/install/registry flow, and `src/commands/skill.rs` will translate those events into human-readable status lines only when `--json` is not enabled.

The design keeps the current machine-readable contract intact by preserving the existing JSON response structs and by avoiding any streaming output in JSON mode. Human-mode output remains line-oriented in all environments; TTY-only behavior is limited to optional colorized status labels, not cursor control or animation. This maps directly to the delta spec requirements for terminal-aware presentation, JSON stability, and reuse of the existing installation lifecycle.

## Architecture Decisions

### Decision: Emit neutral progress events from the suggestion install service

**Choice**: Add a small internal progress reporting interface in `src/skills/suggest.rs` (trait or callback-based) and have the install loop emit typed events at key lifecycle boundaries.

**Alternatives considered**: Print directly from `run_suggest`; add a second installer specialized for suggestion UX.

**Rationale**: The install loop already owns the real state transitions (registry recheck, provider resolve, install call, result classification). Emitting events there keeps the CLI renderer honest, prevents duplicated logic, and preserves the requirement that recommendation installs reuse the existing installation lifecycle rather than inventing a parallel path.

### Decision: Keep `--json` on the existing final-response path only

**Choice**: JSON mode will continue to call the same install methods but with a no-op reporter so stdout remains a single final JSON document.

**Alternatives considered**: Stream JSON events; add progress fields to the final JSON payload; print progress to stderr.

**Rationale**: The spec explicitly requires the current JSON contract to remain unchanged and forbids interleaving human progress content. A no-op reporter keeps the existing data model and contract tests stable while letting human mode gain live feedback.

### Decision: Prefer custom status lines over a progress-bar dependency

**Choice**: Use simple custom status lines with existing `colored` support instead of adding `indicatif`/progress-bar style dependencies.

**Alternatives considered**: Add a progress-bar/spinner dependency; build animated single-line terminal rendering with cursor control.

**Rationale**: Suggest installs are serial, per-skill, and relatively short-lived. A progress bar would add dependency weight, terminal-control complexity, and harder testability without giving much extra value. Plain status lines are easier to verify in non-TTY tests, still readable in TTYs, and fit the proposal’s “simple and safe” constraint.

### Decision: Gate color independently from progress text

**Choice**: Detect an output mode in `src/commands/skill.rs` using `--json`, `stdout.is_terminal()`, and standard color opt-out environment checks (`NO_COLOR`, `CLICOLOR=0`). Human progress text is always emitted; ANSI color is added only when safe.

**Alternatives considered**: Always colorize when stdout is a TTY; globally toggle `colored`; suppress progress entirely on non-TTY.

**Rationale**: The requirement is about readable meaning first, visual enhancement second. Decoupling text from color keeps non-interactive output deterministic and accessible while avoiding global mutable color configuration. TTY gates should control cosmetics, not whether feedback exists.

## Data Flow

### Event emission points

The install loop in `SuggestionService::install_selected_with(...)` will emit events at these exact points for each selected recommendation:

1. **Registry recheck skip**: after reloading installed state and before any provider call, emit `SkippedAlreadyInstalled` when the skill is already present.
2. **Resolve start**: immediately before `provider.resolve(...)`, emit `Resolving`.
3. **Install start**: immediately after resolve succeeds and just before `install_fn(...)`, emit `Installing`.
4. **Resolve failure**: when `provider.resolve(...)` fails, emit `Failed { phase: Resolve, ... }`.
5. **Install failure**: when `install_fn(...)` fails, emit `Failed { phase: Install, ... }`.
6. **Install success**: after `install_fn(...)` succeeds and installed state is updated, emit `Installed`.

No events are emitted for unselected recommendations. Batch-level framing (for example, “Installing 3 recommended skills...”) stays in the command layer because it is presentation-only.

### Sequence

```text
run_suggest
  │
  ├─ suggest() -> SuggestResponse
  │
  ├─ choose output mode
  │    ├─ json -> no-op reporter
  │    └─ human -> status-line reporter
  │
  └─ install_selected_with_reporter(...)
       │
       ├─ reload registry
       ├─ for each selected recommendation
       │    ├─ already installed? -> emit SkippedAlreadyInstalled -> record result
       │    ├─ emit Resolving
       │    ├─ provider.resolve(...)
       │    │    └─ error -> emit Failed(resolve) -> record result
       │    ├─ emit Installing
       │    ├─ install_fn(...)
       │    │    └─ error -> emit Failed(install) -> record result
       │    └─ update installed state -> emit Installed -> record result
       │
       └─ return existing SuggestInstallJsonResponse
            ├─ json: print final JSON only
            └─ human: print compact final summary after streamed lines
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/skills/suggest.rs` | Modify | Add progress event types/reporter plumbing, emit events from the install loop, and preserve current JSON response construction. |
| `src/commands/skill.rs` | Modify | Detect human output mode, instantiate a reporter for suggest-install flows, gate color safely, and keep `--json` on the no-progress path. |
| `tests/unit/suggest_install.rs` | Modify | Add unit coverage for event emission order and emitted statuses for install, skip, and failure paths. |
| `tests/integration/skill_suggest.rs` | Modify | Add non-TTY human-output assertions and reinforce that `--json` emits only final JSON. |
| `tests/contracts/test_skill_suggest_output.rs` | Modify | Keep/extend JSON contract coverage to prove no extra fields or preamble are introduced for suggest install JSON flows. |
| `tests/e2e/scenarios/04-suggest-install-guided.sh` | Modify | Capture pseudo-TTY guided-install transcript and assert visible per-skill status lines before completion. |
| `tests/e2e/scenarios/05-suggest-install-all.sh` | Modify | Add human-mode install-all transcript assertions for install, skip, and no-op messaging. |

## Interfaces / Contracts

The reporting surface should stay internal and additive. One concrete shape:

```rust
pub enum SuggestInstallPhase {
    Resolve,
    Install,
}

pub enum SuggestInstallProgressEvent<'a> {
    Resolving { skill_id: &'a str },
    Installing { skill_id: &'a str },
    SkippedAlreadyInstalled { skill_id: &'a str },
    Installed { skill_id: &'a str },
    Failed {
        skill_id: &'a str,
        phase: SuggestInstallPhase,
        message: &'a str,
    },
}

pub trait SuggestInstallProgressReporter {
    fn on_event(&mut self, event: SuggestInstallProgressEvent<'_>);
}
```

Implementation notes:

- Keep `SuggestInstallJsonResponse`, `SuggestInstallResult`, `SuggestInstallStatus`, and their serde output unchanged.
- Preserve existing public install helpers by having them delegate to a new internal `*_with_reporter` helper using a no-op reporter.
- Human rendering should use explicit textual labels such as `resolving`, `installing`, `installed`, `already installed`, and `failed` so meaning survives even with colors disabled.
- Final human output should remain a completion summary, but per-skill live visibility should come from the streamed reporter events, not from terminal animation.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | Progress events fire in the correct order for success, skip-after-registry-recheck, resolve failure, and install failure. | Use a fake reporter that records events in `tests/unit/suggest_install.rs`; assert exact event sequences alongside existing result assertions. |
| Unit | Output-mode gating chooses plain human mode for non-TTY / `NO_COLOR` / `CLICOLOR=0`, and never enables human progress for `--json`. | Extract a tiny mode-detection helper in `src/commands/skill.rs` and test it directly. |
| Integration | `skill suggest --install --all --json` remains parseable JSON with no progress preamble or ANSI escapes. | Continue using `Command::output()` in non-TTY context and assert stdout parses as JSON exactly once. |
| Integration | Non-TTY human `--install --all` prints stable line-oriented statuses for installed/skipped/failed cases. | Add stdout substring assertions and explicitly reject escape sequences such as `\u001b[` / `\x1b[`. |
| E2E | Guided install under pseudo-TTY shows live status lines before completion. | Extend `04-suggest-install-guided.sh` to tee the transcript to a file and assert ordered status markers for selected skills. |
| E2E | Human install-all shows progress and clear no-op messaging when nothing is installable. | Extend `05-suggest-install-all.sh` to run both first-install and repeat-install human flows and assert `already installed` / `nothing installable` text. |

## Migration / Rollout

No migration required.

## Open Questions

- [ ] None blocking. If maintainers want animated spinner behavior later, it should be a follow-up after the line-based reporter lands and proves sufficient.
