# Proposal: issue-496-async-http-refactor

## Intent

Remove `reqwest::blocking` usage from the three identified call sites and replace with async `reqwest::Client` + Tokio runtime. The `blocking` feature in reqwest creates OS threads that contend with Tokio's async scheduler — eliminating it improves throughput under load and aligns HTTP operations with the existing async culture in `install.rs`. After migration, the `blocking` feature will be removed from `Cargo.toml`.

## Scope

### In Scope
- Migrate `src/update_check.rs:fetch_latest_version()` from blocking client on a `std::thread` to async client, keeping the named detached thread with a dedicated Tokio runtime via `Runtime::block_on`
- Migrate `src/skills/provider.rs:resolve_via_search()` from blocking client to async client, applying the bridge pattern already established in `install.rs:244-253`
- Migrate `tests/test_catalog_integrity.rs` from blocking client to `#[tokio::test]` with async client
- Remove `blocking` feature from `reqwest` in `Cargo.toml` after all three conversions pass verification
- Add error context to HTTP error types (timeout, connection, status) so failures are diagnosable

### Out of Scope
- Converting `main.rs` to async entry point (remains synchronous; Tokio used as library)
- Changes to `install.rs` (already uses async reqwest internally — no `reqwest::blocking` there)
- Changes to other HTTP clients or networking code beyond the three confirmed files

## Approach

### File-by-File Strategy

#### 1. `src/update_check.rs` — `fetch_latest_version()`
**Current**: `thread::Builder::spawn(check_and_notify)` → blocking reqwest on that thread.  
**Target**: named detached `std::thread` with a dedicated Tokio runtime via `Runtime::block_on` → async reqwest on that runtime.

The `spawn()` function is called once at startup from `main.rs:run()` after `Cli::parse`. Keeping the OS thread wrapper and creating a dedicated Tokio runtime for this one-shot task is the cleanest path — `main.rs` has no Tokio runtime at all, so `tokio::spawn` is not an option there.

Implementation: keep `std::thread::Builder::new().name("agentsync-update-check")` and inside the thread create `tokio::runtime::Runtime::new()` and run `rt.block_on(check_and_notify_async())`. The `fetch_latest_version` function becomes `async fn fetch_latest_version_async()` using `reqwest::Client` (not blocking).

#### 2. `src/skills/provider.rs` — `resolve_via_search()`
**Current**: blocking reqwest called from synchronous CLI paths (suggest/install commands).  
**Target**: async reqwest using the bridge pattern from `install.rs:244-253`.

The bridge pattern checks for an existing Tokio runtime and either runs the async operation inline or spins up a temporary runtime:

```rust
let result = match tokio::runtime::Handle::try_current() {
    Ok(handle) => handle.block_on(async_http_call()),
    Err(_) => {
        let rt = tokio::runtime::Runtime::new().map_err(MyError::AsyncSetup)?;
        rt.block_on(async_http_call())
    }
};
```

This mirrors exactly what `install.rs:244-253` does with `fetch_and_unpack_to_tempdir`. The async HTTP call helper is extracted as `async fn resolve_via_search_http(source: &str) -> Result<...>`.

#### 3. `tests/test_catalog_integrity.rs` — E2E test
**Current**: synchronous test using `reqwest::blocking::Client`.  
**Target**: `#[tokio::test]` using `reqwest::Client`.

The test function becomes `async fn` with `#[tokio::test]`. The blocking client builder is replaced with `reqwest::Client::builder()`. Retry logic stays the same. The test gate remains `RUN_E2E=1`.

#### 4. `Cargo.toml`
After all three conversions verified green, `"blocking"` was removed from the reqwest features:
```toml
# Before
reqwest = { version = "0.13.3", features = ["json", "gzip", "stream", "blocking"] }
# After
reqwest = { version = "0.13.3", features = ["json", "gzip", "stream"] }
```

`tokio` gained the `"time"` feature so `tests/test_catalog_integrity.rs` can call `tokio::time::sleep` directly instead of relying on the transitive feature from reqwest.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/update_check.rs` | Modified | `spawn()` keeps the named thread and gains a dedicated Tokio runtime; `fetch_latest_version` becomes async |
| `src/skills/provider.rs` | Modified | `resolve_via_search` bridges to async; async HTTP helper extracted |
| `tests/test_catalog_integrity.rs` | Modified | `#[tokio::test]` + async reqwest client |
| `Cargo.toml` | Modified | Remove `blocking` feature from reqwest; add `time` feature to tokio |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `resolve_via_search` bridge pattern blocks main thread under heavy load | Low | The `Handle::try_current` path only blocks if called from inside a Tokio runtime; all current callers are sync-only |
| Tokio runtime conflict if `main.rs` ever adds a runtime | Low | Bridge pattern handles this gracefully via `try_current` detection |
| Test timeout flakiness in E2E tests | Low | E2E tests are gated behind `RUN_E2E=1` and use existing 15s timeout |
| Regressions in update check timing/behavior | Low | Covered by existing unit tests + manual verification |

## Rollback Plan

1. `git checkout HEAD~1 -- Cargo.toml src/update_check.rs src/skills/provider.rs tests/test_catalog_integrity.rs` — reverts all four files in one command
2. `cargo build --all-targets` — verify compilation restores original behavior
3. No database or state migration needed — this is a pure code refactor with no persistent state changes

## Dependencies

- `tokio` runtime features: `rt-multi-thread`, `macros`, `fs` were already present; `time` was added for `tokio::time::sleep` in the E2E test
- No new external dependencies
- No changes to config files or environment

## Success Criteria

- [ ] `cargo build --all-targets` succeeds with no warnings
- [ ] `cargo test --all-features` passes
- [ ] Full verification passes with `RUN_E2E=1 make verify-all` (includes the E2E catalog integrity test)
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes clean
- [ ] `reqwest::blocking` is not used anywhere in `src/` or `tests/` after the change
- [ ] HTTP errors in `resolve_via_search` and `fetch_latest_version` carry useful context (timeout vs. connection error vs. non-200 status)
- [ ] `Cargo.toml` no longer includes `"blocking"` in reqwest features
- [ ] Any function that must remain synchronous (if any) is documented with `// SAFETY:` or `// Note: runs on sync path` comment
