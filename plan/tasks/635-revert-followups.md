# Plan — Issue #635 revert follow-ups

## Ruta y alcance

- Ruta: Plan Mode, Delegated direct. Sin SDD formal.
- Autorización: analizar y planificar #635; implementar solo tras aprobar este plan. No builds amplios, no commits.
- Alcance: 5 follow-ups de `revert` dejados fuera de #630. `main` actual ya usa restore por snapshot (`restore_mcp_ownership`), no `remove_servers` por nombre/valor. El plan revalida cada punto contra ese diseño y solo implementa gaps reales.
- Fuera de alcance: #631 (`.git/info/exclude`), #556, #573, stack #632/#633.
- TDD: una regresión por comportamiento, confirmar RED antes de implementar.

## Estado verificado en `main`

- `src/linker/mod.rs:462 restore_mcp_ownership` + `restore_ownership_records:579` — restore por journal `ownership.json` (`original_content`, `pre_write_sha256`, `applied_sha256`, `agent_ids`).
- `src/mcp_ownership.rs:17 OwnershipManifest` / `25 OwnershipRecord` — schema v1, sin expiración.
- `src/linker/revert.rs:742 revert_nested_glob_target` — re-descubre desde sources existentes; `revert_destination:861` con `expected=None` hace warn + skip. Orphan con source borrado queda sin revertir. **Gap real.**
- `src/plugins.rs:140 LockedPlugin.mcp_servers`, `1653 read_plugin_mcp` — resolución actual exige leer plugin; revert por snapshot no la toca.
- `website/docs/src/content/docs/guides/mcp.mdx:26-55` — ya documenta: snapshot cubre archivo completo incluyendo top-level y output de plugins; restore usa `agent_ids` grabados aunque el agente esté deshabilitado; sin journal no borra por nombre/valor; revert de plugins no resuelve ni materializa.
- `git log`: `b79ba75 (#633)` + `37e0087 (#632)` ya en `main`. La descripción original de #635 asumía diseño `remove_managed_mcp` / `remove_servers`; ese diseño fue sustituido por snapshot.

## Tareas

- [x] **T1 — Revalidar punto 1 (plugins) contra snapshot.** Confirmar con test que `revert` + `restore_mcp_ownership` no llama a materialización ni red aunque haya servidores `plugin/...`. Si falta el regresor explícito, añadirlo en `tests/plugins_mcp.rs` con lock local y red bloqueada. Criterio: journal basta, cero `fetch_locked_git_snapshot` / `discover_plugin` en el path de revert.
- [x] **T2 — Revalidar punto 2 (4 formatos) contra snapshot.** Verificar que Claude Desktop global, Gemini CLI, OpenCode y Z-Code se restauran byte-exacto vía `restore_mcp_config_bytes` y que `remove_generated_mcp_config` solo borra si coincide con el estado grabado. Añadir round-trip tests por formato solo si falta cobertura. Si el snapshot ya lo cubre, cerrar el punto como resuelto por diseño y actualizar `mcp.mdx`.
- [x] **T3 — Resolver punto 3 (nested-glob orphans). OPCIÓN A APROBADA (2026-10-09): scan dest-side con ownership estricto.** Diseñar así, sin journal nuevo:
  - Fase 1: revert source-driven existente intacto (`enumerate_nested_glob` → `revert_destination` con `expected=Some`).
  - Fase 2 (nueva, solo si discovery es `Complete` o el search_root sigue existiendo; si el root falta por completo, scan limitado al prefijo estático del template o skip documentado): caminar candidatos dest-side buscando gemelos `dest` + `dest.bak` donde: (a) `dest` es symlink, (b) `dest.bak` existe y es fichero/dir regular — nunca symlink ni especial, (c) el target del symlink está dentro de `project_root.join(target.source)` (search_root) y ya no existe en disco (dangling), (d) el path relativo de `dest` pasa `ensure_safe_destination` y respeta prefijo/sufijo estático del template (texto antes del primer `{` y después del último `}`), (e) nunca restaurar sobre fichero real aparecido tras el apply (misma guarda de `revert_destination:1036`).
  - Reutilizar `revert_destination` con `expected` reconstruido solo cuando el dangling target sea determinable (leer `read_link` y comprobar que el padre/target falta); si no es determinable, warn + skip como hoy. No inventar `expected`.
  - Respetar `dry_run` (solo contar `removed`), `keep_backups` (copiar en vez de mover), `verbose`, y contadores `removed/restored/skipped/errors` existentes. Destinos inseguros → `errors` como hace revert hoy; discovery incompleto → `skipped`, sin scan.
  - Regresión TDD: `revert_nested_glob_restores_orphan_when_source_file_vanished` (source borrado, symlink dangling + `.bak` hermano → `restored=1`) y `revert_nested_glob_ignores_user_bak_without_dangling_link` (`.bak` ajeno sin symlink dangling hacia search_root → intacto, `skipped`).
  - Límite documentado: sin journal de symlinks, el ownership es heurístico (dangling + `.bak` + prefijo/sufijo + dentro de search_root). Un usuario que fabrique a mano esa tripleta podría ver su symlink tocado; se documenta en `troubleshooting.mdx` + `mcp.mdx` si aplica. No tocar `.bak` ajenos sin symlink dangling.
- [x] **T4 — Revalidar punto 4 (config cambiada tras apply).** Confirmar que el journal retiene `original_content` y que el restore es condicional (coincide con `applied_sha256` o restaura original; si divergió, skip sin merge). Añadir regresión de cambio de valor en `agentsync.toml` tras apply si falta. Prohibido: borrado por solo-nombre.
- [x] **T5 — Revalidar punto 5 (agente deshabilitado).** Confirmar que `restore_ownership_records` usa `record.agent_ids` grabados (línea 592: exige todos los owners seleccionados) y no `enabled` actual. Añadir regresión disable-tras-apply si falta. Prohibido: inferir ownership desde `enabled` actual.
- [x] **T6 — Docs en `guides/mcp.mdx`.** Actualizar solo lo que cambie tras T1-T5: estado final de plugins, tabla de los 10 formatos, orphans nested-glob (revertido o limitación), y ubicación del journal. Validar con build de docs del workspace.
- [x] **T7 — Verificación antes de cierre.** `cargo fmt --all -- --check`, `cargo check --all-targets --all-features`, `cargo clippy --all-targets --all-features -- -D warnings`, tests enfocados (`cargo test restore_mcp`, `cargo test revert_nested_glob`, `cargo test plugins_mcp`), y `cargo test --all-features` solo si el cambio toca producción. Registrar evidencia por tarea.

## Aceptación (de #635, reinterpretada a snapshot)

- Revert quita output de plugins sin materializar ni red — vía snapshot, con regresor.
- Los 10 formatos se restauran seguro (byte-exacto o borrado condicional) con tests.
- Orphans de source borrado se revierten o quedan documentados explícito.
- Docs actualizadas en `guides/mcp.mdx`.

## Evidencia y siguiente paso

- Evidencia inicial: lecturas CodeGraph de `revert`, `restore_mcp_ownership`, `OwnershipManifest`, `LockedPlugin`, más `mcp.mdx:26-55` y `git log` (`b79ba75`, `37e0087`).
- Evidencia T1/T4/T5 (2026-10-09, read-only, sin tocar producción):
  - `cargo test --all-features --test plugins_mcp` → 1 passed (`plugin_mcp_is_fanned_out_to_supported_agents_without_execution`).
  - `cargo test --all-features --lib mcp_ownership` → 30 passed.
  - `cargo test --all-features --test test_revert_cli` → 35 passed, incluyendo `test_revert_restores_journal_when_mcp_is_disabled_in_current_config` (punto 5), `test_revert_without_journal_warns_and_leaves_legacy_mcp_unchanged` (punto 4, sin borrado por nombre), `test_revert_removes_generated_mcp_config_when_it_still_matches_journal` (borrado condicional seguro).
  - `cargo test --all-features --lib linker::revert` → 43 passed.
- Evidencia T2 (gap parcial): `tests/test_revert_cli.rs` no contiene casos por formato (`claude-desktop|gemini|opencode|zcode` → 0 matches). El restore por snapshot es agnóstico al formato, pero faltan los 4 round-trip tests que pide la issue. T2 sigue abierto solo como tests, no como lógica por formato.
- Evidencia T2 cerrado (2026-10-09): 4 round-trip snapshot tests añadidos en `tests/test_revert_cli.rs`, todos GREEN a la primera (el diseño snapshot ya los cubría — son cobertura, no fix): `test_revert_restores_gemini_config_with_top_level_settings_exactly`, `test_revert_restores_opencode_config_with_top_level_settings_exactly`, `test_revert_restores_zcode_config_with_top_level_settings_exactly`, `test_revert_restores_claude_desktop_global_config_exactly` (global aislado vía `XDG_CONFIG_HOME`+`HOME` a temp dirs). Suite completa `--test test_revert_cli` → 39 passed; clippy limpio; fmt limpio.
- Evidencia T3: `test_revert_with_missing_nested_glob_source_reports_incomplete_and_keeps_gitignore` confirma el comportamiento actual: source ausente = skip incompleto, sin restaurar huérfanos. Decisión scan-vs-documentar sigue pendiente.
- Evidencia T3-A implementado (2026-10-09, TDD RED→GREEN):
  - RED: `revert_nested_glob_restores_orphan_when_source_file_vanished` falló (`is_symlink` aún presente) antes del fix.
  - GREEN: `cargo test --all-features --lib revert_nested_glob` → 3 passed; `--lib linker::revert` → 45 passed; `--test test_revert_cli` → 35 passed; `cargo clippy --all-targets --all-features -- -D warnings` limpio; `cargo fmt --check` limpio.
  - Cambio: `src/linker/revert.rs` — `revert_nested_glob_target` en 2 fases + `nested_glob_template_affix` + `revert_nested_glob_orphans` (walk fail-closed, ownership dangling-en-search-root + `.bak` regular + affix, mutación vía `revert_destination`). Root ausente intenta solo orphans; walk inseguro sigue fail-closed.
- Evidencia Code Review fixes + Coverage boost (2026-10-09, commit 76ca1ed):
  - Incomplete discovery tipado con `MissingRoot` estructurado en `NestedGlobDiscoveryStatus` enum (sin parsear string) garantizando fail-closed si la raíz es un archivo regular o no-directorio.
  - Normalización léxica (`normalize_lexical_path`) de `resolved` y `search_root` para prevenir bypasses por traversal relativo.
  - Corrección de `dirs::config_dir` en macOS usando `Library/Application Support` bajo `fake_home`.
  - 5 tests unitarios adicionales añadidos para ramas de orphan scan (missing root, invalid file root, symlinked .bak sibling, outside link, live source link) para satisfacer la cobertura de patch de Codecov.
- Siguiente paso: Monitorear CI de GitHub Actions para el commit `76ca1ed`.
- Estado: Ready.
