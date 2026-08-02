# Diseño: Curated, Verifiable Skill Registry

## Enfoque técnico

Añadir un registro curado versionado como fuente primaria de resolución. El catálogo actual
`src/skills/catalog.v1.toml` seguirá siendo el contrato de recomendaciones: conserva
`provider_skill_id`, `local_skill_id`, títulos, resúmenes, aliases, tecnologías y combos. El
registro añade el artefacto instalable y su evidencia de integridad, sin convertir el catálogo en
lockfile.

La resolución será `Registry -> Provider -> install`: una entrada curada produce un `SkillSource`
con repositorio, commit, subruta y hashes; el provider obtiene un archivo local/fixture o el ZIP
del commit inmutable; `install.rs` descomprime de forma segura, valida `SKILL.md`, verifica hashes
y solo entonces reemplaza el directorio y actualiza el registro instalado.

```text
catalog.v1.toml (recomendación) -> RegistryResolver -> PinnedProvider
                                      |                  |
                                      +-- validation <---+ fetch/fixture
                                                         |
                                      staging -> manifest/hash/license -> install + registry.json
```

## Decisiones de arquitectura

| Decisión | Alternativas / trade-off | Elección y rationale |
|---|---|---|
| Registro separado del catálogo | Meter SHA/licencia en `catalog.v1.toml` simplifica archivos, pero mezcla recomendación con distribución | `src/skills/registry.v1.toml` (o recurso equivalente empaquetado) separado: permite actualizar ranking/metadatos sin cambiar pins y mantiene compatibilidad v1 |
| Lockfile vs manifest | Un único archivo facilita mantenimiento, pero no distingue intención de estado resuelto | Manifest curado declara identidad, compatibilidad y política; lockfile generado (`registry.lock.toml`) contiene commit, archive/file hashes y fecha de captura. El lockfile se regenera, nunca se edita manualmente |
| Structs tipados | `serde_json::Value` sería flexible, pero desplaza errores a runtime | `RegistryDocument`, `RegistryEntry`, `SourcePin`, `FileHash`, `ManifestExpectation`, `LicenseEvidence` y `ValidationMetadata`, con `serde` y `BTreeMap` para determinismo |
| Curated-first con fallback explícito | Mantener provider actual como primaria conserva comportamiento, pero deja HEAD mutable | Resolver registry primero; fallback a `SkillsShProvider` solo con modo explícito/diagnóstico. Entradas inválidas no hacen fallback silencioso |
| Provenance fuera de contrato instalado | Cambiar `registry.json` rompe consumidores | Mantener `Registry`/`SkillEntry` y añadir campos opcionales solo si son backward-compatible; provenance completa vive en metadata del install y no altera claves canónicas |

## Flujo y contratos

`src/skills/registry.rs` debe separar el contrato existente de instalación (`Registry`, `SkillEntry`)
del registro curado. `src/skills/provider.rs` expone una resolución común que pueda devolver fuente
local o URL pinned, no `HEAD`. `install.rs` recibe expectativas de verificación mediante un contexto
de instalación, conserva staging/rollback y no escribe estado hasta pasar todas las validaciones.

```rust
pub struct RegistryEntry {
    pub provider_skill_id: String,
    pub local_skill_id: String,
    pub source: SourcePin,
    pub manifest: ManifestExpectation,
    pub files: Vec<FileHash>,
    pub license: LicenseEvidence,
    pub validation: ValidationMetadata,
}
pub struct SourcePin { pub repository: String, pub commit: String, pub subpath: String }
pub struct FileHash { pub path: String, pub sha256: String }
```

La validación exige schema soportado, IDs consistentes con `catalog.v1.toml`, commit SHA completo,
rutas seguras, `SKILL.md`, hash SHA-256 por archivo declarado, manifest esperado y evidencia SPDX.
La política rechaza licencia ausente/incompatible. El hash se calcula sobre bytes extraídos y
rutas normalizadas, no sobre el ZIP completo.

## Estructura de archivos

| Archivo | Acción | Propósito |
|---|---|---|
| `src/skills/registry.rs` | Modificar | Añadir loader/validator del registro curado; preservar API `registry.json`. |
| `src/skills/registry.v1.toml` | Crear | Manifest humano, linkage, licencias y política. |
| `src/skills/registry.lock.toml` | Crear | Pins, hashes y snapshots reproducibles. |
| `src/skills/provider.rs` | Modificar | `PinnedProvider`/fuente local y fallback explícito. |
| `src/skills/install.rs` | Modificar | Contexto de verificación, hashes, manifest y provenance antes del commit atómico. |
| `src/skills/catalog.rs`, `catalog.v1.toml` | Modificar | Añadir linkage opcional sin cambiar IDs ni salida de recomendaciones. |
| `src/commands/skill.rs` | Modificar | Comando mantenedor `registry sync`/validación, separado de install de usuario. |
| `tools/registry-sync` o `src/commands/skill.rs` | Crear/Modificar | Descarga commit fijado, calcula hashes, valida licencia/manifest y actualiza manifest+lockfile en una operación revisable. |
| `tests/fixtures/curated-skills/` | Crear | Repositorios locales con casos válido, hash alterado, licencia inválida y manifest inválido. |
| `tests/test_catalog_integration.rs`, `tests/test_catalog_integrity.rs`, `.github/workflows/catalog-e2e.yml` | Modificar | Sustituir dependencia HEAD por fixtures/pins; dejar red como job explícito de refresh. |

## Testing y rollout

Unit tests cubren parseo, schema, aliases, SHA, rutas, SPDX y separación manifest/lock. Integration
tests cubren `catalog -> registry -> provider -> install`, rollback y preservación de `registry.json`.
E2E usa fixtures locales por defecto; CI valida todos los entries offline. Un workflow mantenedor
opcional ejecuta sync contra upstream, requiere cambios explícitos de lockfile y revisión de licencia.

Migración incremental: primero entries de `dallay/agents-skills`, luego externos; fallback existente
permanece desactivado por defecto durante transición y se elimina cuando la cobertura esté completa.

## Preguntas abiertas

- [ ] Confirmar ubicación final del comando mantenedor (subcomando `skill registry sync` versus binario `tools/registry-sync`).
- [ ] Definir política exacta para licencias duales y redistribución de contenido externo.
