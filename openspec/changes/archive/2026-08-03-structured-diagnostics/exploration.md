# Exploration: structured diagnostics and machine-readable logging (issue #499)

Change name (proposed): `structured-diagnostics` — `feat(output): add structured diagnostics and machine-readable logging`.

> Investigación realizada el 2026-08-03 sobre el repo en `main` (HEAD `017f71d`). Todos los
> números de línea son del código actual en disco. No se modificó ningún archivo de código.

---

## 1. Salida actual del CLI

La salida está **solo parcialmente centralizada**. Existe `src/output.rs` (735 líneas) con
helpers de formato, pero la mayoría de los puntos de emisión son `println!` directos repartidos por
todo el crate.

### API pública de `src/output.rs`

- `LabelKind` enum (líneas 5-13): `Info | Warning | Success | Failure | Muted`.
- `OutputMode` enum (17-20): `Json | Human { use_color }` — ya existe el concepto de "formato",
  pero **solo lo usan `status` y `skill`**.
- `HumanFormatter` (27-108):
  - `format_label(symbol, label, kind)` (37) — "✔ installed".
  - `format_heading(heading)` (43) — bold cuando hay color.
  - `format_muted` (53), `format_key_value(label, value)` (63), `format_summary_line` (74),
    `format_hint` (81) — `#[allow(dead_code)]` en 52, 62, 73, 80 (casi todo está muerto salvo
    format_label/format_heading).
  - `style_by_kind` (90-107) — mapeo ANSI por LabelKind.
- `detect_output_mode(json, stdout_is_tty, no_color, clicolor, term)` (116-131) — función pura:
  `json` tiene prioridad absoluta; color solo si stdout es TTY y no hay overrides.
- `output_mode(json)` (134-142) — wrapper que lee el entorno real.
- `human_use_color()` (144-149), `print_lines(&[String])` (151-155, **println! a stdout**),
  `print_header()` (157-167, banner), `init_next_steps_lines(wizard)` (169-178).
- Renderers que devuelven `Vec<String>` (el patrón "render" separa formato de emisión):
  `render_dry_run_notice` (188), `render_clean_phase_with_color` (196),
  `render_sync_phase_with_color` (208), `render_apply_summary_with_color` (229),
  `render_clean_summary_with_color` (278), `render_gitignore_phase_with_color` (319),
  `render_mcp_phase` (333), `render_mcp_summary_with_color` (345).

### Puntos de emisión directa (`println!`) que NO pasan por output.rs

- `src/main.rs`: 230, 240, 244, 248, 251, 274-278 (verbose "Using config"), 285, 295, 327, 338,
  348, 368, 405, 414.
- `src/linker/apply.rs`: 40 (dry-run), 47, 60, 72 (skips verbose), 88 (header agente), 93
  (processing target verbose), 264, 282.
- `src/linker/clean.rs`: 16, 60, 65, 94, 99, 152, 157, 179, 192, 197.
- `src/linker/mod.rs`: 206, 214 (dirs creados).
- `src/linker/discovery.rs`: 103, 118, 132, 224, 263.
- `src/linker/symlinks.rs`: 27, 67, 95, 116, 123, 134, 148, 161.
- `src/init.rs`: ~40 `println!` (351-401 en `init()`, 1506-2246 en wizard).
- `src/mcp.rs`: 1384, 1391, 1407, 1414, 1454.
- `src/commands/doctor.rs`: todo el comando (19-265).
- `src/commands/skill.rs`: 71 (line reporter), 745, 772, 776, 819 (JSON), 823, 835, 841, 965
  (JSON error), 973, 1061 (JSON install), 1087 (JSON error), 1095.

**Conclusión**: el sistema de "formato" existe pero es per-command (`status --json`,
`skill ... --json`, `devinstall --json`). `apply/clean/init/doctor` no tienen `--json` ni
`OutputMode`; imprimen humano directo a stdout.

---

## 2. Logging actual

- Inicialización: `src/main.rs:152-155`:
  ```rust
  fn main() -> Result<()> {
      tracing_subscriber::fmt::init();
  ```
- **HALLAZGO CRÍTICO**: el writer por defecto de `fmt::init()` es **STDOUT** (verificado en el
  código fuente de tracing-subscriber 0.3.23, `src/fmt/fmt_layer.rs:749` → `make_writer: io::stdout`,
  y `fmt_layer.rs:68` → `W = fn() -> io::Stdout`). Es decir, **hoy TODOS los eventos tracing van a
  stdout**. Cualquier `warn!`/`error!` que dispare durante un comando `--json` corrompe el contrato
  machine-readable. Es un bug latente: los tests de contrato pasan porque los paths felices no
  emiten WARN+.
- `RUST_LOG` **sí funciona** aunque no esté el feature `env-filter`: `fmt::try_init()`
  (fmt/mod.rs:1200-1246) construye un filtro `Targets` desde `RUST_LOG` cuando `env-filter` está
  deshabilitado; default `INFO` cuando no está seteado (fmt/mod.rs:349 `DEFAULT_MAX_LEVEL =
  LevelFilter::INFO`). El comentario de main.rs:153 ("Respects RUST_LOG env var") es correcto a
  grandes rasgos, pero **no se pueden usar directivas de EnvFilter** (span fields, etc.) ni el
  formato `.json()` porque el feature `json` de tracing-subscriber **no está habilitado**
  (Cargo.toml:58 `tracing-subscriber = "0.3"` sin features; Cargo.lock confirma 0.3.23 con solo
  nu-ansi-term/sharded-slab/smallvec/thread_local/tracing-core/tracing-log).
- **No hay spans en ningún lado**: grep de `info_span|debug_span|error_span|warn_span|.enter()` =
  0 resultados en todo el repo.
- Los 15 call sites de macros tracing:
  - `src/mcp.rs:1468` `warn!(error, path, "Failed to remediate restricted permissions...")`,
    `src/mcp.rs:1518` `error!(agent, error, "Error generating agent config")`.
  - `src/init.rs:124,130,138` `warn!(path/error, "User config template...")`.
  - `src/main.rs:381` `error!(%e, "Error syncing MCP configs")`.
  - `src/linker/apply.rs:104` `error!(target, error, "Error processing target")`.
  - `src/linker/discovery.rs:231` `debug!(error, path, "WalkDir entry skipped...")`.
  - `src/commands/skill.rs:708` `warn!`, `1028` `debug!("install")`, `1055` `warn!`,
    `1235` `info!(original, converted, "Converted GitHub URL to ZIP format")`,
    `1254`/`1268` `warn!(skill_id, provider_skill_id, "Failed to resolve...")`.
  - `src/skills/uninstall.rs:83` `warn!`.
- `eprintln!` existe en un solo lugar: `src/update_check.rs:137` (aviso de versión nueva → stderr).
  Es el patrón correcto de referencia.
- **`println!` que deberían ser logs** (info de diagnóstico/verbose en stdout):
  - `src/main.rs:274-278` ("Using config: ..." solo con `-v`).
  - `src/linker/apply.rs:47,60,72,93` (skips y "Processing target" solo con `verbose`).
  - `src/linker/discovery.rs:103-263` (entradas saltadas en nested-glob).
  - `src/commands/doctor.rs` (líneas de diagnóstico humano — hoy son salida funcional).

---

## 3. Comandos y operaciones principales (puntos de instrumentación)

| Comando | Handler | Implementación | Operaciones que merecen span |
|---|---|---|---|
| `apply` | main.rs:181-197 → `handle_apply` (267) | `linker.sync` (apply.rs:28-112), `process_target` (115-160), `create_symlink` (symlinks.rs), `clean` (clean.rs), gitignore (main.rs:336), MCP (`handle_apply_mcp` 361 → `sync_mcp` → `generate_all` mcp.rs:1483) | sync por agente/target, clean, gitignore, mcp; campos `agent_id`, `target`, `path`, `operation`, `outcome` |
| `clean` | main.rs:198-203 → `handle_clean` (388) | `linker.clean` (clean.rs:13-197) | remover symlinks por destino (`path`) |
| `init` | main.rs:174-180 → `handle_init` (220) | `init::init` (init.rs:341-405), `init_wizard` (≈2006+), `init_wizard_experimental_tui` (1492) | crear dirs/config/AGENTS.md |
| `status` | main.rs:164-168 → `run_status` (status.rs:139) | `collect_status_entries` (71-137), validación por entrada | validación por agente/target/entrada |
| `doctor` | main.rs:169-173 → `run_doctor` (doctor.rs:267) | `check_source_directory` (19), `check_target_sources` (37), `check_destination_conflicts` (103), `check_mcp_servers` (141), `check_gitignore` (178), `check_unmanaged_skills` (258) | cada check |
| `skill install/update/uninstall/suggest/registry` | main.rs:159-163 → `run_skill` (skill.rs:754) | `run_install` (1015), `run_update`, `run_uninstall`, `run_suggest` (785), `run_suggest_install` (846), `run_registry_command` (768) | install/update/uninstall por `skill_id`; resolve/install phases |

**Puntos naturales para spans** (loop anidado agente→target ya existente):
- `src/linker/apply.rs:43-109` — bucle de agentes: span por `agent_name` (campos `agent_id`,
  `operation = "sync"`).
- `src/linker/apply.rs:91-108` — bucle de targets: span con `target`, `path` (destino), `outcome`
  (ok/error/skipped/created/updated).
- `src/linker/apply.rs:115-160` (`process_target`) — `operation` por SyncType
  (symlink / symlink-contents / nested-glob / module-map).
- `src/linker/clean.rs:13-197` — span por `path`/`operation = "remove"`.
- `src/mcp.rs:1497-1522` (`generate_all`) — span por `agent`.
- `src/commands/skill.rs` — `run_install` (1015) y fases resolve/install de
  `run_suggest_install` (846-899).

---

## 4. Errores

- Fronteras: `anyhow::Result` en main.rs y comandos; `.context()` puntual
  (main.rs:29, apply.rs:144-149). `thiserror` solo en skills:
  `SkillInstallError` (skills/install.rs:16-33, variantes Io/Network/ZipArchive/Registry/
  PathTraversal/Validation/Other), más `update.rs`/`uninstall.rs`.
- `main() -> Result<()>` (main.rs:152): el `Termination` de std imprime el chain de anyhow a
  **stderr** (formato "Error: ...: ...").
- **Contexto que se pierde** (lo que la issue quiere mejorar):
  - apply.rs:103-106: el error de un target se loguea solo con `target` + `%e`; **falta
    `agent_id` y `path`**.
  - main.rs:380-383: error de MCP solo con `%e`; **falta `agent`/`config_path`**.
  - `doctor` **traga errores**: doctor.rs:280-282 y 290-293 imprimen y `return Ok(())` — exit
    code siempre 0 aunque haya fallos de parseo. Sin JSON ni estructura.
  - `status --json` con problemas: imprime el array JSON y `std::process::exit(1)` (status.rs:154)
    — no emite un objeto de error estructurado.
  - Los errores de skill SÍ tienen envelope estructurado: `handle_suggest_error`
    (skill.rs:936-978) produce `{"error","code","remediation"}`; igual en run_install
    (1082-1087), update (732-735), uninstall (1175, 1209-1212). Se imprime a stdout **y** se
    propaga Err → el chain de anyhow va a stderr. Este es el patrón existente a reutilizar.

---

## 5. El `--json` actual

- `status`: `StatusArgs { json: bool }` (status.rs:13-17), `#[command(flatten)]` en
  main.rs:73-75; dispatch `run_status(args.json, project_root)` (main.rs:167); salida
  `serde_json::to_string_pretty(&entries)` (status.rs:152); structs serializables
  `StatusEntry`/`StatusIssue`/`StatusChildEntry`/`DestinationKind`/`StatusIssueKind`
  (status.rs:19-69, kebab-case); exit 1 si `problems > 0` (154).
- `skill suggest --json`: `print_suggest_output` (skill.rs:833-844) →
  `serde_json::to_string(&response.to_json_response())`; install: `run_suggest_inner`
  (818-820) imprime `SuggestInstallJsonResponse`; success install: `render_skill_success_json`
  (566-581); errores: envelope `{error, code, remediation}` (960-965, 1082-1087, 732-735).
- `devinstall --json` (main.rs:204-215) → `run_install`.
- **No hay `--json` global**: el flag es per-command. `apply/clean/init/doctor` no lo tienen.
- Docs: `website/docs/src/content/docs/reference/cli.mdx` documenta `--json` para status (321:
  "Output machine-readable JSON (pretty-printed)... Useful for CI"), skill install/suggest/update.
  `cli-tui-compatibility.md:11-14` codifica el contrato: *"When --json is set: stdout must contain
  machine-readable JSON only. Human headings, colors, spinners, progress frames, and hints must not
  be mixed into stdout."*

---

## 6. Contratos y tests

- `tests/contracts/test_install_output.rs` (94 líneas): corre el binario y **parsea TODO el
  stdout como JSON** (27-28, 71-72). `install_json_contract` exige campos
  id/status/name/description/files/manifest_hash/installed_at; `install_json_error_contract`
  exige `error`/`code`/`remediation` con `code != "unknown"` y exit no-success.
- `tests/contracts/test_skill_suggest_output.rs` (397 líneas, 9 tests): `serde_json::from_slice(&output.stdout)` (30, 71, 100, 155, 204, 291); test clave
  `..._has_no_progress_preamble` (213-274) y `..._has_no_human_progress_lines` (307-334) que
  afirman stdout empieza con `{`, termina con `}`, sin "Installing ", sin `\r`, sin `⠋`, sin
  ANSI; error estructurado `interactive_tty_required` (291-304, 332-334).
- `tests/test_status_cli.rs`: afirma líneas humanas en stdout ("OK:", "Hint:",
  "Status: All good") (58-67).
- **Ningún test afirma contenido de stderr** (solo lo usan como diagnóstico en el mensaje de
  fallo: 34 ocurrencias de `stderr` en tests, todas en asserts de error). → Mover logs a stderr
  **no rompe ningún test existente**, siempre que stdout siga limpio.
- Otros: `tests/all_tests.rs` (harness con módulos integration::*, unit::*), `tests/e2e/`
  (Docker, `RUN_E2E=1`), `tests/security_repro/`, `tests/manual/`.
- `src/output.rs` tiene 30+ tests unitarios de formato exacto (plain y ANSI) — p.ej.
  `apply_summary_preserves_exact_plain_contract` (376-398).

---

## 7. Secrets

- **No se encontró logging de tokens/API keys** en src. Los 15 call sites de tracing loguean solo
  paths, errores y skill_ids.
- Áreas sensibles que NO deben loguearse: MCP `env`/`headers` (pueden contener credenciales:
  config.rs:1241 test `Authorization = "Bearer token123"`, mcp.rs:2785 test), `command`/`args` de
  MCP con secretos embebidos. `set_restricted_permissions` (mcp.rs:1157, 1467-1468) protege los
  permisos del archivo y el warn loguea solo `path` — correcto.
- `doctor` imprime el command de MCP (doctor.rs:150-173) — nombre del binario, no sensible.
- `try_convert_github_url` normaliza URLs y el log de skill.rs:1235 registra la URL convertida; el
  test de skill.rs:1559 confirma que `?token=` se maneja (query descartado). Al añadir spans, el
  campo `url` debe loguearse siempre sin query ni credenciales.
- **Recomendación**: un helper de redacción (`redact_url`, nunca loguear `env`/`headers` de MCP)
  y una regla explícita en el spec: los campos `path`/`url` se loguean solo tras sanitización.

---

## 8. init.rs y wizard

- Wizard estándar: `dialoguer` (Confirm/MultiSelect/Select) en init.rs:2006-2071 y en
  skill.rs:1003-1007. **Verificado en el fuente de dialoguer-0.12.0**: los prompts se renderizan a
  **stderr** (`src/prompts/confirm.rs:102` "The dialog is rendered on stderr", `:108`
  `self.interact_on(&Term::stderr())`).
- Las líneas de progreso/estado del wizard van a **stdout** vía `println!` (init.rs:1506-2246).
- TUI experimental: ratatui+crossterm pantalla completa (init.rs:1474-1520, 3947-3963); requiere
  stdin y stdout TTY, si no, baila con mensaje (3962-3963).
- **Implicación para logs a stderr**: dialoguer (prompts interactivos) y tracing (logs) compartirían
  stderr → riesgo de intercalado durante wizard/suggest interactivo. En CI no hay prompts
  interactivos (`ensure_interactive_install_supported` skill.rs:980-988 exige TTY; el wizard
  estándar en no-TTY fallaría igual). Mitigación sugerida: logs por defecto solo WARN+ durante
  fases interactivas, o aceptar el solapamiento porque prompts y logs comparten stderr solo en
  TTY (caso interactivo, donde el usuario ve ambos).

---

## Recomendaciones preliminares (opciones de diseño)

### A. Separar `--log-format json` del `--json` funcional

1. **Flags globales en `Cli` (recomendado)**: añadir en `struct Cli` (main.rs:51-54) args globales
   `--log-format <human|json>` (default `human`) y opcional `--log-level <trace|debug|info|warn|error>`
   o reutilizar `RUST_LOG`. Clap permite args globales que se leen en `main()` antes del dispatch
   del subcomando. El `--json` funcional queda intacto por comando.
2. **Flag per-comando** `--log-format` en cada subcomando: repetitivo, inconsistente, más trabajo.
3. **Solo env var** `AGENTSYNC_LOG_FORMAT` (+ `RUST_LOG`): cero flags nuevos, pero la issue pide un
   flag explícito y es menos descubrible.
4. (Complementario a cualquiera) Habilitar feature `json` de tracing-subscriber en Cargo.toml:58 →
   `tracing-subscriber = { version = "0.3", features = ["json"] }` y construir el subscriber con
   `fmt().json().with_writer(io::stderr).with_env_filter(...)` cuando `--log-format json`, y
   `fmt().with_writer(io::stderr)` en el caso human. `serde_json` ya está en el árbol.

### B. Qué spans/campos instrumentar y dónde

- Niveles: `INFO` para span de comando + outcome por agente/target; `DEBUG` para detalles por
  archivo (create/update/skip); `WARN/ERROR` para fallos. Campos fijos: `operation`, `outcome`,
  `agent_id`, `target`, `path`, `skill_id` según el contexto.
- Puntos: `main.rs` (span raíz por subcomando, `operation = "apply|clean|init|status|doctor|skill"`),
  `linker/apply.rs:43-109` (agente+target), `apply.rs:115-160` (SyncType), `linker/clean.rs`
  (path), `mcp.rs:1497-1522` (agent), `commands/skill.rs` `run_install`/`run_update`/`run_uninstall`
  (skill_id, phases resolve/install). Usar `tracing::info_span!` + `.in_scope()` (evita añadir
  `tracing-attributes` como dep directa, aunque ya está en el árbol como transitiva de tracing).
- No instrumentar el inner loop de creación de symlinks con spans anidados por archivo a INFO
  (ruido); usar `debug!`.

### C. Garantizar logs→stderr y output→stdout

- **Cambio #1 (crítico)**: reemplazar `tracing_subscriber::fmt::init()` (main.rs:154) por un
  builder con `.with_writer(io::stderr)`. Hoy los logs van a stdout (bug latente).
- Mantener `println!`/`print_lines` (output.rs:151) para salida funcional; migrar los `println!`
  de diagnóstico/verbose (main.rs:274-278, apply.rs:47/60/72/93, discovery.rs) a `tracing::info!`/
  `debug!` para que pasen a stderr automáticamente.
- `update_check` ya usa `eprintln!` (update_check.rs:137) — patrón de referencia.
- En modo `--log-format json`, cada evento es una línea JSON en stderr; la salida funcional
  (humana o `--json`) sigue en stdout.

### D. Riesgos de romper contratos

- **ALTO (latente, hoy)**: cualquier evento WARN+ durante un comando `--json` escribe en stdout y
  rompe `serde_json::from_slice(&output.stdout)` de los contracts. El fix (stderr) **elimina** este
  riesgo; no lo introduce. Riesgo nuevo: implementar `--log-format json` con writer a stdout por
  error → rompería todo. El spec debe exigir `with_writer(io::stderr)` en ambos formatos.
- **MEDIO**: dialoguer renderiza prompts a stderr; logs a stderr se intercalarían en wizard/suggest
  interactivos. Mitigar con default WARN+ en fases interactivas o aceptarlo (solo ocurre en TTY).
- **MEDIO**: doctor siempre devuelve exit 0 (traga errores) y status usa `process::exit(1)` —
  si el cambio introduce "outcome" por comando, decidir si se tocan exit codes (fuera de scope de
  la issue salvo que el spec lo pida).
- **BAJO**: tests de stdout humano exacto (test_status_cli.rs, output.rs unit tests) no se ven
  afectados si no se cambia la salida funcional.
- **BAJO**: el envelope de error `{error, code, remediation}` de skill ya es contrato (contracts
  tests) — no cambiar su forma al añadir logs.

### Ready for proposal
Sí. La exploración confirma que el cambio es factible con bajo riesgo: la pieza crítica es un
one-liner (writer→stderr) + flags globales + spans en 4-6 puntos. Se recomienda que sdd-propose
use el nombre de cambio `structured-diagnostics`.
