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
  - Fase 2 (nueva, solo si discovery es `Complete` o el search_root sigue existiendo; si el root falta por completo, scan limitado al prefijo estático del template o skip documentado): caminar candidatos dest-side buscando gemelos `dest` + `dest.bak` donde: (a) `dest` es symlink, (b) `dest.bak` existe y es fichero/dir regular — nunca symlink ni especial, (c) el target del symlink está dentro de `project_root.join(target.source)` (search_root) y ya no existe en disco (dangling), (d) **el scan-root pasa `revalidate_path` y la apertura del directorio padre usa `open_project_relative_directory` + `open_dir_nofollow` en `revert_destination` — verificación de scan-root, no `ensure_safe_destination` por candidato**, (e) nunca restaurar sobre fichero real aparecido tras el apply (misma guarda de `revert_destination:1036`).
  - Reutilizar `revert_destination` con `expected=None` para el scan; guarda (a)-(e) sustituye `ensure_safe_destination`.
  - Respetar `dry_run` (solo contar `removed`), `keep_backups` (copiar en vez de mover), `verbose`, y contadores `removed/restored/skipped/errors` existentes. Destinos inseguros para `errors` como hace revert hoy; discovery incompleto para `skipped`, sin scan.
- [ ] **T4 — Revalidar punto 4 (MCP server orphan).** Confirmar que `locked_source` filtra servidores plugin orphan: el journal graba `agent_ids` y `original_content` para servers generados por plugin, y restore los elimina incluso si el plugin ya no está bloqueado. Si falta, añadir en `OwnershipRecord.source = PluginSource(plugin, key)` y test con lock removed.
- [ ] **T5 — Revalidar punto 5 (journal migration).** Añadir `original_content` + `applied_sha256` a `OwnershipRecord` en schema v1; verificar que `restore_ownership_records` hace migrate in-place sin romper revert existente (backward compat). Test: crear journal sin campos nuevos, hacer restore, verificar que se migra.

## Evidencia y definición de done

- T1: `cargo test plugins::revert_no_fetch_locked_git_snapshot`
- T2: 4 tests round-trip pasando, `mcp.mdx` actualizado si aplicable
- T3: test `revert_nested_glob_orphan_restored_from_backup` pasando; `cargo test nested_glob` completo verde
- T4: test con plugin lock removed + restore limpiando server orphan
- T5: test con journal v0 → v1 migration passando

## Riesgos

- T3: scan dest-side puede ser costoso en repos grandes; limitar a prefijo/sufijo estático del template y `keep_backups` para reducir el espacio de búsqueda.
- T5: migration in-place sin backup del journal podría perder datos si falla a mitad; usar rename+write atómico.
