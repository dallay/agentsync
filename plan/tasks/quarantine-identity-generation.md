# Plan de implementación — Identidad de generación del quarantine

> **Para el agente que implemente:** ejecutar las tareas en orden sobre el Stack existente; cada tarea entrega una capa revisable y verificable.

**Objetivo:** evitar que una entrada reemplazada se confunda con la original cuando el filesystem reutiliza su par `dev/ino`, y restaurar la compilación de los tests Windows.

**Arquitectura:** ampliar `EntryIdentity` con la hora de creación expuesta por `cap_std::fs::Metadata::created()`. Las comparaciones que autorizan una operación destructiva deben exigir que ambas identidades tengan esa señal y coincidan; si falta, no se elimina ni mueve la entrada. Auditar las comparaciones directas de identidad para que no eludan esta política. Tipar explícitamente el argumento `Path` de la closure Windows.

**Stack y presupuesto:** estrategia `github-stacked-prs`; corregir Layer 2 (`feat/cov-quarantine-removal`) y Layer 3 (`feat/cov-quarantine-move`), luego rebasar Layers 3–8 y actualizar las ocho ramas remotas con `--force-with-lease`. Mantener cada layer en ≤400 líneas cambiadas; si alguna excede el presupuesto, detenerse y pedir autorización, sin crear capas ni excepción automáticamente. Mantener las ocho PRs como draft; no hacer merge.

**Precondición de reproducción:** `CARGO_TARGET_DIR` debe apuntar a un directorio ext4 disponible y `AGENTSYNC_LOCAL_SKILLS_REPO` al checkout sibling; confirmar el filesystem con `df -T "$CARGO_TARGET_DIR"` antes del test RED.

---

## Decisiones y evidencia

- El usuario autorizó la política **fail closed**, el rebase y la actualización remota con `--force-with-lease`.
- En ext4, la recreación del symlink reutilizó `dev/ino` en 1.000/1.000 probes; el test `remove_replaced_symlink_if_unchanged_preserves_replacement` falló con el outcome incorrecto. El mismo test pasó 100/100 en `/tmp` tmpfs.
- `ctime` no es señal válida después de quarantine: cambia al renombrar el symlink, archivo o directorio. `created()` cambió al recrear la entrada y se mantuvo al renombrarla en ext4; la comprobación también obtuvo `created()` en tmpfs.
- Rust documenta la fuente como `statx btime` en Linux, `birthtime` en otros Unix y `ftCreationTime` en Windows. Puede no existir en algunos filesystems; en ese caso la operación destructiva debe preservar la entrada y reportar/categorizar el resultado como cambiado/no verificable.
- CI Windows falla durante la compilación de tests en `src/linker/quarantine_tests.rs:195`, antes de ejecutar la prueba enfocada de backup. El compilador identifica la closure declarada en la línea 182 y recomienda tipar su argumento como `&std::path::Path`.
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

## Tarea 2 — Corregir compilación Windows en Layer 3

**Rama:** `feat/cov-quarantine-move` (`9a3e2d9`, PR #647); base `feat/cov-quarantine-removal`.

**Archivo:** `src/linker/quarantine_tests.rs`.

- [x] **Paso 1: conservar el diagnóstico RED de CI.** Windows reportó `implementation of FnOnce is not general enough` para `after_move`; la suite filtrada no llegó a ejecutar el test de backup.
- [x] **Paso 2: aplicar el cambio mínimo:** cambiar `move |_| { ... }` por `move |_: &std::path::Path| { ... }` en `remove_nonempty_directory_if_unchanged_restores_contents`.
- [x] **Paso 3: ejecutar `cargo fmt --all -- --check` y `git diff --check`; el delta de Layer 3 contra su base histórica es 392 líneas. La compilación nativa Windows queda para CI.
- [x] **Checkpoint Layer 3:** commit `7ecdc3b` tipa el callback Windows.

## Tarea 3 — Rebasar, publicar y verificar el Stack

**Ramas descendientes:** `feat/cov-unix-ops-seam`, `feat/cov-quarantine-fault-removal`, `feat/cov-quarantine-fault-move`, `feat/cov-doctor-cli`, `feat/cov-coverage-gates`.

- [x] **Paso 1: guardar puntos de recuperación locales** `backup/quarantine-identity-before-rebase-layer-1` a `-layer-8`; las refs remotas no habían cambiado.
- [x] **Paso 2: con `feat/cov-quarantine-move` activo, ejecutar `gh stack rebase --upstack --no-trunk`**; rebasó Layers 3–8 sin mover `main`. Se resolvió un conflicto mecánico en `quarantine.rs` preservando fail-closed y la seam; otro conflicto de inserción conservó el test de identity y los FaultOps. Layer 5 quedó en 324 líneas.
- [x] **Paso 3: comprobar el grafo.** `gh stack view --json` muestra la cadena lineal de ocho ramas, sus bases inmediatas y PRs abiertas.
- [x] **Paso 4: medir los ocho deltas** contra sus padres: 295, 400, 392, 225, 324, 370, 380 y 329; todos ≤400.
- [ ] **Paso 5: actualizar descripciones de PR #646–#652** con el propósito, tests y deltas nuevos, manteniendo la plantilla y Chain Context; conservar todos los drafts y no hacer merge.
- [x] **Paso 6: actualizar `plan/tasks/cross-platform-coverage-80.md`** con el fail-closed, la closure Windows, el resultado de coverage y los límites de evidencia por OS.
- [ ] **Paso 7: ejecutar `gh stack push` una sola vez** para actualizar las ocho ramas publicadas mediante el mecanismo `--force-with-lease` de Stack. Si hay rechazo o fallo parcial, inspeccionar refs/PRs y no reintentar a ciegas.
- [ ] **Paso 8: verificar CI en los tres sistemas**: tests y generación/subida LCOV Linux, macOS y Windows; cobertura >80% global y en `src/linker/quarantine.rs` y `src/commands/doctor.rs`; checks Codecov/Sonar, build y clippy. Las PRs quedan draft hasta decisión posterior.

## Evidencia de finalización

- Regresión Unix pasa en ext4; identidad ausente no ejecuta la operación destructiva.
- Windows compila la suite; el callback del directorio no vacío y el resto de tests ejecutan.
- `cargo test --all-features`, formato y clippy pasan localmente.
- Los ocho deltas siguen dentro de 400 líneas y el Stack remoto conserva sus ocho PRs draft, en la misma cadena.
- Los resultados macOS/Windows se afirman solo con sus runners nativos; LCOV Linux se regenera tras el fix.

## Estado

- **Ruta:** Delegated direct; sin SDD.
- **Autorización:** fail closed + rebase y actualización remota `--force-with-lease` aprobados explícitamente.
- **TDD:** usar primero la regresión existente que falla en ext4; añadir el caso sin `created`; implementar después.
- **No autorizado:** merge de PRs, cierre de PRs o cambio de objetivo del Stack.
- **Estado actual:** Layers 2/3 corregidas y commiteadas; el rebase local terminó; RPI local actualizada. Faltan los cuerpos de PR, `gh stack push` y CI nativa.
