# Remediación Round 8 — Revert/MCP

## Ruta y autorización

- Ruta: Direct inline, sin SDD. La persona autorizó los seis fixes Round 8 y el traversal/unlink no-follow por handles de `clean`.
- Mantener intactos los cambios locales previos. La autorización inicial no incluía publicación; el 5 de octubre de 2026 la persona autorizó crear un commit convencional y hacer push de la candidata local, sin cambiar el estado de las PRs.
- TDD por corte vertical: una regresión RED, confirmar el fallo esperado, aplicar el fix mínimo y verificar GREEN antes del siguiente hallazgo.

## Tareas y aceptación

- [x] **RPI-029 — Traversal no-follow por handle en clean:** abrir cada componente del destino desde la raíz del proyecto usando directorios-capacidad; rechazar symlinks/reparse points, enumerar desde el handle abierto y hacer remove/rmdir relativo a los handles padre/hijo conservados. Prueba determinista de sustitución concurrente después de abrir: no recorrer ni borrar entradas del directorio reemplazante; documentar cualquier límite de atomicidad sobre la última entrada.
- [x] **RPI-030 — Filtro `pattern` en clean:** no borrar children de `symlink-contents` que apply no habría creado; mantener la regla Z-Code.
- [x] **RPI-031 — Errores de metadata en revert:** tratar sólo `NotFound` como ausencia; registrar los demás errores y bloquear cualquier restauración/limpieza de `.gitignore` que dependa de un revert incompleto.
- [x] **RPI-032 — Salida comprimida:** rechazar symlink final, incluido dangling, y reemplazar atómicamente un archivo regular para que un hard link vecino no sea modificado; el acceso se hace relativo al directorio-capacidad.
- [x] **RPI-033 — NULL DACL:** rechazar DACL nulo en `--keep-backups` antes de publicar; conservar `.bak` y no publicar un archivo de permisos amplios.
- [x] **RPI-034 — Journal MCP Windows:** proteger y verificar directorios, lock y tempfile del manifest con el DACL privado del SID del token; aplicar la ACL al tempfile antes de escribir snapshots.
- [x] **RPI-035 — ACL de `.gitignore`:** preservar owner/group Unix, ACL POSIX/macOS/FreeBSD y DACL Windows antes del reemplazo atómico; si no pueden preservarse/verificarse, fallar sin publicar el temporal.

## Verificación

- Añadir pruebas de regresión antes de cada cambio de producción y observar RED/GREEN en cada tarea.
- Ejecutar primero suites enfocadas; luego `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` y `cargo test --all-features`.
- Compilar los targets soportados de Windows/macOS cuando haya toolchains instalados. No afirmar runtime ACL Windows sin ejecutar en Windows/NTFS; registrar explícitamente esa limitación.
- Revisar diff y estado del repositorio para confirmar que ningún cambio previo ajeno fue descartado.

## Evidencia y estado

- Estado: RPI-029..035 implementados. La persona autorizó commit/push con los riesgos finales explícitos; commit `a9faa7c93b25e4a8e6208abf0198277fee7be677` está publicado en `feat/revert-mcp-630`. No implica aceptar silenciosamente los riesgos como resueltos.
- Decisión técnica: `cap-std`/`cap-fs-ext` 3.4.4 proporcionan apertura no-follow y operaciones relativas a handles. En Linux se copia el atributo POSIX ACL con `xattr` para evitar depender de `libacl`; `exacl` 0.13.0 se usa sólo en macOS/FreeBSD. Cargo resolvió dependencias compatibles con `rust-version = 1.89`.
- Evidencia RPI-029: la regresión `clean_does_not_unlink_children_from_directory_replaced_after_metadata` falló antes del fix al borrar el symlink ajeno, y pasó después con el contenedor reemplazado por un symlink interno tras abrir el handle. `cargo test clean_ -- --nocapture`: 26 unit tests, 2 main tests y 1 integration test relevantes pasaron.
- Evidencia RPI-030: `clean_symlink_contents_respects_target_pattern` falló antes del fix porque `clean` eliminó el child `excluded.md` además del `selected.txt`. Después del fix, `cargo test clean_ -- --nocapture`: 27 unit tests, 2 main tests y 1 integration test relevantes pasaron; el child excluido se conserva. La regla usa el nombre fuente de apply y contempla el mapeo `.agent.md` de Z-Code.
- Evidencia RPI-031: `revert_reports_destination_metadata_errors_instead_of_treating_them_as_absence` falló antes del fix porque el destino se limpió y `errors` quedó en cero. Tras el fix se agregaron pruebas para errores de destino y backup; `cargo test revert_ -- --nocapture` pasó: 25 unit tests, 8 de main y 22 de integración. `revert_should_cleanup_gitignore` ya bloquea cleanup cuando `errors > 0` y sus pruebas pasan.
- Evidencia RPI-032: `sync_refuses_existing_and_dangling_symlink_compressed_outputs` falló antes del fix al seguir/escribir por symlink; `sync_does_not_modify_other_hardlinks_to_compressed_output` falló antes al cambiar el inode compartido. Ambos pasan con apertura no-follow y publicación atómica relativa al handle. `test_sync_compresses_agents_md_when_enabled` también pasa.
- Evidencia RPI-033: `restore_backup_dacl_policy_rejects_null_dacl` falló antes porque `present=true, acl=null` se aceptaba; ahora se rechaza con diagnóstico explícito. Runtime Windows/NTFS no disponible.
- Evidencia RPI-034: `ownership_manifest_is_secured_before_snapshot_bytes_are_written` falló antes porque el tempfile ya contenía 126 bytes en el hook; ahora permisos/ACL se aplican antes de `write_all`. `cargo test ownership_ -- --nocapture`: 7 tests pasan en Linux.
- Evidencia RPI-035: `update_and_cleanup_gitignore_preserve_regular_file_permissions` verifica modo, uid y gid. La prueba de ACL extendida Linux detectó que el filesystem del runner rechaza `system.posix_acl_access` con `EINVAL` y la omite explícitamente; Windows/NTFS y macOS runtime no ejecutados.
- Evidencia de verificación: `cargo fmt --all -- --check`, `git diff --check`, `cargo check --all-targets --all-features`, `cargo clippy --all-targets --all-features -- -D warnings` pasan; `cargo test --all-features`: 728 library, 198 binary + 1 ignored, 124 integration + 2 ignored; demás targets pasan.
- Límite de entorno: Windows GNU cross-check no inició porque falta `x86_64-w64-mingw32-gcc`; target macOS no está instalado. El test ACL POSIX no pudo ejecutar en este filesystem (`EINVAL`).
- Límite residual RPI-029: el handle fija la carpeta abierta y no sigue symlinks; una sustitución del nombre child entre metadata y unlink todavía puede cambiar qué entrada de esa carpeta se elimina. Si la carpeta abierta se renombra fuera del destino durante `clean`, el handle sigue apuntando a ella. No existe unlink portable condicional por identidad de leaf.
- Triage final (hallazgos no resueltos, visibles para revisión humana): ambos revisores señalan el límite de identidad del child/directorio abierto en `clean`; ambos reportan `set_mcp_restricted_permissions` como validación seguida de chmod/DACL path-based. Mirror señala unlink path-based en single/nested-glob/module-map y el CAS MCP contra `persist`; Lens señala owner/group de `.gitignore` Windows y la ventana de creación del tempfile MCP antes de restringir DACL. La carrera CAS MCP permanece fuera de esta ronda, como follow-up separado #635. Los demás no se corrigieron en esta tanda; la persona optó por revisarlos como riesgos explícitos.
- Evidencia de publicación: push no-forzado confirmó `feat/revert-mcp-630` en `a9faa7c93b25e4a8e6208abf0198277fee7be677`; no se modificó el estado draft de las PRs. GitHub informó en el push una alerta existente en `origin/main`: una vulnerabilidad alta en Dependabot #117; no corresponde al diff de esta rama.
- Próximo paso: revisión humana del candidato y decisión sobre los riesgos documentados; PRs continúan draft hasta instrucción separada.
