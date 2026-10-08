# Cobertura multiplataforma >80% — RPI

## Ruta y autorización

- **Ruta:** Delegated direct, ejecución inline por falta de un agente general de implementación configurado; sin SDD.
- **Autorización:** el usuario aprobó el diseño estricto: tests y cobertura medidos en Linux, macOS y Windows; cobertura global y los dos archivos prioritarios >80% por OS.
- **Plataformas:** Linux, macOS, Windows. FreeBSD y ejecución runtime en todos los release triples quedan fuera de este alcance.
- **TDD:** escribir cada test antes de tocar producción. Si el comportamiento ya es correcto, un PASS inicial que aumente cobertura es válido; si el test descubre un defecto, comprobar RED por la aserción esperada antes del cambio mínimo y GREEN después.
- **Publicación:** el usuario autorizó la cadena oficial GitHub Stack, el rebase de descendientes y la actualización remota con `--force-with-lease`. No se hará merge; la actualización de estas correcciones queda pendiente.

## Capas de revisión aprobadas

Cadena lineal; cada PR depende de la inmediatamente anterior. Budget: 400 líneas cambiadas por layer, comprobado antes de cada commit.

| Posición | Branch | Base | Parent | Alcance | Delta |
|---|---|---|---|---|---:|
| 1/8 | `feat/cov-quarantine-windows` | `main` | `main` | Extraer los tres tests Windows existentes fuera de `quarantine.rs`. | 295 |
| 2/8 | `feat/cov-quarantine-removal` | `feat/cov-quarantine-windows` | `feat/cov-quarantine-windows` | Contratos Unix de remove/directorios y fix fail-closed para reutilización de `dev/ino`. | 400 |
| 3/8 | `feat/cov-quarantine-move` | `feat/cov-quarantine-removal` | `feat/cov-quarantine-removal` | Movimiento/invalid-name, cobertura Windows y tipo explícito para callback Windows. | 392 |
| 4/8 | `feat/cov-unix-ops-seam` | `feat/cov-quarantine-move` | `feat/cov-quarantine-move` | Seam `UnixQuarantineOps` y restore ante unlink inyectado. | 225 |
| 5/8 | `feat/cov-quarantine-fault-removal` | `feat/cov-unix-ops-seam` | `feat/cov-unix-ops-seam` | `FaultOps` para errores de remove/directorio y rollback; el rebase reubicó el test Linux de identity junto a esos contratos. | 324 |
| 6/8 | `feat/cov-quarantine-fault-move` | `feat/cov-quarantine-fault-removal` | `feat/cov-quarantine-fault-removal` | Fallos de move/publish, reemplazo concurrente y restauración. | 370 |
| 7/8 | `feat/cov-doctor-cli` | `feat/cov-quarantine-fault-move` | `feat/cov-quarantine-fault-move` | Ocho tests CLI black-box para Doctor. | 380 |
| 8/8 | `feat/cov-coverage-gates` | `feat/cov-doctor-cli` | `feat/cov-doctor-cli` | CI/Codecov/Sonar y tracker RPI. | 329 |

- Issue: None. Linear URL: None.
- El Stack local y remoto contiene ocho branches en el orden indicado; las PRs #645–#652 siguen draft. Las correcciones Layer 2/3 están committeadas localmente y el rebase descendiente terminó; falta actualizar las refs remotas.
- `backup/cov-stack-before-repair` preserva la punta local anterior al arreglo del layer 3; no forma parte del Stack.

## Estado inicial

- HEAD actual: `3171f32792e78393e854bfdc11b3932b056abd16` (`main`, sincronizada con `origin/main`).
- Baseline textual local Linux antes de tests nuevos: 89.12% líneas, 89.33% regiones, 86.26% funciones; 1202 tests pasaron, 0 fallaron, 6 ignorados.
- `src/linker/quarantine.rs`: baseline Linux 59.46%; antes de separar el Stack el LCOV Linux (`DA`) daba 98.56%; la tabla textual de `cargo llvm-cov report` marcaba 69.67% en `Lines` y 66.16% en `Regions`. Son métricas distintas. La medición inline de 80.98% se descarta por incluir líneas de test.
- `src/commands/doctor.rs`: 8 pruebas black-box multiplataforma nuevas en `tests/test_doctor_cli.rs`; LCOV Linux actual 96.74%. La evidencia macOS/Windows sigue pendiente de CI.
- `codecov.yml` usa `target: auto`, `threshold: 1%`; cobertura CI está solo en Ubuntu.
- Árbol antes del trabajo: archivo preexistente `plan/tasks/revert-review-632-633-remediation.md` sigue sin seguimiento; conservarlo intacto.

## Estado actual

- `UnixQuarantineOps` y `CapStdUnixQuarantineOps` están implementados solo en la ruta no-Windows; Windows conserva sus operaciones por handles. El Stack contiene 39 pruebas Unix y 6 Windows en `src/linker/quarantine_tests.rs`; los tests Unix se ejecutaron localmente.
- LCOV Linux después del fix: `src/linker/quarantine.rs` 413/420 (98.33%), `src/commands/doctor.rs` 504/521 (96.74%) y unión de Rust bajo `src/` 28,543/31,212 (91.45%). La tabla textual informa `quarantine.rs` 69.88% en `Lines` y 66.73% en `Regions`; Doctor 96.62% y 96.06%; `cargo llvm-cov report` totaliza 89.77% en `Lines` y 89.81% en `Regions`. Codecov consume LCOV `DA`.
- `cargo test --all-features`: 1,049 pasaron, 0 fallaron, 5 ignorados. La generación LCOV ejecutó el mismo conjunto; `cargo fmt --all -- --check` y Clippy estricto pasan. La sintaxis YAML pasó con `js-yaml`; el validador oficial de Codecov respondió `Valid!`.
- El test fail-closed se validó en `/proc` (sin creation time) y en ext4/tmpfs. La closure Windows quedó tipada; todavía requiere runner Windows nativo. La ejecución remota de macOS/Windows y Codecov/Sonar para los heads corregidos queda pendiente del nuevo push.

## Aceptación

- [ ] Estado global Codecov supera 80% (target `80.01%`, tolerancia `0%`) usando coverage reports de los tres OS.
- [ ] `src/linker/quarantine.rs` supera 80% individualmente en Linux, macOS y Windows.
- [ ] `src/commands/doctor.rs` supera 80% individualmente en Linux, macOS y Windows.
- [ ] Cada uno de los tres runners produce su flag Codecov; falta de informe/flag es fallo, no éxito silencioso.
- [ ] `cargo fmt`, Clippy estricto y todas las pruebas siguen verdes.
- [ ] No se modifica la compatibilidad para FreeBSD ni la matriz de release triples.

## Tareas

- [x] Revisar líneas no cubiertas en el reporte Linux antes de tocar fuentes. Evidencia: `src/linker/quarantine.rs` deja rutas de error/restore sin ejecutar en 173-227, 253-368, 394-480 y 568-587; `src/commands/doctor.rs` deja incompletos `check_target_sources` (37-100), conflictos (103-139), MCP (141-175), `.gitignore` (178-255), flujo `run_doctor` (267-321) y búsqueda de comandos (361-411). La cobertura macOS/Windows se medirá en sus runners, no se infiere de este reporte Linux.
- [x] Llevar `src/linker/quarantine.rs` por encima de 80% de líneas en Linux (98.33%), manteniendo las pruebas separadas en `src/linker/quarantine_tests.rs`; macOS/Windows pendientes de CI nativa.
- [x] Seam interna autorizada: el adapter Unix de producción conserva `cap_std`/`rustix`; `UnixQuarantineOps` permitirá inyectar fallos I/O solo desde tests, sin API pública ni cambio de semántica.
- [x] Implementar `UnixQuarantineOps` con adapter `CapStdUnixQuarantineOps` y conectarlo a los helpers Unix de remove/remove-directory/move.
- [x] Añadir `FaultOps` y tests para fallos de unlink, open-dir, metadata/entries, remove-dir y rename, verificando rollback y límite de reintentos.
- [x] Añadir pruebas Windows de eliminación/restauración de directorio y movimiento de backup sin sobrescribir destinos; sus resultados quedan pendientes del runner Windows.
- [x] Añadir 8 pruebas black-box de `doctor` en `tests/test_doctor_cli.rs`, ejecutando en subprocessos con `PATH` aislado; Linux LCOV mide 96.74% de líneas en `src/commands/doctor.rs`; macOS/Windows quedan para CI.
- [x] Integrar `cargo-llvm-cov` 0.9.1 en la matriz de tests existente y generar un upload Codecov por `linux`, `macos`, `windows`.
- [x] Ajustar `sonarcloud.yml` para conservar el reporte Sonar Linux sin duplicar el upload Codecov no etiquetado.
- [x] Reemplazar targets Codecov relativos por un piso global y seis status path+flag para los dos archivos críticos; cada status falla si falta su reporte. El YAML pasó el validador oficial.
- [x] Esperar los tres uploads OS con `codecov.notify.after_n_builds: 3` para evitar evaluar statuses antes de completar la matriz.
- [x] Confirmar la métrica del gate: Codecov consume LCOV `DA`; el Stack Linux registra 413/420 líneas para `quarantine.rs`, 504/521 para `doctor.rs` y 91.45% de unión en `src/`. Conservar aparte la cifra regional/textual de `cargo llvm-cov`.
- [x] Ejecutar verificaciones locales: `cargo test --all-features` (1,049/0/5), `cargo fmt --all -- --check`, Clippy estricto, parseo YAML y validador Codecov.
- [ ] Revisar evidencia CI nativa para macOS y Windows; hasta entonces no dar por satisfechos los umbrales por OS.

## Evidencia y riesgos

- Reporte inicial Linux: `/tmp/opencode/agentsync-llvm-cov-final.log`.
- El código multiplataforma no puede validarse completamente desde este host Linux; READY depende de CI macOS y Windows.
- La suma de cobertura debe combinar los tres flags, y cada status por archivo debe filtrar por un solo flag y una sola ruta; comprobarlo con Codecov antes de cerrar.
