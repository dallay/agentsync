## Exploration: background-version-check

### Current State
The CLI entry point is `src/main.rs` → `fn main()`. It:
1. Initializes tracing subscriber
2. Parses CLI via `clap::Parser`
3. Dispatches to command handlers (`run_skill`, `run_status`, `run_doctor`, etc.)
4. Returns `Result<()>` — on `Ok`, process exits cleanly

Exit flow is clean; no threading exists (only one `thread::sleep` in `linker.rs:2825`).

### Affected Areas
- `src/main.rs` — Spawn the background thread immediately after `tracing_subscriber::fmt::init()`, before CLI parsing (so it runs regardless of command)
- `Cargo.toml` — Add `is-terminal = "0.4"` for TTY detection (or use `atty` crate)
- `src/` — New module `src/update_check.rs` + expose in `src/lib.rs`

### Dependencies Analysis
| Dependency | Available? | Notes |
|---|---|---|
| reqwest (blocking) | Yes | Already has `blocking` feature (Cargo.toml:54) |
| semver | Yes | Already available (Cargo.toml:62) |
| dirs | No | Not in deps — need to use `dirs_next` or implement manually via `home_dir()` + `.cache` |
| is-terminal | No | Not in deps — use `is-terminal = "0.4"` or `atty` |
| colored | Yes | Already available (Cargo.toml:44) |

### Approach
Create `src/update_check.rs` with:
- `CheckedVersion` struct (version, checked_at timestamp, notified_for_version)
- `spawn_version_check()` — spawns `std::thread::Builder::spawn` with `"agentsync-update-check"` name, detached (no `.join()`), uses `reqwest::blocking::Client` with 3s timeout to GET `https://crates.io/api/v1/crates/agentsync`
- Cache file: `~/.cache/agentsync/update-check.json`
- Logic:
  1. Check `AGENTSYNC_NO_UPDATE_CHECK` / `CI` env vars → skip
  2. Check `stderr.is_terminal()` → skip if not TTY
  3. Read cache; if fresh (24h TTL) and already notified for current cached version → skip HTTP
  4. Fetch crates.io API; compare with current binary version via `semver::Version::parse`
  5. If newer → write cache + print hint to stderr (once per version via `notified_for_version` field)
- Import `VERSION` from crate or use `env!("CARGO_PKG_VERSION")`

Module should be `pub(crate)` in lib.rs. Call `update_check::spawn_version_check()` at top of `main()` before any CLI work.

### Risks
- Thread may outlive main process on very fast exits — acceptable since output goes to stderr only
- Cache dir may not exist — use `dirs::cache_dir()` or `home_dir()` with fallback
- crates.io rate limiting — catch errors silently in background thread
- Version string from crates.io may include pre-release (e.g., `1.33.0-beta.1`) — handle with `semver::Version::parse` or fallback gracefully

### Ready for Proposal
Yes. The implementation is straightforward: ~150-200 LOC in a new module, minimal deps, follows existing patterns (blocking HTTP, serde JSON, colored output, chrono timestamps). No breaking changes.
