# Revert command — #630

Issue: https://github.com/dallay/agentsync/issues/630
Spec temporal: `tmp/plans/2026-10-04-revert-command-design.md`
Plan táctico: `tmp/plans/2026-10-04-revert-command-implementation.md`
Estado: Working (implementación TDD en curso — Task 1 RED)

## Tareas

- [x] RPI-001 Test de regresión tracer bullet (tests/test_revert_cli.rs) — RED visto (`unrecognized subcommand`), GREEN
- [x] RPI-002 `Commands::Revert` + `handle_revert` en `src/main.rs` (+ renders, merge fn, literales)
- [x] RPI-003 `Linker::revert()` 4 tipos + restore `.bak` + `agent_selected` compartido con apply
- [x] RPI-004 Remoción de MCP gestionados (solo nuestros servers) — trait `remove_servers` + 6 impls + 4 bails + `remove_all` + fase en handler + 5 tests
- [x] RPI-005 `cleanup_gitignore` en revert (solo runs sin filtro) + renders
- [x] RPI-006 Flags `--dry-run --agents --keep-backups --verbose` + locks CLI (5 tests)
- [x] RPI-007a Documentar Part 1: `reference/cli.mdx`, `troubleshooting.mdx`, `README.md`, doc comments, `--help` — docs build OK
- [x] RPI-007b Documentar Part 2: `guides/mcp.mdx` nota de limitaciones — docs build OK (17 págs)
- [x] RPI-008 Verificación final: fmt OK, clippy `-D warnings` OK, suite completa verde salvo 2 fallos ambientales pre-existentes (probado en árbol prístino vía stash: `test_catalog_integration::phase1_bobmatnyc_*` necesitan checkout hermano/red)

## Criterios de aceptación (de #630)

- apply → revert restaura originales byte-idénticos, sin symlinks, sin `.bak` (salvo `--keep-backups`)
- apply → revert en proyecto limpio borra todo lo generado
- `--dry-run` no escribe; `--agents claude` solo toca claude
- Regulares no gestionados jamás se tocan
- Tests por los 4 tipos + MCP + gitignore

## Evidencia

- RED tracer: `error: unrecognized subcommand 'revert'` (test_revert_cli, pre-implementación)
- GREEN: `cargo test --test test_revert_cli` 4/4, `--lib` 634/634, `--bin agentsync` 191/191
- `cargo check --all-targets` limpio; `cargo clippy -p agentsync --lib` limpio
- Docs: `astro build` 17 páginas OK (bypass a `pnpm prepare` roto del entorno: falta binario npm agentsync, no relacionado)
- TDD honesty note: flags `--dry-run/--agents/--keep-backups` se cablearon en Task 2 y se blindaron con 4 tests de caracterización en Task 5; la lógica de filtro (`agent_selected`) es compartida con apply y quedó cubierta por los tests existentes de apply (631 lib verdes tras el refactor).
- Part 2 TDD: RED compile-break (10 formatters sin `remove_servers`), RED unit (skip-test falló por comparación de strings → fix a intersección semántica), GREEN 87 lib MCP + 5 CLI.
- Final: fmt OK, clippy `-D warnings` OK, `test_cli_tui_compatibility_contract` OK, `astro build` 17 págs OK. Suite: todo verde menos `test_catalog_integration::phase1_bobmatnyc_*` (2), que fallan idéntico en árbol prístino (stash) — ambiental, fuera de alcance.

## Progreso

- 2026-10-04: issues #630/#631 creadas; alcance (completo con MCP) y semántica MCP (remover servers) aprobados; spec temporal escrito y auto-revisado.

## Siguiente paso

- ~~Usuario revisa spec temporal y confirma → invocar `writing-plans` para plan táctico → implementar RPI-001.~~ Hecho: spec + plan aprobados.
- En curso: Task 1 RED (tests/test_revert_cli.rs tracer bullet) → Task 2 GREEN.
