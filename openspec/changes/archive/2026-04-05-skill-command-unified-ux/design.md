# Design: Unified CLI UX for Skill Commands

## Technical Approach

Extract the formatting abstractions already proven in the `suggest --install` flow
(`SuggestInstallHumanFormatter`, `SuggestInstallLabelKind`, output-mode detection) into a
command-agnostic sibling module `src/commands/skill_fmt.rs`. The three single-operation commands
(`install`, `update`, `uninstall`) will import the shared types to render colored status lines on
success and failure. JSON output paths remain untouched. The `suggest --install` code is refactored
to delegate to the shared types (rename only — zero behavior change). No spinners are added to
single-operation commands (REQ-015, proposal scope).

This maps directly to the proposal's four-phase approach and satisfies REQ-015, REQ-020, REQ-021,
and REQ-022 from the delta spec.

## Architecture Decisions

### Decision: Flat sibling module (`skill_fmt.rs`) over nested submodule directory

**Choice**: Create `src/commands/skill_fmt.rs` and add `pub mod skill_fmt;` to
`src/commands/mod.rs`.

**Alternatives considered**: Converting `skill.rs` into a `skill/` directory with `mod.rs` +
`fmt.rs`.

**Rationale**: The existing commands directory uses a flat layout (`doctor.rs`, `skill.rs`,
`status.rs`). A single new sibling file follows the established convention. The directory approach
would require moving `skill.rs` → `skill/mod.rs`, touching every import that currently references
`commands::skill`, and inflating the diff for zero functional benefit. If the module grows beyond
~200 lines in the future, restructuring into a directory is a trivial follow-up.

### Decision: Simplified two-variant `OutputMode` for single-op commands

**Choice**: The shared `OutputMode` enum has two variants: `Json` and `Human { use_color: bool }`.

**Alternatives considered**: Reusing the three-variant `SuggestInstallOutputMode` (with
`HumanLive`) everywhere.

**Rationale**: Single-operation commands execute one blocking call — there is nothing to animate.
Exposing a `HumanLive` variant would force callers to handle a case that can never occur.
`SuggestInstallOutputMode` stays as a local enum inside `skill.rs` that adds the `HumanLive`
variant on top of the shared `OutputMode`'s color detection. The detection logic for `use_color`
is shared; only the live-vs-line distinction is local.

### Decision: Keep `SuggestInstallOutputMode` as a local three-variant enum

**Choice**: `SuggestInstallOutputMode` stays in `skill.rs` with variants `Json`, `HumanLine`,
`HumanLive`. It delegates to the shared `detect_output_mode()` for color detection and adds the
TTY-based live/line split locally.

**Alternatives considered**: Merging everything into a single four-variant shared enum.

**Rationale**: The spinner infrastructure (`SuggestInstallLiveReporter`, `LiveCommand`,
`LiveWriter`, worker thread) is deeply specific to the suggest-install batch flow. Leaking that
distinction into a shared module violates single-responsibility and would force every consumer to
handle an impossible variant. The suggest-install code calls `detect_output_mode()` to get
`use_color`, then locally decides live-vs-line based on `stdout_is_tty && !term_is_dumb`.

### Decision: Rename shared types to command-agnostic names

**Choice**: `SuggestInstallHumanFormatter` → `HumanFormatter`,
`SuggestInstallLabelKind` → `LabelKind`. Drop the `SuggestInstall` prefix for types that move
to the shared module.

**Alternatives considered**: `SkillHumanFormatter` / `StatusLabelKind` (as proposed).

**Rationale**: The types live inside `commands::skill_fmt` — the module path already scopes them
to skill commands. Adding `Skill` or `Status` to the type name is redundant
(`skill_fmt::SkillHumanFormatter` vs `skill_fmt::HumanFormatter`). Shorter names reduce noise.
The fully-qualified path `commands::skill_fmt::HumanFormatter` is unambiguous. This follows
Rust's idiomatic convention of letting the module provide namespace context.

## Data Flow

### Single-operation command (install/update/uninstall)

```
CLI args (--json flag)
    │
    ▼
skill_fmt::detect_output_mode(json, tty, env...)
    │
    ├─ OutputMode::Json ──► existing JSON serialization (unchanged)
    │
    └─ OutputMode::Human { use_color }
         │
         ▼
    skill_fmt::HumanFormatter::new(use_color)
         │
         ├─ success ──► fmt.format_label("✔", verb, LabelKind::Success) ──► println!
         │
         └─ failure ──► fmt.format_label("✗", "failed", LabelKind::Failure) ──► println!
                        println!("Hint: {remediation}")
```

### Suggest-install flow (refactored delegation)

```
CLI args (--json, tty state)
    │
    ▼
skill_fmt::detect_output_mode(json, tty, env...)
    │
    ├─ OutputMode::Json ──► existing JSON path
    │
    └─ OutputMode::Human { use_color }
         │
         ▼
    Local: is_live = stdout_is_tty && !term_is_dumb
         │
         ├─ true  ──► SuggestInstallLiveReporter(skill_fmt::HumanFormatter)
         │
         └─ false ──► SuggestInstallLineReporter(skill_fmt::HumanFormatter)
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/commands/skill_fmt.rs` | Create | Shared module: `HumanFormatter`, `LabelKind`, `OutputMode`, `detect_output_mode()`, `output_mode()` convenience wrapper |
| `src/commands/mod.rs` | Modify | Add `pub mod skill_fmt;` line |
| `src/commands/skill.rs` | Modify | (1) Remove `SuggestInstallHumanFormatter`, `SuggestInstallLabelKind`, `detect_suggest_install_output_mode`, `suggest_install_output_mode`; (2) Import shared types from `skill_fmt`; (3) Replace `println!` in `run_install`/`run_update`/`run_uninstall` success+error paths with formatter calls; (4) Refactor `SuggestInstallLineReporter`, `SuggestInstallLiveReporter`, and `render_suggest_install_*` functions to use `skill_fmt::HumanFormatter` and `skill_fmt::LabelKind`; (5) Keep `SuggestInstallOutputMode` locally with delegated color detection |

## Interfaces / Contracts

### `src/commands/skill_fmt.rs` — full public API

```rust
use colored::Colorize;
use std::io::IsTerminal;

/// Semantic kind for a status label. Determines color when color is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    Info,
    Warning,
    Success,
    Failure,
}

/// Output mode for single-operation skill commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Json,
    Human { use_color: bool },
}

/// Formatter for human-readable skill command output.
/// When `use_color` is true, applies ANSI color+bold via the `colored` crate.
/// When false, returns plain text preserving Unicode symbols.
#[derive(Debug, Clone, Copy)]
pub struct HumanFormatter {
    use_color: bool,
}

impl HumanFormatter {
    pub fn new(use_color: bool) -> Self {
        Self { use_color }
    }

    /// Format a status label: "{symbol} {label}" with color per kind.
    pub fn format_label(&self, symbol: &str, label: &str, kind: LabelKind) -> String {
        let text = format!("{symbol} {label}");
        if !self.use_color {
            return text;
        }
        match kind {
            LabelKind::Info => text.cyan().bold().to_string(),
            LabelKind::Warning => text.yellow().bold().to_string(),
            LabelKind::Success => text.green().bold().to_string(),
            LabelKind::Failure => text.red().bold().to_string(),
        }
    }

    /// Format a heading: bold when color is enabled, plain otherwise.
    pub fn format_heading(&self, heading: &str) -> String {
        if self.use_color {
            heading.bold().to_string()
        } else {
            heading.to_string()
        }
    }
}

/// Detect output mode from CLI flags and environment.
///
/// Pure function — accepts pre-read values for testability.
/// The `json` flag takes absolute priority.
/// Color is enabled only when stdout is a TTY AND none of the
/// NO_COLOR / CLICOLOR=0 / TERM=dumb overrides are active.
pub fn detect_output_mode(
    json: bool,
    stdout_is_tty: bool,
    no_color: Option<&str>,
    clicolor: Option<&str>,
    term: Option<&str>,
) -> OutputMode {
    if json {
        return OutputMode::Json;
    }
    let no_color_set = no_color.is_some_and(|v| !v.is_empty());
    let clicolor_zero = clicolor.is_some_and(|v| v == "0");
    let dumb_term = term.is_some_and(|v| v.eq_ignore_ascii_case("dumb"));
    let use_color = stdout_is_tty && !dumb_term && !no_color_set && !clicolor_zero;
    OutputMode::Human { use_color }
}

/// Convenience wrapper that reads real environment state.
pub fn output_mode(json: bool) -> OutputMode {
    detect_output_mode(
        json,
        std::io::stdout().is_terminal(),
        std::env::var("NO_COLOR").ok().as_deref(),
        std::env::var("CLICOLOR").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
    )
}
```

### Usage pattern in `run_install` (representative — `run_update`/`run_uninstall` follow same shape)

```rust
use super::skill_fmt::{self, HumanFormatter, LabelKind, OutputMode};

// At top of run_install, before the operation:
let mode = skill_fmt::output_mode(args.json);

// Success path:
match mode {
    OutputMode::Json => { /* existing JSON serialization — unchanged */ }
    OutputMode::Human { use_color } => {
        let fmt = HumanFormatter::new(use_color);
        println!("{} {skill_id}", fmt.format_label("✔", "installed", LabelKind::Success));
    }
}

// Error path:
match mode {
    OutputMode::Json => { /* existing JSON error serialization — unchanged */ }
    OutputMode::Human { use_color } => {
        let fmt = HumanFormatter::new(use_color);
        error!(%code, %err_string, "Install failed");
        println!(
            "{} {skill_id}: {err_string}",
            fmt.format_label("✗", "failed", LabelKind::Failure)
        );
        println!("Hint: {}", remediation);
    }
}
```

### Refactored `SuggestInstallOutputMode` delegation (stays in `skill.rs`)

```rust
use super::skill_fmt::{self, HumanFormatter, LabelKind};

// Local enum retains HumanLive for spinner support
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuggestInstallOutputMode {
    Json,
    HumanLine { use_color: bool },
    HumanLive { use_color: bool },
}

fn suggest_install_output_mode(json: bool) -> SuggestInstallOutputMode {
    let base = skill_fmt::output_mode(json);
    match base {
        skill_fmt::OutputMode::Json => SuggestInstallOutputMode::Json,
        skill_fmt::OutputMode::Human { use_color } => {
            let stdout_is_tty = std::io::stdout().is_terminal();
            let term_is_dumb = std::env::var("TERM")
                .ok()
                .is_some_and(|v| v.eq_ignore_ascii_case("dumb"));
            if stdout_is_tty && !term_is_dumb {
                SuggestInstallOutputMode::HumanLive { use_color }
            } else {
                SuggestInstallOutputMode::HumanLine { use_color }
            }
        }
    }
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `HumanFormatter::format_label` colored vs plain per `LabelKind` | `skill_fmt.rs` `#[cfg(test)]` — assert ANSI codes present/absent. 4 kinds × 2 modes. |
| Unit | `HumanFormatter::format_heading` bold vs plain | Same module. 2 tests. |
| Unit | `detect_output_mode` JSON priority | Same module. Assert `OutputMode::Json` regardless of other params. |
| Unit | `detect_output_mode` color gating combinations | Table-driven: normal TTY, non-TTY, NO_COLOR, CLICOLOR=0, TERM=dumb, empty NO_COLOR. 6+ cases. |
| Unit | Suggest-install output-mode delegation still correct | Existing tests in `skill.rs` updated to call new delegation — same assertions, same expected values. |
| Existing | `render_suggest_install_completion_summary` tests | No change — uses `HumanFormatter` via renamed import. |
| Existing | `SuggestInstallLiveReporter` finalize test | No change — reporter uses `HumanFormatter` via renamed import. |
| Contract | `tests/contracts/test_install_output.rs` | No change — tests JSON paths only. |
| Contract | `tests/test_update_output.rs` | No change — tests JSON paths only. |

### Focused new tests for human output of install/update/uninstall

```rust
// In src/commands/skill_fmt.rs #[cfg(test)]

#[test]
fn format_label_success_colored() {
    let fmt = HumanFormatter::new(true);
    let result = fmt.format_label("✔", "installed", LabelKind::Success);
    assert!(result.contains("✔ installed"));
    assert!(result.contains("\x1b["));  // ANSI escape present
}

#[test]
fn format_label_success_plain() {
    let fmt = HumanFormatter::new(false);
    let result = fmt.format_label("✔", "installed", LabelKind::Success);
    assert_eq!(result, "✔ installed");
    assert!(!result.contains("\x1b["));
}

#[test]
fn format_label_failure_colored() {
    let fmt = HumanFormatter::new(true);
    let result = fmt.format_label("✗", "failed", LabelKind::Failure);
    assert!(result.contains("✗ failed"));
    assert!(result.contains("\x1b["));
}

#[test]
fn format_label_each_kind_uses_distinct_color() {
    let fmt = HumanFormatter::new(true);
    let info = fmt.format_label("i", "info", LabelKind::Info);
    let warn = fmt.format_label("!", "warn", LabelKind::Warning);
    let ok = fmt.format_label("✔", "ok", LabelKind::Success);
    let fail = fmt.format_label("✗", "fail", LabelKind::Failure);
    // All contain ANSI escapes
    for c in [&info, &warn, &ok, &fail] {
        assert!(c.contains("\x1b["), "{c}");
    }
    // All four are distinct strings (different color codes)
    use std::collections::HashSet;
    let unique: HashSet<&String> = [&info, &warn, &ok, &fail].into_iter().collect();
    assert_eq!(unique.len(), 4);
}

#[test]
fn format_heading_bold_when_colored() {
    let fmt = HumanFormatter::new(true);
    let result = fmt.format_heading("Summary");
    assert!(result.contains("\x1b["));
    assert!(result.contains("Summary"));
}

#[test]
fn format_heading_plain_when_no_color() {
    let fmt = HumanFormatter::new(false);
    let result = fmt.format_heading("Summary");
    assert_eq!(result, "Summary");
}

#[test]
fn detect_output_mode_json_takes_priority() {
    assert_eq!(
        detect_output_mode(true, true, None, None, Some("xterm")),
        OutputMode::Json
    );
}

#[test]
fn detect_output_mode_tty_with_color() {
    assert_eq!(
        detect_output_mode(false, true, None, None, Some("xterm-256color")),
        OutputMode::Human { use_color: true }
    );
}

#[test]
fn detect_output_mode_no_tty_no_color() {
    assert_eq!(
        detect_output_mode(false, false, None, None, Some("xterm-256color")),
        OutputMode::Human { use_color: false }
    );
}

#[test]
fn detect_output_mode_no_color_env() {
    assert_eq!(
        detect_output_mode(false, true, Some("1"), None, Some("xterm")),
        OutputMode::Human { use_color: false }
    );
}

#[test]
fn detect_output_mode_clicolor_zero() {
    assert_eq!(
        detect_output_mode(false, true, None, Some("0"), Some("xterm")),
        OutputMode::Human { use_color: false }
    );
}

#[test]
fn detect_output_mode_dumb_term() {
    assert_eq!(
        detect_output_mode(false, true, None, None, Some("dumb")),
        OutputMode::Human { use_color: false }
    );
}
```

## Migration / Rollout

No migration required. All changes are additive refactoring within a single module area.
Human output is not a stable contract (JSON is the machine-readable interface). The `colored`
crate is already a dependency — no new crates needed.

## Open Questions

None. All technical decisions are resolved by existing codebase patterns and the spec requirements.
