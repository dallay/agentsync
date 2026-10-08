# Plan de implementación — Identidad de generación del quarantine

> **Para el agente que implemente:** ejecutar las tareas en orden sobre el Stack existente; cada tarea entrega una capa revisable y verificable.

**Objetivo:** evitar que una entrada reemplazada se confunda con la original cuando el filesystem reutiliza su par `dev/ino`, y corregir la apertura Windows del directorio en quarantine.

**Arquitectura:** ampliar `EntryIdentity` con la hora de creación expuesta por `cap_std::fs::Metadata::created()`. Las comparaciones que autorizan una operación destructiva deben exigir que ambas identidades tengan esa señal y coincidan; si falta, no se elimina ni mueve la entrada. Auditar las comparaciones directas de identidad para que no eludan esta política. En Windows, pedir `FILE_LIST_DIRECTORY` en el handle exclusivo existente y construir `CapabilityDir` desde un clon de ese handle, sin reabrir el path ni permitir `FILE_SHARE_DELETE`.

**Stack y presupuesto:** estrategia `github-stacked-prs`; corregir Layer 2 (`feat/cov-quarantine-removal`) y Layer 3 (`feat/cov-quarantine-move`), luego rebasar Layers 3–8 y actualizar las ocho ramas remotas con `--force-with-lease`. Mantener cada layer en ≤400 líneas cambiadas; si alguna excede el presupuesto, detenerse y pedir autorización, sin crear capas ni excepción automáticamente. Mantener las ocho PRs como draft; no hacer merge.

**Precondición de reproducción:** `CARGO_TARGET_DIR` debe apuntar a un directorio ext4 disponible y `AGENTSYNC_LOCAL_SKILLS_REPO` al checkout sibling; confirmar el filesystem con `df -T "$CARGO_TARGET_DIR"` antes del test RED.

---

## Decisiones y evidencia

- El usuario autorizó la política **fail closed**, el rebase y la actualización remota con `--force-with-lease`.
- En ext4, la recreación del symlink reutilizó `dev/ino` en 1.000/1.000 probes; el test `remove_replaced_symlink_if_unchanged_preserves_replacement` falló con el outcome incorrecto. El mismo test pasó 100/100 en `/tmp` tmpfs.
- `ctime` no es señal válida después de quarantine: cambia al renombrar el symlink, archivo o directorio. `created()` cambió al recrear la entrada y se mantuvo al renombrarla en ext4; la comprobación también obtuvo `created()` en tmpfs.
- Rust documenta la fuente como `statx btime` en Linux, `birthtime` en otros Unix y `ftCreationTime` en Windows. Puede no existir en algunos filesystems; en ese caso la operación destructiva debe preservar la entrada y reportar/categorizar el resultado como cambiado/no verificable.
- CI Windows falla durante la compilación de tests en `src/linker/quarantine_tests.rs:195`, antes de ejecutar la prueba enfocada de backup. El compilador identifica la closure declarada en la línea 182 y recomienda tipar su argumento como `&std::path::Path`.
- En el CI Windows del head `a5cb45e`, los tests `remove_empty_directory_if_unchanged_removes_empty_directory` y `remove_nonempty_directory_if_unchanged_restores_contents` fallan con OS error 32 al ejecutar `CapabilityDir::reopen_dir(&file)`, antes de rename/delete. `reopen_dir` abre otra vez `.` con `FILE_SHARE_READ | FILE_SHARE_WRITE`; el handle exclusivo original tiene acceso `DELETE` y no comparte delete. El usuario autorizó corregir este fallo de producción, rebasar y actualizar remotos si Layer 3 permanece ≤400 líneas.
- Estado inicial: worktree limpio en `feat/cov-coverage-gates`; ocho branches lineales y PRs #645–#652 draft dentro del Stack #653.

## Archivos y responsabilidades

| Archivo | Responsabilidad del cambio |
|---|---|
| `src/linker/quarantine.rs` | Capturar `created`, comparar generaciones y fallar cerrado cuando la metadata no permita acreditar identidad. |
| `src/linker/enumerate.rs` | Reemplazar la comparación de identidad de root que autoriza abrir/cachear la capability por la comparación fail-closed. |
| `src/linker/revert.rs` | Auditar comparaciones directas de `EntryIdentity` usadas al verificar copias/restores/staging y hacerlas respetar la misma política. |
| `src/linker/quarantine_tests.rs` | Conservar la regresión de reemplazo y añadir cobertura para identidad no verificable; tipar la closure Windows en Layer 3. |
| `plan/tasks/cross-platform-coverage-80.md` | Actualizar evidencia Linux/Windows y estado CI después de la validación remota. |
| Descripciones de PR #646–#652 | Reflejar los deltas, tests y dependencias resultantes, preservando el formato/template existente. |

## Tarea 1 — Endurecer la identidad en Layer 2

**Rama:** `feat/cov-quarantine-removal` (`68cee08`, PR #646); base `feat/cov-quarantine-windows`.

**Archivos:** `src/linker/quarantine.rs`, `src/linker/enumerate.rs`, `src/linker/revert.rs`, `src/linker/quarantine_tests.rs`.

- [x] **Paso 1: confirmar RED en el branch Layer 2.** Ejecutar el test existente en un directorio `TMPDIR` del filesystem ext4; se observó el fallo en la aserción que espera `RemoveOutcome::Changed`.

```bash
probe=$(mktemp -d "$CARGO_TARGET_DIR/quarantine-red.XXXXXX")
trap 'rm -rf "$probe"' EXIT
test_tmp="$probe"
TMPDIR="$test_tmp" CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
  AGENTSYNC_LOCAL_SKILLS_REPO="$AGENTSYNC_LOCAL_SKILLS_REPO" \
  cargo test --all-features --lib \
  linker::quarantine_tests::unix_tests::remove_replaced_symlink_if_unchanged_preserves_replacement \
  -- --exact --nocapture
```

Esperado: fallo de la aserción del outcome en ext4 por identidad `dev/ino` reutilizada. El test no debe fallar por compilación o fixture.

- [x] **Paso 2: escribir y ejecutar la prueba fail-closed antes del cambio.** En este runner Linux, `/proc/self` no ofrece creation time (`Metadata::created()` devuelve `Unsupported`); el test verifica que esa metadata no autoriza una coincidencia:

```rust
#[test]
#[cfg(target_os = "linux")]
fn identity_without_creation_time_is_not_considered_same_generation() {
    let directory = Dir::open_ambient_dir("/proc/self", ambient_authority()).unwrap();
    let metadata = directory.metadata(".").unwrap();
    assert!(!EntryIdentity::capture(&metadata).matches(&metadata));
}
```

Run: `cargo test --all-features --lib linker::quarantine_tests::unix_tests::identity_without_creation_time_is_not_considered_same_generation -- --exact --nocapture`.

Esperado antes del fix: falla porque `matches` solo compara `device`/`file`. El fixture `/proc/self` fue confirmado localmente sin `created()`.

- [x] **Paso 3: implementar la comparación mínima.** La identidad conserva el timestamp cap-std copiable y comparable:

```rust
struct EntryIdentity {
    device: u64,
    file: u64,
    created: Option<cap_std::time::SystemTime>,
}

fn matches(self, metadata: &cap_std::fs::Metadata) -> bool {
    self.device == metadata.dev() && self.file == metadata.ino()
        && self.created.is_some()
        && self.created == metadata.created().ok()
}
```

`capture` obtiene `metadata.created().ok()`. Las comparaciones directas que autorizaban borrar, mover, publicar o copiar se reemplazaron por `EntryIdentity::matches` sobre la metadata actual. Si la metadata capturada o actual no ofrece `created()`, `matches` devuelve false: preservar la entrada y no completar la operación destructiva.

- [x] **Paso 4: verificar GREEN y el fail-closed.** La regresión de reemplazo en ext4, el test Linux sobre `/proc`, los tests Unix de quarantine y la suite completa pasaron.

```bash
test_tmp="$CARGO_TARGET_DIR"
TMPDIR="$test_tmp" \
CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
AGENTSYNC_LOCAL_SKILLS_REPO="$AGENTSYNC_LOCAL_SKILLS_REPO" \
cargo test --all-features --lib linker::quarantine_tests::unix_tests::
```

- [x] **Paso 5: validar formato, lint y presupuesto de Layer 2.** `cargo fmt --all -- --check`, Clippy estricto y `git diff --check` pasaron; delta final: 400 líneas.
- [x] **Checkpoint Layer 2:** commit `9fab24d` agrupa identidad, regresión y tests fail-closed; no incluye cambios de Layer 3.

## Tarea 2 — Corregir quarantine de directorios en Windows (Layer 3)

**Rama:** `feat/cov-quarantine-move` (`9a3e2d9`, PR #647); base `feat/cov-quarantine-removal`.

**Archivos:** `src/linker/quarantine.rs`, `src/linker/quarantine_tests.rs`.

- [x] **Paso 1: conservar el diagnóstico RED de CI.** Windows reportó `implementation of FnOnce is not general enough` para `after_move`; la suite filtrada no llegó a ejecutar el test de backup.
- [x] **Paso 2: aplicar el cambio mínimo:** cambiar `move |_| { ... }` por `move |_: &std::path::Path| { ... }` en `remove_nonempty_directory_if_unchanged_restores_contents`.
- [x] **Paso 3: ejecutar `cargo fmt --all -- --check` y `git diff --check`; el delta de Layer 3 contra su base histórica es 392 líneas. La compilación nativa Windows queda para CI.
- [x] **Checkpoint Layer 3 inicial:** commit `7ecdc3b` tipa el callback Windows; delta histórico 389 inserciones + 3 eliminaciones = 392 líneas.
- [x] **Paso 4: preservar el RED de CI Windows.** Los dos tests de directorio anteriores fallan en el mismo `reopen_dir` con OS error 32. Este fallo remoto es el RED del cambio: ambos prueban la ruta pública de eliminación/restauración y llegan al punto afectado.
- [x] **Paso 5: corregir la apertura sin debilitar el bloqueo.** Añadir `FILE_LIST_DIRECTORY` al `access_mode` original y sustituir la reapertura por `CapabilityDir::from_std_file(file.try_clone()?.into_std())`. `try_clone` comparte el handle existente, evitando un nuevo chequeo de share mode; mantener `FILE_SHARE_READ | FILE_SHARE_WRITE` y no añadir `FILE_SHARE_DELETE`.
- [x] **Paso 6: verificar localmente y respetar el presupuesto.** `cargo fmt --all -- --check`, `git diff --check` y los 22 tests Unix enfocados pasaron. El `cargo check --all-features --lib --target x86_64-pc-windows-gnu` no pudo completar porque falta `x86_64-w64-mingw32-gcc` para compilar `aws-lc-sys`; esto no verifica el código Windows. El delta real de Layer 3 contra Layer 2 es **exactamente 400 líneas** (393 inserciones + 7 eliminaciones); no añadir más cambios a esa capa. Los dos tests Windows y la suite nativa quedan pendientes de CI.
- [x] **Checkpoint Layer 3 actualizado:** commit `d4f2f00` (`fix: reuse exclusive Windows quarantine directory handle`).

## Tarea 3 — Rebasar, publicar y verificar el Stack

**Ramas descendientes:** `feat/cov-unix-ops-seam`, `feat/cov-quarantine-fault-removal`, `feat/cov-quarantine-fault-move`, `feat/cov-doctor-cli`, `feat/cov-coverage-gates`.

- [x] **Paso 1: guardar puntos de recuperación locales** `backup/quarantine-identity-before-rebase-layer-1` a `-layer-8`; las refs remotas no habían cambiado.
- [x] **Paso 2: con `feat/cov-quarantine-move` activo, ejecutar `gh stack rebase --upstack --no-trunk`**; rebasó Layers 3–8 sin mover `main`. Se resolvió un conflicto mecánico en `quarantine.rs` preservando fail-closed y la seam; otro conflicto de inserción conservó el test de identity y los FaultOps. Layer 5 quedó en 324 líneas.
- [x] **Paso 3: comprobar el grafo.** `gh stack view --json` muestra la cadena lineal de ocho ramas, sus bases inmediatas y PRs abiertas.
- [x] **Paso 4: medir los ocho deltas** contra sus padres: 295, 400, 392, 225, 324, 370, 380 y 329; todos ≤400.
- [x] **Paso 5: actualizar descripciones de PR #646–#652** con el propósito, tests y deltas nuevos, manteniendo la plantilla y Chain Context; conservar todos los drafts y no hacer merge.
- [x] **Paso 6: actualizar `plan/tasks/cross-platform-coverage-80.md`** con el fail-closed, la closure Windows, el resultado de coverage y los límites de evidencia por OS.
- [x] **Paso 7: ejecutar `gh stack push` una sola vez** para actualizar las ocho ramas publicadas mediante el mecanismo `--force-with-lease` de Stack. El push terminó correctamente y los ocho heads remotos coinciden con los locales.
- [x] **Paso 8: respaldar y rebasar localmente.** Se guardaron refs `backup/windows-dir-quarantine-before-rebase-layer-1` a `-layer-8` y se ejecutó `gh stack rebase --upstack --no-trunk` desde Layer 3. No hubo conflictos ni cambios a `main`; Layers 4–8 quedaron sobre el nuevo Layer 3. Los ocho deltas medidos son 295, 400, 400, 225, 324, 370 y 380 líneas para Layers 1–7, y 329 para Layer 8.
- [x] **Paso 9: verificar el head rebasado local.** `cargo fmt --all -- --check`, `git diff --check`, `cargo clippy --all-targets --all-features -- -D warnings` y los tests de quarantine pasaron (39/39).
- [ ] **Paso 10: publicar una sola vez con `gh stack push`**; actualización remota con `--force-with-lease` autorizada. Verificar heads/bases, plantilla de PR, drafts y que no hubo merge.
- [ ] **Paso 11: verificar CI en los tres sistemas**: tests y generación/subida LCOV Linux, macOS y Windows; cobertura >80% global y en `src/linker/quarantine.rs` y `src/commands/doctor.rs`; checks Codecov/Sonar, build, clippy y demás jobs. Las PRs quedan draft hasta decisión posterior.

## Evidencia de finalización

- Regresión Unix pasa en ext4; identidad ausente no ejecuta la operación destructiva.
- Los dos tests Windows de directorio dieron RED en CI antes del fix; deben dar GREEN en CI nativo después del rebase/publicación. El chequeo cruzado local quedó bloqueado por falta de MinGW para `aws-lc-sys`.
- `cargo test --all-features`, formato y clippy pasan localmente.
- Los ocho deltas siguen dentro de 400 líneas y el Stack remoto conserva sus ocho PRs draft, en la misma cadena.
- Los resultados macOS/Windows se afirman solo con sus runners nativos; LCOV Linux se regenera tras el fix.

## Estado

- **Ruta:** Delegated direct; sin SDD.
- **Autorización:** fail closed, fix de handle Windows, rebase y actualización remota `--force-with-lease` aprobados explícitamente.
- **TDD:** las dos regresiones Windows existentes dieron RED en el runner nativo antes de cambiar producción; el fix está implementado y su GREEN nativo está pendiente.
- **No autorizado:** merge de PRs, cierre de PRs o cambio de objetivo del Stack.
- **Estado actual:** fix Windows de directory quarantine implementado en Layer 3 como `d4f2f00`, delta exacto 400. Descendientes 4–8 ya fueron respaldados y rebasados localmente sin conflictos; los ocho deltas están dentro de presupuesto. Fmt, Clippy y los tests de quarantine rebasados (39/39) pasaron. El cross-check Windows local no llegó a compilar por falta de MinGW; falta publicar y obtener GREEN del CI nativo. Las PRs siguen draft y no se hará merge.
