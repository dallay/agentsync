# Gitignore Management Specification

## Purpose

Define the expected behavior for AgentSync-managed `.gitignore` entries, including how managed
entries are scoped, how user-authored entries are preserved, and how audit-style checks evaluate
generated output.

## Requirements

### Requirement: Root-Level Managed File Destinations Are Root-Scoped

The system MUST render auto-generated managed `.gitignore` entries for concrete repository-root file
destinations as root-scoped patterns.

For this requirement, a concrete repository-root file destination is an auto-generated managed entry
that represents a single file at the repository root and does not already contain a slash.

#### Scenario: Root-level managed file destination gains leading slash

- GIVEN managed `.gitignore` generation is enabled
- AND an auto-generated managed destination resolves to `AGENTS.md` at the repository root
- WHEN the managed `.gitignore` entries are assembled
- THEN the resulting managed entry MUST be `/AGENTS.md`
- AND the resulting managed entry MUST NOT be `AGENTS.md`

#### Scenario: Nested canonical file is not matched by root-scoped managed destination

- GIVEN managed `.gitignore` generation is enabled
- AND the managed output contains the root-scoped entry `/AGENTS.md`
- AND the repository contains `.agents/AGENTS.md`
- WHEN Git evaluates the managed ignore pattern
- THEN the managed entry MUST apply only to the repository-root `AGENTS.md`
- AND `.agents/AGENTS.md` MUST remain unaffected by that managed entry

### Requirement: Root-Level Managed Backup Entries Are Root-Scoped

The system MUST render auto-generated managed backup ignore entries for repository-root managed
files as root-scoped patterns.

#### Scenario: Root-level backup entry gains leading slash

- GIVEN managed `.gitignore` generation is enabled
- AND an auto-generated managed destination resolves to `AGENTS.md` at the repository root
- WHEN the corresponding backup ignore entry is assembled
- THEN the resulting managed backup entry MUST be `/AGENTS.md.bak`
- AND the resulting managed backup entry MUST NOT be `AGENTS.md.bak`

#### Scenario: Root-level backup entry does not match nested backup file

- GIVEN managed `.gitignore` generation is enabled
- AND the managed output contains `/AGENTS.md.bak`
- AND the repository contains `.agents/AGENTS.md.bak`
- WHEN Git evaluates the managed backup pattern
- THEN the managed backup entry MUST apply only to the repository-root backup file
- AND `.agents/AGENTS.md.bak` MUST remain unaffected by that managed entry

### Requirement: Known Root-Level Ignore Patterns Are Root-Scoped

The system MUST render known auto-generated managed ignore patterns for concrete repository-root
files as root-scoped patterns.

This requirement applies to built-in managed patterns whose intended location is the repository
root, including canonical root files and known generated root artifacts.

#### Scenario: Known root-level generated patterns are normalized

- GIVEN managed `.gitignore` generation is enabled
- AND the built-in managed ignore set includes root-level filenames such as `.mcp.json`,
  `opencode.json`, `CLAUDE.md`, `GEMINI.md`, and `WARP.md`
- WHEN the managed `.gitignore` entries are assembled
- THEN each root-level built-in managed filename MUST be emitted with a leading `/`

#### Scenario: Existing slash-containing known pattern remains as authored

- GIVEN managed `.gitignore` generation is enabled
- AND a built-in managed ignore pattern already contains a slash
- WHEN the managed `.gitignore` entries are assembled
- THEN that built-in managed ignore pattern MUST remain unchanged
- AND the system MUST NOT prepend an additional `/` solely because the entry is managed

### Requirement: Slash-Containing Generated Entries Remain Unchanged

The system MUST NOT rewrite an auto-generated managed `.gitignore` entry that already contains a
slash.

#### Scenario: Generated nested destination is preserved verbatim

- GIVEN managed `.gitignore` generation is enabled
- AND an auto-generated managed destination resolves to `src/api/CLAUDE.md`
- WHEN the managed `.gitignore` entries are assembled
- THEN the resulting managed entry MUST be `src/api/CLAUDE.md`
- AND the system MUST NOT rewrite it to `/src/api/CLAUDE.md`

#### Scenario: Generated nested backup entry is preserved verbatim

- GIVEN managed `.gitignore` generation is enabled
- AND an auto-generated managed backup entry resolves to `src/api/CLAUDE.md.bak`
- WHEN the managed `.gitignore` entries are assembled
- THEN the resulting managed backup entry MUST be `src/api/CLAUDE.md.bak`
- AND the system MUST NOT rewrite it to `/src/api/CLAUDE.md.bak`

### Requirement: Manual Gitignore Entries Remain Unchanged

The system MUST NOT normalize, reinterpret, or rewrite user-supplied `[gitignore].entries`.

User-authored entries SHALL retain their existing matching semantics exactly as configured, whether
they are root-scoped, slash-containing, or bare filenames.

#### Scenario: Manual bare filename remains bare

- GIVEN a user configures `[gitignore].entries = ["AGENTS.md"]`
- WHEN the effective `.gitignore` entries are assembled
- THEN the user-supplied entry MUST remain `AGENTS.md`
- AND the system MUST NOT rewrite it to `/AGENTS.md`

#### Scenario: Manual slash-containing pattern remains unchanged

- GIVEN a user configures `[gitignore].entries = ["docs/AGENTS.md"]`
- WHEN the effective `.gitignore` entries are assembled
- THEN the user-supplied entry MUST remain `docs/AGENTS.md`
- AND the system MUST NOT alter its spelling or scope

### Requirement: Gitignore Default Enablement Remains Unchanged

The system MUST keep `[gitignore].enabled = true` as the product default.

#### Scenario: Default config still enables managed gitignore generation

- GIVEN a configuration that does not explicitly set `[gitignore].enabled`
- WHEN the configuration is loaded
- THEN managed gitignore generation MUST remain enabled by default

### Requirement: Audit Uses The Same Normalized Managed Entries

Any doctor or audit behavior that evaluates managed `.gitignore` content MUST use the same
normalized managed entry set that rendering uses.

Doctor or audit checks MUST treat legacy unscoped managed root-file entries as drift when the
normalized managed form is root-scoped.

#### Scenario: Audit accepts normalized root-scoped managed entries

- GIVEN managed `.gitignore` generation is enabled
- AND the repository `.gitignore` managed section contains normalized entries such as `/AGENTS.md`
  and `/AGENTS.md.bak`
- WHEN `agentsync doctor` or an equivalent managed gitignore audit is run
- THEN the audit MUST treat those normalized entries as the expected managed output

#### Scenario: Audit flags legacy unscoped managed root entry as drift

- GIVEN managed `.gitignore` generation is enabled
- AND the repository `.gitignore` managed section contains a legacy unscoped managed entry
  `AGENTS.md`
- WHEN `agentsync doctor` or an equivalent managed gitignore audit is run
- THEN the audit MUST report that managed `.gitignore` content is out of sync with the expected
  normalized output
- AND the expected managed output MUST use `/AGENTS.md`
