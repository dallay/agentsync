# Design: Modularización del motor de sincronización del linker

## Technical Approach

Será una extracción mecánica y compatible hacia `src/linker/`, sin rediseñar el motor. `src/linker.rs`
se moverá atómicamente a `src/linker/mod.rs`; no existirán ambos archivos a la vez. `Linker` seguirá
siendo el único dueño de configuración, estado y caches. Los cinco módulos hijos contendrán bloques
`impl Linker` y reutilizarán ese estado mediante `&self`; no se introducirán servicios, traits, nuevos
caches, async ni concurrencia.

## Architecture Decisions

| Decisión | Alternativas rechazadas | Rationale |
|---|---|---|
| Renombrar/mover el archivo en una transición atómica | Mantener el monolito y añadir wrappers; crear ambos módulos | Rust no permite resolver `linker.rs` y `linker/mod.rs` simultáneamente; el movimiento hace el rollback mecánico y deja una sola fuente de implementación. |
| Mantener `Linker` y sus `RefCell` en `mod.rs` | Context structs, servicios o traits | Conserva ownership, firmas, scopes de borrow y comportamiento de caches; minimiza el riesgo del borrow checker y el diff semántico. |
| Mantener la API pública y distribuir solo implementaciones | Renombrar métodos o exponer submódulos | `lib.rs`, CLI, `status`, `doctor` y tests ya dependen de `agentsync::linker::*`; la ubicación física no debe cambiar el contrato. |
| Conservar los tests inline en `mod.rs` durante toda la extracción | Partirlos antes de compilar | Los 83 tests acceden a privados; mantenerlos evita ampliar visibilidad artificialmente y permite detectar regresiones por bloque. |

## Data Flow

```text
CLI/status/doctor
        │  Linker::new + API pública existente
        ▼
     mod.rs (estado, tipos, façade)
        ├── apply.rs ──► paths.rs ──► symlinks.rs ──► filesystem
        │       └──────► discovery.rs ───────────────┘
        └── clean.rs ──► paths/discovery ──► symlinks/filesystem
```

## File Changes

| File | Action | Description |
|---|---|---|
| `src/linker.rs` → `src/linker/mod.rs` | Move atómico | Tipos compartidos, estado privado de `Linker`, constructor, accessors, helpers públicos de status, declaración de módulos, `sync_mcp` y tests inline. |
| `src/linker/apply.rs` | Create | `sync`, selección de agentes, dispatch de targets, resolución/compresión de fuentes, creación de directorios y `module-map`. |
| `src/linker/clean.rs` | Create | `clean` y limpieza de `symlink`, `symlink-contents`, `nested-glob` y `module-map`, sin filtros de agentes. |
| `src/linker/discovery.rs` | Create | Templates, cache/walk de nested-glob y matchers de patrones/exclusiones. |
| `src/linker/paths.rs` | Create | Canonicalización, seguridad de destinos, revalidación TOCTOU, unlink seguro y `relative_path`. |
| `src/linker/symlinks.rs` | Create | Creación/actualización de enlaces, backups, contents, removal y gates Unix/Windows. |
| `src/lib.rs`, `src/main.rs`, `src/commands/{status,doctor}.rs` | Verify only | No cambios; se conservan re-exports, llamadas, output y contratos públicos. |

`mod.rs` conserva `SyncOptions`, `SyncResult`, `ResolvedSource`, `SymlinkContentsChildExpectation`,
aliases de caches, `Linker` y todos sus campos privados. También conserva `new`, `project_root`,
`config`, `expected_source_path`, `symlink_contents_expected_children` y `sync_mcp`. `sync` y
`clean` mantienen exactamente sus firmas públicas, aunque sus cuerpos vivan en `apply.rs` y
`clean.rs` respectivamente.

## Interfaces / Contracts

- `pub` únicamente para `Linker`, `SyncOptions`, `SyncResult`, `SymlinkContentsChildExpectation` y
  los métodos públicos existentes.
- Los campos de `Linker`, tipos internos y helpers permanecen privados; los helpers consumidos por
  otro módulo hijo usan `pub(super)`. No habrá `pub(crate)` nuevo ni submódulos públicos.
- Los módulos comparten `Linker` y sus caches existentes (`path_cache`, `compression_cache`,
  `glob_cache`, `ensured_dirs`, `ensured_compressed`, `canonical_project_root`) por referencia y
  `RefCell`; se preservan scopes de borrow e invalidaciones.

## Testing Strategy

| Capa | Qué validar | Enfoque |
|---|---|---|
| Baseline/TDD | 83 tests inline y contadores/output | Ejecutar `cargo test --lib linker` antes y después; cada extracción debe compilar antes de mover el siguiente bloque. |
| Paths/symlinks | Seguridad, TOCTOU, traversal, unlink, backups y plataformas | Mantener `tests/unit/linker_security.rs` dentro de `all_tests`, más `tests/test_security.rs` y los tests inline; no cambiar fixtures. |
| Integración | Status, adoption, module-map y CLI | Ejecutar los targets existentes `test_security`, `test_agent_adoption`, `test_module_map_cli`, `test_status_cli`. |
| Validación final | Estructura y regresión completa | `cargo fmt --all -- --check`, `cargo check --all-targets --all-features` y `cargo test --all-features`. |

El orden de extracción será `paths` → `symlinks` → `discovery` → `apply` → `clean`. Cada paso
preserva firmas y ejecuta formato, check y tests focalizados; así los errores de visibilidad/borrow
quedan aislados antes de combinar dispatch y limpieza.

## Migration / Rollout

No hay migración, feature flag ni cambio de rollout. La transición es atómica: crear el directorio,
mover el archivo, declarar módulos y extraer bloques. El rollback es revertir el movimiento y los
commits de extracción para restaurar `src/linker.rs`; no se modifican datos, CLI, seguridad,
concurrencia, rendimiento, async ni output.

## Open Questions

Ninguna. La spec base `core-sync-engine/spec.md` y `state.yaml` no se modifican.
