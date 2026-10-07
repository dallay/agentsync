# Remediación tras revisión fresca Round 4

## Ruta y decisión

Delegated direct; nuevo ciclo autorizado expresamente por el usuario. Las PR #632/#633 mantienen su cadena y sus excepciones de tamaño aprobadas; permanecen draft y no se publican cambios.

Contrato MCP aceptado: serializar operaciones de AgentSync por destino y comparar el contenido de nuevo inmediatamente antes de publicar/restaurar. Esto protege frente a otros procesos AgentSync que respetan el lock y detecta ediciones externas observadas por el último compare. No se promete inmunidad ante una escritura concurrente de un editor externo que no coopera con el lock.

## Estado

Plan creado antes de cambios de código. Los RPI autorizados 023c, 020e/f/g/h/i, 026f/h/i, 027b/c/d, 025a y 025b están implementados; cross-check Windows GNU completo pasa. RPI-026g e RPI-026i aún esperan runtime ACL Windows.

## Tareas

- [x] **RPI-023c — Colisión Z-Code:** con fuentes distintas que mapean al mismo `.zcode/commands/foo.md`, revert usa el mismo orden/ganador de `apply`.
- [x] **RPI-020e — Data root:** normalizar la ruta local de estado igual en lock/read y rechazar que el journal resuelto quede dentro de la raíz del proyecto.
- [x] **RPI-020f — Lock por destino:** bloqueo persistente derivado del hash de ruta nativa normalizada serializa AgentSync entre proyectos/owners que comparten una config MCP.
- [x] **RPI-020g — Último compare:** verificar path/bytes tras preparar staging e inmediatamente antes de publicar; mismatch descarta staging y conserva config/journal. Carrera externa no cooperante documentada.
- [x] **RPI-026f — Linux antiguo:** publicación no-replace usa Rustix `linux_raw` en Linux GNU, evitando el wrapper glibc `renameat2`; unsupported preserva `.bak`.
- [ ] **RPI-026g — ACL Windows:** preservar DACL/ACL al restaurar archivos/directorios o fallar seguro antes de publicar si no se pueden preservar.
- [x] **RPI-026h — Restore normal por move:** cuando `--keep-backups=false`, mover el `.bak` original con `rename_exclusive` para preservar metadata y restaurar archivos unreadable; staged copy queda para `--keep-backups`.
- [x] **RPI-026i — DACL heredado:** en `--keep-backups`, rechazar DACL no protegida/heredada antes de publicar; conservar `.bak` y dejar destino ausente. Para DACL protegida, mantener copia verificada.
- [x] **RPI-025b — Permisos `.gitignore`:** aplicar el modo existente después de crear el temporal para vencer el filtrado por `umask` (autorizado Round 6).
- [x] **RPI-027d — Clean con lectura fallida:** contar/reportar el target como skip/error y continuar con los demás (autorizado Round 6).
- [x] **RPI-020h — ACL restore MCP Windows:** restringir el temp snapshot antes de escribir credenciales y verificar que el archivo publicado no hereda un DACL más amplio.
- [x] **RPI-020i — Snapshot global cross-project:** si el current no coincide con hashes pre-write/applied del manifest, abortar apply antes de persist/write y conservar snapshot original. Tradeoff aceptado: resolver cambios de proyecto/editores antes de re-aplicar. RED/GREEN y verificaciones en `plan/tasks/mcp-ownership-journal.md`.
- [x] **RPI-025a — Writer `.gitignore` Windows:** quitar el uso de `Permissions::from_readonly(false)` de cfg no-Unix; el cross-check Windows completo ahora pasa.
- [x] **RPI-027b — Contenedor symlink:** `clean` no recorre un destino symlink-contents que apunta a otro directorio interno ni borra sus enlaces.
- [x] **RPI-027c — Clean parcial:** `clean` reporta/counta discovery nested-glob incompleto, continúa con otros targets y no afirma scan completo.
- [ ] **RPI-022 — Cierre:** correr suite completa, docs build si el entorno se repara, revisión ciega fresca y actualizar workflows solo después de autorización para publicar.

### Evidencia RPI-025b

- RED: el test Unix ejecutó el writer real en un subprocesso con `umask(077)`; reemplazar el `.gitignore` de modo `0644` observó `0600` (`left: 384`, esperado `0644` / `420`).
- GREEN: los permisos capturados se aplican tras escribir el temporal privado y antes de sincronizar/publicar. La regresión pasa y verifica además que un archivo nuevo mantiene el modo predeterminado filtrado por umask (`0666 & !077 = 0600`). `cargo test -p agentsync --lib gitignore` (51 passed), fmt check, Clippy `-D warnings` y `git diff --check` pasan.

### Evidencia RPI-027d

- RED: `clean_continues_after_read_failure_for_replaced_symlink_contents_container` falló porque el target omitido reportaba `skipped=0`; el target válido de forma independiente ya se había limpiado (`removed=1`).
- GREEN: el error de lectura suma exactamente un skip, registra un warning y continúa; el contrato queda `removed=1`, `skipped=1`, `errors=0`, preservando el contenedor inseguro y el archivo externo. La prueba enfocada (1 passed), `cargo test -p agentsync --lib clean_` (18 passed), fmt check y Clippy `-D warnings` pasan.

### Evidencia RPI-026i

- RED: `cargo test -p agentsync --lib restore_backup_dacl_policy_accepts_protected_and_rejects_inherited` falló con E0425 porque todavía no existía `validate_keep_backup_dacl`.
- GREEN: el helper acepta DACL protegidas y rechaza las heredadas con una explicación que recomienda restore normal sin `--keep-backups`. La ruta Windows valida el archivo antes de `CopyFileExW` y vuelve a validar el DACL al copiarlo; cada DACL de directorio se valida durante la aplicación staged. Cualquier error impide publicar el staging; el backup permanece y el error se muestra al usuario. Se añadieron regresiones `#[cfg(windows)]` para archivo y directorio, además de mantener el test existente de DACL protegida.
- Verificación local: test de política (1 passed), `cargo test -p agentsync --lib restore_backup_` (7 passed), `cargo fmt --all`, fmt check, Clippy `-D warnings` y `git diff --check` OK. Cross-check completo Windows GNU pasó y compiló tests cfg(windows); runtime Windows ACL no ejecutado.

## Criterios de aceptación

- Revert calcula el mismo target Z-Code que ganó en apply; si no puede deducirlo, deja enlace/backup intactos con skip/error visible.
- Lock/read usan el mismo data root canonicalizado, y el journal de producción no se almacena bajo el proyecto.
- Dos operaciones AgentSync sobre el mismo archivo MCP comparten un lock incluso si pertenecen a raíces de proyecto distintas.
- Una edición detectada después de preparar staging no se pisa; queda journal para recovery. El límite con editores no cooperantes se documenta.
- Restore staged no usa DACL más amplia que la original en Windows; Linux GNU sin wrapper glibc `renameat2` sigue usando operación exclusiva segura.
- `clean` no sigue contenedores symlink y visibiliza scans nested-glob parciales sin abortar otros targets.

## Pronóstico de revisión

| Campo | Valor |
|---|---|
| Presupuesto estándar | 400 líneas cambiadas |
| Workload | Alto; varios comportamientos de core/MCP y plataformas |
| Estrategia | `size-exception` ya autorizada para #632 y #633; conservar cadena actual |
| Capas | #632: Z-Code/clean/restore portable; #633: journal y coordinación MCP |
| Publicación | Ninguna hasta cerrar tests/revisión y recibir autorización explícita posterior |

## Riesgos por resolver

- El lock serializa AgentSync, no editores externos. El compare final reduce la ventana pero no es una operación CAS portable.
- ACL requiere API Windows real; `std::fs::Permissions` no representa DACL.
- El syscall Linux debe funcionar en las arquitecturas Linux soportadas; no atar la solución a una sola ABI.
- El build de docs sigue bloqueado por el `prepare` de pnpm y la instalación Astro incompleta.

## Siguiente paso

Completar runtime Windows de RPI-026g cuando haya runner; después ejecutar suite integrada, docs build si se repara el entorno y revisión doble ciega fresca. No cambiar estados de PR.
