# Exploration: issue-496-async-http-refactor

### Current State

The codebase uses `reqwest::blocking` for three HTTP operations that need to be converted to async reqwest:

1. **`update_check.rs`** — `fetch_latest_version()` (line 84–106): Background version check against crates.io
2. **`skills/provider.rs`** — `resolve_via_search()` (line 291–344): Skills.sh API lookup for simple skill IDs
3. **`tests/test_catalog_integrity.rs`** — E2E catalog integrity checks (lines 32–35)

---

### Call Chain Analysis

#### 1. `update_check.rs` → `fetch_latest_version()`

**Entry point**: `main.rs` line 188 → `agentsync::update_check::spawn()`

```
main.rs:188: agentsync::update_check::spawn()
  └── update_check.rs:146: pub fn spawn()
        └── spawns detached thread named "agentsync-update-check"
              └── update_check.rs:108: fn check_and_notify()
                    └── update_check.rs:84: fn fetch_latest_version()
                          └── update_check.rs:97: reqwest::blocking::Client::builder()...
```

**Key observation**: This runs on a **dedicated detached OS thread** (`thread::Builder::new().spawn(check_and_notify)`). No Tokio runtime involved — pure std thread with blocking reqwest. The `spawn()` function is called synchronously from `main.rs`'s `run()` function before any command dispatch.

**Is it async?** No — completely synchronous call chain from a background std thread.

---

#### 2. `skills/provider.rs` → `resolve_via_search()`

**Callers** (traced via grep `.resolve(`):

```
suggest.rs:506: provider.resolve(&recommendation.provider_skill_id)
  └── suggest.rs:503-548: install_selected_with_reporter() — sync path
        └── suggest.rs:420-472: install_selected() — returns Result
              └── suggest.rs:390-418: run_suggest_install() — sync
                    └── commands/skill.rs: run_suggest() → sync

commands/skill.rs:1178: self.fallback.resolve(id)
  └── SuggestInstallProvider impl — sync

commands/skill.rs:1304: provider.resolve(skill_id)
  └── SuggestInstallProvider impl — sync

skills/provider.rs:241: provider.resolve(provider_skill_id)?
  └── resolve_catalog_install_source() — sync helper
```

**Key observation**: `resolve_via_search()` is called from **entirely synchronous code paths**. The `Provider` trait methods (`resolve`, `manifest`, `recommendation_catalog`) all return `Result<T>` with no async in sight. The suggest/install flow in `commands/skill.rs` runs on the main CLI thread with no Tokio runtime.

**Is it async?** No — all callers are synchronous.

---

#### 3. `blocking_fetch_and_install_skill()` — NOT in scope but related

**Important finding**: `src/skills/install.rs:244` already uses Tokio for the actual download:

```rust
pub fn blocking_fetch_and_install_skill(...) -> Result<(), SkillInstallError> {
    let tempdir = match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(fetch_and_unpack_to_tempdir(source))?,  // ← async download!
        Err(_) => {
            let rt = tokio::runtime::Runtime::new().map_err(SkillInstallError::Io)?;
            rt.block_on(fetch_and_unpack_to_tempdir(source))?
        }
    };
```

This function is called from `commands/skill.rs:924` via `install_skill_callback` — **inside the synchronous CLI flow**. It bridges sync → async by spawning a Tokio runtime or reusing an existing one.

**Relation to this change**: `blocking_fetch_and_install_skill` already uses async reqwest internally. The blocking `reqwest::Client` in this file is **NOT used** — only `reqwest::Client` (async) at line 5 and `fetch_and_unpack_to_tempdir()` async function.

---

### Cargo.toml Dependencies

**reqwest** (line 55):
```toml
reqwest = { version = "0.13.3", features = ["json", "gzip", "stream", "blocking"] }
```
- Features: `json`, `gzip`, `stream`, `blocking`
- The `blocking` feature is what provides `reqwest::blocking::Client`

**tokio** (line 56):
```toml
tokio = { version = "1", features = ["rt-multi-thread", "macros", "fs", "time"] }
```
- Features: `rt-multi-thread`, `macros`, `fs`, `time`
- Already present for async file operations in `install.rs` (`tokio::fs`, `tokio::io::AsyncWriteExt`)
- `time` is required for `tokio::time::sleep` in `tests/test_catalog_integrity.rs` and must be declared explicitly rather than relying on the transitive feature from reqwest
- **No `#[tokio::main]` in the binary** — main.rs uses a plain `fn main()` / `fn run() -> Result<()>`

---

### Tokio Runtime Usage in Codebase

**7 matches for `tokio::`** in src/:

1. **`src/skills/install.rs:250`**: `tokio::runtime::Handle::try_current()` in `blocking_fetch_and_install_skill` — already bridging sync→async
2. **`src/skills/install.rs:253`**: `tokio::runtime::Runtime::new()` — fallback when no runtime exists
3. **`src/skills/install.rs:451`**: `tokio::fs::File::from_std()` — async file ops
4. **`src/skills/install.rs:455`**: `use tokio::io::AsyncWriteExt` — async write
5. **`src/skills/install.rs:1207`**: `#[tokio::test]` — async test
6. **`src/commands/skill.rs:700`**: `tokio::runtime::Handle::try_current()` in `run_update_inner` — same pattern
7. **`src/commands/skill.rs:707`**: `tokio::runtime::Runtime::new()` — fallback

**No `#[tokio::main]` anywhere** in the main binary. The Tokio runtime is **only used as a library**, not as the main async runtime. The CLI entry point is synchronous.

---

### `test_catalog_integrity.rs` Analysis

**Structure**:
- Single E2E test: `catalog_dallay_skill_urls_are_reachable()`
- Gated behind `RUN_E2E=1` environment variable (never runs in normal CI)
- Uses `reqwest::blocking::Client` with 15s timeout (lines 32–35)
- Iterates over curated registry entries
- For each entry, calls GitHub Contents API to verify `SKILL.md` exists at pinned commit
- Retry logic: retries once after 2s on failure
- Optional `GITHUB_TOKEN` for authenticated requests

**How the blocking client is used**:
```rust
let client = reqwest::blocking::Client::builder()
    .timeout(std::time::Duration::from_secs(15))
    .build()
    .expect("failed to build HTTP client");

// Usage in closure:
let send_request = || {
    let mut req = client.get(&url).header("User-Agent", "agentsync-catalog-integrity-test");
    if let Some(ref token) = github_token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }
    req.send()
};
```

**Sync context**: This is a test file — runs under `cargo test`. No Tokio runtime present in test context. Blocking client is appropriate for test isolation.

---

### Third File Using `reqwest::blocking` — No Others Found

**Confirmed: Only 3 files total use `reqwest::blocking`**:

1. `src/update_check.rs:97` — `reqwest::blocking::Client::builder()`
2. `src/skills/provider.rs:294` — `reqwest::blocking::Client::builder()`
3. `tests/test_catalog_integrity.rs:32` — `reqwest::blocking::Client::builder()`

The skill registry and previous exploration docs confirm this is the complete list.

**Note**: `src/skills/install.rs:5` uses `reqwest::{Client, Error}` (the **async** version, not blocking).

---

### Summary: Entry Point Context

| File | Function | Entry Point | Context |
|------|----------|-------------|---------|
| `update_check.rs` | `fetch_latest_version()` | `spawn()` → detached thread | Sync std thread |
| `provider.rs` | `resolve_via_search()` | CLI commands → suggest/install | Sync, main thread |
| `test_catalog_integrity.rs` | test body | `cargo test` | Sync test runtime |
| `install.rs` | `fetch_and_unpack_to_tempdir()` | `blocking_fetch_and_install_skill()` | Bridges to Tokio async |

---

### Risks

- **`main.rs` has no Tokio runtime**: Converting `resolve_via_search()` to async means callers (all sync) must either spawn a runtime or the function must self-spawn like `blocking_fetch_and_install_skill` does
- **`update_check.rs` already spawns a thread**: Converting to async there is straightforward — keep `std::thread::Builder` with the explicit name `"agentsync-update-check"` and run a dedicated Tokio runtime inside via `Runtime::block_on`
- **`test_catalog_integrity.rs`**: Convert test to `#[tokio::test]` — straightforward
- **`reqwest 0.13.3`**: The blocking client removal after migration must be done carefully — the async client features (`json`, `gzip`, `stream`) remain needed
- **`skills/install.rs` already bridges sync→async**: The pattern of checking `Handle::try_current()` and falling back to `Runtime::new()` is already established and should be replicated

---

### Ready for Proposal

**Yes** — the codebase is well-understood. The refactor is straightforward:

1. `update_check.rs`: Replace blocking client with async client, keeping `std::thread::Builder` + dedicated Tokio runtime via `Runtime::block_on`
2. `provider.rs`: Add Tokio runtime detection (same pattern as `install.rs`) or make callers async
3. `test_catalog_integrity.rs`: Convert to `#[tokio::test]` with async client
4. `Cargo.toml`: Remove `blocking` feature from reqwest after all three conversions
