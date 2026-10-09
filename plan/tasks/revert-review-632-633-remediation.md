# Remediación de revisión — PR #632 → #633

## Ruta y alcance

- Ruta: Delegated direct, RPI sin SDD. Trabajo local en la cadena apilada: primero la capa core de `feat/revert-core-630` (#632), luego MCP en `feat/revert-mcp-630` (#633).
- Autorización: corregir los bloqueos de esta cadena y publicar remediaciones, autorizado tras los RED y reiterado por el usuario con «sube los cambios». No rebase, force-push, responder/resolver hilos ni cambiar el estado draft. Publicar respetando el stack: #632 primero, luego #633.
- Fuera de alcance: Issue #631 (`.git/info/exclude`), seguimiento #635 (carrera compare-then-persist), las demás PRs abiertas.
- TDD: una regresión por comportamiento, confirmar RED, implementar el cambio mínimo y comprobar GREEN antes del siguiente comportamiento.

## Estado inicial y aceptación

- Estado: refs remotas actuales #632 `5e0c57c`, #633 (PR #659 merged to main). Ambas PR ya merged. La descripción original de #635 asumía diseño `remove_managed_mcp` / `remove_servers`; ese diseño fue sustituido por snapshot.
- #632: merged to `main` at `5e0c57c`. Tras error 87 con FileRenameInfoEx, FileRenameInfo y buffer NUL, se añadió `ReOpenFile` del mismo handle con `FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE` para RootDirectory; sin path lookup. Linux validation y pre-push full pasaron. **Histórico: esta PR ya está merged.**
- #633: merged to `main` at `d52c6e7`, integra parent `5e0c57c` por merge normal. **Histórico: esta PR ya está merged.**

## Problemas resueltos

- [x] Error 87 con FileRenameInfoEx → mitigado con `ReOpenFile` para RootDirectory
- [x] Path traversal en `ensure_safe_destination` → verificado que no hay path traversal en revert core
- [x] Z-Code config path → documentado y testeado con fixture

## Seguimiento #635

- Creado `plan/tasks/635-revert-followups.md` para trackear los 5 follow-ups restantes
- T1-T3 en progreso, T4-T5 pendientes

## Definición de done

- #635 plan aprobado por el usuario
- T1-T5 implementados y testeados
