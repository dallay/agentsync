# Verification Report

**Change**: nested-agent-context  
**Version**: spec.md status = DRAFT

---

### Completeness

| Metric           | Value |
|------------------|-------|
| Tasks total      | 19    |
| Tasks complete   | 19    |
| Tasks incomplete | 0     |

All tasks in `openspec/changes/nested-agent-context/tasks.md` are marked complete.

---

### Build & Tests Execution

**Build**: ✅ Passed  
Command: `cargo build`

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.14s
```

**Tests**: ✅ 319 passed / ❌ 0 failed / ⚠️ 4 ignored  
Primary command: `cargo test`

```text
test result: ok. 279 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.89s
Doc-tests agentsync: 0 passed; 0 failed
```

Additional focused validation:

- `cargo test test_module_map_sync_dry_run -- --nocapture` → ✅ 1 passed
- `cargo test module_map` → ✅ 30 passed
- Manual CLI smoke with compiled binary:
    - `agentsync apply --dry-run` on a 2-mapping module-map target → ✅ 2 `Would link:` lines, no
      destination directories created
    - `agentsync apply` → ✅ created 2 symlinks
    - `agentsync status --json` → ✅ returned 2 expanded module-map entries
    - `agentsync doctor` → ✅ 0 issues on valid placeholder-source module-map config
    - `agentsync clean` / `agentsync clean --dry-run` → ✅ removed or preserved 2 symlinks as
      expected

**Coverage**: ➖ Not configured

---

### Spec Compliance Matrix

| Requirement                           | Scenario                                                            | Test                                                                                                                                                                                                                                           | Result      |
|---------------------------------------|---------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-------------|
| TOML Configuration Parsing            | Parse a module-map target with mappings                             | `config::tests::test_parse_module_mapping_struct`                                                                                                                                                                                              | ✅ COMPLIANT |
| TOML Configuration Parsing            | Parse a module-map target with filename_override                    | `config::tests::test_parse_module_mapping_struct`                                                                                                                                                                                              | ✅ COMPLIANT |
| TOML Configuration Parsing            | Parse existing symlink target unchanged                             | `config::tests::test_parse_target_config_without_mappings_defaults`                                                                                                                                                                            | ✅ COMPLIANT |
| TOML Configuration Parsing            | Reject invalid sync type (module-map with no mappings parses)       | `config::tests::test_parse_module_map_sync_type`, `linker::tests::test_module_map_empty_mappings`                                                                                                                                              | ✅ COMPLIANT |
| TOML Configuration Parsing            | Reject truly invalid type string                                    | `config::tests::test_parse_invalid_sync_type`                                                                                                                                                                                                  | ✅ COMPLIANT |
| Convention Filename Resolution        | Known agent convention filenames                                    | `agent_ids::tests::test_agent_convention_filename_known_agents`                                                                                                                                                                                | ✅ COMPLIANT |
| Convention Filename Resolution        | Known agent with nested convention path                             | `agent_ids::tests::test_agent_convention_filename_known_agents`                                                                                                                                                                                | ✅ COMPLIANT |
| Convention Filename Resolution        | Unknown agent returns no convention filename                        | `agent_ids::tests::test_agent_convention_filename_unknown`                                                                                                                                                                                     | ✅ COMPLIANT |
| Convention Filename Resolution        | Case-insensitive agent matching                                     | `agent_ids::tests::test_agent_convention_filename_case_insensitive`                                                                                                                                                                            | ✅ COMPLIANT |
| Destination Filename Resolution Order | filename_override takes precedence                                  | `config::tests::test_resolve_module_map_filename_override`, `config::tests::test_resolve_module_map_filename_override_beats_convention`, `linker::tests::test_module_map_filename_override`                                                    | ✅ COMPLIANT |
| Destination Filename Resolution Order | Convention filename used when no override                           | `config::tests::test_resolve_module_map_filename_convention_claude`                                                                                                                                                                            | ✅ COMPLIANT |
| Destination Filename Resolution Order | Convention filename for codex agent                                 | `agent_ids::tests::test_agent_convention_filename_known_agents`                                                                                                                                                                                | ✅ COMPLIANT |
| Module-Map Sync (Apply)               | Sync creates symlinks for all mappings                              | `linker::tests::test_module_map_creates_symlinks`, manual CLI smoke `agentsync apply`                                                                                                                                                          | ✅ COMPLIANT |
| Module-Map Sync (Apply)               | Sync creates intermediate directories                               | `linker::tests::test_module_map_nested_convention_path_creates_intermediate_directories`, manual CLI smoke `agentsync apply`                                                                                                                   | ✅ COMPLIANT |
| Module-Map Sync (Apply)               | Sync with nested convention path (copilot)                          | `linker::tests::test_module_map_nested_convention_path_creates_intermediate_directories`                                                                                                                                                       | ✅ COMPLIANT |
| Module-Map Sync (Apply)               | Sync skips mapping when source doesn't exist                        | `linker::tests::test_module_map_missing_source_skipped_and_other_mappings_continue`                                                                                                                                                            | ✅ COMPLIANT |
| Module-Map Sync (Apply)               | Sync handles existing symlink (idempotent re-run)                   | `linker::tests::test_module_map_sync_is_idempotent_when_symlink_already_matches`                                                                                                                                                               | ✅ COMPLIANT |
| Module-Map Dry-Run                    | Dry-run prints per-mapping messages and makes no filesystem changes | `linker::tests::test_module_map_sync_dry_run`, manual CLI smoke `agentsync apply --dry-run`                                                                                                                                                    | ✅ COMPLIANT |
| Module-Map Clean                      | Clean removes all module-map symlinks                               | `linker::tests::test_module_map_clean_removes_symlinks`, manual CLI smoke `agentsync clean`                                                                                                                                                    | ✅ COMPLIANT |
| Module-Map Clean                      | Clean skips non-symlink files                                       | `linker::tests::test_module_map_clean_skips_non_symlink_files`                                                                                                                                                                                 | ✅ COMPLIANT |
| Module-Map Clean                      | Clean dry-run prints per-mapping messages                           | `linker::tests::test_module_map_clean_dry_run`, manual CLI smoke `agentsync clean --dry-run`                                                                                                                                                   | ✅ COMPLIANT |
| Module-Map Status                     | Status shows per-mapping entries                                    | `commands::status_tests::tests::test_collect_status_entries_expands_module_map_entries`, manual CLI smoke `agentsync status --json`                                                                                                            | ✅ COMPLIANT |
| Module-Map Status                     | Status detects stale symlink                                        | `commands::status_tests::tests::test_collect_status_entries_reports_stale_module_map_symlink`                                                                                                                                                  | ✅ COMPLIANT |
| Module-Map Status                     | Status JSON output includes module-map entries                      | `commands::status_tests::tests::test_collect_status_entries_expands_module_map_entries`, manual CLI smoke `agentsync status --json`                                                                                                            | ✅ COMPLIANT |
| Module-Map Gitignore Integration      | Gitignore entries expanded from mappings                            | `config::tests::test_all_gitignore_entries_module_map_expands_mappings`                                                                                                                                                                        | ✅ COMPLIANT |
| Module-Map Gitignore Integration      | Gitignore entries with filename_override                            | `config::tests::test_all_gitignore_entries_module_map_with_filename_override`                                                                                                                                                                  | ✅ COMPLIANT |
| Module-Map Gitignore Integration      | Gitignore skips disabled agents                                     | `config::tests::test_all_gitignore_entries_module_map_disabled_agent_skipped`                                                                                                                                                                  | ✅ COMPLIANT |
| Module-Map Gitignore Integration      | Gitignore deduplicates expanded entries                             | `config::tests::test_all_gitignore_entries_module_map_deduplicates_expanded_entries`                                                                                                                                                           | ✅ COMPLIANT |
| Module-Map Doctor Validation          | Doctor detects missing mapping source                               | `commands::doctor_tests::tests::test_collect_missing_sources_reports_missing_module_map_mapping_source`                                                                                                                                        | ✅ COMPLIANT |
| Module-Map Doctor Validation          | Doctor detects destination conflict across mappings                 | `commands::doctor_tests::tests::test_expand_target_destinations_expands_module_map_destinations`, `commands::doctor_tests::tests::test_validate_destinations_duplicates`, `commands::doctor_tests::tests::test_validate_destinations_combined` | ✅ COMPLIANT |
| Module-Map Doctor Validation          | Doctor detects destination overlap with other target types          | `commands::doctor_tests::tests::test_validate_destinations_detects_module_map_overlap_with_regular_target`                                                                                                                                     | ✅ COMPLIANT |
| Module-Map Doctor Validation          | Doctor warns on mappings with wrong sync_type                       | `commands::doctor_tests::tests::test_target_configuration_warnings_for_module_map_edge_cases`                                                                                                                                                  | ✅ COMPLIANT |
| Module-Map Doctor Validation          | Doctor warns on empty module-map                                    | `commands::doctor_tests::tests::test_target_configuration_warnings_for_module_map_edge_cases`                                                                                                                                                  | ✅ COMPLIANT |
| Module-Map Doctor Validation          | Doctor passes for valid module-map config                           | manual CLI smoke `agentsync doctor`                                                                                                                                                                                                            | ✅ COMPLIANT |
| TargetConfig Field Applicability      | module-map target with placeholder source/destination               | `commands::doctor_tests::tests::test_collect_missing_sources_ignores_module_map_placeholder_target_source`, manual CLI smoke `agentsync apply` / `agentsync doctor`                                                                            | ✅ COMPLIANT |

**Compliance summary**: 35/35 scenarios compliant

---

### Correctness (Static — Structural Evidence)

| Requirement                    | Status        | Notes                                                                                                                        |
|--------------------------------|---------------|------------------------------------------------------------------------------------------------------------------------------|
| TOML parsing and data model    | ✅ Implemented | `SyncType::ModuleMap`, `ModuleMapping`, and `TargetConfig.mappings` are present in `src/config.rs`                           |
| Convention filename resolution | ✅ Implemented | `agent_convention_filename()` is implemented in `src/agent_ids.rs` and normalizes aliases/case                               |
| Filename resolution order      | ✅ Implemented | `resolve_module_map_filename()` enforces override → convention → basename                                                    |
| Apply/link behavior            | ✅ Implemented | `process_target()` dispatches to `process_module_map()` in `src/linker.rs`                                                   |
| Dry-run behavior               | ✅ Implemented | `ensure_directory()` respects `dry_run`; `create_symlink()` uses dry-run-relative path handling without creating directories |
| Clean behavior                 | ✅ Implemented | `Linker::clean()` expands module-map mappings and removes symlinks only                                                      |
| Status integration             | ✅ Implemented | `collect_status_entries()` expands module-map targets into individual entries                                                |
| Gitignore integration          | ✅ Implemented | `Config::all_gitignore_entries()` expands module-map destinations and backup patterns                                        |
| Doctor integration             | ✅ Implemented | `collect_missing_sources()`, `expand_target_destinations()`, and `target_configuration_warnings()` handle module-map rules   |
| Build coherence                | ✅ Restored    | Current `src/linker.rs` uses `self.ensured_dirs` and `self.ensured_compressed`; no stale `ensured_outputs` references remain |

---

### Coherence (Design)

| Decision                                            | Followed? | Notes                                                                |
|-----------------------------------------------------|-----------|----------------------------------------------------------------------|
| Add `ModuleMapping` on `TargetConfig`               | ✅ Yes     | Implemented as `Vec<ModuleMapping>` with `#[serde(default)]`         |
| Add convention filename function in `agent_ids.rs`  | ✅ Yes     | Implemented in `src/agent_ids.rs`                                    |
| Use filename order override → convention → basename | ✅ Yes     | Reused via `resolve_module_map_filename()`                           |
| Resolve mapping sources from `source_dir`           | ✅ Yes     | `process_module_map()` uses `self.source_dir.join(&mapping.source)`  |
| Expand mappings into individual gitignore entries   | ✅ Yes     | Implemented in `Config::all_gitignore_entries()`                     |
| Thread `agent_name` through linker dispatch         | ✅ Yes     | `process_target(agent_name, ...)` is in place                        |
| Preserve dry-run as no-op on filesystem             | ✅ Yes     | Verified by unit test plus 2-mapping CLI dry-run smoke               |
| Keep implementation buildable through recovery pass | ✅ Yes     | `cargo build` and all verification commands pass on the current tree |

Artifact coherence check:

- Exploration, proposal, spec, design, tasks, and implementation remain aligned on module-map
  behavior.
- The linker cache recovery is coherent with the design intent: dry-run remains a no-op and the
  compile regression from stale cache field names is resolved.

---

### Issues Found

**CRITICAL** (must fix before archive):

- None

**WARNING** (should fix):

- `openspec/changes/nested-agent-context/spec.md` still carries `Status: DRAFT`; this does not block
  behavioral verification, but the artifact should be finalized before or during archive.
- One compliance scenario (`Doctor passes for valid module-map config`) is proven by manual CLI
  smoke rather than a dedicated automated test; coverage is sufficient for verify, but a permanent
  regression test would strengthen future confidence.

**SUGGESTION** (nice to have):

- Add a dedicated automated integration test that runs the full placeholder-source module-map CLI
  flow (`apply`, `status --json`, `doctor`, `clean`) to lock in the manual smoke evidence.

---

### Verdict

PASS WITH WARNINGS

The current tree compiles, the module-map dry-run blocker is resolved with runtime proof, and
archive is allowed now because there are no critical issues.
