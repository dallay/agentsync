# Tasks: background-version-check

### 1. Setup

1.1. [x] **Add `is-terminal` dependency to `Cargo.toml`**
   - Add `is-terminal = "0.4"` to the `[dependencies]` section
   - Verify: `cargo check --all-targets --all-features` succeeds

---

### 2. Implementation

2.1. [x] **Create `src/update_check.rs` with module-level items and `CheckedVersion` struct**
   - Add necessary `use` imports: `chrono::{DateTime, Utc}`, `colored::Colorize`, `serde::{Deserialize, Serialize}`, `std::{fs, io, path::PathBuf}`, `semver::Version`, `anyhow::Context`
   - Define `CheckedVersion` struct with fields `last_checked: DateTime<Utc>`, `latest_version: String`, `notified_for_version: Option<String>`
   - Derive `Debug, Serialize, Deserialize]` on `CheckedVersion`
   - Define `Cache` struct with `path: PathBuf` field
   - Derive `Debug, Clone, Copy)]` on `Cache`
   - Verify: `cargo check` shows no errors in the new file

2.2. [x] **Implement `Cache::load()` method**
   - Open the cache file at `self.path`, read its contents, parse JSON into `CheckedVersion`
   - Return `None` on any error: file not found, invalid JSON, missing fields, parse failure
   - Use `fs::read_to_string` and `serde_json::from_str`
   - Verify: `cargo check` compiles without errors

2.3. [x] **Implement `Cache::save()` method**
   - Create parent directories via `fs::create_dir_all(self.path.parent().unwrap())`
   - Write the `CheckedVersion` as JSON to the file
   - Wrap errors in `anyhow`
   - Verify: `cargo check` compiles without errors

2.4. [x] **Implement `spawn()` function and thread workflow**
   - Add `use std::thread;` and `use is_terminal::IsTerminal;`
   - Define `const CACHE_TTL_HOURS: i64 = 24`
   - Define `const CRATES_IO_URL: &str = "https://crates.io/api/v1/crates/agentsync"`
   - Define `pub(crate) fn spawn()` — entry point called from `main()`
   - Inside `spawn()`, check opt-out conditions first:
     - Return early if `AGENTSYNC_NO_UPDATE_CHECK` env var equals `"1"` (case-insensitive)
     - Return early if `CI` env var equals `"true"` (case-insensitive)
     - Return early if `stderr` is not a TTY
   - Spawn a detached thread named `"agentsync-update-check"` using `thread::Builder`
     - Set `.daemon(true)` and call `.spawn().ok()` — drop the handle immediately
   - Inside the thread closure:
     - Build the cache path at `~/.cache/agentsync/update-check.json`
     - Load cache with `Cache { path }.load()`
     - Check cache freshness: if `last_checked` is within 24 hours AND `notified_for_version == latest_version` → return silently
     - If cache miss or stale → proceed to HTTP fetch
   - HTTP fetch: create `reqwest::blocking::Client` with 3s timeout, `GET CRATES_IO_URL`, parse `crate.newest_version` from JSON
   - Version comparison: parse current version from `env!("CARGO_PKG_VERSION")` with `semver::Version::parse`; skip if pre-release (`.pre.is_empty()` returns false); compare: only proceed if remote > current
   - If newer: update `last_checked` to `Utc::now()`, `notified_for_version` to the remote version string, `latest_version` to the remote version string; save cache; print hint to stderr with yellow bold text: `💡 A new version of agentsync is available: {latest} (you have {current}). Run cargo install agentsync to update.`
   - All errors inside the thread: silently drop with `.ok()` or `match` that does nothing
   - Verify: `cargo check --all-targets --all-features` compiles without errors

2.5. [x] **Add unit tests to `src/update_check.rs`**
   - Test `Cache::load()` returns `None` for non-existent path
   - Test `Cache::load()` returns `None` for corrupted JSON
   - Test `Cache::load()` returns `None` for missing fields
   - Test `Cache::save()` creates parent directories
   - Test `Cache::load()` round-trip: save then load produces equivalent data
   - Test version comparison skips pre-release versions
   - Test version comparison detects newer stable version
   - Test version comparison skips when running version >= remote
   - Test that opt-out env vars cause early return (can test the guard logic by spawning and checking no panic)
   - Use `tempfile::TempDir` for cache paths in tests
   - Verify: `cargo test --lib update_check` passes all tests

---

### 3. Integration

3.1. [x] **Add module declaration to `src/lib.rs`**
   - Add `pub(crate) mod update_check;` after the other `pub(crate)` module declarations
   - Verify: `cargo check --lib` succeeds

3.2. [x] **Wire `spawn()` call into `src/main.rs`**
   - In `fn main()`, after `tracing_subscriber::fmt::init()` (line 118) and before `Cli::parse()` (line 119), add `agentsync::update_check::spawn();`
   - Add `use agentsync::update_check;` or inline call `update_check::spawn();`
   - Verify: `cargo build` succeeds, binary runs without panics

3.3. [x] **Verify full build and CLI runs without errors**
   - Run `cargo build --release`
   - Run `./target/release/agentsync --version` and confirm no errors
   - Run `CI=true ./target/release/agentsync --version` and confirm silent behavior
   - Verify: no output to stderr from the update check path

---

### 4. Testing

4.1. [x] **Run full test suite**
   - Run `cargo test --all-features`
   - Verify: all existing tests pass, no regressions

4.2. [x] **Run clippy and format checks**
   - Run `cargo fmt --all -- --check`
   - Run `cargo clippy --all-targets --all-features -- -D warnings`
   - Verify: no warnings or formatting issues

---

## Dependency Notes

- **Setup (1.1)** must complete before **Implementation (2.x)** tasks, because the new `is-terminal` dependency is used in `spawn()`.
- **Implementation (2.1–2.5)** must complete before **Integration (3.x)** tasks, because `update_check::spawn()` must exist to be called.
- **Integration (3.x)** must complete before **Testing (4.x)** tasks, because the full wiring must be in place for end-to-end verification.

(End of file - total 106 lines)
