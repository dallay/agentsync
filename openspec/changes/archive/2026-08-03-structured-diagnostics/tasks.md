# Tasks: Structured Diagnostics (#499)

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~550-650 |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | PR 1 → PR 2 |
| Delivery strategy | ask-on-risk |
| Chain strategy | pending |

Decision needed before apply: Yes
Chained PRs recommended: Yes
Chain strategy: DECIDED — size-exception aprobada por el usuario (opción B): un solo PR con todo el alcance (~550-650 líneas). Aprobación explícita del dueño del repo.
400-line budget risk: High (mitigado por size-exception)

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Logging infra: `logging.rs`, flags, stderr fix | PR 1 | base=trunk; alone fixes latent bug |
| 2 | Spans, verbose migration, error ctx, contracts, docs | PR 2 | depends on PR 1 |

## Phase 1: Logging Infra (PR 1)

- [x] 1.1 RED: unit test `LogFormat` FromStr (human/json/invalid)
- [x] 1.2 GREEN: `src/logging.rs` — `enum LogFormat` + FromStr
- [x] 1.3 RED: unit test `resolve_level_filter` (flag > RUST_LOG > INFO)
- [x] 1.4 GREEN: implementar `resolve_level_filter`
- [x] 1.5 RED: unit test `redact_url` (userinfo, query, plana)
- [x] 1.6 GREEN: implementar `redact_url`
- [x] 1.7 GREEN: `Cargo.toml:58` — feature `json` de tracing-subscriber
- [x] 1.8 RED: test parse flags — `--log-format json apply`, default human (mod tests main.rs)
- [x] 1.9 GREEN: flags globales `--log-format`/`--log-level` en `struct Cli` (main.rs:51-54)
- [x] 1.10 GREEN: `init_logging(format, level)` — `.with_writer(io::stderr)` siempre, `.json()` condicional; reemplazar `fmt::init()` (main.rs:152-154)
- [x] 1.11 RED: black-box `tests/test_logging.rs` — `apply` sin flags: stderr humano, stdout sin tracing (bug latente)
- [x] 1.12 GREEN: validar 1.11 (stderr fix)

## Phase 2: Instrumentación (PR 2)

- [x] 2.1 RED: `tests/test_logging.rs` — `apply --log-format json`: span raíz con `operation`
- [x] 2.2 GREEN: span raíz por subcomando (main.rs dispatch 158-215), record `outcome`
- [x] 2.3 RED: `tests/test_logging.rs` — skips visibles con `--log-level debug`
- [x] 2.4 GREEN: migrar `println!` diagnóstico → `info!`/`debug!` (main.rs:274-278, apply.rs:47/60/72/93)
- [x] 2.5 RED: `tests/test_logging.rs` — span agente→target con `agent_id`/`target`/`path`/`outcome`
- [x] 2.6 GREEN: spans apply.rs (43-109 agente, 91-108 target)
- [x] 2.7 GREEN: error context apply.rs:104 — añadir `agent_id`+`path` al `error!`
- [x] 2.8 GREEN: span clean.rs (13-197) — `operation="remove"`, `path`, `outcome`
- [x] 2.9 RED: `tests/test_logging.rs` — span mcp `operation="mcp"`; sin `env`/`headers` en eventos
- [x] 2.10 GREEN: span mcp.rs generate_all (1497-1522) + error 1518 `config_path` + `redact_url`
- [x] 2.11 RED: `tests/test_logging.rs` — span skill install con `skill_id`
- [x] 2.12 GREEN: spans skill.rs install/update/uninstall/suggest (1015, 785-899)
- [x] 2.13 GREEN: error context main.rs:381 — añadir `agent_id`+`config_path`

## Phase 3: Contratos, Docs, Verificación

- [x] 3.1 RED: contract regression — WARN inducido (MCP restricted) con `--json`: stdout 1 JSON, WARN en stderr (tests/contracts/)
- [x] 3.2 RED: contract — `status --log-format json --json`: stdout array, stderr logs
- [x] 3.3 GREEN: fijar 3.1-3.2 con asserts de stderr en contracts
- [x] 3.4 Docs: `website/docs/src/content/docs/reference/cli.mdx` — documentar `--log-format`/`--log-level`
- [x] 3.5 Verificación global: `cargo fmt --check`, clippy `-D warnings`, `cargo test --all-features`, contracts, `tests/test_logging.rs`
