# Revert command — #630 (Part 1: core revert)

Issue: https://github.com/dallay/agentsync/issues/630
Spec temporal: `tmp/plans/2026-10-04-revert-command-design.md`
Plan táctico: `tmp/plans/2026-10-04-revert-command-implementation.md`
Estado: Segundo ciclo de corrección/revisión autorizado; PR #632 y PR #633 siguen
en draft hasta cerrar los riesgos y que los workflows actuales terminen en verde.

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
- [x] RPI-009 Evitar que `--keep-backups` copie a través de un symlink que no pudo eliminarse; regresión TDD del rechazo en `restore_backup` y conflictos en dry-run/ejecución real
- [x] RPI-010 Verificar destinos `symlink-contents` usando el mismo `pattern` de `apply`; conservar enlaces excluidos como ajenos
- [x] RPI-011 Rechazar `.gitignore` symlink antes de leer/escribir en update y cleanup; prueba que protege el archivo externo
- [x] RPI-012 No limpiar el bloque `.gitignore` si revert deja errores/destinos gestionados sin revertir; incluye agentes deshabilitados seleccionados por `default_agents`
- [x] RPI-013 No borrar directorios vacíos que `apply` no puede probar que creó; documentar el contenedor vacío conservado
- [ ] RPI-014 No reemplazar archivos MCP configurados como symlink durante persistencia de revert; saltar con aviso seguro
- [ ] RPI-015 Documentar límites de ownership MCP: si el valor actual difiere del esperado se conserva; ownership persistente requiere manifest/follow-up
- [ ] RPI-016 Ejecutar suite local integrada; revisar/responder todos los hilos; esperar workflows GitHub de ambas PRs

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
- RED/GREEN RPI-009: la prueba directa de `restore_backup` primero observó que el contenido del backup sobrescribía el archivo externo por el symlink; después el helper lo rechazó y preservó ambos archivos. El conflicto con destino regular falló en dry-run (reportaba restore) y luego dio `errors=1`, `restored=0` en ambos modos sin alterar destino ni `.bak`.
- RED/GREEN RPI-010: el enlace excluido por `pattern` primero se eliminó; tras aplicar el filtro espejo de `apply`, quedó conservado y contado como skip.
- RED/GREEN RPI-011: update y cleanup tienen pruebas separadas; cada una mostró RED contra el symlink y GREEN devolviendo error sin cambiar bytes externos.
- RED/GREEN RPI-012: las pruebas CLI mostraron la limpieza indebida de `.gitignore` tras error/skip; las pruebas unitarias cubren además `default_agents` que no selecciona un agente deshabilitado, errores y skips.
- RED/GREEN RPI-013: la prueba del contenedor vacío mostró que se borraba antes del cambio y que ahora permanece.
- Verificación repetida localmente: `cargo test -p agentsync --lib` (647 OK), `cargo test --test test_revert_cli` (7 OK), `cargo test --bin agentsync` (198 OK, 1 ignorado), `cargo fmt --all -- --check`, clippy con `-D warnings` limpios. Una ejecución paralela inicial de la suite lib tuvo un fallo transitorio de HTTP local; la repetición serial pasó 647/647.
- Docs tras esta edición: `cd website/docs && ./node_modules/.bin/astro build` OK (con warnings existentes de i18n/404); `pnpm run docs:build` no alcanzó el build porque su instalación automática ejecutó `prepare` y faltó el binario workspace `agentsync`.

## Progreso

- 2026-10-04: issues #630/#631 creadas; alcance (completo con MCP) y semántica MCP (remover servers) aprobados; spec temporal escrito y auto-revisado.
- 2026-10-04: RPI-009..013 cerradas con RED/GREEN por comportamiento; pruebas de enlace/gitignore/selección añadidas, documentación del contenedor vacío actualizada. Para RPI-009, no se pudo inyectar de forma determinista un fallo de `unlink`; se cubrió directamente el rechazo de `restore_backup` ante un destino symlink, más el guard de estado posterior a la eliminación.

## Siguiente paso

- Corregir RPI-014..RPI-015 en la rama MCP, prueba primero.
- Re-ejecutar jueces y workflows; no resolver hilos Semgrep ni quitar draft mientras quede un blocker.
