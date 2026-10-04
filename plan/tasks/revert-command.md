# Revert command — #630 (Part 1: core revert)

Issue: https://github.com/dallay/agentsync/issues/630
Spec temporal: `tmp/plans/2026-10-04-revert-command-design.md`
Plan táctico: `tmp/plans/2026-10-04-revert-command-implementation.md`
Estado: Part 1 core en esta rama (`feat/revert-core-630`); MCP removal vive en
la rama follow-up (NO en este diff).

Alcance de ESTA rama (Part 1 solamente):

- `Commands::Revert` + `handle_revert` en `src/main.rs` (sin fase MCP)
- `Linker::revert()` 4 tipos + restore `.bak` + `agent_selected` compartido
- `cleanup_gitignore` en revert (solo runs sin filtro) + renders
- Flags `--dry-run --agents --keep-backups --verbose`

Fuera de alcance aquí (rama follow-up): remoción de MCP gestionados
(`remove_servers`, fase MCP en handler, docs `guides/mcp.mdx` Part 2).

## Tareas

- [x] RPI-001 Test de regresión tracer bullet (tests/test_revert_cli.rs) — RED visto (`unrecognized subcommand`), GREEN
- [x] RPI-002 `Commands::Revert` + `handle_revert` en `src/main.rs` (+ renders, merge fn, literales)
- [x] RPI-003 `Linker::revert()` 4 tipos + restore `.bak` + `agent_selected` compartido con apply
- [ ] RPI-004 Remoción de MCP gestionados — MOVIDO a la rama follow-up (no implementado en este diff)
- [x] RPI-005 `cleanup_gitignore` en revert (solo runs sin filtro) + renders
- [x] RPI-006 Flags `--dry-run --agents --keep-backups --verbose` + locks CLI (5 tests)
- [x] RPI-007a Documentar Part 1: `reference/cli.mdx`, `troubleshooting.mdx`, `README.md`, doc comments, `--help` — docs build OK
- [ ] RPI-007b Documentar Part 2 (`guides/mcp.mdx`) — MOVIDO a la rama follow-up
- [x] RPI-008 Verificación final: fmt OK, clippy `-D warnings` OK, suite verde para el alcance Part 1

## Criterios de aceptación (de #630, alcance Part 1)

- apply → revert restaura originales byte-idénticos, sin symlinks, sin `.bak` (salvo `--keep-backups`)
- apply → revert en proyecto limpio borra todo lo generado
- `--dry-run` no escribe; `--agents claude` solo toca claude
- Regulares no gestionados jamás se tocan
- Tests por los 4 tipos + gitignore (MCP: follow-up)

## Evidencia (Part 1 solamente)

- RED tracer: `error: unrecognized subcommand 'revert'` (test_revert_cli, pre-implementación)
- GREEN Part 1: `cargo test --test test_revert_cli`, `--lib revert_*`, `--bin agentsync` verdes
- `cargo check --all-targets` limpio; clippy `-D warnings` limpio
- Docs Part 1: `astro build` OK
- TDD honesty note: flags `--dry-run/--agents/--keep-backups` se cablearon en Task 2 y se blindaron con tests de caracterización en Task 5; la lógica de filtro (`agent_selected`) es compartida con apply y quedó cubierta por los tests existentes de apply.

## Progreso

- 2026-10-04: issues #630/#631 creadas; alcance (completo con MCP) y semántica MCP (remover servers) aprobados; spec temporal escrito y auto-revisado.

## Siguiente paso

- Part 1 (esta rama): revisión + merge del core revert.
- Follow-up (otra rama): RPI-004 + RPI-007b (MCP removal + docs Part 2) que cierran #630.
