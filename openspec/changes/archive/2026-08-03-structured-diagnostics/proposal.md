# Proposal: Structured Diagnostics & Machine-Readable Logging

## Intent

AgentSync carece de observabilidad estructurada. `tracing_subscriber::fmt::init()` (src/main.rs:154) escribe logs a **STDOUT** — **bug latente**: cualquier `warn!`/`error!` durante `--json` corrompe el contrato machine-readable; los contract tests pasan solo porque los happy paths no emiten WARN+. Sin spans, sin feature `json` (Cargo.toml:58), `apply/clean/init/doctor` con `println!` directo. Issue #499: logs a stderr, output funcional en stdout, JSON estable, sin secretos, CI-usable.

## Scope

### In Scope
- Subscriber con `.with_writer(io::stderr)` (fix del bug latente).
- Flags globales `--log-format <human|json>` + `--log-level` opcional; feature `json` de tracing-subscriber.
- Spans en apply/clean/init/status/doctor/skill/mcp con campos `operation`, `outcome`, `agent_id`, `target`, `path`, `skill_id`; verbose `println!`→`info!`/`debug!`.
- Contexto de error (apply.rs:103-106, main.rs:380-383) con `agent_id`/`path`.
- Redacción: nunca loguear MCP `env`/`headers` ni URLs con credenciales.
- Tests: asserts de stderr nuevos; contracts de stdout intactos.

### Out of Scope
- Exit codes (doctor exit 0, status exit 1).
- Unificar la emisión de todo el output funcional (refactor mayor).
- Envelope `{error, code, remediation}` de skill — ya es contrato.
- Spans INFO por archivo (ruido; se usa `debug!`).
- Interleaving de prompts dialoguer con logs en stderr (se acepta, solo TTY).

## Capabilities

### New Capabilities
- `cli-diagnostics`: spans con campos, flag `--log-format human|json`, logs→stderr, redacción, CI-usability.

### Modified Capabilities
- `cli-output`: nuevo requisito — logs MUST ir a stderr y stdout MUST contener solo salida funcional; contrato JSON intacto.

## Approach

Reemplazar `fmt::init()` (main.rs:154) por builder: `fmt().with_writer(io::stderr)` o `.json().with_writer(io::stderr)`, filtrado por `--log-level`/`RUST_LOG`. Flags globales en `struct Cli` leídos en `main()` antes del dispatch; `--json` funcional intacto. `info_span!` + `.in_scope()` en 6-8 puntos. `serde_json` ya está en el árbol.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/main.rs` | Modified | Subscriber→stderr, flags, span raíz |
| `Cargo.toml` | Modified | Feature `json` |
| `src/linker/apply.rs`, `clean.rs` | Modified | Spans, contexto de error |
| `src/mcp.rs` | Modified | Span por agent, redacción |
| `src/commands/skill.rs`, `doctor.rs` | Modified | Spans por operación |
| `src/output.rs` | Modified | Verbose→logs |
| `tests/contracts/` | Modified | Asserts de stderr |
| `website/docs/.../cli.mdx` | Modified | Documentar `--log-format` |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `--log-format json` a stdout rompe contratos | Med | Spec exige `with_writer(io::stderr)` en ambos formatos + tests |
| Interleaving prompt/log en stderr | Med | Default WARN+ en fases interactivas; solo TTY |
| Romper tests de stdout humano | Low | Salida funcional intacta |

## Rollback Plan

Revertir main.rs:154 a `fmt::init()` y quitar flags + spans. La pieza crítica es un one-liner (writer→stderr) sin impacto en salida funcional ni exit codes; resto son adiciones aisladas en un PR único reversible.

## Dependencies

- tracing-subscriber feature `json` (Cargo.toml). `tracing` y `serde_json` ya presentes.

## Success Criteria

- [ ] Ningún evento tracing en stdout durante `--json` (test con WARN inducido); contract tests pasan
- [ ] `apply --log-format json 2>logs.json` → JSON parseable en stderr; `cargo test` + clippy limpios
- [ ] MCP `env`/`headers` y URLs con credenciales ausentes de logs
- [ ] `--log-format` documentado en cli.mdx
