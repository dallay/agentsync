# Design: issue-496-async-http-refactor

## Technical Approach

Remove `reqwest::blocking` from the three confirmed call sites and replace with async `reqwest::Client` + Tokio runtime. The `blocking` feature spawns OS threads that contend with Tokio's async scheduler; eliminating it improves throughput under load. The change uses a **runtime-bridge pattern** (already established in `install.rs:244-253`) to bridge async HTTP calls into both synchronous CLI paths and Tokio contexts uniformly.

## Architecture Decisions

### Decision: Runtime bridge strategy for `resolve_via_search`

**Choice**: Apply the `Handle::try_current` bridge pattern to `provider.rs` exactly as used in `install.rs:244-253`.

```rust
// In resolve_via_search(), extract async HTTP to a helper:
async fn resolve_via_search_http(id: &str) -> Result<SkillInstallInfo> {
    let url = format!("https://skills.sh/api/search?q={}", urlencoding::encode(id));
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let resp: SearchResponse = client.get(&url).send().await?.json().await?;
    // ... same match logic ...
}

// Bridge: detect existing runtime, block or spin up
let result = match tokio::runtime::Handle::try_current() {
    Ok(handle) => handle.block_on(resolve_via_search_http(id)),
    Err(_) => {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| anyhow::anyhow!("failed to create runtime: {}", e))?;
        rt.block_on(resolve_via_search_http(id))
    }
};
```

**Alternatives considered**: (a) Convert all callers of `resolve_via_search` to async — rejected; the CLI entry points are sync and changing the whole call tree is out of scope. (b) Use `tokio::task::spawn_blocking` — rejected; `spawn_blocking` is for CPU-bound sync work, not for making async HTTP calls ergonomic.

**Rationale**: Mirrors exactly what `install.rs:244-253` does. The pattern is already reviewed and approved. It handles both cases: called from inside an existing Tokio runtime (e.g., future-proofing) and called from a plain sync thread (current CLI paths).

### Decision: `update_check.rs` spawn strategy

**Choice**: `spawn()` creates a new `tokio::runtime::Runtime` scoped to the task and runs the async check via `Runtime::block_on` on a detached `std::thread`. No detection needed — `main.rs` has no Tokio runtime at all, so the bridge pattern is unnecessary here.

```rust
pub fn spawn() {
    if should_skip_update_check() {
        return;
    }
    std::thread::Builder::new()
        .name("agentsync-update-check".to_string())
        .spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(check_and_notify_async());
        });
}

async fn check_and_notify_async() {
    // fetch_latest_version becomes async fn using async Client
}
```

**Alternatives considered**: (a) Use `tokio::spawn` from main — rejected; `main.rs` has no Tokio runtime, so `tokio::spawn` would panic. (b) Move update check into an async main — rejected; out of scope per proposal. (c) Bridge pattern in `spawn()` — rejected; adds complexity with no benefit since there's no pre-existing runtime to reuse.

**Rationale**: `main.rs:188` calls `spawn()` once at startup on a detached `std::thread`. Creating a dedicated `Runtime` for this one-shot task is the simplest correct approach. `std::thread` is retained for the OS thread wrapper (naming, background behavior) but the actual HTTP work runs on the Tokio runtime.

### Decision: Error context on HTTP failures

**Choice**: Add a new `UpdateCheckError` enum with variants `Timeout`, `Connection`, `HttpStatus`, `ParseError` — replacing the silent `.ok()?` fallthrough in `fetch_latest_version`. Each network variant carries `url` plus a contextual field so diagnostics identify the failing request.

```rust
#[derive(Debug, thiserror::Error)]
pub enum UpdateCheckError {
    #[error("update check timed out after {duration_secs}s for url {url}")]
    Timeout { url: String, duration_secs: u64 },
    #[error("connection failed for {url}: {reason}")]
    Connection { url: String, reason: String },
    #[error("unexpected HTTP status {status} for {url}")]
    HttpStatus { url: String, status: u16 },
    #[error("failed to parse version: {0}")]
    ParseError(String),
}
```

**Alternatives considered**: Using `anyhow` for all errors — rejected; the success criteria requires categorizing errors (timeout vs. connection vs. status). `thiserror` gives structured variants for QA and logging.

**Rationale**: Aligns with `SkillInstallError` in `install.rs` which already uses `thiserror` with `Network` variants. Structured errors make the acceptance criteria verifiable. A timeout detected while decoding the response body (`response.json().await` with `e.is_timeout()`) maps to `Timeout` as well, not `ParseError`.

## Data Flow

```text
main.rs:run()
  └── update_check::spawn()          [std::thread, named "agentsync-update-check"]
        └── tokio::runtime::Runtime  [new, single-use]
              └── rt.block_on(check_and_notify_async())
                    └── async fetch via reqwest::Client (non-blocking)
                          └── Cache read/write (sync, std::fs)

CLI suggest/install commands
  └── SkillsShProvider::resolve()
        └── resolve_via_search()
              └── Handle::try_current()?
                    ├── Ok(handle) → handle.block_on(resolve_via_search_http())
                    └── Err(_)    → Runtime::new().block_on(resolve_via_search_http())

test_catalog_integrity.rs
  └── #[tokio::test] fn catalog_dallay_skill_urls_are_reachable()
        └── async block with reqwest::Client (non-blocking)
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/update_check.rs` | Modify | Replace `fetch_latest_version` with async fn + `async fn check_and_notify`; replace `thread::Builder::spawn` with std thread wrapping Tokio runtime; add `UpdateCheckError` enum |
| `src/skills/provider.rs` | Modify | Extract `async fn resolve_via_search_http`; add bridge pattern to `resolve_via_search`; keep `resolve_deterministic` unchanged (no network) |
| `tests/test_catalog_integrity.rs` | Modify | Change `#[test]` to `#[tokio::test]`; replace `reqwest::blocking::Client` with `reqwest::Client`; `send_request()` becomes `async fn` |
| `Cargo.toml` | Modify | Remove `"blocking"` from reqwest features |

## Interfaces / Contracts

### New error types

**`src/update_check.rs`** — `UpdateCheckError`:
```rust
#[derive(Debug, thiserror::Error)]
pub enum UpdateCheckError {
    #[error("update check timed out after {duration_secs}s for url {url}")]
    Timeout { url: String, duration_secs: u64 },
    #[error("connection failed for {url}: {reason}")]
    Connection { url: String, reason: String },
    #[error("unexpected HTTP status {status} for {url}")]
    HttpStatus { url: String, status: u16 },
    #[error("failed to parse version: {0}")]
    ParseError(String),
}
```

**`src/skills/provider.rs`** — reuse `SkillInstallError::Network` from `install.rs` or add context at call site. Since `provider.rs` currently returns `anyhow::Result`, add context via `.with_context()` rather than a new error enum:

```rust
let resp = client.get(&url).send().await
    .with_context(|| format!("skills.sh search failed for id={}", id))?;
```

### What stays synchronous (documented)

| Function/Path | Reason |
|---------------|--------|
| `src/skills/provider.rs:resolve_deterministic` | Pure URL construction, no network — no reason to async |
| `Cache::load` / `Cache::save` | File I/O on small JSON; blocking is appropriate and fast |
| `install_from_dir`, `install_from_zip`, `blocking_fetch_and_install_skill` | Already async internally via bridge; outer sync boundary is the CLI contract |
| `main.rs` sync entry point | Out of scope; remains synchronous |

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `UpdateCheckError` variants | Test each `thiserror` variant parses correctly; test timeout detection via mock |
| Unit | `resolve_via_search_http` success path | Mock `skills.sh` HTTP response; verify URL construction and subpath logic unchanged |
| Unit | Bridge pattern `Handle::try_current` paths | Unit test that calls `resolve_via_search` from a sync context (existing tests) |
| Integration | Full `fetch_latest_version` with real network | Existing `cargo test` covers cache logic; add `#[tokio::test]` variant that hits crates.io with short timeout |
| E2E | `test_catalog_integrity` against live GitHub API | `RUN_E2E=1` test already exists; convert to `#[tokio::test]` — no functional change to what it validates |

**New test file**: `tests/test_update_check_async.rs` — tests for `UpdateCheckError`:
```rust
#[tokio::test]
async fn test_fetch_latest_version_timeout() {
    // Set very short timeout, verify Timeout variant
}

#[tokio::test]
async fn test_fetch_latest_version_invalid_json() {
    // Mock server returns non-JSON, verify ParseError variant
}

#[tokio::test]
async fn test_fetch_latest_version_404() {
    // Mock server returns 404, verify HttpStatus(404) variant
}
```

## Migration / Rollout

No migration required. This is a pure refactor with no persistent state changes. The rollout sequence:

1. Convert `src/update_check.rs` → verify `cargo test --lib` passes
2. Convert `src/skills/provider.rs` → verify `cargo test --lib` passes
3. Convert `tests/test_catalog_integrity.rs` → verify `RUN_E2E=1 cargo test --test test_catalog_integrity` passes
4. Remove `"blocking"` from `Cargo.toml` → verify `cargo build --all-targets` passes
5. Run full `cargo clippy --all-targets --all-features -- -D warnings` — must be clean

Rollback per proposal: `git checkout HEAD~1 -- Cargo.toml src/update_check.rs src/skills/provider.rs tests/test_catalog_integrity.rs`

## Open Questions

- [ ] None — all decisions are resolved by the proposal and the existing `install.rs:244-253` bridge pattern precedent.
