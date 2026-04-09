# Documentation Audit Report - AgentSync

**Date**: 2026-04-09T18:04:26Z
**Auditor**: Jules

## Executive Summary
This report documents inconsistencies and errors found in the AgentSync documentation (located in `website/docs/`) compared to the actual CLI implementation and behavior. While the core functionality is robust, there are several discrepancies in command-line flags, unimplemented features, and internal consistency.

---

## 1. Command-Line Interface (CLI) Inconsistencies

### Flag Discrepancies
| Command | Documented Flag | Actual CLI Flag | Status | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `init` | `--path`, `--project-root` | `-p`, `--path`, `--project-root` | ✔ OK | `--project-root` is an alias in code. |
| `apply` | `--path`, `--project-root` | `-p`, `--path`, `--project-root` | ✔ OK | `--project-root` is an alias in code. |
| `clean` | `--path`, `--project-root` | `-p`, `--path`, `--project-root` | ✔ OK | `--project-root` is an alias in code. |
| `doctor` | `--project-root` | `--project-root` | ⚠ Partial | Docs don't mention the `-p` alias, and `doctor -p` fails because it's not defined as a short flag in `src/main.rs` (only long `project-root`). |
| `status` | `--project-root` | `--project-root` | ⚠ Partial | Docs don't mention the `-p` alias, and it lacks the short flag `-p` in implementation. |
| `skill` | `--project-root` | `--project-root` | ⚠ Confusion | `--project-root` must be passed *before* the subcommand (e.g., `skill --project-root . suggest`). Docs are sometimes ambiguous about this placement. |

### Unimplemented Features
- **`agentsync skill list`**: Documented as "not implemented". Confirmed that running the command returns `Error: list command not implemented`.

---

## 2. Behavioral Issues & Edge Cases

### Path Safety Check (`ensure_safe_destination`)
- **Observation**: Running `agentsync apply` with a destination like `CLAUDE.md` (directly in root) sometimes fails with `Destination path resolves outside project root: CLAUDE.md`.
- **Root Cause**: The `Linker` in `src/linker.rs` is extremely strict about path canonicalization and safety.
- **Documentation Gap**: The documentation should warn users that relative paths in `destination` should avoid leading `./` if it causes issues, or better explain how the project root is determined.

### Skill Update Requirements
- **Observation**: `agentsync skill update` fails if the `SKILL.md` manifest does not contain a `version` field.
- **Documentation Gap**: This requirement isn't explicitly mentioned in the `skills.mdx` guide as a prerequisite for the `update` command.

---

## 3. Configuration Discrepancies (`agentsync.toml`)

### Missing or Under-documented Fields
- **`default_agents`**: Mentioned in `cli.mdx` under `apply` behavior, but not prominently featured in `configuration.mdx`.
- **`compress_agents_md`**: Available in code and mentioned in some memories, but lacks detailed examples in the reference docs.

---

## 4. Diagnostics (`doctor`)
- **Observation**: The `doctor` command correctly identifies most issues (mismatched modes, unmanaged skills, gitignore drift).
- **Icon Consistency**: The CLI uses different icons (✔, ✗, ⚠, ✨) than what might be implied by plain text descriptions in docs.

---

## Recommendations
1. **Unify Flags**: Standardize on `--project-root` and `-p` across ALL commands in `src/main.rs`.
2. **Update Docs**: Explicitly list `skill list` as a "Coming Soon" or remove from active command lists if not intended for the current release.
3. **Enhance Manifest Documentation**: Add a section in `skills.mdx` about the mandatory `version` field for the `update` flow.
4. **Relax/Clarify Path Safety**: Investigate why `CLAUDE.md` sometimes triggers the "outside project root" error and improve the error message or documentation.
