# Proposal: Unified CLI UX for Skill Commands

## Intent

The `skill suggest --install` command has a polished output style: colored status lines with Unicode
symbols (`✔`, `✗`, `○`), TTY-aware color gating (`NO_COLOR`, `CLICOLOR`, `TERM=dumb`), and a clean
non-TTY fallback. Meanwhile, `skill install`, `skill update`, and `skill uninstall` use bare
`println!("Installed {}")` / `println!("Updated {}")` / `println!("Uninstalled {}")` for success
and `error!()` + `println!("Hint: ...")` for failures — no color, no symbols, no TTY awareness.

This inconsistency makes the CLI feel unfinished. A user who installs a recommended skill via
`suggest --install` sees `✔ installed my-skill`, but installing the same skill directly via
`skill install my-skill` sees plain `Installed my-skill`. Error output is equally inconsistent.

The formatting abstractions already exist in `src/commands/skill.rs` but are tightly coupled to the
suggest-install flow via the `SuggestInstall*` prefix. This change extracts them into reusable,
command-agnostic abstractions and wires the three single-operation commands to use them.

## Scope

### In Scope

- Extract shared formatting types from `src/commands/skill.rs` into a reusable module within the
  skill command area (e.g., `src/commands/skill_fmt.rs` or a `src/commands/skill/fmt.rs` submodule)
- Rename `SuggestInstallHumanFormatter` → `SkillHumanFormatter` (or similar command-agnostic name)
- Rename `SuggestInstallLabelKind` → `StatusLabelKind` (or similar)
- Extract `detect_suggest_install_output_mode()` into a generic `detect_output_mode()` that returns
  a simplified enum (`Json`, `HumanColor`, `HumanPlain`) suitable for single-operation commands
  (no `HumanLive` variant needed — spinners stay suggest-install-only)
- Wire `run_install` success: `✔ installed {skill_id}` (green+bold in TTY, plain symbol in non-TTY)
- Wire `run_install` error: `✗ failed {skill_id}: {message}` (red+bold) + `Hint: {remediation}`
- Wire `run_update` success: `✔ updated {skill_id}` (green+bold)
- Wire `run_update` error: `✗ failed {skill_id}: {message}` (red+bold) + `Hint: {remediation}`
- Wire `run_uninstall` success: `✔ uninstalled {skill_id}` (green+bold)
- Wire `run_uninstall` error: `✗ failed {skill_id}: {message}` (red+bold) + `Hint: {remediation}`
- Respect `NO_COLOR`, `CLICOLOR=0`, and `TERM=dumb` in all human output paths
- Keep JSON output (`--json`) completely unchanged — no symbols, no color, same schema
- Update `suggest --install` internals to use the new shared types (rename only, no behavior change)
- Add unit tests for the extracted formatter and output-mode detection
- Update existing contract/integration tests if output strings change

### Out of Scope

- Adding spinners to `install`, `update`, or `uninstall` (single-operation commands don't need them)
- Changing the `SuggestInstallLiveReporter` or spinner worker — those stay suggest-install-specific
- Changing JSON output schema for any command
- Adding progress reporting to single-operation commands
- Changing `skill list` (not yet implemented)
- Changing `skill suggest` (non-install) output format

## Approach

### Phase 1: Extract shared formatting module

Create `src/commands/skill/fmt.rs` (or `src/commands/skill_fmt.rs` if we keep the flat layout) with:

1. **`StatusLabelKind`** enum: `Info`, `Warning`, `Success`, `Failure` — same variants as current
   `SuggestInstallLabelKind`
2. **`SkillHumanFormatter`** struct with `use_color: bool`:
   - `format_label(symbol, label, kind) -> String` — colored or plain
   - `format_heading(heading) -> String` — bold or plain
3. **`OutputMode`** enum: `Json`, `Human { use_color: bool }` — simplified from the current
   three-variant `SuggestInstallOutputMode` (the `HumanLive` variant stays in the suggest-install
   code as a local extension)
4. **`detect_output_mode(json: bool) -> OutputMode`** — reusable TTY + env detection function,
   extracted from `detect_suggest_install_output_mode()` with the live/line distinction removed

### Phase 2: Wire single-operation commands

Replace the `println!` calls in `run_install`, `run_update`, `run_uninstall` with formatter calls:

```rust
// Success example (install)
let mode = detect_output_mode(args.json);
// ... (operation) ...
match mode {
    OutputMode::Json => { /* existing JSON path, unchanged */ }
    OutputMode::Human { use_color } => {
        let fmt = SkillHumanFormatter::new(use_color);
        println!("{} {skill_id}", fmt.format_label("✔", "installed", StatusLabelKind::Success));
    }
}
```

Error paths follow the same pattern with `✗ failed` + `Hint:` on a second line.

### Phase 3: Refactor suggest-install to use shared types

- `SuggestInstallOutputMode` keeps its three variants but the `HumanLine`/`HumanLive` variants
  internally use the shared `SkillHumanFormatter` and `StatusLabelKind`
- `SuggestInstallLabelKind` is removed; all references point to `StatusLabelKind`
- `SuggestInstallHumanFormatter` is removed; all references point to `SkillHumanFormatter`
- `detect_suggest_install_output_mode()` delegates to the shared detection for `use_color` and
  adds the live-vs-line distinction locally

### Phase 4: Tests

- Unit tests for `SkillHumanFormatter`: verify colored vs plain output for each `StatusLabelKind`
- Unit tests for `detect_output_mode()`: verify JSON priority, TTY+env combinations
- Update existing integration/contract tests (`tests/contracts/test_install_output.rs`,
  `tests/test_update_output.rs`) to expect the new symbol-prefixed output format
- Existing suggest-install tests must continue to pass (rename-only refactor)

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/commands/skill.rs` | Modified | Extract formatter, output mode, label types; update run_install/update/uninstall |
| `src/commands/skill_fmt.rs` (new) | New | Shared formatting module: SkillHumanFormatter, StatusLabelKind, OutputMode, detect_output_mode |
| `src/commands/mod.rs` | Modified | Add `pub mod skill_fmt;` (or adjust if restructuring to submodule) |
| `tests/contracts/test_install_output.rs` | Modified | Update expected human output to include `✔ installed` / `✗ failed` |
| `tests/test_update_output.rs` | Modified | Update expected human output to include `✔ updated` / `✗ failed` |
| `openspec/specs/skill-lifecycle/spec.md` | Modified | Update REQ-015 (CLI Output Format) to reflect new styled output |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Breaking existing integration/contract tests that assert on exact output strings | High | Identify all tests asserting on human output before implementation; update in the same PR |
| Subtle color escape code differences across terminals | Low | The `colored` crate already handles this; we reuse its existing integration |
| Downstream scripts parsing human output break | Low | Human output is not a stable contract (JSON is); document the change in CHANGELOG |
| Renaming types breaks compile in unexpected places | Low | Compiler will catch all references; run `cargo check` after rename phase |

## Rollback Plan

1. Revert the PR branch — all changes are additive refactoring in a single module area
2. No data migration, no config changes, no external API changes
3. If partially landed, the old `SuggestInstall*` types can coexist with new types temporarily —
   there is no runtime state dependency

## Dependencies

- None. All required crates (`colored`, `std::io::IsTerminal`) are already in the dependency tree.
- No new external dependencies needed.

## Success Criteria

- [ ] `agentsync skill install my-skill` shows `✔ installed my-skill` (green+bold) in TTY
- [ ] `agentsync skill install bad-id` shows `✗ failed bad-id: ...` (red+bold) in TTY
- [ ] `agentsync skill update my-skill` shows `✔ updated my-skill` (green+bold) in TTY
- [ ] `agentsync skill uninstall my-skill` shows `✔ uninstalled my-skill` (green+bold) in TTY
- [ ] All human output respects `NO_COLOR`, `CLICOLOR=0`, and `TERM=dumb` (symbols without color)
- [ ] Non-TTY output shows symbols but no ANSI escape codes
- [ ] `--json` output is byte-identical to current behavior for all commands
- [ ] `suggest --install` behavior is unchanged (visual output identical)
- [ ] No new `SuggestInstall*`-prefixed types remain — all shared types use generic names
- [ ] `cargo test --all-features` passes with zero regressions
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
