# Specification: Skill Lifecycle

**Type**: RETROSPEC  
**Date**: 2026-04-01  
**Status**: RETROSPEC  
**Source of Truth**: `src/skills/install.rs`, `src/skills/uninstall.rs`, `src/skills/update.rs`,
`src/skills/manifest.rs`, `src/skills/registry.rs`, `src/skills/provider.rs`,
`src/skills/transaction.rs`, `src/commands/skill.rs`

## Purpose

Define the behavior of the skill lifecycle system — installing, uninstalling, and updating skills
from local or remote sources. This spec covers the install flow (from directories, archives, and
URLs), uninstall flow (directory removal and registry cleanup), update flow (version comparison,
atomic swap, rollback), manifest validation, registry persistence, provider resolution, transaction
safety, CLI interface, and error handling.

This is a **retrospec** — every requirement and scenario is traced to existing code behavior and
verified by existing tests.

### Cross-References

- **Skill recommendations/suggest behavior**: partially specified in
  `openspec/specs/skill-recommendations/spec.md`
- **Skill adoption/detection**: partially specified in `openspec/specs/skill-adoption/spec.md`
- This spec focuses exclusively on the **install/uninstall/update lifecycle**, not recommendation
  or detection logic

---

## Data Model

### SkillManifest

Parsed from YAML frontmatter in `SKILL.md`:

| Field         | Type             | Required | Description                                             |
|---------------|------------------|----------|---------------------------------------------------------|
| `name`        | `String`         | Yes      | Skill identifier, must match `^[a-z0-9]+(-[a-z0-9]+)*$` |
| `version`     | `Option<String>` | No       | Semver version string (e.g., `"1.0.0"`)                 |
| `description` | `Option<String>` | No       | Human-readable description                              |

### SkillEntry (Registry)

Persisted per-skill in `registry.json`:

| Field           | Type                  | Serialized As  | Description                       |
|-----------------|-----------------------|----------------|-----------------------------------|
| `name`          | `Option<String>`      | `name`         | Display name from manifest        |
| `version`       | `Option<String>`      | `version`      | Installed version                 |
| `description`   | `Option<String>`      | `description`  | Description from manifest         |
| `provider`      | `Option<String>`      | `provider`     | Provider that resolved this skill |
| `source`        | `Option<String>`      | `source`       | Original source path/URL          |
| `installed_at`  | `Option<String>`      | `installedAt`  | RFC 3339 timestamp of install     |
| `files`         | `Option<Vec<String>>` | `files`        | List of installed files           |
| `manifest_hash` | `Option<String>`      | `manifestHash` | Hash of manifest content          |

### Registry

Top-level registry file structure (`registry.json`):

| Field            | Type                                   | Serialized As   | Description              |
|------------------|----------------------------------------|-----------------|--------------------------|
| `schema_version` | `u32`                                  | `schemaVersion` | Always `1`               |
| `last_updated`   | `Option<String>`                       | `last_updated`  | RFC 3339 timestamp       |
| `skills`         | `Option<BTreeMap<String, SkillEntry>>` | `skills`        | Map of skill_id to entry |

### InstalledSkillState

Derived view used for querying installed state:

| Field       | Type             | Description                           |
|-------------|------------------|---------------------------------------|
| `installed` | `bool`           | Always `true` for entries in registry |
| `version`   | `Option<String>` | Installed version                     |

### SkillInstallInfo (Provider)

Returned by provider resolution:

| Field          | Type     | Description                       |
|----------------|----------|-----------------------------------|
| `download_url` | `String` | URL or path to download the skill |
| `format`       | `String` | Archive format (e.g., `"zip"`)    |

### SkillInstallError

| Variant         | Description                        |
|-----------------|------------------------------------|
| `Io`            | Filesystem I/O error               |
| `Network`       | HTTP request error                 |
| `ZipArchive`    | Zip extraction error               |
| `Registry`      | Registry read/write error          |
| `PathTraversal` | Malicious path detected in archive |
| `Validation`    | Manifest validation failure        |
| `Other`         | Catch-all for unknown errors       |

### SkillUninstallError

| Variant      | Description                                    |
|--------------|------------------------------------------------|
| `Io`         | Filesystem I/O error                           |
| `Registry`   | Registry read/write error                      |
| `NotFound`   | Skill directory does not exist                 |
| `Validation` | Invalid skill ID (empty, path traversal, etc.) |

### SkillUpdateError

| Variant      | Description                                       |
|--------------|---------------------------------------------------|
| `Io`         | Filesystem I/O error                              |
| `Install`    | Wraps `SkillInstallError` from fetch/validate     |
| `Registry`   | Registry read/write error                         |
| `Atomic`     | Atomic rename/swap operation failed               |
| `Validation` | Version comparison or manifest validation failure |

### Provider Trait

| Method                     | Returns                                   | Description                       |
|----------------------------|-------------------------------------------|-----------------------------------|
| `manifest()`               | `Result<String>`                          | Provider identity string          |
| `resolve(id)`              | `Result<SkillInstallInfo>`                | Resolve skill ID to download info |
| `recommendation_catalog()` | `Result<Option<ProviderCatalogMetadata>>` | Optional catalog metadata         |

### CLI Subcommands

| Subcommand  | Args                             | Description                                 |
|-------------|----------------------------------|---------------------------------------------|
| `install`   | `skill_id`, `--source`, `--json` | Install a skill from provider or source     |
| `uninstall` | `skill_id`, `--json`             | Uninstall an installed skill                |
| `update`    | `skill_id`, `--source`, `--json` | Update a skill to a newer version           |
| `suggest`   | `--json`, `--install`, `--all`   | Suggest skills (see skill-recommendations)  |
| `list`      | (none)                           | List installed skills (not yet implemented) |

---

## Requirements

### REQ-001: Skill Manifest Parsing and Validation

The system MUST parse skill manifests from `SKILL.md` files containing YAML frontmatter delimited
by `---` markers.

The `name` field MUST be present and MUST match the pattern `^[a-z0-9]+(-[a-z0-9]+)*$` (lowercase
alphanumeric segments separated by single hyphens).

The `version` field is optional; if present, it MUST be a valid semver string.

The `description` field is optional.

If YAML frontmatter is missing, the system MUST return a validation error.

If the `name` field is missing or invalid, the system MUST return a validation error.

If the `version` field is present but not valid semver, the system MUST return a validation error.

#### Scenario: SC-001a — Valid manifest parsed successfully

- GIVEN a `SKILL.md` file with content `---\nname: sample-skill\ndescription: A skill\n---\n# Body`
- WHEN `parse_skill_manifest()` is called
- THEN it MUST return a `SkillManifest` with `name = "sample-skill"` and
  `description = Some("A skill")`

#### Scenario: SC-001b — Invalid name rejected

- GIVEN a `SKILL.md` file with content `---\nname: INVALID_NAME\n---\n# Body`
- WHEN `parse_skill_manifest()` is called
- THEN it MUST return a validation error containing "invalid name"

#### Scenario: SC-001c — Missing frontmatter rejected

- GIVEN a `SKILL.md` file without `---` frontmatter delimiters
- WHEN `parse_skill_manifest()` is called
- THEN it MUST return a validation error containing "missing frontmatter"

#### Scenario: SC-001d — Invalid semver rejected

- GIVEN a `SKILL.md` file with `version: "not-a-version"`
- WHEN `parse_skill_manifest()` is called
- THEN it MUST return a validation error containing "invalid semver"

---

### REQ-002: Install from Local Directory

The system MUST support installing skills from a local directory containing a valid `SKILL.md`.

The install process MUST:

1. Copy source directory contents to a staging temp directory
2. Validate the manifest in the staging directory
3. If the target skill directory already exists, back it up to `{skill_id}.backup`
4. Copy staged files to the final skill directory at `{target_root}/{skill_id}/`
5. Update the registry with the new skill entry

If manifest validation fails, the system MUST NOT create the skill directory (or MUST roll back
if it was partially created).

If copying to the final location fails, the system MUST restore the backup (if one was created).

On success, the system MUST clean up the backup directory.

#### Scenario: SC-002a — Successful install from directory

- GIVEN a directory containing a valid `SKILL.md` with `name: sample-skill`
- AND a target root directory exists
- WHEN `install_from_dir("sample-skill", src_dir, target_root)` is called
- THEN `{target_root}/sample-skill/SKILL.md` MUST exist
- AND the registry at `{target_root}/registry.json` MUST contain an entry for `"sample-skill"`

#### Scenario: SC-002b — Install preserves asset files

- GIVEN a directory containing `SKILL.md` and `assets/icon.png`
- WHEN `install_from_dir()` is called
- THEN both `SKILL.md` and `assets/icon.png` MUST exist in the installed skill directory

#### Scenario: SC-002c — Install with invalid manifest rolls back

- GIVEN a directory with an invalid `SKILL.md` (e.g., `name: "Invalid Name!"`)
- WHEN `install_from_dir()` is called
- THEN it MUST return an error
- AND the skill directory MUST NOT exist in the target root

#### Scenario: SC-002d — Re-install over existing skill

- GIVEN a skill `sample-skill` already installed at `{target_root}/sample-skill/`
- WHEN `install_from_dir("sample-skill", new_src, target_root)` is called with a valid source
- THEN a backup MUST be created at `{target_root}/sample-skill.backup`
- AND on success, the backup MUST be cleaned up
- AND the new skill content MUST be in `{target_root}/sample-skill/`

---

### REQ-003: Install from Zip Archive

The system MUST support installing skills from zip archives via `install_from_zip()`.

The system MUST extract zip contents to a temporary directory, then delegate to
`install_from_dir()`.

The system MUST reject zip entries with absolute paths (starting with `/`) or path traversal
components (`..`).

#### Scenario: SC-003a — Successful install from zip

- GIVEN a zip archive containing a valid `SKILL.md` with `name: sample-skill`
- WHEN `install_from_zip("sample-skill", reader, target_root)` is called
- THEN `{target_root}/sample-skill/SKILL.md` MUST exist

#### Scenario: SC-003b — Zip with path traversal rejected

- GIVEN a zip archive containing an entry with path `../../etc/passwd`
- WHEN `install_from_zip()` is called
- THEN it MUST return a `PathTraversal` error

#### Scenario: SC-003c — Zip with invalid manifest rolls back

- GIVEN a zip archive containing a `SKILL.md` with an invalid name field
- WHEN `install_from_zip("sample-skill-invalid", reader, target_root)` is called
- THEN it MUST return an error
- AND the directory `{target_root}/sample-skill-invalid/` MUST NOT exist

---

### REQ-004: Install from Remote URL (Fetch and Install)

The system MUST support installing skills from remote HTTP/HTTPS URLs via
`blocking_fetch_and_install_skill()`.

The system MUST support the following source types:

- HTTP/HTTPS URLs pointing to `.zip` or `.tar.gz` archives
- Local file paths (absolute or `file://` protocol)
- Local directories

For remote URLs, the system MUST stream the response to a temporary file to avoid buffering
the entire archive in memory.

For zip archives, the system MUST detect and strip common root directories (as GitHub zip
downloads include a `{repo}-{branch}/` prefix).

For tar.gz archives, the system MUST similarly detect and strip common root directories.

The system MUST support URL fragment subpaths (e.g., `https://example.com/archive.zip#subpath`)
to extract only a subdirectory from the archive.

After extraction, the system MUST use `find_best_skill_dir()` to locate the best directory
containing a `SKILL.md` manifest.

#### Scenario: SC-004a — Install from local directory path

- GIVEN a local directory path containing a valid `SKILL.md`
- WHEN `blocking_fetch_and_install_skill("my-skill", "/path/to/skill", target_root)` is called
- THEN the skill MUST be installed at `{target_root}/my-skill/`

#### Scenario: SC-004b — Install from file:// URL

- GIVEN a `file://` URL pointing to a local directory
- WHEN `blocking_fetch_and_install_skill()` is called
- THEN the directory contents MUST be copied and installed

#### Scenario: SC-004c — Empty file:// path rejected

- GIVEN a `file://` URL with an empty path
- WHEN `fetch_and_unpack_to_tempdir("file://")` is called
- THEN it MUST return a validation error

#### Scenario: SC-004d — Unknown archive format rejected

- GIVEN a source that is neither `.zip` nor `.tar.gz`
- WHEN `fetch_and_unpack_to_tempdir()` processes it
- THEN it MUST return an error "unknown archive format"

---

### REQ-005: Best Skill Directory Resolution

When an archive is unpacked, the system MUST locate the best directory to use as the skill root
via `find_best_skill_dir()`.

The resolution priority MUST be:

1. If `SKILL.md` exists at the archive root, use the root
2. If a subdirectory whose name matches `skill_id` contains `SKILL.md`, use that directory
3. If exactly one subdirectory anywhere contains `SKILL.md`, use that directory
4. Otherwise, fall back to the archive root

#### Scenario: SC-005a — SKILL.md at root

- GIVEN an unpacked archive with `SKILL.md` at the top level
- WHEN `find_best_skill_dir(temp_path, "my-skill")` is called
- THEN it MUST return `temp_path`

#### Scenario: SC-005b — SKILL.md in matching subdirectory

- GIVEN an unpacked archive with `my-skill/SKILL.md` and `other-skill/SKILL.md`
- WHEN `find_best_skill_dir(temp_path, "my-skill")` is called
- THEN it MUST return `temp_path/my-skill`

#### Scenario: SC-005c — Single SKILL.md in non-matching subdirectory

- GIVEN an unpacked archive with only `some-dir/SKILL.md`
- WHEN `find_best_skill_dir(temp_path, "my-skill")` is called
- THEN it MUST return `temp_path/some-dir`

---

### REQ-006: Skill Uninstall

The system MUST remove a skill by deleting its directory and updating the registry.

The uninstall process MUST:

1. Validate the `skill_id` (see REQ-009)
2. Check that the skill directory exists; return `NotFound` error if not
3. Remove the skill entry from `registry.json` (if the registry exists and contains the entry)
4. Remove the skill directory via `fs::remove_dir_all()`

The registry MUST be updated before the directory is removed.

If the skill entry is not present in the registry but the directory exists, the registry write
MUST be skipped (no-op) and directory removal MUST proceed.

If directory removal fails after registry update, the system MUST log a warning and return an
I/O error.

#### Scenario: SC-006a — Successful uninstall

- GIVEN a skill `test-skill` installed at `{target_root}/test-skill/`
- AND a registry entry for `test-skill` in `{target_root}/registry.json`
- WHEN `uninstall_skill("test-skill", target_root)` is called
- THEN the skill directory MUST be removed
- AND the registry entry for `test-skill` MUST be removed
- AND it MUST return `Ok(())`

#### Scenario: SC-006b — Uninstall non-existent skill

- GIVEN no skill directory exists for `non-existent-skill`
- WHEN `uninstall_skill("non-existent-skill", target_root)` is called
- THEN it MUST return `Err(SkillUninstallError::NotFound(_))`

#### Scenario: SC-006c — Uninstall without registry file

- GIVEN a skill directory exists but no `registry.json` file
- WHEN `uninstall_skill("test-skill", target_root)` is called
- THEN the skill directory MUST still be removed
- AND it MUST return `Ok(())`

---

### REQ-007: Skill Update with Version Comparison

The system MUST support updating an installed skill to a newer version via `update_skill_async()`.

The update process MUST:

1. Resolve the current installed version from the registry (fallback to `SKILL.md` in the
   existing skill directory, fallback to `0.0.0`)
2. Parse the candidate version from the update source's `SKILL.md`
3. Reject the update if the candidate version is not strictly greater than the installed version
4. Atomically rename the existing skill directory to `{skill_id}.bak`
5. Copy the update source to the skill directory
6. Validate the new manifest
7. Update the registry entry
8. Clean up the backup directory on success

The candidate `SKILL.md` MUST have a `version` field; if missing, the system MUST return a
validation error.

The candidate version MUST be valid semver; if not, the system MUST return a validation error.

#### Scenario: SC-007a — Successful update from v1 to v2

- GIVEN a skill installed at version `1.0.0`
- AND an update source with version `2.0.0`
- WHEN `update_skill_async("sample-skill", target_root, update_source)` is called
- THEN the skill directory MUST contain the v2 content
- AND the registry MUST reflect version `2.0.0`
- AND the backup `{skill_id}.bak` MUST be cleaned up

#### Scenario: SC-007b — Update rejected when version not greater

- GIVEN a skill installed at version `2.0.0`
- AND an update source with version `1.0.0`
- WHEN `update_skill_async()` is called
- THEN it MUST return `Err(SkillUpdateError::Validation(_))`
- AND the error message MUST contain "not greater than installed"
- AND the existing installation MUST remain unchanged

#### Scenario: SC-007c — Update rejected when same version

- GIVEN a skill installed at version `1.0.0`
- AND an update source with version `1.0.0`
- WHEN `update_skill_async()` is called
- THEN it MUST return a validation error (candidate <= installed)

#### Scenario: SC-007d — Update rejected when candidate has no version

- GIVEN an update source with `SKILL.md` missing the `version` field
- WHEN `update_skill_async()` is called
- THEN it MUST return `Err(SkillUpdateError::Validation("missing version in SKILL.md"))`

---

### REQ-008: Update Rollback on Failure

The update process MUST roll back to the previous state if any step fails after the atomic rename.

If manifest validation fails on the new skill:

1. The new skill directory MUST be removed
2. The backup MUST be renamed back to the original skill directory

If registry update fails:

1. The new skill directory MUST be removed
2. The backup MUST be renamed back to the original skill directory
3. The previous registry entry MUST be restored (if one existed)

#### Scenario: SC-008a — Rollback on invalid new manifest

- GIVEN a skill `sample-skill` installed with valid v1
- AND an update source with a broken `SKILL.md` (missing version for update validation)
- WHEN `update_skill_async()` is called
- THEN it MUST return an error
- AND the original v1 skill directory MUST be restored
- AND the backup directory MUST NOT remain

#### Scenario: SC-008b — Rollback restores previous registry entry

- GIVEN a skill `sample-skill` with registry entry at version `1.0.0`
- AND the registry update step fails during update
- WHEN rollback occurs
- THEN the registry MUST be restored to contain the original entry

---

### REQ-009: Skill ID Validation

The system MUST validate skill IDs at multiple layers to prevent path traversal attacks.

In the CLI layer (`commands/skill.rs`), the system MUST reject skill IDs that:

- Are empty
- Contain `/` or `\` path separators
- Are absolute paths
- Contain `.` (current dir) or `..` (parent dir) components
- Consist of more than one path segment

In the uninstall layer (`uninstall.rs`), the system MUST independently validate skill IDs with
the same constraints.

#### Scenario: SC-009a — Valid skill IDs accepted

- GIVEN skill IDs `"weather-skill"`, `"hello"`, `"a"`, `"skill_123"`
- WHEN validated
- THEN all MUST pass validation

#### Scenario: SC-009b — Path separator rejected

- GIVEN a skill ID `"foo/bar"` or `"foo\\bar"`
- WHEN validated
- THEN it MUST return a validation error

#### Scenario: SC-009c — Empty string rejected

- GIVEN a skill ID `""`
- WHEN validated
- THEN it MUST return a validation error

#### Scenario: SC-009d — Dot and dot-dot rejected

- GIVEN skill IDs `"."` and `".."`
- WHEN validated
- THEN both MUST return validation errors

#### Scenario: SC-009e — Absolute paths rejected

- GIVEN a skill ID `"/abs/path"`
- WHEN validated
- THEN it MUST return a validation error

---

### REQ-010: Registry Persistence

The system MUST persist installed skill state in `registry.json` at the skills root directory.

The registry file MUST use JSON format with pretty printing.

The `schemaVersion` MUST always be `1`.

The `last_updated` field MUST be set to the current UTC time in RFC 3339 format on every write.

The `skills` map MUST use `BTreeMap` for deterministic key ordering.

#### Write Registry

`write_registry()` MUST create a new empty registry file, creating parent directories if needed.

#### Read Registry

`read_registry()` MUST deserialize the registry file and return the `Registry` struct.

#### Update Entry

`update_registry_entry()` MUST:

1. Read the existing registry (or create a new one if the file doesn't exist)
2. Insert or replace the entry for the given `skill_id`
3. Update `last_updated`
4. Write the registry back to disk

#### Read Installed States

`read_installed_skill_states()` MUST return a `BTreeMap<String, InstalledSkillState>` with
`installed: true` for every entry in the registry. If the registry file does not exist, it
MUST return an empty map.

#### Scenario: SC-010a — Write and read registry round-trip

- GIVEN no existing registry file
- WHEN `write_registry(path)` is called
- THEN the file MUST contain `"schemaVersion"` and valid JSON
- AND `read_registry(path)` MUST return a `Registry` with `schema_version = 1`

#### Scenario: SC-010b — Update registry entry

- GIVEN an existing empty registry
- AND a `SkillEntry` with `name: "sample"`, `version: "1.0"`, `provider: "skills.sh"`
- WHEN `update_registry_entry(path, "sample", entry)` is called
- THEN the registry file MUST contain `"sample"` as a key
- AND the entry MUST have the provided fields

#### Scenario: SC-010c — Update registry creates file if missing

- GIVEN no registry file exists
- WHEN `update_registry_entry(path, "skill-id", entry)` is called
- THEN the file MUST be created with the entry

#### Scenario: SC-010d — Read installed states from missing registry

- GIVEN a registry file that does not exist
- WHEN `read_installed_skill_states(path)` is called
- THEN it MUST return an empty `BTreeMap`

---

### REQ-011: Provider Resolution (skills.sh)

The `SkillsShProvider` MUST resolve skill IDs by querying the skills.sh search API.

The provider MUST:

1. Send a GET request to `https://skills.sh/api/search?q={encoded_skill_id}`
2. Set a 10-second timeout on the HTTP client
3. Parse the response as a `SearchResponse` containing a list of skills
4. Find the best match: exact ID match preferred, then match on the last segment of the ID
5. Construct a GitHub archive download URL from the `source` field (`owner/repo`)
6. Compute a subpath if the skill ID extends beyond the source path
7. For repos named `skills`, `agent-skills`, or `agentic-skills`, prefix the subpath with `skills/`
8. Return a `SkillInstallInfo` with the download URL (including `#subpath` fragment if applicable)

If no matching skill is found, the provider MUST return an error "Skill not found on skills.sh".

#### Scenario: SC-011a — Provider resolves to GitHub zip URL

- GIVEN a skill with `id: "owner/repo/my-skill"` and `source: "owner/repo"`
- WHEN `resolve("my-skill")` finds a match
- THEN the download URL MUST be `https://github.com/owner/repo/archive/HEAD.zip#my-skill`

#### Scenario: SC-011b — Provider returns not found

- GIVEN no skill matching the query exists on skills.sh
- WHEN `resolve("nonexistent")` is called
- THEN it MUST return an error containing "Skill not found on skills.sh"

---

### REQ-012: CLI Source Resolution

The CLI MUST resolve the skill source through `resolve_source()` with the following priority:

1. If `--source` is provided, use it directly (converting GitHub URLs to zip format if applicable)
2. If the skill ID looks like a URL or path (contains `://`, starts with `/` or `.`), use it as-is
3. Otherwise, resolve via `SkillsShProvider`

#### Scenario: SC-012a — Explicit source used directly

- GIVEN `--source /path/to/skill`
- WHEN `resolve_source("my-skill", Some("/path/to/skill"))` is called
- THEN it MUST return `"/path/to/skill"`

#### Scenario: SC-012b — GitHub URL auto-converted

- GIVEN `--source https://github.com/obra/superpowers`
- WHEN `resolve_source()` is called
- THEN it MUST return `"https://github.com/obra/superpowers/archive/HEAD.zip"`

#### Scenario: SC-012c — GitHub tree URL with subpath converted

- GIVEN `--source https://github.com/obra/superpowers/tree/main/skills/systematic-debugging`
- WHEN `resolve_source()` is called
- THEN it MUST return
  `"https://github.com/obra/superpowers/archive/refs/heads/main.zip#skills/systematic-debugging"`

#### Scenario: SC-012d — Already-archive URLs pass through

- GIVEN `--source https://github.com/owner/repo/archive/HEAD.zip`
- WHEN `resolve_source()` is called
- THEN it MUST return the URL unchanged

#### Scenario: SC-012e — Bare skill ID resolved via provider

- GIVEN no `--source` and skill ID `"my-skill"` (no URL or path indicators)
- WHEN `resolve_source("my-skill", None)` is called
- THEN it MUST attempt resolution via `SkillsShProvider`

---

### REQ-013: Transaction Rollback Helper

The system MUST provide a `with_rollback()` function that:

1. Executes an operation function
2. If the operation succeeds, returns `Ok(())`
3. If the operation fails, executes a cleanup function and returns the original error

The cleanup function MUST be called exactly once on failure and MUST NOT be called on success.

#### Scenario: SC-013a — Rollback on failure

- GIVEN an operation that returns an error
- AND a cleanup function that removes a directory
- WHEN `with_rollback(op, cleanup)` is called
- THEN the cleanup function MUST be called
- AND the directory MUST be removed
- AND the result MUST be `Err`

#### Scenario: SC-013b — No rollback on success

- GIVEN an operation that returns `Ok(())`
- AND a cleanup function
- WHEN `with_rollback(op, cleanup)` is called
- THEN the cleanup function MUST NOT be called
- AND the result MUST be `Ok(())`

---

### REQ-014: Archive Path Traversal Protection

The system MUST reject archive entries that attempt path traversal.

For zip archives, the system MUST reject entries whose filenames:

- Start with `/` (absolute path)
- Contain `..` (parent directory traversal)

For tar.gz archives, the system MUST reject entries whose paths contain:

- `ParentDir` components (`..`)
- `RootDir` components (`/`)
- `Prefix` components (Windows drive letters)

#### Scenario: SC-014a — Zip with absolute path rejected

- GIVEN a zip entry with filename `/etc/passwd`
- WHEN extraction is attempted
- THEN a `PathTraversal` error MUST be returned

#### Scenario: SC-014b — Zip with parent traversal rejected

- GIVEN a zip entry with filename `../../etc/passwd`
- WHEN extraction is attempted
- THEN a `PathTraversal` error MUST be returned

#### Scenario: SC-014c — Tar.gz with parent dir rejected

- GIVEN a tar.gz entry with a `ParentDir` component
- WHEN extraction is attempted
- THEN a `PathTraversal` error MUST be returned

---

### REQ-015: CLI Output Format

The CLI MUST support two output modes: human-readable (default) and JSON (`--json`).

#### Install Success

- Human: `"Installed {skill_id}"`
- JSON:
  `{"id": "{skill_id}", "name": ..., "description": ..., "version": ..., "files": [...], "manifest_hash": ..., "installed_at": ..., "status": "installed"}`

#### Install Error

- Human: error log with hint message
- JSON: `{"error": "{message}", "code": "install_error", "remediation": "{hint}"}`

#### Uninstall Success

- Human: `"Uninstalled {skill_id}"`
- JSON: `{"id": "{skill_id}", "status": "uninstalled"}`

#### Uninstall Error (Not Found)

- Human: `"Hint: Skill '{skill_id}' is not installed. Use 'list' to see installed skills."`
- JSON:
  `{"error": "{message}", "code": "skill_not_found", "remediation": "Try 'list' to verify installed skills"}`

#### Uninstall Error (Validation)

- JSON:
  `{"error": "{message}", "code": "validation_error", "remediation": "Ensure the skill ID is valid..."}`

#### Update Success

- Human: `"Updated {skill_id}"`
- JSON: `{"id": "{skill_id}", ..., "status": "updated"}`

#### Update Error

- JSON: `{"error": "{message}", "code": "update_error", "remediation": "{hint}"}`

#### Scenario: SC-015a — JSON install output contract

- GIVEN a successful install
- WHEN `--json` flag is set
- THEN the output MUST be valid JSON with fields `id`, `status`, `name`, `description`, `version`,
  `files`, `manifest_hash`, `installed_at`
- AND `status` MUST be `"installed"`

#### Scenario: SC-015b — JSON error output contract

- GIVEN a failed install
- WHEN `--json` flag is set
- THEN the output MUST be valid JSON with fields `error`, `code`, `remediation`
- AND `code` MUST NOT be `"unknown"`
- AND `error` MUST NOT be empty
- AND `remediation` MUST NOT be empty

#### Scenario: SC-015c — JSON uninstall not-found output

- GIVEN an attempt to uninstall a non-existent skill
- WHEN `--json` flag is set
- THEN the output MUST include `"code": "skill_not_found"`

---

### REQ-016: Error Remediation Messages

The CLI MUST provide context-aware remediation hints based on error message content.

| Error contains                      | Remediation hint                                                |
|-------------------------------------|-----------------------------------------------------------------|
| `"manifest"`                        | Check SKILL.md syntax, frontmatter, and name field requirements |
| `"network"`, `"download"`, `"HTTP"` | Check network connection and source URL                         |
| `"archive"`                         | Verify archive is valid zip or tar.gz                           |
| `"permission"`                      | Check file permissions                                          |
| `"registry"`                        | Ensure write access and registry file integrity                 |
| (other)                             | Generic: run with increased verbosity or check documentation    |

#### Scenario: SC-016a — Manifest error gets manifest remediation

- GIVEN an error message containing "manifest"
- WHEN `remediation_for_error()` is called
- THEN it MUST return a hint mentioning "SKILL.md syntax"

---

### REQ-017: Skills Target Root Convention

The CLI MUST install skills to `{project_root}/.agents/skills/`.

The CLI MUST create the target root directory if it does not exist before installing.

The registry file MUST be located at `{project_root}/.agents/skills/registry.json`.

#### Scenario: SC-017a — Target root created on install

- GIVEN a project root without `.agents/skills/`
- WHEN `skill install my-skill` is called
- THEN the directory `.agents/skills/` MUST be created

---

### REQ-018: List Command (Not Implemented)

The `skill list` subcommand MUST return an error indicating it is not yet implemented.

#### Scenario: SC-018a — List returns error

- GIVEN the `list` subcommand is invoked
- WHEN `run_skill(SkillCommand::List, project_root)` is called
- THEN it MUST return an error containing "list command not implemented"

---

### REQ-019: Idempotency Behavior

#### Install Idempotency

When installing a skill that already exists, the system MUST overwrite the existing installation.
The existing directory is backed up to `{skill_id}.backup`, the new version is installed, and on
success the backup is cleaned up. This means re-running install with identical content succeeds
and produces the same final state.

#### Uninstall Idempotency

When uninstalling a skill that does not exist, the system MUST return a `NotFound` error. The
uninstall operation is NOT idempotent — a second call will fail.

When uninstalling a skill whose registry entry was already removed but whose directory still
exists, the system MUST still remove the directory (the registry removal is a no-op).

#### Update Idempotency

When updating a skill with a version that is not strictly greater than the installed version,
the system MUST reject the update. Re-running update with the same version will always fail.

#### Scenario: SC-019a — Re-install succeeds

- GIVEN a skill `sample-skill` already installed
- WHEN `install_from_dir("sample-skill", same_source, target_root)` is called again
- THEN it MUST succeed
- AND the skill directory MUST contain the current content

#### Scenario: SC-019b — Uninstall of missing skill fails

- GIVEN no skill `missing` is installed
- WHEN `uninstall_skill("missing", target_root)` is called
- THEN it MUST return `NotFound`

---

## Non-Functional Requirements

### NF-1: Security — Path Traversal Prevention

Skill IDs MUST be validated at CLI and library layers to prevent directory traversal. Archive
extraction MUST reject malicious paths. These checks form defense-in-depth: both the CLI
(`validate_skill_id`) and library (`uninstall.rs` validation, archive extraction) independently
enforce safety.

### NF-2: Atomicity — Backup and Rollback

Install and update operations MUST use a backup-and-restore pattern to ensure that failures
leave the system in a consistent state. The `with_rollback()` helper and inline rollback logic
in `install_from_dir()` and `update_skill_async()` implement this guarantee.

### NF-3: Streaming Downloads

Remote archive downloads MUST stream to a temporary file rather than buffering entirely in
memory, to support large skill archives without excessive memory consumption.

### NF-4: Deterministic Registry Ordering

The registry MUST use `BTreeMap` for the skills map to ensure deterministic JSON output ordering
across writes.

### NF-5: Tokio Runtime Compatibility

Functions that require async (HTTP fetch, update) MUST detect whether a Tokio runtime is already
active and reuse it (`Handle::try_current().block_on()`) to avoid panics from nested runtime
creation.

---

## Acceptance Criteria

1. `parse_skill_manifest()` correctly parses valid YAML frontmatter and rejects invalid names,
   missing frontmatter, and invalid semver
2. `install_from_dir()` copies skill contents, validates manifest, updates registry, and rolls back
   on failure
3. `install_from_zip()` extracts archives safely with path traversal protection and delegates to
   `install_from_dir()`
4. `blocking_fetch_and_install_skill()` handles local paths, `file://` URLs, HTTP URLs, zip, and
   tar.gz formats
5. `find_best_skill_dir()` prioritizes root SKILL.md, then matching subdirectory, then sole manifest
6. `uninstall_skill()` validates ID, removes registry entry, removes directory, and returns
   `NotFound` for missing skills
7. `update_skill_async()` compares semver versions, rejects non-upgrades, performs atomic swap with
   backup, and rolls back on failure
8. `with_rollback()` calls cleanup on failure and skips cleanup on success
9. Zip and tar.gz archives reject entries with path traversal components
10. CLI validates skill IDs at the command layer before dispatching to library functions
11. CLI outputs valid JSON with `id`, `status`, `error`, `code`, `remediation` fields as appropriate
12. Registry uses `BTreeMap` ordering, pretty JSON, RFC 3339 timestamps, and `schemaVersion: 1`
13. `SkillsShProvider` queries `skills.sh/api/search`, constructs GitHub archive URLs with subpath
    fragments
14. GitHub URL conversion handles simple repos, tree/blob URLs, and already-archive URLs
15. All existing tests pass (no regressions)

---

## Code References

| Requirement | Primary Code Location                                                                                                        | Test Coverage                                                                                                                                                                                                            |
|-------------|------------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| REQ-001     | `src/skills/manifest.rs` — `parse_skill_manifest()`                                                                          | `tests/unit/manifest.rs`: `parse_valid_manifest`, `reject_invalid_name`                                                                                                                                                  |
| REQ-002     | `src/skills/install.rs` — `install_from_dir()`                                                                               | `tests/integration/skill_install.rs`: `integration_skill_install_fixture`                                                                                                                                                |
| REQ-003     | `src/skills/install.rs` — `install_from_zip()`                                                                               | `tests/unit/install.rs`: `install_from_zip_safety`; `tests/integration/skill_install.rs`: `integration_skill_install_invalid_manifest_rollback`                                                                          |
| REQ-004     | `src/skills/install.rs` — `blocking_fetch_and_install_skill()`, `fetch_and_unpack_to_tempdir()`                              | Implicit via integration tests                                                                                                                                                                                           |
| REQ-005     | `src/skills/install.rs` — `find_best_skill_dir()`                                                                            | Implicit via install integration tests                                                                                                                                                                                   |
| REQ-006     | `src/skills/uninstall.rs` — `uninstall_skill()`, `remove_registry_entry()`                                                   | `src/skills/uninstall.rs` inline tests: `test_uninstall_skill_success`, `test_uninstall_skill_not_found`, `test_uninstall_skill_invalid_id`, `test_uninstall_skill_dot_rejected`, `test_uninstall_skill_dotdot_rejected` |
| REQ-007     | `src/skills/update.rs` — `update_skill_async()`                                                                              | `tests/integration/skill_update.rs`: `integration_skill_update_success` (scaffold); fixture files: `sample-skill-v1`, `sample-skill-v2`, `sample-skill-v2-invalid`                                                       |
| REQ-008     | `src/skills/update.rs` — rollback branches in `update_skill_async()`                                                         | `tests/integration/skill_update.rs`: `integration_skill_update_rollback_on_invalid_new` (scaffold)                                                                                                                       |
| REQ-009     | `src/commands/skill.rs` — `validate_skill_id()`; `src/skills/uninstall.rs` — ID validation                                   | `src/commands/skill.rs` inline tests: `validate_skill_id_accepts_simple_names`, `validate_skill_id_rejects_invalid_inputs`; `src/skills/uninstall.rs` inline tests                                                       |
| REQ-010     | `src/skills/registry.rs` — `write_registry()`, `read_registry()`, `update_registry_entry()`, `read_installed_skill_states()` | `tests/unit/registry.rs`: `write_and_read_registry`                                                                                                                                                                      |
| REQ-011     | `src/skills/provider.rs` — `SkillsShProvider::resolve()`                                                                     | `tests/unit/provider.rs`: `dummy_provider_resolves` (trait contract)                                                                                                                                                     |
| REQ-012     | `src/commands/skill.rs` — `resolve_source()`, `try_convert_github_url()`                                                     | `src/commands/skill.rs` inline tests: `github_url_converter_*` (8 tests)                                                                                                                                                 |
| REQ-013     | `src/skills/transaction.rs` — `with_rollback()`                                                                              | `tests/unit/transaction.rs`: `rollback_on_failure_deletes_dir`, `no_rollback_on_success`                                                                                                                                 |
| REQ-014     | `src/skills/install.rs` — zip/tar extraction with path checks                                                                | `tests/integration/skill_install.rs`: `integration_skill_install_invalid_manifest_rollback` (partial)                                                                                                                    |
| REQ-015     | `src/commands/skill.rs` — `run_install()`, `run_uninstall()`, `run_update()` JSON branches                                   | `tests/contracts/test_install_output.rs`: `install_json_contract`, `install_json_error_contract`                                                                                                                         |
| REQ-016     | `src/commands/skill.rs` — `remediation_for_error()`                                                                          | Implicit via contract tests                                                                                                                                                                                              |
| REQ-017     | `src/commands/skill.rs` — `run_install()`, `run_uninstall()`, `run_update()` target root setup                               | Implicit via integration tests                                                                                                                                                                                           |
| REQ-018     | `src/commands/skill.rs` — `run_skill(SkillCommand::List, ...)`                                                               | `src/commands/skill.rs` inline test: `run_skill_list_returns_error`                                                                                                                                                      |
| REQ-019     | Multiple: `install_from_dir()` backup logic, `uninstall_skill()` NotFound, `update_skill_async()` version check              | Various tests above                                                                                                                                                                                                      |
