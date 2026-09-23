# Update notification and version banner

## Goal

Make interactive AgentSync users aware of the installed version and of available releases, with a clear command to update through the installation channel they use.

## Scope

- Add the current package version to the existing human CLI banner.
- Keep the existing asynchronous crates.io update check, cache, timeout, and opt-out behavior.
- Change a detected release notification from an internal-only tracing message into a visible human warning with channel-aware update instructions.
- Preserve non-interactive behavior: no banner/notification on JSON output, CI, non-terminal execution, or `apply --offline`.
- Do not implement automatic self-update.

## Decisions

- Version source: Cargo package metadata via `env!("CARGO_PKG_VERSION")`; npm/package release synchronization remains unchanged.
- Release source: crates.io, using the existing `src/update_check.rs` implementation.
- Installation channel: detect the executable path for npm's `node_modules/.bin` layout; otherwise fall back to Cargo.
- Update commands:
  - npm: `npm install -g @dallay/agentsync@latest`
  - Cargo: `cargo install agentsync`
- Update checks remain best-effort and must never make a CLI command fail.
- The current version is shown only by the existing human banner, not machine-readable output.

## Implementation slices (TDD)

1. **Banner version**
   - Add a failing unit test for rendering the banner with the current version.
   - Change the banner renderer to append a stable version line using `CARGO_PKG_VERSION`.
   - Run the focused output tests.

2. **Visible update notice**
   - Add failing tests for update notice text and channel-specific command selection.
   - Introduce a pure formatter/model for the notice so network and terminal behavior remain separate from presentation.
   - Emit the notice through human terminal output rather than structured logging; keep failures silent/debug-only.
   - Run focused update-check tests.

3. **Startup integration**
   - Ensure the notification is started only for interactive human flows and remains skipped for CI, non-terminal, JSON, and offline apply.
   - Keep the existing banner call sites and avoid changing command semantics.
   - Run CLI/unit checks covering the integrated behavior.

4. **Documentation and verification**
   - Update user-facing documentation only if the current update-check behavior is not already documented.
   - Run `cargo fmt --all -- --check` and focused Rust tests; run broader checks only if a focused failure or changed contract requires it.

## Files expected to change

- `src/output.rs` or `src/banner.txt`: banner rendering/version line.
- `src/update_check.rs`: notice presentation, installation-channel detection, and tests.
- `src/main.rs`: only if startup/output integration requires a small wiring change.
- Documentation only if needed after repository inspection.

## Acceptance criteria

- A human invocation displays `AgentSync`'s current version in the banner.
- A newer stable crates.io version produces a visible warning containing the installed and latest versions plus an actionable update command.
- npm-installed executables receive the npm command; other installations receive the Cargo command.
- No network/update message appears in JSON, CI, non-terminal, or `apply --offline` execution.
- A failed update check does not fail or delay the main command beyond the existing bounded timeout.
- Existing cache and opt-out behavior remain intact.
