# Remediación de findings Round 7

## Ruta y autorización

**Ruta:** Delegated direct. El usuario autorizó corregir los findings aplicables del review final con TDD. No commit, push, cambio de draft ni edición remota. Mantener la cadena #632 → #633.

**Estado:** plan creado antes de cualquier cambio de código y autorizado por el usuario. Cada tarea avanza verticalmente RED → GREEN → verificación enfocada. Los findings son de varios subsistemas; las tareas con archivos compartidos se ejecutan en secuencia.

## Alcance

- [x] **RPI-020j — Identidad Windows:** eliminar el fallback de `USERNAME` a `Users`; aplicar ACL sólo al SID del usuario del token del proceso, o fallar cerrado si no se puede resolver/verificar. No dejar ACEs explícitas amplias.
- [x] **RPI-020k — Stage MCP apply:** restringir y verificar el DACL del temporal Windows antes de escribir bytes MCP; el permiso debe acompañar al archivo publicado. Fallar antes de escribir/persistir si falla la restricción.
- [x] **RPI-023d — Elegibilidad Z-Code común:** `apply`, `clean` y `revert` deben usar la misma regla `.md` para destinos `.zcode/commands`, también cuando la configuración omite `pattern` o usa `*`.
- [x] **RPI-026j — Documentar `--keep-backups`:** la CLI reference indica que Windows staged copy falla cerrado y conserva `.bak` si algún backup tiene DACL heredada/no protegida; recomienda revert normal por move.
- [x] **RPI-026k — Target comprimido de revert:** calcular la ruta que `apply` enlazó (`AGENTS.compact.md`) aunque el archivo comprimido haya desaparecido; no usar el fallback de `expected_source_path` orientado a status.
- [x] **RPI-027e — Errores de inspección en clean:** distinguir `NotFound` inocuo de otros errores de `symlink_metadata`/`is_dir`/`is_symlink`; contar `skipped=1`, advertir y continuar los demás.
- [x] **RPI-027f — Symlink interno en límite de traversal:** `read_contents_entries` debe rechazar el componente final si es symlink inmediatamente antes de enumerar; un symlink a otro directorio interno no se debe seguir ni limpiar.
- [ ] **RPI-023e — Pattern durante clean Z-Code:** cotejar `target.pattern` con el source/destino antes de quitar child symlinks que `apply` no habría creado.
- [ ] **RPI-026m — Symlink final de AGENTS.compact.md:** rechazar un final-component symlink con `symlink_metadata` antes de leer/escribir el output comprimido; cubrir symlink interno y dangling. El writer ya existía en `origin/main`, pero puede sobrescribir otro archivo.
- [ ] **RPI-027h — Metadata al revertir:** tratar `NotFound` como ausencia, reportar otros errores de inspección del destino/backup y no permitir que el gate de `.gitignore` dé éxito si revert no pudo verificar.

## Hallazgos verificados que quedan fuera

- `handle_clean` devuelve éxito aunque `SyncResult.errors > 0`; esta conducta es igual a `origin/main`. Es una decisión de exit-code preexistente y no se cambiará en esta tanda.
- El fallback de DACL `USERNAME → Users:F` también está en `origin/main`, pero el usuario autorizó corregirlo porque el flujo de snapshots actual depende de la promesa owner-only.

## Archivos previstos

- `src/mcp.rs`, `Cargo.toml`: SID del token Windows, helper de DACL, restricción previa a `write_all` y verificación del tempfile.
- `src/linker/apply.rs`, `src/linker/enumerate.rs`, `src/linker/symlinks.rs`: regla común de elegibilidad Z-Code.
- `src/linker/revert.rs`, `src/linker/mod.rs`: resolver el source comprimido realmente enlazado y endurecer el traversal final de `symlink-contents`.
- `src/linker/clean.rs`: manejar explícitamente errores de inspección y mantener la continuidad.
- `website/docs/src/content/docs/reference/cli.mdx`: documentar la restricción Windows de `--keep-backups`.
- Tests unitarios/CLI junto a cada comportamiento y este tracker.

## Criterios de aceptación

- El DACL Windows concedido corresponde al usuario del token, nunca a `Users` como fallback; si falla SID lookup o escritura/verificación DACL, el apply no escribe credenciales.
- El temporal MCP queda restringido antes de `write_all`; el compare final y persist atómico existentes siguen funcionando.
- Con Z-Code y `pattern=None`/`*`, apply omite no-Markdown de `.zcode/commands`, y clean/revert no dejan links/backups residuales de archivos aplicados.
- Si el comprimido desaparece tras apply pero `AGENTS.md` original permanece, revert reconoce el target compacto originalmente enlazado y restaura backup.
- Los errores de metadata de clean aparecen en resultados/log y los otros targets todavía se procesan.
- Una sustitución del contenedor por symlink interno en el seam previo a traversal se rechaza; el directorio interno y sus symlinks ajenos permanecen intactos.
- CLI reference explica DACL heredada, retención del backup y el fallback de revert normal.

## Evidencia RPI-023d / RPI-026k

- **RPI-023d RED/GREEN:** la prueba Linker real `zcode_apply_clean_and_revert_skip_non_markdown_contents_without_pattern` falló antes del cambio porque apply enlazó los dos entries (`created=2`, esperado `1`). GREEN comparte el filtro Z-Code `.md` de clean/revert antes de resolver o crear cada child y cuenta el omitido como `skipped=1`; la prueba confirma apply, clean y revert no dejan enlace no-Markdown.
- **RPI-026k RED/GREEN:** las regresiones reales para `symlink` y `symlink-contents` fallaron inicialmente con `removed=0`: revert usaba el fallback a `AGENTS.md` aunque apply había enlazado `AGENTS.compact.md`. GREEN añade `expected_applied_source_path` solo para targets de revert, deja intacto el fallback de status, y permite computar el relativo al compacto ausente mientras el original exista. Ambos casos quitan el enlace y restauran el `.bak` byte-exacto; si el original no existe, la expectativa sigue siendo `None`.
- **Verificación:** `cargo test -p agentsync --lib zcode` (6 passed), `cargo test -p agentsync --lib revert_symlink_contents` (5 passed), `cargo test -p agentsync --lib compressed_source_is_missing` (2 passed), `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` y `git diff --check` OK.

## Evidencia RPI-027e/f

- **RPI-027e RED/GREEN:** el seam privado `clean_metadata_error_path` inyecta `PermissionDenied` para un path concreto, sin cambiar permisos ni depender de ejecutar como root. Se ejercitaron los cinco probes de clean: contenedor `symlink-contents`, child `nested-glob`, target `symlink`, destino `module-map` y child `symlink-contents`. Cada prueba mostró primero `skipped=0` y eliminación del enlace afectado junto con un target independiente; tras el fix cada una confirma `skipped=1`, `errors=0`, preservación del path no inspeccionable y limpieza del enlace independiente. Los tres paths añadidos inspeccionan una sola vez con `clean_symlink_metadata`; `NotFound` permanece como desaparición inocua, y rutas regulares no-symlink mantienen su comportamiento sin skip.
- **RPI-027f RED/GREEN:** `clean_before_read_contents_hook` cambia determinísticamente el contenedor ya comprobado a un symlink que apunta a un directorio interno con un child symlink al mismo source gestionado. RED eliminó el child ajeno; GREEN rechaza el componente final symlink antes de `read_dir`, conserva child/target/contenedor, cuenta un skip y continúa limpiando otro target válido.
- **Verificación:** `cargo test -p agentsync --lib clean_` (25 passed), `cargo test -p agentsync --lib nested_glob` (20 passed), `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` y `git diff --check` OK. La revalidación continúa siendo path-based; hay una ventana residual entre `symlink_metadata` y `read_dir`, sin garantía de atomicidad a nivel de handle.

## Verificación y publicación

- TDD por unidad; primero tests enfocados y suites de cada módulo.
- Cierre Linux: `cargo test --all-features`, `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `git diff --check`.
- Cierre Windows: `cargo check --target x86_64-pc-windows-gnu --all-targets --all-features --locked --offline` en contenedor MinGW; sin runner Windows no afirmar ejecución de ACL runtime.
- Evidencia actual Round7: el cross-check completo Windows GNU terminó con exit 0 y compiló tests cfg(windows). Reportó warnings de imports/variables cfg-gated y un `unused_mut` en el builder Unix-only de `src/gitignore.rs`; Windows runtime ACL no se ejecutó.
- Fresh double-blind 4R después de estos fixes; no corregir hallazgos nuevos de un solo juez sin triage.
- Los checks remotos #632/#633 no reflejan el worktree mientras no haya commit/push. Esperar autorización explícita separada para publicar y conservar drafts.

## Triage Round 8 — Pendiente de autorización

- [ ] **RPI-026l — NULL DACL:** `copy_path_dacl` debe rechazar el caso `present=true, acl=None`; Microsoft Learn define que NULL DACL concede full access. Agregar policy/test Windows de archivo y directory backup.
- [ ] **RPI-020l — Ownership journal ACL Windows:** `set_private_file_permissions` y `set_private_directory_permissions` son no-op en Windows; el manifest con snapshots MCP se escribe en tempfile heredado antes de la restricción. Reutilizar el helper token-SID previo a escribir manifest bytes y para los state directories.
- [ ] **RPI-025c — ACL nativa en `.gitignore`:** `fs::Permissions` no conserva Windows DACL ni POSIX extended ACL; decidir si preservar descriptor nativo del archivo previo o fallar seguro ante ACL que el writer no puede reproducir.
- [ ] **RPI-027g — Traversal clean por handle:** ambos jueces repiten la ventana entre `symlink_metadata` y `read_dir`; el recheck path-based actual no ancla identidad. Cerrar requeriría handle-relative no-follow traversal/unlink; decidir si aceptar explícitamente el residual o ampliar arquitectura.
- Findings de R8 que ya están resueltos en Round7: apply stage MCP ACL + SID (RPI-020j/k), metadatos clean (RPI-027e), Z-Code (RPI-023d), compact source (RPI-026k) y CLI docs (RPI-026j). No reimplementar salvo regresión.
