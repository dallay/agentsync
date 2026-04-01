# Technical Design: background-version-check

## 1. Architecture

### Module Structure

```
src/
  update_check.rs    # New module (pub(crate))
  lib.rs             # Add: pub(crate) mod update_check
  main.rs            # Add: update_check::spawn() call
```

**Public API surface** (all `pub(crate)`):

- `update_check::spawn()` — Call site: `main()`, fires and forgets
- No other public items; all internals are `pub(super)` or private

### Spawn Point

`srC/main.rs:118` — after `tracing_subscriber::fmt::init()`, before `Cli::parse()`.

**Rationale**: The check must run for every command invocation regardless of which subcommand fires.
Spawning before CLI parsing also means it starts as early as possible, maximizing the window for the
background thread to complete before the process exits. Tracing must be initialized first so the
background thread can use `tracing` for any diagnostics.

---

## 2. Code Structure

### `src/update_check.rs`

#### `CheckedVersion` struct

```rust
struct CheckedVersion {
    last_checked: DateTime<Utc>,
    latest_version: String,
    notified_for_version: Option<String>,
}
```

Fields:

- `last_checked`: when the cache was written
- `latest_version`: the version string returned by crates.io (e.g. `"1.33.0"`)
- `notified_for_version`: the version the user was already notified about (`None` = never notified)

**Rationale**: `notified_for_version` is a separate field so the "once per version" behavior
survives across cache refreshes. The hint only re-appears when a *new* version is detected.

#### `Cache` struct

```rust
struct Cache {
    path: PathBuf,
}
impl Cache {
    fn load(&self) -> Option<CheckedVersion>;
    fn save(&self, v: &CheckedVersion) -> anyhow::Result<()>;
}
```

- `load()` returns `None` on any I/O or parse error (silent drop)
- `save()` creates parent directories via `fs::create_dir_all`; wraps errors in `anyhow`
- File path: `~/.cache/agentsync/update-check.json`

#### `spawn()` — Entry Point

```rust
pub(crate) fn spawn()
```

1. Checks opt-out conditions
2. Spawns a detached thread named `"agentsync-update-check"`
3. Returns immediately; no `JoinHandle` kept

### Thread Work Flow

```
1. Opt-out guard
   └─ AGENTSYNC_NO_UPDATE_CHECK=1 → return
   └─ CI=true → return
   └─ !stderr.is_terminal() → return

2. Load cache
   ├─ Read ~/.cache/agentsync/update-check.json
   ├─ Parse → CheckedVersion
   ├─ If fresh (< 24h) AND notified_for_version == latest_version → return (already told)
   └─ If stale or absent → proceed

3. HTTP fetch (reqwest blocking, 3s timeout)
   ├─ GET https://crates.io/api/v1/crates/agentsync
   ├─ Extract JSON: `crate.newest_version` (semver string)
   └─ On error → return silently

4. Compare versions
   ├─ Parse current: `env!("CARGO_PKG_VERSION")`
   ├─ Parse remote:  `semver::Version::parse`
   ├─ Skip pre-releases (`.pre.is_empty()`)
   └─ If remote > current → proceed

5. Write cache + emit hint
   ├─ Update: notified_for_version = remote version
   ├─ Write ~/.cache/agentsync/update-check.json
   └─ eprintln!(yellow bold hint)
```

---

## 3. Data Flow

```
main()
  └─ update_check::spawn()
        └─ std::thread::Builder::spawn (detached)
              ├─ opt-out check (env + TTY)
              ├─ cache load  ─────────────────────────────────┐
              │   └─ ~/.cache/agentsync/update-check.json     │
              ├─ reqwest::blocking::Client (3s timeout)       │
              │   └─ GET crates.io/api/v1/crates/agentsync   │
              ├─ semver::Version::parse comparison            │
              ├─ cache save  ────────────────────────────────┘
              └─ eprintln!(hint) → stderr (TTY-only)
```

---

## 4. Key Decisions

| Decision                                    | Rationale                                                                                                                             |
|---------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------|
| **Daemon thread (no `join()`)**             | The thread is purely advisory; it must not block or delay process exit. Process termination kills the thread automatically.           |
| **`is_terminal` on `stderr`**               | The hint must not pollute redirected output in scripts/CI. Using `stderr` (not `stdout`) also avoids interfering with command output. |
| **Skip pre-releases**                       | Avoids noisy hints for beta/alpha users when a stable release is behind a pre-release.                                                |
| **`CheckedVersion.notified_for_version`**   | Enables "once per new version" without a separate flag file. The cache itself records what was already seen.                          |
| **Silent error handling**                   | All errors in the background thread are swallowed. No logging, no user-facing errors. The feature is purely advisory.                 |
| **Cache dir creation via `create_dir_all`** | Avoids failure if `~/.cache` doesn't exist on first run.                                                                              |
| **`chrono::DateTime<Utc>` for timestamps**  | Already a dependency (used elsewhere in the crate), provides reliable UTC timestamps for cache freshness checks.                      |
| **`reqwest::blocking::Client`**             | Simpler than async for a single short-lived HTTP call in a background thread. Avoids bringing async runtime concerns into `main`.     |
| **`env!("CARGO_PKG_VERSION")`**             | Compile-time constant — no need to pass the version at runtime. Works reliably in any build.                                          |

---

## 5. Dependencies

| Dependency                  | Change   | Purpose                         |
|-----------------------------|----------|---------------------------------|
| `is-terminal = "0.4"`       | **Add**  | `is_terminal(Stderr)` guard     |
| `reqwest` (with `blocking`) | Existing | crates.io HTTP fetch            |
| `semver`                    | Existing | Version parse + compare         |
| `chrono` (with `serde`)     | Existing | UTC timestamp for cache TTL     |
| `serde` + `serde_json`      | Existing | Cache serialization             |
| `colored`                   | Existing | Yellow bold hint formatting     |
| `anyhow`                    | Existing | Error wrapping in `Cache::save` |

**No new crates needed.** `is-terminal` is the only addition.

---

## 6. Integration Points

### `src/main.rs`

```rust
fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    update_check::spawn();   // <— insert here
    let cli = Cli::parse();
    // ...
}
```

### `src/lib.rs`

```rust
pub(crate) mod update_check;
```

### `Cargo.toml`

```toml
# Add to [dependencies]
is-terminal = "0.4"
```

---

## 7. Cache File Format

`~/.cache/agentsync/update-check.json`:

```json
{
  "last_checked": "2026-03-22T10:00:00Z",
  "latest_version": "1.33.0",
  "notified_for_version": "1.33.0"
}
```

All fields are strings. Missing or malformed cache files are treated as absent cache.

---

## 8. Hint Output

Format (written to stderr, yellow, bold):

```
⚡ A new version of agentsync is available: 1.33.0 (you have 1.32.0). Run `cargo install agentsync` to update.
```

Only emitted once per new version. Subsequent runs with the same cached `latest_version` are silent.

---

## 9. Testing Approach

- **Unit tests** in `src/update_check.rs`: mock the cache path via a `Cache` constructor that
  accepts a `PathBuf`; test TTL logic, pre-release skipping, once-per-version logic.
- **Integration tests** in `tests/`: use `cargo test --test all_tests` with a temporary cache dir to
  verify the spawn doesn't panic and doesn't block.
- **Contract test**: verify the hint output format matches the expected emoji + version pattern.
