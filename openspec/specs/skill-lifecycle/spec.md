# Specification: Skill Lifecycle

**Type**: RETROSPEC (updated with forward specs REQ-020–REQ-022)  
**Date**: 2026-04-05  
**Status**: ACTIVE  
**Source of Truth**: `src/skills/install.rs`, `src/skills/uninstall.rs`, `src/skills/update.rs`,
`src/skills/manifest.rs`, `src/skills/registry.rs`, `src/skills/provider.rs`,
`src/skills/transaction.rs`, `src/commands/skill.rs`, `src/commands/skill_fmt.rs`

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

### REQ-015: CLI Human Output Format

The CLI MUST support two output modes: human-readable (default) and JSON (`--json`).

The CLI MUST render human-readable output for `install`, `update`, and `uninstall` commands using
colored status lines with Unicode symbols and TTY-aware color gating.

#### Success output

On success, the CLI MUST print a single status line in the format:

```
{symbol} {verb} {skill_id}
```

| Command     | Symbol | Verb          |
|-------------|--------|---------------|
| `install`   | `✔`    | `installed`   |
| `update`    | `✔`    | `updated`     |
| `uninstall` | `✔`    | `uninstalled` |

When color is enabled, the status line MUST be rendered green and bold.
When color is disabled, the status line MUST be rendered as plain text with the Unicode symbol
preserved (no ANSI escape codes).

#### Error output

On error, the CLI MUST print two lines:

1. A failure status line: `✗ failed {skill_id}: {error_message}`
2. A hint line: `Hint: {remediation_message}`

When color is enabled, the failure status line MUST be rendered red and bold.
When color is disabled, both lines MUST be rendered as plain text with the Unicode symbol preserved.

The hint line MUST NOT be colored.

#### JSON output unchanged

JSON output (`--json`) MUST remain byte-identical to current behavior for all commands. No symbols,
no color, same schema, same field names, same serialization.

#### Install Success (JSON)

- JSON:
  `{"id": "{skill_id}", "name": ..., "description": ..., "version": ..., "files": [...], "manifest_hash": ..., "installed_at": ..., "status": "installed"}`

#### Install Error (JSON)

- JSON: `{"error": "{message}", "code": "install_error", "remediation": "{hint}"}`

#### Uninstall Success (JSON)

- JSON: `{"id": "{skill_id}", "status": "uninstalled"}`

#### Uninstall Error — Not Found (JSON)

- JSON:
  `{"error": "{message}", "code": "skill_not_found", "remediation": "Try 'list' to verify installed skills"}`

#### Uninstall Error — Validation (JSON)

- JSON:
  `{"error": "{message}", "code": "validation_error", "remediation": "Ensure the skill ID is valid..."}`

#### Update Success (JSON)

- JSON: `{"id": "{skill_id}", ..., "status": "updated"}`

#### Update Error (JSON)

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

#### Scenario: SC-015d — Install success in TTY with color

- GIVEN stdout is a TTY
- AND `NO_COLOR` is not set
- AND `CLICOLOR` is not `0`
- AND `TERM` is not `dumb`
- WHEN `agentsync skill install my-skill` succeeds
- THEN stdout MUST contain `✔ installed my-skill`
- AND the output MUST include ANSI green+bold escape codes around the status text

#### Scenario: SC-015e — Install success in non-TTY (piped)

- GIVEN stdout is NOT a TTY (e.g., piped to a file or another process)
- WHEN `agentsync skill install my-skill` succeeds
- THEN stdout MUST contain `✔ installed my-skill`
- AND the output MUST NOT contain any ANSI escape codes

#### Scenario: SC-015f — Update success in TTY with color

- GIVEN stdout is a TTY with color enabled
- WHEN `agentsync skill update my-skill` succeeds
- THEN stdout MUST contain `✔ updated my-skill`
- AND the output MUST include ANSI green+bold escape codes

#### Scenario: SC-015g — Uninstall success in TTY with color

- GIVEN stdout is a TTY with color enabled
- WHEN `agentsync skill uninstall my-skill` succeeds
- THEN stdout MUST contain `✔ uninstalled my-skill`
- AND the output MUST include ANSI green+bold escape codes

#### Scenario: SC-015h — Install error in TTY with color

- GIVEN stdout is a TTY with color enabled
- WHEN `agentsync skill install bad-skill` fails with error message `"source not found"`
- THEN stdout MUST contain `✗ failed bad-skill: source not found`
- AND the failure line MUST include ANSI red+bold escape codes
- AND stdout MUST contain a second line starting with `Hint:`

#### Scenario: SC-015i — Install error in non-TTY

- GIVEN stdout is NOT a TTY
- WHEN `agentsync skill install bad-skill` fails
- THEN stdout MUST contain `✗ failed bad-skill:` followed by the error message
- AND stdout MUST contain `Hint:` on a subsequent line
- AND the output MUST NOT contain any ANSI escape codes

#### Scenario: SC-015j — Uninstall not-found error in TTY

- GIVEN stdout is a TTY with color enabled
- AND skill `missing-skill` is not installed
- WHEN `agentsync skill uninstall missing-skill` is invoked
- THEN stdout MUST contain `✗ failed missing-skill:` followed by the not-found message
- AND stdout MUST contain `Hint:` with remediation mentioning `list`

#### Scenario: SC-015k — JSON output unchanged after refactor

- GIVEN `--json` flag is set
- WHEN any of `install`, `update`, or `uninstall` succeeds or fails
- THEN the JSON output MUST match the existing schema exactly (fields, types, values)
- AND the output MUST NOT contain Unicode symbols or ANSI escape codes

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

### REQ-020: TTY-Aware Color Gating

All human-mode output paths for `install`, `update`, and `uninstall` commands MUST determine
whether to use color using the same detection logic as `suggest --install`.

The color gating algorithm MUST evaluate the following inputs in order:

1. If `--json` is set, output mode is JSON — no color, no symbols.
2. Otherwise, detect color eligibility:
   - `stdout.is_terminal()` MUST return `true`
   - `NO_COLOR` environment variable MUST NOT be set to a non-empty value
   - `CLICOLOR` environment variable MUST NOT be set to `"0"`
   - `TERM` environment variable MUST NOT be `"dumb"` (case-insensitive)
3. Color is enabled only if ALL four conditions in step 2 are satisfied.

When color is disabled, Unicode symbols (`✔`, `✗`) MUST still be printed. Only ANSI escape
sequences MUST be suppressed.

The detection function MUST be reusable across all skill commands — it SHALL NOT be duplicated
per command.

#### Scenario: SC-020a — NO_COLOR suppresses color

- GIVEN `NO_COLOR=1` is set in the environment
- AND stdout is a TTY
- WHEN `agentsync skill install my-skill` succeeds
- THEN stdout MUST contain `✔ installed my-skill`
- AND the output MUST NOT contain ANSI escape codes

#### Scenario: SC-020b — CLICOLOR=0 suppresses color

- GIVEN `CLICOLOR=0` is set in the environment
- AND stdout is a TTY
- WHEN `agentsync skill update my-skill` succeeds
- THEN stdout MUST contain `✔ updated my-skill`
- AND the output MUST NOT contain ANSI escape codes

#### Scenario: SC-020c — TERM=dumb suppresses color

- GIVEN `TERM=dumb` is set in the environment
- AND stdout is a TTY
- WHEN `agentsync skill uninstall my-skill` succeeds
- THEN stdout MUST contain `✔ uninstalled my-skill`
- AND the output MUST NOT contain ANSI escape codes

#### Scenario: SC-020d — Color enabled when all conditions met

- GIVEN `NO_COLOR` is not set
- AND `CLICOLOR` is not set
- AND `TERM=xterm-256color`
- AND stdout is a TTY
- WHEN `agentsync skill install my-skill` succeeds
- THEN stdout MUST contain ANSI green+bold escape codes in the status line

#### Scenario: SC-020e — Non-TTY always suppresses color

- GIVEN stdout is NOT a TTY (piped)
- AND `NO_COLOR` is not set, `CLICOLOR` is not set, `TERM=xterm-256color`
- WHEN `agentsync skill install my-skill` succeeds
- THEN stdout MUST contain `✔ installed my-skill`
- AND the output MUST NOT contain ANSI escape codes

---

### REQ-021: Shared Formatting Abstractions

The colored status-line formatting logic MUST be extracted into command-agnostic types reusable
across all skill subcommands.

The shared module (`src/commands/skill_fmt.rs`) MUST provide:

1. **`LabelKind`** — an enum with variants: `Info`, `Warning`, `Success`, `Failure`.
2. **`HumanFormatter`** — a struct parameterized by `use_color: bool` that provides:
   - `format_label(symbol, label, kind) -> String` — returns the symbol+label with ANSI color
     when `use_color` is true, or plain text when false.
   - `format_heading(heading) -> String` — returns the heading bold when `use_color` is true,
     or plain text when false.
3. **`OutputMode`** — an enum with variants: `Json`, `Human { use_color: bool }`. This is the
   simplified mode for single-operation commands (no `HumanLive` variant needed).
4. **`detect_output_mode(json, stdout_is_tty, no_color, clicolor, term) -> OutputMode`** — the
   reusable detection function.

The `suggest --install` flow MUST be refactored to use these shared types. The existing
`SuggestInstallHumanFormatter` and `SuggestInstallLabelKind` types MUST be removed and all
references MUST point to the shared types.

The `SuggestInstallOutputMode` MAY retain its `HumanLive` variant as a local extension that wraps
the shared `OutputMode`, since spinners are specific to the suggest-install flow.

No `SuggestInstall*`-prefixed types SHOULD remain for functionality that is shared across commands.

> **Design note**: The spec originally proposed names `StatusLabelKind` and `SkillHumanFormatter`.
> During design, these were shortened to `LabelKind` and `HumanFormatter` because the module path
> (`commands::skill_fmt`) already provides sufficient namespace context. The implementation uses
> the shorter names.

#### Scenario: SC-021a — Formatter produces colored output when use_color is true

- GIVEN a `HumanFormatter` with `use_color = true`
- WHEN `format_label("✔", "installed", LabelKind::Success)` is called
- THEN the result MUST contain ANSI green+bold escape codes wrapping `✔ installed`

#### Scenario: SC-021b — Formatter produces plain output when use_color is false

- GIVEN a `HumanFormatter` with `use_color = false`
- WHEN `format_label("✔", "installed", LabelKind::Success)` is called
- THEN the result MUST be exactly `✔ installed` with no ANSI escape codes

#### Scenario: SC-021c — Formatter applies correct color per kind

- GIVEN a `HumanFormatter` with `use_color = true`
- WHEN `format_label` is called with each `LabelKind` variant
- THEN `Info` MUST produce cyan+bold, `Warning` MUST produce yellow+bold, `Success` MUST produce
  green+bold, `Failure` MUST produce red+bold

#### Scenario: SC-021d — detect_output_mode returns Json when json flag is true

- GIVEN `json = true`
- WHEN `detect_output_mode(true, true, None, None, Some("xterm"))` is called
- THEN the result MUST be `OutputMode::Json`

#### Scenario: SC-021e — detect_output_mode returns Human with color for normal TTY

- GIVEN `json = false`, stdout is TTY, no env overrides
- WHEN `detect_output_mode(false, true, None, None, Some("xterm-256color"))` is called
- THEN the result MUST be `OutputMode::Human { use_color: true }`

#### Scenario: SC-021f — detect_output_mode returns Human without color for non-TTY

- GIVEN `json = false`, stdout is NOT a TTY
- WHEN `detect_output_mode(false, false, None, None, Some("xterm-256color"))` is called
- THEN the result MUST be `OutputMode::Human { use_color: false }`

#### Scenario: SC-021g — suggest --install still works identically after refactor

- GIVEN the suggest-install flow uses the shared `HumanFormatter` and `LabelKind`
- WHEN `agentsync skill suggest --install` is invoked
- THEN the visual output MUST be identical to the pre-refactor behavior
- AND all existing suggest-install tests MUST pass without modification

---

### REQ-022: Hint Line Formatting Consistency

Error output for `install`, `update`, and `uninstall` commands MUST use a consistent two-line
format for human-mode errors.

Line 1 MUST be a failure status line rendered via the shared formatter:
`{formatted ✗ failed} {skill_id}: {error_message}`

Line 2 MUST be a plain-text hint line: `Hint: {remediation_message}`

The hint line MUST NOT be colored. The remediation message MUST be produced by the existing
`remediation_for_error()` function (or command-specific remediation logic for uninstall).

The `tracing::error!()` call for structured logging MUST be preserved — it is not part of user
output but is emitted to stderr for diagnostic purposes.

#### Scenario: SC-022a — Error output has two lines

- GIVEN a human-mode error from any skill command
- WHEN the error is rendered
- THEN stdout MUST contain exactly two relevant lines:
  1. A line matching `✗ failed {skill_id}: {message}`
  2. A line matching `Hint: {remediation}`

#### Scenario: SC-022b — Hint line is never colored

- GIVEN stdout is a TTY with color enabled
- WHEN a skill command fails
- THEN the `Hint:` line MUST NOT contain ANSI escape codes

#### Scenario: SC-022c — Uninstall not-found uses specific remediation

- GIVEN skill `missing` is not installed
- WHEN `agentsync skill uninstall missing` fails in human mode
- THEN the hint line MUST contain text about using `list` to verify installed skills

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
16. Human output for `install`, `update`, `uninstall` uses `✔`/`✗` symbols with green/red bold
    coloring when TTY+color conditions are met, plain text otherwise (REQ-015, REQ-020)
17. `detect_output_mode()` correctly evaluates `--json`, TTY, `NO_COLOR`, `CLICOLOR`, `TERM=dumb`
    and returns the appropriate `OutputMode` variant (REQ-020)
18. `LabelKind`, `HumanFormatter`, `OutputMode`, and `detect_output_mode` are shared across all
    skill commands; no `SuggestInstall*`-prefixed shared types remain (REQ-021)
19. Error output uses two-line format: colored failure line + plain `Hint:` line (REQ-022)
20. `suggest --install` output is visually identical after refactor to shared types (REQ-021)
16. Human output for `install`, `update`, `uninstall` uses colored status lines with `✔`/`✗` symbols
17. TTY-aware color gating respects `NO_COLOR`, `CLICOLOR=0`, `TERM=dumb`, and non-TTY stdout
18. Shared `LabelKind`, `HumanFormatter`, `OutputMode`, and `detect_output_mode` are used across
    all skill commands including `suggest --install`
19. Error output uses consistent two-line format (failure status + plain `Hint:` line)
20. JSON output remains byte-identical after refactor

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
| REQ-015     | `src/commands/skill.rs` — `run_install()`, `run_uninstall()`, `run_update()`; `src/commands/skill_fmt.rs` — `HumanFormatter`  | `tests/contracts/test_install_output.rs`: `install_json_contract`, `install_json_error_contract`; `src/commands/skill_fmt.rs` tests: `install_success_human_output_*`, `update_success_human_output_*`, `uninstall_success_human_output_*`, `install_error_human_output_*`, `uninstall_error_human_output_*` |
| REQ-016     | `src/commands/skill.rs` — `remediation_for_error()`                                                                          | Implicit via contract tests                                                                                                                                                                                              |
| REQ-017     | `src/commands/skill.rs` — `run_install()`, `run_uninstall()`, `run_update()` target root setup                               | Implicit via integration tests                                                                                                                                                                                           |
| REQ-018     | `src/commands/skill.rs` — `run_skill(SkillCommand::List, ...)`                                                               | `src/commands/skill.rs` inline test: `run_skill_list_returns_error`                                                                                                                                                      |
| REQ-019     | Multiple: `install_from_dir()` backup logic, `uninstall_skill()` NotFound, `update_skill_async()` version check              | Various tests above                                                                                                                                                                                                      |
| REQ-020     | `src/commands/skill_fmt.rs` — `detect_output_mode()`, `output_mode()`                                                        | `src/commands/skill_fmt.rs` tests: `detect_output_mode_no_color_env`, `detect_output_mode_no_color_any_nonempty_value`, `detect_output_mode_clicolor_zero`, `detect_output_mode_dumb_term`, `detect_output_mode_tty_with_color`, `detect_output_mode_no_tty_no_color` |
| REQ-021     | `src/commands/skill_fmt.rs` — `LabelKind`, `HumanFormatter`, `OutputMode`, `detect_output_mode`; `src/commands/skill.rs` — suggest-install refactored to use shared types | `src/commands/skill_fmt.rs` tests: `format_label_success_colored`, `format_label_success_plain`, `format_label_each_kind_uses_distinct_color`, `detect_output_mode_json_takes_priority`, `detect_output_mode_json_ignores_env_overrides`; `src/commands/skill.rs` tests: `suggest_install_output_mode_*` (6), `suggest_install_completion_summary_*` (2) |
| REQ-022     | `src/commands/skill.rs` — error paths in `run_install()`, `run_uninstall()`, `run_update()`; `src/commands/skill_fmt.rs` — `HumanFormatter` | `src/commands/skill_fmt.rs` tests: `install_error_human_output_pattern`, `uninstall_error_human_output_pattern`, `uninstall_error_colored_has_ansi_on_failure_line_only` |
