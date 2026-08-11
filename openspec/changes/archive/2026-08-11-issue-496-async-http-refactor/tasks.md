# Tasks: issue-496-async-http-refactor

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~320-360 |
| 400-line budget risk | Medium |
| Chained PRs recommended | No |
| Suggested split | Single PR — all conversions are in distinct files with no cross-dependency |
| Delivery strategy | ask-on-risk |
| Chain strategy | single-pr |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: single-pr
400-line budget risk: Medium

### Implementation Order
`update_check.rs` first (new error types, self-contained), then `provider.rs` (mirrors install.rs), then `test_catalog_integrity.rs` (independent), then `Cargo.toml` cleanup last (only after all three verify green).

---

## Phase 1: Infrastructure — New Error Type

- [x] 1.1 Add `UpdateCheckError` enum to `src/update_check.rs` with `Timeout { url, duration_secs }`, `Connection { url, reason }`, `HttpStatus { url, status }`, `ParseError(String)` variants using `thiserror`

## Phase 2: Core — update_check.rs

- [x] 2.1 RED: Add test in `src/update_check.rs` — `test_fetch_latest_version_timeout` sets 1ms timeout, expects `UpdateCheckError::Timeout`
- [x] 2.2 RED: Add test — `test_fetch_latest_version_invalid_json` mocks a non-JSON response, expects `UpdateCheckError::ParseError`
- [x] 2.3 RED: Add test — `test_fetch_latest_version_404` returns HTTP 404, expects `UpdateCheckError::HttpStatus(404)`
- [x] 2.4 GREEN: Add `async fn fetch_latest_version_async() -> Result<String, UpdateCheckError>` using `reqwest::Client` (non-blocking) with 3s timeout; map reqwest timeout → `Timeout`, reqwest error → `Connection`, non-200 → `HttpStatus`, JSON parse fail → `ParseError` (a timeout during `.json()` also maps to `Timeout`, not `ParseError`)
- [x] 2.5 GREEN: Add `async fn check_and_notify_async()` wrapping `fetch_latest_version_async()` with cache read/write (sync, std::fs — mark `// Note: sync path`); keep same notification logic
- [x] 2.6 GREEN: Refactor `spawn()` in `src/update_check.rs` — replace `thread::Builder::spawn(check_and_notify)` with `std::thread::Builder::new().name("agentsync-update-check".to_string()).spawn(|| { let rt = tokio::runtime::Runtime::new().unwrap(); rt.block_on(check_and_notify_async()); });`
- [x] 2.7 REFACTOR: Mark `Cache::load` and `Cache::save` with `// Note: sync path` comments per spec
- [x] 2.8 VERIFY: Run `cargo test --lib` — all update_check tests pass

## Phase 3: Core — provider.rs (Bridge Pattern)

- [x] 3.1 RED: Add test in `src/skills/provider.rs` — `test_resolve_via_search_timeout` uses an in-process delayed TCP server (`spawn_delayed_server`) with a 50ms client timeout, expects context-bearing error
- [x] 3.2 RED: Add test — `test_resolve_via_search_invalid_response` uses an in-process TCP server returning non-JSON, expects parse/format error with context
- [x] 3.3 GREEN: In `SkillsShProvider`, extract `async fn resolve_via_search_http(id: &str) -> Result<SkillInstallInfo>` using `reqwest::Client` with 10s timeout; apply `.with_context(|| format!("skills.sh search failed for url={}", url))` on errors
- [x] 3.4 GREEN: Refactor `resolve_via_search()` to use bridge pattern: `match tokio::runtime::Handle::try_current() { Ok(handle) => handle.block_on(resolve_via_search_http(id)), Err(_) => { let rt = tokio::runtime::Runtime::new().map_err(|e| anyhow::anyhow!("failed to create runtime: {}", e))?; rt.block_on(resolve_via_search_http(id)) } }`
- [x] 3.5 REFACTOR: Verify `resolve_deterministic` is unchanged (no network call, no async needed)
- [x] 3.6 VERIFY: Run `cargo test --lib` — all provider tests pass

## Phase 4: Testing — test_catalog_integrity.rs

- [x] 4.1 RED: Change `#[test]` to `#[tokio::test]`; change `fn catalog_dallay_skill_urls_are_reachable()` to `async fn catalog_dallay_skill_urls_are_reachable()`
- [x] 4.2 GREEN: Replace `reqwest::blocking::Client::builder()` with `reqwest::Client::builder()` (non-blocking); add `.timeout(Duration::from_secs(15))`
- [x] 4.3 GREEN: Change `send_request` to `async fn send_request()`; replace `.send()` with `.send().await`; replace retry `std::thread::sleep` with `tokio::time::sleep(Duration::from_secs(2)).await`
- [x] 4.4 REFACTOR: Wrap body in `tokio::test` block with `client` built inside; keep `RUN_E2E` gate and `GITHUB_TOKEN` logic unchanged
- [x] 4.5 VERIFY: Run `RUN_E2E=1 cargo test --test test_catalog_integrity -- --nocapture` — passes

## Phase 5: Cleanup — Cargo.toml

- [x] 5.1 Remove `"blocking"` from `reqwest` features in `Cargo.toml`: `reqwest = { version = "0.13.3", features = ["json", "gzip", "stream"] }`
- [x] 5.2 VERIFY: `cargo build --all-targets` succeeds with no `reqwest::blocking` in tree
- [x] 5.3 VERIFY: `grep -r "reqwest::blocking" src/ tests/` returns no results
- [x] 5.4 VERIFY: `cargo clippy --all-targets --all-features -- -D warnings` is clean

---

### Work-Unit Summary

| Unit | Goal | Scope |
|------|------|-------|
| 1 | `update_check.rs` async conversion + `UpdateCheckError` | `src/update_check.rs` + new tests |
| 2 | `provider.rs` bridge pattern | `src/skills/provider.rs` + new tests |
| 3 | `test_catalog_integrity.rs` tokio::test conversion | `tests/test_catalog_integrity.rs` |
| 4 | Remove `blocking` feature | `Cargo.toml` only (after 1-3 green) |
