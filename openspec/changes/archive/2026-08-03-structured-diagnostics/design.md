# Design: Structured Diagnostics & Machine-Readable Logging

## Technical Approach

Replace `tracing_subscriber::fmt::init()` (main.rs:154) with an explicit `init_logging()` builder that always writes to `io::stderr` and switches between human/JSON format via global clap flags `--log-format`/`--log-level`. Instrument 8 operation points with `info_span!` + `.in_scope()`, migrate diagnostic `println!` to `info!`/`debug!`, enrich existing error events with structured fields, and add a `redact_url` helper. Functional stdout (human + per-command `--json`) is untouched; this fixes the latent bug where any WARN+ event during `--json` corrupts the machine-readable contract.

    main() ──> Cli::parse() ──> init_logging(format, level) ──> subscriber → stderr
      │
      ├── span root {operation} ──> dispatch handler
      │        └── span per agent {agent_id, operation, outcome}
      │              └── span per target {target, path, outcome}
      ├── error! enriched fields ──> stderr JSON event (envelope unchanged)
      └── println!/render_* ──> stdout (functional output, intact)

## Architecture Decisions

| # | Decision | Option | Tradeoff | Choice |
|---|----------|--------|----------|--------|
| 1 | Where init lives | (a) main.rs inline (b) output.rs (c) **new `src/logging.rs`** | (a) bloats 450-line main (b) mixes formatting with subscriber setup (c) single-responsibility, unit-testable | **(c)** new `logging.rs`: `LogFormat`, `init_logging()`, `redact_url()` |
| 2 | Flag shape | (a) **global args on `Cli`** (b) per-command (c) env-only | (a) one definition, readable before dispatch (b) repetitive (c) issue asks explicit flag | **(a)** `#[arg(long, global = true)]` on `struct Cli` (main.rs:43-54) |
| 3 | Filter source | (a) flag only (b) **flag OR `RUST_LOG`** (c) EnvFilter directives | (a) drops existing RUST_LOG support (b) keeps it, no new feature (c) needs `env-filter` feature + more surface | **(b)** `--log-level` wins; else parse `RUST_LOG` as single LevelFilter; default INFO. Advanced directives out of scope |
| 4 | Spans | (a) `info_span!`+`.in_scope()` (b) `#[instrument]` | (a) no new direct dep, explicit control, no enter/exit leak (b) tracing-attributes proc-macro dep | **(a)** |
| 5 | Error context | (a) **enrich fields only** (b) rewrite message | (a) human text identical, anyhow chain to stderr unchanged (b) breaks existing messages | **(a)** add `agent_id`/`target`/`path`/`config_path` fields |
| 6 | Redaction | (a) **`redact_url` manual** (b) `url` crate | (a) zero new deps, strips userinfo+query (b) robust parse but new dep | **(a)** manual strip: `user:pass@` up to `@`, drop `?`-query |
| 7 | Span tests | (a) **black-box stderr tests** (b) `tracing-test` dev-dep | (a) contract-level, zero new deps (b) in-process capture, adds dep | **(a)** run binary, assert stderr JSON lines |

## Span Convention

`info_span!("agentsync", operation, outcome, agent_id, target, path, skill_id, config_path)` — only fields relevant to scope. Levels: INFO spans (command/agent/target outcome), DEBUG per-file detail (create/update/skip), WARN/ERROR failures. Close with `span.record("outcome", ...)` before `.in_scope()` returns. No per-file INFO spans (noise).

## Instrumentation Points

| File:lines | Span / Change |
|-----------|----------------|
| `main.rs` (dispatch 158-215) | Root span per subcommand: `operation = "apply\|clean\|init\|status\|doctor\|skill"`, `outcome` recorded at end |
| `main.rs:274-278` | `println!` "Using config" → `info!(config_path, "Using config")` |
| `main.rs:381` | `error!(%e, ...)` → add `agent_id = agent.name()`, `config_path` |
| `linker/apply.rs:43-109` | Span per agent: `operation="sync", agent_id`; `outcome` ok/skipped/error |
| `linker/apply.rs:47,60,72` | verbose skips → `debug!(agent_id, reason)` |
| `linker/apply.rs:91-108` | Span per target: `target, path`; `outcome` created/updated/skipped/error |
| `linker/apply.rs:93` | verbose "Processing target" → `debug!(target)` |
| `linker/apply.rs:104` | `error!` → add `agent_id`, `path` (target destination) |
| `linker/clean.rs:13-197` | Span per removed path: `operation="remove", path, outcome` |
| `mcp.rs:1497-1522` (`generate_all`) | Span per agent: `operation="mcp", agent_id, outcome`; error (1518) add `config_path` |
| `commands/skill.rs` (`run_install` 1015, `run_suggest` 785-899) | Span `operation="skill_install\|skill_suggest", skill_id, outcome`; reuse `{error,code,remediation}` envelope fields |
| `update_check.rs:137` | Keep `eprintln!` (already stderr; not tracing to avoid JSON noise) |

## Interfaces / Contracts

```rust
// src/logging.rs
pub enum LogFormat { Human, Json }                       // FromStr from "--log-format"
pub fn init_logging(format: LogFormat, level: Option<LevelFilter>); // .with_writer(io::stderr) always;
                                                            // .json() iff Json; filter = level
                                                            // .or_else(RUST_LOG).unwrap_or(INFO)
pub fn resolve_level_filter(flag: Option<LevelFilter>, rust_log: Option<&str>) -> LevelFilter; // pure, testable
pub fn redact_url(url: &str) -> String;                  // strip userinfo + query
```
JSON event (stderr, one line per event):
```json
{"timestamp":"...","level":"INFO","fields":{"operation":"sync","agent_id":"claude","outcome":"ok"},"target":"agentsync::linker::apply"}
```
**Rule**: MCP `env`/`headers` (may hold `Authorization: Bearer …`) are NEVER logged as fields; URLs pass through `redact_url`.

## Testing Strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit (`logging.rs`) | `redact_url` (userinfo, query, plain), `resolve_level_filter`, `LogFormat` parse | pure fn tests |
| Unit (main.rs) | Global flags parse: `agentsync --log-format json apply` | existing `Cli::parse` test module |
| Integration `tests/test_logging.rs` (new) | `apply --log-format json` on fixture → every stderr line parses as JSON with `operation`/`outcome`; stdout human output intact; `--log-level error` suppresses INFO | black-box run + `serde_json` per stderr line |
| Contract (`tests/contracts/`) | **Regression for latent bug**: induce WARN (e.g. MCP restricted-permission path) with `--json` → stdout still parses as one JSON doc, WARN appears only in stderr | extend `test_install_output.rs` / `test_skill_suggest_output.rs` asserts |
| Existing | stdout human/JSON contracts | unchanged — verify no breakage |

## Migration / Rollout

No data migration. Single reversible PR; rollback = restore `fmt::init()` at main.rs:154 and drop flags/spans. Writer→stderr is the critical one-liner; everything else is isolated additions.

## Open Questions

- [ ] Add `env-filter` feature later for target-specific directives? (out of scope now)
- [ ] Should `--log-level` appear in `--help` docs section ordering (auto by clap)?
- [ ] Keep `doctor`/`status` functional stdout as-is (exit codes out of scope per proposal)?
