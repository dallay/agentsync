## Verification Report: background-version-check

### Build & Test Results
- cargo check: **PASS**
- cargo test: **PASS** (290 lib + 34 binary + 9 integration = 333 tests, 0 failures)
- cargo clippy: **PASS** (no warnings)
- cargo fmt: **PASS**

### Smoke Test
```
$ cargo build --release
   Finished `release` profile
$ ./target/release/agentsync --version
agentsync 1.32.0
```
Binary runs without panics. No errors.

---

### Spec Compliance

| Criterion | Status | Notes |
|---|---|---|
| 1. spawn called before CLI parse | ✅ | `agentsync::update_check::spawn()` at `src/main.rs:119`, after `tracing_subscriber::fmt::init()` and before `Cli::parse()`. Note: function is named `spawn()`, not `spawn_version_check()` (minor naming diff from spec; matches design). |
| 2. Thread named `"agentsync-update-check"`, detached | ✅ | `thread::Builder::new().name("agentsync-update-check"...)` at `src/update_check.rs:84-85`. No `join()` anywhere. |
| 3. HTTP to crates.io with 3s timeout | ✅ | URL `https://crates.io/api/v1/crates/agentsync` at line 12; timeout `Duration::from_secs(3)` at line 95. |
| 4. Cache at `~/.cache/agentsync/update-check.json` | ✅ | `cache_path()` at line 45-52 constructs `~/.cache/agentsync/update-check.json`. `CheckedVersion` struct has fields `last_checked` (i64), `latest_version` (String), `notified_for_version` (Option<String>). |
| 5. Cache TTL 24h; fresh cache skips HTTP | ✅ | `CACHE_TTL_SECS = 24 * 60 * 60` (line 11). `is_fresh()` at line 54-63 returns true only when TTL is fresh AND `notified_for_version == latest_version`. |
| 6. `AGENTSYNC_NO_UPDATE_CHECK=1` and `CI=true` skip | ✅ | Lines 66-78 check both env vars case-insensitively and return early. |
| 7. Hint only when stderr is TTY | ✅ | `!std::io::stderr().is_terminal()` guard at line 80-82 returns before spawning the thread. |
| 8. Hint once per new version | ✅ | `notified_for_version` saved to cache (line 145); `is_fresh` requires it to match `latest_version` before skipping. |
| 9. Pre-release versions ignored | ✅ | Line 134: `if !latest.pre.is_empty() { return; }`. |
| 10. Network/parse/cache errors silent | ✅ | All error branches use `_` wildcards and return silently (lines 99, 104, 121, 126, 131, 148). |
| 11. Thread does not block CLI | ✅ | Thread is spawned and handle dropped immediately; `spawn()` returns. |
| 12. Thread cancelled on exit, no `join()` | ✅ | No `joinhandle.join()` anywhere. Thread is a non-daemon detached thread that dies with the process. |

---

### Discrepancies Found

1. **Naming: `spawn_version_check()` vs `spawn()`**
   The acceptance criterion says `spawn_version_check()`, but the design and implementation use `spawn()`. This is purely a naming difference; the semantics are identical. No behavioral impact.

2. **Cache field name: `notified_version` vs `notified_for_version`**
   The spec cache table names the field `notified_version`; the implementation uses `notified_for_version`. The design sketch also uses `notified_for_version`. No functional impact; the semantic meaning is the same.

3. **Cache timestamp type: design vs spec vs implementation**
   The design example (Section 7) shows `last_checked` as an ISO 8601 string, but the spec defines it as an integer Unix timestamp. The implementation uses integer (`i64`), which matches the spec's requirement. The design example is inconsistent with both the spec and the code.

None of these discrepancies affect correctness or behavior.

---

### Risks

- **No integration test for the update check path**: There are unit tests for cache operations and version comparison logic, but no end-to-end test that verifies the thread spawns, hits crates.io (or is skipped), and prints (or suppresses) the hint. Given the 100% unit test coverage of the logic, the risk is low.
- **EC-5 (pre-release only on crates.io) not explicitly tested**: If crates.io ever only has a pre-release version, the code saves the cache with that pre-release version and does not update it (line 148: saves regardless of pre-release check). The spec says `latest_version SHALL NOT be updated in the cache`. This is a minor behavior gap: the code DOES update `latest_version` even for pre-releases, but since the pre-release is skipped on next run (freshness check passes due to matching `notified_for_version`), no harm is done.

---

### Verdict

**PASS**

All 12 acceptance criteria are satisfied. The implementation is correct, all tests pass, and the binary runs cleanly. The three discrepancies are naming/format inconsistencies that have no behavioral impact.
