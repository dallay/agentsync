# Proposal: background-version-check

## Summary

Implement a non-blocking background version check that queries crates.io on every CLI invocation and displays an update hint when a newer version is available. The check runs on a detached background thread, respects user opt-out via environment variables, and caches results to avoid redundant network calls.

## Intent

Provide CLI users with frictionless awareness of available updates by:
- Checking for new versions in the background without slowing down command execution
- Using a local cache to minimize API calls and enable offline operation
- Outputting a one-time hint when a new version is first detected
- Allowing users to opt out via `AGENTSYNC_NO_UPDATE_CHECK` or `CI` environment variables

## Scope

### In Scope
- Background thread spawning on every CLI invocation (detached, no `join()`)
- crates.io API query: `GET https://crates.io/api/v1/crates/agentsync` with 3s timeout
- Cache file: `~/.cache/agentsync/update-check.json` with 24-hour TTL
- Hint output to stderr, TTY-only, with emoji prefix
- Opt-out via `AGENTSYNC_NO_UPDATE_CHECK=1` or `CI=true` env vars

### Out of Scope
- Automatic updates (no self-update mechanism)
-GUI/TTY progress indicators or interactive prompts
- Cross-crate version comparison (only checks agentsync)

## Approach

Create a new `src/update_check.rs` module with:
- `CheckedVersion` struct: stores `last_checked`, `latest_version`, `notified_version`
- `spawn_version_check()` function: spawns detached thread, performs the check
- Cache I/O: read/write JSON with serde
- Version comparison: parse with `semver::Version`, skip pre-releases
- TTY detection: use `is-terminal` crate to check stderr

Flow:
1. Early in `main()`: call `spawn_version_check()` before CLI parsing
2. Background thread checks opt-out conditions first
3. If cache is fresh (<24h) and already notified → skip HTTP
4. Fetch crates.io API; compare versions
5. If newer: write cache, print hint to stderr (once per version)
6. Thread exits naturally on process termination

## Affected Modules/Packages

| File | Change |
|------|--------|
| `src/main.rs` | Call `update_check::spawn_version_check()` after tracing init |
| `Cargo.toml` | Add `is-terminal = "0.4"` dependency |
| `src/lib.rs` | Add `pub(crate) mod update_check` |
| `src/update_check.rs` | New file (~150-200 LOC) |

## Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| is-terminal | 0.4 | Detect if stderr is a TTY |
| reqwest (blocking) | existing | HTTP client for crates.io API |
| semver | existing | Parse and compare versions |
| serde / serde_json | existing | Cache serialization |
| colored | existing | TTY-colored hint output |

No new dependencies beyond `is-terminal`. All other primitives already exist in the crate.

## Rollback Plan

1. Remove `update_check::spawn_version_check()` call from `src/main.rs`
2. Remove `pub(crate) mod update_check` from `src/lib.rs`
3. Delete `src/update_check.rs`
4. Remove `is-terminal` from `Cargo.toml`
5. Run `cargo build` to verify compilation
6. All changes are additive; no migration or data cleanup required

The cache file at `~/.cache/agentsync/update-check.json` can remain; it will be ignored on next run.

## Open Questions

- Should we include pre-release versions in comparison? (e.g., `0.4.0-beta.1` vs `0.3.1`) — **Decision: skip pre-releases**
- What if the cache directory doesn't exist? — **Decision: create parent dirs via `std::fs::create_dir_all`**
- Should we add a `--check-updates` flag for manual checks? — **Deferred to future iteration**

---

*Proposal status: ready for review*